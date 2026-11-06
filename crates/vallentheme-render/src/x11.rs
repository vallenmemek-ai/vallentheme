//! Pembuatan window "desktop" di X11.
//!
//! Window ini dipetakan seukuran monitor, diberi tipe `_NET_WM_WINDOW_TYPE_DESKTOP`
//! dan state `BELOW | STICKY | SKIP_TASKBAR | SKIP_PAGER`, lalu diturunkan ke
//! dasar tumpukan. mpv kemudian menggambar ke dalamnya lewat `--wid`.
//!
//! Rujukan: `panduan/06-referensi-teknis.md` bagian 3.

use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateGCAux, CreateWindowAux, EventMask, PropMode, Rectangle,
    StackMode, Window, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::COPY_DEPTH_FROM_PARENT;

use crate::error::{x11_err, RenderError, Result};

/// Nilai `_NET_WM_DESKTOP` yang berarti "tampil di semua workspace".
pub const ALL_DESKTOPS: u32 = 0xFFFF_FFFF;

/// Satu monitor yang dilaporkan RandR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
    /// Nama konektor, misalnya `LVDS-1`.
    pub name: String,
    /// Posisi X di layar virtual.
    pub x: i16,
    /// Posisi Y di layar virtual.
    pub y: i16,
    /// Lebar dalam piksel.
    pub width: u16,
    /// Tinggi dalam piksel.
    pub height: u16,
    /// Monitor utama.
    pub primary: bool,
}

impl MonitorInfo {
    /// Lebar dikali tinggi, untuk memilih monitor terbesar bila perlu.
    pub fn area(&self) -> u32 {
        u32::from(self.width) * u32::from(self.height)
    }
}

/// Koneksi X11 beserta informasi layar.
pub struct X11Context {
    /// Koneksi ke server X.
    pub conn: RustConnection,
    /// Nomor layar yang dipakai.
    pub screen_num: usize,
    /// Window root layar tersebut.
    pub root: Window,
}

impl X11Context {
    /// Hubungkan ke server X sesuai variabel `DISPLAY`.
    pub fn connect() -> Result<Self> {
        if std::env::var_os("DISPLAY").is_none() {
            return Err(RenderError::NotX11);
        }
        let (conn, screen_num) =
            x11rb::connect(None).map_err(|e| RenderError::X11Connect(e.to_string()))?;
        let root = conn
            .setup()
            .roots
            .get(screen_num)
            .ok_or(RenderError::NoMonitors)?
            .root;
        Ok(Self {
            conn,
            screen_num,
            root,
        })
    }

    /// Daftar monitor menurut RandR.
    pub fn monitors(&self) -> Result<Vec<MonitorInfo>> {
        let reply = self
            .conn
            .randr_get_monitors(self.root, false)
            .map_err(|e| x11_err("randr_get_monitors", e))?
            .reply()
            .map_err(|e| x11_err("randr_get_monitors", e))?;

        let mut daftar = Vec::with_capacity(reply.monitors.len());
        for mon in reply.monitors {
            daftar.push(MonitorInfo {
                name: self.atom_name(mon.name)?,
                x: mon.x,
                y: mon.y,
                width: mon.width,
                height: mon.height,
                primary: mon.primary,
            });
        }
        if daftar.is_empty() {
            return Err(RenderError::NoMonitors);
        }
        Ok(daftar)
    }

    /// Ambil nama sebuah atom.
    pub fn atom_name(&self, atom: u32) -> Result<String> {
        let reply = self
            .conn
            .get_atom_name(atom)
            .map_err(|e| x11_err("get_atom_name", e))?
            .reply()
            .map_err(|e| x11_err("get_atom_name", e))?;
        Ok(String::from_utf8_lossy(&reply.name).into_owned())
    }

    /// Intern sebuah atom dari nama.
    pub fn atom(&self, name: &str) -> Result<u32> {
        Ok(self
            .conn
            .intern_atom(false, name.as_bytes())
            .map_err(|e| x11_err("intern_atom", e))?
            .reply()
            .map_err(|e| x11_err("intern_atom", e))?
            .atom)
    }

    /// Buat window desktop untuk satu monitor.
    ///
    /// `override_redirect = true` melewati window manager; dipakai sebagai
    /// percobaan cadangan bila WM mengganggu.
    pub fn create_desktop_window(
        &self,
        monitor: &MonitorInfo,
        background: u32,
        override_redirect: bool,
    ) -> Result<Window> {
        let win = self
            .conn
            .generate_id()
            .map_err(|e| x11_err("generate_id", e))?;

        let aux = CreateWindowAux::new()
            .background_pixel(background)
            .border_pixel(0)
            .override_redirect(u32::from(override_redirect))
            .event_mask(
                EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY | EventMask::PROPERTY_CHANGE,
            );

        self.conn
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                win,
                self.root,
                monitor.x,
                monitor.y,
                monitor.width,
                monitor.height,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &aux,
            )
            .map_err(|e| x11_err("create_window", e))?;

        self.mark_as_desktop(win)?;
        self.set_identity(win)?;

        self.conn
            .map_window(win)
            .map_err(|e| x11_err("map_window", e))?;
        self.lower(win)?;
        self.conn.flush().map_err(|e| x11_err("flush", e))?;
        Ok(win)
    }

    /// Tandai window sebagai bagian desktop: di bawah semua jendela, di semua
    /// workspace, dan tidak muncul di taskbar/pager.
    pub fn mark_as_desktop(&self, win: Window) -> Result<()> {
        let tipe = self.atom("_NET_WM_WINDOW_TYPE")?;
        let desktop = self.atom("_NET_WM_WINDOW_TYPE_DESKTOP")?;
        let state = self.atom("_NET_WM_STATE")?;
        let below = self.atom("_NET_WM_STATE_BELOW")?;
        let sticky = self.atom("_NET_WM_STATE_STICKY")?;
        let skip_taskbar = self.atom("_NET_WM_STATE_SKIP_TASKBAR")?;
        let skip_pager = self.atom("_NET_WM_STATE_SKIP_PAGER")?;
        let wm_desktop = self.atom("_NET_WM_DESKTOP")?;

        self.conn
            .change_property32(PropMode::REPLACE, win, tipe, AtomEnum::ATOM, &[desktop])
            .map_err(|e| x11_err("set _NET_WM_WINDOW_TYPE", e))?;

        self.conn
            .change_property32(
                PropMode::REPLACE,
                win,
                state,
                AtomEnum::ATOM,
                &[below, sticky, skip_taskbar, skip_pager],
            )
            .map_err(|e| x11_err("set _NET_WM_STATE", e))?;

        self.conn
            .change_property32(
                PropMode::REPLACE,
                win,
                wm_desktop,
                AtomEnum::CARDINAL,
                &[ALL_DESKTOPS],
            )
            .map_err(|e| x11_err("set _NET_WM_DESKTOP", e))?;

        Ok(())
    }

    /// Nama dan kelas window, agar mudah dikenali lewat `xwininfo`/`wmctrl`.
    pub fn set_identity(&self, win: Window) -> Result<()> {
        self.conn
            .change_property8(
                PropMode::REPLACE,
                win,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                b"vallentheme wallpaper",
            )
            .map_err(|e| x11_err("set WM_NAME", e))?;

        self.conn
            .change_property8(
                PropMode::REPLACE,
                win,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                b"vallentheme\0Vallentheme\0",
            )
            .map_err(|e| x11_err("set WM_CLASS", e))?;

        let pid_atom = self.atom("_NET_WM_PID")?;
        self.conn
            .change_property32(
                PropMode::REPLACE,
                win,
                pid_atom,
                AtomEnum::CARDINAL,
                &[std::process::id()],
            )
            .map_err(|e| x11_err("set _NET_WM_PID", e))?;

        Ok(())
    }

    /// Turunkan window ke dasar tumpukan.
    pub fn lower(&self, win: Window) -> Result<()> {
        let aux = x11rb::protocol::xproto::ConfigureWindowAux::new().stack_mode(StackMode::BELOW);
        self.conn
            .configure_window(win, &aux)
            .map_err(|e| x11_err("configure_window", e))?;
        self.conn.flush().map_err(|e| x11_err("flush", e))?;
        Ok(())
    }

    /// Ubah geometri window agar pas dengan monitor.
    pub fn fit_to_monitor(&self, win: Window, monitor: &MonitorInfo) -> Result<()> {
        let aux = x11rb::protocol::xproto::ConfigureWindowAux::new()
            .x(i32::from(monitor.x))
            .y(i32::from(monitor.y))
            .width(u32::from(monitor.width))
            .height(u32::from(monitor.height));
        self.conn
            .configure_window(win, &aux)
            .map_err(|e| x11_err("configure_window", e))?;
        self.conn.flush().map_err(|e| x11_err("flush", e))?;
        Ok(())
    }

    /// Cat warna solid ke seluruh window.
    ///
    /// Diperlukan karena `background_pixel` hanya terpakai saat window
    /// di-*clear*; dengan compositor aktif (marco) isi window tidak otomatis
    /// terisi, sehingga harus dilukis sendiri setiap kali menerima `Expose`.
    pub fn fill_solid(&self, win: Window, color: u32, lebar: u16, tinggi: u16) -> Result<()> {
        let gc = self
            .conn
            .generate_id()
            .map_err(|e| x11_err("generate_id", e))?;
        let aux = CreateGCAux::new().foreground(color);
        self.conn
            .create_gc(gc, win, &aux)
            .map_err(|e| x11_err("create_gc", e))?;
        let kotak = [Rectangle {
            x: 0,
            y: 0,
            width: lebar,
            height: tinggi,
        }];
        self.conn
            .poly_fill_rectangle(win, gc, &kotak)
            .map_err(|e| x11_err("poly_fill_rectangle", e))?;
        self.conn.free_gc(gc).map_err(|e| x11_err("free_gc", e))?;
        self.conn.flush().map_err(|e| x11_err("flush", e))?;
        Ok(())
    }

    /// Naikkan window satu tingkat di atas window lain pada tumpukan.
    pub fn raise_above(&self, win: Window) -> Result<()> {
        let aux = x11rb::protocol::xproto::ConfigureWindowAux::new().stack_mode(StackMode::ABOVE);
        self.conn
            .configure_window(win, &aux)
            .map_err(|e| x11_err("configure_window", e))?;
        self.conn.flush().map_err(|e| x11_err("flush", e))?;
        Ok(())
    }

    /// Buang semua peristiwa yang menunggu; kembalikan `true` bila ada `Expose`.
    pub fn drain_events_had_expose(&self) -> bool {
        let mut ada = false;
        while let Ok(Some(event)) = self.conn.poll_for_event() {
            if matches!(event, x11rb::protocol::Event::Expose(_)) {
                ada = true;
            }
        }
        ada
    }

    /// Minta server mengirim `PropertyNotify` untuk window root.
    pub fn watch_root_properties(&self) -> Result<()> {
        let aux = x11rb::protocol::xproto::ChangeWindowAttributesAux::new()
            .event_mask(EventMask::PROPERTY_CHANGE);
        self.conn
            .change_window_attributes(self.root, &aux)
            .map_err(|e| x11_err("change_window_attributes", e))?;
        self.conn.flush().map_err(|e| x11_err("flush", e))?;
        Ok(())
    }

    /// Window yang sedang aktif menurut `_NET_ACTIVE_WINDOW`.
    pub fn active_window(&self) -> Result<Option<Window>> {
        let atom = self.atom("_NET_ACTIVE_WINDOW")?;
        let reply = self
            .conn
            .get_property(false, self.root, atom, AtomEnum::WINDOW, 0, 1)
            .map_err(|e| x11_err("get_property _NET_ACTIVE_WINDOW", e))?
            .reply()
            .map_err(|e| x11_err("get_property _NET_ACTIVE_WINDOW", e))?;
        Ok(reply
            .value32()
            .and_then(|mut it| it.next())
            .filter(|w| *w != 0))
    }

    /// Daftar nama atom pada `_NET_WM_STATE` sebuah window.
    pub fn wm_state(&self, win: Window) -> Result<Vec<String>> {
        let atom = self.atom("_NET_WM_STATE")?;
        let reply = self
            .conn
            .get_property(false, win, atom, AtomEnum::ATOM, 0, 64)
            .map_err(|e| x11_err("get_property _NET_WM_STATE", e))?
            .reply()
            .map_err(|e| x11_err("get_property _NET_WM_STATE", e))?;

        let Some(nilai) = reply.value32() else {
            return Ok(Vec::new());
        };
        let mut hasil = Vec::new();
        for atom_id in nilai {
            hasil.push(self.atom_name(atom_id)?);
        }
        Ok(hasil)
    }

    /// Ambil satu peristiwa `PropertyNotify` bila ada: (nama atom, window).
    pub fn poll_property_notify(&self) -> Option<(String, Window)> {
        use x11rb::protocol::Event;

        loop {
            let event = self.conn.poll_for_event().ok().flatten()?;
            if let Event::PropertyNotify(ev) = event {
                let nama = self.atom_name(ev.atom).unwrap_or_default();
                return Some((nama, ev.window));
            }
        }
    }

    /// Daftar window klien menurut `_NET_CLIENT_LIST_STACKING` (bawah → atas).
    pub fn client_list_stacking(&self) -> Result<Vec<Window>> {
        let atom = self.atom("_NET_CLIENT_LIST_STACKING")?;
        let reply = self
            .conn
            .get_property(false, self.root, atom, AtomEnum::WINDOW, 0, 1024)
            .map_err(|e| x11_err("get_property _NET_CLIENT_LIST_STACKING", e))?
            .reply()
            .map_err(|e| x11_err("get_property _NET_CLIENT_LIST_STACKING", e))?;
        Ok(reply.value32().map(|it| it.collect()).unwrap_or_default())
    }

    /// Tipe window menurut `_NET_WM_WINDOW_TYPE`.
    pub fn window_type(&self, win: Window) -> Result<Vec<String>> {
        let atom = self.atom("_NET_WM_WINDOW_TYPE")?;
        let reply = self
            .conn
            .get_property(false, win, atom, AtomEnum::ATOM, 0, 32)
            .map_err(|e| x11_err("get_property _NET_WM_WINDOW_TYPE", e))?
            .reply()
            .map_err(|e| x11_err("get_property _NET_WM_WINDOW_TYPE", e))?;
        let Some(nilai) = reply.value32() else {
            return Ok(Vec::new());
        };
        let mut hasil = Vec::new();
        for atom_id in nilai {
            hasil.push(self.atom_name(atom_id)?);
        }
        Ok(hasil)
    }

    /// Naikkan window tepat di atas window desktop milik aplikasi lain
    /// (misalnya Caja di MATE) yang menutupinya.
    ///
    /// Mengembalikan `true` bila ada window desktop lain yang dilewati.
    pub fn raise_above_other_desktop(&self, win: Window) -> Result<bool> {
        let daftar = self.client_list_stacking()?;
        let mut target: Option<Window> = None;
        for kandidat in daftar {
            if kandidat == win {
                continue;
            }
            let tipe = self.window_type(kandidat).unwrap_or_default();
            if tipe.iter().any(|t| t.ends_with("WINDOW_TYPE_DESKTOP")) {
                target = Some(kandidat);
            }
        }

        let Some(saudara) = target else {
            return Ok(false);
        };

        let aux = x11rb::protocol::xproto::ConfigureWindowAux::new()
            .stack_mode(StackMode::ABOVE)
            .sibling(saudara);
        self.conn
            .configure_window(win, &aux)
            .map_err(|e| x11_err("configure_window above sibling", e))?;
        self.conn.flush().map_err(|e| x11_err("flush", e))?;
        Ok(true)
    }

    /// Hancurkan window dengan aman (abaikan bila sudah hilang).
    pub fn destroy_window(&self, win: Window) {
        if let Err(err) = self.conn.destroy_window(win) {
            tracing::debug!(error = %err, "gagal menghancurkan window desktop");
        }
        let _ = self.conn.flush();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn area_monitor_dihitung_benar() {
        let mon = MonitorInfo {
            name: "LVDS-1".to_string(),
            x: 0,
            y: 0,
            width: 1366,
            height: 768,
            primary: true,
        };
        assert_eq!(mon.area(), 1366 * 768);
    }

    #[test]
    fn konstanta_desktop_semua_workspace() {
        assert_eq!(ALL_DESKTOPS, 0xFFFF_FFFF);
    }
}
