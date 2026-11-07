//! Penyiapan `adw::Application`: tema, CSS, dan aksi global.

use gtk::gio;
use gtk::prelude::*;

use crate::state::Shared;

use crate::window;

/// ID aplikasi (dipakai GApplication untuk instance tunggal).
pub const APP_ID: &str = "id.vallentheme.App";

/// Bangun aplikasi GTK siap dijalankan.
pub fn bangun(state: Shared) -> adw::Application {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::empty())
        .build();

    app.set_resource_base_path(Some("/id/vallentheme"));

    let state_startup = state.clone();

    app.connect_startup(move |app| {
        // GResource hasil build.rs harus didaftarkan dulu agar
        // `/id/vallentheme/style.css` bisa dimuat.
        if let Err(err) = gio::resources_register_include!("vallentheme.gresource") {
            tracing::error!(error = %err, "gagal mendaftarkan GResource");
        }
        muat_tema(app);
        daftar_aksi(app, &state_startup);
    });

    app.connect_activate(move |app| {
        if let Some(jendela) = app.active_window() {
            jendela.present();
            return;
        }
        let jendela = window::bangun(app, state.clone());
        jendela.present();
    });

    app
}

/// Paksa tema gelap dan pasang CSS dari GResource.
fn muat_tema(app: &adw::Application) {
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);

    let provider = gtk::CssProvider::new();
    provider.load_from_resource("/id/vallentheme/style.css");

    if let Some(display) = gtk::gdk::Display::default() {
        #[allow(deprecated)]
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        // Ikon simbolik yang dibundel didaftarkan di sini saat sudah tersedia.
        gtk::IconTheme::for_display(&display).add_resource_path("/id/vallentheme/icons");
    } else {
        tracing::warn!("display tidak tersedia; tema tidak bisa dipasang");
    }

    let _ = app;
}

/// Daftarkan aksi global beserta pintasan papan tunas.
fn daftar_aksi(app: &adw::Application, state: &Shared) {
    let aksi_quit = gio::SimpleAction::new("quit", None);
    let app_quit = app.clone();
    aksi_quit.connect_activate(move |_, _| app_quit.quit());
    app.add_action(&aksi_quit);

    let aksi_prefs = gio::SimpleAction::new("preferences", None);
    let state_prefs = state.clone();
    aksi_prefs.connect_activate(move |_, _| {
        let (nav, toast) = (
            state_prefs.nav.borrow().clone(),
            state_prefs.toast.borrow().clone(),
        );
        let (Some(nav), Some(toast)) = (nav, toast) else {
            return;
        };
        // Halaman satu instance saja; aksi lain cukup kembali ke halaman itu.
        if let Some(halaman) = nav.find_page("pengaturan") {
            nav.pop_to_page(&halaman);
            return;
        }
        nav.push(&crate::settings::build(state_prefs.clone(), toast));
    });
    app.add_action(&aksi_prefs);

    // Dialog Tentang lengkap menyusul di Fase 8 bersama PreferencesDialog.
    let aksi_about = gio::SimpleAction::new("about", None);
    aksi_about.connect_activate(|_, _| {
        tracing::info!(
            versi = env!("CARGO_PKG_VERSION"),
            "aksi Tentang dipanggil; dialog lengkap menyusul di Fase 8"
        );
    });
    app.add_action(&aksi_about);

    let aksi_stop = gio::SimpleAction::new("stop", None);
    let state_stop = state.clone();
    aksi_stop.connect_activate(move |_, _| {
        let toast = state_stop.toast.borrow().clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::dbus_client::stop(""));
        });
        crate::edit::tunggu_stop(rx, toast);
    });
    app.add_action(&aksi_stop);

    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("app.preferences", &["<Control>comma"]);
    app.set_accels_for_action("app.stop", &["<Control>period"]);
}
