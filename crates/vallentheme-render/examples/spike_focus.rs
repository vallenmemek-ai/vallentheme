//! Spike Fase 1.6 — deteksi jendela fullscreen/maximize.
//!
//! Memantau `_NET_ACTIVE_WINDOW` pada window root lalu membaca `_NET_WM_STATE`
//! dari jendela yang aktif. Setiap perubahan dicetak beserta latensinya.
//!
//! ```text
//! cargo run -p vallentheme-render --example spike_focus -- 60
//! ```

use std::thread::sleep;
use std::time::{Duration, Instant};

use vallentheme_render::x11::X11Context;

fn main() -> anyhow::Result<()> {
    let durasi = std::env::args()
        .nth(1)
        .and_then(|nilai| nilai.parse::<u64>().ok())
        .unwrap_or(60);

    let ctx = X11Context::connect()?;
    ctx.watch_root_properties()?;
    println!("memantau _NET_ACTIVE_WINDOW selama {durasi} detik…");

    let batas = Instant::now() + Duration::from_secs(durasi);
    let mut terakhir = String::new();
    let mut jumlah_perubahan = 0u32;

    while Instant::now() < batas {
        let Some((atom, _window)) = ctx.poll_property_notify() else {
            sleep(Duration::from_millis(20));
            continue;
        };
        if atom != "_NET_ACTIVE_WINDOW" && atom != "_NET_CLIENT_LIST_STACKING" {
            continue;
        }

        let waktu = Instant::now();
        let aktif = ctx.active_window()?;
        let status = match aktif {
            None => "tidak ada jendela aktif".to_string(),
            Some(win) => {
                let state = ctx.wm_state(win).unwrap_or_default();
                let penuh = state.iter().any(|s| s.ends_with("FULLSCREEN"));
                let maks = state.iter().any(|s| s.ends_with("MAXIMIZED_VERT"))
                    && state.iter().any(|s| s.ends_with("MAXIMIZED_HORZ"));
                let label = if penuh {
                    "FULLSCREEN"
                } else if maks {
                    "MAXIMIZE"
                } else {
                    "normal"
                };
                format!("0x{win:x} {label}")
            }
        };

        if status != terakhir {
            jumlah_perubahan += 1;
            println!(
                "[{:>6.1} ms] {atom} -> {status}",
                waktu.elapsed().as_secs_f64() * 1000.0
            );
            terakhir = status;
        }
    }

    println!("selesai; {jumlah_perubahan} perubahan status terdeteksi");
    Ok(())
}
