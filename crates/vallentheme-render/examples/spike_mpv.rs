//! Spike Fase 1.2, 1.3, 1.7, 1.8 — menempelkan mpv ke window desktop,
//! mengendalikannya lewat IPC, memulihkannya saat mati, dan mengukur beban.
//!
//! ```text
//! cargo run -p vallentheme-render --example spike_mpv -- /tmp/uji.mp4 60 gpu auto-safe
//! ```
//!
//! Argumen: `<video> <detik> [vo] [hwdec]`

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use vallentheme_render::x11::X11Context;

/// Klien IPC mpv minimal: satu permintaan, satu balasan, cocok lewat `request_id`.
struct Ipc {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
    berikut: u64,
}

impl Ipc {
    fn connect(path: &Path, batas: Duration) -> anyhow::Result<Self> {
        let mulai = Instant::now();
        loop {
            match UnixStream::connect(path) {
                Ok(stream) => {
                    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                    let reader = BufReader::new(stream.try_clone()?);
                    return Ok(Self {
                        reader,
                        writer: stream,
                        berikut: 0,
                    });
                }
                Err(err) => {
                    if mulai.elapsed() > batas {
                        return Err(anyhow::anyhow!(
                            "soket IPC {} tidak muncul: {err}",
                            path.display()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }

    fn perintah(&mut self, argumen: Vec<Value>) -> anyhow::Result<Value> {
        self.berikut += 1;
        let id = self.berikut;
        let permintaan = json!({ "command": argumen, "request_id": id });
        writeln!(self.writer, "{permintaan}")?;
        self.writer.flush()?;

        loop {
            let mut baris = String::new();
            if self.reader.read_line(&mut baris)? == 0 {
                return Err(anyhow::anyhow!("mpv menutup soket IPC"));
            }
            let Ok(nilai) = serde_json::from_str::<Value>(&baris) else {
                continue;
            };
            if nilai.get("request_id").and_then(Value::as_u64) == Some(id) {
                if nilai.get("error").and_then(Value::as_str) == Some("success") {
                    return Ok(nilai.get("data").cloned().unwrap_or(Value::Null));
                }
                return Err(anyhow::anyhow!("mpv menolak perintah: {nilai}"));
            }
        }
    }

    fn properti(&mut self, nama: &str) -> anyhow::Result<Value> {
        self.perintah(vec![json!("get_property"), json!(nama)])
    }

    fn set(&mut self, nama: &str, nilai: Value) -> anyhow::Result<()> {
        self.perintah(vec![json!("set_property"), json!(nama), nilai])?;
        Ok(())
    }
}

/// Susun argumen mpv sesuai `panduan/06-referensi-teknis.md` bagian 2.
fn build_args(
    wid: u32,
    socket: &Path,
    video: &Path,
    vo: &str,
    hwdec: &str,
    log: &Path,
) -> Vec<String> {
    vec![
        "--no-config".into(),
        format!("--wid={wid}"),
        format!("--input-ipc-server={}", socket.display()),
        format!("--log-file={}", log.display()),
        "--idle=yes".into(),
        "--force-window=yes".into(),
        "--loop-file=inf".into(),
        "--no-osc".into(),
        "--no-osd-bar".into(),
        "--osd-level=0".into(),
        "--no-input-default-bindings".into(),
        "--no-input-cursor".into(),
        "--cursor-autohide=no".into(),
        "--really-quiet".into(),
        "--keep-open=yes".into(),
        format!("--hwdec={hwdec}"),
        format!("--vo={vo}"),
        "--framedrop=vo".into(),
        "--no-audio".into(),
        "--panscan=1.0".into(),
        "--keepaspect=yes".into(),
        "--".into(),
        video.display().to_string(),
    ]
}

/// Baca CPU (persen dari satu core) dan RSS (KB) sebuah proses dari `/proc`.
fn baca_proc(pid: u32) -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let tutup = stat.rfind(')')?;
    let sisa: Vec<&str> = stat[tutup + 1..].split_whitespace().collect();
    let utime: u64 = sisa.get(11)?.parse().ok()?;
    let stime: u64 = sisa.get(12)?.parse().ok()?;

    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let rss = status
        .lines()
        .find(|l| l.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;

    Some((utime + stime, rss))
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let video = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("pemakaian: spike_mpv <video> [detik] [vo] [hwdec]"))?;
    let durasi: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(30);
    let vo = args.next().unwrap_or_else(|| "gpu".to_string());
    let hwdec = args.next().unwrap_or_else(|| "auto-safe".to_string());

    if !video.is_file() {
        return Err(anyhow::anyhow!("berkas tidak ada: {}", video.display()));
    }

    let ctx = X11Context::connect()?;
    let monitors = ctx.monitors()?;
    let mon = monitors
        .iter()
        .max_by_key(|m| m.area())
        .ok_or_else(|| anyhow::anyhow!("tidak ada monitor"))?;
    println!("monitor: {} {}x{}", mon.name, mon.width, mon.height);

    let win = ctx.create_desktop_window(mon, 0x00000000, false)?;
    println!("window: 0x{win:x} ({win})");

    // Caja (MATE) membuat window desktop sendiri yang opaque; naikkan window
    // kita tepat di atasnya supaya wallpaper terlihat.
    match ctx.raise_above_other_desktop(win) {
        Ok(true) => println!("dinaikkan di atas window desktop lain (ikon desktop tertutup)"),
        Ok(false) => println!("tidak ada window desktop lain; wallpaper di lapisan bawah"),
        Err(err) => println!("gagal menaikkan window: {err}"),
    }

    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    let socket = PathBuf::from(&runtime).join("vallentheme-spike.sock");
    let _ = std::fs::remove_file(&socket);
    let log = PathBuf::from("/tmp/vallentheme-spike-mpv.log");

    let mut kegagalan = 0u32;
    let mut restart = 0u32;
    let batas_akhir = Instant::now() + Duration::from_secs(durasi);
    let mut sampel_cpu: Vec<f64> = Vec::new();
    let mut sampel_rss: Vec<u64> = Vec::new();
    let mut drop_awal: Option<i64> = None;
    let mut hwdec_terakhir = String::new();

    while Instant::now() < batas_akhir {
        let argumen = build_args(win, &socket, &video, &vo, &hwdec, &log);
        let mut anak: Child = match vallentheme_render::process::spawn_detached(
            "mpv",
            &argumen.iter().map(Into::into).collect::<Vec<_>>(),
        ) {
            Ok(anak) => anak,
            Err(err) => return Err(anyhow::anyhow!("gagal menjalankan mpv: {err}")),
        };
        let pid = anak.id();

        // Buang stderr agar mpv tidak memblokir; diagnostik ada di --log-file.
        if let Some(stderr) = anak.stderr.take() {
            std::thread::spawn(move || {
                let mut baris = String::new();
                let mut reader = BufReader::new(stderr);
                while let Ok(n) = reader.read_line(&mut baris) {
                    if n == 0 {
                        break;
                    }
                    baris.clear();
                }
            });
        }

        let mut ipc = match Ipc::connect(&socket, Duration::from_secs(10)) {
            Ok(ipc) => ipc,
            Err(err) => {
                println!("IPC gagal: {err}");
                let _ =
                    vallentheme_render::process::terminate_group(&mut anak, Duration::from_secs(1));
                kegagalan += 1;
                if kegagalan >= 3 {
                    anyhow::bail!("mpv tidak pernah siap setelah {kegagalan} percobaan");
                }
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
        };

        std::thread::sleep(Duration::from_secs(2));
        let hwdec_aktif = ipc
            .properti("hwdec-current")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "\"tidak tersedia\"".to_string());
        let lebar = ipc
            .properti("video-params/w")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "?".to_string());
        let fps = ipc
            .properti("estimated-vf-fps")
            .map(|v| v.to_string())
            .unwrap_or_else(|_| "?".to_string());
        hwdec_terakhir = hwdec_aktif.trim_matches('"').to_string();
        println!(
            "mpv siap: pid={pid} hwdec-current={hwdec_aktif} lebar={lebar} vf-fps={fps} vo={vo}"
        );

        // Uji kontrol IPC.
        ipc.set("pause", json!(true))?;
        let jeda = ipc.properti("pause")?;
        ipc.set("pause", json!(false))?;
        ipc.set("volume", json!(35))?;
        let volume = ipc.properti("volume")?;
        println!("IPC: pause={jeda} volume={volume}");

        let mulai = Instant::now();
        let mut sampel_terakhir = Instant::now();
        let mut ticks_awal: Option<u64> = None;

        while Instant::now() < batas_akhir {
            std::thread::sleep(Duration::from_millis(500));

            if let Ok(Some(status)) = anak.try_wait() {
                println!("mpv berhenti sendiri: {status}");
                kegagalan += 1;
                break;
            }

            if sampel_terakhir.elapsed() >= Duration::from_secs(1) {
                sampel_terakhir = Instant::now();
                if let Some((ticks, rss)) = baca_proc(pid) {
                    let awal = *ticks_awal.get_or_insert(ticks);
                    let dt = mulai.elapsed().as_secs_f64();
                    if dt > 1.0 {
                        let hz = 100.0;
                        let cpu = ((ticks - awal) as f64 / hz) / dt * 100.0;
                        sampel_cpu.push(cpu);
                        sampel_rss.push(rss);
                    }
                }
                if let Ok(drop) = ipc.properti("vo-drop-frame-count") {
                    if let Some(n) = drop.as_i64() {
                        drop_awal.get_or_insert(n);
                    }
                }
            }
        }

        let drop_akhir = ipc
            .properti("vo-drop-frame-count")
            .ok()
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let drop = drop_akhir - drop_awal.unwrap_or(drop_akhir);
        println!("frame drop sesi ini: {drop}");

        let masih = anak.try_wait().ok().flatten().is_none();
        if masih {
            vallentheme_render::process::terminate_group(&mut anak, Duration::from_secs(2))?;
        }
        let _ = std::fs::remove_file(&socket);

        if Instant::now() >= batas_akhir {
            break;
        }
        restart += 1;
        let jeda = Duration::from_secs(1u64 << restart.min(4));
        println!("mulai ulang ke-{restart} setelah {:?}", jeda);
        std::thread::sleep(jeda);
    }

    let rata_cpu = if sampel_cpu.is_empty() {
        0.0
    } else {
        sampel_cpu.iter().sum::<f64>() / sampel_cpu.len() as f64
    };
    let rss_akhir = sampel_rss.last().copied().unwrap_or(0);
    println!("--- HASIL ---");
    println!("vo={vo} hwdec={hwdec} hwdec-current={hwdec_terakhir}");
    println!("CPU rata-rata: {rata_cpu:.1}% (dari 1 core)");
    println!("RSS mpv: {} MB", rss_akhir / 1024);
    println!("jumlah restart: {restart}");

    ctx.destroy_window(win);
    Ok(())
}
