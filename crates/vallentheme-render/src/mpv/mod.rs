//! Pengawasan pemutar media mpv.
//!
//! Modul ini menyusun argumen mpv, berbicara lewat protokol IPC JSON per baris,
//! dan menjaga proses mpv tetap hidup dengan backoff. Rujukan:
//! `panduan/06-referensi-teknis.md` bagian 2.

pub mod args;
pub mod ipc;
pub mod supervisor;

use std::path::PathBuf;

use vallentheme_core::model::WallpaperSettings;

/// Semua yang dibutuhkan untuk menjalankan satu instans mpv.
#[derive(Debug, Clone)]
pub struct PlaySpec {
    /// XID window desktop tempat mpv menggambar.
    pub wid: u32,
    /// Path soket IPC khusus instans ini.
    pub socket: PathBuf,
    /// Berkas media yang diputar.
    pub path: PathBuf,
    /// Pengaturan tampilan, audio, dan rotasi.
    pub settings: WallpaperSettings,
}

impl PlaySpec {
    /// Susun spesifikasi baru.
    pub fn new(wid: u32, socket: PathBuf, path: PathBuf, settings: WallpaperSettings) -> Self {
        Self {
            wid,
            socket,
            path,
            settings,
        }
    }
}
