//! Pembuatan dan pembersihan thumbnail.
//!
//! Nama berkas diturunkan dari `blake3(path + mtime + size)` sehingga thumbnail
//! lama otomatis tidak terpakai saat berkas berubah. Rujukan:
//! `panduan/06-referensi-teknis.md` bagian 5.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::probe::jalankan;

/// Lebar thumbnail untuk daftar library.
pub const LEBAR_THUMB: u32 = 480;
/// Lebar frame pratinjau untuk layar Edit.
pub const LEBAR_PRATINJAU: u32 = 960;
/// Timeout pembuatan thumbnail.
pub const TIMEOUT_THUMB: Duration = Duration::from_secs(20);

/// Kunci cache untuk sebuah berkas.
pub fn kunci_cache(path: &Path, mtime: i64, ukuran: i64) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(path.to_string_lossy().as_bytes());
    hasher.update(b"|");
    hasher.update(mtime.to_le_bytes().as_slice());
    hasher.update(b"|");
    hasher.update(ukuran.to_le_bytes().as_slice());
    hasher.finalize().to_hex().to_string()
}

/// Waktu pengambilan frame video: 10% durasi, minimal 1 detik.
pub fn waktu_frame(durasi_ms: Option<i64>) -> f64 {
    match durasi_ms {
        Some(ms) if ms > 0 => {
            let detik = ms as f64 / 1000.0;
            if detik < 1.0 {
                0.0
            } else {
                (detik * 0.1).max(1.0)
            }
        }
        // Foto tidak punya durasi: ambil frame pertama tanpa seek.
        _ => 0.0,
    }
}

/// Path tujuan thumbnail untuk sebuah kunci.
pub fn path_thumb(dir: &Path, kunci: &str) -> PathBuf {
    dir.join(format!("{kunci}.jpg"))
}

/// Buat thumbnail (atau pakai yang sudah ada).
pub fn buat(
    sumber: &Path,
    dir_cache: &Path,
    kunci: &str,
    lebar: u32,
    waktu_detik: f64,
) -> Result<PathBuf> {
    let tujuan = path_thumb(dir_cache, kunci);
    if tujuan.is_file() {
        return Ok(tujuan);
    }

    std::fs::create_dir_all(dir_cache).map_err(|source| Error::Io {
        path: dir_cache.to_path_buf(),
        source,
    })?;

    // Akhiran harus tetap `.jpg` agar ffmpeg bisa menebak format keluaran.
    let sementara = dir_cache.join(format!("{kunci}.tmp.jpg"));
    let mut args: Vec<OsString> = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
    ];
    // Foto (tanpa durasi) tidak boleh di-seek; hasilnya kosong.
    if waktu_detik > 0.0 {
        args.push("-ss".into());
        args.push(format!("{waktu_detik:.3}").into());
    }
    args.extend([
        "-i".into(),
        sumber.as_os_str().to_os_string(),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        format!("scale={lebar}:-2").into(),
        "-q:v".into(),
        "4".into(),
        "--".into(),
        sementara.as_os_str().to_os_string(),
    ]);

    if let Err(err) = jalankan("ffmpeg", &args, TIMEOUT_THUMB) {
        let _ = std::fs::remove_file(&sementara);
        return Err(Error::Thumb(err.to_string()));
    }

    if !sementara.is_file() {
        return Err(Error::Thumb("ffmpeg tidak menghasilkan berkas".to_string()));
    }

    std::fs::rename(&sementara, &tujuan).map_err(|source| Error::Io {
        path: tujuan.clone(),
        source,
    })?;
    Ok(tujuan)
}

/// Hitung ukuran total isi sebuah folder (byte).
pub fn ukuran_folder(dir: &Path) -> u64 {
    let Ok(entri) = std::fs::read_dir(dir) else {
        return 0;
    };
    entri
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

/// Hapus thumbnail paling lama sampai ukuran folder di bawah `batas_byte`.
///
/// Mengembalikan jumlah berkas yang dihapus.
pub fn pangkas_cache(dir: &Path, batas_byte: u64) -> usize {
    let Ok(entri) = std::fs::read_dir(dir) else {
        return 0;
    };

    let mut berkas: Vec<(std::time::SystemTime, u64, PathBuf)> = entri
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            Some((meta.modified().ok()?, meta.len(), e.path()))
        })
        .collect();

    let mut total: u64 = berkas.iter().map(|(_, ukuran, _)| *ukuran).sum();
    if total <= batas_byte {
        return 0;
    }

    // Yang paling lama diakses dihapus lebih dulu.
    berkas.sort_by_key(|(waktu, _, _)| *waktu);

    let mut dihapus = 0;
    for (_, ukuran, path) in berkas {
        if total <= batas_byte {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(ukuran);
            dihapus += 1;
        }
    }
    dihapus
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn kunci_berubah_bila_berkas_berubah() {
        let a = kunci_cache(Path::new("/a.mp4"), 100, 500);
        let b = kunci_cache(Path::new("/a.mp4"), 101, 500);
        let c = kunci_cache(Path::new("/a.mp4"), 100, 501);
        let d = kunci_cache(Path::new("/b.mp4"), 100, 500);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_eq!(a.len(), 64, "blake3 heksadesimal 32 byte");
    }

    #[test]
    fn kunci_stabil_untuk_masukan_sama() {
        assert_eq!(
            kunci_cache(Path::new("/a.mp4"), 1, 2),
            kunci_cache(Path::new("/a.mp4"), 1, 2)
        );
    }

    #[test]
    fn waktu_frame_mengikuti_aturan() {
        assert_eq!(waktu_frame(Some(10_000)), 1.0, "10% dari 10 dtk = 1 dtk");
        assert_eq!(waktu_frame(Some(60_000)), 6.0);
        assert_eq!(waktu_frame(Some(500)), 0.0, "durasi < 1 dtk pakai 0");
        assert_eq!(waktu_frame(None), 0.0, "tanpa durasi: tanpa seek");
        assert_eq!(waktu_frame(Some(0)), 0.0);
    }

    #[test]
    fn path_thumb_memakai_ekstensi_jpg() {
        let p = path_thumb(Path::new("/cache"), "abc");
        assert_eq!(p, PathBuf::from("/cache/abc.jpg"));
    }

    #[test]
    fn pangkas_cache_menghapus_yang_paling_lama() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("{i}.jpg")), vec![0u8; 1000]).unwrap();
            // Beri jeda agar mtime berbeda.
            std::thread::sleep(Duration::from_millis(15));
        }
        assert_eq!(ukuran_folder(dir.path()), 5000);

        let dihapus = pangkas_cache(dir.path(), 2000);
        assert_eq!(dihapus, 3);
        assert!(ukuran_folder(dir.path()) <= 2000);
    }

    #[test]
    fn pangkas_cache_tidak_menghapus_bila_muat() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jpg"), vec![0u8; 100]).unwrap();
        assert_eq!(pangkas_cache(dir.path(), 1_000_000), 0);
        assert!(dir.path().join("a.jpg").exists());
    }

    #[test]
    fn folder_tidak_ada_tidak_panic() {
        assert_eq!(ukuran_folder(Path::new("/tidak/ada")), 0);
        assert_eq!(pangkas_cache(Path::new("/tidak/ada"), 0), 0);
    }
}
