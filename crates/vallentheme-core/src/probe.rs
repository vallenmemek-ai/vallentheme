//! Pembacaan metadata media lewat `ffprobe`.
//!
//! Semua proses dijalankan **tanpa shell**, dengan timeout, dan keluaran
//! dibatasi. Rujukan: `panduan/06-referensi-teknis.md` bagian 5.

use std::ffi::OsString;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::model::MediaKind;

/// Batas ukuran keluaran ffprobe yang mau dibaca (byte).
const BATAS_KELUARAN: usize = 1 << 20;

/// Metadata satu berkas media.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    /// Jenis media (foto atau video).
    pub kind: MediaKind,
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
}

#[derive(Debug, Deserialize)]
struct KeluaranFfprobe {
    #[serde(default)]
    streams: Vec<Stream>,
    #[serde(default)]
    format: Option<Format>,
}

#[derive(Debug, Deserialize)]
struct Stream {
    #[serde(default)]
    codec_type: Option<String>,
    #[serde(default)]
    codec_name: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    avg_frame_rate: Option<String>,
    #[serde(default)]
    nb_frames: Option<String>,
    #[serde(default)]
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Format {
    #[serde(default)]
    duration: Option<String>,
    #[serde(default)]
    size: Option<String>,
}

/// Ubah pecahan `"24000/1001"` menjadi angka.
pub fn parse_fps(pecahan: &str) -> Option<f64> {
    let (atas, bawah) = pecahan.split_once('/')?;
    let atas: f64 = atas.trim().parse().ok()?;
    let bawah: f64 = bawah.trim().parse().ok()?;
    if bawah <= 0.0 || !atas.is_finite() {
        return None;
    }
    let nilai = atas / bawah;
    if nilai.is_finite() && nilai > 0.0 {
        Some(nilai)
    } else {
        None
    }
}

/// Baca metadata sebuah berkas.
pub fn probe(path: &Path, timeout: Duration) -> Result<MediaInfo> {
    let ukuran = std::fs::metadata(path)
        .map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();

    let args: Vec<OsString> = vec![
        "-v".into(),
        "error".into(),
        "-print_format".into(),
        "json".into(),
        "-show_format".into(),
        "-show_streams".into(),
        "--".into(),
        path.as_os_str().to_os_string(),
    ];

    let keluaran = jalankan("ffprobe", &args, timeout)?;
    let data: KeluaranFfprobe = serde_json::from_str(&keluaran)
        .map_err(|e| Error::Probe(format!("keluaran ffprobe tidak bisa dibaca: {e}")))?;

    let video = data
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"));
    let punya_audio = data
        .streams
        .iter()
        .any(|s| s.codec_type.as_deref() == Some("audio"));

    let kind = match video {
        None => MediaKind::Image,
        Some(v) => {
            // GIF/WebP animasi punya lebih dari satu frame → perlakukan sebagai video.
            let banyak_frame = v
                .nb_frames
                .as_deref()
                .and_then(|n| n.parse::<u64>().ok())
                .map(|n| n > 1)
                .unwrap_or(false);
            if banyak_frame {
                MediaKind::Video
            } else {
                crate::model::kind_from_extension(path).unwrap_or(MediaKind::Image)
            }
        }
    };

    let durasi = data
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .or_else(|| video.and_then(|v| v.duration.as_deref()))
        .and_then(|d| d.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
        .map(|d| (d * 1000.0).round() as i64);

    let ukuran_lapor = data
        .format
        .as_ref()
        .and_then(|f| f.size.as_deref())
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or_else(|| i64::try_from(ukuran).unwrap_or(0));

    Ok(MediaInfo {
        kind,
        width: video.and_then(|v| v.width),
        height: video.and_then(|v| v.height),
        fps: video
            .and_then(|v| v.avg_frame_rate.as_deref())
            .and_then(parse_fps),
        duration_ms: durasi,
        codec: video.and_then(|v| v.codec_name.clone()),
        has_audio: punya_audio,
        size_bytes: ukuran_lapor,
    })
}

/// Jalankan program tanpa shell dengan timeout; kembalikan stdout sebagai teks.
pub(crate) fn jalankan(program: &str, args: &[OsString], timeout: Duration) -> Result<String> {
    let mut anak = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Error::ProbeToolMissing(program.to_string()),
            _ => Error::Probe(format!("gagal menjalankan {program}: {e}")),
        })?;

    let batas = Instant::now() + timeout;
    loop {
        match anak.try_wait() {
            Ok(Some(status)) => {
                let keluaran = anak
                    .wait_with_output()
                    .map_err(|e| Error::Probe(format!("gagal membaca keluaran {program}: {e}")))?;
                if !status.success() {
                    let pesan = String::from_utf8_lossy(&keluaran.stderr);
                    let potong: String = pesan.chars().take(400).collect();
                    return Err(Error::Probe(format!(
                        "{program} keluar dengan status {status}: {potong}"
                    )));
                }
                let mut teks = String::from_utf8_lossy(&keluaran.stdout).into_owned();
                teks.truncate(BATAS_KELUARAN);
                return Ok(teks);
            }
            Ok(None) => {
                if Instant::now() >= batas {
                    let _ = anak.kill();
                    let _ = anak.wait();
                    return Err(Error::Probe(format!(
                        "{program} melebihi batas waktu {} detik",
                        timeout.as_secs()
                    )));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => {
                let _ = anak.kill();
                return Err(Error::Probe(format!("gagal menunggu {program}: {e}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn fps_pecahan_dibaca() {
        assert!((parse_fps("24000/1001").unwrap() - 23.976).abs() < 0.01);
        assert_eq!(parse_fps("30/1"), Some(30.0));
        assert_eq!(parse_fps("25"), None, "tanpa garis miring harus None");
        assert_eq!(parse_fps("0/0"), None, "pembagian nol harus None");
        assert_eq!(parse_fps("abc/def"), None);
        assert_eq!(parse_fps("30/0"), None);
    }

    #[test]
    fn berkas_tidak_ada_menghasilkan_error_io() {
        let err = probe(Path::new("/tidak/ada/berkas.mp4"), Duration::from_secs(2)).unwrap_err();
        assert_eq!(err.code(), "VT-IO-001");
    }

    #[test]
    fn berkas_bukan_media_ditolak_dengan_kode_probe() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("palsu.mp4");
        std::fs::write(&path, b"ini hanya teks biasa").unwrap();

        let err = probe(&path, Duration::from_secs(15)).unwrap_err();
        assert_eq!(err.code(), "VT-PROBE-002");
    }
}
