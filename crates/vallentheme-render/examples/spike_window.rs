//! Spike Fase 1.1 & 1.4 — window desktop minimal di X11.
//!
//! Membuat window seukuran monitor terbesar, menandainya sebagai DESKTOP,
//! melukisnya, lalu menahannya selama beberapa detik supaya bisa diperiksa
//! dari luar:
//!
//! ```text
//! cargo run -p vallentheme-render --example spike_window -- 30
//! xwininfo -root -tree | grep -i vallentheme
//! xprop -id <XID> _NET_WM_WINDOW_TYPE _NET_WM_STATE _NET_WM_DESKTOP
//! ```
//!
//! Variabel lingkungan `SPIKE_ABOVE=1` menaikkan window di atas window desktop
//! Caja (untuk memastikan isinya benar-benar dilukis).

use std::thread::sleep;
use std::time::{Duration, Instant};

use vallentheme_render::x11::X11Context;

/// Warna uji: biru gelap `#1A2430`, mudah dibedakan dari latar desktop.
const WARNA: u32 = 0x001A_2430;

fn main() -> anyhow::Result<()> {
    let durasi = std::env::args()
        .nth(1)
        .and_then(|nilai| nilai.parse::<u64>().ok())
        .unwrap_or(20);
    let naikkan = std::env::var("SPIKE_ABOVE").is_ok();

    let ctx = X11Context::connect()?;
    let monitors = ctx.monitors()?;

    println!("monitor terdeteksi: {}", monitors.len());
    for mon in &monitors {
        println!(
            "  {:<10} {}x{}{} pada +{},+{}",
            mon.name,
            mon.width,
            mon.height,
            if mon.primary { " (utama)" } else { "" },
            mon.x,
            mon.y
        );
    }

    let mon = monitors
        .iter()
        .max_by_key(|m| m.area())
        .ok_or_else(|| anyhow::anyhow!("tidak ada monitor"))?;

    let win = ctx.create_desktop_window(mon, WARNA, false)?;
    ctx.fill_solid(win, WARNA, mon.width, mon.height)?;
    println!("window desktop: XID=0x{win:x} desimal={win}");

    if naikkan {
        ctx.raise_above(win)?;
        println!("SPIKE_ABOVE: window dinaikkan di atas window desktop lain");
    }

    println!("menahan window {durasi} detik…");
    let batas = Instant::now() + Duration::from_secs(durasi);
    while Instant::now() < batas {
        if ctx.drain_events_had_expose() {
            ctx.fill_solid(win, WARNA, mon.width, mon.height)?;
        }
        sleep(Duration::from_millis(100));
    }

    ctx.destroy_window(win);
    println!("window dihancurkan; selesai");
    Ok(())
}
