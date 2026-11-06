//! Kebijakan kinerja: **fungsi murni** yang memutuskan level L0–L4.
//!
//! Aturan lengkap ada di `fase/FASE-7-sistem-kinerja.md` bagian 7.3. Modul ini
//! tidak menyentuh mpv, berkas, atau waktu nyata: semua masukan diberikan
//! pemanggil (`now: Duration` adalah waktu sejak daemon start), sehingga
//! seluruh perilaku bisa diuji dengan deret metrik sintetis.
//!
//! Alur pakai di daemon:
//! ```text
//! let mut state = PerfState::new(Level::L0, now);
//! let keputusan = state.step(&metrics, &config, now);   // observe + decide + terapkan
//! ```

use std::fmt;
use std::time::Duration;

use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use crate::config::PerfMode;
use crate::perf::metrics::Metrics;

/// Satu level adaptasi kinerja (L0 asli sampai L4 jeda).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Level(u8);

impl Level {
    /// Kualitas asli, tanpa batas.
    pub const L0: Self = Self(0);
    /// Batas fps (30).
    pub const L1: Self = Self(1);
    /// Hwdec aman + 24 fps.
    pub const L2: Self = Self(2);
    /// Turunkan resolusi render.
    pub const L3: Self = Self(3);
    /// Jeda (bingkai terakhir tampil).
    pub const L4: Self = Self(4);
    /// Semua level, dari ringan ke berat.
    pub const SEMUA: [Self; 5] = [Self::L0, Self::L1, Self::L2, Self::L3, Self::L4];

    /// Nilai numerik 0–4.
    pub fn as_u8(self) -> u8 {
        self.0
    }

    /// Ubah angka menjadi level; `None` bila di luar 0–4.
    pub fn from_u8(nilai: u8) -> Option<Self> {
        (nilai <= 4).then_some(Self(nilai))
    }

    /// Satu langkah **turun kualitas** (L0 → L1 → … → L4), bila belum L4.
    pub fn turun(self) -> Option<Self> {
        Self::from_u8(self.0 + 1)
    }

    /// Satu langkah **pulih** ke kualitas lebih baik (L4 → … → L0), bila belum L0.
    pub fn naik(self) -> Option<Self> {
        self.0.checked_sub(1).map(Self)
    }

    /// Level paling ringan di antara dua nilai.
    pub fn min(self, lain: Self) -> Self {
        if self <= lain {
            self
        } else {
            lain
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "L{}", self.0)
    }
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let nilai = u8::deserialize(d)?;
        Self::from_u8(nilai).ok_or_else(|| de::Error::custom("level kinerja harus 0..=4"))
    }
}

/// Alasan sebuah keputusan diambil; dipakai untuk log dan pesan UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    /// Tidak ada perubahan; kondisi belum memenuhi ambang.
    Hold,
    /// Mode Kualitas asli mengunci level di L0.
    ModeOriginal,
    /// Mode Kustom memakai nilai pengguna tanpa adaptasi.
    ModeCustom,
    /// Level diturunkan agar sesuai plafon mode.
    ModeCeiling,
    /// Antiosilasi: level ditahan setelah naik-turun berulang.
    AntiOscillation,
    /// Laju drop frame terlalu tinggi.
    DropRate,
    /// CPU total terlalu tinggi.
    CpuHigh,
    /// Memori tersedia terlalu sedikit.
    MemLow,
    /// Kondisi stabil cukup lama sehingga level dinaikkan.
    Stable,
}

impl ReasonCode {
    /// Kode stabil untuk log dan test.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::ModeOriginal => "mode_original",
            Self::ModeCustom => "mode_custom",
            Self::ModeCeiling => "mode_ceiling",
            Self::AntiOscillation => "anti_oscillation",
            Self::DropRate => "drop_rate",
            Self::CpuHigh => "cpu_high",
            Self::MemLow => "mem_low",
            Self::Stable => "stable",
        }
    }

    /// ID pesan untuk diterjemahkan lapisan UI.
    pub fn message_id(self) -> &'static str {
        match self {
            Self::Hold => "perf.reason.hold",
            Self::ModeOriginal => "perf.reason.mode_original",
            Self::ModeCustom => "perf.reason.mode_custom",
            Self::ModeCeiling => "perf.reason.mode_ceiling",
            Self::AntiOscillation => "perf.reason.anti_oscillation",
            Self::DropRate => "perf.reason.drop_rate",
            Self::CpuHigh => "perf.reason.cpu_high",
            Self::MemLow => "perf.reason.mem_low",
            Self::Stable => "perf.reason.stable",
        }
    }
}

/// Keputusan yang dikirim ke daemon (dan ke log keputusan).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Decision {
    /// Level yang harus dipakai setelah keputusan ini.
    pub new_level: Level,
    /// Alasan mesin.
    pub reason_code: ReasonCode,
    /// ID pesan untuk teks manusiawi.
    pub message_id: &'static str,
    /// Angka pendukung (laju drop, CPU, memori, detik).
    pub params: serde_json::Value,
}

impl Decision {
    /// Apakah keputusan ini mengubah level.
    pub fn berubah(&self) -> bool {
        self.params
            .get("dari")
            .and_then(serde_json::Value::as_u64)
            .map(|dari| dari != u64::from(self.new_level.as_u8()))
            .unwrap_or(false)
    }
}

/// Batas dan ambang kebijakan; semua bisa dikonfigurasi pengguna.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerfConfig {
    /// Mode kinerja aktif.
    pub mode: PerfMode,
    /// Level tetap untuk mode Kustom.
    pub custom_level: Level,
    /// Level terberat yang boleh dipakai mode Otomatis.
    pub max_level: Level,
    /// Jeda minimal antar perubahan level.
    pub dwell: Duration,
    /// Ambang laju drop yang dianggap buruk (0.05 = 5%).
    pub drop_threshold: f64,
    /// Berapa lama laju drop harus terus buruk sebelum turun level.
    pub drop_window: Duration,
    /// Ambang CPU total yang dianggap buruk (persen).
    pub cpu_threshold: f64,
    /// Berapa lama CPU harus terus tinggi sebelum turun level.
    pub cpu_window: Duration,
    /// `MemAvailable` di bawah nilai ini langsung menurunkan level.
    pub mem_low_mb: u64,
    /// Ambang laju drop yang dianggap stabil.
    pub stable_drop: f64,
    /// Ambang CPU yang dianggap stabil.
    pub stable_cpu: f64,
    /// Berapa lama kondisi harus stabil sebelum naik level.
    pub stable_window: Duration,
    /// Jendela penghitungan osilasi.
    pub oscillation_window: Duration,
    /// Jumlah pasang naik-turun yang memicu antiosilasi.
    pub oscillation_pairs: usize,
    /// Berapa lama level rendah ditahan saat antiosilasi.
    pub oscillation_hold: Duration,
}

impl Default for PerfConfig {
    fn default() -> Self {
        Self {
            mode: PerfMode::Auto,
            custom_level: Level::L1,
            max_level: Level::L4,
            dwell: Duration::from_secs(30),
            drop_threshold: 0.05,
            drop_window: Duration::from_secs(10),
            cpu_threshold: 90.0,
            cpu_window: Duration::from_secs(15),
            mem_low_mb: 150,
            stable_drop: 0.01,
            stable_cpu: 70.0,
            stable_window: Duration::from_secs(60),
            oscillation_window: Duration::from_secs(600),
            oscillation_pairs: 3,
            oscillation_hold: Duration::from_secs(1800),
        }
    }
}

impl PerfConfig {
    /// Plafon level menurut mode; `None` berarti mode tidak membatasi.
    pub fn plafon(&self) -> Option<Level> {
        match self.mode {
            PerfMode::Auto => Some(self.max_level),
            PerfMode::Original => Some(Level::L0),
            PerfMode::Balanced => Some(Level::L1),
            PerfMode::Saver => Some(Level::L2),
            PerfMode::Custom => None,
        }
    }
}

/// Penjaga antiosilasi: menahan level rendah setelah naik-turun berulang.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OscillationGuard {
    perubahan: Vec<Duration>,
    tahan_sampai: Option<Duration>,
}

impl OscillationGuard {
    /// Penjaga kosong.
    pub fn new() -> Self {
        Self::default()
    }

    /// Catat satu perubahan level pada waktu `now`.
    pub fn record_change(&mut self, now: Duration, config: &PerfConfig) {
        self.perubahan
            .retain(|t| now.saturating_sub(*t) <= config.oscillation_window);
        self.perubahan.push(now);
        let jumlah_pasang = self.perubahan.len() / 2;
        if jumlah_pasang >= config.oscillation_pairs {
            self.tahan_sampai = Some(now.saturating_add(config.oscillation_hold));
            self.perubahan.clear();
        }
    }

    /// Apakah level sedang ditahan pada waktu `now`.
    pub fn holding(&self, now: Duration) -> bool {
        self.tahan_sampai.is_some_and(|batas| now < batas)
    }

    /// Sampai kapan level ditahan, bila sedang aktif.
    pub fn tahan_sampai(&self) -> Option<Duration> {
        self.tahan_sampai
    }
}

/// Status kebijakan yang dibawa antar siklus pemantauan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerfState {
    /// Level yang sedang dipakai.
    pub level: Level,
    /// Waktu perubahan level terakhir.
    pub last_change: Duration,
    /// Sejak kapan laju drop melewati ambang (berturut-turut).
    pub drop_bad_since: Option<Duration>,
    /// Sejak kapan CPU total melewati ambang (berturut-turut).
    pub cpu_bad_since: Option<Duration>,
    /// Sejak kapan kondisi dianggap stabil.
    pub good_since: Option<Duration>,
    /// Penjaga antiosilasi.
    pub guard: OscillationGuard,
}

impl PerfState {
    /// Status awal pada level tertentu.
    pub fn new(level: Level, now: Duration) -> Self {
        Self {
            level,
            last_change: now,
            drop_bad_since: None,
            cpu_bad_since: None,
            good_since: None,
            guard: OscillationGuard::new(),
        }
    }

    /// Mulai ulang pengamatan setelah mode atau wallpaper berganti.
    pub fn reset(&mut self, level: Level, now: Duration) {
        self.level = level;
        self.last_change = now;
        self.drop_bad_since = None;
        self.cpu_bad_since = None;
        self.good_since = None;
    }

    /// Perbarui penghitung "berturut-turut" dari sampel terbaru.
    pub fn observe(&mut self, metrics: &Metrics, config: &PerfConfig, now: Duration) {
        let drop_buruk = metrics.drop_rate > config.drop_threshold;
        let cpu_buruk = metrics.cpu_total > config.cpu_threshold;
        let stabil =
            metrics.drop_rate < config.stable_drop && metrics.cpu_total < config.stable_cpu;

        self.drop_bad_since = majukan(self.drop_bad_since, drop_buruk, now);
        self.cpu_bad_since = majukan(self.cpu_bad_since, cpu_buruk, now);
        self.good_since = majukan(self.good_since, stabil, now);
    }

    /// Terapkan keputusan ke status (level, dwell, penghitung, penjaga).
    pub fn terapkan(&mut self, keputusan: &Decision, config: &PerfConfig, now: Duration) {
        if keputusan.new_level == self.level {
            return;
        }
        self.level = keputusan.new_level;
        self.last_change = now;
        self.guard.record_change(now, config);
        self.drop_bad_since = None;
        self.cpu_bad_since = None;
        self.good_since = None;
    }

    /// Satu siklus penuh: amati, putuskan, terapkan, kembalikan keputusan.
    pub fn step(&mut self, metrics: &Metrics, config: &PerfConfig, now: Duration) -> Decision {
        self.observe(metrics, config, now);
        let keputusan = decide(self, metrics, config, now);
        self.terapkan(&keputusan, config, now);
        keputusan
    }
}

fn majukan(saat: Option<Duration>, kondisi: bool, now: Duration) -> Option<Duration> {
    if kondisi {
        Some(saat.unwrap_or(now))
    } else {
        None
    }
}

/// Putuskan level berikutnya. **Fungsi murni**: tidak menyentuh I/O atau waktu.
pub fn decide(
    state: &PerfState,
    metrics: &Metrics,
    config: &PerfConfig,
    now: Duration,
) -> Decision {
    let level = state.level;

    // 1. Mode yang mengunci level sepenuhnya.
    match config.mode {
        PerfMode::Original => {
            return if level == Level::L0 {
                tetap(level, ReasonCode::ModeOriginal, metrics, now)
            } else {
                ubah(level, Level::L0, ReasonCode::ModeOriginal, metrics, now)
            };
        }
        PerfMode::Custom => {
            return if level == config.custom_level {
                tetap(level, ReasonCode::ModeCustom, metrics, now)
            } else {
                ubah(
                    level,
                    config.custom_level,
                    ReasonCode::ModeCustom,
                    metrics,
                    now,
                )
            };
        }
        PerfMode::Auto | PerfMode::Balanced | PerfMode::Saver => {}
    }

    // 2. Plafon mode: turun tanpa menunggu dwell karena ini batas kebijakan.
    if let Some(plafon) = config.plafon() {
        if level > plafon {
            return ubah(level, plafon, ReasonCode::ModeCeiling, metrics, now);
        }
    }

    // 3. Antiosilasi: tahan level apa pun selama jendela tahan aktif.
    if state.guard.holding(now) {
        return tetap(level, ReasonCode::AntiOscillation, metrics, now);
    }

    let dwell_cukup = now.saturating_sub(state.last_change) >= config.dwell;

    // 4. Memori hampir habis: paling mendesak.
    if metrics.mem_available_mb < config.mem_low_mb && dwell_cukup {
        if let Some(turun) = level.turun() {
            return ubah(level, turun, ReasonCode::MemLow, metrics, now);
        }
        return tetap(level, ReasonCode::MemLow, metrics, now);
    }

    // 5. Laju drop terlalu tinggi selama cukup lama.
    if let Some(mulai) = state.drop_bad_since {
        if now.saturating_sub(mulai) >= config.drop_window && dwell_cukup {
            if let Some(turun) = level.turun() {
                return ubah(level, turun, ReasonCode::DropRate, metrics, now);
            }
        }
    }

    // 6. CPU total terlalu tinggi selama cukup lama.
    if let Some(mulai) = state.cpu_bad_since {
        if now.saturating_sub(mulai) >= config.cpu_window && dwell_cukup {
            if let Some(turun) = level.turun() {
                return ubah(level, turun, ReasonCode::CpuHigh, metrics, now);
            }
        }
    }

    // 7. Naik satu level setelah stabil cukup lama.
    if level > Level::L0 && dwell_cukup {
        if let Some(mulai) = state.good_since {
            if now.saturating_sub(mulai) >= config.stable_window {
                if let Some(naik) = level.naik() {
                    return ubah(level, naik, ReasonCode::Stable, metrics, now);
                }
            }
        }
    }

    tetap(level, ReasonCode::Hold, metrics, now)
}

fn params_dasar(level: Level, metrics: &Metrics, now: Duration) -> serde_json::Value {
    serde_json::json!({
        "level": level.as_u8(),
        "drop_rate": metrics.drop_rate,
        "cpu_total": metrics.cpu_total,
        "mem_available_mb": metrics.mem_available_mb,
        "detik": now.as_secs(),
    })
}

fn tetap(level: Level, alasan: ReasonCode, metrics: &Metrics, now: Duration) -> Decision {
    Decision {
        new_level: level,
        reason_code: alasan,
        message_id: alasan.message_id(),
        params: params_dasar(level, metrics, now),
    }
}

fn ubah(dari: Level, ke: Level, alasan: ReasonCode, metrics: &Metrics, now: Duration) -> Decision {
    let mut params = params_dasar(ke, metrics, now);
    if let Some(obj) = params.as_object_mut() {
        obj.insert("dari".to_string(), serde_json::json!(dari.as_u8()));
    }
    Decision {
        new_level: ke,
        reason_code: alasan,
        message_id: alasan.message_id(),
        params,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const D: u64 = 1;

    fn cfg() -> PerfConfig {
        PerfConfig::default()
    }

    fn detik(n: u64) -> Duration {
        Duration::from_secs(n * D)
    }

    fn metrik(drop: f64, cpu: f64, mem: u64) -> Metrics {
        Metrics::new(drop, cpu, mem, 60.0)
    }

    fn sehat() -> Metrics {
        metrik(0.0, 30.0, 4000)
    }

    fn buruk_drop() -> Metrics {
        metrik(0.20, 40.0, 4000)
    }

    /// Satu baris tabel skenario untuk `decide`.
    struct Skenario {
        nama: &'static str,
        state: PerfState,
        metrics: Metrics,
        mode: PerfMode,
        now: u64,
        harap_level: Level,
        harap_alasan: ReasonCode,
    }

    fn jalankan_tabel(daftar: Vec<Skenario>) {
        for s in daftar {
            let mut config = cfg();
            config.mode = s.mode;
            let keputusan = decide(&s.state, &s.metrics, &config, detik(s.now));
            assert_eq!(
                keputusan.new_level, s.harap_level,
                "level salah pada skenario '{}'",
                s.nama
            );
            assert_eq!(
                keputusan.reason_code, s.harap_alasan,
                "alasan salah pada skenario '{}'",
                s.nama
            );
            assert_eq!(keputusan.message_id, s.harap_alasan.message_id());
        }
    }

    #[test]
    fn tabel_skenario_decide() {
        // Perubahan level terakhir dibuat lama (t=0) supaya dwell 30 detik tidak
        // menutupi aturan yang sedang diuji.
        let mut turun_drop = PerfState::new(Level::L1, detik(0));
        turun_drop.drop_bad_since = Some(detik(20));

        let mut belum_cukup = PerfState::new(Level::L1, detik(0));
        belum_cukup.drop_bad_since = Some(detik(22));

        let mut turun_cpu = PerfState::new(Level::L1, detik(0));
        turun_cpu.cpu_bad_since = Some(detik(0));

        let mut cpu_baru = PerfState::new(Level::L1, detik(0));
        cpu_baru.cpu_bad_since = Some(detik(25));

        let mem_rendah = PerfState::new(Level::L1, detik(0));

        let mut masih_dwell = PerfState::new(Level::L2, detik(25));
        masih_dwell.drop_bad_since = Some(detik(0));

        let mut stabil = PerfState::new(Level::L2, detik(0));
        stabil.good_since = Some(detik(0));

        let mut stabil_baru = PerfState::new(Level::L2, detik(0));
        stabil_baru.good_since = Some(detik(40));

        let mut l0_stabil = PerfState::new(Level::L0, detik(0));
        l0_stabil.good_since = Some(detik(0));

        let mut ditahan = PerfState::new(Level::L1, detik(0));
        ditahan.good_since = Some(detik(0));
        ditahan.guard.tahan_sampai = Some(detik(1000));

        jalankan_tabel(vec![
            Skenario {
                nama: "turun karena drop > 5% selama 10 detik",
                state: turun_drop,
                metrics: buruk_drop(),
                mode: PerfMode::Auto,
                now: 30,
                harap_level: Level::L2,
                harap_alasan: ReasonCode::DropRate,
            },
            Skenario {
                nama: "lonjakan drop 8 detik belum menurunkan level",
                state: belum_cukup,
                metrics: buruk_drop(),
                mode: PerfMode::Auto,
                now: 30,
                harap_level: Level::L1,
                harap_alasan: ReasonCode::Hold,
            },
            Skenario {
                nama: "turun karena CPU > 90% selama 15 detik",
                state: turun_cpu,
                metrics: metrik(0.0, 95.0, 4000),
                mode: PerfMode::Auto,
                now: 30,
                harap_level: Level::L2,
                harap_alasan: ReasonCode::CpuHigh,
            },
            Skenario {
                nama: "CPU tinggi 5 detik belum cukup",
                state: cpu_baru,
                metrics: metrik(0.0, 95.0, 4000),
                mode: PerfMode::Auto,
                now: 30,
                harap_level: Level::L1,
                harap_alasan: ReasonCode::Hold,
            },
            Skenario {
                nama: "MemAvailable < 150 MB langsung turun",
                state: mem_rendah,
                metrics: metrik(0.0, 40.0, 120),
                mode: PerfMode::Auto,
                now: 30,
                harap_level: Level::L2,
                harap_alasan: ReasonCode::MemLow,
            },
            Skenario {
                nama: "dwell 30 detik menahan perubahan",
                state: masih_dwell,
                metrics: buruk_drop(),
                mode: PerfMode::Auto,
                now: 30,
                harap_level: Level::L2,
                harap_alasan: ReasonCode::Hold,
            },
            Skenario {
                nama: "naik setelah stabil 60 detik",
                state: stabil,
                metrics: sehat(),
                mode: PerfMode::Auto,
                now: 60,
                harap_level: Level::L1,
                harap_alasan: ReasonCode::Stable,
            },
            Skenario {
                nama: "stabil 20 detik belum menaikkan level",
                state: stabil_baru,
                metrics: sehat(),
                mode: PerfMode::Auto,
                now: 60,
                harap_level: Level::L2,
                harap_alasan: ReasonCode::Hold,
            },
            Skenario {
                nama: "L0 tidak pernah naik lagi",
                state: l0_stabil,
                metrics: sehat(),
                mode: PerfMode::Auto,
                now: 600,
                harap_level: Level::L0,
                harap_alasan: ReasonCode::Hold,
            },
            Skenario {
                nama: "antiosilasi menahan kenaikan",
                state: ditahan,
                metrics: sehat(),
                mode: PerfMode::Auto,
                now: 900,
                harap_level: Level::L1,
                harap_alasan: ReasonCode::AntiOscillation,
            },
            Skenario {
                nama: "mode Original memaksa L0 walau metrik buruk",
                state: PerfState::new(Level::L3, detik(0)),
                metrics: buruk_drop(),
                mode: PerfMode::Original,
                now: 100,
                harap_level: Level::L0,
                harap_alasan: ReasonCode::ModeOriginal,
            },
            Skenario {
                nama: "mode Original tetap L0 walau metrik buruk",
                state: PerfState::new(Level::L0, detik(0)),
                metrics: metrik(0.5, 99.0, 50),
                mode: PerfMode::Original,
                now: 100,
                harap_level: Level::L0,
                harap_alasan: ReasonCode::ModeOriginal,
            },
            Skenario {
                nama: "mode Kustom memakai nilai pengguna",
                state: PerfState::new(Level::L3, detik(0)),
                metrics: metrik(0.5, 99.0, 50),
                mode: PerfMode::Custom,
                now: 100,
                harap_level: Level::L1,
                harap_alasan: ReasonCode::ModeCustom,
            },
            Skenario {
                nama: "mode Seimbang membatasi ke L1",
                state: PerfState::new(Level::L3, detik(0)),
                metrics: sehat(),
                mode: PerfMode::Balanced,
                now: 100,
                harap_level: Level::L1,
                harap_alasan: ReasonCode::ModeCeiling,
            },
            Skenario {
                nama: "mode Hemat membatasi ke L2",
                state: PerfState::new(Level::L4, detik(0)),
                metrics: sehat(),
                mode: PerfMode::Saver,
                now: 100,
                harap_level: Level::L2,
                harap_alasan: ReasonCode::ModeCeiling,
            },
        ]);
    }

    #[test]
    fn mode_custom_tidak_beradaptasi() {
        let mut config = cfg();
        config.mode = PerfMode::Custom;
        config.custom_level = Level::L2;
        let mut state = PerfState::new(Level::L2, detik(0));
        let mut sekarang = 0;
        for _ in 0..200 {
            sekarang += 2;
            let keputusan = state.step(&metrik(0.9, 99.0, 40), &config, detik(sekarang));
            assert_eq!(keputusan.new_level, Level::L2);
            assert_eq!(state.level, Level::L2);
        }
    }

    #[test]
    fn mode_original_tidak_pernah_turun() {
        let mut config = cfg();
        config.mode = PerfMode::Original;
        let mut state = PerfState::new(Level::L0, detik(0));
        for i in 1..=100 {
            let keputusan = state.step(&metrik(0.9, 99.0, 40), &config, detik(i * 2));
            assert_eq!(keputusan.new_level, Level::L0);
        }
        assert_eq!(state.level, Level::L0);
    }

    #[test]
    fn deret_beban_tinggi_turun_lalu_pulih() {
        let config = cfg();
        let mut state = PerfState::new(Level::L0, detik(0));

        // Beban tinggi: 2 detik per sampel, harus turun bertahap.
        let mut sekarang = 0;
        for _ in 0..30 {
            sekarang += 2;
            state.step(&buruk_drop(), &config, detik(sekarang));
        }
        assert!(
            state.level >= Level::L2,
            "harus turun minimal dua level, dapat {}",
            state.level
        );
        let puncak = state.level;

        // Beban hilang: harus pulih ke L0 dalam waktu wajar.
        for _ in 0..200 {
            sekarang += 2;
            state.step(&sehat(), &config, detik(sekarang));
        }
        assert_eq!(state.level, Level::L0, "harus pulih dari {puncak}");
    }

    #[test]
    fn lonjakan_sesaat_tidak_mengubah_level() {
        let config = cfg();
        let mut state = PerfState::new(Level::L0, detik(0));

        // Tiga sampel buruk (6 detik) lalu normal: kurang dari jendela 10 detik.
        state.step(&buruk_drop(), &config, detik(2));
        state.step(&buruk_drop(), &config, detik(4));
        state.step(&buruk_drop(), &config, detik(6));
        for i in 4..=20 {
            let keputusan = state.step(&sehat(), &config, detik(i * 2));
            assert_eq!(keputusan.new_level, Level::L0);
        }
        assert_eq!(state.level, Level::L0);
        assert!(
            state.drop_bad_since.is_none(),
            "penghitung drop harus direset"
        );
    }

    #[test]
    fn osilasi_menahan_level_selama_30_menit() {
        let config = cfg();
        let mut guard = OscillationGuard::new();
        // Tiga pasang naik-turun dalam 10 menit (6 perubahan).
        for i in 0..6 {
            guard.record_change(detik(i * 60), &config);
        }
        assert!(guard.holding(detik(400)));
        assert_eq!(guard.tahan_sampai(), Some(detik(300 + 1800)));

        let mut state = PerfState::new(Level::L1, detik(360));
        state.good_since = Some(detik(0));
        state.guard = guard;
        let keputusan = decide(&state, &sehat(), &config, detik(400));
        assert_eq!(keputusan.reason_code, ReasonCode::AntiOscillation);
        assert_eq!(keputusan.new_level, Level::L1);

        // Setelah 30 menit, tahan berakhir dan level boleh naik.
        assert!(!state.guard.holding(detik(2200)));
        let setelah = decide(&state, &sehat(), &config, detik(2200));
        assert_eq!(setelah.new_level, Level::L0);
    }

    #[test]
    fn osilasi_butuh_enam_perubahan() {
        let config = cfg();
        let mut guard = OscillationGuard::new();
        for i in 0..5 {
            guard.record_change(detik(i * 60), &config);
        }
        assert!(!guard.holding(detik(300)));
        guard.record_change(detik(300), &config);
        assert!(guard.holding(detik(301)));
    }

    #[test]
    fn perubahan_lama_di_luar_jendela_tidak_dihitung() {
        let config = cfg();
        let mut guard = OscillationGuard::new();
        for i in 0..6 {
            guard.record_change(detik(i * 200), &config);
        }
        assert!(!guard.holding(detik(1000)), "perubahan tersebar > 10 menit");
    }

    #[test]
    fn step_mereset_penghitung_setelah_perubahan() {
        let config = cfg();
        let mut state = PerfState::new(Level::L0, detik(0));
        for i in 1..=15 {
            state.step(&buruk_drop(), &config, detik(i * 2));
        }
        assert_eq!(state.level, Level::L1);
        assert!(state.drop_bad_since.is_none());
        assert!(state.good_since.is_none());
    }

    #[test]
    fn level_newtype_berperilaku_benar() {
        assert_eq!(Level::L0.turun(), Some(Level::L1));
        assert_eq!(Level::L4.turun(), None);
        assert_eq!(Level::L0.naik(), None);
        assert_eq!(Level::L4.naik(), Some(Level::L3));
        assert_eq!(Level::L2.min(Level::L1), Level::L1);
        assert_eq!(Level::from_u8(5), None);
        assert_eq!(Level::from_u8(3), Some(Level::L3));
        assert_eq!(Level::L3.to_string(), "L3");
        assert_eq!(serde_json::to_string(&Level::L2).unwrap(), "2");
        assert_eq!(serde_json::from_str::<Level>("4").unwrap(), Level::L4);
        assert!(serde_json::from_str::<Level>("9").is_err());
    }

    #[test]
    fn config_plafon_sesuai_mode() {
        let mut config = cfg();
        assert_eq!(config.plafon(), Some(Level::L4));
        config.mode = PerfMode::Original;
        assert_eq!(config.plafon(), Some(Level::L0));
        config.mode = PerfMode::Balanced;
        assert_eq!(config.plafon(), Some(Level::L1));
        config.mode = PerfMode::Saver;
        assert_eq!(config.plafon(), Some(Level::L2));
        config.mode = PerfMode::Custom;
        assert_eq!(config.plafon(), None);
    }

    #[test]
    fn keputusan_serialisasi() {
        let keputusan = ubah(
            Level::L0,
            Level::L1,
            ReasonCode::DropRate,
            &buruk_drop(),
            detik(12),
        );
        assert!(keputusan.berubah());
        let json = serde_json::to_value(&keputusan).unwrap();
        assert_eq!(json["new_level"], serde_json::json!(1));
        assert_eq!(json["reason_code"], serde_json::json!("drop_rate"));
        assert_eq!(
            json["message_id"],
            serde_json::json!("perf.reason.drop_rate")
        );
        assert_eq!(json["params"]["dari"], serde_json::json!(0));

        // Keputusan tanpa perubahan tidak punya penanda "dari".
        let diam = tetap(Level::L2, ReasonCode::Hold, &sehat(), detik(5));
        assert!(!diam.berubah());
        assert!(diam.params.get("dari").is_none());
    }
}
