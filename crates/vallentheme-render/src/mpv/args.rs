//! Penyusun argumen mpv.
//!
//! Fungsi [`build_args`] murni: tidak menyentuh proses, soket, atau jam. Semua
//! keputusan pemetaan pengaturan ke opsi mpv diuji di sini. Rujukan:
//! `panduan/06-referensi-teknis.md` bagian 2.

use std::ffi::OsString;

use vallentheme_core::model::FillMode;

use super::PlaySpec;

/// Opsi dasar yang selalu dikirim, dalam urutan yang disepakati.
pub const ARGUMEN_DASAR: [&str; 15] = [
    "--no-config",
    "--idle=yes",
    "--force-window=yes",
    "--loop-file=inf",
    "--no-osc",
    "--no-osd-bar",
    "--osd-level=0",
    "--no-input-default-bindings",
    "--no-input-cursor",
    "--cursor-autohide=no",
    "--really-quiet",
    "--keep-open=yes",
    "--hwdec=auto-safe",
    "--vo=gpu",
    "--framedrop=vo",
];

/// Susun seluruh argumen mpv untuk satu [`PlaySpec`].
///
/// Urutan: opsi dasar, `--wid`, `--input-ipc-server`, opsi pengisian layar,
/// rotasi/zoom/geser, audio, lalu `--` dan path media.
pub fn build_args(spec: &PlaySpec) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::with_capacity(32);

    push(&mut args, "--no-config");
    push(&mut args, &format!("--wid={}", spec.wid));
    push(
        &mut args,
        &format!("--input-ipc-server={}", spec.socket.display()),
    );
    for opsi in &ARGUMEN_DASAR[1..] {
        push(&mut args, opsi);
    }

    let s = &spec.settings;
    match s.fill {
        FillMode::Cover | FillMode::Tile => {
            push(&mut args, "--panscan=1.0");
            push(&mut args, "--keepaspect=yes");
        }
        FillMode::Fit => {
            push(&mut args, "--panscan=0.0");
            push(&mut args, "--keepaspect=yes");
        }
        FillMode::Center => {
            push(&mut args, "--video-unscaled=yes");
        }
        FillMode::Stretch => {
            push(&mut args, "--keepaspect=no");
        }
    }

    push(&mut args, &format!("--video-rotate={}", s.rotation));
    push(&mut args, &format!("--video-zoom={}", format_angka(s.zoom)));
    push(
        &mut args,
        &format!("--video-pan-x={}", format_angka(s.pan_x)),
    );
    push(
        &mut args,
        &format!("--video-pan-y={}", format_angka(s.pan_y)),
    );

    if s.mute {
        push(&mut args, "--no-audio");
    } else {
        push(&mut args, "--mute=no");
        push(&mut args, &format!("--volume={}", s.volume.min(100)));
    }

    push(&mut args, "--");
    args.push(spec.path.clone().into_os_string());
    args
}

/// Tambahkan satu argumen teks.
fn push(args: &mut Vec<OsString>, nilai: &str) {
    args.push(OsString::from(nilai));
}

/// Format angka pecahan tanpa nol berekor dan tanpa `-0`.
fn format_angka(nilai: f32) -> String {
    if !nilai.is_finite() {
        return "0".to_string();
    }
    let dibulatkan = (nilai * 1_000_000.0).round() / 1_000_000.0;
    if dibulatkan == 0.0 {
        return "0".to_string();
    }
    let teks = format!("{dibulatkan}");
    teks
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::PathBuf;

    use vallentheme_core::model::WallpaperSettings;

    use super::*;

    fn spec(fill: FillMode) -> PlaySpec {
        PlaySpec::new(
            0x1a2b_3c4d,
            PathBuf::from("/run/user/1000/vallentheme/mpv-HDMI-1.sock"),
            PathBuf::from("/home/pengguna/Video dengan spasi.mp4"),
            WallpaperSettings {
                fill,
                rotation: 90,
                zoom: 0.5,
                pan_x: -0.25,
                pan_y: 0.125,
                mute: true,
                volume: 70,
                ..WallpaperSettings::default()
            },
        )
    }

    fn teks(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn urutan_opsi_dasar_dan_identitas() {
        let hasil = teks(&build_args(&spec(FillMode::Cover)));
        assert_eq!(hasil[0], "--no-config");
        assert_eq!(hasil[1], "--wid=439041101");
        assert_eq!(
            hasil[2],
            "--input-ipc-server=/run/user/1000/vallentheme/mpv-HDMI-1.sock"
        );
        let sisa = &hasil[3..];
        assert_eq!(
            sisa,
            [
                "--idle=yes",
                "--force-window=yes",
                "--loop-file=inf",
                "--no-osc",
                "--no-osd-bar",
                "--osd-level=0",
                "--no-input-default-bindings",
                "--no-input-cursor",
                "--cursor-autohide=no",
                "--really-quiet",
                "--keep-open=yes",
                "--hwdec=auto-safe",
                "--vo=gpu",
                "--framedrop=vo",
                "--panscan=1.0",
                "--keepaspect=yes",
                "--video-rotate=90",
                "--video-zoom=0.5",
                "--video-pan-x=-0.25",
                "--video-pan-y=0.125",
                "--no-audio",
                "--",
                "/home/pengguna/Video dengan spasi.mp4",
            ]
        );
    }

    #[test]
    fn semua_mode_pengisian_dipetakan_benar() {
        let kasus = [
            (FillMode::Cover, vec!["--panscan=1.0", "--keepaspect=yes"]),
            (FillMode::Fit, vec!["--panscan=0.0", "--keepaspect=yes"]),
            (FillMode::Center, vec!["--video-unscaled=yes"]),
            (FillMode::Stretch, vec!["--keepaspect=no"]),
            (FillMode::Tile, vec!["--panscan=1.0", "--keepaspect=yes"]),
        ];

        for (mode, diharapkan) in kasus {
            let hasil = teks(&build_args(&spec(mode)));
            let awal = hasil
                .iter()
                .position(|a| a.starts_with("--video-rotate"))
                .unwrap();
            assert_eq!(
                &hasil[awal - diharapkan.len()..awal],
                diharapkan.as_slice(),
                "mode {mode:?} salah"
            );
        }
    }

    #[test]
    fn mode_center_tidak_memakai_keepaspect_atau_panscan() {
        let hasil = teks(&build_args(&spec(FillMode::Center)));
        assert!(!hasil.iter().any(|a| a.starts_with("--panscan")));
        assert!(!hasil.iter().any(|a| a.starts_with("--keepaspect")));
        assert!(hasil.iter().any(|a| a == "--video-unscaled=yes"));
    }

    #[test]
    fn mode_stretch_tidak_memakai_video_unscaled() {
        let hasil = teks(&build_args(&spec(FillMode::Stretch)));
        assert!(!hasil.iter().any(|a| a == "--video-unscaled=yes"));
        assert!(hasil.iter().any(|a| a == "--keepaspect=no"));
    }

    #[test]
    fn audio_aktif_memakai_volume_dan_tanpa_no_audio() {
        let mut s = spec(FillMode::Cover);
        s.settings.mute = false;
        s.settings.volume = 150;
        let hasil = teks(&build_args(&s));
        assert!(!hasil.iter().any(|a| a == "--no-audio"));
        assert!(hasil.iter().any(|a| a == "--mute=no"));
        assert!(hasil.iter().any(|a| a == "--volume=100"));
    }

    #[test]
    fn audio_bisu_memakai_no_audio() {
        let hasil = teks(&build_args(&spec(FillMode::Fit)));
        assert!(hasil.iter().any(|a| a == "--no-audio"));
        assert!(!hasil.iter().any(|a| a.starts_with("--volume")));
    }

    #[test]
    fn path_selalu_berada_setelah_pemisah() {
        let hasil = teks(&build_args(&spec(FillMode::Cover)));
        let posisi = hasil.iter().position(|a| a == "--").unwrap();
        assert_eq!(posisi, hasil.len() - 2);
        assert_eq!(
            hasil.last().unwrap(),
            "/home/pengguna/Video dengan spasi.mp4"
        );
    }

    #[test]
    fn nilai_liar_tidak_menghasilkan_nan() {
        let mut s = spec(FillMode::Cover);
        s.settings.zoom = f32::NAN;
        s.settings.pan_x = f32::INFINITY;
        let hasil = teks(&build_args(&s));
        assert!(hasil.iter().any(|a| a == "--video-zoom=0"));
        assert!(hasil.iter().any(|a| a == "--video-pan-x=0"));
    }
}
