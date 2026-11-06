//! Antarmuka vallentheme (GTK4 + libadwaita).
//!
//! Beberapa komponen (chip status lain, halaman Edit, keadaan kosong) baru
//! dipakai pada Fase 4–6, jadi sementara ditandai `allow(dead_code)`.
#![allow(dead_code)]

mod app;
mod dbus_client;
mod edit;
mod i18n;
mod library;
mod pages;
mod state;
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

    // Basis data + thread khusus agar I/O tidak menyentuh main loop.
    let (db, cadangan) =
        vallentheme_core::db::Db::open_recovering(&paths.db_file).context("membuka library")?;
    if let Some(cadangan) = cadangan {
        tracing::warn!(berkas = %cadangan.display(), "library lama dicadangkan");
    }
    let actor =
        vallentheme_core::db_actor::DbActor::spawn(db, 32).context("menyiapkan aktor DB")?;

    // Jalur bantu untuk pengujian/otomasi: impor lalu keluar.
    let argumen: Vec<String> = std::env::args().collect();
    if let Some(pos) = argumen.iter().position(|a| a == "--import") {
        if let Some(path) = argumen.get(pos + 1) {
            let batal = std::sync::atomic::AtomicBool::new(false);
            let folder = std::path::PathBuf::from(path);
            let berkas = vallentheme_core::scan::collect_files(
                std::slice::from_ref(&folder),
                &vallentheme_core::scan::ScanOptions::default(),
                &batal,
            );
            let laporan = actor.call(move |db| {
                let id = db.insert_folder(&folder, true)?;
                let batal = std::sync::atomic::AtomicBool::new(false);
                vallentheme_core::scan::import_paths(db, &berkas, Some(id), &batal, |_, _| {})
            })?;
            println!(
                "impor: {} ditambah, {} diperbarui, {} dilewati",
                laporan.ditambah,
                laporan.diperbarui,
                laporan.dilewati.len()
            );
            return Ok(());
        }
    }

    let shared = std::rc::Rc::new(state::AppState {
        paths: paths.clone(),
        db: std::sync::Arc::new(actor),
        config: std::cell::RefCell::new(config),
    });

    let aplikasi = app::bangun(shared);
    let kode = aplikasi.run();
    tracing::info!("vallentheme-gui berhenti");
    std::process::exit(i32::from(kode));
}
