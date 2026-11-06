//! Basis data library (SQLite, mode WAL).
//!
//! Skema persis mengikuti `panduan/06-referensi-teknis.md` bagian 7. Migrasi
//! bernomor dijalankan dalam transaksi dan versinya dicatat di tabel `meta`.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, Row};

use crate::error::{Error, Result};
use crate::model::{
    Folder, FolderId, MediaKind, SortKey, Wallpaper, WallpaperId, WallpaperSettings,
};
use crate::search;

/// Versi skema yang dikenal aplikasi ini.
pub const SCHEMA_VERSION: u32 = 1;

/// Skema awal.
const M001_INIT: &str = r#"
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE folders (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  recursive INTEGER NOT NULL DEFAULT 1,
  added_at INTEGER NOT NULL
);

CREATE TABLE wallpapers (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL CHECK (kind IN ('image','video')),
  title TEXT NOT NULL,
  width INTEGER, height INTEGER,
  fps REAL, duration_ms INTEGER, codec TEXT,
  has_audio INTEGER NOT NULL DEFAULT 0,
  size_bytes INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL,
  thumb_path TEXT,
  favorite INTEGER NOT NULL DEFAULT 0,
  missing INTEGER NOT NULL DEFAULT 0,
  sort_index INTEGER NOT NULL,
  added_at INTEGER NOT NULL,
  last_used_at INTEGER,
  deleted_at INTEGER,
  settings_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_wp_kind ON wallpapers(kind);
CREATE INDEX idx_wp_folder ON wallpapers(folder_id);
CREATE INDEX idx_wp_added ON wallpapers(added_at DESC);
CREATE INDEX idx_wp_used ON wallpapers(last_used_at DESC);
CREATE INDEX idx_wp_fav ON wallpapers(favorite) WHERE favorite = 1;
"#;

/// Daftar migrasi berurutan: (nama, SQL).
const MIGRATIONS: &[(&str, &str)] = &[("m001_init", M001_INIT)];

/// Kolom yang dibaca saat memetakan baris menjadi [`Wallpaper`].
const KOLOM_WALLPAPER: &str = "id, path, kind, title, width, height, fps, duration_ms, codec, \
     has_audio, size_bytes, mtime, folder_id, thumb_path, favorite, missing, sort_index, \
     added_at, last_used_at, settings_json";

/// Hasil operasi impor satu berkas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpsertOutcome {
    /// Entri baru dibuat.
    Ditambah,
    /// Entri lama diperbarui.
    Diperbarui,
}

/// Data satu media hasil pemindaian, siap dimasukkan ke basis data.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaEntry {
    /// Path absolut berkas.
    pub path: PathBuf,
    /// Jenis media.
    pub kind: MediaKind,
    /// Judul tampilan.
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
    /// Waktu modifikasi (detik Unix).
    pub mtime: i64,
    /// Folder asal.
    pub folder_id: Option<FolderId>,
}

/// Pegangan basis data library.
pub struct Db {
    conn: Connection,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Db { .. }")
    }
}

/// Waktu sekarang sebagai detik Unix.
pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn db_err(err: rusqlite::Error) -> Error {
    Error::Db(err.to_string())
}

/// Magic header berkas SQLite.
const HEADER_SQLITE: &[u8] = b"SQLite format 3\0";

/// Tolak berkas yang ada tetapi jelas bukan basis data SQLite.
fn periksa_header(path: &Path) -> Result<()> {
    let Ok(meta) = std::fs::metadata(path) else {
        return Ok(()); // belum ada; akan dibuat oleh SQLite
    };
    if meta.len() < HEADER_SQLITE.len() as u64 {
        return Ok(()); // kosong atau terlalu pendek; biar SQLite yang menilai
    }
    let mut buf = [0u8; 16];
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let terbaca = file.read(&mut buf).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if terbaca >= HEADER_SQLITE.len() && &buf[..HEADER_SQLITE.len()] != HEADER_SQLITE {
        return Err(Error::DbCorrupt("bukan berkas SQLite".to_string()));
    }
    Ok(())
}

impl Db {
    /// Buka basis data pada `path`, siapkan pragma, lalu jalankan migrasi.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        periksa_header(path)?;
        let conn = Connection::open(path).map_err(db_err)?;
        Self::siapkan(conn)
    }

    /// Basis data di memori, untuk pengujian.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        Self::siapkan(conn)
    }

    /// Buka basis data; bila korup, cadangkan lalu buat baru.
    ///
    /// Mengembalikan `(db, Some(path_cadangan))` bila pemulihan terjadi.
    pub fn open_recovering(path: &Path) -> Result<(Self, Option<PathBuf>)> {
        match Self::open(path) {
            Ok(db) => Ok((db, None)),
            Err(Error::DbCorrupt(_)) => {
                let cap = path.with_extension(format!("db.corrupt-{}", now_unix()));
                std::fs::rename(path, &cap).map_err(|source| Error::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
                // Berkas WAL/SHM ikut dipindahkan agar tidak mengotori DB baru.
                for akhiran in ["-wal", "-shm"] {
                    let samping = PathBuf::from(format!("{}{akhiran}", path.display()));
                    if samping.exists() {
                        let tujuan = PathBuf::from(format!("{}{akhiran}", cap.display()));
                        let _ = std::fs::rename(&samping, tujuan);
                    }
                }
                tracing::warn!(cadangan = %cap.display(), "basis data korup dicadangkan");
                let db = Self::open(path)?;
                Ok((db, Some(cap)))
            }
            Err(err) => Err(err),
        }
    }

    fn siapkan(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(db_err)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(db_err)?;

        let hasil: String = conn
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(db_err)?;
        if hasil != "ok" {
            return Err(Error::DbCorrupt(hasil));
        }

        let db = Self { conn };
        db.migrasi()?;
        Ok(db)
    }

    /// Jalankan migrasi yang belum diterapkan.
    fn migrasi(&self) -> Result<()> {
        let ada_meta: bool = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='meta'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map_err(db_err)?
            > 0;

        let versi_sekarang: u32 = if ada_meta {
            let teks: Option<String> = self
                .conn
                .query_row(
                    "SELECT value FROM meta WHERE key='schema_version'",
                    [],
                    |row| row.get(0),
                )
                .ok();
            teks.and_then(|t| t.parse().ok()).unwrap_or(0)
        } else {
            0
        };

        if versi_sekarang > SCHEMA_VERSION {
            return Err(Error::DbNewerSchema(versi_sekarang));
        }

        for (indeks, (nama, sql)) in MIGRATIONS.iter().enumerate() {
            let nomor = u32::try_from(indeks + 1).unwrap_or(u32::MAX);
            if nomor <= versi_sekarang {
                continue;
            }
            tracing::info!(migrasi = nama, "menerapkan migrasi basis data");
            self.conn.execute_batch(sql).map_err(db_err)?;
            self.conn
                .execute(
                    "INSERT INTO meta(key, value) VALUES('schema_version', ?1)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    params![nomor.to_string()],
                )
                .map_err(db_err)?;
        }
        Ok(())
    }

    /// Akses langsung ke koneksi (untuk pengujian dan modul internal).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // ---------- Wallpaper ----------

    /// Indeks urutan berikutnya untuk entri baru.
    fn sort_index_berikut(&self) -> Result<i64> {
        let nilai: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(sort_index), 0) + 1 FROM wallpapers",
                [],
                |row| row.get(0),
            )
            .map_err(db_err)?;
        Ok(nilai)
    }

    /// Masukkan atau perbarui satu media berdasarkan path.
    pub fn upsert(&mut self, entry: &MediaEntry) -> Result<(WallpaperId, UpsertOutcome)> {
        let path_teks = entry.path.to_string_lossy().to_string();
        let ada: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM wallpapers WHERE path = ?1",
                params![path_teks],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = ada {
            self.conn
                .execute(
                    "UPDATE wallpapers SET kind=?2, title=?3, width=?4, height=?5, fps=?6,
                        duration_ms=?7, codec=?8, has_audio=?9, size_bytes=?10, mtime=?11,
                        folder_id=COALESCE(?12, folder_id), missing=0
                     WHERE id=?1",
                    params![
                        id,
                        entry.kind.as_str(),
                        entry.title,
                        entry.width,
                        entry.height,
                        entry.fps,
                        entry.duration_ms,
                        entry.codec,
                        i64::from(entry.has_audio),
                        entry.size_bytes,
                        entry.mtime,
                        entry.folder_id.map(|f| f.0),
                    ],
                )
                .map_err(db_err)?;
            return Ok((WallpaperId(id), UpsertOutcome::Diperbarui));
        }

        let sort_index = self.sort_index_berikut()?;
        self.conn
            .execute(
                "INSERT INTO wallpapers
                   (path, kind, title, width, height, fps, duration_ms, codec, has_audio,
                    size_bytes, mtime, folder_id, sort_index, added_at, settings_json)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,'{}')",
                params![
                    path_teks,
                    entry.kind.as_str(),
                    entry.title,
                    entry.width,
                    entry.height,
                    entry.fps,
                    entry.duration_ms,
                    entry.codec,
                    i64::from(entry.has_audio),
                    entry.size_bytes,
                    entry.mtime,
                    entry.folder_id.map(|f| f.0),
                    sort_index,
                    now_unix(),
                ],
            )
            .map_err(db_err)?;
        Ok((
            WallpaperId(self.conn.last_insert_rowid()),
            UpsertOutcome::Ditambah,
        ))
    }

    /// Ambil satu wallpaper menurut ID.
    pub fn get(&self, id: WallpaperId) -> Result<Option<Wallpaper>> {
        let sql = format!("SELECT {KOLOM_WALLPAPER} FROM wallpapers WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql).map_err(db_err)?;
        let mut baris = stmt.query(params![id.0]).map_err(db_err)?;
        match baris.next().map_err(db_err)? {
            Some(row) => Ok(Some(peta_wallpaper(row)?)),
            None => Ok(None),
        }
    }

    /// Daftar wallpaper sesuai filter dan urutan.
    pub fn list(
        &self,
        filter: &crate::model::Filter,
        sort: SortKey,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Wallpaper>> {
        let (where_sql, nilai) = klausa_filter(filter);

        // Pencarian token dilakukan di Rust agar semantiknya konsisten.
        let ada_query = !filter.query.trim().is_empty();
        let sql = if ada_query {
            format!(
                "SELECT {KOLOM_WALLPAPER} FROM wallpapers WHERE {where_sql} ORDER BY {}",
                sort.order_by()
            )
        } else {
            format!(
                "SELECT {KOLOM_WALLPAPER} FROM wallpapers WHERE {where_sql} ORDER BY {} LIMIT ?{} OFFSET ?{}",
                sort.order_by(),
                nilai.len() + 1,
                nilai.len() + 2
            )
        };

        let mut semua = Vec::new();
        let mut stmt = self.conn.prepare(&sql).map_err(db_err)?;

        if ada_query {
            let mut baris = stmt.query(params_from_iter(nilai.iter())).map_err(db_err)?;
            while let Some(row) = baris.next().map_err(db_err)? {
                semua.push(peta_wallpaper(row)?);
            }
            let kueri = search::parse(&filter.query);
            semua.retain(|w| search::matches(w, &kueri));
            let mulai = usize::try_from(offset).unwrap_or(0);
            let ambil = usize::try_from(limit).unwrap_or(usize::MAX);
            Ok(semua.into_iter().skip(mulai).take(ambil).collect())
        } else {
            let mut nilai_semua = nilai;
            nilai_semua.push(Value::Integer(i64::from(limit)));
            nilai_semua.push(Value::Integer(i64::from(offset)));
            let mut baris = stmt
                .query(params_from_iter(nilai_semua.iter()))
                .map_err(db_err)?;
            while let Some(row) = baris.next().map_err(db_err)? {
                semua.push(peta_wallpaper(row)?);
            }
            Ok(semua)
        }
    }

    /// Hitung jumlah wallpaper sesuai filter.
    pub fn count(&self, filter: &crate::model::Filter) -> Result<u32> {
        let (where_sql, nilai) = klausa_filter(filter);
        let sql = format!("SELECT COUNT(*) FROM wallpapers WHERE {where_sql}");
        let jumlah: i64 = self
            .conn
            .query_row(&sql, params_from_iter(nilai.iter()), |row| row.get(0))
            .map_err(db_err)?;
        Ok(u32::try_from(jumlah).unwrap_or(u32::MAX))
    }

    /// Tandai favorit.
    pub fn set_favorite(&self, id: WallpaperId, favorit: bool) -> Result<()> {
        self.conn
            .execute(
                "UPDATE wallpapers SET favorite = ?2 WHERE id = ?1",
                params![id.0, i64::from(favorit)],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Simpan pengaturan per wallpaper.
    pub fn set_settings(&self, id: WallpaperId, settings: &WallpaperSettings) -> Result<()> {
        self.conn
            .execute(
                "UPDATE wallpapers SET settings_json = ?2 WHERE id = ?1",
                params![id.0, settings.to_json()],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Catat waktu terakhir dipakai.
    pub fn mark_used(&self, id: WallpaperId, kapan: i64) -> Result<()> {
        self.conn
            .execute(
                "UPDATE wallpapers SET last_used_at = ?2 WHERE id = ?1",
                params![id.0, kapan],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Tandai berkas hilang atau kembali ada.
    pub fn mark_missing(&self, ids: &[WallpaperId], hilang: bool) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("UPDATE wallpapers SET missing = ?2 WHERE id = ?1")
            .map_err(db_err)?;
        for id in ids {
            stmt.execute(params![id.0, i64::from(hilang)])
                .map_err(db_err)?;
        }
        Ok(())
    }

    /// Simpan path thumbnail.
    pub fn set_thumb_path(&self, id: WallpaperId, path: Option<&Path>) -> Result<()> {
        self.conn
            .execute(
                "UPDATE wallpapers SET thumb_path = ?2 WHERE id = ?1",
                params![id.0, path.map(|p| p.to_string_lossy().to_string())],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Hapus dari library secara lunak (bisa diurungkan).
    pub fn soft_delete(&self, ids: &[WallpaperId]) -> Result<()> {
        let kapan = now_unix();
        let mut stmt = self
            .conn
            .prepare("UPDATE wallpapers SET deleted_at = ?2 WHERE id = ?1")
            .map_err(db_err)?;
        for id in ids {
            stmt.execute(params![id.0, kapan]).map_err(db_err)?;
        }
        Ok(())
    }

    /// Batalkan penghapusan lunak.
    pub fn restore(&self, ids: &[WallpaperId]) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare("UPDATE wallpapers SET deleted_at = NULL WHERE id = ?1")
            .map_err(db_err)?;
        for id in ids {
            stmt.execute(params![id.0]).map_err(db_err)?;
        }
        Ok(())
    }

    /// Hapus permanen semua entri yang ditandai terhapus sebelum `sebelum`.
    pub fn purge_deleted(&self, sebelum: i64) -> Result<usize> {
        self.conn
            .execute(
                "DELETE FROM wallpapers WHERE deleted_at IS NOT NULL AND deleted_at <= ?1",
                params![sebelum],
            )
            .map_err(db_err)
    }

    /// Semua ID beserta path-nya, untuk pemeriksaan keberadaan berkas.
    pub fn semua_id_dan_path(&self) -> Result<Vec<(WallpaperId, PathBuf)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM wallpapers WHERE deleted_at IS NULL")
            .map_err(db_err)?;
        let baris = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let path: String = row.get(1)?;
                Ok((WallpaperId(id), PathBuf::from(path)))
            })
            .map_err(db_err)?;
        let mut hasil = Vec::new();
        for item in baris {
            hasil.push(item.map_err(db_err)?);
        }
        Ok(hasil)
    }

    // ---------- Folder ----------

    /// Tambahkan folder; path yang sudah ada mengembalikan ID lama.
    pub fn insert_folder(&mut self, path: &Path, recursive: bool) -> Result<FolderId> {
        let teks = path.to_string_lossy().to_string();
        if let Ok(id) = self.conn.query_row(
            "SELECT id FROM folders WHERE path = ?1",
            params![teks],
            |row| row.get::<_, i64>(0),
        ) {
            return Ok(FolderId(id));
        }
        let nama = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| teks.clone());
        self.conn
            .execute(
                "INSERT INTO folders(path, name, recursive, added_at) VALUES (?1, ?2, ?3, ?4)",
                params![teks, nama, i64::from(recursive), now_unix()],
            )
            .map_err(db_err)?;
        Ok(FolderId(self.conn.last_insert_rowid()))
    }

    /// Daftar folder beserta jumlah item.
    pub fn list_folders(&self) -> Result<Vec<Folder>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT f.id, f.path, f.name, f.recursive, f.added_at,
                        (SELECT COUNT(*) FROM wallpapers w
                          WHERE w.folder_id = f.id AND w.deleted_at IS NULL)
                 FROM folders f ORDER BY f.name COLLATE NOCASE",
            )
            .map_err(db_err)?;
        let baris = stmt
            .query_map([], |row| {
                Ok(Folder {
                    id: FolderId(row.get(0)?),
                    path: PathBuf::from(row.get::<_, String>(1)?),
                    name: row.get(2)?,
                    recursive: row.get::<_, i64>(3)? != 0,
                    added_at: row.get(4)?,
                    item_count: u32::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
                })
            })
            .map_err(db_err)?;
        let mut hasil = Vec::new();
        for item in baris {
            hasil.push(item.map_err(db_err)?);
        }
        Ok(hasil)
    }

    /// Hapus folder (entri wallpaper tetap ada, `folder_id` menjadi NULL).
    pub fn delete_folder(&self, id: FolderId) -> Result<()> {
        self.conn
            .execute("DELETE FROM folders WHERE id = ?1", params![id.0])
            .map_err(db_err)?;
        Ok(())
    }
}

/// Susun klausa WHERE beserta nilai terikatnya.
fn klausa_filter(filter: &crate::model::Filter) -> (String, Vec<Value>) {
    let mut klausa = vec!["deleted_at IS NULL".to_string()];
    let mut nilai: Vec<Value> = Vec::new();

    if let Some(folder) = filter.folder {
        klausa.push("folder_id = ?".to_string());
        nilai.push(Value::Integer(folder.0));
    }
    if filter.only_favorites {
        klausa.push("favorite = 1".to_string());
    }
    if let Some(kind) = filter.kind {
        klausa.push("kind = ?".to_string());
        nilai.push(Value::Text(kind.as_str().to_string()));
    }
    (klausa.join(" AND "), nilai)
}

/// Petakan satu baris menjadi [`Wallpaper`].
fn peta_wallpaper(row: &Row<'_>) -> Result<Wallpaper> {
    let kind_teks: String = row.get(2).map_err(db_err)?;
    let kind = MediaKind::parse(&kind_teks).unwrap_or_else(|_| {
        tracing::warn!(jenis = %kind_teks, "jenis media tidak dikenal di basis data");
        MediaKind::Image
    });
    let settings_teks: String = row.get(19).map_err(db_err)?;

    Ok(Wallpaper {
        id: WallpaperId(row.get(0).map_err(db_err)?),
        path: PathBuf::from(row.get::<_, String>(1).map_err(db_err)?),
        kind,
        title: row.get(3).map_err(db_err)?,
        width: row
            .get::<_, Option<i64>>(4)
            .map_err(db_err)?
            .and_then(|v| u32::try_from(v).ok()),
        height: row
            .get::<_, Option<i64>>(5)
            .map_err(db_err)?
            .and_then(|v| u32::try_from(v).ok()),
        fps: row.get(6).map_err(db_err)?,
        duration_ms: row.get(7).map_err(db_err)?,
        codec: row.get(8).map_err(db_err)?,
        has_audio: row.get::<_, i64>(9).map_err(db_err)? != 0,
        size_bytes: row.get(10).map_err(db_err)?,
        mtime: row.get(11).map_err(db_err)?,
        folder_id: row.get::<_, Option<i64>>(12).map_err(db_err)?.map(FolderId),
        thumb_path: row
            .get::<_, Option<String>>(13)
            .map_err(db_err)?
            .map(PathBuf::from),
        favorite: row.get::<_, i64>(14).map_err(db_err)? != 0,
        missing: row.get::<_, i64>(15).map_err(db_err)? != 0,
        sort_index: row.get(16).map_err(db_err)?,
        added_at: row.get(17).map_err(db_err)?,
        last_used_at: row.get(18).map_err(db_err)?,
        settings: WallpaperSettings::from_json(&settings_teks),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::model::Filter;

    fn entri(nama: &str, kind: MediaKind) -> MediaEntry {
        MediaEntry {
            path: PathBuf::from(format!("/video/{nama}")),
            kind,
            title: nama
                .rsplit_once('.')
                .map(|(a, _)| a)
                .unwrap_or(nama)
                .to_string(),
            width: Some(1920),
            height: Some(1080),
            fps: Some(24.0),
            duration_ms: Some(10_000),
            codec: Some("h264".to_string()),
            has_audio: true,
            size_bytes: 1024,
            mtime: 1_700_000_000,
            folder_id: None,
        }
    }

    fn db_isi() -> Db {
        let mut db = Db::open_in_memory().unwrap();
        for nama in ["gunung.mp4", "pantai.mp4", "kota.jpg"] {
            let kind = if nama.ends_with(".jpg") {
                MediaKind::Image
            } else {
                MediaKind::Video
            };
            db.upsert(&entri(nama, kind)).unwrap();
        }
        db
    }

    #[test]
    fn skema_dibuat_dan_versi_tercatat() {
        let db = Db::open_in_memory().unwrap();
        let versi: String = db
            .conn()
            .query_row(
                "SELECT value FROM meta WHERE key='schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(versi, SCHEMA_VERSION.to_string());
    }

    #[test]
    fn upsert_menambah_lalu_memperbarui() {
        let mut db = Db::open_in_memory().unwrap();
        let (id1, hasil1) = db.upsert(&entri("a.mp4", MediaKind::Video)).unwrap();
        assert_eq!(hasil1, UpsertOutcome::Ditambah);

        let (id2, hasil2) = db.upsert(&entri("a.mp4", MediaKind::Video)).unwrap();
        assert_eq!(hasil2, UpsertOutcome::Diperbarui);
        assert_eq!(id1, id2, "path sama tidak boleh membuat entri baru");
        assert_eq!(db.count(&Filter::default()).unwrap(), 1);
    }

    #[test]
    fn sort_index_naik_untuk_entri_baru() {
        let db = db_isi();
        let semua = db.list(&Filter::default(), SortKey::Mine, 100, 0).unwrap();
        let indeks: Vec<i64> = semua.iter().map(|w| w.sort_index).collect();
        assert_eq!(indeks, vec![1, 2, 3]);
    }

    #[test]
    fn filter_jenis_dan_favorit() {
        let db = db_isi();
        let hanya_video = Filter {
            kind: Some(MediaKind::Video),
            ..Filter::default()
        };
        assert_eq!(db.count(&hanya_video).unwrap(), 2);

        let id = db.list(&Filter::default(), SortKey::Mine, 1, 0).unwrap()[0].id;
        db.set_favorite(id, true).unwrap();
        let fav = Filter {
            only_favorites: true,
            ..Filter::default()
        };
        assert_eq!(db.count(&fav).unwrap(), 1);
    }

    #[test]
    fn pencarian_token_bekerja() {
        let db = db_isi();
        let f = Filter {
            query: "pantai".to_string(),
            ..Filter::default()
        };
        let hasil = db.list(&f, SortKey::Mine, 100, 0).unwrap();
        assert_eq!(hasil.len(), 1);
        assert_eq!(hasil[0].title, "pantai");
    }

    #[test]
    fn urutan_nama_dan_ukuran() {
        let db = db_isi();
        let nama = db.list(&Filter::default(), SortKey::Name, 100, 0).unwrap();
        let judul: Vec<&str> = nama.iter().map(|w| w.title.as_str()).collect();
        assert_eq!(judul, vec!["gunung", "kota", "pantai"]);
    }

    #[test]
    fn paginasi_limit_offset() {
        let db = db_isi();
        let halaman = db.list(&Filter::default(), SortKey::Mine, 2, 1).unwrap();
        assert_eq!(halaman.len(), 2);
        assert_eq!(halaman[0].sort_index, 2);
    }

    #[test]
    fn hapus_lunak_dan_pulihkan() {
        let db = db_isi();
        let id = db.list(&Filter::default(), SortKey::Mine, 1, 0).unwrap()[0].id;

        db.soft_delete(&[id]).unwrap();
        assert_eq!(db.count(&Filter::default()).unwrap(), 2);
        assert!(
            db.get(id).unwrap().is_some(),
            "baris masih ada, hanya ditandai"
        );

        db.restore(&[id]).unwrap();
        assert_eq!(db.count(&Filter::default()).unwrap(), 3);
    }

    #[test]
    fn purge_menghapus_permanen() {
        let db = db_isi();
        let id = db.list(&Filter::default(), SortKey::Mine, 1, 0).unwrap()[0].id;
        db.soft_delete(&[id]).unwrap();
        let dihapus = db.purge_deleted(now_unix()).unwrap();
        assert_eq!(dihapus, 1);
        assert!(db.get(id).unwrap().is_none());
    }

    #[test]
    fn pengaturan_tersimpan_dan_terbaca() {
        let db = db_isi();
        let id = db.list(&Filter::default(), SortKey::Mine, 1, 0).unwrap()[0].id;
        let s = WallpaperSettings {
            fill: crate::model::FillMode::Fit,
            volume: 80,
            ..WallpaperSettings::default()
        };
        db.set_settings(id, &s).unwrap();
        assert_eq!(db.get(id).unwrap().unwrap().settings, s);
    }

    #[test]
    fn folder_dan_jumlah_item() {
        let mut db = Db::open_in_memory().unwrap();
        let folder = db.insert_folder(Path::new("/video"), true).unwrap();
        let mut e = entri("x.mp4", MediaKind::Video);
        e.folder_id = Some(folder);
        db.upsert(&e).unwrap();

        let daftar = db.list_folders().unwrap();
        assert_eq!(daftar.len(), 1);
        assert_eq!(daftar[0].item_count, 1);
        assert!(daftar[0].recursive);
    }

    #[test]
    fn folder_sama_tidak_digandakan() {
        let mut db = Db::open_in_memory().unwrap();
        let a = db.insert_folder(Path::new("/video"), true).unwrap();
        let b = db.insert_folder(Path::new("/video"), false).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn hapus_folder_menyisakan_wallpaper() {
        let mut db = Db::open_in_memory().unwrap();
        let folder = db.insert_folder(Path::new("/video"), true).unwrap();
        let mut e = entri("y.mp4", MediaKind::Video);
        e.folder_id = Some(folder);
        let (id, _) = db.upsert(&e).unwrap();

        db.delete_folder(folder).unwrap();
        let w = db.get(id).unwrap().unwrap();
        assert_eq!(w.folder_id, None, "folder_id harus menjadi NULL");
    }

    #[test]
    fn versi_skema_lebih_baru_ditolak() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        {
            let db = Db::open(&path).unwrap();
            db.conn()
                .execute("UPDATE meta SET value='999' WHERE key='schema_version'", [])
                .unwrap();
        }
        match Db::open(&path) {
            Err(Error::DbNewerSchema(999)) => {}
            lain => panic!("seharusnya menolak skema lebih baru, dapat: {lain:?}"),
        }
    }

    #[test]
    fn berkas_bukan_sqlite_dipulihkan() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        std::fs::write(&path, b"ini bukan basis data").unwrap();

        let (db, cadangan) = Db::open_recovering(&path).unwrap();
        assert!(cadangan.is_some(), "berkas rusak harus dicadangkan");
        assert_eq!(db.count(&Filter::default()).unwrap(), 0);
    }

    #[test]
    fn mark_missing_dan_thumb_path() {
        let db = db_isi();
        let id = db.list(&Filter::default(), SortKey::Mine, 1, 0).unwrap()[0].id;
        db.mark_missing(&[id], true).unwrap();
        assert!(db.get(id).unwrap().unwrap().missing);

        db.set_thumb_path(id, Some(Path::new("/cache/a.jpg")))
            .unwrap();
        assert_eq!(
            db.get(id).unwrap().unwrap().thumb_path,
            Some(PathBuf::from("/cache/a.jpg"))
        );
    }

    #[test]
    fn mark_used_mengubah_urutan_recent() {
        let db = db_isi();
        let id = db.list(&Filter::default(), SortKey::Mine, 1, 0).unwrap()[0].id;
        db.mark_used(id, 2_000_000_000).unwrap();
        let terbaru = db.list(&Filter::default(), SortKey::Recent, 1, 0).unwrap();
        assert_eq!(terbaru[0].id, id);
    }
}
