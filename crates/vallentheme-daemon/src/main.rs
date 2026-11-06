//! Daemon vallentheme: proses latar yang menerapkan dan merawat wallpaper.
//!
//! Fase 0 hanya menyiapkan kerangka: menyiapkan folder, memuat konfigurasi,
//! menyiapkan logging, lalu keluar. D-Bus, state machine, dan pengawasan mpv
//! ditambahkan di Fase 5.

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let paths = vallentheme_core::paths::Paths::resolve().context("menentukan folder XDG")?;
    paths.ensure().context("menyiapkan folder aplikasi")?;

    let _log_guard = vallentheme_core::logging::init_logging("vallentheme-daemon", &paths.logs_dir)
        .context("menyiapkan logging")?;

    let config =
        vallentheme_core::config::Config::load(&paths.config_file).context("memuat konfigurasi")?;

    tracing::info!(
        versi = vallentheme_render::VERSION,
        mode_kinerja = ?config.performance.mode,
        "vallentheme-daemon mulai (kerangka Fase 0)"
    );
    tracing::info!("tidak ada wallpaper yang diterapkan; keluar dengan bersih");

    Ok(())
}
