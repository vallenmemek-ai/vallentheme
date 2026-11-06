//! Model data vallentheme.
//!
//! Tipe kuat untuk ID, jenis media, dan pengaturan per wallpaper. Rujukan:
//! `fase/FASE-2-core.md` bagian 2.1 dan `panduan/06-referensi-teknis.md`
//! bagian 7.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// ID baris `wallpapers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WallpaperId(pub i64);

/// ID baris `folders`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FolderId(pub i64);

impl std::fmt::Display for WallpaperId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for FolderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Jenis media yang didukung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    /// Foto diam.
    Image,
    /// Video atau GIF animasi.
    Video,
}

impl MediaKind {
    /// Nilai yang disimpan di kolom `kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }

    /// Kebalikan dari [`MediaKind::as_str`].
    pub fn parse(nilai: &str) -> Result<Self> {
        match nilai {
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            lain => Err(Error::Unsupported(format!(
                "jenis media tidak dikenal: {lain}"
            ))),
        }
    }

    /// Nama untuk ditampilkan.
    pub fn label(self) -> &'static str {
        match self {
            Self::Image => "Foto",
            Self::Video => "Video",
        }
    }
}

/// Ekstensi berkas yang diterima, dipetakan ke jenis media.
pub const SUPPORTED_EXTENSIONS: &[(&str, MediaKind)] = &[
    ("mp4", MediaKind::Video),
    ("mkv", MediaKind::Video),
    ("webm", MediaKind::Video),
    ("mov", MediaKind::Video),
    ("avi", MediaKind::Video),
    ("m4v", MediaKind::Video),
    ("gif", MediaKind::Video),
    ("jpg", MediaKind::Image),
    ("jpeg", MediaKind::Image),
    ("png", MediaKind::Image),
    ("webp", MediaKind::Image),
    ("bmp", MediaKind::Image),
    ("tiff", MediaKind::Image),
    ("avif", MediaKind::Image),
];

/// Tebak jenis media dari ekstensi path.
pub fn kind_from_extension(path: &std::path::Path) -> Option<MediaKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    SUPPORTED_EXTENSIONS
        .iter()
        .find(|(nama, _)| *nama == ext)
        .map(|(_, kind)| *kind)
}

/// Cara media mengisi layar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FillMode {
    /// Isi penuh, boleh terpangkas.
    #[default]
    Cover,
    /// Seluruh gambar terlihat, boleh ada bilah hitam.
    Fit,
    /// Ukuran asli di tengah.
    Center,
    /// Tarik memenuhi layar, rasio berubah.
    Stretch,
    /// Diulang (hanya foto).
    Tile,
}

impl FillMode {
    /// Nilai yang disimpan di JSON pengaturan.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Fit => "fit",
            Self::Center => "center",
            Self::Stretch => "stretch",
            Self::Tile => "tile",
        }
    }
}

/// Efek perpindahan wallpaper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transition {
    /// Ganti langsung.
    #[default]
    None,
    /// Pudar (butuh compositor).
    Fade,
}

/// Kebijakan hemat daya per wallpaper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerPolicy {
    /// Ikuti pengaturan global.
    #[default]
    Default,
    /// Selalu jalan.
    Always,
    /// Jeda saat fullscreen/maximize.
    PauseFullscreen,
    /// Jeda saat memakai baterai.
    PauseBattery,
    /// Selalu hemat.
    AlwaysSaver,
}

/// Pengaturan per wallpaper, disimpan sebagai JSON di `settings_json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WallpaperSettings {
    /// Cara media mengisi layar.
    pub fill: FillMode,
    /// Rotasi dalam derajat: 0, 90, 180, atau 270.
    pub rotation: u16,
    /// Zoom dalam skala log2, 0.0–2.0.
    pub zoom: f32,
    /// Geser horizontal, −0.5–0.5.
    pub pan_x: f32,
    /// Geser vertikal, −0.5–0.5.
    pub pan_y: f32,
    /// Efek perpindahan.
    pub transition: Transition,
    /// Bisukan suara.
    pub mute: bool,
    /// Volume 0–100.
    pub volume: u8,
    /// Kebijakan hemat daya.
    pub power: PowerPolicy,
    /// Monitor tujuan; `None` berarti semua monitor.
    pub monitor: Option<String>,
}

impl Default for WallpaperSettings {
    fn default() -> Self {
        Self {
            fill: FillMode::Cover,
            rotation: 0,
            zoom: 0.0,
            pan_x: 0.0,
            pan_y: 0.0,
            transition: Transition::None,
            // Bisu secara bawaan agar wallpaper tidak mengejutkan pengguna.
            mute: true,
            volume: 50,
            power: PowerPolicy::Default,
            monitor: None,
        }
    }
}

/// Rentang nilai yang sah untuk pengaturan.
pub const ZOOM_MAX: f32 = 2.0;
/// Batas geser horizontal/vertikal.
pub const PAN_MAX: f32 = 0.5;

impl WallpaperSettings {
    /// Koreksi semua nilai agar berada di rentang sah.
    pub fn validate(&mut self) {
        self.rotation = match self.rotation {
            0 | 90 | 180 | 270 => self.rotation,
            _ => 0,
        };
        if !self.zoom.is_finite() {
            self.zoom = 0.0;
        }
        self.zoom = self.zoom.clamp(0.0, ZOOM_MAX);

        if !self.pan_x.is_finite() {
            self.pan_x = 0.0;
        }
        if !self.pan_y.is_finite() {
            self.pan_y = 0.0;
        }
        let batas = self.pan_limit();
        self.pan_x = self.pan_x.clamp(-batas, batas);
        self.pan_y = self.pan_y.clamp(-batas, batas);

        self.volume = self.volume.min(100);

        if self.fill != FillMode::Cover {
            // Pan/zoom hanya bermakna pada mode Penuhi; nilai tetap disimpan.
        }
    }

    /// Batas geser yang membuat tepi gambar tidak menampakkan area kosong.
    ///
    /// Pada zoom 0 gambar sudah menutupi layar, jadi tidak ada ruang geser.
    pub fn pan_limit(&self) -> f32 {
        // Skala = 2^zoom; ruang geser sebanding dengan (skala - 1) / (2 * skala).
        let skala = 2f32.powf(self.zoom);
        let ruang = (skala - 1.0) / (2.0 * skala);
        ruang.clamp(0.0, PAN_MAX)
    }

    /// Baca pengaturan dari JSON; toleran terhadap field hilang atau rusak.
    pub fn from_json(teks: &str) -> Self {
        if teks.trim().is_empty() {
            return Self::default();
        }
        match serde_json::from_str::<Self>(teks) {
            Ok(mut nilai) => {
                nilai.validate();
                nilai
            }
            Err(err) => {
                tracing::warn!(error = %err, "settings_json tidak bisa dibaca; memakai bawaan");
                Self::default()
            }
        }
    }

    /// Tulis pengaturan ke JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Satu baris tabel `wallpapers`.
#[derive(Debug, Clone, PartialEq)]
pub struct Wallpaper {
    /// ID baris.
    pub id: WallpaperId,
    /// Path absolut berkas asli.
    pub path: std::path::PathBuf,
    /// Jenis media.
    pub kind: MediaKind,
    /// Judul (nama berkas tanpa ekstensi).
    pub title: String,
    /// Lebar piksel.
    pub width: Option<u32>,
    /// Tinggi piksel.
    pub height: Option<u32>,
    /// Frame per detik.
    pub fps: Option<f64>,
    /// Durasi milidetik.
    pub duration_ms: Option<i64>,
    /// Nama codec.
    pub codec: Option<String>,
    /// Punya trek audio.
    pub has_audio: bool,
    /// Ukuran berkas.
    pub size_bytes: i64,
    /// Waktu modifikasi berkas (detik Unix).
    pub mtime: i64,
    /// Folder asal.
    pub folder_id: Option<FolderId>,
    /// Path thumbnail di cache.
    pub thumb_path: Option<std::path::PathBuf>,
    /// Ditandai favorit.
    pub favorite: bool,
    /// Berkas tidak ditemukan.
    pub missing: bool,
    /// Urutan manual.
    pub sort_index: i64,
    /// Waktu ditambahkan (detik Unix).
    pub added_at: i64,
    /// Waktu terakhir dipakai.
    pub last_used_at: Option<i64>,
    /// Pengaturan per wallpaper.
    pub settings: WallpaperSettings,
}

/// Satu baris tabel `folders`.
#[derive(Debug, Clone, PartialEq)]
pub struct Folder {
    /// ID baris.
    pub id: FolderId,
    /// Path folder.
    pub path: std::path::PathBuf,
    /// Nama tampilan.
    pub name: String,
    /// Sertakan subfolder saat memindai.
    pub recursive: bool,
    /// Waktu ditambahkan.
    pub added_at: i64,
    /// Jumlah wallpaper di folder ini (hasil hitung, bukan kolom).
    pub item_count: u32,
}

/// Urutan daftar library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    /// Urutan tambah manual.
    #[default]
    Mine,
    /// Terbaru ditambahkan.
    Newest,
    /// Terakhir dipakai.
    Recent,
    /// Nama A–Z.
    Name,
    /// Ukuran berkas menurun.
    Size,
    /// Resolusi menurun.
    Resolution,
    /// Favorit lebih dulu.
    Favorite,
}

impl SortKey {
    /// Ubah ke `ORDER BY` SQL yang aman (tanpa masukan pengguna).
    pub fn order_by(self) -> &'static str {
        match self {
            Self::Mine => "sort_index ASC, id ASC",
            Self::Newest => "added_at DESC, id DESC",
            Self::Recent => "last_used_at IS NULL, last_used_at DESC, id DESC",
            Self::Name => "title COLLATE NOCASE ASC, id ASC",
            Self::Size => "size_bytes DESC, id ASC",
            Self::Resolution => "(COALESCE(width,0) * COALESCE(height,0)) DESC, id ASC",
            Self::Favorite => "favorite DESC, sort_index ASC, id ASC",
        }
    }
}

/// Kriteria penyaringan daftar library.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filter {
    /// Teks pencarian bebas.
    pub query: String,
    /// Batasi ke satu folder.
    pub folder: Option<FolderId>,
    /// Hanya favorit.
    pub only_favorites: bool,
    /// Batasi ke satu jenis media.
    pub kind: Option<MediaKind>,
}

impl Filter {
    /// Tidak ada penyaringan aktif.
    pub fn is_empty(&self) -> bool {
        self.query.trim().is_empty()
            && self.folder.is_none()
            && !self.only_favorites
            && self.kind.is_none()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn jenis_media_bolak_balik() {
        assert_eq!(MediaKind::parse("video").unwrap(), MediaKind::Video);
        assert_eq!(MediaKind::parse("image").unwrap(), MediaKind::Image);
        assert!(MediaKind::parse("audio").is_err());
        assert_eq!(MediaKind::Video.as_str(), "video");
    }

    #[test]
    fn ekstensi_dikenali() {
        assert_eq!(
            kind_from_extension(std::path::Path::new("/a/B.MP4")),
            Some(MediaKind::Video)
        );
        assert_eq!(
            kind_from_extension(std::path::Path::new("/a/b.jpeg")),
            Some(MediaKind::Image)
        );
        assert_eq!(kind_from_extension(std::path::Path::new("/a/b.txt")), None);
        assert_eq!(
            kind_from_extension(std::path::Path::new("/a/tanpa-ekstensi")),
            None
        );
    }

    #[test]
    fn bawaan_mute_dan_penuhi() {
        let s = WallpaperSettings::default();
        assert_eq!(s.fill, FillMode::Cover);
        assert!(s.mute, "bawaan harus bisu");
        assert_eq!(s.volume, 50);
        assert_eq!(s.rotation, 0);
    }

    #[test]
    fn validasi_mengoreksi_nilai_liar() {
        let mut s = WallpaperSettings {
            rotation: 45,
            zoom: 9.0,
            pan_x: 3.0,
            pan_y: -3.0,
            volume: 200,
            ..WallpaperSettings::default()
        };
        s.validate();
        assert_eq!(s.rotation, 0);
        assert_eq!(s.zoom, ZOOM_MAX);
        assert!(s.pan_x <= PAN_MAX && s.pan_x >= -PAN_MAX);
        assert_eq!(s.volume, 100);
    }

    #[test]
    fn rotasi_sah_dipertahankan() {
        for derajat in [0u16, 90, 180, 270] {
            let mut s = WallpaperSettings {
                rotation: derajat,
                ..WallpaperSettings::default()
            };
            s.validate();
            assert_eq!(s.rotation, derajat);
        }
    }

    #[test]
    fn batas_geser_nol_saat_tanpa_zoom() {
        let s = WallpaperSettings::default();
        assert_eq!(s.pan_limit(), 0.0);
    }

    #[test]
    fn batas_geser_membesar_dengan_zoom() {
        let sedang = WallpaperSettings {
            zoom: 1.0,
            ..WallpaperSettings::default()
        }
        .pan_limit();
        let besar = WallpaperSettings {
            zoom: 2.0,
            ..WallpaperSettings::default()
        }
        .pan_limit();
        assert!(sedang > 0.0);
        assert!(besar > sedang);
        assert!(besar <= PAN_MAX);
    }

    #[test]
    fn json_toleran_field_hilang() {
        let s = WallpaperSettings::from_json("{\"fill\":\"fit\"}");
        assert_eq!(s.fill, FillMode::Fit);
        assert!(s.mute, "field hilang memakai bawaan");
    }

    #[test]
    fn json_rusak_memakai_bawaan() {
        let s = WallpaperSettings::from_json("{bukan json");
        assert_eq!(s, WallpaperSettings::default());
    }

    #[test]
    fn json_kosong_memakai_bawaan() {
        assert_eq!(
            WallpaperSettings::from_json(""),
            WallpaperSettings::default()
        );
        assert_eq!(
            WallpaperSettings::from_json("   "),
            WallpaperSettings::default()
        );
    }

    #[test]
    fn json_bolak_balik() {
        let s = WallpaperSettings {
            fill: FillMode::Stretch,
            rotation: 180,
            zoom: 0.5,
            pan_x: 0.1,
            pan_y: -0.1,
            transition: Transition::Fade,
            mute: false,
            volume: 75,
            power: PowerPolicy::PauseBattery,
            monitor: Some("HDMI-1".to_string()),
        };
        assert_eq!(WallpaperSettings::from_json(&s.to_json()), s);
    }

    #[test]
    fn filter_kosong_dikenali() {
        assert!(Filter::default().is_empty());
        let f = Filter {
            query: "  ".to_string(),
            ..Filter::default()
        };
        assert!(f.is_empty());
        let f = Filter {
            query: "gunung".to_string(),
            ..Filter::default()
        };
        assert!(!f.is_empty());
    }

    #[test]
    fn order_by_tidak_memuat_masukan_pengguna() {
        for key in [
            SortKey::Mine,
            SortKey::Newest,
            SortKey::Recent,
            SortKey::Name,
            SortKey::Size,
            SortKey::Resolution,
            SortKey::Favorite,
        ] {
            let sql = key.order_by();
            assert!(!sql.contains(';'), "ORDER BY tidak boleh berisi titik koma");
            assert!(!sql.contains('\''), "ORDER BY tidak boleh berisi kutip");
        }
    }
}
