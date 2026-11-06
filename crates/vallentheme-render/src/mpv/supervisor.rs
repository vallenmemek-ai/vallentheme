//! Supervisor proses mpv.
//!
//! Menjalankan mpv sebagai anak yang ikut mati bersama daemon, memeriksa
//! kesehatan lewat IPC setiap 5 detik, dan memulai ulang dengan backoff
//! 1, 2, 4, 8, 16, lalu 30 detik. Lima kegagalan dalam dua menit membuat
//! supervisor menyerah dan masuk keadaan [`SupervisorState::Gagal`].
//!
//! Rujukan: `fase/FASE-5-daemon-produksi.md` bagian 5.2.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::Child;
use std::time::{Duration, Instant};

use crate::error::{RenderError, Result};
use crate::process::{spawn_detached, terminate_group};

use super::args::build_args;
use super::ipc::{Ipc, BATAS_WAKTU};
use super::PlaySpec;

/// Jarak antar pemeriksaan kesehatan.
pub const INTERVAL_KESEHATAN: Duration = Duration::from_secs(5);

/// Jumlah kegagalan kesehatan berturut-turut sebelum mpv dianggap hang.
pub const MAKS_GAGAL_BERTURUT: u32 = 3;

/// Jumlah kegagalan total yang memicu penyerahan.
pub const MAKS_KEGAGALAN: usize = 5;

/// Jendela waktu untuk menghitung kegagalan beruntun.
pub const JENDELA_KEGAGALAN: Duration = Duration::from_secs(120);

/// Keadaan supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorState {
    /// Belum berjalan atau sudah dihentikan.
    Berhenti,
    /// mpv hidup dan lolos pemeriksaan terakhir.
    Berjalan,
    /// Terlalu banyak kegagalan; supervisor berhenti mencoba.
    Gagal,
}

/// Peristiwa yang dilaporkan [`MpvSupervisor::tick`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorEvent {
    /// Tidak ada yang perlu dilaporkan.
    TidakAda,
    /// mpv berhasil dijalankan ulang.
    Pulih,
    /// mpv hang dan sudah dimatikan; restart dijadwalkan.
    Hang,
    /// Supervisor menyerah dan berhenti mencoba.
    Gagal(String),
}

/// Hitung jeda backoff untuk kegagalan ke-`n` (dimulai dari 1).
pub fn backoff_ke(n: usize) -> Duration {
    match n {
        0 | 1 => Duration::from_secs(1),
        2 => Duration::from_secs(2),
        3 => Duration::from_secs(4),
        4 => Duration::from_secs(8),
        5 => Duration::from_secs(16),
        _ => Duration::from_secs(30),
    }
}

/// Apakah supervisor harus menyerah.
///
/// Syaratnya minimal lima kegagalan yang semuanya terjadi dalam rentang dua
/// menit. `rentang` adalah selisih waktu antara kegagalan pertama dan terakhir.
pub fn harus_menyerah(jumlah_kegagalan: usize, rentang: Duration) -> bool {
    jumlah_kegagalan >= MAKS_KEGAGALAN && rentang <= JENDELA_KEGAGALAN
}

/// Pengawas satu proses mpv.
pub struct MpvSupervisor {
    spec: PlaySpec,
    child: Option<Child>,
    ipc: Option<Ipc>,
    state: SupervisorState,
    gagal_berturut: u32,
    kegagalan: VecDeque<Instant>,
    next_retry: Option<Instant>,
}

impl MpvSupervisor {
    /// Buat supervisor untuk satu spesifikasi; mpv belum dijalankan.
    pub fn new(spec: PlaySpec) -> Self {
        Self {
            spec,
            child: None,
            ipc: None,
            state: SupervisorState::Berhenti,
            gagal_berturut: 0,
            kegagalan: VecDeque::new(),
            next_retry: None,
        }
    }

    /// Keadaan supervisor saat ini.
    pub fn state(&self) -> SupervisorState {
        self.state
    }

    /// Spesifikasi yang diawasi.
    pub fn spec(&self) -> &PlaySpec {
        &self.spec
    }

    /// Jalankan mpv dan hubungkan IPC. Kegagalan dicatat untuk backoff.
    pub fn mulai(&mut self) -> Result<()> {
        match self.spawn_proses() {
            Ok(()) => {
                self.state = SupervisorState::Berjalan;
                self.gagal_berturut = 0;
                self.next_retry = None;
                Ok(())
            }
            Err(err) => {
                self.catat_kegagalan(Instant::now());
                Err(err)
            }
        }
    }

    /// Periksa kesehatan; tiga kegagalan berturut mematikan mpv.
    pub fn periksa_kesehatan(&mut self) -> bool {
        let Some(ipc) = self.ipc.as_mut() else {
            self.gagal_berturut = self.gagal_berturut.saturating_add(1);
            return self.gagal_berturut < MAKS_GAGAL_BERTURUT;
        };

        match ipc.get_property("time-pos") {
            Ok(_) => {
                self.gagal_berturut = 0;
                true
            }
            Err(err) => {
                self.gagal_berturut = self.gagal_berturut.saturating_add(1);
                tracing::warn!(
                    kode = err.code(),
                    gagal = self.gagal_berturut,
                    "pemeriksaan kesehatan mpv gagal"
                );
                if self.gagal_berturut >= MAKS_GAGAL_BERTURUT {
                    tracing::error!("mpv tidak merespons tiga kali; menghentikan proses");
                    self.bunuh();
                    false
                } else {
                    true
                }
            }
        }
    }

    /// Hentikan mpv dengan sopan, lalu paksa bila perlu.
    pub fn hentikan(&mut self) {
        if let Some(ipc) = self.ipc.as_mut() {
            let _ = ipc.kirim_tanpa_balas(serde_json::json!(["quit"]));
        }
        self.bunuh();
        self.state = SupervisorState::Berhenti;
        self.next_retry = None;
    }

    /// Jeda pemutaran tanpa mematikan mpv.
    pub fn jeda(&mut self) -> Result<()> {
        match self.ipc.as_mut() {
            Some(ipc) => ipc.set_property("pause", serde_json::Value::Bool(true)),
            None => Err(RenderError::IpcDisconnected),
        }
    }

    /// Lanjutkan pemutaran setelah jeda.
    pub fn lanjutkan(&mut self) -> Result<()> {
        match self.ipc.as_mut() {
            Some(ipc) => ipc.set_property("pause", serde_json::Value::Bool(false)),
            None => Err(RenderError::IpcDisconnected),
        }
    }

    /// Jalankan satu siklus pengawasan pada waktu `now`.
    pub fn tick(&mut self, now: Instant) -> SupervisorEvent {
        match self.state {
            SupervisorState::Gagal => SupervisorEvent::Gagal("mpv gagal berulang kali".to_string()),
            SupervisorState::Berjalan => self.tick_berjalan(now),
            SupervisorState::Berhenti => self.tick_berhenti(now),
        }
    }

    fn tick_berjalan(&mut self, now: Instant) -> SupervisorEvent {
        let sudah_keluar = self
            .child
            .as_mut()
            .and_then(|child| child.try_wait().ok().flatten())
            .is_some();

        if sudah_keluar {
            tracing::warn!("proses mpv berhenti tak terduga");
            self.child = None;
            self.ipc = None;
            self.catat_kegagalan(now);
            return self.event_setelah_kegagalan();
        }

        if self.periksa_kesehatan() {
            return SupervisorEvent::TidakAda;
        }

        self.catat_kegagalan(now);
        if self.state == SupervisorState::Gagal {
            self.event_setelah_kegagalan()
        } else {
            SupervisorEvent::Hang
        }
    }

    fn tick_berhenti(&mut self, now: Instant) -> SupervisorEvent {
        let Some(waktu) = self.next_retry else {
            return SupervisorEvent::TidakAda;
        };
        if now < waktu {
            return SupervisorEvent::TidakAda;
        }

        match self.spawn_proses() {
            Ok(()) => {
                self.state = SupervisorState::Berjalan;
                self.gagal_berturut = 0;
                self.next_retry = None;
                tracing::info!("mpv berhasil dijalankan ulang");
                SupervisorEvent::Pulih
            }
            Err(err) => {
                tracing::warn!(kode = err.code(), "percobaan menjalankan ulang mpv gagal");
                self.catat_kegagalan(now);
                self.event_setelah_kegagalan()
            }
        }
    }

    fn event_setelah_kegagalan(&self) -> SupervisorEvent {
        if self.state == SupervisorState::Gagal {
            SupervisorEvent::Gagal("mpv gagal lima kali dalam dua menit".to_string())
        } else {
            SupervisorEvent::TidakAda
        }
    }

    /// Catat kegagalan dan jadwalkan percobaan berikutnya.
    fn catat_kegagalan(&mut self, now: Instant) {
        self.kegagalan.push_back(now);
        if let Some(batas) = now.checked_sub(JENDELA_KEGAGALAN) {
            while self.kegagalan.front().is_some_and(|waktu| *waktu < batas) {
                self.kegagalan.pop_front();
            }
        }

        let rentang = self
            .kegagalan
            .front()
            .map(|awal| now.saturating_duration_since(*awal))
            .unwrap_or_default();

        if harus_menyerah(self.kegagalan.len(), rentang) {
            tracing::error!(
                jumlah = self.kegagalan.len(),
                "terlalu banyak kegagalan mpv; supervisor berhenti mencoba"
            );
            self.bunuh();
            self.state = SupervisorState::Gagal;
            self.next_retry = None;
        } else {
            self.state = SupervisorState::Berhenti;
            self.next_retry = Some(now + backoff_ke(self.kegagalan.len()));
        }
    }

    fn spawn_proses(&mut self) -> Result<()> {
        if let Some(parent) = self.spec.socket.parent() {
            std::fs::create_dir_all(parent).map_err(|err| RenderError::Ipc(err.to_string()))?;
        }
        let _ = std::fs::remove_file(&self.spec.socket);

        let args = build_args(&self.spec);
        let mut child = spawn_detached("mpv", &args)?;

        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                let pembaca = BufReader::new(stderr);
                for baris in pembaca.lines().map_while(std::result::Result::ok) {
                    tracing::debug!(target: "mpv", "{baris}");
                }
            });
        }

        match Ipc::connect(&self.spec.socket, BATAS_WAKTU) {
            Ok(ipc) => {
                let pid = child.id();
                self.ipc = Some(ipc);
                self.child = Some(child);
                tracing::info!(pid, "mpv berjalan");
                Ok(())
            }
            Err(err) => {
                let _ = terminate_group(&mut child, Duration::from_secs(1));
                Err(err)
            }
        }
    }

    /// Matikan proses dan lepaskan soket.
    fn bunuh(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = terminate_group(&mut child, Duration::from_secs(2));
        }
        self.ipc = None;
        self.gagal_berturut = 0;
    }
}

impl Drop for MpvSupervisor {
    fn drop(&mut self) {
        self.bunuh();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn backoff_mengikuti_urutan_yang_disepakati() {
        let detik: Vec<u64> = (1..=8).map(|n| backoff_ke(n).as_secs()).collect();
        assert_eq!(detik, vec![1, 2, 4, 8, 16, 30, 30, 30]);
        assert_eq!(backoff_ke(0).as_secs(), 1);
    }

    #[test]
    fn menyerah_setelah_lima_kegagalan_dalam_dua_menit() {
        assert!(harus_menyerah(5, Duration::from_secs(1)));
        assert!(harus_menyerah(5, Duration::from_secs(120)));
        assert!(harus_menyerah(9, Duration::from_secs(30)));
    }

    #[test]
    fn tidak_menyerah_bila_kegagalan_kurang_atau_terlalu_lama() {
        assert!(!harus_menyerah(4, Duration::from_secs(1)));
        assert!(!harus_menyerah(5, Duration::from_secs(121)));
        assert!(!harus_menyerah(0, Duration::ZERO));
    }

    #[test]
    fn keadaan_awal_berhenti() {
        let spec = PlaySpec::new(
            1,
            std::path::PathBuf::from("/tmp/tidak-ada.sock"),
            std::path::PathBuf::from("/tmp/tidak-ada.mp4"),
            vallentheme_core::model::WallpaperSettings::default(),
        );
        let mut sup = MpvSupervisor::new(spec);
        assert_eq!(sup.state(), SupervisorState::Berhenti);
        assert_eq!(
            sup.tick(Instant::now()),
            SupervisorEvent::TidakAda,
            "belum ada jadwal restart"
        );
    }
}
