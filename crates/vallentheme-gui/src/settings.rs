//! Halaman Pengaturan: batas fps dan tinggi render.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use crate::i18n::tr;
use crate::state::Shared;

/// Pilihan batas fps, sesuai `FPS_ALLOWED` di core.
const OPSI_FPS: [u32; 5] = [0, 24, 30, 60, 144];
/// Pilihan tinggi render, sesuai `HEIGHT_ALLOWED` di core.
const OPSI_TINGGI: [u32; 7] = [0, 720, 1080, 1440, 2160, 2880, 3840];

/// Bangun halaman Pengaturan yang menulis `config.toml` saat nilai berubah.
pub fn build(state: Shared, toast: adw::ToastOverlay) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(&tr("Preferences"), "")));

    let grup = adw::PreferencesGroup::new();
    grup.set_title(&tr("Performance"));

    let (fps, tinggi) = {
        let c = state.config.borrow();
        (c.performance.fps_cap, c.performance.max_render_height)
    };

    let label_fps: Vec<String> = OPSI_FPS
        .iter()
        .map(|n| {
            if *n == 0 {
                tr("Unlimited")
            } else {
                format!("{n} fps")
            }
        })
        .collect();
    let rujukan_fps: Vec<&str> = label_fps.iter().map(String::as_str).collect();
    let baris_fps = adw::ComboRow::builder()
        .title(tr("Frame rate limit"))
        .subtitle("Batas fps wallpaper video")
        .model(&gtk::StringList::new(&rujukan_fps))
        .build();
    baris_fps.set_selected(OPSI_FPS.iter().position(|n| *n == fps).unwrap_or(0) as u32);

    let label_tinggi: Vec<String> = OPSI_TINGGI
        .iter()
        .map(|n| {
            if *n == 0 {
                tr("Original")
            } else {
                format!("{n}p")
            }
        })
        .collect();
    let rujukan_tinggi: Vec<&str> = label_tinggi.iter().map(String::as_str).collect();
    let baris_tinggi = adw::ComboRow::builder()
        .title(tr("Render resolution"))
        .subtitle("Tinggi render maksimum; asli memakai resolusi berkas")
        .model(&gtk::StringList::new(&rujukan_tinggi))
        .build();
    baris_tinggi.set_selected(OPSI_TINGGI.iter().position(|n| *n == tinggi).unwrap_or(0) as u32);

    grup.add(&baris_fps);
    grup.add(&baris_tinggi);

    let halaman_isi = adw::PreferencesPage::new();
    halaman_isi.add(&grup);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&halaman_isi));

    // Simpan tiap perubahan; debounce kecil agar geser cepat tidak menulis berulang.
    let simpan = {
        let state = state.clone();
        let toast = toast.clone();
        let sumber: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));
        Rc::new(move || {
            if let Some(id) = sumber.borrow_mut().take() {
                id.remove();
            }
            let state = state.clone();
            let toast = toast.clone();
            let id = gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(250),
                move || {
                    let hasil = state.config.borrow().save(&state.paths.config_file);
                    if let Err(err) = hasil {
                        tracing::error!(error = %err, "gagal menyimpan pengaturan");
                        toast.add_toast(adw::Toast::new(&tr("Failed")));
                    }
                },
            );
            *sumber.borrow_mut() = Some(id);
        })
    };

    {
        let state = state.clone();
        let simpan = simpan.clone();
        baris_fps.connect_selected_notify(move |row| {
            let nilai = OPSI_FPS.get(row.selected() as usize).copied().unwrap_or(0);
            state.config.borrow_mut().performance.fps_cap = nilai;
            simpan();
        });
    }
    {
        let state = state.clone();
        let simpan = simpan.clone();
        baris_tinggi.connect_selected_notify(move |row| {
            let nilai = OPSI_TINGGI
                .get(row.selected() as usize)
                .copied()
                .unwrap_or(0);
            state.config.borrow_mut().performance.max_render_height = nilai;
            simpan();
        });
    }

    adw::NavigationPage::builder()
        .title(tr("Preferences"))
        .tag("pengaturan")
        .child(&toolbar)
        .build()
}
