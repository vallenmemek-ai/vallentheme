//! State daemon yang disimpan di `daemon-state.json`.
//!
//! Berkas ini memuat `ApplyRequest` terakhir per monitor dan nilai desktop yang
//! harus dipulihkan saat wallpaper dihentikan. Penyimpanan bersifat atomik dan
//! toleran: berkas rusak dicadangkan lalu state dimulai bersih.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use vallentheme_core::model::{MediaKind, WallpaperSettings};
use vallentheme_render::setter::RestoreRecord;

/// Versi skema `daemon-state.json`.
pub const STATE_VERSION: u32 = 1;

/// Nama monitor untuk permintaan yang berlaku ke semua monitor.
pub const SEMUA_MONITOR: &str = "all";

/// Permintaan penerapan wallpaper dari GUI atau `--restore`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApplyRequest {
    /// Versi skema payload.
    #[serde(default = "versi_bawaan")]
    pub v: u32,
    /// ID baris library; boleh kosong bila dipulihkan tanpa DB.
    #[serde(default)]
    pub wallpaper_id: Option<i64>,
    /// Path media yang sudah divalidasi pemanggil.
    pub path: PathBuf,
    /// Jenis media: `image` atau `video`.
    pub kind: MediaKind,
    /// Nama monitor atau `all`.
    #[serde(default = "monitor_bawaan")]
    pub monitor: String,
    /// Pengaturan tampilan dan audio.
    #[serde(default)]
    pub settings: WallpaperSettings,
}

fn versi_bawaan() -> u32 {
    STATE_VERSION
}

fn monitor_bawaan() -> String {
    SEMUA_MONITOR.to_string()
}

impl ApplyRequest {
    /// Kunci monitor yang dipakai di peta state.
    pub fn kunci_monitor(&self) -> String {
        let nama = self.monitor.trim();
        if nama.is_empty() {
            SEMUA_MONITOR.to_string()
        } else {
            nama.to_string()
        }
    }

    /// Periksa rentang pengaturan tanpa mengubah nilai.
    ///
    /// Mengembalikan nama field yang tidak sah agar pemanggil bisa membuat
    /// pesan error berkode.
    pub fn periksa_pengaturan(&self) -> Result<(), String> {
        let s = &self.settings;
        if !matches!(s.rotation, 0 | 90 | 180 | 270) {
            return Err(format!("rotasi {} tidak sah", s.rotation));
        }
        if !s.zoom.is_finite() || !(0.0..=vallentheme_core::model::ZOOM_MAX).contains(&s.zoom) {
            return Err(format!("zoom {} di luar rentang 0–2", s.zoom));
        }
        let batas = vallentheme_core::model::PAN_MAX;
        if !s.pan_x.is_finite() || !(-batas..=batas).contains(&s.pan_x) {
            return Err(format!("geser horizontal {} di luar rentang", s.pan_x));
        }
        if !s.pan_y.is_finite() || !(-batas..=batas).contains(&s.pan_y) {
            return Err(format!("geser vertikal {} di luar rentang", s.pan_y));
        }
        if s.volume > 100 {
            return Err(format!("volume {} di luar rentang 0–100", s.volume));
        }
        Ok(())
    }
}

/// State satu monitor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorEntry {
    /// Permintaan terakhir untuk monitor ini.
    pub request: ApplyRequest,
    /// Nilai desktop yang harus dikembalikan saat berhenti.
    #[serde(default)]
    pub restore_on_stop: Vec<RestoreRecord>,
    /// Alasan jeda bila sedang dijeda.
    #[serde(default)]
    pub paused_reason: Option<String>,
    /// Waktu perubahan terakhir (detik Unix).
    #[serde(default)]
    pub updated_at: i64,
}

/// Seluruh isi `daemon-state.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonState {
    /// Versi skema berkas.
    #[serde(default = "versi_bawaan")]
    pub v: u32,
    /// State per monitor.
    #[serde(default)]
    pub monitors: BTreeMap<String, MonitorEntry>,
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            v: STATE_VERSION,
            monitors: BTreeMap::new(),
        }
    }
}

impl DaemonState {
    /// Muat state dari `path`.
    ///
    /// - Berkas tidak ada → state kosong.
    /// - Berkas rusak → dicadangkan ke `daemon-state.json.rusak-<waktu>` lalu
    ///   state kosong dipakai. Tidak pernah panic.
    pub fn load(path: &Path) -> Self {
        let teks = match fs::read_to_string(path) {
            Ok(teks) => teks,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(err) => {
                tracing::warn!(error = %err, "state daemon tidak bisa dibaca; mulai bersih");
                return Self::default();
            }
        };

        match serde_json::from_str::<Self>(&teks) {
            Ok(state) => state,
            Err(err) => {
                let cadangan = path_cadangan(path);
                if let Err(salin) = fs::copy(path, &cadangan) {
                    tracing::warn!(
                        error = %salin,
                        "state daemon rusak dan cadangan gagal dibuat"
                    );
                } else {
                    tracing::warn!(
                        backup = %cadangan.display(),
                        "state daemon rusak; cadangan dibuat dan state dimulai bersih"
                    );
                }
                tracing::warn!(error = %err, "isi state daemon tidak valid");
                Self::default()
            }
        }
    }

    /// Simpan state secara atomik (`*.tmp` → `fsync` → `rename`).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let teks = serde_json::to_string_pretty(self)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

        let tmp = tmp_path(path);
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(teks.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&tmp, path)
    }

    /// Catat permintaan terakhir untuk satu monitor.
    pub fn set_request(&mut self, request: ApplyRequest) {
        let kunci = request.kunci_monitor();
        let waktu = sekarang_unix();
        let entri = self.monitors.entry(kunci).or_insert_with(|| MonitorEntry {
            request: request.clone(),
            restore_on_stop: Vec::new(),
            paused_reason: None,
            updated_at: waktu,
        });
        entri.request = request;
        entri.updated_at = waktu;
        self.v = STATE_VERSION;
    }

    /// Simpan nilai desktop yang perlu dipulihkan untuk satu monitor.
    pub fn set_restore(&mut self, monitor: &str, records: Vec<RestoreRecord>) {
        if let Some(entri) = self.monitors.get_mut(monitor) {
            entri.restore_on_stop = records;
        }
    }

    /// Hapus state satu monitor; `true` bila ada yang dihapus.
    pub fn remove(&mut self, monitor: &str) -> bool {
        self.monitors.remove(monitor).is_some()
    }

    /// Semua nama monitor yang tersimpan.
    pub fn nama_monitor(&self) -> Vec<String> {
        self.monitors.keys().cloned().collect()
    }
}

/// Nama berkas cadangan dengan cap waktu.
fn path_cadangan(path: &Path) -> PathBuf {
    let stamp = sekarang_unix();
    let nama = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "daemon-state.json".to_string());
    path.with_file_name(format!("{nama}.rusak-{stamp}"))
}

/// Path sementara di folder yang sama agar `rename` tetap atomik.
fn tmp_path(path: &Path) -> PathBuf {
    let nama = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "daemon-state.json".to_string());
    path.with_file_name(format!("{nama}.tmp"))
}

/// Detik Unix saat ini; `0` bila jam sistem tidak masuk akal.
fn sekarang_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn tmp() -> PathBuf {
        static NOMOR: AtomicU64 = AtomicU64::new(0);
        let nomor = NOMOR.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "vallentheme-daemon-state-{}-{nomor}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn contoh() -> ApplyRequest {
        ApplyRequest {
            v: 1,
            wallpaper_id: Some(42),
            path: PathBuf::from("/home/pengguna/Video/raiden.mp4"),
            kind: MediaKind::Video,
            monitor: "HDMI-1".to_string(),
            settings: WallpaperSettings {
                fill: vallentheme_core::model::FillMode::Fit,
                rotation: 180,
                zoom: 0.5,
                pan_x: 0.1,
                pan_y: -0.1,
                mute: false,
                volume: 75,
                ..WallpaperSettings::default()
            },
        }
    }

    #[test]
    fn simpan_muat_bolak_balik() {
        let dir = tmp();
        let path = dir.join("sub/daemon-state.json");

        let mut state = DaemonState::default();
        state.set_request(contoh());
        state.set_restore(
            "HDMI-1",
            vec![RestoreRecord {
                program: "gsettings".to_string(),
                args: vec!["set".to_string(), "org.mate.background".to_string()],
            }],
        );
        state.save(&path).unwrap();

        let muat = DaemonState::load(&path);
        assert_eq!(muat, state);
        assert_eq!(
            muat.monitors.get("HDMI-1").unwrap().request.wallpaper_id,
            Some(42)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn berkas_hilang_mulai_bersih() {
        let dir = tmp();
        let state = DaemonState::load(&dir.join("tidak-ada.json"));
        assert!(state.monitors.is_empty());
        assert_eq!(state.v, STATE_VERSION);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn berkas_rusak_dicadangkan() {
        let dir = tmp();
        let path = dir.join("daemon-state.json");
        fs::write(&path, "{ini bukan json").unwrap();

        let state = DaemonState::load(&path);
        assert!(state.monitors.is_empty());

        let cadangan: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".rusak-"))
            .collect();
        assert_eq!(cadangan.len(), 1, "cadangan harus dibuat: {cadangan:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tidak_ada_berkas_tmp_tersisa() {
        let dir = tmp();
        let path = dir.join("daemon-state.json");
        DaemonState::default().save(&path).unwrap();

        let sisa: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(sisa.is_empty(), "berkas sementara tertinggal: {sisa:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn monitor_kosong_menjadi_all() {
        let mut req = contoh();
        req.monitor = "  ".to_string();
        assert_eq!(req.kunci_monitor(), SEMUA_MONITOR);
    }

    #[test]
    fn validasi_rentang_pengaturan() {
        assert!(contoh().periksa_pengaturan().is_ok());

        let mut req = contoh();
        req.settings.zoom = 9.0;
        assert!(req.periksa_pengaturan().is_err());

        let mut req = contoh();
        req.settings.rotation = 45;
        assert!(req.periksa_pengaturan().is_err());

        let mut req = contoh();
        req.settings.volume = 101;
        assert!(req.periksa_pengaturan().is_err());
    }

    #[test]
    fn hapus_monitor() {
        let mut state = DaemonState::default();
        state.set_request(contoh());
        assert!(state.remove("HDMI-1"));
        assert!(!state.remove("HDMI-1"));
        assert!(state.monitors.is_empty());
    }
}
