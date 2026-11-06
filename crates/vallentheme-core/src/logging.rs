//! Inisialisasi logging untuk seluruh binary vallentheme.
//!
//! Keluaran ganda: `stderr` (untuk pengembangan) dan berkas harian di
//! `~/.local/state/vallentheme/logs/`. Berkas lama dipangkas otomatis.

use std::fs;
use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

use crate::error::Result;

/// Jumlah berkas log yang disimpan per komponen.
const MAX_LOG_FILES: usize = 5;

/// Level bawaan bila `RUST_LOG` tidak diatur.
const DEFAULT_FILTER: &str = "info";

/// Siapkan logging untuk satu komponen (`vallentheme-gui`, `vallentheme-daemon`, …).
///
/// Nilai balik **wajib** disimpan sampai program selesai; bila `WorkerGuard`
/// dibuang, penulis berkas berhenti dan log terakhir hilang.
pub fn init_logging(component: &str, logs_dir: &Path) -> Result<Option<WorkerGuard>> {
    if let Err(err) = fs::create_dir_all(logs_dir) {
        eprintln!("peringatan: folder log tidak bisa dibuat: {err}");
        return Ok(None);
    }

    prune_old_logs(logs_dir, component, MAX_LOG_FILES);

    let file_appender = tracing_appender::rolling::daily(logs_dir, component);
    let (writer, guard) = tracing_appender::non_blocking(file_appender);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true);

    let result = tracing_subscriber::registry()
        .with(filter)
        .with(stderr_layer)
        .with(file_layer)
        .try_init();

    if result.is_err() {
        // Sudah diinisialisasi (misalnya di dalam test). Bukan kegagalan fatal.
        eprintln!("peringatan: logging sudah diinisialisasi sebelumnya");
        return Ok(None);
    }

    Ok(Some(guard))
}

/// Hapus berkas log lama sehingga hanya `keep` berkas terbaru yang tersisa.
fn prune_old_logs(dir: &Path, component: &str, keep: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let mut milik_komponen: Vec<(std::time::SystemTime, std::path::PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(component))
        .filter_map(|entry| {
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();

    if milik_komponen.len() <= keep {
        return;
    }

    milik_komponen.sort_by_key(|(waktu, _)| std::cmp::Reverse(*waktu));
    for (_, path) in milik_komponen.into_iter().skip(keep) {
        if let Err(err) = fs::remove_file(&path) {
            tracing::debug!(error = %err, path = %path.display(), "gagal menghapus log lama");
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn log_lama_dipangkas() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..8 {
            let name = format!("vallentheme-gui.2026-01-0{}.log", i + 1);
            fs::write(dir.path().join(name), "x").unwrap();
        }
        fs::write(dir.path().join("komponen-lain.log"), "x").unwrap();

        prune_old_logs(dir.path(), "vallentheme-gui", 3);

        let tersisa: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();

        let milik_gui = tersisa
            .iter()
            .filter(|n| n.starts_with("vallentheme-gui"))
            .count();
        assert_eq!(milik_gui, 3, "harus menyisakan 3 berkas: {tersisa:?}");
        assert!(
            tersisa.iter().any(|n| n == "komponen-lain.log"),
            "berkas komponen lain tidak boleh dihapus"
        );
    }
}
