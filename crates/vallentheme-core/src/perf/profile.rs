//! Deteksi profil perangkat sekali saat start.
//!
//! Membaca `/proc` dan `/sys` secara toleran: berkas yang tidak ada atau tidak
//! terbaca menghasilkan `None`, bukan kesalahan. Hasilnya diserialkan ke JSON
//! untuk `GetDeviceProfile` dan dialog diagnostik. Rujukan: `fase/FASE-7`
//! bagian 7.1, `panduan/06` bagian 8.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::perf::metrics::parse_meminfo_mb;

/// Jenis sesi grafis yang sedang berjalan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionType {
    /// Sesi X11.
    X11,
    /// Sesi Wayland.
    Wayland,
    /// Tidak diketahui.
    #[default]
    Unknown,
}

impl SessionType {
    /// Terjemahkan nilai `XDG_SESSION_TYPE`.
    pub fn from_env(nilai: Option<&str>) -> Self {
        let bersih = nilai.map(str::trim).unwrap_or("");
        if bersih.eq_ignore_ascii_case("x11") {
            Self::X11
        } else if bersih.eq_ignore_ascii_case("wayland") {
            Self::Wayland
        } else {
            Self::Unknown
        }
    }

    /// Nilai yang dipakai di JSON dan properti D-Bus.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X11 => "x11",
            Self::Wayland => "wayland",
            Self::Unknown => "unknown",
        }
    }
}

/// Ringkasan perangkat yang dipakai untuk memilih level kinerja.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeviceProfile {
    /// Jumlah core CPU menurut `/proc/cpuinfo`.
    pub cpu_cores: Option<u32>,
    /// RAM total dalam MB menurut `/proc/meminfo`.
    pub ram_total_mb: Option<u64>,
    /// Nama model CPU menurut `/proc/cpuinfo`.
    pub cpu_model: Option<String>,
    /// Ada baterai (`/sys/class/power_supply/BAT*`).
    pub has_battery: bool,
    /// Sesi grafis menurut `XDG_SESSION_TYPE`.
    pub session: SessionType,
    /// Nama kandidat hwdec yang terbukti bekerja (mis. `vaapi`, `nvdec`).
    pub hwdec_working: Vec<String>,
}

impl DeviceProfile {
    /// Serialkan profil ke JSON satu baris.
    pub fn to_json(&self) -> crate::Result<String> {
        serde_json::to_string(self).map_err(|e| {
            crate::Error::ConfigSave(format!("profil perangkat tidak bisa disimpan: {e}"))
        })
    }

    /// Label ringkas untuk log.
    pub fn ringkas(&self) -> String {
        format!(
            "{} core, RAM {} MB, baterai {}, sesi {}",
            self.cpu_cores
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".to_string()),
            self.ram_total_mb
                .map(|n| n.to_string())
                .unwrap_or_else(|| "?".to_string()),
            if self.has_battery { "ada" } else { "tidak" },
            self.session.as_str()
        )
    }
}

/// Deteksi profil perangkat mesin ini.
pub fn detect() -> DeviceProfile {
    let sesi = std::env::var("XDG_SESSION_TYPE").ok();
    detect_in(Path::new("/proc"), Path::new("/sys"), sesi.as_deref())
}

/// Deteksi profil dari akar `/proc` dan `/sys` yang bisa diganti (untuk uji).
pub fn detect_in(proc_root: &Path, sys_root: &Path, session: Option<&str>) -> DeviceProfile {
    let cpuinfo = std::fs::read_to_string(proc_root.join("cpuinfo")).ok();
    let (cpu_model, cpu_cores) = cpuinfo
        .as_deref()
        .map(parse_cpuinfo)
        .unwrap_or((None, None));

    let ram_total_mb = std::fs::read_to_string(proc_root.join("meminfo"))
        .ok()
        .and_then(|teks| parse_meminfo_mb(&teks, "MemTotal"));

    DeviceProfile {
        cpu_cores,
        ram_total_mb,
        cpu_model,
        has_battery: ada_baterai(sys_root),
        session: SessionType::from_env(session),
        hwdec_working: Vec::new(),
    }
}

/// Urai `model name` dan jumlah core dari isi `/proc/cpuinfo`.
pub fn parse_cpuinfo(teks: &str) -> (Option<String>, Option<u32>) {
    let mut model: Option<String> = None;
    let mut core: u32 = 0;
    for baris in teks.lines() {
        let Some((kunci, nilai)) = baris.split_once(':') else {
            continue;
        };
        let kunci = kunci.trim();
        let nilai = nilai.trim();
        if kunci == "processor" {
            core = core.saturating_add(1);
        } else if model.is_none()
            && (kunci == "model name" || kunci == "Model")
            && !nilai.is_empty()
        {
            model = Some(nilai.to_string());
        }
    }
    (model, (core > 0).then_some(core))
}

/// Apakah mesin punya baterai (`BAT*` di `/sys/class/power_supply`).
fn ada_baterai(sys_root: &Path) -> bool {
    let Ok(entri) = std::fs::read_dir(sys_root.join("class/power_supply")) else {
        return false;
    };
    entri.flatten().any(|e| {
        e.file_name()
            .to_str()
            .map(|n| n.starts_with("BAT"))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn tulis(path: &Path, isi: &str) {
        if let Some(induk) = path.parent() {
            std::fs::create_dir_all(induk).unwrap();
        }
        std::fs::write(path, isi).unwrap();
    }

    #[test]
    fn cpuinfo_model_dan_core_dibaca() {
        let teks = "processor\t: 0\nmodel name\t: AMD Ryzen 5 3500U\n\n\
                    processor\t: 1\nmodel name\t: AMD Ryzen 5 3500U\n";
        let (model, core) = parse_cpuinfo(teks);
        assert_eq!(model.as_deref(), Some("AMD Ryzen 5 3500U"));
        assert_eq!(core, Some(2));

        assert_eq!(parse_cpuinfo(""), (None, None));
        assert_eq!(parse_cpuinfo("sampah"), (None, None));
    }

    #[test]
    fn profil_lengkap_dari_pohon_palsu() {
        let dir = tempfile::tempdir().unwrap();
        let proc_root = dir.path().join("proc");
        let sys_root = dir.path().join("sys");

        tulis(
            &proc_root.join("cpuinfo"),
            "processor\t: 0\nmodel name\t: Intel Core i5\nprocessor\t: 1\n",
        );
        tulis(
            &proc_root.join("meminfo"),
            "MemTotal:        8192000 kB\nMemAvailable:    4096000 kB\n",
        );
        tulis(&sys_root.join("class/power_supply/BAT0/capacity"), "77\n");
        tulis(&sys_root.join("class/power_supply/AC/online"), "1\n");

        let profil = detect_in(&proc_root, &sys_root, Some("wayland"));
        assert_eq!(profil.cpu_cores, Some(2));
        assert_eq!(profil.ram_total_mb, Some(8000));
        assert_eq!(profil.cpu_model.as_deref(), Some("Intel Core i5"));
        assert!(profil.has_battery);
        assert_eq!(profil.session, SessionType::Wayland);
        assert!(profil.hwdec_working.is_empty());

        let json = profil.to_json().unwrap();
        let balik: DeviceProfile = serde_json::from_str(&json).unwrap();
        assert_eq!(balik, profil);
    }

    #[test]
    fn berkas_hilang_tidak_error() {
        let dir = tempfile::tempdir().unwrap();
        let kosong = dir.path().join("kosong");
        let profil = detect_in(&kosong, &kosong, None);
        assert_eq!(profil, DeviceProfile::default());
        assert!(!profil.has_battery);
        assert_eq!(profil.session, SessionType::Unknown);
        assert!(profil.ringkas().contains("core"));
    }

    #[test]
    fn sesi_dari_env() {
        assert_eq!(SessionType::from_env(Some("x11")), SessionType::X11);
        assert_eq!(
            SessionType::from_env(Some(" WAYLAND ")),
            SessionType::Wayland
        );
        assert_eq!(SessionType::from_env(Some("tty")), SessionType::Unknown);
        assert_eq!(SessionType::from_env(None), SessionType::Unknown);
    }

    #[test]
    fn deteksi_mesin_nyata_tidak_panic() {
        let profil = detect();
        assert!(profil.cpu_cores.is_none() || profil.cpu_cores.unwrap() > 0);
    }
}
