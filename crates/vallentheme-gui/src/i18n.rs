//! Penerjemahan antarmuka.
//!
//! Kunci adalah string Inggris; nilai adalah terjemahan Indonesia. Semua teks
//! yang tampil ke pengguna **wajib** lewat `tr!`.
//!
//! Catatan: mekanisme ini menggantikan gettext untuk sementara (lihat
//! `docs/PROGRESS.md` Fase 3). API `tr!` sengaja dibuat sama supaya bisa
//! ditukar ke gettext tanpa mengubah pemanggil.

/// Pasangan kunci Inggris → teks Indonesia.
pub const TERJEMAHAN: &[(&str, &str)] = &[
    ("vallentheme", "vallentheme"),
    ("Running", "Berjalan"),
    ("Not running", "Tidak berjalan"),
    ("Paused", "Dijeda"),
    ("Degraded", "Terbatas"),
    ("Failed", "Gagal"),
    (
        "Search wallpapers or folders…",
        "Cari wallpaper atau folder…",
    ),
    ("All wallpapers", "Semua wallpaper"),
    ("Favorites", "Favorit"),
    ("My order", "Urutan saya"),
    ("Recent", "Terbaru"),
    ("Video", "Video"),
    ("Photo", "Foto"),
    ("No wallpapers yet", "Belum ada wallpaper"),
    (
        "Add a photo or video to get started",
        "Tambahkan foto atau video untuk memulai",
    ),
    ("Add folder", "Tambah folder"),
    ("From link", "Dari tautan"),
    ("Add", "Tambah"),
    ("Select", "Pilih"),
    ("Preferences", "Pengaturan"),
    ("About vallentheme", "Tentang vallentheme"),
    ("Stop wallpaper", "Hentikan wallpaper"),
    ("Quit", "Keluar"),
    ("Quit and stop daemon", "Keluar dan hentikan daemon"),
    ("Edit wallpaper", "Edit wallpaper"),
    ("Set as wallpaper", "Jadikan wallpaper"),
    ("Rotate 90°", "Putar 90°"),
    ("Reset crop", "Setel ulang pangkas"),
    ("Fill method", "Metode pengisian"),
    ("Transition", "Transisi"),
    ("Mute", "Bisu"),
    ("Power saving", "Hemat daya"),
    ("Default", "Bawaan"),
    ("Coming soon", "Segera hadir"),
    ("Missing", "HILANG"),
    ("wallpapers", "wallpaper"),
    (
        "This session is not supported yet",
        "Sesi ini belum didukung",
    ),
    (
        "Wayland is not fully supported. Please log in with an X11 session.",
        "Wayland belum didukung penuh. Silakan masuk dengan sesi X11.",
    ),
    ("Copy debug info", "Salin info debug"),
    ("License not decided yet", "Lisensi belum ditentukan"),
];

/// Terjemahkan satu kunci. Kunci tak dikenal dikembalikan apa adanya.
pub fn tr(kunci: &str) -> String {
    TERJEMAHAN
        .iter()
        .find(|(k, _)| *k == kunci)
        .map(|(_, v)| (*v).to_string())
        .unwrap_or_else(|| kunci.to_string())
}

/// Apakah sebuah kunci punya terjemahan Indonesia.
pub fn punya_terjemahan(kunci: &str) -> bool {
    TERJEMAHAN.iter().any(|(k, _)| *k == kunci)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::collections::BTreeSet;

    /// Kumpulkan semua kunci yang dipakai lewat pemanggilan tr di seluruh sumber.
    fn kunci_dipakai() -> BTreeSet<String> {
        let akar = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut hasil = BTreeSet::new();
        kumpulkan(&akar, &mut hasil);
        hasil
    }

    fn kumpulkan(dir: &std::path::Path, hasil: &mut BTreeSet<String>) {
        let Ok(entri) = std::fs::read_dir(dir) else {
            return;
        };
        for entri in entri.flatten() {
            let path = entri.path();
            if path.is_dir() {
                kumpulkan(&path, hasil);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            // Berkas ini memuat tabel terjemahan dan contoh uji; jangan dipindai.
            if path.file_name().and_then(|n| n.to_str()) == Some("i18n.rs") {
                continue;
            }
            let Ok(isi) = std::fs::read_to_string(&path) else {
                continue;
            };
            for kunci in ekstrak(&isi) {
                hasil.insert(kunci);
            }
        }
    }

    /// Ambil argumen literal dari setiap pemanggilan `tr("...")` / `tr!("...")`.
    fn ekstrak(isi: &str) -> Vec<String> {
        let mut hasil = Vec::new();
        let mut i = 0usize;
        while let Some(pos) = isi[i..].find("tr") {
            let mut j = i + pos + 2;
            if isi[j..].starts_with('!') {
                j += 1;
            }
            if isi[j..].starts_with('(') {
                j += 1;
                if isi[j..].starts_with('"') {
                    j += 1;
                    if let Some(tutup) = isi[j..].find('"') {
                        hasil.push(isi[j..j + tutup].to_string());
                        i = j + tutup + 1;
                        continue;
                    }
                }
            }
            i += pos + 2;
        }
        hasil
    }

    #[test]
    fn setiap_string_ui_punya_terjemahan() {
        let dipakai = kunci_dipakai();
        assert!(
            !dipakai.is_empty(),
            "tidak ada pemakaian tr! terdeteksi; pemeriksaan i18n tidak bermakna"
        );
        let hilang: Vec<&String> = dipakai.iter().filter(|k| !punya_terjemahan(k)).collect();
        assert!(hilang.is_empty(), "kunci tanpa terjemahan: {hilang:?}");
    }

    #[test]
    fn kunci_terjemahan_unik() {
        let mut terlihat = BTreeSet::new();
        for (k, _) in TERJEMAHAN {
            assert!(terlihat.insert(*k), "kunci ganda: {k}");
        }
    }

    #[test]
    fn tidak_ada_terjemahan_kosong() {
        for (k, v) in TERJEMAHAN {
            assert!(!v.trim().is_empty(), "terjemahan kosong untuk {k}");
        }
    }

    #[test]
    fn kunci_tak_dikenal_dikembalikan_apa_adanya() {
        assert_eq!(tr("Kunci Aneh"), "Kunci Aneh");
    }

    #[test]
    fn ekstraksi_kunci_bekerja() {
        let contoh = r#"let a = tr!("Running"); let b = tr!("Quit");"#;
        assert_eq!(
            ekstrak(contoh),
            vec!["Running".to_string(), "Quit".to_string()]
        );
    }
}
