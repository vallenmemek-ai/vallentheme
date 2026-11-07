//! Klien D-Bus untuk GUI.
//!
//! Memakai API blocking zbus dan **selalu dipanggil dari thread pekerja**, bukan
//! dari main loop GTK. Setiap panggilan membuka koneksi singkat agar tidak ada
//! state yang perlu dijaga saat daemon mati/restart.

/// Nama bus daemon.
pub const NAMA_BUS: &str = "id.vallentheme.Daemon";
/// Path objek daemon.
pub const PATH_OBJEK: &str = "/id/vallentheme/Daemon";
/// Interface daemon.
pub const INTERFACE: &str = "id.vallentheme.Daemon1";

/// Kirim `Apply` dengan `request_json`; kembalikan `result_json`.
pub fn apply(request_json: &str) -> Result<String, String> {
    let koneksi = zbus::blocking::Connection::session()
        .map_err(|e| format!("tidak bisa terhubung ke bus sesi: {e}"))?;
    let balasan = koneksi
        .call_method(
            Some(NAMA_BUS),
            PATH_OBJEK,
            Some(INTERFACE),
            "Apply",
            &(request_json,),
        )
        .map_err(|e| format!("Apply gagal: {e}"))?;
    balasan
        .body()
        .deserialize()
        .map_err(|e| format!("balasan Apply tidak bisa dibaca: {e}"))
}

/// Ambil `status_json` dari daemon.
pub fn status() -> Result<String, String> {
    let koneksi = zbus::blocking::Connection::session()
        .map_err(|e| format!("tidak bisa terhubung ke bus sesi: {e}"))?;
    let balasan = koneksi
        .call_method(
            Some(NAMA_BUS),
            PATH_OBJEK,
            Some(INTERFACE),
            "GetStatus",
            &(),
        )
        .map_err(|e| format!("GetStatus gagal: {e}"))?;
    balasan
        .body()
        .deserialize()
        .map_err(|e| format!("status tidak bisa dibaca: {e}"))
}

/// Hentikan wallpaper. `monitor` kosong berarti semua.
pub fn stop(monitor: &str) -> Result<bool, String> {
    let koneksi = zbus::blocking::Connection::session()
        .map_err(|e| format!("tidak bisa terhubung ke bus sesi: {e}"))?;
    let balasan = koneksi
        .call_method(
            Some(NAMA_BUS),
            PATH_OBJEK,
            Some(INTERFACE),
            "Stop",
            &(monitor,),
        )
        .map_err(|e| format!("Stop gagal: {e}"))?;
    balasan
        .body()
        .deserialize()
        .map_err(|e| format!("balasan Stop tidak bisa dibaca: {e}"))
}

/// Minta daemon keluar.
pub fn quit() -> Result<(), String> {
    let koneksi = zbus::blocking::Connection::session()
        .map_err(|e| format!("tidak bisa terhubung ke bus sesi: {e}"))?;
    koneksi
        .call_method(Some(NAMA_BUS), PATH_OBJEK, Some(INTERFACE), "Quit", &())
        .map_err(|e| format!("Quit gagal: {e}"))?;
    Ok(())
}

/// Berlangganan sinyal `StateChanged` dan kirim setiap `status_json` lewat kanal.
///
/// Thread ini mencoba ulang tiap 2 detik bila daemon belum ada, dan mengirim
/// status `idle` saat daemon hilang supaya chip tidak basi.
pub fn pantau_status() -> std::sync::mpsc::Receiver<String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let idle = r#"{"v":1,"state":"idle","monitors":[]}"#.to_string();
        let kirim = |json: String| tx.send(json).is_ok();
        loop {
            let langganan = zbus::blocking::Connection::session()
                .ok()
                .and_then(|koneksi| {
                    zbus::blocking::Proxy::new(&koneksi, NAMA_BUS, PATH_OBJEK, INTERFACE)
                        .ok()
                        .and_then(|proxy| proxy.receive_signal("StateChanged").ok())
                });

            let Some(langganan) = langganan else {
                if !kirim(idle.clone()) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
                continue;
            };

            // Status awal diambil setelah langganan aktif agar tidak ada perubahan
            // yang terlewat di antara keduanya.
            match status() {
                Ok(json) => {
                    if !kirim(json) {
                        return;
                    }
                }
                Err(_) => {
                    if !kirim(idle.clone()) {
                        return;
                    }
                }
            }

            for pesan in langganan {
                match pesan.body().deserialize::<String>() {
                    Ok(json) => {
                        if !kirim(json) {
                            return;
                        }
                    }
                    Err(err) => tracing::warn!(error = %err, "status dari daemon tidak terbaca"),
                }
            }

            if !kirim(idle.clone()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });
    rx
}

/// Apakah daemon sedang berjalan di bus.
pub fn tersedia() -> bool {
    match zbus::blocking::Connection::session() {
        Ok(koneksi) => koneksi
            .call_method(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                Some("org.freedesktop.DBus"),
                "NameHasOwner",
                &(NAMA_BUS,),
            )
            .ok()
            .and_then(|b| b.body().deserialize::<bool>().ok())
            .unwrap_or(false),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn konstanta_dbus_sesuai_panduan() {
        assert_eq!(NAMA_BUS, "id.vallentheme.Daemon");
        assert_eq!(PATH_OBJEK, "/id/vallentheme/Daemon");
        assert_eq!(INTERFACE, "id.vallentheme.Daemon1");
    }

    #[test]
    fn tersedia_tidak_panic_tanpa_daemon() {
        // Hanya memastikan tidak panic; nilai benar/salah tergantung lingkungan.
        let _ = tersedia();
    }
}
