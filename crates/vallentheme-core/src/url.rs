//! Validasi tautan untuk fitur "Dari tautan".
//!
//! Aturan (`panduan/00` bagian 3, `fase/FASE-8` bagian 8.1): hanya `http` dan
//! `https`, panjang maksimal 2048 karakter, tidak diawali `-`, dan tidak
//! mengandung karakter yang berbahaya bila string bocor ke shell. Perintah
//! `yt-dlp` tetap dijalankan **tanpa shell** dan dengan `--` sebelum URL, jadi
//! validasi ini adalah lapisan kedua, bukan satu-satunya.

use url::Url;

use crate::error::{Error, Result};

/// Panjang maksimum tautan yang diterima (karakter).
pub const MAX_PANJANG: usize = 2048;

/// Karakter yang ditolak karena berbahaya di shell atau tidak pernah sah di URL.
///
/// `&`, `?`, `=`, `%`, `+`, `#`, `:`, `/`, `@`, `(`, `)`, `[`, `]`, `*`, `~`,
/// `,`, `!` **diizinkan** karena sering muncul di tautan video yang sah.
const TERLARANG: [char; 16] = [
    ';', '|', '`', '$', '<', '>', '"', '\'', '\\', '{', '}', '^', ' ', '\t', '\n', '\r',
];

/// Periksa sebuah tautan dan kembalikan [`Url`] yang sudah diurai.
///
/// Urutan pemeriksaan sengaja murah ke mahal: panjang, awalan `-`, karakter
/// berbahaya, baru kemudian penguraian dengan crate `url`.
pub fn validate(url_str: &str) -> Result<Url> {
    if url_str.is_empty() {
        return Err(Error::UrlInvalid("tautan masih kosong".to_string()));
    }
    if url_str.chars().count() > MAX_PANJANG {
        return Err(Error::UrlInvalid(format!(
            "tautan lebih dari {MAX_PANJANG} karakter"
        )));
    }
    if url_str.starts_with('-') {
        return Err(Error::UrlInvalid(
            "tautan tidak boleh diawali tanda minus".to_string(),
        ));
    }
    if let Some(jelek) = url_str
        .chars()
        .find(|c| c.is_control() || TERLARANG.contains(c))
    {
        return Err(Error::UrlInvalid(format!(
            "tautan mengandung karakter terlarang: {jelek:?}"
        )));
    }

    let url = Url::parse(url_str)
        .map_err(|e| Error::UrlInvalid(format!("tautan tidak bisa dibaca: {e}")))?;

    match url.scheme() {
        "http" | "https" => {}
        lain => {
            return Err(Error::UrlInvalid(format!(
                "hanya tautan http/https yang didukung, bukan {lain}"
            )))
        }
    }
    if url.host_str().map_or(true, str::is_empty) {
        return Err(Error::UrlInvalid(
            "tautan tidak memuat alamat situs".to_string(),
        ));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn tautan_sah_diterima() {
        let sah = [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://www.youtube.com/watch?v=abc&t=10s&list=xyz",
            "http://example.com/video.mp4",
            "https://example.com/a/b/c%20d.mp4?x=1&y=2#bagian",
            "https://youtu.be/abc123",
            "https://example.com:8443/v/1",
        ];
        for s in sah {
            let url = validate(s).unwrap_or_else(|e| panic!("{s} harus sah: {e}"));
            assert!(url.scheme() == "http" || url.scheme() == "https");
        }
    }

    #[test]
    fn kumpulan_kasus_berbahaya_ditolak() {
        let jahat = [
            "https://example.com/; rm -rf /",
            "https://example.com/$(reboot)",
            "https://example.com/`id`",
            "https://example.com/a b.mp4",
            "https://example.com/a\tb",
            "https://example.com/a\nb",
            "-https://example.com/v.mp4",
            "--output=/etc/passwd",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "ftp://example.com/v.mp4",
            "https://example.com/|cat",
            "https://example.com/a>b",
            "https://example.com/\"kutip\"",
            "https://example.com/'kutip'",
            "https://example.com/\\backslash",
            "https://example.com/{a,b}",
        ];
        for s in jahat {
            let hasil = validate(s);
            assert!(hasil.is_err(), "harus ditolak: {s:?}");
            let err = hasil.unwrap_err();
            assert_eq!(err.code(), "VT-URL-001");
        }
    }

    #[test]
    fn tautan_kosong_atau_kepanjangan_ditolak() {
        assert!(validate("").is_err());
        let panjang = format!("https://example.com/{}", "a".repeat(MAX_PANJANG));
        assert!(panjang.len() > MAX_PANJANG);
        assert!(validate(&panjang).is_err());

        let pas = format!("https://example.com/{}", "a".repeat(MAX_PANJANG - 20));
        assert!(validate(&pas).is_ok());
    }

    #[test]
    fn tanpa_skema_dan_tanpa_host_ditolak() {
        assert!(validate("example.com/video.mp4").is_err());
        assert!(validate("https://").is_err());
    }

    #[test]
    fn pesan_error_ramah_untuk_pengguna() {
        let err = validate("javascript:alert(1)").unwrap_err();
        assert!(err.user_message().contains("Tautan ini tidak bisa dipakai"));
    }
}
