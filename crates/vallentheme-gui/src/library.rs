//! Halaman Library: daftar, cari, sortir, impor, favorit, hapus, mode Pilih.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;

use gtk::prelude::*;

use vallentheme_core::model::{Filter, MediaKind, SortKey, Wallpaper, WallpaperId};
use vallentheme_core::scan::{self, ScanOptions};

use crate::i18n::tr;
use crate::state::Shared;
use crate::widgets::{status_chip, Status};

/// Data yang bisa berubah selama halaman hidup.
struct Ui {
    filter: Filter,
    sort: SortKey,
    mode_pilih: bool,
    terpilih: Vec<WallpaperId>,
    jumlah_total: u32,
    hitung: Option<gtk::Label>,
    debounce: Option<gtk::glib::SourceId>,
}

/// Bangun halaman Library yang tersambung ke basis data.
pub fn build(
    state: Shared,
    nav: adw::NavigationView,
    toast: adw::ToastOverlay,
) -> adw::NavigationPage {
    let ui = Rc::new(RefCell::new(Ui {
        filter: Filter::default(),
        sort: match state.config.borrow().ui.sort {
            vallentheme_core::config::SortSetting::Mine => SortKey::Mine,
            vallentheme_core::config::SortSetting::Newest => SortKey::Newest,
            vallentheme_core::config::SortSetting::Recent => SortKey::Recent,
            vallentheme_core::config::SortSetting::Name => SortKey::Name,
            vallentheme_core::config::SortSetting::Size => SortKey::Size,
            vallentheme_core::config::SortSetting::Resolution => SortKey::Resolution,
            vallentheme_core::config::SortSetting::Favorite => SortKey::Favorite,
        },
        mode_pilih: false,
        terpilih: Vec::new(),
        jumlah_total: 0,
        hitung: None,
        debounce: None,
    }));

    // ---------- Header ----------
    let header = adw::HeaderBar::new();
    header.pack_start(&status_chip(Status::TidakBerjalan));

    let judul = gtk::Label::new(Some("vallentheme"));
    judul.add_css_class("vt-header-title");
    header.set_title_widget(Some(&judul));
    header.pack_end(&menu_utama());

    // ---------- Pencarian ----------
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

    // ---------- Kontrol ----------
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
    urut.set_menu_model(Some(&menu_urut()));

    kontrol.append(&pilih_folder);
    kontrol.append(&urut);

    // ---------- Banner progres ----------
    let banner = gtk::Label::new(None);
    banner.add_css_class("vt-count");
    banner.set_xalign(0.0);
    banner.set_margin_start(16);
    banner.set_margin_end(16);
    banner.set_visible(false);

    // ---------- Daftar ----------
    let flow = gtk::FlowBox::builder()
        .max_children_per_line(6)
        .min_children_per_line(1)
        .column_spacing(16)
        .row_spacing(16)
        .homogeneous(true)
        .selection_mode(gtk::SelectionMode::None)
        .margin_start(16)
        .margin_end(16)
        .margin_top(8)
        .margin_bottom(16)
        .build();

    let gulir = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&flow)
        .build();

    let isi = gtk::Box::new(gtk::Orientation::Vertical, 0);
    isi.append(&clamp);
    isi.append(&kontrol);
    isi.append(&banner);
    isi.append(&gulir);

    // ---------- Bilah bawah ----------
    let bilah = bilah_bawah(
        state.clone(),
        ui.clone(),
        flow.clone(),
        nav.clone(),
        toast.clone(),
    );

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&isi));
    toolbar.add_bottom_bar(&bilah);

    let halaman = adw::NavigationPage::builder()
        .title("vallentheme")
        .tag("library")
        .child(&toolbar)
        .build();

    // ---------- Aksi ----------
    // Pencarian dengan debounce 150 ms.
    {
        let ui = ui.clone();
        let state = state.clone();
        let flow = flow.clone();
        let banner = banner.clone();
        let toast = toast.clone();
        cari.connect_search_changed(move |entry| {
            let teks = entry.text().to_string();
            if let Some(id) = ui.borrow_mut().debounce.take() {
                id.remove();
            }
            let ui2 = ui.clone();
            let state2 = state.clone();
            let flow2 = flow.clone();
            let banner2 = banner.clone();
            let toast2 = toast.clone();
            let id = gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(150),
                move || {
                    ui2.borrow_mut().filter.query = teks.clone();
                    muat_ulang(&state2, &ui2, &flow2, &banner2, &toast2);
                },
            );
            ui.borrow_mut().debounce = Some(id);
        });
    }

    // Sortir.
    {
        let ui = ui.clone();
        let state = state.clone();
        let flow2 = flow.clone();
        let banner2 = banner.clone();
        let toast2 = toast.clone();
        let aksi = gtk::gio::SimpleActionGroup::new();
        for (nama, kunci) in [
            ("mine", SortKey::Mine),
            ("newest", SortKey::Newest),
            ("recent", SortKey::Recent),
            ("name", SortKey::Name),
            ("size", SortKey::Size),
            ("resolution", SortKey::Resolution),
            ("favorite", SortKey::Favorite),
        ] {
            let aksi_item = gtk::gio::SimpleAction::new(nama, None);
            let ui = ui.clone();
            let state = state.clone();
            let flow = flow2.clone();
            let banner = banner2.clone();
            let toast = toast2.clone();
            aksi_item.connect_activate(move |_, _| {
                ui.borrow_mut().sort = kunci;
                let mut cfg = state.config.borrow_mut();
                cfg.ui.sort = match kunci {
                    SortKey::Mine => vallentheme_core::config::SortSetting::Mine,
                    SortKey::Newest => vallentheme_core::config::SortSetting::Newest,
                    SortKey::Recent => vallentheme_core::config::SortSetting::Recent,
                    SortKey::Name => vallentheme_core::config::SortSetting::Name,
                    SortKey::Size => vallentheme_core::config::SortSetting::Size,
                    SortKey::Resolution => vallentheme_core::config::SortSetting::Resolution,
                    SortKey::Favorite => vallentheme_core::config::SortSetting::Favorite,
                };
                let cfg = cfg.clone();
                drop(cfg);
                muat_ulang(&state, &ui, &flow, &banner, &toast);
            });
            aksi.add_action(&aksi_item);
        }
        urut.insert_action_group("sort", Some(&aksi));
    }

    // Muat pertama kali.
    muat_ulang(&state, &ui, &flow, &banner, &toast);

    halaman
}

/// Bilah bawah dengan tombol impor dan mode Pilih.
fn bilah_bawah(
    state: Shared,
    ui: Rc<RefCell<Ui>>,
    flow: gtk::FlowBox,
    nav: adw::NavigationView,
    toast: adw::ToastOverlay,
) -> gtk::Widget {
    let bilah = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bilah.add_css_class("vt-bottombar");

    let hitung = gtk::Label::new(Some(&format!("0 {}", tr("wallpapers"))));
    ui.borrow_mut().hitung = Some(hitung.clone());
    hitung.add_css_class("vt-count");
    hitung.set_xalign(0.0);
    hitung.set_hexpand(true);

    let tambah_folder = tombol(&tr("Add folder"), "folder-symbolic", "vt-secondary");
    let tambah = tombol(&tr("Add"), "list-add-symbolic", "vt-primary");
    let pilih = tombol(&tr("Select"), "object-select-symbolic", "vt-secondary");

    // Tambah folder.
    {
        let state = state.clone();
        let ui = ui.clone();
        let flow = flow.clone();
        let toast2 = toast.clone();
        tambah_folder.clone().connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder().title(tr("Add folder")).build();
            let state = state.clone();
            let ui = ui.clone();
            let flow = flow.clone();
            let toast3 = toast2.clone();
            dialog.select_folder(
                None::<&gtk::Window>,
                None::<&gtk::gio::Cancellable>,
                move |hasil| {
                    let Ok(berkas) = hasil else { return };
                    let Some(path) = berkas.path() else { return };
                    impor_folder(&state, &ui, &flow, &toast3, path);
                },
            );
        });
    }

    // Tambah berkas.
    {
        let state = state.clone();
        let ui = ui.clone();
        let flow = flow.clone();
        let toast2 = toast.clone();
        tambah.clone().connect_clicked(move |_| {
            let dialog = gtk::FileDialog::builder().title(tr("Add")).build();
            let state = state.clone();
            let ui = ui.clone();
            let flow = flow.clone();
            let toast3 = toast2.clone();
            dialog.open_multiple(
                None::<&gtk::Window>,
                None::<&gtk::gio::Cancellable>,
                move |hasil| {
                    let Ok(model) = hasil else { return };
                    let mut paths = Vec::new();
                    for i in 0..model.n_items() {
                        if let Some(obj) = model.item(i) {
                            if let Ok(berkas) = obj.downcast::<gtk::gio::File>() {
                                if let Some(p) = berkas.path() {
                                    paths.push(p);
                                }
                            }
                        }
                    }
                    impor_berkas(&state, &ui, &flow, &toast3, paths);
                },
            );
        });
    }

    // Mode Pilih.
    {
        let ui2 = ui.clone();
        let pilih2 = pilih.clone();
        pilih.clone().connect_clicked(move |_| {
            let aktif = {
                let mut u = ui2.borrow_mut();
                u.mode_pilih = !u.mode_pilih;
                if !u.mode_pilih {
                    u.terpilih.clear();
                }
                u.mode_pilih
            };
            let _ = aktif;
            pilih2.set_label(&tr("Select"));
            pilih2.set_tooltip_text(Some(&tr("Select")));
            perbarui_hitungan(&ui2);
        });
    }

    // Pintasan hapus saat ada pilihan.
    let hapus = tombol(&tr("Missing"), "user-trash-symbolic", "vt-secondary");
    hapus.set_visible(false);
    {
        let state = state.clone();
        let ui = ui.clone();
        let flow = flow.clone();
        let toast2 = toast.clone();
        hapus.clone().connect_clicked(move |_| {
            hapus_terpilih(&state, &ui, &flow, &toast2);
        });
    }

    bilah.append(&hitung);
    bilah.append(&tambah_folder);
    bilah.append(&tambah);
    bilah.append(&hapus);
    bilah.append(&pilih);

    let _ = nav;
    bilah.upcast()
}

fn tombol(teks: &str, ikon: &str, kelas: &str) -> gtk::Button {
    let isi = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    isi.append(&gtk::Image::from_icon_name(ikon));
    isi.append(&gtk::Label::new(Some(teks)));
    let b = gtk::Button::builder().child(&isi).build();
    b.add_css_class(kelas);
    b.set_tooltip_text(Some(teks));
    b.update_property(&[gtk::accessible::Property::Label(teks)]);
    b
}

fn menu_urut() -> gtk::gio::Menu {
    let menu = gtk::gio::Menu::new();
    let bagian = gtk::gio::Menu::new();
    for (nama, label) in [
        ("sort.mine", "My order"),
        ("sort.newest", "Recent"),
        ("sort.recent", "Recent"),
        ("sort.name", "Recent"),
        ("sort.size", "Recent"),
        ("sort.resolution", "Recent"),
        ("sort.favorite", "Favorites"),
    ] {
        bagian.append(Some(&tr(label)), Some(nama));
    }
    menu.append_section(None, &bagian);
    menu
}

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

/// Muat ulang daftar dari basis data (pekerjaan di thread terpisah).
fn muat_ulang(
    state: &Shared,
    ui: &Rc<RefCell<Ui>>,
    flow: &gtk::FlowBox,
    banner: &gtk::Label,
    toast: &adw::ToastOverlay,
) {
    let filter = ui.borrow().filter.clone();
    let sort = ui.borrow().sort;
    let db = std::sync::Arc::clone(&state.db);
    let dir_thumb = state.paths.thumbs_dir.clone();

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let hasil = db
            .call(move |db| {
                // Buat thumbnail yang belum ada (ffmpeg, di luar main thread).
                let awal = db.list(&filter, sort, 2000, 0)?;
                for w in &awal {
                    let sudah = w.thumb_path.as_ref().map(|p| p.is_file()).unwrap_or(false);
                    if sudah || w.missing {
                        continue;
                    }
                    if let Err(err) = scan::buat_thumbnail(
                        db,
                        w.id,
                        &dir_thumb,
                        w.mtime,
                        w.size_bytes,
                        w.duration_ms,
                    ) {
                        tracing::warn!(wallpaper = w.id.0, error = %err, "thumbnail gagal dibuat");
                    }
                }
                let daftar = db.list(&filter, sort, 2000, 0)?;
                let total = db.count(&filter)?;
                Ok((daftar, total))
            })
            .map_err(|e| e.to_string());
        let _ = tx.send(hasil);
    });

    let rx = Rc::new(RefCell::new(rx));
    let state2 = state.clone();
    let ui2 = ui.clone();
    let flow2 = flow.clone();
    let banner2 = banner.clone();
    let toast2 = toast.clone();

    gtk::glib::timeout_add_local(std::time::Duration::from_millis(40), move || {
        let hasil = rx.borrow().try_recv();
        match hasil {
            Ok(Ok((daftar, total))) => {
                ui2.borrow_mut().jumlah_total = total;
                isi_kartu(&state2, &ui2, &flow2, &toast2, &daftar);
                perbarui_hitungan(&ui2);
                banner2.set_visible(false);
                gtk::glib::ControlFlow::Break
            }
            Ok(Err(pesan)) => {
                tracing::error!(error = %pesan, "gagal memuat library");
                banner2.set_text(&format!("{} — {pesan}", tr("Failed")));
                banner2.set_visible(true);
                toast2.add_toast(adw::Toast::new(&tr("Failed")));
                gtk::glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
        }
    });
}

/// Isi ulang `FlowBox` dengan kartu.
fn isi_kartu(
    state: &Shared,
    ui: &Rc<RefCell<Ui>>,
    flow: &gtk::FlowBox,
    toast: &adw::ToastOverlay,
    daftar: &[Wallpaper],
) {
    while let Some(anak) = flow.first_child() {
        flow.remove(&anak);
    }

    for w in daftar {
        let kartu = kartu_wallpaper(state, ui, flow, toast, w);
        flow.insert(&kartu, -1);
    }

    if daftar.is_empty() {
        let kosong = adw::StatusPage::builder()
            .icon_name("image-x-generic-symbolic")
            .title(tr("No wallpapers yet"))
            .description(tr("Add a photo or video to get started"))
            .build();
        flow.insert(&kosong, -1);
    }
}

/// Satu kartu wallpaper nyata.
fn kartu_wallpaper(
    state: &Shared,
    ui: &Rc<RefCell<Ui>>,
    flow: &gtk::FlowBox,
    toast: &adw::ToastOverlay,
    w: &Wallpaper,
) -> gtk::Widget {
    let kartu = gtk::Box::new(gtk::Orientation::Vertical, 0);
    kartu.add_css_class("wp-card");
    kartu.set_size_request(232, 160);
    if w.missing {
        kartu.add_css_class("missing");
    }

    let gambar = gtk::Picture::new();
    gambar.add_css_class("wp-thumb");
    gambar.set_content_fit(gtk::ContentFit::Cover);
    gambar.set_can_shrink(true);
    gambar.set_size_request(-1, 112);
    gambar.set_vexpand(true);
    if let Some(thumb) = &w.thumb_path {
        if thumb.is_file() {
            gambar.set_filename(Some(thumb));
        }
    }

    let lencana = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .halign(gtk::Align::Start)
        .valign(gtk::Align::Start)
        .margin_start(8)
        .margin_top(8)
        .build();
    if w.kind == MediaKind::Video {
        lencana.append(&badge(&tr("Video"), ""));
    }
    if let Some(lebar) = w.width {
        if lebar >= 3840 {
            lencana.append(&badge("4K", "res"));
        } else if lebar >= 2560 {
            lencana.append(&badge("2K", "res"));
        } else if lebar >= 1920 {
            lencana.append(&badge("FHD", "res"));
        }
    }
    if w.missing {
        lencana.append(&badge(&tr("Missing"), "missing"));
    }

    let lapis = gtk::Overlay::new();
    lapis.set_child(Some(&gambar));
    lapis.add_overlay(&lencana);

    let teks = gtk::Box::new(gtk::Orientation::Vertical, 2);
    teks.set_margin_start(10);
    teks.set_margin_end(10);
    teks.set_margin_top(8);
    teks.set_margin_bottom(10);

    let nama = gtk::Label::new(Some(&w.title));
    nama.add_css_class("wp-name");
    nama.set_xalign(0.0);
    nama.set_ellipsize(gtk::pango::EllipsizeMode::End);
    nama.set_tooltip_text(Some(&w.title));

    let meta = gtk::Label::new(Some(&meta_teks(w)));
    meta.add_css_class("wp-meta");
    meta.set_xalign(0.0);
    meta.set_ellipsize(gtk::pango::EllipsizeMode::End);

    teks.append(&nama);
    teks.append(&meta);

    // Tombol favorit.
    let fav = gtk::Button::from_icon_name(if w.favorite {
        "starred-symbolic"
    } else {
        "non-starred-symbolic"
    });
    fav.add_css_class("vt-flat");
    fav.set_tooltip_text(Some(&tr("Favorites")));
    {
        let state = state.clone();
        let ui = ui.clone();
        let flow = flow.clone();
        let toast = toast.clone();
        let id = w.id;
        let sekarang = w.favorite;
        let fav2 = fav.clone();
        fav.connect_clicked(move |_| {
            let baru = !sekarang;
            let hasil = state.db.call(move |db| db.set_favorite(id, baru));
            if hasil.is_ok() {
                fav2.set_icon_name(if baru {
                    "starred-symbolic"
                } else {
                    "non-starred-symbolic"
                });
            } else {
                toast.add_toast(adw::Toast::new(&tr("Failed")));
            }
            muat_ulang(&state, &ui, &flow, &gtk::Label::new(None), &toast);
        });
    }

    let aksi = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    aksi.add_css_class("wp-actions");
    aksi.set_halign(gtk::Align::End);
    aksi.set_valign(gtk::Align::End);
    aksi.set_margin_end(8);
    aksi.set_margin_bottom(8);
    aksi.append(&fav);

    lapis.add_overlay(&aksi);
    kartu.append(&lapis);
    kartu.append(&teks);

    // Klik kartu → toggle pilih atau buka Edit.
    let gestur = gtk::GestureClick::new();
    {
        let state = state.clone();
        let ui = ui.clone();
        let id = w.id;
        let toast = toast.clone();
        gestur.connect_released(move |_, _, _, _| {
            let mode = ui.borrow().mode_pilih;
            if mode {
                let mut u = ui.borrow_mut();
                if let Some(pos) = u.terpilih.iter().position(|x| *x == id) {
                    u.terpilih.remove(pos);
                } else {
                    u.terpilih.push(id);
                }
            } else {
                tracing::info!(wallpaper = id.0, "membuka halaman Edit (menyusul Fase 6)");
                let _ = &toast;
                let _ = &state;
            }
        });
    }
    kartu.add_controller(gestur);
    kartu.set_cursor_from_name(Some("pointer"));

    kartu.upcast()
}

fn meta_teks(w: &Wallpaper) -> String {
    let ukuran = format!("{:.1} MB", w.size_bytes as f64 / 1_048_576.0);
    match (w.width, w.height, w.fps) {
        (Some(l), Some(t), Some(fps)) if w.kind == MediaKind::Video => {
            format!("{l}×{t} · {fps:.0}fps · {ukuran}")
        }
        (Some(l), Some(t), _) => format!("{l}×{t} · {ukuran}"),
        _ => ukuran,
    }
}

fn badge(teks: &str, kelas: &str) -> gtk::Widget {
    let label = gtk::Label::new(Some(teks));
    label.add_css_class("wp-badge");
    if !kelas.is_empty() {
        label.add_css_class(kelas);
    }
    label.upcast()
}

fn perbarui_hitungan(ui: &Rc<RefCell<Ui>>) {
    let u = ui.borrow();
    let Some(hitung) = u.hitung.as_ref() else {
        return;
    };
    if u.mode_pilih {
        hitung.set_text(&format!("{} {}", u.terpilih.len(), tr("Select")));
    } else {
        hitung.set_text(&format!("{} {}", u.jumlah_total, tr("wallpapers")));
    }
}

fn hapus_terpilih(
    state: &Shared,
    ui: &Rc<RefCell<Ui>>,
    flow: &gtk::FlowBox,
    toast: &adw::ToastOverlay,
) {
    let ids = ui.borrow().terpilih.clone();
    if ids.is_empty() {
        return;
    }
    let jumlah = ids.len();
    let hasil = state.db.call(move |db| db.soft_delete(&ids));
    match hasil {
        Ok(()) => {
            ui.borrow_mut().terpilih.clear();
            let t = adw::Toast::new(&format!("{} {}", jumlah, tr("Missing")));
            toast.add_toast(t);
            muat_ulang(state, ui, flow, &gtk::Label::new(None), toast);
            perbarui_hitungan(ui);
        }
        Err(e) => {
            tracing::error!(error = %e, "gagal menghapus dari library");
            toast.add_toast(adw::Toast::new(&tr("Failed")));
        }
    }
}

/// Impor berkas yang dipilih pengguna.
fn impor_berkas(
    state: &Shared,
    ui: &Rc<RefCell<Ui>>,
    flow: &gtk::FlowBox,
    toast: &adw::ToastOverlay,
    paths: Vec<PathBuf>,
) {
    if paths.is_empty() {
        return;
    }
    let db = std::sync::Arc::clone(&state.db);
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let hasil = db
            .call(move |db| {
                let batal = AtomicBool::new(false);
                scan::import_paths(db, &paths, None, &batal, |_, _| {})
            })
            .map(|l| l.ditambah)
            .map_err(|e| e.to_string());
        let _ = tx.send(hasil);
    });

    tunggu_impor(state, ui, flow, toast, rx);
}

/// Impor seluruh isi folder.
fn impor_folder(
    state: &Shared,
    ui: &Rc<RefCell<Ui>>,
    flow: &gtk::FlowBox,
    toast: &adw::ToastOverlay,
    folder: PathBuf,
) {
    let db = std::sync::Arc::clone(&state.db);
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        let batal = AtomicBool::new(false);
        let berkas = scan::collect_files(
            std::slice::from_ref(&folder),
            &ScanOptions::default(),
            &batal,
        );
        let hasil = db
            .call(move |db| {
                let id_folder = db.insert_folder(&folder, true)?;
                let batal = AtomicBool::new(false);
                scan::import_paths(db, &berkas, Some(id_folder), &batal, |_, _| {})
            })
            .map(|l| l.ditambah)
            .map_err(|e| e.to_string());
        let _ = tx.send(hasil);
    });

    tunggu_impor(state, ui, flow, toast, rx);
}

/// Tunggu hasil impor tanpa memblokir main loop.
fn tunggu_impor(
    state: &Shared,
    ui: &Rc<RefCell<Ui>>,
    flow: &gtk::FlowBox,
    toast: &adw::ToastOverlay,
    rx: std::sync::mpsc::Receiver<Result<usize, String>>,
) {
    let rx = Rc::new(RefCell::new(rx));
    let state2 = state.clone();
    let ui2 = ui.clone();
    let flow2 = flow.clone();
    let toast2 = toast.clone();

    gtk::glib::timeout_add_local(std::time::Duration::from_millis(80), move || {
        let hasil = rx.borrow().try_recv();
        match hasil {
            Ok(Ok(jumlah)) => {
                toast2.add_toast(adw::Toast::new(&format!("{jumlah} {}", tr("wallpapers"))));
                muat_ulang(&state2, &ui2, &flow2, &gtk::Label::new(None), &toast2);
                perbarui_hitungan(&ui2);
                gtk::glib::ControlFlow::Break
            }
            Ok(Err(pesan)) => {
                tracing::error!(error = %pesan, "impor gagal");
                toast2.add_toast(adw::Toast::new(&tr("Failed")));
                gtk::glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => gtk::glib::ControlFlow::Break,
        }
    });
}
