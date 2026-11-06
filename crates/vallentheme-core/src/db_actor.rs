//! Aktor basis data.
//!
//! Satu thread memiliki `Db`; pemanggil mengirim pekerjaan lewat kanal berbatas
//! dan menunggu hasilnya. Ini mencegah kunci SQLite bertumpuk dan menjaga semua
//! I/O basis data jauh dari main loop GUI.
//! Rujukan: `panduan/01-arsitektur.md` bagian 6.

use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::thread::JoinHandle;

use crate::db::Db;
use crate::error::{Error, Result};

/// Pekerjaan yang dijalankan di thread basis data.
type Kerja = Box<dyn FnOnce(&mut Db) + Send + 'static>;

/// Pegangan ke thread basis data.
pub struct DbActor {
    pengirim: Option<SyncSender<Kerja>>,
    pegangan: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for DbActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DbActor { .. }")
    }
}

impl DbActor {
    /// Jalankan thread basis data dengan antrean berkapasitas `kapasitas`.
    ///
    /// Kapasitas kecil (misalnya 32) sudah cukup: GUI tidak boleh menumpuk
    /// ribuan permintaan sekaligus.
    pub fn spawn(db: Db, kapasitas: usize) -> Result<Self> {
        let (pengirim, penerima): (SyncSender<Kerja>, Receiver<Kerja>) =
            sync_channel(kapasitas.max(1));

        let pegangan = std::thread::Builder::new()
            .name("vallentheme-db".to_string())
            .spawn(move || {
                let mut db = db;
                while let Ok(kerja) = penerima.recv() {
                    kerja(&mut db);
                }
                tracing::debug!("thread basis data berhenti");
            })
            .map_err(|e| Error::Db(format!("gagal membuat thread basis data: {e}")))?;

        Ok(Self {
            pengirim: Some(pengirim),
            pegangan: Some(pegangan),
        })
    }

    /// Jalankan `kerja` di thread basis data dan tunggu hasilnya.
    pub fn call<T, F>(&self, kerja: F) -> Result<T>
    where
        F: FnOnce(&mut Db) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let pengirim = self
            .pengirim
            .as_ref()
            .ok_or_else(|| Error::Db("aktor basis data sudah ditutup".to_string()))?;

        let (balas_tx, balas_rx) = sync_channel(1);

        pengirim
            .send(Box::new(move |db: &mut Db| {
                let hasil = kerja(db);
                // Penerima mungkin sudah pergi; abaikan kegagalan kirim.
                let _ = balas_tx.send(hasil);
            }))
            .map_err(|_| Error::Db("thread basis data tidak menerima pekerjaan".to_string()))?;

        balas_rx
            .recv()
            .map_err(|_| Error::Db("thread basis data berhenti sebelum membalas".to_string()))?
    }

    /// Tutup antrean dan tunggu thread selesai.
    pub fn tutup(&mut self) {
        self.pengirim.take();
        if let Some(pegangan) = self.pegangan.take() {
            let _ = pegangan.join();
        }
    }
}

impl Drop for DbActor {
    fn drop(&mut self) {
        self.tutup();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::model::Filter;

    #[test]
    fn pekerjaan_dijalankan_dan_hasil_dikembalikan() {
        let db = Db::open_in_memory().unwrap();
        let aktor = DbActor::spawn(db, 8).unwrap();

        let jumlah = aktor.call(|db| db.count(&Filter::default())).unwrap();
        assert_eq!(jumlah, 0);
    }

    #[test]
    fn beberapa_pekerjaan_berurutan_aman() {
        let db = Db::open_in_memory().unwrap();
        let aktor = DbActor::spawn(db, 8).unwrap();

        for i in 0..50 {
            let nilai = aktor
                .call(move |db| db.count(&Filter::default()).map(|c| c + i))
                .unwrap();
            assert_eq!(nilai, i);
        }
    }

    #[test]
    fn error_diteruskan_ke_pemanggil() {
        let db = Db::open_in_memory().unwrap();
        let aktor = DbActor::spawn(db, 4).unwrap();

        let hasil: Result<()> = aktor.call(|_db| Err(Error::Db("sengaja gagal".to_string())));
        assert!(hasil.is_err());
    }

    #[test]
    fn setelah_ditutup_panggilan_gagal_dengan_pesan_jelas() {
        let db = Db::open_in_memory().unwrap();
        let mut aktor = DbActor::spawn(db, 4).unwrap();
        aktor.tutup();

        let hasil: Result<u32> = aktor.call(|db| db.count(&Filter::default()));
        assert!(hasil.is_err());
    }
}
