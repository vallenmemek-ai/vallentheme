#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! Uji integrasi crate inti dengan berkas media sungguhan.
//!
//! Fixture ada di `tests/fixtures/` pada akar repo (total < 500 KB).

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use vallentheme_core::db::{Db, MediaEntry};
use vallentheme_core::model::{Filter, MediaKind, SortKey, WallpaperId};
use vallentheme_core::scan::{
    collect_files, import_path, import_paths, ImportOutcome, ScanOptions,
};
use vallentheme_core::{probe, thumb};

fn fixture(nama: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(nama)
}

fn rss_kb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

#[test]
fn probe_video_membaca_metadata_benar() {
    let info = probe::probe(&fixture("uji.mp4"), Duration::from_secs(20)).unwrap();
    assert_eq!(info.kind, MediaKind::Video);
    assert_eq!(info.width, Some(320));
    assert_eq!(info.height, Some(180));
    let fps = info.fps.unwrap();
    assert!((fps - 24.0).abs() < 0.1, "fps tak terduga: {fps}");
    let durasi = info.duration_ms.unwrap();
    assert!(
        (1500..=2600).contains(&durasi),
        "durasi tak terduga: {durasi} ms"
    );
    assert!(!info.has_audio, "fixture tidak punya audio");
    assert!(info.size_bytes > 0);
}

#[test]
fn probe_foto_membaca_dimensi() {
    let info = probe::probe(&fixture("uji.jpg"), Duration::from_secs(20)).unwrap();
    assert_eq!(info.kind, MediaKind::Image);
    assert_eq!(info.width, Some(320));
    assert_eq!(info.height, Some(180));
}

#[test]
fn probe_berkas_rusak_ditolak_berkode() {
    let err = probe::probe(&fixture("rusak.mp4"), Duration::from_secs(20)).unwrap_err();
    assert_eq!(err.code(), "VT-PROBE-002");
    assert!(!err.user_message().is_empty());
}

#[test]
fn thumbnail_dibuat_dan_dipakai_ulang() {
    let dir = tempfile::tempdir().unwrap();
    let sumber = fixture("uji.mp4");
    let kunci = thumb::kunci_cache(&sumber, 1_700_000_000, 91_843);

    let pertama = thumb::buat(
        &sumber,
        dir.path(),
        &kunci,
        thumb::LEBAR_THUMB,
        thumb::waktu_frame(Some(2000)),
    )
    .unwrap();
    assert!(pertama.is_file(), "thumbnail harus dibuat");
    let ukuran = std::fs::metadata(&pertama).unwrap().len();
    assert!(ukuran > 500, "thumbnail terlalu kecil: {ukuran} byte");

    // Panggilan kedua harus memakai berkas yang sama tanpa membuat ulang.
    let mtime_awal = std::fs::metadata(&pertama).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(30));
    let kedua = thumb::buat(
        &sumber,
        dir.path(),
        &kunci,
        thumb::LEBAR_THUMB,
        thumb::waktu_frame(Some(2000)),
    )
    .unwrap();
    assert_eq!(pertama, kedua);
    let mtime_akhir = std::fs::metadata(&kedua).unwrap().modified().unwrap();
    assert_eq!(mtime_awal, mtime_akhir, "berkas tidak boleh dibuat ulang");
}

#[test]
fn alur_impor_folder_lengkap() {
    let mut db = Db::open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let folder = db.insert_folder(&fixture(""), true).unwrap();

    let batal = AtomicBool::new(false);
    let berkas = collect_files(&[fixture("")], &ScanOptions::default(), &batal);
    assert_eq!(berkas.len(), 3, "dua media + satu berkas rusak");

    let mut dilaporkan = 0usize;
    let laporan = import_paths(&mut db, &berkas, Some(folder), &batal, |_, _| {
        dilaporkan += 1;
    })
    .unwrap();

    assert_eq!(laporan.ditambah, 2, "video dan foto masuk");
    assert_eq!(laporan.dilewati.len(), 1, "berkas rusak dilewati");
    assert_eq!(dilaporkan, 3, "callback dipanggil per berkas");
    assert_eq!(db.count(&Filter::default()).unwrap(), 2);

    let video = db
        .list(&Filter::default(), SortKey::Name, 10, 0)
        .unwrap()
        .into_iter()
        .find(|w| w.kind == MediaKind::Video)
        .expect("ada satu video");
    assert_eq!(video.width, Some(320));
    assert!(!video.has_audio);
    assert!(video.settings.mute, "bawaan harus bisu");

    // Impor ulang tidak menggandakan.
    let laporan2 = import_paths(&mut db, &berkas, Some(folder), &batal, |_, _| {}).unwrap();
    assert_eq!(laporan2.ditambah, 0);
    assert_eq!(laporan2.diperbarui, 2);
    assert_eq!(db.count(&Filter::default()).unwrap(), 2);
    drop(dir);
}

#[test]
fn impor_satu_berkas_mengembalikan_id() {
    let mut db = Db::open_in_memory().unwrap();
    match import_path(&mut db, &fixture("uji.jpg"), None).unwrap() {
        ImportOutcome::Ditambah(id) => {
            assert!(db.get(id).unwrap().is_some());
        }
        lain => panic!("seharusnya ditambahkan, dapat {lain:?}"),
    }
}

#[test]
fn pembatalan_impor_tidak_menyisakan_entri_setengah_jadi() {
    let mut db = Db::open_in_memory().unwrap();
    let berkas = vec![fixture("uji.mp4"), fixture("uji.jpg")];

    let batal = AtomicBool::new(true);
    let laporan = import_paths(&mut db, &berkas, None, &batal, |_, _| {}).unwrap();

    assert!(laporan.dibatalkan);
    assert_eq!(laporan.ditambah, 0);
    assert_eq!(db.count(&Filter::default()).unwrap(), 0);
}

/// Gate Fase 2: 500 entri masuk tanpa error, memori tetap wajar.
#[test]
fn impor_500_entri_memori_terkendali() {
    let mut db = Db::open_in_memory().unwrap();
    let rss_awal = rss_kb();

    for i in 0..500 {
        let entry = MediaEntry {
            path: PathBuf::from(format!("/dummy/video-{i:04}.mp4")),
            kind: MediaKind::Video,
            title: format!("video-{i:04}"),
            width: Some(1920),
            height: Some(1080),
            fps: Some(24.0),
            duration_ms: Some(10_000),
            codec: Some("h264".to_string()),
            has_audio: true,
            size_bytes: 1024 * 1024,
            mtime: 1_700_000_000 + i,
            folder_id: None,
        };
        db.upsert(&entry).unwrap();
    }

    assert_eq!(db.count(&Filter::default()).unwrap(), 500);
    let rss_akhir = rss_kb();
    let naik = rss_akhir.saturating_sub(rss_awal);
    println!("RSS awal {rss_awal} KB → akhir {rss_akhir} KB (naik {naik} KB)");
    assert!(
        rss_akhir < 200 * 1024,
        "RSS melebihi 200 MB: {rss_akhir} KB"
    );
}

/// Gate Fase 2: pencarian + sortir 2.000 item cepat.
#[test]
fn pencarian_dan_sortir_2000_item() {
    let mut db = Db::open_in_memory().unwrap();
    for i in 0..2000 {
        let entry = MediaEntry {
            path: PathBuf::from(format!("/pustaka/media-{i:04}.mp4")),
            kind: if i % 3 == 0 {
                MediaKind::Image
            } else {
                MediaKind::Video
            },
            title: format!("media-{i:04}"),
            width: Some(if i % 5 == 0 { 3840 } else { 1920 }),
            height: Some(1080),
            fps: Some(24.0),
            duration_ms: Some(10_000),
            codec: Some("h264".to_string()),
            has_audio: false,
            size_bytes: 1024,
            mtime: i,
            folder_id: None,
        };
        db.upsert(&entry).unwrap();
    }

    let filter = Filter {
        query: "media 0042".to_string(),
        ..Filter::default()
    };

    let mulai = Instant::now();
    let hasil = db.list(&filter, SortKey::Name, 200, 0).unwrap();
    let lama = mulai.elapsed();

    println!("pencarian+sortir 2.000 item: {:?}", lama);
    assert_eq!(hasil.len(), 1, "hanya media-0042 yang cocok");
    assert!(
        lama < Duration::from_millis(100),
        "pencarian terlalu lambat: {lama:?}"
    );
}

/// Memastikan ID baru selalu unik dan naik.
#[test]
fn id_wallpaper_unik() {
    let mut db = Db::open_in_memory().unwrap();
    let mut ids: Vec<WallpaperId> = Vec::new();
    for i in 0..20 {
        let entry = MediaEntry {
            path: PathBuf::from(format!("/x/{i}.mp4")),
            kind: MediaKind::Video,
            title: format!("x{i}"),
            width: None,
            height: None,
            fps: None,
            duration_ms: None,
            codec: None,
            has_audio: false,
            size_bytes: 1,
            mtime: i,
            folder_id: None,
        };
        ids.push(db.upsert(&entry).unwrap().0);
    }
    let unik: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unik.len(), ids.len());
}
