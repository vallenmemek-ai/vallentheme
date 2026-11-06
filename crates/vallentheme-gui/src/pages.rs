//! Halaman-halaman aplikasi.
//!
//! Fase 3 masih memakai data contoh; penyambungan ke basis data terjadi di
//! Fase 4 dan 6.

use gtk::prelude::*;

use crate::i18n::tr;
use crate::widgets::{bottom_bar, empty_state, status_chip, wallpaper_card, KartuContoh, Status};

/// Bangun halaman Library (masih data contoh).
pub fn library() -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    header.pack_start(&status_chip(Status::TidakBerjalan));

    let judul = gtk::Label::new(Some("vallentheme"));
    judul.add_css_class("vt-header-title");
    header.set_title_widget(Some(&judul));
    header.pack_end(&menu_utama());

    // Baris pencarian.
    let cari = gtk::SearchEntry::builder()
        .placeholder_text(tr("Search wallpapers or folders…"))
        .hexpand(true)
        .build();
    cari.add_css_class("vt-search");
    cari.set_accessible_role(gtk::AccessibleRole::SearchBox);

    let clamp = adw::Clamp::builder().maximum_size(560).child(&cari).build();
    clamp.set_margin_top(12);
    clamp.set_margin_bottom(8);
    clamp.set_margin_start(16);
    clamp.set_margin_end(16);

    // Baris kontrol.
    let kontrol = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    kontrol.set_margin_start(16);
    kontrol.set_margin_end(16);
    kontrol.set_margin_bottom(12);

    let pilih_folder = gtk::MenuButton::builder()
        .label(tr("All wallpapers"))
        .icon_name("folder-symbolic")
        .build();
    pilih_folder.add_css_class("vt-secondary");
    pilih_folder.set_tooltip_text(Some(&tr("All wallpapers")));

    let urut = gtk::MenuButton::builder()
        .label(tr("My order"))
        .icon_name("view-sort-descending-symbolic")
        .halign(gtk::Align::End)
        .hexpand(true)
        .build();
    urut.add_css_class("vt-secondary");
    urut.set_tooltip_text(Some(&tr("My order")));

    kontrol.append(&pilih_folder);
    kontrol.append(&urut);

    // Daftar kartu contoh.
    let daftar = gtk::Box::new(gtk::Orientation::Vertical, 12);
    daftar.set_margin_start(16);
    daftar.set_margin_end(16);
    daftar.set_margin_top(8);
    daftar.set_margin_bottom(16);

    daftar.append(&judul_bagian("TERBARU"));
    daftar.append(&baris_kartu(&kartu_contoh()[..3]));

    daftar.append(&judul_bagian("VIDEO"));
    daftar.append(&baris_kartu(&kartu_contoh()[3..6]));

    daftar.append(&judul_bagian("FOTO"));
    daftar.append(&baris_kartu(&kartu_contoh()[6..9]));

    let gulir = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&daftar)
        .build();

    let isi = gtk::Box::new(gtk::Orientation::Vertical, 0);
    isi.append(&clamp);
    isi.append(&kontrol);
    isi.append(&gulir);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&isi));
    toolbar.add_bottom_bar(&bottom_bar(9));

    adw::NavigationPage::builder()
        .title("vallentheme")
        .tag("library")
        .child(&toolbar)
        .build()
}

/// Halaman Edit (placeholder Fase 3).
pub fn edit(nama: &str) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    let judul = adw::WindowTitle::new(&tr("Edit wallpaper"), nama);
    header.set_title_widget(Some(&judul));

    let isi = gtk::Box::new(gtk::Orientation::Vertical, 16);
    isi.set_valign(gtk::Align::Center);
    isi.set_halign(gtk::Align::Center);

    let halaman = adw::StatusPage::builder()
        .icon_name("image-x-generic-symbolic")
        .title(nama)
        .description(tr("Coming soon"))
        .build();

    isi.append(&halaman);

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&isi));

    adw::NavigationPage::builder()
        .title(nama)
        .tag("edit")
        .child(&toolbar)
        .build()
}

/// Halaman kosong (dipakai saat library belum ada isinya).
pub fn kosong() -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    let judul = gtk::Label::new(Some("vallentheme"));
    judul.add_css_class("vt-header-title");
    header.set_title_widget(Some(&judul));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&empty_state()));

    adw::NavigationPage::builder()
        .title("vallentheme")
        .tag("kosong")
        .child(&toolbar)
        .build()
}

fn judul_bagian(teks: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(teks));
    label.add_css_class("vt-section-title");
    label.set_xalign(0.0);
    label.upcast()
}

fn baris_kartu(data: &[KartuContoh]) -> gtk::Widget {
    let baris = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    baris.set_homogeneous(true);
    for kartu in data {
        baris.append(&wallpaper_card(kartu));
    }
    baris.upcast()
}

fn kartu_contoh() -> Vec<KartuContoh> {
    let nama = [
        "raiden-shogun-underwater",
        "gunung-fuji-senja",
        "kota-malam-neon",
        "hujan-di-jendela",
        "pantai-berbatu",
        "awan-bergerak",
        "hutan-pinus",
        "danau-tenang",
        "gurun-pasir",
    ];
    nama.iter()
        .enumerate()
        .map(|(i, n)| {
            let video = i % 2 == 0;
            let meta = if video {
                "4K · 24fps · 15 MB"
            } else {
                "3840×2160 · 2,1 MB"
            };
            KartuContoh::baru(n, meta, video)
        })
        .collect()
}

/// Tombol menu `⋯` di header.
fn menu_utama() -> gtk::MenuButton {
    let menu = gtk::gio::Menu::new();
    menu.append(Some(&tr("Preferences")), Some("app.preferences"));
    menu.append(Some(&tr("About vallentheme")), Some("app.about"));
    menu.append(Some(&tr("Stop wallpaper")), Some("app.stop"));
    menu.append(Some(&tr("Quit")), Some("app.quit"));

    gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .menu_model(&menu)
        .tooltip_text(tr("Preferences"))
        .build()
}
