//! Pembacaan metrik sistem tanpa crate berat.
//!
//! Semua pembaca memakai `/proc` dan **tidak pernah** gagal keras: berkas yang
//! tidak ada atau tidak terbaca menghasilkan `None`/`0.0`. Rujukan:
//! `panduan/06-referensi-teknis.md` bagian 8 dan `fase/FASE-7` bagian 7.2.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Jumlah tik pemroses per detik (`sysconf(_SC_CLK_TCK)`), hampir selalu 100.
pub const CLK_TCK: f64 = 100.0;

/// Satu sampel metrik yang dikirim ke kebijakan kinerja.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Laju frame terlewat sebagai pecahan 0.0–1.0 dari frame yang seharusnya tampil.
    pub drop_rate: f64,
    /// Pemakaian CPU total sistem dalam persen (0–100).
    pub cpu_total: f64,
    /// `MemAvailable` dalam MB.
    pub mem_available_mb: u64,
    /// Frame per detik yang diharapkan (`min(fps video, batas)`).
    pub fps_target: f64,
}

impl Metrics {
    /// Susun sampel metrik.
    pub fn new(drop_rate: f64, cpu_total: f64, mem_available_mb: u64, fps_target: f64) -> Self {
        Self {
            drop_rate,
            cpu_total,
            mem_available_mb,
            fps_target,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            drop_rate: 0.0,
            cpu_total: 0.0,
            mem_available_mb: 0,
            fps_target: 0.0,
        }
    }
}

/// Nilai waktu CPU dari baris `cpu` di `/proc/stat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CpuTimes {
    /// Total tik semua kolom.
    pub total: u64,
    /// Total tik diam (`idle` + `iowait`).
    pub idle: u64,
}

/// Baca `MemAvailable` (atau kunci lain) dari isi `/proc/meminfo`, dalam MB.
pub fn parse_meminfo_mb(teks: &str, kunci: &str) -> Option<u64> {
    for baris in teks.lines() {
        let Some((nama, nilai)) = baris.split_once(':') else {
            continue;
        };
        if nama.trim() != kunci {
            continue;
        }
        let angka = nilai.split_whitespace().next()?;
        let kb: u64 = angka.parse().ok()?;
        return Some(kb / 1024);
    }
    None
}

/// Baca `MemAvailable` langsung dari `/proc/meminfo`.
pub fn mem_available_mb() -> Option<u64> {
    let teks = std::fs::read_to_string("/proc/meminfo").ok()?;
    parse_meminfo_mb(&teks, "MemAvailable")
}

/// Urai baris `cpu` pertama dari isi `/proc/stat`.
pub fn parse_proc_stat(teks: &str) -> Option<CpuTimes> {
    let baris = teks.lines().find(|b| b.starts_with("cpu "))?;
    let nilai: Vec<u64> = baris
        .split_whitespace()
        .skip(1)
        .filter_map(|t| t.parse::<u64>().ok())
        .collect();
    let idle = nilai.get(3).copied()?;
    let iowait = nilai.get(4).copied().unwrap_or(0);
    let total: u64 = nilai.iter().take(8).sum();
    Some(CpuTimes {
        total,
        idle: idle.saturating_add(iowait),
    })
}

/// Sampel `/proc/stat` saat ini.
pub fn baca_cpu_times() -> Option<CpuTimes> {
    let teks = std::fs::read_to_string("/proc/stat").ok()?;
    parse_proc_stat(&teks)
}

/// Pemakaian CPU total (persen) dari dua sampel `/proc/stat`.
pub fn cpu_percent(sebelum: CpuTimes, sesudah: CpuTimes) -> Option<f64> {
    let delta_total = sesudah.total.checked_sub(sebelum.total)?;
    if delta_total == 0 {
        return None;
    }
    let delta_idle = sesudah.idle.saturating_sub(sebelum.idle);
    let sibuk = delta_total.saturating_sub(delta_idle);
    let persen = (sibuk as f64) * 100.0 / (delta_total as f64);
    Some(persen.clamp(0.0, 100.0))
}

/// Urai `utime + stime` (tik) dari isi `/proc/<pid>/stat`.
///
/// Kolom `comm` bisa berisi spasi dan tanda kurung, jadi penguraian dimulai
/// dari tanda `)` terakhir.
pub fn parse_proc_pid_ticks(teks: &str) -> Option<u64> {
    let akhir = teks.rfind(')')?;
    let sisa = teks.get(akhir + 1..)?;
    let mut kolom = sisa.split_whitespace();
    let utime: u64 = kolom.nth(11)?.parse().ok()?;
    let stime: u64 = kolom.next()?.parse().ok()?;
    Some(utime.saturating_add(stime))
}

/// Baca `utime + stime` sebuah proses.
pub fn baca_proses_ticks(pid: u32) -> Option<u64> {
    let path = Path::new("/proc").join(pid.to_string()).join("stat");
    let teks = std::fs::read_to_string(path).ok()?;
    parse_proc_pid_ticks(&teks)
}

/// Pemakaian CPU satu proses (persen dari seluruh mesin) dari dua sampel tik.
pub fn proses_cpu_percent(sebelum: u64, sesudah: u64, dt: Duration, cores: u32) -> Option<f64> {
    let detik = dt.as_secs_f64();
    if detik <= 0.0 || cores == 0 {
        return None;
    }
    let delta = sesudah.checked_sub(sebelum)?;
    let persen = (delta as f64) / CLK_TCK / detik / f64::from(cores) * 100.0;
    Some(persen.clamp(0.0, 100.0))
}

/// Laju drop: `delta_drop / (fps_target × dt)`, dibatasi 0.0–1.0.
pub fn drop_rate(delta_drop: u64, fps_target: f64, dt: Duration) -> f64 {
    let frame = fps_target * dt.as_secs_f64();
    if !frame.is_finite() || frame <= 0.0 {
        return 0.0;
    }
    ((delta_drop as f64) / frame).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn meminfo_mem_available_dibaca() {
        let teks = "MemTotal:       16318744 kB\nMemFree:          300000 kB\n\
                    MemAvailable:    8192000 kB\n";
        assert_eq!(parse_meminfo_mb(teks, "MemAvailable"), Some(8000));
        assert_eq!(parse_meminfo_mb(teks, "MemTotal"), Some(15936));
        assert_eq!(parse_meminfo_mb(teks, "TidakAda"), None);
        assert_eq!(parse_meminfo_mb("sampah tanpa titik dua", "MemTotal"), None);
    }

    #[test]
    fn proc_stat_dan_cpu_percent() {
        let awal = parse_proc_stat("cpu  100 0 100 800 0 0 0 0 0 0\ncpu0 1 2 3\n").unwrap();
        assert_eq!(awal.total, 1000);
        assert_eq!(awal.idle, 800);

        let akhir = parse_proc_stat("cpu  150 0 150 900 0 0 0 0 0 0\n").unwrap();
        // delta total 200, delta idle 100 → 50% sibuk.
        let persen = cpu_percent(awal, akhir).unwrap();
        assert!((persen - 50.0).abs() < 0.001, "dapat {persen}");

        assert_eq!(parse_proc_stat("tidak ada baris cpu"), None);
        assert_eq!(cpu_percent(awal, awal), None, "tanpa delta harus None");
    }

    #[test]
    fn proc_pid_stat_tahan_spasi_di_comm() {
        // comm berisi spasi dan tanda kurung.
        let teks = "42 (mpv (uji)) S 1 42 42 0 -1 4194560 100 0 0 0 30 12 0 0 20 0 1 0";
        assert_eq!(parse_proc_pid_ticks(teks), Some(42));
        assert_eq!(parse_proc_pid_ticks("rusak"), None);
        assert_eq!(parse_proc_pid_ticks("42 (mpv) S 1 2"), None);
    }

    #[test]
    fn proses_cpu_percent_memakai_clk_tck_dan_core() {
        // 100 tik dalam 1 detik pada 4 core = 25%.
        let persen = proses_cpu_percent(0, 100, Duration::from_secs(1), 4).unwrap();
        assert!((persen - 25.0).abs() < 0.001, "dapat {persen}");
        assert_eq!(proses_cpu_percent(10, 5, Duration::from_secs(1), 4), None);
        assert_eq!(proses_cpu_percent(0, 10, Duration::from_secs(0), 4), None);
        assert_eq!(proses_cpu_percent(0, 10, Duration::from_secs(1), 0), None);
    }

    #[test]
    fn drop_rate_dihitung_dari_frame_harapan() {
        // 6 drop dari 60 fps × 2 detik = 120 frame → 5%.
        let laju = drop_rate(6, 60.0, Duration::from_secs(2));
        assert!((laju - 0.05).abs() < 1e-9, "dapat {laju}");
        assert_eq!(drop_rate(10, 0.0, Duration::from_secs(2)), 0.0);
        assert_eq!(drop_rate(10, 60.0, Duration::from_secs(0)), 0.0);
        assert_eq!(drop_rate(1000, 1.0, Duration::from_secs(1)), 1.0);
    }

    #[test]
    fn pembaca_sistem_tidak_panic() {
        // Tidak ada jaminan nilai tertentu; yang penting tidak gagal keras.
        let _ = mem_available_mb();
        let _ = baca_cpu_times();
        let _ = baca_proses_ticks(u32::MAX);
    }
}
