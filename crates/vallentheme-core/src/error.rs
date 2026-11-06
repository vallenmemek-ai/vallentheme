//! Tipe error tunggal untuk seluruh crate inti.
//!
//! Setiap varian membawa **kode** (`VT-<KATEGORI>-<NNN>`) dan **pesan Indonesia
//! yang ramah** untuk ditampilkan ke pengguna.

use std::path::PathBuf;

/// Hasil standar crate inti.
pub type Result<T> = std::result::Result<T, Error>;

/// Semua kegagalan yang bisa muncul dari crate inti.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Kegagalan I/O pada path tertentu.
    #[error("gagal mengakses {path}: {source}")]
    Io {
        /// Path yang sedang diproses.
        path: PathBuf,
        /// Sumber kesalahan dari sistem.
        #[source]
        source: std::io::Error,
    },

    /// Kegagalan I/O tanpa path yang jelas.
    #[error("kesalahan I/O: {0}")]
    IoBare(#[from] std::io::Error),

    /// Isi konfigurasi tidak masuk akal (misalnya tipe salah).
    #[error("konfigurasi tidak valid: {0}")]
    ConfigInvalid(String),

    /// Konfigurasi tidak dapat dibaca.
    #[error("gagal membaca konfigurasi: {0}")]
    ConfigLoad(String),

    /// Konfigurasi tidak dapat disimpan.
    #[error("gagal menyimpan konfigurasi: {0}")]
    ConfigSave(String),

    /// Direktori data pengguna tidak dapat ditentukan.
    #[error("direktori data pengguna tidak tersedia")]
    NoDataDir,
}

impl Error {
    /// Kode stabil yang bisa ditampilkan ke pengguna dan dicari di dokumentasi.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } | Self::IoBare(_) => "VT-IO-001",
            Self::ConfigInvalid(_) => "VT-CONFIG-001",
            Self::ConfigLoad(_) => "VT-CONFIG-002",
            Self::ConfigSave(_) => "VT-CONFIG-003",
            Self::NoDataDir => "VT-CONFIG-004",
        }
    }

    /// Pesan singkat berbahasa Indonesia: apa yang terjadi dan apa yang bisa dilakukan.
    pub fn user_message(&self) -> String {
        match self {
            Self::Io { .. } | Self::IoBare(_) => {
                "vallentheme tidak bisa mengakses berkas yang dibutuhkan. \
                 Periksa izin folder dan ruang disk, lalu coba lagi."
                    .to_string()
            }
            Self::ConfigInvalid(_) => "Berkas pengaturan vallentheme tidak bisa dibaca. \
                 Pengaturan akan dikembalikan ke bawaan."
                .to_string(),
            Self::ConfigLoad(_) => "Pengaturan vallentheme gagal dimuat. \
                 Coba jalankan ulang aplikasi; bila tetap gagal, hapus \
                 ~/.config/vallentheme/config.toml."
                .to_string(),
            Self::ConfigSave(_) => "Pengaturan vallentheme gagal disimpan. \
                 Periksa ruang disk dan izin folder ~/.config/vallentheme."
                .to_string(),
            Self::NoDataDir => "Folder data pengguna tidak ditemukan. \
                 Pastikan variabel HOME atau XDG_DATA_HOME sudah diatur."
                .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn setiap_varian_punya_kode_dan_pesan() {
        let varian = [
            Error::IoBare(std::io::Error::other("uji")),
            Error::ConfigInvalid("x".into()),
            Error::ConfigLoad("x".into()),
            Error::ConfigSave("x".into()),
            Error::NoDataDir,
        ];
        for e in &varian {
            let kode = e.code();
            let bagian: Vec<&str> = kode.split('-').collect();
            assert_eq!(bagian.len(), 3, "format kode harus VT-KATEGORI-NNN: {kode}");
            assert_eq!(bagian[0], "VT");
            assert_eq!(bagian[2].len(), 3, "nomor harus 3 digit: {kode}");
            assert!(!e.user_message().is_empty());
        }
    }

    #[test]
    fn kode_berbeda_untuk_varian_berbeda() {
        assert_eq!(Error::NoDataDir.code(), "VT-CONFIG-004");
        assert_eq!(Error::ConfigInvalid("a".into()).code(), "VT-CONFIG-001");
    }
}
