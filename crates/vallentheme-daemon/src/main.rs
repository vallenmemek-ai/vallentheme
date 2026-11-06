//! Daemon vallentheme: menerapkan dan merawat wallpaper.
//!
//! Binary ini memegang satu instance (flock), menyediakan layanan D-Bus
//! `id.vallentheme.Daemon`, memulihkan wallpaper saat login (`--restore`), dan
//! membersihkan mpv serta pengaturan desktop saat menerima SIGTERM/SIGINT.

mod dbus;
mod state;

use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use vallentheme_core::config::Config;
use vallentheme_core::logging::init_logging;
use vallentheme_core::paths::Paths;

use crate::dbus::{DaemonService, NAMA_BUS, PATH_OBJEK};

/// Flag baris perintah yang dikenali.
#[derive(Debug, Default)]
struct Flag {
    restore: bool,
    install_autostart: bool,
    remove_autostart: bool,
    daemonize: bool,
}

fn main() -> anyhow::Result<()> {
    let flag = baca_flag();

    let paths = Paths::resolve().context("menentukan folder XDG")?;
    paths.ensure().context("menyiapkan folder aplikasi")?;

    let _log_guard =
        init_logging("vallentheme-daemon", &paths.logs_dir).context("menyiapkan logging")?;

    if flag.install_autostart {
        pasang_autostart(&paths)?;
        return Ok(());
    }
    if flag.remove_autostart {
        hapus_autostart(&paths)?;
        return Ok(());
    }
    if flag.daemonize {
        // Diterima agar skrip lama tidak gagal; daemon sudah berjalan di latar.
        tracing::info!("flag --daemonize diterima; proses tetap di depan");
    }

    let _lock = ambil_lock(&paths.runtime_dir.join("daemon.lock"))?;

    let config = Config::load(&paths.config_file).context("memuat konfigurasi")?;
    tracing::info!(
        versi = vallentheme_render::VERSION,
        mode_kinerja = ?config.performance.mode,
        "vallentheme-daemon mulai"
    );

    let state_ada = paths.daemon_state_file.is_file();
    let service = DaemonService::new(paths.clone());

    if flag.restore {
        if state_ada {
            service.pulihkan_semua();
        } else {
            tracing::warn!(
                kode = "VT-IO-001",
                "berkas state tidak ditemukan; wallpaper terakhir tidak bisa dipulihkan"
            );
        }
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("membuat runtime tokio")?;

    runtime.block_on(jalankan(service, paths))
}

/// Jalankan layanan D-Bus sampai diminta berhenti.
async fn jalankan(service: DaemonService, paths: Paths) -> anyhow::Result<()> {
    let koneksi = zbus::Connection::session()
        .await
        .context("tidak bisa terhubung ke bus sesi D-Bus")?;

    koneksi
        .object_server()
        .at(PATH_OBJEK, service.clone())
        .await
        .context("tidak bisa mendaftarkan objek D-Bus")?;

    koneksi
        .request_name(NAMA_BUS)
        .await
        .context("nama id.vallentheme.Daemon sudah dipakai proses lain")?;

    tracing::info!("layanan D-Bus siap di {NAMA_BUS}");

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("memasang penangan SIGTERM")?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .context("memasang penangan SIGINT")?;

    tokio::select! {
        _ = service.tunggu_berhenti() => {
            tracing::info!("Quit diterima; keluar dengan bersih");
        }
        _ = sigterm.recv() => {
            tracing::info!("SIGTERM diterima; keluar dengan bersih");
            service.minta_berhenti();
        }
        _ = sigint.recv() => {
            tracing::info!("SIGINT diterima; keluar dengan bersih");
            service.minta_berhenti();
        }
    }

    service.bersihkan();
    bersihkan_soket(&paths.runtime_dir);
    tracing::info!("vallentheme-daemon berhenti");
    Ok(())
}

/// Baca flag dari `std::env::args`; flag tak dikenal hanya dicatat.
fn baca_flag() -> Flag {
    let mut flag = Flag::default();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--restore" => flag.restore = true,
            "--install-autostart" => flag.install_autostart = true,
            "--remove-autostart" => flag.remove_autostart = true,
            "--daemonize" => flag.daemonize = true,
            lain => tracing::warn!(argumen = lain, "argumen tidak dikenal, diabaikan"),
        }
    }
    flag
}

/// Pegangan lock yang dilepas otomatis saat proses keluar.
struct InstanceLock {
    _file: File,
}

/// Ambil `flock` eksklusif non-blokir pada berkas lock.
fn ambil_lock(path: &Path) -> anyhow::Result<InstanceLock> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("membuat folder runtime")?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("membuka berkas lock {}", path.display()))?;

    // SAFETY: `flock` hanya menyentuh deskriptor berkas milik proses ini.
    #[allow(unsafe_code)]
    let hasil = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if hasil != 0 {
        bail!(
            "vallentheme-daemon sudah berjalan. Hentikan instance lama dahulu \
             (lock: {})",
            path.display()
        );
    }
    Ok(InstanceLock { _file: file })
}

/// Path berkas autostart menurut spesifikasi XDG.
fn path_autostart(paths: &Paths) -> PathBuf {
    let config = paths
        .config_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| paths.config_dir.clone());
    config
        .join("autostart")
        .join("id.vallentheme.Daemon.desktop")
}

/// Tulis berkas autostart yang menjalankan daemon dengan `--restore`.
fn pasang_autostart(paths: &Paths) -> anyhow::Result<()> {
    let path = path_autostart(paths);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("membuat folder autostart")?;
    }
    let exe = std::env::current_exe().context("menentukan path binary daemon")?;
    let isi = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=vallentheme\n\
         Comment=Wallpaper video dan foto vallentheme\n\
         Exec={} --restore\n\
         X-GNOME-Autostart-enabled=true\n\
         NoDisplay=true\n",
        exe.display()
    );
    fs::write(&path, isi).with_context(|| format!("menulis autostart {}", path.display()))?;
    tracing::info!(path = %path.display(), "autostart dipasang");
    Ok(())
}

/// Hapus berkas autostart bila ada.
fn hapus_autostart(paths: &Paths) -> anyhow::Result<()> {
    let path = path_autostart(paths);
    match fs::remove_file(&path) {
        Ok(()) => {
            tracing::info!(path = %path.display(), "autostart dihapus");
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!("autostart memang belum terpasang");
            Ok(())
        }
        Err(err) => Err(err).with_context(|| format!("menghapus autostart {}", path.display())),
    }
}

/// Hapus soket IPC mpv yang tertinggal.
fn bersihkan_soket(runtime_dir: &Path) {
    let Ok(entries) = fs::read_dir(runtime_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let nama = entry.file_name().to_string_lossy().into_owned();
        if nama.starts_with("mpv-") && nama.ends_with(".sock") {
            if let Err(err) = fs::remove_file(entry.path()) {
                tracing::debug!(error = %err, nama, "gagal menghapus soket mpv");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::state::DaemonState;

    #[test]
    fn lock_kedua_ditolak() {
        let dir = std::env::temp_dir().join(format!("vt-lock-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("daemon.lock");

        let pertama = ambil_lock(&path).unwrap();
        assert!(ambil_lock(&path).is_err(), "lock kedua harus ditolak");
        drop(pertama);
        assert!(ambil_lock(&path).is_ok(), "lock harus bisa diambil lagi");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_autostart_berada_di_config_autostart() {
        let paths = Paths::resolve().unwrap();
        let path = path_autostart(&paths);
        assert!(path.ends_with("autostart/id.vallentheme.Daemon.desktop"));
    }

    #[test]
    fn state_hilang_tidak_membuat_panic() {
        let path = std::env::temp_dir().join("vt-state-pasti-tidak-ada.json");
        let state = DaemonState::load(&path);
        assert!(state.monitors.is_empty());
    }
}
