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

    /// Format atau jenis berkas tidak didukung.
    #[error("tidak didukung: {0}")]
    Unsupported(String),

    /// Kesalahan umum pada basis data.
    #[error("kesalahan basis data: {0}")]
    Db(String),

    /// Berkas basis data rusak.
    #[error("basis data rusak: {0}")]
    DbCorrupt(String),

    /// Basis data dibuat oleh versi vallentheme yang lebih baru.
    #[error("basis data versi {0} lebih baru dari yang didukung aplikasi ini")]
    DbNewerSchema(u32),

    /// `ffprobe` gagal membaca berkas.
    #[error("gagal membaca metadata media: {0}")]
    Probe(String),

    /// Program `ffprobe`/`ffmpeg` tidak terpasang.
    #[error("{0} belum terpasang")]
    ProbeToolMissing(String),

    /// Pembuatan thumbnail gagal.
    #[error("gagal membuat thumbnail: {0}")]
    Thumb(String),
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
            Self::Unsupported(_) => "VT-UNSUPPORTED-001",
            Self::Db(_) => "VT-DB-001",
            Self::DbCorrupt(_) => "VT-DB-002",
            Self::DbNewerSchema(_) => "VT-DB-003",
            Self::ProbeToolMissing(_) => "VT-PROBE-001",
            Self::Probe(_) => "VT-PROBE-002",
            Self::Thumb(_) => "VT-THUMB-001",
        }
    }

    /// Pesan singkat berbahasa Indonesia: apa yang terjadi dan apa yang bisa dilakukan.
    pub fn user_message(&self) -> String {
        match self {
            Self::Io { .. } | Self::IoBare(_) => "vallentheme tidak bisa mengakses berkas yang \
                 dibutuhkan. Periksa izin folder dan ruang disk, lalu coba lagi."
                .to_string(),
            Self::ConfigInvalid(_) => "Berkas pengaturan vallentheme tidak bisa dibaca. \
                 Pengaturan akan dikembalikan ke bawaan."
                .to_string(),
            Self::ConfigLoad(_) => "Pengaturan vallentheme gagal dimuat. Coba jalankan ulang \
                 aplikasi; bila tetap gagal, hapus ~/.config/vallentheme/config.toml."
                .to_string(),
            Self::ConfigSave(_) => "Pengaturan vallentheme gagal disimpan. Periksa ruang disk \
                 dan izin folder ~/.config/vallentheme."
                .to_string(),
            Self::NoDataDir => "Folder data pengguna tidak ditemukan. Pastikan variabel HOME \
                 atau XDG_DATA_HOME sudah diatur."
                .to_string(),
            Self::Unsupported(_) => "Format berkas ini belum didukung vallentheme. Gunakan foto \
                 (JPG, PNG, WebP, AVIF) atau video (MP4, MKV, WebM, GIF)."
                .to_string(),
            Self::Db(_) => "Library vallentheme tidak bisa diakses. Coba jalankan ulang \
                 aplikasi. Bila tetap gagal, library bisa dibuat ulang dari Pengaturan."
                .to_string(),
            Self::DbCorrupt(_) => "Berkas library rusak. vallentheme sudah mencadangkan berkas \
                 lama dan membuat library baru; tambahkan ulang folder Anda."
                .to_string(),
            Self::DbNewerSchema(_) => "Library ini dibuat oleh vallentheme versi lebih baru. \
                 Perbarui vallentheme untuk membukanya."
                .to_string(),
            Self::ProbeToolMissing(program) => format!(
                "{program} belum terpasang, jadi vallentheme tidak bisa membaca video. \
                 Pasang dengan: sudo apt install ffmpeg"
            ),
            Self::Probe(_) => "Berkas ini tidak bisa dibaca sebagai foto atau video. \
                 Berkas mungkin rusak atau formatnya belum didukung."
                .to_string(),
            Self::Thumb(_) => "Pratinjau tidak bisa dibuat. Wallpaper tetap bisa dipakai, \
                 hanya gambarnya tidak muncul di daftar."
                .to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn semua_varian() -> Vec<Error> {
        vec![
            Error::IoBare(std::io::Error::other("uji")),
            Error::ConfigInvalid("x".into()),
            Error::ConfigLoad("x".into()),
            Error::ConfigSave("x".into()),
            Error::NoDataDir,
            Error::Unsupported("uji".into()),
            Error::Db("x".into()),
            Error::DbCorrupt("x".into()),
            Error::DbNewerSchema(99),
            Error::Probe("x".into()),
            Error::ProbeToolMissing("ffprobe".into()),
            Error::Thumb("x".into()),
        ]
    }

    #[test]
    fn setiap_varian_punya_kode_dan_pesan() {
        for e in semua_varian() {
            let kode = e.code();
            let bagian: Vec<&str> = kode.split('-').collect();
            assert_eq!(bagian.len(), 3, "format kode harus VT-KATEGORI-NNN: {kode}");
            assert_eq!(bagian[0], "VT");
            assert_eq!(bagian[2].len(), 3, "nomor harus 3 digit: {kode}");
            let pesan = e.user_message();
            assert!(
                pesan.len() > 20,
                "pesan terlalu pendek untuk {kode}: {pesan}"
            );
        }
    }

    #[test]
    fn kode_unik_per_varian() {
        let mut kode: Vec<&'static str> = semua_varian().iter().map(|e| e.code()).collect();
        kode.sort_unstable();
        let jumlah_sebelum = kode.len();
        kode.dedup();
        assert_eq!(kode.len(), jumlah_sebelum, "ada kode duplikat: {kode:?}");
    }

    #[test]
    fn kode_berbeda_untuk_varian_berbeda() {
        assert_eq!(Error::NoDataDir.code(), "VT-CONFIG-004");
        assert_eq!(Error::ConfigInvalid("a".into()).code(), "VT-CONFIG-001");
        assert_eq!(Error::DbNewerSchema(9).code(), "VT-DB-003");
    }
}
