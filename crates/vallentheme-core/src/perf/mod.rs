//! Sistem kinerja: profil perangkat, metrik `/proc`, dan kebijakan level L0–L4.
//!
//! Semua bagian di sini **murni** atau hanya membaca berkas sistem; tidak ada
//! GUI, daemon, mpv, atau jaringan. Rujukan: `fase/FASE-7-sistem-kinerja.md`.

pub mod metrics;
pub mod policy;
pub mod profile;

pub use metrics::{
    baca_cpu_times, baca_proses_ticks, cpu_percent, drop_rate, mem_available_mb, parse_meminfo_mb,
    parse_proc_pid_ticks, parse_proc_stat, proses_cpu_percent, CpuTimes, Metrics, CLK_TCK,
};
pub use policy::{decide, Decision, Level, OscillationGuard, PerfConfig, PerfState, ReasonCode};
pub use profile::{detect, detect_in, DeviceProfile, SessionType};
