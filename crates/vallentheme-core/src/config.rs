//! Konfigurasi pengguna (`~/.config/vallentheme/config.toml`).
//!
//! Loader wajib toleran: field hilang memakai nilai bawaan, field asing
//! diabaikan dengan peringatan log, nilai di luar rentang dikoreksi, dan
//! penyimpanan dilakukan secara atomik. Lihat `panduan/06-referensi-teknis.md`
//! bagian 10.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Versi skema konfigurasi yang ditulis aplikasi.
pub const CONFIG_VERSION: u32 = 1;

/// Bahasa antarmuka.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Ikuti locale sistem.
    #[default]
    System,
    /// Bahasa Indonesia.
    Id,
    /// Bahasa Inggris.
    En,
}

/// Urutan daftar library yang tersimpan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortSetting {
    /// Urutan tambah manual.
    #[default]
    Mine,
    /// Terbaru ditambahkan.
    Newest,
    /// Terakhir dipakai.
    Recent,
    /// Nama A–Z.
    Name,
    /// Ukuran berkas.
    Size,
    /// Resolusi.
    Resolution,
    /// Favorit lebih dulu.
    Favorite,
}

/// Preferensi pengurangan animasi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReduceMotion {
    /// Ikuti pengaturan sistem.
    #[default]
    System,
    /// Selalu kurangi.
    On,
    /// Jangan kurangi.
    Off,
}

/// Mode kinerja global.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerfMode {
    /// Adaptif otomatis.
    #[default]
    Auto,
    /// Kualitas asli tanpa batas.
    Original,
    /// Seimbang.
    Balanced,
    /// Hemat daya.
    Saver,
    /// Nilai manual.
    Custom,
}

/// Pilihan dekode perangkat keras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HwdecMode {
    /// Pilih otomatis.
    #[default]
    Auto,
    /// VA-API.
    Vaapi,
    /// NVDEC.
    Nvdec,
    /// Perangkat lunak.
    None,
}

/// Prioritas proses mpv.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NiceLevel {
    /// Prioritas normal.
    #[default]
    Normal,
    /// Prioritas rendah.
    Low,
    /// Prioritas sangat rendah.
    Lowest,
}

/// Ukuran dan posisi jendela terakhir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Lebar dalam piksel.
    pub width: i32,
    /// Tinggi dalam piksel.
    pub height: i32,
    /// Posisi X dalam piksel.
    pub x: i32,
    /// Posisi Y dalam piksel.
    pub y: i32,
    /// Jendela dalam keadaan maximize.
    #[serde(default)]
    pub maximized: bool,
}

/// Pengaturan umum aplikasi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Bahasa antarmuka.
    pub language: Language,
    /// Jalankan daemon saat login.
    pub autostart: bool,
    /// Tetap jalankan wallpaper saat GUI ditutup.
    pub run_in_background: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            language: Language::System,
            autostart: true,
            run_in_background: true,
        }
    }
}

/// Pengaturan antarmuka.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Folder terakhir yang dipilih (`all`, `fav`, atau id folder).
    pub last_folder: String,
    /// Urutan daftar.
    pub sort: SortSetting,
    /// Preferensi animasi.
    pub reduce_motion: ReduceMotion,
    /// Geometri jendela terakhir.
    pub window: Option<WindowGeometry>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            last_folder: "all".to_string(),
            sort: SortSetting::Mine,
            reduce_motion: ReduceMotion::System,
            window: None,
        }
    }
}

/// Pengaturan kinerja.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceConfig {
    /// Mode kinerja.
    pub mode: PerfMode,
    /// Dekode perangkat keras.
    pub hwdec: HwdecMode,
    /// Batas fps; `0` berarti tanpa batas.
    pub fps_cap: u32,
    /// Tinggi render maksimum; `0` berarti asli.
    pub max_render_height: u32,
    /// Jeda saat ada jendela fullscreen.
    pub pause_on_fullscreen: bool,
    /// Jeda saat ada jendela maximize.
    pub pause_on_maximized: bool,
    /// Jeda saat memakai baterai.
    pub pause_on_battery: bool,
    /// Ambang baterai (persen).
    pub battery_threshold: u32,
    /// Jeda saat layar terkunci.
    pub pause_on_lock: bool,
    /// Jeda saat mode hemat daya sistem.
    pub pause_on_system_saver: bool,
    /// Prioritas proses mpv.
    pub nice: NiceLevel,
    /// Optimalkan video saat impor.
    pub optimize_on_import: bool,
    /// Tinggi target optimasi.
    pub optimize_target_height: u32,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            mode: PerfMode::Auto,
            hwdec: HwdecMode::Auto,
            fps_cap: 0,
            max_render_height: 0,
            pause_on_fullscreen: true,
            pause_on_maximized: true,
            pause_on_battery: true,
            battery_threshold: 30,
            pause_on_lock: true,
            pause_on_system_saver: true,
            nice: NiceLevel::Normal,
            optimize_on_import: false,
            optimize_target_height: 1080,
        }
    }
}

/// Pengaturan penyimpanan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Folder unduhan; kosong berarti `~/Videos/vallentheme`.
    pub download_dir: String,
    /// Batas ukuran cache dalam MB.
    pub cache_limit_mb: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            download_dir: String::new(),
            cache_limit_mb: 2048,
        }
    }
}

/// Konfigurasi lengkap aplikasi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Versi skema berkas.
    pub version: u32,
    /// Bagian `[general]`.
    pub general: GeneralConfig,
    /// Bagian `[ui]`.
    pub ui: UiConfig,
    /// Bagian `[performance]`.
    pub performance: PerformanceConfig,
    /// Bagian `[storage]`.
    pub storage: StorageConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            general: GeneralConfig::default(),
            ui: UiConfig::default(),
            performance: PerformanceConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

/// Batas nilai yang diterima, dipakai saat mengoreksi masukan pengguna.
const BATTERY_MIN: u32 = 5;
const BATTERY_MAX: u32 = 90;
const CACHE_MIN_MB: u32 = 64;
const CACHE_MAX_MB: u32 = 102_400;
const FPS_ALLOWED: [u32; 4] = [0, 24, 30, 60];
const HEIGHT_ALLOWED: [u32; 4] = [0, 720, 1080, 1440];
const OPTIMIZE_HEIGHT_ALLOWED: [u32; 2] = [720, 1080];

impl Config {
    /// Muat konfigurasi dari `path`.
    ///
    /// - Berkas tidak ada → tulis bawaan lalu kembalikan.
    /// - Berkas rusak → cadangkan ke `config.toml.bak`, pakai bawaan, tulis ulang.
    /// - Nilai di luar rentang → dikoreksi dan dikembalikan sudah bersih.
    pub fn load(path: &Path) -> Result<Self> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                config.save(path)?;
                return Ok(config);
            }
            Err(source) => {
                return Err(Error::Io {
                    path: path.to_path_buf(),
                    source,
                })
            }
        };

        if let Ok(value) = text.parse::<toml::Value>() {
            warn_unknown_keys(&value);
        }

        match toml::from_str::<Self>(&text) {
            Ok(mut config) => {
                config.validate();
                Ok(config)
            }
            Err(err) => {
                let backup = path.with_extension("toml.bak");
                if let Err(copy_err) = fs::copy(path, &backup) {
                    tracing::warn!(
                        error = %copy_err,
                        "konfigurasi rusak dan cadangan gagal dibuat"
                    );
                } else {
                    tracing::warn!(backup = %backup.display(), "cadangan konfigurasi rusak dibuat");
                }
                tracing::warn!(error = %err, "konfigurasi tidak bisa dibaca; memakai bawaan");

                let config = Self::default();
                config.save(path)?;
                Ok(config)
            }
        }
    }

    /// Simpan konfigurasi secara atomik (tulis `*.tmp`, `fsync`, `rename`).
    pub fn save(&self, path: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|e| Error::ConfigSave(e.to_string()))?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let tmp = path.with_extension("toml.tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(|source| Error::Io {
                path: tmp.clone(),
                source,
            })?;
            file.write_all(text.as_bytes())
                .map_err(|source| Error::Io {
                    path: tmp.clone(),
                    source,
                })?;
            file.sync_all().map_err(|source| Error::Io {
                path: tmp.clone(),
                source,
            })?;
        }
        fs::rename(&tmp, path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    /// Koreksi semua nilai agar berada di rentang yang sah.
    pub fn validate(&mut self) {
        if self.version == 0 {
            self.version = CONFIG_VERSION;
        }

        let perf = &mut self.performance;
        perf.battery_threshold = perf.battery_threshold.clamp(BATTERY_MIN, BATTERY_MAX);
        if !FPS_ALLOWED.contains(&perf.fps_cap) {
            perf.fps_cap = 0;
        }
        if !HEIGHT_ALLOWED.contains(&perf.max_render_height) {
            perf.max_render_height = 0;
        }
        if !OPTIMIZE_HEIGHT_ALLOWED.contains(&perf.optimize_target_height) {
            perf.optimize_target_height = 1080;
        }
        self.storage.cache_limit_mb = self
            .storage
            .cache_limit_mb
            .clamp(CACHE_MIN_MB, CACHE_MAX_MB);

        if self.ui.last_folder.trim().is_empty() {
            self.ui.last_folder = "all".to_string();
        }
    }

    /// Folder unduhan efektif; memakai bawaan bila belum diatur.
    pub fn download_dir(&self, fallback: &Path) -> std::path::PathBuf {
        let raw = self.storage.download_dir.trim();
        if raw.is_empty() {
            fallback.to_path_buf()
        } else {
            std::path::PathBuf::from(raw)
        }
    }
}

/// Nama kunci yang dikenal per bagian, untuk mendeteksi field asing.
const KNOWN_KEYS: [(&str, &[&str]); 5] = [
    ("", &["version", "general", "ui", "performance", "storage"]),
    ("general", &["language", "autostart", "run_in_background"]),
    ("ui", &["last_folder", "sort", "reduce_motion", "window"]),
    (
        "performance",
        &[
            "mode",
            "hwdec",
            "fps_cap",
            "max_render_height",
            "pause_on_fullscreen",
            "pause_on_maximized",
            "pause_on_battery",
            "battery_threshold",
            "pause_on_lock",
            "pause_on_system_saver",
            "nice",
            "optimize_on_import",
            "optimize_target_height",
        ],
    ),
    ("storage", &["download_dir", "cache_limit_mb"]),
];

/// Catat peringatan untuk setiap kunci yang tidak dikenal.
fn warn_unknown_keys(value: &toml::Value) {
    let Some(root) = value.as_table() else {
        return;
    };
    warn_unknown_in(root, "");

    let mut sections: BTreeMap<&str, &toml::Value> = BTreeMap::new();
    for (name, entry) in root {
        sections.insert(name.as_str(), entry);
    }
    for (section, entry) in sections {
        if let Some(table) = entry.as_table() {
            warn_unknown_in(table, section);
        }
    }
}

/// Bandingkan kunci tabel dengan daftar yang dikenal.
fn warn_unknown_in(table: &toml::Table, section: &str) {
    let Some((_, known)) = KNOWN_KEYS.iter().find(|(name, _)| *name == section) else {
        return;
    };
    for key in table.keys() {
        if !known.contains(&key.as_str()) {
            let label = if section.is_empty() {
                key.to_string()
            } else {
                format!("{section}.{key}")
            };
            tracing::warn!(key = %label, "field konfigurasi tidak dikenal, diabaikan");
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn berkas_tidak_ada_menghasilkan_bawaan_dan_ditulis() {
        let dir = tmp();
        let path = dir.path().join("config.toml");

        let config = Config::load(&path).unwrap();
        assert_eq!(config, Config::default());
        assert!(path.exists(), "berkas bawaan harus dibuat");
    }

    #[test]
    fn berkas_kosong_memakai_bawaan() {
        let dir = tmp();
        let path = dir.path().join("config.toml");
        fs::write(&path, "").unwrap();

        let config = Config::load(&path).unwrap();
        assert_eq!(config.version, CONFIG_VERSION);
        assert_eq!(config.performance.mode, PerfMode::Auto);
        assert!(config.general.autostart);
    }

    #[test]
    fn berkas_rusak_dicadangkan_dan_diganti_bawaan() {
        let dir = tmp();
        let path = dir.path().join("config.toml");
        fs::write(&path, "ini bukan toml [[[").unwrap();

        let config = Config::load(&path).unwrap();
        assert_eq!(config, Config::default());

        let backup = path.with_extension("toml.bak");
        assert!(backup.exists(), "cadangan harus dibuat");
        assert_eq!(fs::read_to_string(&backup).unwrap(), "ini bukan toml [[[");
    }

    #[test]
    fn field_asing_diabaikan() {
        let dir = tmp();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "version = 1\naneh = 5\n[general]\nlanguage = \"id\"\ntidak_dikenal = true\n",
        )
        .unwrap();

        let config = Config::load(&path).unwrap();
        assert_eq!(config.general.language, Language::Id);
        assert!(config.general.autostart, "field hilang memakai bawaan");
    }

    #[test]
    fn nilai_di_luar_rentang_dikoreksi() {
        let dir = tmp();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[performance]\nbattery_threshold = 500\nfps_cap = 17\nmax_render_height = 123\n\
             optimize_target_height = 999\n[storage]\ncache_limit_mb = 1\n",
        )
        .unwrap();

        let config = Config::load(&path).unwrap();
        assert_eq!(config.performance.battery_threshold, 90);
        assert_eq!(config.performance.fps_cap, 0);
        assert_eq!(config.performance.max_render_height, 0);
        assert_eq!(config.performance.optimize_target_height, 1080);
        assert_eq!(config.storage.cache_limit_mb, 64);
    }

    #[test]
    fn simpan_dan_muat_ulang_bolak_balik() {
        let dir = tmp();
        let path = dir.path().join("sub/config.toml");

        let mut config = Config::default();
        config.general.language = Language::En;
        config.ui.sort = SortSetting::Size;
        config.performance.mode = PerfMode::Saver;
        config.storage.download_dir = "/tmp/unduhan saya".to_string();
        config.ui.window = Some(WindowGeometry {
            width: 960,
            height: 660,
            x: 10,
            y: 20,
            maximized: false,
        });
        config.save(&path).unwrap();

        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn tidak_ada_berkas_tmp_tersisa() {
        let dir = tmp();
        let path = dir.path().join("config.toml");
        Config::default().save(&path).unwrap();

        let sisa: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains("tmp"))
            .collect();
        assert!(sisa.is_empty(), "berkas sementara tertinggal: {sisa:?}");
    }

    #[test]
    fn folder_unduhan_bawaan_dipakai_saat_kosong() {
        let config = Config::default();
        let fallback = Path::new("/tmp/vallentheme-unduhan");
        assert_eq!(config.download_dir(fallback), fallback);
    }
}
