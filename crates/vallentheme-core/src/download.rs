//! Pembungkus `yt-dlp` untuk fitur "Dari tautan".
//!
//! Dua bagian:
//! - [`build_ytdlp_args`] — **fungsi murni** penyusun argumen, mudah diuji.
//! - [`DownloadJob`] — menjalankan `yt-dlp` tanpa shell, membaca progres per
//!   baris, dan bisa dibatalkan.
//!
//! Rujukan: `panduan/06` bagian 6 dan `fase/FASE-8` bagian 8.1. Proses dibuat
//! lewat [`crate::probe::spawn`] supaya semua proses anak di crate ini memakai
//! bentuk `Command` yang sama (tanpa shell, stdin tertutup, keluaran pipa).

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use url::Url;

use crate::error::{Error, Result};

/// Nama program bawaan.
pub const PROGRAM_BAWAAN: &str = "yt-dlp";

/// Tinggi maksimum bawaan bila pengguna tidak memilih.
pub const TINGGI_MAKS_BAWAAN: u32 = 1080;

/// Batas ukuran unduhan bawaan (MB) — sama dengan `panduan/05`.
pub const MAX_FILESIZE_MB_BAWAAN: u32 = 2048;

/// Jeda antar pemeriksaan proses saat menunggu `yt-dlp`.
const JEDA_PANTAU: Duration = Duration::from_millis(50);

/// Susun argumen `yt-dlp` sesuai `panduan/06` bagian 6.
///
/// URL **selalu** diletakkan setelah `--` sehingga tidak mungkin dibaca sebagai
/// opsi, dan tidak ada string yang diteruskan ke shell.
pub fn build_ytdlp_args(
    url: &Url,
    dir_tujuan: &Path,
    tinggi_maks: u32,
    max_filesize_mb: u32,
) -> Vec<OsString> {
    let pemilih_format = format!(
        "bv*[height<=?{h}]+ba/b[height<=?{h}]/bv*+ba/b",
        h = tinggi_maks
    );
    let pola_nama = dir_tujuan.join("%(title).100B [%(id)s].%(ext)s");

    let mut args: Vec<OsString> = Vec::with_capacity(18);
    for tetap in [
        "--no-playlist",
        "--newline",
        "--no-exec",
        "--restrict-filenames",
    ] {
        args.push(OsString::from(tetap));
    }
    args.push(OsString::from("--max-filesize"));
    args.push(OsString::from(format!("{max_filesize_mb}M")));
    args.push(OsString::from("-f"));
    args.push(OsString::from(pemilih_format));
    args.push(OsString::from("--merge-output-format"));
    args.push(OsString::from("mp4"));
    args.push(OsString::from("--progress-template"));
    args.push(OsString::from("download:%(progress._percent_str)s"));
    args.push(OsString::from("-o"));
    args.push(pola_nama.into_os_string());
    args.push(OsString::from("--"));
    args.push(OsString::from(url.as_str()));
    args
}

/// Ambil angka persen dari satu baris keluaran `yt-dlp`.
///
/// Baris contoh: `download:  45.3%`, `[download] 100.0% of 12.3MiB`.
pub fn parse_progress_percent(baris: &str) -> Option<f32> {
    let bersih = baris.trim();
    let bersih = bersih.strip_prefix("download:").unwrap_or(bersih).trim();
    let (depan, _) = bersih.split_once('%')?;
    let angka: String = depan
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if angka.is_empty() {
        return None;
    }
    let nilai: f32 = angka.parse().ok()?;
    if nilai.is_finite() && (0.0..=100.0).contains(&nilai) {
        Some(nilai)
    } else {
        None
    }
}

/// Kemajuan satu baris unduhan.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadProgress {
    /// Persen 0–100 bila baris memuat angka persen.
    pub percent: Option<f32>,
    /// Baris asli dari `yt-dlp`.
    pub baris: String,
}

/// Peristiwa yang dikirim dari utas kerja ke pemanggil.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Satu baris progres dari stdout.
    Progress(DownloadProgress),
    /// Baris diagnostik dari stderr.
    Baris(String),
    /// Proses selesai (sukses, gagal, atau dibatalkan).
    Selesai(DownloadOutcome),
    /// Proses tidak bisa dijalankan atau ditunggu.
    Gagal(String),
}

/// Hasil akhir sebuah unduhan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DownloadOutcome {
    /// `yt-dlp` keluar dengan status 0.
    pub sukses: bool,
    /// Dihentikan lewat [`DownloadJob::batal`].
    pub dibatalkan: bool,
    /// Kode keluar bila tersedia.
    pub kode: Option<i32>,
}

/// Ringkasan setelah [`DownloadJob::tunggu`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Laporan {
    /// Semua baris progres yang terbaca, berurutan.
    pub progres: Vec<DownloadProgress>,
    /// Baris diagnostik (stderr) dan pesan kesalahan.
    pub pesan: Vec<String>,
    /// Hasil akhir; `None` bila batas waktu tercapai.
    pub hasil: Option<DownloadOutcome>,
}

/// Apakah `yt-dlp` terpasang di mesin ini.
pub fn tersedia() -> bool {
    crate::probe::jalankan(
        PROGRAM_BAWAAN,
        &[OsString::from("--version")],
        Duration::from_secs(5),
    )
    .is_ok()
}

/// Satu pekerjaan unduhan: satu URL, satu proses.
pub struct DownloadJob {
    program: OsString,
    args: Vec<OsString>,
    batal: Arc<AtomicBool>,
    pengirim: Option<Sender<Event>>,
    penerima: Receiver<Event>,
    pekerja: Option<JoinHandle<()>>,
}

impl DownloadJob {
    /// Siapkan pekerjaan untuk `url` dengan program bawaan `yt-dlp`.
    pub fn new(url: &Url, dir_tujuan: &Path, tinggi_maks: u32, max_filesize_mb: u32) -> Self {
        Self::dengan_program(
            PROGRAM_BAWAAN,
            url,
            dir_tujuan,
            tinggi_maks,
            max_filesize_mb,
        )
    }

    /// Siapkan pekerjaan dengan program lain (dipakai uji dengan `yt-dlp` palsu).
    pub fn dengan_program(
        program: impl Into<OsString>,
        url: &Url,
        dir_tujuan: &Path,
        tinggi_maks: u32,
        max_filesize_mb: u32,
    ) -> Self {
        let (pengirim, penerima) = mpsc::channel();
        Self {
            program: program.into(),
            args: build_ytdlp_args(url, dir_tujuan, tinggi_maks, max_filesize_mb),
            batal: Arc::new(AtomicBool::new(false)),
            pengirim: Some(pengirim),
            penerima,
            pekerja: None,
        }
    }

    /// Argumen yang akan dipakai (berguna untuk log dan uji).
    pub fn args(&self) -> &[OsString] {
        &self.args
    }

    /// Jalankan proses di utas terpisah.
    pub fn start(&mut self) -> Result<()> {
        if self.pekerja.is_some() {
            return Err(Error::Download("unduhan ini sudah berjalan".to_string()));
        }
        let Some(pengirim) = self.pengirim.take() else {
            return Err(Error::Download(
                "unduhan ini sudah pernah dijalankan".to_string(),
            ));
        };
        let program = self.program.clone();
        let args = self.args.clone();
        let batal = Arc::clone(&self.batal);

        let pekerja = thread::Builder::new()
            .name("vallentheme-ytdlp".to_string())
            .spawn(move || jalankan_anak(&program, &args, &batal, &pengirim))
            .map_err(|e| Error::Download(format!("gagal memulai utas unduhan: {e}")))?;
        self.pekerja = Some(pekerja);
        Ok(())
    }

    /// Minta proses berhenti. Proses dibunuh pada pemeriksaan berikutnya.
    pub fn batal(&self) {
        self.batal.store(true, Ordering::SeqCst);
    }

    /// Ambil satu peristiwa, menunggu paling lama `batas`.
    pub fn terima(&self, batas: Duration) -> Option<Event> {
        self.penerima.recv_timeout(batas).ok()
    }

    /// Tunggu sampai selesai (atau batas waktu) dan kumpulkan hasilnya.
    ///
    /// Bila batas waktu tercapai, proses dibatalkan lebih dulu supaya tidak ada
    /// proses yatim.
    pub fn tunggu(mut self, batas: Duration) -> Laporan {
        let akhir = Instant::now() + batas;
        let mut laporan = Laporan::default();
        loop {
            let sisa = akhir.saturating_duration_since(Instant::now());
            if sisa.is_zero() {
                break;
            }
            match self.penerima.recv_timeout(sisa) {
                Ok(Event::Progress(p)) => laporan.progres.push(p),
                Ok(Event::Baris(b)) => laporan.pesan.push(b),
                Ok(Event::Selesai(hasil)) => {
                    laporan.hasil = Some(hasil);
                    break;
                }
                Ok(Event::Gagal(pesan)) => {
                    laporan.pesan.push(pesan);
                    break;
                }
                Err(_) => break,
            }
        }
        if laporan.hasil.is_none() {
            self.batal();
        }
        if let Some(pekerja) = self.pekerja.take() {
            let _ = pekerja.join();
        }
        laporan
    }
}

/// Badan utas kerja: jalankan proses, alirkan keluaran, tangani pembatalan.
fn jalankan_anak(program: &OsStr, args: &[OsString], batal: &AtomicBool, tx: &Sender<Event>) {
    let mut anak = match crate::probe::spawn(program, args) {
        Ok(anak) => anak,
        Err(e) => {
            let pesan = match e {
                Error::ProbeToolMissing(nama) => Error::DownloadToolMissing(nama).to_string(),
                lain => Error::Download(lain.to_string()).to_string(),
            };
            let _ = tx.send(Event::Gagal(pesan));
            return;
        }
    };

    let pembaca_stdout = anak.stdout.take().and_then(|pipa| {
        let tx = tx.clone();
        thread::Builder::new()
            .name("vallentheme-ytdlp-stdout".to_string())
            .spawn(move || {
                for baris in BufReader::new(pipa)
                    .lines()
                    .map_while(std::result::Result::ok)
                {
                    let percent = parse_progress_percent(&baris);
                    let peristiwa = Event::Progress(DownloadProgress { percent, baris });
                    if tx.send(peristiwa).is_err() {
                        return;
                    }
                }
            })
            .ok()
    });

    let pembaca_stderr = anak.stderr.take().and_then(|pipa| {
        let tx = tx.clone();
        thread::Builder::new()
            .name("vallentheme-ytdlp-stderr".to_string())
            .spawn(move || {
                for baris in BufReader::new(pipa)
                    .lines()
                    .map_while(std::result::Result::ok)
                {
                    if tx.send(Event::Baris(baris)).is_err() {
                        return;
                    }
                }
            })
            .ok()
    });

    let hasil = loop {
        if batal.load(Ordering::SeqCst) {
            let _ = anak.kill();
            let _ = anak.wait();
            break DownloadOutcome {
                sukses: false,
                dibatalkan: true,
                kode: None,
            };
        }
        match anak.try_wait() {
            Ok(Some(status)) => {
                break DownloadOutcome {
                    sukses: status.success(),
                    dibatalkan: false,
                    kode: status.code(),
                }
            }
            Ok(None) => thread::sleep(JEDA_PANTAU),
            Err(e) => {
                let _ = anak.kill();
                let _ = anak.wait();
                let _ = tx.send(Event::Gagal(format!("gagal menunggu yt-dlp: {e}")));
                gabung(pembaca_stdout);
                gabung(pembaca_stderr);
                return;
            }
        }
    };

    // Pipa tertutup setelah proses berhenti, jadi pembaca akan selesai sendiri.
    gabung(pembaca_stdout);
    gabung(pembaca_stderr);
    let _ = tx.send(Event::Selesai(hasil));
}

fn gabung(pegangan: Option<JoinHandle<()>>) {
    if let Some(pegangan) = pegangan {
        let _ = pegangan.join();
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn url_uji() -> Url {
        crate::url::validate("https://www.youtube.com/watch?v=abc123").unwrap()
    }

    fn teks(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn argumen_sesuai_panduan_06() {
        let args = build_ytdlp_args(
            &url_uji(),
            Path::new("/home/user/Videos/vallentheme"),
            720,
            2048,
        );
        let harap = vec![
            "--no-playlist",
            "--newline",
            "--no-exec",
            "--restrict-filenames",
            "--max-filesize",
            "2048M",
            "-f",
            "bv*[height<=?720]+ba/b[height<=?720]/bv*+ba/b",
            "--merge-output-format",
            "mp4",
            "--progress-template",
            "download:%(progress._percent_str)s",
            "-o",
            "/home/user/Videos/vallentheme/%(title).100B [%(id)s].%(ext)s",
            "--",
            "https://www.youtube.com/watch?v=abc123",
        ];
        assert_eq!(teks(&args), harap);
    }

    #[test]
    fn url_selalu_setelah_pemisah() {
        let args = build_ytdlp_args(&url_uji(), Path::new("/tmp/x"), 1080, 100);
        let posisi = args
            .iter()
            .position(|a| a.to_str() == Some("--"))
            .expect("harus ada pemisah --");
        assert_eq!(posisi, args.len() - 2, "URL harus tepat setelah --");
        assert_eq!(args[posisi + 1], OsString::from(url_uji().as_str()));
    }

    #[test]
    fn tinggi_maks_dan_batas_ukuran_masuk_ke_argumen() {
        let args = teks(&build_ytdlp_args(
            &url_uji(),
            Path::new("/tmp/x"),
            2160,
            512,
        ));
        assert!(args
            .iter()
            .any(|a| a == "bv*[height<=?2160]+ba/b[height<=?2160]/bv*+ba/b"));
        assert!(args.iter().any(|a| a == "512M"));
    }

    #[test]
    fn folder_dengan_spasi_dan_unicode_tetap_utuh() {
        let dir = Path::new("/home/user/Vídeo Wallpaper/unduhan");
        let args = teks(&build_ytdlp_args(&url_uji(), dir, 720, 100));
        let pola = args.iter().find(|a| a.contains("%(title)")).unwrap();
        assert!(pola.starts_with("/home/user/Vídeo Wallpaper/unduhan/"));
    }

    #[test]
    fn parser_persen() {
        assert_eq!(parse_progress_percent("download:  45.3%"), Some(45.3));
        assert_eq!(parse_progress_percent("download: 100%"), Some(100.0));
        assert_eq!(
            parse_progress_percent("[download]   7.5% of 12.3MiB"),
            Some(7.5)
        );
        assert_eq!(parse_progress_percent("download:   0.0%"), Some(0.0));
        assert_eq!(
            parse_progress_percent("[download] Destination: video.mp4"),
            None
        );
        assert_eq!(parse_progress_percent(""), None);
        assert_eq!(parse_progress_percent("download: abc%"), None);
        assert_eq!(parse_progress_percent("download: 150.0%"), None);
    }

    #[cfg(unix)]
    fn tulis_skrip(dir: &Path, isi: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let path = dir.join("yt-dlp-palsu.sh");
        std::fs::write(&path, isi).unwrap();
        let mut izin = std::fs::metadata(&path).unwrap().permissions();
        izin.set_mode(0o755);
        std::fs::set_permissions(&path, izin).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn ytdlp_palsu_mengalirkan_progres_dan_sukses() {
        let dir = tempfile::tempdir().unwrap();
        let skrip = tulis_skrip(
            dir.path(),
            "#!/bin/sh\necho 'download:  10.0%'\necho 'download:  55.5%'\n\
             echo 'download: 100.0%'\nexit 0\n",
        );

        let mut job =
            DownloadJob::dengan_program(skrip.as_os_str(), &url_uji(), dir.path(), 720, 100);
        assert_eq!(job.args().len(), 16);
        job.start().unwrap();
        let laporan = job.tunggu(Duration::from_secs(10));

        let hasil = laporan.hasil.expect("harus selesai");
        assert!(hasil.sukses);
        assert!(!hasil.dibatalkan);
        assert_eq!(hasil.kode, Some(0));
        let persen: Vec<f32> = laporan.progres.iter().filter_map(|p| p.percent).collect();
        assert_eq!(persen, vec![10.0, 55.5, 100.0]);
    }

    #[cfg(unix)]
    #[test]
    fn ytdlp_palsu_gagal_dilaporkan() {
        let dir = tempfile::tempdir().unwrap();
        let skrip = tulis_skrip(
            dir.path(),
            "#!/bin/sh\necho 'gagal: video privat' >&2\nexit 2\n",
        );

        let mut job =
            DownloadJob::dengan_program(skrip.as_os_str(), &url_uji(), dir.path(), 720, 100);
        job.start().unwrap();
        let laporan = job.tunggu(Duration::from_secs(10));

        let hasil = laporan.hasil.expect("harus selesai");
        assert!(!hasil.sukses);
        assert_eq!(hasil.kode, Some(2));
        assert!(laporan.pesan.iter().any(|p| p.contains("video privat")));
    }

    #[cfg(unix)]
    #[test]
    fn unduhan_bisa_dibatalkan() {
        let dir = tempfile::tempdir().unwrap();
        let skrip = tulis_skrip(
            dir.path(),
            "#!/bin/sh\necho 'download:   1.0%'\nexec sleep 10\n",
        );

        let mut job =
            DownloadJob::dengan_program(skrip.as_os_str(), &url_uji(), dir.path(), 720, 100);
        job.start().unwrap();
        // Beri waktu proses hidup, lalu batalkan.
        let _ = job.terima(Duration::from_millis(300));
        job.batal();
        let laporan = job.tunggu(Duration::from_secs(5));

        let hasil = laporan.hasil.expect("harus selesai setelah dibatalkan");
        assert!(hasil.dibatalkan);
        assert!(!hasil.sukses);
    }

    #[test]
    fn program_tidak_ada_menghasilkan_pesan_jelas() {
        let dir = tempfile::tempdir().unwrap();
        let mut job =
            DownloadJob::dengan_program("/tidak/ada/yt-dlp", &url_uji(), dir.path(), 720, 100);
        job.start().unwrap();
        let laporan = job.tunggu(Duration::from_secs(5));
        assert!(laporan.hasil.is_none());
        assert!(laporan.pesan.iter().any(|p| p.contains("belum terpasang")));
    }
}
