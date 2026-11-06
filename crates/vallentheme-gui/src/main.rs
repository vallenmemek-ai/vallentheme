//! Antarmuka vallentheme (GTK4 + libadwaita).
//!
//! Fase 0 hanya menyiapkan kerangka: menyiapkan folder, memuat konfigurasi,
//! menyiapkan logging, lalu keluar. Jendela dan tema dibangun di Fase 3.

use anyhow::Context;

fn main() -> anyhow::Result<()> {
    let paths = vallentheme_core::paths::Paths::resolve().context("menentukan folder XDG")?;
    paths.ensure().context("menyiapkan folder aplikasi")?;

    let _log_guard = vallentheme_core::logging::init_logging("vallentheme-gui", &paths.logs_dir)
        .context("menyiapkan logging")?;

    let config =
        vallentheme_core::config::Config::load(&paths.config_file).context("memuat konfigurasi")?;

    tracing::info!(
        versi = env!("CARGO_PKG_VERSION"),
        bahasa = ?config.general.language,
        "vallentheme-gui mulai (kerangka Fase 0)"
    );
    tracing::info!("antarmuka belum dibangun; keluar dengan bersih");

    Ok(())
}
