//! Layanan D-Bus `id.vallentheme.Daemon1`.
//!
//! Semua payload kompleks berupa string JSON dengan field `"v": 1`. Metode
//! memvalidasi masukan dan mengembalikan error berkode, tidak pernah panic.
//! Rujukan: `panduan/06-referensi-teknis.md` bagian 1.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::time::Instant;

use serde_json::{json, Value};
use tokio::sync::Notify;
use vallentheme_core::model::{MediaKind, WallpaperSettings};
use vallentheme_core::paths::Paths;
use vallentheme_render::mpv::supervisor::{MpvSupervisor, SupervisorEvent, INTERVAL_KESEHATAN};
use vallentheme_render::mpv::PlaySpec;
use vallentheme_render::setter::Setter;
use vallentheme_render::x11::{MonitorInfo, X11Context};

use crate::state::{ApplyRequest, DaemonState, SEMUA_MONITOR};

/// Nama bus yang dimiliki daemon.
pub const NAMA_BUS: &str = "id.vallentheme.Daemon";
/// Path objek D-Bus.
pub const PATH_OBJEK: &str = "/id/vallentheme/Daemon";

/// Perintah yang dikirim ke thread pemutar video.
enum PerintahSesi {
    /// Jeda pemutaran.
    Jeda,
    /// Lanjutkan pemutaran.
    Lanjut,
    /// Hentikan dan bersihkan.
    Hentikan,
}

/// Pegangan satu sesi video.
struct SesiVideo {
    pengirim: mpsc::Sender<PerintahSesi>,
}

/// Bagian state yang dilindungi mutex.
struct Inti {
    paths: Paths,
    state: DaemonState,
    dijeda: bool,
    alasan_jeda: Option<String>,
    sesi: HashMap<String, SesiVideo>,
    setter: Option<Setter>,
}

/// Layanan D-Bus yang bisa dibagikan antar-tugas.
#[derive(Clone)]
pub struct DaemonService {
    inti: Arc<Mutex<Inti>>,
    berhenti: Arc<Notify>,
    keluar: Arc<AtomicBool>,
}

impl DaemonService {
    /// Buat layanan dari path XDG; state lama dimuat bila ada.
    pub fn new(paths: Paths) -> Self {
        let state = DaemonState::load(&paths.daemon_state_file);
        let inti = Inti {
            paths,
            state,
            dijeda: false,
            alasan_jeda: None,
            sesi: HashMap::new(),
            setter: None,
        };
        Self {
            inti: Arc::new(Mutex::new(inti)),
            berhenti: Arc::new(Notify::new()),
            keluar: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Tunggu sampai `Quit` dipanggil atau sinyal proses diterima.
    pub async fn tunggu_berhenti(&self) {
        self.berhenti.notified().await;
    }

    /// Minta layanan berhenti (dipakai penangan sinyal).
    pub fn minta_berhenti(&self) {
        self.keluar.store(true, Ordering::SeqCst);
        self.berhenti.notify_one();
    }

    /// Terapkan semua permintaan yang tersimpan (`--restore`).
    ///
    /// Berkas state yang hilang tidak dianggap fatal: desktop dibiarkan apa
    /// adanya dan hanya dicatat di log.
    pub fn pulihkan_semua(&self) {
        let permintaan: Vec<ApplyRequest> = match self.inti.lock() {
            Ok(inti) => inti
                .state
                .monitors
                .values()
                .map(|entri| entri.request.clone())
                .collect(),
            Err(_) => {
                tracing::error!("state daemon tidak bisa diakses saat restore");
                return;
            }
        };

        if permintaan.is_empty() {
            tracing::info!("tidak ada wallpaper tersimpan untuk dipulihkan");
            return;
        }

        for req in permintaan {
            let hasil = match self.inti.lock() {
                Ok(mut inti) => inti.terapkan(&req),
                Err(_) => Err((
                    "VT-DAEMON-001".to_string(),
                    "state daemon tidak bisa diakses".to_string(),
                )),
            };
            match hasil {
                Ok(_) => tracing::info!(path = %req.path.display(), "wallpaper dipulihkan"),
                Err((kode, pesan)) => {
                    tracing::warn!(kode, pesan, "gagal memulihkan wallpaper");
                }
            }
        }
    }

    /// Hentikan semua sesi, pulihkan desktop, dan simpan state.
    pub fn bersihkan(&self) {
        let hasil = match self.inti.lock() {
            Ok(mut inti) => {
                inti.hentikan_semua();
                inti.pulihkan_desktop();
                let simpan = inti.state.save(&inti.paths.daemon_state_file);
                if let Err(err) = simpan {
                    tracing::warn!(error = %err, "state daemon gagal disimpan saat keluar");
                }
                Ok(())
            }
            Err(_) => Err("state daemon tidak bisa diakses saat keluar"),
        };
        if let Err(pesan) = hasil {
            tracing::error!(pesan, "pembersihan daemon tidak lengkap");
        }
    }

    /// Ringkasan status dalam JSON.
    pub fn status_json(&self) -> String {
        match self.inti.lock() {
            Ok(inti) => inti.status_json(),
            Err(_) => json!({
                "v": 1,
                "state": "failed",
                "monitors": [],
                "error": {"code": "VT-DAEMON-001", "message": "state daemon tidak bisa diakses"}
            })
            .to_string(),
        }
    }

    /// Nama keadaan ringkas: `idle|running|paused|failed`.
    pub fn keadaan(&self) -> String {
        let teks = self.status_json();
        serde_json::from_str::<Value>(&teks)
            .ok()
            .and_then(|v| v.get("state").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_else(|| "failed".to_string())
    }

    fn kunci(&self) -> Result<MutexGuard<'_, Inti>, (String, String)> {
        self.inti.lock().map_err(|_| {
            (
                "VT-DAEMON-001".to_string(),
                "state daemon tidak bisa diakses".to_string(),
            )
        })
    }
}

impl Inti {
    fn terapkan(&mut self, req: &ApplyRequest) -> Result<String, (String, String)> {
        req.periksa_pengaturan()
            .map_err(|pesan| ("VT-VALID-003".to_string(), pesan))?;

        let kunci = req.kunci_monitor();
        self.hentikan_sesi(&kunci);

        match req.kind {
            MediaKind::Image => self.terapkan_foto(req, &kunci)?,
            MediaKind::Video => self.terapkan_video(req, &kunci)?,
        }

        if let Err(err) = self.state.save(&self.paths.daemon_state_file) {
            tracing::warn!(error = %err, "state daemon gagal disimpan");
        }

        Ok(json!({
            "v": 1,
            "ok": true,
            "monitor": kunci,
            "kind": req.kind.as_str(),
            "state": if self.dijeda { "paused" } else { "running" },
        })
        .to_string())
    }

    fn terapkan_foto(&mut self, req: &ApplyRequest, kunci: &str) -> Result<(), (String, String)> {
        if self.setter.is_none() {
            let desktop = Setter::detect().ok_or_else(|| {
                (
                    "VT-UNSUPPORTED-002".to_string(),
                    "desktop ini belum didukung untuk wallpaper foto".to_string(),
                )
            })?;
            self.setter = Some(Setter::new(desktop));
        }

        let monitor = if req.kunci_monitor() == SEMUA_MONITOR {
            None
        } else {
            Some(req.monitor.as_str())
        };

        let hasil = match self.setter.as_mut() {
            Some(setter) => setter.apply(&req.path, req.settings.fill, monitor),
            None => Err(vallentheme_render::RenderError::DesktopUnknown),
        };
        if let Err(err) = hasil {
            return Err((
                err.code().to_string(),
                format!("wallpaper foto gagal dipasang: {err}"),
            ));
        }

        let records = self
            .setter
            .as_ref()
            .map(|setter| setter.records().to_vec())
            .unwrap_or_default();

        self.state.set_request(req.clone());
        self.state.set_restore(kunci, records);
        Ok(())
    }

    fn terapkan_video(&mut self, req: &ApplyRequest, kunci: &str) -> Result<(), (String, String)> {
        if std::env::var("XDG_SESSION_TYPE")
            .map(|nilai| nilai.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
        {
            return Err((
                "VT-UNSUPPORTED-001".to_string(),
                "sesi Wayland belum mendukung wallpaper video".to_string(),
            ));
        }

        if std::process::Command::new("mpv")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_err()
        {
            return Err((
                "VT-MPV-001".to_string(),
                "mpv belum terpasang; pasang dengan: sudo apt install mpv".to_string(),
            ));
        }

        // Validasi X11 lebih dulu agar kegagalan bisa dilaporkan ke pemanggil.
        let ctx = X11Context::connect().map_err(|err| (err.code().to_string(), err.to_string()))?;
        let monitors = ctx
            .monitors()
            .map_err(|err| (err.code().to_string(), err.to_string()))?;
        let target = pilih_monitor(&monitors, &req.monitor)
            .ok_or_else(|| ("VT-X11-004".to_string(), "tidak ada monitor".to_string()))?;
        drop(ctx);

        let nama_soket = sanitasi_nama(&target.name);
        let socket = self
            .paths
            .runtime_dir
            .join(format!("mpv-{nama_soket}.sock"));
        let (pengirim, penerima) = mpsc::channel();
        let path = req.path.clone();
        let settings = req.settings.clone();
        let socket_thread = socket.clone();
        let nama_monitor = target.name.clone();

        std::thread::Builder::new()
            .name(format!("mpv-{nama_soket}"))
            .spawn(move || {
                jalankan_sesi_video(socket_thread, path, settings, nama_monitor, penerima);
            })
            .map_err(|err| {
                (
                    "VT-MPV-001".to_string(),
                    format!("tidak bisa membuat thread mpv: {err}"),
                )
            })?;

        self.sesi.insert(kunci.to_string(), SesiVideo { pengirim });
        self.state.set_request(req.clone());
        self.state.set_restore(kunci, Vec::new());
        Ok(())
    }

    fn hentikan_sesi(&mut self, kunci: &str) {
        if let Some(sesi) = self.sesi.remove(kunci) {
            let _ = sesi.pengirim.send(PerintahSesi::Hentikan);
        }
    }

    fn hentikan_semua(&mut self) {
        for (_, sesi) in self.sesi.drain() {
            let _ = sesi.pengirim.send(PerintahSesi::Hentikan);
        }
    }

    fn pulihkan_desktop(&mut self) {
        if let Some(mut setter) = self.setter.take() {
            if let Err(err) = setter.restore() {
                tracing::warn!(kode = err.code(), error = %err, "pemulihan desktop gagal");
            }
        }
    }

    fn status_json(&self) -> String {
        let monitors: Vec<Value> = self
            .state
            .monitors
            .values()
            .map(|entri| {
                json!({
                    "name": entri.request.kunci_monitor(),
                    "state": if self.dijeda { "paused" } else { "running" },
                    "wallpaper_id": entri.request.wallpaper_id,
                    "path": entri.request.path,
                    "kind": entri.request.kind.as_str(),
                    "paused_reason": entri.paused_reason,
                    "updated_at": entri.updated_at,
                })
            })
            .collect();

        let keadaan = if self.dijeda {
            "paused"
        } else if monitors.is_empty() {
            "idle"
        } else {
            "running"
        };

        json!({
            "v": 1,
            "state": keadaan,
            "monitors": monitors,
        })
        .to_string()
    }
}

/// Jalankan mpv di thread khusus yang memiliki koneksi X11 dan window desktop.
fn jalankan_sesi_video(
    socket: PathBuf,
    path: PathBuf,
    settings: WallpaperSettings,
    nama_monitor: String,
    penerima: mpsc::Receiver<PerintahSesi>,
) {
    let hasil = (|| -> Result<(), String> {
        let ctx = X11Context::connect().map_err(|err| err.to_string())?;
        let monitors = ctx.monitors().map_err(|err| err.to_string())?;
        let target = pilih_monitor(&monitors, &nama_monitor)
            .ok_or_else(|| "monitor tujuan tidak ditemukan".to_string())?;
        let win = ctx
            .create_desktop_window(target, 0, false)
            .map_err(|err| err.to_string())?;

        let spec = PlaySpec::new(win, socket.clone(), path, settings);
        let mut supervisor = MpvSupervisor::new(spec);
        if let Err(err) = supervisor.mulai() {
            tracing::warn!(
                kode = err.code(),
                "mpv gagal dijalankan; supervisor akan mencoba ulang"
            );
        }

        loop {
            match penerima.recv_timeout(INTERVAL_KESEHATAN) {
                Ok(PerintahSesi::Hentikan) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Ok(PerintahSesi::Jeda) => {
                    if let Err(err) = supervisor.jeda() {
                        tracing::warn!(kode = err.code(), "gagal menjeda mpv");
                    }
                }
                Ok(PerintahSesi::Lanjut) => {
                    if let Err(err) = supervisor.lanjutkan() {
                        tracing::warn!(kode = err.code(), "gagal melanjutkan mpv");
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }

            if let SupervisorEvent::Gagal(pesan) = supervisor.tick(Instant::now()) {
                tracing::error!(pesan, "sesi mpv berhenti karena gagal berulang");
                break;
            }
        }

        supervisor.hentikan();
        ctx.destroy_window(win);
        Ok(())
    })();

    if let Err(pesan) = hasil {
        tracing::error!(pesan, "sesi video berakhir dengan kesalahan");
    }
    let _ = std::fs::remove_file(&socket);
}

/// Pilih monitor berdasarkan nama; `all`/kosong memilih monitor utama.
fn pilih_monitor<'a>(monitors: &'a [MonitorInfo], nama: &str) -> Option<&'a MonitorInfo> {
    let nama = nama.trim();
    if !nama.is_empty() && nama != SEMUA_MONITOR {
        if let Some(mon) = monitors.iter().find(|m| m.name.eq_ignore_ascii_case(nama)) {
            return Some(mon);
        }
    }
    monitors
        .iter()
        .find(|m| m.primary)
        .or_else(|| monitors.first())
}

/// Ganti karakter yang tidak aman pada nama soket.
fn sanitasi_nama(nama: &str) -> String {
    let hasil: String = nama
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if hasil.is_empty() {
        "monitor".to_string()
    } else {
        hasil
    }
}

/// Parse dan validasi `ApplyRequest` dari JSON.
pub fn parse_request(teks: &str) -> Result<ApplyRequest, (String, String)> {
    let nilai: Value = serde_json::from_str(teks).map_err(|err| {
        (
            "VT-VALID-001".to_string(),
            format!("permintaan bukan JSON yang sah: {err}"),
        )
    })?;

    let versi = nilai.get("v").and_then(Value::as_u64).unwrap_or(0);
    if versi != 1 {
        return Err((
            "VT-VALID-001".to_string(),
            format!("versi payload {versi} tidak didukung; gunakan \"v\": 1"),
        ));
    }

    let req: ApplyRequest = serde_json::from_value(nilai).map_err(|err| {
        (
            "VT-VALID-002".to_string(),
            format!("isi permintaan tidak valid: {err}"),
        )
    })?;

    if !req.path.is_file() {
        return Err((
            "VT-IO-002".to_string(),
            format!("berkas {} tidak ditemukan", req.path.display()),
        ));
    }

    req.periksa_pengaturan()
        .map_err(|pesan| ("VT-VALID-003".to_string(), pesan))?;
    Ok(req)
}

/// Error D-Bus dengan kode yang bisa ditampilkan GUI.
fn galat(kode: &str, pesan: &str) -> zbus::fdo::Error {
    zbus::fdo::Error::Failed(format!("{kode}: {pesan}"))
}

#[zbus::interface(name = "id.vallentheme.Daemon1")]
impl DaemonService {
    /// Terapkan wallpaper dari payload `ApplyRequest`.
    async fn apply(
        &self,
        request_json: String,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<String> {
        let req = match parse_request(&request_json) {
            Ok(req) => req,
            Err((kode, pesan)) => {
                let _ = Self::error(emitter, &kode, &pesan).await;
                return Err(galat(&kode, &pesan));
            }
        };

        let hasil = match self.kunci() {
            Ok(mut inti) => inti.terapkan(&req),
            Err((kode, pesan)) => Err((kode, pesan)),
        };

        match hasil {
            Ok(payload) => {
                let status = self.status_json();
                let _ = Self::status_changed(emitter, &status).await;
                Ok(payload)
            }
            Err((kode, pesan)) => {
                let _ = Self::error(emitter, &kode, &pesan).await;
                Err(galat(&kode, &pesan))
            }
        }
    }

    /// Stub: ubah pengaturan wallpaper aktif (Fase 6).
    async fn update(&self, _patch_json: String) -> zbus::fdo::Result<String> {
        Err(zbus::fdo::Error::NotSupported(
            "belum didukung: Update menyusul di Fase 6".to_string(),
        ))
    }

    /// Hentikan wallpaper; string kosong atau `all` berarti semua monitor.
    async fn stop(
        &self,
        monitor: String,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<bool> {
        let dihentikan = match self.kunci() {
            Ok(mut inti) => {
                let semua = monitor.trim().is_empty() || monitor.trim() == SEMUA_MONITOR;
                let kunci: Vec<String> = if semua {
                    inti.state.nama_monitor()
                } else {
                    vec![monitor.trim().to_string()]
                };

                let mut ada = false;
                for k in &kunci {
                    inti.hentikan_sesi(k);
                    ada |= inti.state.remove(k);
                }
                if inti.state.monitors.is_empty() {
                    inti.pulihkan_desktop();
                }
                if let Err(err) = inti.state.save(&inti.paths.daemon_state_file) {
                    tracing::warn!(error = %err, "state daemon gagal disimpan setelah Stop");
                }
                ada
            }
            Err((kode, pesan)) => return Err(galat(&kode, &pesan)),
        };

        let status = self.status_json();
        let _ = Self::status_changed(emitter, &status).await;
        Ok(dihentikan)
    }

    /// Jeda semua wallpaper yang sedang berjalan.
    async fn pause(
        &self,
        reason: String,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<bool> {
        let alasan = if reason.trim().is_empty() {
            "manual".to_string()
        } else {
            reason.trim().to_string()
        };

        match self.kunci() {
            Ok(mut inti) => {
                inti.dijeda = true;
                inti.alasan_jeda = Some(alasan.clone());
                for sesi in inti.sesi.values() {
                    let _ = sesi.pengirim.send(PerintahSesi::Jeda);
                }
                for entri in inti.state.monitors.values_mut() {
                    entri.paused_reason = Some(alasan.clone());
                }
                if let Err(err) = inti.state.save(&inti.paths.daemon_state_file) {
                    tracing::warn!(error = %err, "state daemon gagal disimpan setelah Pause");
                }
            }
            Err((kode, pesan)) => return Err(galat(&kode, &pesan)),
        }

        let status = self.status_json();
        let _ = Self::status_changed(emitter, &status).await;
        Ok(true)
    }

    /// Lanjutkan wallpaper setelah jeda.
    async fn resume(
        &self,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<bool> {
        match self.kunci() {
            Ok(mut inti) => {
                inti.dijeda = false;
                inti.alasan_jeda = None;
                for sesi in inti.sesi.values() {
                    let _ = sesi.pengirim.send(PerintahSesi::Lanjut);
                }
                for entri in inti.state.monitors.values_mut() {
                    entri.paused_reason = None;
                }
                if let Err(err) = inti.state.save(&inti.paths.daemon_state_file) {
                    tracing::warn!(error = %err, "state daemon gagal disimpan setelah Resume");
                }
            }
            Err((kode, pesan)) => return Err(galat(&kode, &pesan)),
        }

        let status = self.status_json();
        let _ = Self::status_changed(emitter, &status).await;
        Ok(true)
    }

    /// Status lengkap per monitor.
    async fn get_status(&self) -> zbus::fdo::Result<String> {
        Ok(self.status_json())
    }

    /// Stub: konfigurasi kinerja (Fase 7).
    async fn set_performance(&self, _perf_json: String) -> zbus::fdo::Result<bool> {
        Err(zbus::fdo::Error::NotSupported(
            "belum didukung: SetPerformance menyusul di Fase 7".to_string(),
        ))
    }

    /// Stub: riwayat keputusan kinerja (Fase 7).
    async fn get_decision_log(&self, _limit: u32) -> zbus::fdo::Result<String> {
        Err(zbus::fdo::Error::NotSupported(
            "belum didukung: GetDecisionLog menyusul di Fase 7".to_string(),
        ))
    }

    /// Stub: profil perangkat (Fase 7).
    async fn get_device_profile(&self) -> zbus::fdo::Result<String> {
        Err(zbus::fdo::Error::NotSupported(
            "belum didukung: GetDeviceProfile menyusul di Fase 7".to_string(),
        ))
    }

    /// Keluar bersih.
    async fn quit(&self) {
        self.minta_berhenti();
    }

    /// Sinyal perubahan status.
    #[zbus(signal, name = "StateChanged")]
    async fn status_changed(
        emitter: zbus::object_server::SignalEmitter<'_>,
        status_json: &str,
    ) -> zbus::Result<()>;

    /// Sinyal kesalahan berkode.
    #[zbus(signal)]
    async fn error(
        emitter: zbus::object_server::SignalEmitter<'_>,
        code: &str,
        message: &str,
    ) -> zbus::Result<()>;

    /// Versi daemon.
    #[zbus(property)]
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Keadaan ringkas.
    #[zbus(property)]
    fn state(&self) -> String {
        self.keadaan()
    }

    /// Jenis sesi grafis.
    #[zbus(property)]
    fn session(&self) -> String {
        std::env::var("XDG_SESSION_TYPE")
            .map(|nilai| nilai.to_ascii_lowercase())
            .unwrap_or_else(|_| "unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::fs;

    use super::*;

    fn berkas_uji() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NOMOR: AtomicU64 = AtomicU64::new(0);
        let nomor = NOMOR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "vallentheme-apply-{}-{nomor}.mp4",
            std::process::id()
        ));
        fs::write(&path, b"uji").unwrap();
        path
    }

    fn payload(path: &PathBuf) -> String {
        json!({
            "v": 1,
            "wallpaper_id": 42,
            "path": path,
            "kind": "video",
            "monitor": "all",
            "settings": {
                "fill": "cover", "rotation": 0, "zoom": 0.0,
                "pan_x": 0.0, "pan_y": 0.0, "mute": true, "volume": 50
            }
        })
        .to_string()
    }

    #[test]
    fn permintaan_sah_diterima() {
        let path = berkas_uji();
        let req = parse_request(&payload(&path)).unwrap();
        assert_eq!(req.wallpaper_id, Some(42));
        assert_eq!(req.kind, MediaKind::Video);
        assert_eq!(req.kunci_monitor(), SEMUA_MONITOR);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn versi_salah_ditolak() {
        let path = berkas_uji();
        let mut nilai: Value = serde_json::from_str(&payload(&path)).unwrap();
        nilai["v"] = json!(2);
        let err = parse_request(&nilai.to_string()).unwrap_err();
        assert_eq!(err.0, "VT-VALID-001");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn jenis_media_tidak_dikenal_ditolak() {
        let path = berkas_uji();
        let mut nilai: Value = serde_json::from_str(&payload(&path)).unwrap();
        nilai["kind"] = json!("audio");
        let err = parse_request(&nilai.to_string()).unwrap_err();
        assert_eq!(err.0, "VT-VALID-002");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn berkas_hilang_ditolak() {
        let path = std::env::temp_dir().join("vallentheme-pasti-tidak-ada-xyz.mp4");
        let err = parse_request(&payload(&path)).unwrap_err();
        assert_eq!(err.0, "VT-IO-002");
    }

    #[test]
    fn pengaturan_di_luar_rentang_ditolak() {
        let path = berkas_uji();
        let mut nilai: Value = serde_json::from_str(&payload(&path)).unwrap();
        nilai["settings"]["zoom"] = json!(9.0);
        let err = parse_request(&nilai.to_string()).unwrap_err();
        assert_eq!(err.0, "VT-VALID-003");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn json_rusak_ditolak() {
        let err = parse_request("{bukan json").unwrap_err();
        assert_eq!(err.0, "VT-VALID-001");
    }

    #[test]
    fn sanitasi_nama_monitor() {
        assert_eq!(sanitasi_nama("HDMI-1"), "HDMI-1");
        assert_eq!(sanitasi_nama("DP/1"), "DP-1");
        assert_eq!(sanitasi_nama(""), "monitor");
    }

    #[test]
    fn pilih_monitor_memakai_utama_lalu_pertama() {
        let monitors = vec![
            MonitorInfo {
                name: "HDMI-1".to_string(),
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                primary: false,
            },
            MonitorInfo {
                name: "eDP-1".to_string(),
                x: 0,
                y: 0,
                width: 1366,
                height: 768,
                primary: true,
            },
        ];
        assert_eq!(pilih_monitor(&monitors, "all").unwrap().name, "eDP-1");
        assert_eq!(pilih_monitor(&monitors, "HDMI-1").unwrap().name, "HDMI-1");
        assert!(pilih_monitor(&[], "all").is_none());
    }
}
