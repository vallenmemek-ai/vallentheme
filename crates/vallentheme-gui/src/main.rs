//! Antarmuka vallentheme (GTK4 + libadwaita).
//!
//! Beberapa komponen (chip status lain, halaman Edit, keadaan kosong) baru
//! dipakai pada Fase 4–6, jadi sementara ditandai `allow(dead_code)`.
#![allow(dead_code)]

mod app;
mod i18n;
mod pages;
mod widgets;
mod window;

use anyhow::Context;
use gtk::prelude::*;

/// Pesan bantuan saat tidak ada sesi grafis.
const PESAN_TANPA_DISPLAY: &str =
    "vallentheme membutuhkan sesi grafis (X11). Variabel DISPLAY tidak diatur.\n\
     Jalankan dari dalam sesi desktop, atau pakai `xvfb-run` untuk pengujian.";

fn main() -> anyhow::Result<()> {
    let paths = vallentheme_core::paths::Paths::resolve().context("menentukan folder XDG")?;
    paths.ensure().context("menyiapkan folder aplikasi")?;

    let _log_guard = vallentheme_core::logging::init_logging("vallentheme-gui", &paths.logs_dir)
        .context("menyiapkan logging")?;

    let config =
        vallentheme_core::config::Config::load(&paths.config_file).context("memuat konfigurasi")?;

    // Tanpa display, GTK gagal dengan pesan yang membingungkan. Beri pesan jelas.
    let ada_display =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if !ada_display {
        eprintln!("{PESAN_TANPA_DISPLAY}");
        tracing::error!("tidak ada DISPLAY; keluar");
        std::process::exit(1);
    }

    tracing::info!(
        versi = env!("CARGO_PKG_VERSION"),
        bahasa = ?config.general.language,
        "vallentheme-gui mulai"
    );

    let aplikasi = app::bangun(paths, config);
    let kode = aplikasi.run();
    tracing::info!("vallentheme-gui berhenti");
    std::process::exit(i32::from(kode));
}
