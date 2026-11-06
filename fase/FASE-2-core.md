# FASE 2 — Core: Data, Scanner, Thumbnail

**Baca dulu:** `panduan/00-aturan-agent.md`, `panduan/01-arsitektur.md` (bagian 2, 6–8), `panduan/06-referensi-teknis.md` (bagian 5, 7, 10).

## Tujuan
Pustaka inti **tanpa GTK/X11** yang teruji penuh: model data, SQLite dengan migrasi, scanner media, metadata, thumbnail, pencarian/sortir, dan config.

## Modul (crate `vallentheme-core`)
`model`, `db` (koneksi, migrasi, repositori), `scan`, `probe`, `thumb`, `search`, `settings` (struct `WallpaperSettings`), `error`.

## Langkah rinci

### 2.1 Model dan tipe
- `WallpaperId(i64)`, `FolderId(i64)` (newtype).
- `MediaKind { Image, Video }`.
- `FillMode { Cover, Fit, Center, Stretch, Tile }`, `Transition { None, Fade }`, `PowerPolicy { Default, Always, PauseFullscreen, PauseBattery, AlwaysSaver }`.
- `WallpaperSettings { fill, rotation(0|90|180|270), zoom(0.0..=2.0), pan_x(-0.5..=0.5), pan_y(-0.5..=0.5), transition, mute, volume(0..=100), power }` dengan `Default`, `validate()` (klem), serde ke JSON (`#[serde(default)]` agar toleran).
- `Wallpaper` (cermin tabel), `Folder`.
- `SortKey`, `Filter { query, folder, only_favorites, kind }`.

### 2.2 Database
- SQLite via `rusqlite`. Satu koneksi tulis dalam **aktor DB** (thread khusus + channel berbatas) agar tidak ada kunci tumpang tindih; pembacaan boleh koneksi terpisah (WAL).
- Skema persis di `panduan/06-referensi-teknis.md` bagian 7.
- Migrasi: daftar fungsi bernomor, dijalankan dalam transaksi; `meta.schema_version`. Tolak membuka DB dengan versi **lebih baru** dari yang dikenal (pesan jelas).
- Pemeriksaan integritas saat buka: `PRAGMA quick_check`. Bila gagal: cadangkan file ke `library.db.corrupt-<waktu>` dan buat DB baru (+ sinyal kepada pemanggil agar GUI memberi tahu pengguna).
- Repositori: `insert_or_update_wallpaper`, `get`, `list(filter, sort, limit, offset)`, `count`, `set_favorite`, `set_settings`, `mark_used`, `soft_delete`, `restore`, `purge_deleted`, `mark_missing(ids)`, folder CRUD.
- `sort_index`: bilangan yang naik untuk item baru (urutan "Urutan saya").

### 2.3 Probe (ffprobe)
- Jalankan ffprobe tanpa shell, timeout 15 detik, batasi ukuran keluaran.
- Parse JSON → `MediaInfo { kind, width, height, fps, duration_ms, codec, has_audio, size_bytes }`.
- `avg_frame_rate` berbentuk pecahan; tangani `0/0`.
- Gambar animasi (GIF/WebP animasi): jika ffprobe menunjukkan > 1 frame → `Video`.
- Ekstensi dukungan (awal): video `mp4 mkv webm mov avi m4v gif`; foto `jpg jpeg png webp bmp tiff avif`. File di luar daftar ditolak dengan alasan `UNSUPPORTED`.

### 2.4 Scanner
- `scan_files(paths)` dan `scan_folder(path, recursive, kinds)`.
- Kedalaman maks 8, deteksi loop symlink (simpan `(dev, inode)` terlihat), abaikan file tersembunyi dan folder `.git`/`node_modules`.
- Berjalan di worker pool (`min(core, 2)`), mengirim hasil lewat **channel berbatas** dalam batch (untuk dipakai GUI). Mendukung pembatalan (`AtomicBool`/token).
- Idempoten: path sama → perbarui metadata dan `mtime`, **jangan** buat entri baru.
- Mengembalikan ringkasan: `{ditambah, diperbarui, dilewati: Vec<(path, alasan)>}`.

### 2.5 Thumbnail
- Hash kunci: `blake3(path + mtime + size)` → nama file `<hash>.jpg` di cache.
- Gunakan perintah di `panduan/06-referensi-teknis.md` bagian 5; timeout 20 detik; hapus file keluaran bila gagal.
- Lebar 480 (`thumb`), dan fungsi `preview_frame(id, width=960)` untuk layar Edit.
- Pemilihan waktu frame video: `max(1s, 10% durasi)`, bila durasi < 1 s gunakan 0.
- Antrean prioritas (item terlihat lebih dulu) dengan batas paralel; dedupe permintaan yang sama.
- Pembersihan cache: hapus thumbnail yatim, patuhi `cache_limit_mb` (hapus yang paling lama dipakai).

### 2.6 Pencarian dan sortir
- Pencarian: pisah token, cocokkan (tanpa membedakan huruf, normalisasi aksen sederhana) terhadap judul/folder; token khusus `video`, `foto`, `4k`, `fav`.
- Sortir sesuai `SortKey` (`panduan/03-gui-layar-library.md` bagian 4). Urutan stabil (tie-break `id`).
- Lakukan di SQL bila cepat; jika tidak, di memori (2.000 item harus < 20 ms).

### 2.7 Pemeriksaan file hilang
`verify_files(ids)` memeriksa keberadaan/izin baca, memperbarui `missing`. Dipanggil lambat (di latar) dan saat folder diubah.

### 2.8 Katalog error awal
Enum `Error` dengan kode `VT-IO-*`, `VT-DB-*`, `VT-PROBE-*`, `VT-THUMB-*`, `VT-CONFIG-*`, `VT-UNSUPPORTED-*`. Tiap varian punya `code()` dan `user_message()` (Indonesia, ramah). Daftar lengkap disusun bertahap, final di Fase 9.

## Kasus tepi yang wajib diuji
- File 0 byte, file rusak (header palsu), file `.mp4` yang sebenarnya teks.
- Nama file dengan spasi, tanda kutip, emoji, newline.
- Path sangat panjang; izin baca ditolak; file dihapus saat dipindai.
- Folder dengan 10.000 file; symlink loop; folder tak terbaca.
- Video tanpa durasi (stream live), fps variabel, fps `0/0`.
- ffprobe/ffmpeg tidak terpasang → error jelas `VT-PROBE-001` (bukan panic).
- Dua proses membuka DB bersamaan (WAL) – baca tetap jalan.
- DB korup, DB versi lebih baru, DB dikunci.
- Pembatalan scan di tengah jalan: tidak ada entri setengah jadi.

## Tes
- Unit test untuk semua fungsi publik.
- Integrasi memakai `tests/fixtures/` (video 2 detik 320×180, foto kecil, file rusak). Fixture kecil (< 500 KB total) dibuat dengan ffmpeg dan di-commit.
- Benchmark sederhana (test `#[ignore]`): impor 500 file dummy.

## Gate
- [ ] Semua fungsi publik core punya test
- [ ] Impor 500 file dummy: memori proses < 200 MB, tidak ada thread menggantung, selesai tanpa error
- [ ] File rusak/asing ditolak dengan error berkode, tidak ada panic
- [ ] Migrasi dari skema v1 ke skema berikutnya teruji (buat migrasi `m002` dummy untuk membuktikan mekanismenya, lalu hapus)
- [ ] DB korup dipulihkan dengan aman (file lama dicadangkan)
- [ ] Pembatalan scan teruji
- [ ] Pencarian + sortir 2.000 item < 20 ms

## Artefak
Crate `core` lengkap + test + fixture.

## Jangan lakukan
Memakai GTK/GLib di crate ini. Menghapus file asli pengguna. Memanggil ffmpeg/ffprobe lewat shell.
