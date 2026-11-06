//! Halaman Edit wallpaper: pratinjau + pengaturan per wallpaper.
//!
//! Setiap perubahan disimpan otomatis ke `settings_json` (debounce 400 ms).
//! Penerapan langsung ke daemon (`Update`) disambungkan setelah daemon Fase 5
//! tersedia; sampai saat itu tombol "Jadikan wallpaper" dinonaktifkan dengan
//! keterangan jelas agar tidak menjanjikan sesuatu yang belum ada.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use vallentheme_core::model::{FillMode, MediaKind, PowerPolicy, Transition, Wallpaper};

use crate::i18n::tr;
use crate::state::Shared;

/// Bangun halaman Edit untuk satu wallpaper.
pub fn build(state: Shared, toast: adw::ToastOverlay, w: Wallpaper) -> adw::NavigationPage {
    let awal = w.settings.clone();
    let nilai = Rc::new(RefCell::new(awal));
    let id = w.id;

    // ---------- Header ----------
    let header = adw::HeaderBar::new();
    let judul = adw::WindowTitle::new(&tr("Edit wallpaper"), &w.title);
    header.set_title_widget(Some(&judul));

    // ---------- Kolom pratinjau ----------
    let gambar = gtk::Picture::new();
    gambar.add_css_class("vt-preview");
    gambar.set_content_fit(gtk::ContentFit::Cover);
    gambar.set_can_shrink(true);
    gambar.set_size_request(320, 180);
    gambar.set_vexpand(true);
    if let Some(thumb) = &w.thumb_path {
        if thumb.is_file() {
            gambar.set_filename(Some(thumb));
        }
    }

    let bingkai = gtk::Box::new(gtk::Orientation::Vertical, 8);
    bingkai.set_size_request(320, -1);
    bingkai.append(&gambar);

    let info = gtk::Label::new(Some(&format!(
        "{}×{} · {}",
        w.width.unwrap_or(0),
        w.height.unwrap_or(0),
        w.kind.label()
    )));
    info.add_css_class("wp-meta");
    info.set_xalign(0.0);
    bingkai.append(&info);

    let tombol_pratinjau = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let putar = gtk::Button::with_label(&tr("Rotate 90°"));
    putar.add_css_class("vt-flat");
    let reset = gtk::Button::with_label(&tr("Reset crop"));
    reset.add_css_class("vt-flat");
    tombol_pratinjau.append(&putar);
    tombol_pratinjau.append(&reset);
    bingkai.append(&tombol_pratinjau);

    // ---------- Kolom pengaturan ----------
    let grup = adw::PreferencesGroup::new();

    let opsi_fill = gtk::StringList::new(&["Penuhi", "Pas", "Tengah", "Regang", "Ubin"]);
    let baris_fill = adw::ComboRow::builder()
        .title(tr("Fill method"))
        .subtitle("Cara media mengisi layar")
        .model(&opsi_fill)
        .build();

    let baris_transisi = adw::ComboRow::builder()
        .title(tr("Transition"))
        .subtitle("Wallpaper berganti tanpa efek apa pun.")
        .model(&gtk::StringList::new(&["Tidak ada", "Pudar"]))
        .build();

    let baris_bisu = adw::SwitchRow::builder().title(tr("Mute")).build();

    let baris_volume = adw::ActionRow::builder()
        .title("Volume")
        .subtitle(format!("{}%", w.settings.volume))
        .build();
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 100.0, 1.0);
    slider.set_value(f64::from(w.settings.volume));
    slider.set_size_request(180, -1);
    slider.set_valign(gtk::Align::Center);
    baris_volume.add_suffix(&slider);

    let bawaan = tr("Default");
    let opsi_daya = gtk::StringList::new(&[
        bawaan.as_str(),
        "Selalu jalan",
        "Jeda saat fullscreen",
        "Jeda saat baterai",
        "Selalu hemat",
    ]);
    let baris_daya = adw::ComboRow::builder()
        .title(tr("Power saving"))
        .subtitle("Bawaan mengikuti Pengaturan")
        .model(&opsi_daya)
        .build();

    grup.add(&baris_fill);
    grup.add(&baris_transisi);
    grup.add(&baris_bisu);
    grup.add(&baris_volume);
    grup.add(&baris_daya);

    // Nilai awal widget mengikuti pengaturan tersimpan.
    baris_fill.set_selected(indeks_fill(w.settings.fill));
    baris_transisi.set_selected(match w.settings.transition {
        Transition::None => 0,
        Transition::Fade => 1,
    });
    baris_bisu.set_active(w.settings.mute);
    baris_daya.set_selected(indeks_daya(w.settings.power));

    // Foto tidak punya audio/daya; video tidak bisa diubin.
    let video = w.kind == MediaKind::Video;
    baris_bisu.set_visible(video);
    baris_volume.set_visible(video);
    baris_daya.set_visible(video);

    let tombol_terapkan = gtk::Button::with_label(&tr("Set as wallpaper"));
    tombol_terapkan.add_css_class("vt-primary");
    tombol_terapkan.add_css_class("large");
    tombol_terapkan.set_sensitive(true);
    tombol_terapkan.set_tooltip_text(Some(&tr("Set as wallpaper")));
    {
        let nilai = nilai.clone();
        let toast = toast.clone();
        let tombol = tombol_terapkan.clone();
        let id_wallpaper = w.id.0;
        let path = w.path.clone();
        let kind = w.kind;
        tombol_terapkan.connect_clicked(move |_| {
            let s = nilai.borrow().clone();
            let permintaan = permintaan_apply(id_wallpaper, &path, kind, &s);
            tombol.set_sensitive(false);
            let toast = toast.clone();
            let tombol2 = tombol.clone();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(crate::dbus_client::apply(&permintaan));
            });
            tunggu_apply(rx, toast, tombol2);
        });
    }

    let kolom = gtk::Box::new(gtk::Orientation::Vertical, 24);
    kolom.set_size_request(320, -1);
    kolom.set_hexpand(true);
    kolom.append(&grup);
    kolom.append(&tombol_terapkan);

    let isi = gtk::Box::new(gtk::Orientation::Horizontal, 32);
    isi.set_margin_top(24);
    isi.set_margin_bottom(24);
    isi.set_margin_start(24);
    isi.set_margin_end(24);
    isi.append(&bingkai);
    isi.append(&kolom);

    let clamp = adw::Clamp::builder().maximum_size(960).child(&isi).build();

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&clamp));

    // ---------- Penyimpanan otomatis ----------
    let jadwalkan = {
        let state = state.clone();
        let nilai = nilai.clone();
        let toast = toast.clone();
        let sumber: Rc<RefCell<Option<gtk::glib::SourceId>>> = Rc::new(RefCell::new(None));

        Rc::new(move || {
            if let Some(id) = sumber.borrow_mut().take() {
                id.remove();
            }
            let state = state.clone();
            let nilai = nilai.clone();
            let toast = toast.clone();
            let id = gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(400),
                move || {
                    let s = nilai.borrow().clone();
                    let hasil = state.db.call(move |db| db.set_settings(id, &s));
                    if let Err(e) = hasil {
                        tracing::error!(error = %e, "gagal menyimpan pengaturan");
                        toast.add_toast(adw::Toast::new(&tr("Failed")));
                    }
                },
            );
            *sumber.borrow_mut() = Some(id);
        })
    };

    // Isi metode pengisian.
    {
        let nilai = nilai.clone();
        let jadwalkan = jadwalkan.clone();
        baris_fill.connect_selected_notify(move |row| {
            nilai.borrow_mut().fill = fill_dari_indeks(row.selected());
            jadwalkan();
        });
    }

    // Transisi.
    {
        let nilai = nilai.clone();
        let jadwalkan = jadwalkan.clone();
        let baris = baris_transisi.clone();
        baris_transisi.connect_selected_notify(move |row| {
            let pudar = row.selected() == 1;
            nilai.borrow_mut().transition = if pudar {
                Transition::Fade
            } else {
                Transition::None
            };
            baris.set_subtitle(if pudar {
                "Wallpaper berganti dengan efek memudar."
            } else {
                "Wallpaper berganti tanpa efek apa pun."
            });
            jadwalkan();
        });
    }

    // Bisu.
    {
        let nilai = nilai.clone();
        let jadwalkan = jadwalkan.clone();
        let volume = baris_volume.clone();
        baris_bisu.connect_active_notify(move |row| {
            let bisu = row.is_active();
            nilai.borrow_mut().mute = bisu;
            volume.set_sensitive(!bisu);
            jadwalkan();
        });
        baris_volume.set_sensitive(!w.settings.mute);
    }

    // Volume.
    {
        let nilai = nilai.clone();
        let jadwalkan = jadwalkan.clone();
        let baris = baris_volume.clone();
        slider.connect_value_changed(move |s| {
            let v = s.value().round().clamp(0.0, 100.0);
            let v = u8::try_from(v as i64).unwrap_or(50);
            nilai.borrow_mut().volume = v;
            baris.set_subtitle(&format!("{v}%"));
            jadwalkan();
        });
    }

    // Hemat daya.
    {
        let nilai = nilai.clone();
        let jadwalkan = jadwalkan.clone();
        baris_daya.connect_selected_notify(move |row| {
            nilai.borrow_mut().power = daya_dari_indeks(row.selected());
            jadwalkan();
        });
    }

    // Putar 90°.
    {
        let nilai = nilai.clone();
        let jadwalkan = jadwalkan.clone();
        putar.connect_clicked(move |_| {
            let mut v = nilai.borrow_mut();
            v.rotation = (v.rotation + 90) % 360;
            drop(v);
            jadwalkan();
        });
    }

    // Setel ulang pangkas.
    {
        let nilai = nilai.clone();
        let jadwalkan = jadwalkan.clone();
        reset.connect_clicked(move |_| {
            let mut v = nilai.borrow_mut();
            v.zoom = 0.0;
            v.pan_x = 0.0;
            v.pan_y = 0.0;
            v.validate();
            drop(v);
            jadwalkan();
        });
    }

    adw::NavigationPage::builder()
        .title(tr("Edit wallpaper"))
        .tag("edit")
        .child(&toolbar)
        .build()
}

/// Susun `ApplyRequest` JSON sesuai `panduan/06` bagian 1.
pub fn permintaan_apply(
    wallpaper_id: i64,
    path: &std::path::Path,
    kind: MediaKind,
    s: &vallentheme_core::model::WallpaperSettings,
) -> String {
    let jenis = match kind {
        MediaKind::Video => "video",
        MediaKind::Image => "image",
    };
    let transisi = match s.transition {
        Transition::None => "none",
        Transition::Fade => "fade",
    };
    let daya = match s.power {
        PowerPolicy::Default => "default",
        PowerPolicy::Always => "always",
        PowerPolicy::PauseFullscreen => "pause_fullscreen",
        PowerPolicy::PauseBattery => "pause_battery",
        PowerPolicy::AlwaysSaver => "always_saver",
    };
    let nilai = serde_json::json!({
        "v": 1,
        "wallpaper_id": wallpaper_id,
        "path": path.to_string_lossy(),
        "kind": jenis,
        "monitor": s.monitor.clone().unwrap_or_else(|| "all".to_string()),
        "settings": {
            "fill": s.fill.as_str(),
            "rotation": s.rotation,
            "zoom": s.zoom,
            "pan_x": s.pan_x,
            "pan_y": s.pan_y,
            "transition": transisi,
            "mute": s.mute,
            "volume": s.volume,
            "power": daya,
        }
    });
    nilai.to_string()
}

/// Tunggu hasil `Apply` tanpa memblokir main loop.
fn tunggu_apply(
    rx: std::sync::mpsc::Receiver<Result<String, String>>,
    toast: adw::ToastOverlay,
    tombol: gtk::Button,
) {
    let rx = Rc::new(RefCell::new(rx));
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
        let hasil = rx.borrow().try_recv();
        match hasil {
            Ok(Ok(_json)) => {
                tombol.set_sensitive(true);
                toast.add_toast(adw::Toast::new(&tr("Set as wallpaper")));
                gtk::glib::ControlFlow::Break
            }
            Ok(Err(pesan)) => {
                tracing::error!(error = %pesan, "Apply gagal");
                tombol.set_sensitive(true);
                toast.add_toast(adw::Toast::new(&tr("Failed")));
                gtk::glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
        }
    });
}

/// Indeks ComboRow untuk [`FillMode`].
pub fn indeks_fill(mode: FillMode) -> u32 {
    match mode {
        FillMode::Cover => 0,
        FillMode::Fit => 1,
        FillMode::Center => 2,
        FillMode::Stretch => 3,
        FillMode::Tile => 4,
    }
}

/// Kebalikan dari [`indeks_fill`]; indeks asing menjadi `Cover`.
pub fn fill_dari_indeks(indeks: u32) -> FillMode {
    match indeks {
        1 => FillMode::Fit,
        2 => FillMode::Center,
        3 => FillMode::Stretch,
        4 => FillMode::Tile,
        _ => FillMode::Cover,
    }
}

/// Indeks ComboRow untuk [`PowerPolicy`].
pub fn indeks_daya(policy: PowerPolicy) -> u32 {
    match policy {
        PowerPolicy::Default => 0,
        PowerPolicy::Always => 1,
        PowerPolicy::PauseFullscreen => 2,
        PowerPolicy::PauseBattery => 3,
        PowerPolicy::AlwaysSaver => 4,
    }
}

/// Kebalikan dari [`indeks_daya`]; indeks asing menjadi `Default`.
pub fn daya_dari_indeks(indeks: u32) -> PowerPolicy {
    match indeks {
        1 => PowerPolicy::Always,
        2 => PowerPolicy::PauseFullscreen,
        3 => PowerPolicy::PauseBattery,
        4 => PowerPolicy::AlwaysSaver,
        _ => PowerPolicy::Default,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn pemetaan_fill_bolak_balik() {
        for mode in [
            FillMode::Cover,
            FillMode::Fit,
            FillMode::Center,
            FillMode::Stretch,
            FillMode::Tile,
        ] {
            assert_eq!(fill_dari_indeks(indeks_fill(mode)), mode);
        }
    }

    #[test]
    fn indeks_fill_asing_menjadi_cover() {
        assert_eq!(fill_dari_indeks(99), FillMode::Cover);
    }

    #[test]
    fn pemetaan_daya_bolak_balik() {
        for policy in [
            PowerPolicy::Default,
            PowerPolicy::Always,
            PowerPolicy::PauseFullscreen,
            PowerPolicy::PauseBattery,
            PowerPolicy::AlwaysSaver,
        ] {
            assert_eq!(daya_dari_indeks(indeks_daya(policy)), policy);
        }
    }

    #[test]
    fn indeks_daya_asing_menjadi_default() {
        assert_eq!(daya_dari_indeks(42), PowerPolicy::Default);
    }
}
