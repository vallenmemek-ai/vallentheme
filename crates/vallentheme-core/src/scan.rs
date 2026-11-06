//! Pemindaian folder dan impor ke library.
//!
//! Batas kedalaman 8, deteksi loop symlink lewat `(dev, inode)`, abaikan berkas
//! tersembunyi serta folder `.git`/`node_modules`, dan dukung pembatalan.
//! Rujukan: `fase/FASE-2-core.md` bagian 2.4.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::db::{Db, MediaEntry, UpsertOutcome};
use crate::error::{Error, Result};
use crate::model::{kind_from_extension, FolderId, MediaKind, WallpaperId};
use crate::{probe, thumb};

/// Kedalaman maksimum penelusuran folder.
pub const KEDALAMAN_MAKS: usize = 8;

/// Nama folder yang selalu dilewati.
const FOLDER_DILEWATI: &[&str] = &[".git", "node_modules", ".cache", ".local"];

/// Pengaturan pemindaian.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOptions {
    /// Telusuri subfolder.
    pub recursive: bool,
    /// Batasi ke jenis media tertentu; `None` berarti semua.
    pub kinds: Option<Vec<MediaKind>>,
    /// Kedalaman maksimum.
    pub max_depth: usize,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            recursive: true,
            kinds: None,
            max_depth: KEDALAMAN_MAKS,
        }
    }
}

impl ScanOptions {
    /// Apakah jenis ini diterima.
    fn terima(&self, kind: MediaKind) -> bool {
        match &self.kinds {
            None => true,
            Some(daftar) => daftar.contains(&kind),
        }
    }
}

/// Ringkasan satu sesi impor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// Jumlah entri baru.
    pub ditambah: usize,
    /// Jumlah entri yang diperbarui.
    pub diperbarui: usize,
    /// Berkas yang dilewati beserta alasannya.
    pub dilewati: Vec<(PathBuf, String)>,
    /// Impor dihentikan pengguna.
    pub dibatalkan: bool,
}

/// Hasil impor satu berkas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportOutcome {
    /// Entri baru dibuat.
    Ditambah(WallpaperId),
    /// Entri lama diperbarui.
    Diperbarui(WallpaperId),
    /// Berkas dilewati dengan alasan.
    Dilewati(String),
}

/// Kumpulkan berkas media yang didukung dari satu atau beberapa akar.
pub fn collect_files(akar: &[PathBuf], opts: &ScanOptions, batal: &AtomicBool) -> Vec<PathBuf> {
    let mut hasil = Vec::new();
    let mut dikunjungi: HashSet<(u64, u64)> = HashSet::new();

    for root in akar {
        if batal.load(Ordering::Relaxed) {
            break;
        }
        let Ok(meta) = std::fs::metadata(root) else {
            continue;
        };

        if meta.is_file() {
            if let Some(kind) = kind_from_extension(root) {
                if opts.terima(kind) {
                    hasil.push(root.clone());
                }
            }
            continue;
        }
        if !meta.is_dir() {
            continue;
        }

        telusuri(root, 0, opts, batal, &mut dikunjungi, &mut hasil);
    }

    hasil.sort();
    hasil.dedup();
    hasil
}

fn telusuri(
    dir: &Path,
    kedalaman: usize,
    opts: &ScanOptions,
    batal: &AtomicBool,
    dikunjungi: &mut HashSet<(u64, u64)>,
    hasil: &mut Vec<PathBuf>,
) {
    if batal.load(Ordering::Relaxed) || kedalaman > opts.max_depth {
        return;
    }

    if let Some(kunci) = identitas(dir) {
        if !dikunjungi.insert(kunci) {
            tracing::debug!(folder = %dir.display(), "loop symlink terdeteksi, dilewati");
            return;
        }
    }

    let Ok(entri) = std::fs::read_dir(dir) else {
        tracing::debug!(folder = %dir.display(), "folder tidak bisa dibaca");
        return;
    };

    for entri in entri.flatten() {
        if batal.load(Ordering::Relaxed) {
            return;
        }
        let nama = entri.file_name();
        let nama_teks = nama.to_string_lossy();

        if nama_teks.starts_with('.') || FOLDER_DILEWATI.contains(&nama_teks.as_ref()) {
            continue;
        }

        let path = entri.path();
        let Ok(meta) = entri.metadata() else {
            continue;
        };

        if meta.is_dir() {
            if opts.recursive {
                telusuri(&path, kedalaman + 1, opts, batal, dikunjungi, hasil);
            }
            continue;
        }

        // Ikuti symlink ke berkas biasa, tetapi jangan ke folder (hindari loop).
        let kandidat = if meta.file_type().is_symlink() {
            match std::fs::metadata(&path) {
                Ok(m) if m.is_file() => path.clone(),
                _ => continue,
            }
        } else if meta.is_file() {
            path.clone()
        } else {
            continue;
        };

        if let Some(kind) = kind_from_extension(&kandidat) {
            if opts.terima(kind) {
                hasil.push(kandidat);
            }
        }
    }
}

/// Identitas `(device, inode)` untuk deteksi loop.
#[cfg(unix)]
fn identitas(path: &Path) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.dev(), meta.ino()))
}

/// Di luar Unix deteksi loop tidak tersedia.
#[cfg(not(unix))]
fn identitas(_path: &Path) -> Option<(u64, u64)> {
    None
}

/// Impor satu berkas ke library.
pub fn import_path(db: &mut Db, path: &Path, folder: Option<FolderId>) -> Result<ImportOutcome> {
    let Some(kind) = kind_from_extension(path) else {
        return Ok(ImportOutcome::Dilewati("format tidak didukung".to_string()));
    };

    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(source) => {
            return Ok(ImportOutcome::Dilewati(match source.kind() {
                std::io::ErrorKind::NotFound => "berkas tidak ditemukan".to_string(),
                std::io::ErrorKind::PermissionDenied => "izin baca ditolak".to_string(),
                _ => source.to_string(),
            }))
        }
    };

    if meta.len() == 0 {
        return Ok(ImportOutcome::Dilewati(
            "berkas kosong (0 byte)".to_string(),
        ));
    }

    let mtime = meta
        .modified()
        .ok()
        .and_then(|w| w.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(0))
        .unwrap_or(0);

    let info = match probe::probe(path, Duration::from_secs(15)) {
        Ok(info) => info,
        Err(err) => return Ok(ImportOutcome::Dilewati(err.to_string())),
    };

    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "tanpa nama".to_string());

    let entry = MediaEntry {
        path: path.to_path_buf(),
        kind: if info.kind == MediaKind::Image {
            kind
        } else {
            info.kind
        },
        title,
        width: info.width,
        height: info.height,
        fps: info.fps,
        duration_ms: info.duration_ms,
        codec: info.codec,
        has_audio: info.has_audio,
        size_bytes: i64::try_from(meta.len()).unwrap_or(i64::MAX),
        mtime,
        folder_id: folder,
    };

    let (id, hasil) = db.upsert(&entry)?;
    Ok(match hasil {
        UpsertOutcome::Ditambah => ImportOutcome::Ditambah(id),
        UpsertOutcome::Diperbarui => ImportOutcome::Diperbarui(id),
    })
}

/// Impor banyak berkas; `on_item` dipanggil per berkas untuk progres bertahap.
pub fn import_paths<F>(
    db: &mut Db,
    paths: &[PathBuf],
    folder: Option<FolderId>,
    batal: &AtomicBool,
    mut on_item: F,
) -> Result<ScanReport>
where
    F: FnMut(&Path, &ImportOutcome),
{
    let mut laporan = ScanReport::default();

    for path in paths {
        if batal.load(Ordering::Relaxed) {
            laporan.dibatalkan = true;
            break;
        }
        let hasil = import_path(db, path, folder)?;
        match &hasil {
            ImportOutcome::Ditambah(_) => laporan.ditambah += 1,
            ImportOutcome::Diperbarui(_) => laporan.diperbarui += 1,
            ImportOutcome::Dilewati(alasan) => {
                laporan.dilewati.push((path.clone(), alasan.clone()));
            }
        }
        on_item(path, &hasil);
    }

    Ok(laporan)
}

/// Periksa keberadaan berkas dan perbarui tanda `missing`.
///
/// Mengembalikan jumlah entri yang statusnya berubah.
pub fn verify_files(db: &Db, batas: usize) -> Result<usize> {
    let semua = db.semua_id_dan_path()?;
    let mut berubah = 0;

    for (id, path) in semua.into_iter().take(batas) {
        let ada = path.is_file();
        let sebelumnya = db.get(id)?.map(|w| w.missing).unwrap_or(false);
        if sebelumnya != !ada {
            db.mark_missing(&[id], !ada)?;
            berubah += 1;
        }
    }
    Ok(berubah)
}

/// Buat thumbnail untuk satu wallpaper dan simpan path-nya.
pub fn buat_thumbnail(
    db: &Db,
    id: WallpaperId,
    dir_cache: &Path,
    mtime: i64,
    ukuran: i64,
    durasi_ms: Option<i64>,
) -> Result<PathBuf> {
    let w = db
        .get(id)?
        .ok_or_else(|| Error::Thumb(format!("wallpaper {id} tidak ada")))?;
    let kunci = thumb::kunci_cache(&w.path, mtime, ukuran);
    let path = thumb::buat(
        &w.path,
        dir_cache,
        &kunci,
        thumb::LEBAR_THUMB,
        thumb::waktu_frame(durasi_ms),
    )?;
    db.set_thumb_path(id, Some(&path))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::model::Filter;

    fn sentuh(path: &Path) {
        std::fs::write(path, b"x").unwrap();
    }

    #[test]
    fn mengumpulkan_berkas_didukung_saja() {
        let dir = tempfile::tempdir().unwrap();
        sentuh(&dir.path().join("a.mp4"));
        sentuh(&dir.path().join("b.jpg"));
        sentuh(&dir.path().join("c.txt"));
        sentuh(&dir.path().join("tanpa_ekstensi"));

        let batal = AtomicBool::new(false);
        let hasil = collect_files(&[dir.path().to_path_buf()], &ScanOptions::default(), &batal);
        assert_eq!(hasil.len(), 2);
        assert!(hasil.iter().any(|p| p.ends_with("a.mp4")));
        assert!(hasil.iter().any(|p| p.ends_with("b.jpg")));
    }

    #[test]
    fn berkas_tersembunyi_dilewati() {
        let dir = tempfile::tempdir().unwrap();
        sentuh(&dir.path().join(".rahasia.mp4"));
        sentuh(&dir.path().join("tampak.mp4"));

        let batal = AtomicBool::new(false);
        let hasil = collect_files(&[dir.path().to_path_buf()], &ScanOptions::default(), &batal);
        assert_eq!(hasil.len(), 1);
    }

    #[test]
    fn folder_terlarang_dilewati() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join(".git");
        std::fs::create_dir_all(&git).unwrap();
        sentuh(&git.join("x.mp4"));

        let batal = AtomicBool::new(false);
        let hasil = collect_files(&[dir.path().to_path_buf()], &ScanOptions::default(), &batal);
        assert!(hasil.is_empty());
    }

    #[test]
    fn tidak_rekursif_hanya_level_pertama() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        sentuh(&dir.path().join("atas.mp4"));
        sentuh(&sub.join("dalam.mp4"));

        let opts = ScanOptions {
            recursive: false,
            ..ScanOptions::default()
        };
        let batal = AtomicBool::new(false);
        let hasil = collect_files(&[dir.path().to_path_buf()], &opts, &batal);
        assert_eq!(hasil.len(), 1);
        assert!(hasil[0].ends_with("atas.mp4"));
    }

    #[test]
    fn filter_jenis_dihormati() {
        let dir = tempfile::tempdir().unwrap();
        sentuh(&dir.path().join("a.mp4"));
        sentuh(&dir.path().join("b.jpg"));

        let opts = ScanOptions {
            kinds: Some(vec![MediaKind::Image]),
            ..ScanOptions::default()
        };
        let batal = AtomicBool::new(false);
        let hasil = collect_files(&[dir.path().to_path_buf()], &opts, &batal);
        assert_eq!(hasil.len(), 1);
        assert!(hasil[0].ends_with("b.jpg"));
    }

    #[test]
    fn pembatalan_menghentikan_penelusuran() {
        let dir = tempfile::tempdir().unwrap();
        sentuh(&dir.path().join("a.mp4"));

        let batal = AtomicBool::new(true);
        let hasil = collect_files(&[dir.path().to_path_buf()], &ScanOptions::default(), &batal);
        assert!(hasil.is_empty());
    }

    #[test]
    fn kedalaman_dibatasi() {
        let dir = tempfile::tempdir().unwrap();
        let mut dalam = dir.path().to_path_buf();
        for i in 0..12 {
            dalam = dalam.join(format!("d{i}"));
        }
        std::fs::create_dir_all(&dalam).unwrap();
        sentuh(&dalam.join("jauh.mp4"));

        let batal = AtomicBool::new(false);
        let hasil = collect_files(&[dir.path().to_path_buf()], &ScanOptions::default(), &batal);
        assert!(hasil.is_empty(), "berkas di kedalaman 12 harus dilewati");
    }

    #[test]
    fn impor_berkas_asing_dilewati() {
        let mut db = Db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catatan.txt");
        sentuh(&path);

        match import_path(&mut db, &path, None).unwrap() {
            ImportOutcome::Dilewati(alasan) => assert!(alasan.contains("tidak didukung")),
            lain => panic!("seharusnya dilewati, dapat {lain:?}"),
        }
        assert_eq!(db.count(&Filter::default()).unwrap(), 0);
    }

    #[test]
    fn impor_berkas_kosong_dilewati() {
        let mut db = Db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kosong.mp4");
        std::fs::write(&path, b"").unwrap();

        match import_path(&mut db, &path, None).unwrap() {
            ImportOutcome::Dilewati(alasan) => assert!(alasan.contains("0 byte")),
            lain => panic!("seharusnya dilewati, dapat {lain:?}"),
        }
    }

    #[test]
    fn impor_berkas_palsu_dilewati_tanpa_panic() {
        let mut db = Db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("palsu.mp4");
        std::fs::write(&path, b"bukan video sungguhan").unwrap();

        let hasil = import_path(&mut db, &path, None).unwrap();
        assert!(matches!(hasil, ImportOutcome::Dilewati(_)));
    }

    #[test]
    fn verify_menandai_berkas_hilang() {
        let mut db = Db::open_in_memory().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hilang.mp4");
        sentuh(&path);

        // Sisipkan manual agar tidak perlu berkas media sungguhan.
        let entry = MediaEntry {
            path: path.clone(),
            kind: MediaKind::Video,
            title: "hilang".to_string(),
            width: Some(1920),
            height: Some(1080),
            fps: Some(24.0),
            duration_ms: Some(1000),
            codec: Some("h264".to_string()),
            has_audio: false,
            size_bytes: 1,
            mtime: 0,
            folder_id: None,
        };
        let (id, _) = db.upsert(&entry).unwrap();

        std::fs::remove_file(&path).unwrap();
        let berubah = verify_files(&db, 100).unwrap();
        assert_eq!(berubah, 1);
        assert!(db.get(id).unwrap().unwrap().missing);
    }
}
