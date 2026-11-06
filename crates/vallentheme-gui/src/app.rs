//! Penyiapan `adw::Application`: tema, CSS, dan aksi global.

use gtk::gio;
use gtk::prelude::*;

use vallentheme_core::config::Config;
use vallentheme_core::paths::Paths;

use crate::window;

/// ID aplikasi (dipakai GApplication untuk instance tunggal).
pub const APP_ID: &str = "id.vallentheme.App";

/// Bangun aplikasi GTK siap dijalankan.
pub fn bangun(paths: Paths, config: Config) -> adw::Application {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::empty())
        .build();

    app.set_resource_base_path(Some("/id/vallentheme"));

    let paths_startup = paths.clone();
    let config_startup = config.clone();

    app.connect_startup(move |app| {
        muat_tema(app);
        daftar_aksi(app, &paths_startup, &config_startup);
    });

    app.connect_activate(|app| {
        if let Some(jendela) = app.active_window() {
            jendela.present();
            return;
        }
        let jendela = window::bangun(app);
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
fn daftar_aksi(app: &adw::Application, paths: &Paths, config: &Config) {
    let aksi_quit = gio::SimpleAction::new("quit", None);
    let app_quit = app.clone();
    aksi_quit.connect_activate(move |_, _| app_quit.quit());
    app.add_action(&aksi_quit);

    let config_file = paths.config_file.clone();
    let aksi_prefs = gio::SimpleAction::new("preferences", None);
    let app_prefs = app.clone();
    aksi_prefs.connect_activate(move |_, _| {
        tracing::info!(config = %config_file.display(), "aksi Pengaturan dipanggil");
        if let Some(jendela) = app_prefs.active_window() {
            jendela.present();
        }
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
    aksi_stop.connect_activate(|_, _| {
        tracing::info!("aksi Hentikan wallpaper dipanggil (daemon menyusul di Fase 5)");
    });
    app.add_action(&aksi_stop);

    let _ = config;

    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("app.preferences", &["<Control>comma"]);
    app.set_accels_for_action("app.stop", &["<Control>period"]);
}
