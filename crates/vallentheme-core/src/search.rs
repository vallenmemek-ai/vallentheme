//! Pencarian teks pada daftar library.
//!
//! Token dipisah spasi. Setiap token harus cocok (AND). Beberapa token punya
//! arti khusus: `video`, `foto`, `fav`, `4k`, `2k`, `fhd`.
//! Rujukan: `fase/FASE-2-core.md` bagian 2.6.

use crate::model::{MediaKind, Wallpaper};

/// Kueri yang sudah diurai.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    /// Token teks biasa yang harus cocok.
    pub tokens: Vec<String>,
    /// Batasi ke jenis media tertentu.
    pub kinds: Vec<MediaKind>,
    /// Hanya favorit.
    pub only_favorites: bool,
    /// Lebar minimum (dari token `4k`, `2k`, `fhd`).
    pub min_width: Option<u32>,
    /// Kueri kosong berarti tidak menyaring.
    pub kosong: bool,
}

/// Buang diakritik sederhana pada huruf Latin.
fn normalisasi(teks: &str) -> String {
    teks.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            lain => lain,
        })
        .collect()
}

/// Urai teks pencarian menjadi [`Query`].
pub fn parse(teks: &str) -> Query {
    let bersih = normalisasi(teks.trim());
    if bersih.is_empty() {
        return Query {
            kosong: true,
            ..Query::default()
        };
    }

    let mut query = Query::default();
    for token in bersih.split_whitespace() {
        match token {
            "video" => query.kinds.push(MediaKind::Video),
            "foto" | "image" | "gambar" => query.kinds.push(MediaKind::Image),
            "fav" | "favorit" => query.only_favorites = true,
            "4k" | "2160p" => query.min_width = Some(3840),
            "2k" | "1440p" => query.min_width = Some(2560),
            "fhd" | "1080p" => query.min_width = Some(1920),
            lain => query.tokens.push(lain.to_string()),
        }
    }
    query
}

/// Apakah satu wallpaper cocok dengan kueri.
pub fn matches(w: &Wallpaper, query: &Query) -> bool {
    if query.kosong {
        return true;
    }
    if !query.kinds.is_empty() && !query.kinds.contains(&w.kind) {
        return false;
    }
    if query.only_favorites && !w.favorite {
        return false;
    }
    if let Some(min) = query.min_width {
        if w.width.unwrap_or(0) < min {
            return false;
        }
    }

    let judul = normalisasi(&w.title);
    let path = normalisasi(&w.path.to_string_lossy());
    let codec = w.codec.as_deref().map(normalisasi).unwrap_or_default();

    query.tokens.iter().all(|t| {
        judul.contains(t.as_str()) || path.contains(t.as_str()) || codec.contains(t.as_str())
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::model::{WallpaperId, WallpaperSettings};
    use std::path::PathBuf;

    fn contoh(judul: &str, kind: MediaKind, lebar: u32, fav: bool) -> Wallpaper {
        Wallpaper {
            id: WallpaperId(1),
            path: PathBuf::from(format!("/video/{judul}.mp4")),
            kind,
            title: judul.to_string(),
            width: Some(lebar),
            height: Some(lebar * 9 / 16),
            fps: Some(24.0),
            duration_ms: Some(1000),
            codec: Some("h264".to_string()),
            has_audio: true,
            size_bytes: 100,
            mtime: 0,
            folder_id: None,
            thumb_path: None,
            favorite: fav,
            missing: false,
            sort_index: 1,
            added_at: 0,
            last_used_at: None,
            settings: WallpaperSettings::default(),
        }
    }

    #[test]
    fn kueri_kosong_cocok_semua() {
        let q = parse("   ");
        assert!(q.kosong);
        assert!(matches(&contoh("apa", MediaKind::Video, 1920, false), &q));
    }

    #[test]
    fn token_biasa_mencocokkan_judul() {
        let q = parse("gunung");
        assert!(matches(
            &contoh("Gunung Fuji", MediaKind::Image, 1920, false),
            &q
        ));
        assert!(!matches(
            &contoh("Pantai", MediaKind::Image, 1920, false),
            &q
        ));
    }

    #[test]
    fn beberapa_token_harus_semua_cocok() {
        let q = parse("gunung fuji");
        assert!(matches(
            &contoh("Gunung Fuji", MediaKind::Image, 1920, false),
            &q
        ));
        assert!(!matches(
            &contoh("Gunung Rinjani", MediaKind::Image, 1920, false),
            &q
        ));
    }

    #[test]
    fn token_khusus_video_dan_foto() {
        let video = parse("video");
        assert!(matches(&contoh("a", MediaKind::Video, 1920, false), &video));
        assert!(!matches(
            &contoh("a", MediaKind::Image, 1920, false),
            &video
        ));

        let foto = parse("foto");
        assert!(matches(&contoh("a", MediaKind::Image, 1920, false), &foto));
    }

    #[test]
    fn token_favorit() {
        let q = parse("fav");
        assert!(matches(&contoh("a", MediaKind::Video, 1920, true), &q));
        assert!(!matches(&contoh("a", MediaKind::Video, 1920, false), &q));
    }

    #[test]
    fn token_resolusi() {
        let q4k = parse("4k");
        assert!(matches(&contoh("a", MediaKind::Video, 3840, false), &q4k));
        assert!(!matches(&contoh("a", MediaKind::Video, 1920, false), &q4k));

        let qfhd = parse("fhd");
        assert!(matches(&contoh("a", MediaKind::Video, 1920, false), &qfhd));
        assert!(matches(&contoh("a", MediaKind::Video, 3840, false), &qfhd));
    }

    #[test]
    fn huruf_besar_kecil_diabaikan() {
        let q = parse("GUNUNG");
        assert!(matches(
            &contoh("gunung", MediaKind::Image, 1920, false),
            &q
        ));
    }

    #[test]
    fn diakritik_dinormalisasi() {
        let q = parse("cafe");
        assert!(matches(&contoh("Café", MediaKind::Image, 1920, false), &q));
    }

    #[test]
    fn cocok_lewat_path() {
        let q = parse("video");
        let w = contoh("x", MediaKind::Video, 1920, false);
        assert!(matches(&w, &q));
    }
}
