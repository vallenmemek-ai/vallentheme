//! Setter wallpaper foto untuk desktop MATE, GNOME, Cinnamon, dan XFCE.
//!
//! Semua perintah dijalankan lewat `Command::new(...).args([...])` tanpa shell.
//! Nilai awal dibaca lebih dulu dan disimpan sebagai [`RestoreRecord`] agar
//! `Stop` bisa memulihkan pengaturan desktop pengguna.
//!
//! Rujukan: `panduan/06-referensi-teknis.md` bagian 4.

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use vallentheme_core::model::FillMode;

use crate::error::{RenderError, Result};

/// Desktop yang didukung setter foto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desktop {
    /// MATE (`org.mate.background`).
    Mate,
    /// GNOME (`org.gnome.desktop.background`).
    Gnome,
    /// Cinnamon (skema GNOME).
    Cinnamon,
    /// XFCE (`xfconf-query`).
    Xfce,
}

impl Desktop {
    /// Nama untuk log dan pesan pengguna.
    pub fn nama(self) -> &'static str {
        match self {
            Self::Mate => "MATE",
            Self::Gnome => "GNOME",
            Self::Cinnamon => "Cinnamon",
            Self::Xfce => "XFCE",
        }
    }
}

/// Satu nilai awal yang harus dikembalikan saat wallpaper dihentikan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestoreRecord {
    /// Program yang dipakai (`gsettings` atau `xfconf-query`).
    pub program: String,
    /// Argumen lengkap untuk mengembalikan nilai awal.
    pub args: Vec<String>,
}

/// Pemasang wallpaper foto yang mencatat nilai awal.
pub struct Setter {
    desktop: Desktop,
    records: Vec<RestoreRecord>,
}

impl Setter {
    /// Buat setter untuk desktop tertentu.
    pub fn new(desktop: Desktop) -> Self {
        Self {
            desktop,
            records: Vec::new(),
        }
    }

    /// Deteksi desktop dari `XDG_CURRENT_DESKTOP`, lalu `DESKTOP_SESSION`.
    pub fn detect() -> Option<Desktop> {
        let kandidat = std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .into_iter()
            .chain(std::env::var("DESKTOP_SESSION").ok());
        for nilai in kandidat {
            for bagian in nilai.split(':') {
                if let Some(desktop) = dari_nama(bagian) {
                    return Some(desktop);
                }
            }
        }
        None
    }

    /// Desktop yang dipakai setter ini.
    pub fn desktop(&self) -> Desktop {
        self.desktop
    }

    /// Nilai awal yang sudah dicatat.
    pub fn records(&self) -> &[RestoreRecord] {
        &self.records
    }

    /// Terapkan foto sebagai wallpaper.
    pub fn apply(&mut self, path: &Path, fill: FillMode, monitor: Option<&str>) -> Result<()> {
        if !path.is_file() {
            return Err(RenderError::Setter {
                desktop: self.desktop.nama().to_string(),
                pesan: format!("berkas {} tidak ditemukan", path.display()),
            });
        }
        match self.desktop {
            Desktop::Mate => self.apply_mate(path, fill),
            Desktop::Gnome | Desktop::Cinnamon => self.apply_gnome(path, fill),
            Desktop::Xfce => self.apply_xfce(path, monitor),
        }
    }

    /// Kembalikan semua nilai awal, urutan terbalik dari saat dicatat.
    pub fn restore(&mut self) -> Result<()> {
        let mut kegagalan: Option<RenderError> = None;
        for record in self.records.drain(..).rev() {
            match jalankan(&record.program, &record.args) {
                Ok(()) => {}
                Err(err) => {
                    tracing::warn!(
                        program = %record.program,
                        error = %err,
                        "gagal memulihkan pengaturan desktop"
                    );
                    if kegagalan.is_none() {
                        kegagalan = Some(err);
                    }
                }
            }
        }
        match kegagalan {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn apply_mate(&mut self, path: &Path, fill: FillMode) -> Result<()> {
        let schema = "org.mate.background";
        self.catat_gsettings(schema, "picture-filename")?;
        let uri = file_uri(path);
        self.set_gsettings(
            schema,
            "picture-filename",
            &format!("'{}'", escape_gvariant(&uri)),
        )?;

        self.catat_gsettings(schema, "picture-options")?;
        let opsi = picture_option(fill);
        self.set_gsettings(schema, "picture-options", &format!("'{opsi}'"))
    }

    fn apply_gnome(&mut self, path: &Path, fill: FillMode) -> Result<()> {
        let schema = "org.gnome.desktop.background";
        let uri = file_uri(path);
        let nilai = format!("'{}'", escape_gvariant(&uri));

        self.catat_gsettings(schema, "picture-uri")?;
        self.set_gsettings(schema, "picture-uri", &nilai)?;

        // Kunci mode gelap tidak selalu ada pada versi lama; kegagalan diabaikan.
        if self.catat_gsettings(schema, "picture-uri-dark").is_ok() {
            self.set_gsettings(schema, "picture-uri-dark", &nilai)?;
        }

        self.catat_gsettings(schema, "picture-options")?;
        let opsi = picture_option(fill);
        self.set_gsettings(schema, "picture-options", &format!("'{opsi}'"))
    }

    fn apply_xfce(&mut self, path: &Path, monitor: Option<&str>) -> Result<()> {
        let daftar = jalankan_output(
            "xfconf-query",
            &[
                "-c".to_string(),
                "xfce4-desktop".to_string(),
                "-l".to_string(),
            ],
        )?;

        let filter = monitor.map(|m| m.to_ascii_lowercase());
        let mut dipasang = 0usize;
        for properti in daftar.lines().map(str::trim).filter(|p| !p.is_empty()) {
            if !properti.ends_with("/last-image") {
                continue;
            }
            if let Some(filter) = filter.as_deref() {
                if !properti.to_ascii_lowercase().contains(filter) {
                    continue;
                }
            }

            let args_get = vec![
                "-c".to_string(),
                "xfce4-desktop".to_string(),
                "-p".to_string(),
                properti.to_string(),
            ];
            let nilai_awal = jalankan_output("xfconf-query", &args_get)?;
            self.records.push(RestoreRecord {
                program: "xfconf-query".to_string(),
                args: vec![
                    "-c".to_string(),
                    "xfce4-desktop".to_string(),
                    "-p".to_string(),
                    properti.to_string(),
                    "-s".to_string(),
                    nilai_awal.trim().to_string(),
                ],
            });

            let args_set = vec![
                "-c".to_string(),
                "xfce4-desktop".to_string(),
                "-p".to_string(),
                properti.to_string(),
                "-s".to_string(),
                path.display().to_string(),
            ];
            jalankan("xfconf-query", &args_set)?;
            dipasang += 1;
        }

        if dipasang == 0 {
            return Err(RenderError::Setter {
                desktop: self.desktop.nama().to_string(),
                pesan: "tidak ada properti latar XFCE yang bisa diubah".to_string(),
            });
        }
        Ok(())
    }

    fn catat_gsettings(&mut self, schema: &str, kunci: &str) -> Result<()> {
        let nilai = jalankan_output(
            "gsettings",
            &["get".to_string(), schema.to_string(), kunci.to_string()],
        )?;
        self.records.push(RestoreRecord {
            program: "gsettings".to_string(),
            args: vec![
                "set".to_string(),
                schema.to_string(),
                kunci.to_string(),
                nilai.trim().to_string(),
            ],
        });
        Ok(())
    }

    fn set_gsettings(&self, schema: &str, kunci: &str, nilai: &str) -> Result<()> {
        jalankan(
            "gsettings",
            &[
                "set".to_string(),
                schema.to_string(),
                kunci.to_string(),
                nilai.to_string(),
            ],
        )
    }
}

/// Petakan [`FillMode`] ke `picture-options` desktop.
pub fn picture_option(fill: FillMode) -> &'static str {
    match fill {
        FillMode::Cover => "zoom",
        FillMode::Fit => "scaled",
        FillMode::Center => "centered",
        FillMode::Stretch => "stretched",
        FillMode::Tile => "wallpaper",
    }
}

/// Ubah nama desktop menjadi enum; pencocokan tidak peka huruf besar.
pub fn dari_nama(nama: &str) -> Option<Desktop> {
    let n = nama.trim().to_ascii_lowercase();
    if n.contains("cinnamon") {
        Some(Desktop::Cinnamon)
    } else if n.contains("mate") {
        Some(Desktop::Mate)
    } else if n.contains("gnome") {
        Some(Desktop::Gnome)
    } else if n.contains("xfce") {
        Some(Desktop::Xfce)
    } else {
        None
    }
}

/// Bentuk URI `file://` sederhana dengan persen-encoding untuk byte khusus.
pub fn file_uri(path: &Path) -> String {
    let teks = path.to_string_lossy();
    let mut hasil = String::with_capacity(teks.len() + 7);
    hasil.push_str("file://");
    for byte in teks.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                hasil.push(char::from(*byte));
            }
            lain => hasil.push_str(&format!("%{lain:02X}")),
        }
    }
    hasil
}

/// Escape teks agar aman sebagai string GVariant berkutip tunggal.
fn escape_gvariant(nilai: &str) -> String {
    nilai.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Jalankan perintah dan pastikan kode keluarnya nol.
fn jalankan(program: &str, args: &[String]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .map_err(|err| RenderError::Setter {
            desktop: program.to_string(),
            pesan: err.to_string(),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(RenderError::Setter {
            desktop: program.to_string(),
            pesan: format!("perintah keluar dengan status {status}"),
        })
    }
}

/// Jalankan perintah dan kembalikan stdout.
fn jalankan_output(program: &str, args: &[String]) -> Result<String> {
    let keluaran =
        Command::new(program)
            .args(args)
            .output()
            .map_err(|err| RenderError::Setter {
                desktop: program.to_string(),
                pesan: err.to_string(),
            })?;
    if !keluaran.status.success() {
        return Err(RenderError::Setter {
            desktop: program.to_string(),
            pesan: String::from_utf8_lossy(&keluaran.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&keluaran.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn pemetaan_picture_options_lengkap() {
        assert_eq!(picture_option(FillMode::Cover), "zoom");
        assert_eq!(picture_option(FillMode::Fit), "scaled");
        assert_eq!(picture_option(FillMode::Center), "centered");
        assert_eq!(picture_option(FillMode::Stretch), "stretched");
        assert_eq!(picture_option(FillMode::Tile), "wallpaper");
    }

    #[test]
    fn deteksi_nama_desktop() {
        assert_eq!(dari_nama("MATE"), Some(Desktop::Mate));
        assert_eq!(dari_nama("ubuntu:GNOME"), Some(Desktop::Gnome));
        assert_eq!(dari_nama("X-Cinnamon"), Some(Desktop::Cinnamon));
        assert_eq!(dari_nama("XFCE"), Some(Desktop::Xfce));
        assert_eq!(dari_nama("KDE"), None);
    }

    #[test]
    fn uri_mengencode_spasi_dan_unicode() {
        let uri = file_uri(Path::new("/home/pengguna/Gambar saya/壁紙.jpg"));
        assert_eq!(
            uri,
            "file:///home/pengguna/Gambar%20saya/%E5%A3%81%E7%B4%99.jpg"
        );
    }

    #[test]
    fn escape_kutip_gvariant() {
        assert_eq!(escape_gvariant("a'b\\c"), "a\\'b\\\\c");
    }
}
