# 00 — Aturan Agent

Dokumen ini mengikat. Bila ada konflik dengan kebiasaan umum, dokumen ini yang menang.

## 1. Proses kerja

1. Satu fase = satu branch git: `fase-N-nama-singkat` (contoh `fase-2-core`).
2. Commit kecil dan sering dengan format Conventional Commits:
   `feat(core): tambah scanner folder`, `fix(daemon): pulihkan mpv setelah crash`, `docs: ...`, `test: ...`, `chore: ...`.
3. Merge ke `main` hanya jika semua Gate fase lulus dan `scripts/check.sh` hijau.
4. Jangan menambah fitur di luar fase. Ide baru ditulis di `docs/IDEAS.md`.
5. Jangan memperbarui versi dependensi tanpa alasan. `Cargo.lock` wajib di-commit.
6. Di akhir fase, isi `docs/PROGRESS.md` (format ada di README).
7. Jangan mengubah file di `panduan/` atau `fase/`. Bila dokumen perlu diubah, tulis BLOKIR.

## 2. Kualitas kode

### Wajib
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` harus hijau.
- **Dilarang** `unwrap()`, `expect()`, `panic!`, `unreachable!()` yang bisa tercapai, indexing array yang bisa out-of-range, dan `as` untuk konversi angka yang bisa terpotong (pakai `try_from`). Di test boleh.
- Aktifkan lint workspace: `unsafe_code = "forbid"` kecuali di modul kecil yang jelas diberi komentar alasan (misalnya `pre_exec`/`prctl`), `clippy::unwrap_used = "deny"`, `clippy::expect_used = "deny"`, `clippy::panic = "deny"` untuk crate non-test.
- Error: `thiserror` di crate library (`core`, `render`), `anyhow` hanya di `main` binary. Setiap error yang sampai ke pengguna punya **kode** dan **pesan Indonesia yang ramah** (lihat katalog di Fase 9).
- Logging: `tracing` (`error!`, `warn!`, `info!`, `debug!`). Jangan `println!` di kode produksi.
- Tipe kuat: gunakan `newtype` untuk ID (`WallpaperId(i64)`), enum untuk mode (`FillMode`, `PerfMode`), bukan string bebas.
- Fungsi publik diberi `///` satu-dua kalimat. Modul diberi `//!`.
- Tidak ada fungsi > ~80 baris tanpa alasan. Pecah.

### Konkurensi
- **GUI**: semua pekerjaan berat lewat `gio::spawn_blocking` / thread terpisah, hasilnya dikirim kembali ke main context dengan `glib::spawn_future_local` atau channel. Dilarang memblok main loop (tidak boleh `thread::sleep`, `Command::output()`, I/O disk besar, query DB panjang di handler sinyal).
- **Daemon**: `tokio` runtime ringan. Tugas yang bisa menggantung harus punya **timeout**.
- Setiap proses anak (mpv, ffmpeg, ffprobe, yt-dlp) harus: dijalankan dengan `Command::new(...).args([...])` (**tanpa shell**), punya timeout/penghentian, dan dibersihkan saat induk keluar.
- Kanal (channel) berbatas (bounded). Jangan antrean tak terbatas.

### I/O file
- Tulis atomik: tulis ke `file.tmp` di direktori sama, `fsync`, `rename`.
- Gunakan `PathBuf`/`OsString`, bukan `String`, untuk path. Dukung spasi dan unicode.
- Jangan ikuti symlink keluar dari folder yang diminta pengguna saat memindai rekursif tanpa batas kedalaman (batasi kedalaman 8, deteksi loop).

## 3. Keamanan

- Tidak ada `sh -c`. Argumen proses selalu array.
- URL (fitur "Dari tautan") divalidasi dengan crate `url`: hanya `http`/`https`, panjang maksimal 2048, tidak boleh diawali `-`. Selalu sertakan `--` sebelum URL pada perintah `yt-dlp`.
- Nama file hasil unduhan disanitasi (`yt-dlp` memakai `-o` dengan template aman dan `--restrict-filenames` atau `--windows-filenames`).
- Tidak ada telemetri. Tidak ada koneksi jaringan di luar `yt-dlp` yang dipicu pengguna.
- Soket IPC mpv dan lock file disimpan di `$XDG_RUNTIME_DIR/vallentheme/` dengan izin `0700`.
- Jangan menjalankan apa pun sebagai root. Aplikasi tidak butuh `sudo` saat berjalan.

## 4. Keselamatan data pengguna

- File asli (foto/video) **tidak pernah** diubah, dipindah, atau dihapus oleh aplikasi.
- "Hapus" di library hanya menghapus entri database. Menghapus file dari disk **tidak ada** di v0.1.
- Hasil konversi/unduhan disimpan di cache atau folder yang dipilih pengguna, bukan menimpa apa pun.
- Setting desktop yang diubah aplikasi (misalnya gambar latar MATE atau `draw-background`) harus **dicatat nilai awalnya** dan **dipulihkan** saat berhenti/uninstal.

## 5. Dependensi

### Sistem (apt)
`build-essential pkg-config libgtk-4-dev libadwaita-1-dev libsqlite3-dev mpv i965-va-driver vainfo` (ffmpeg dan yt-dlp sudah ada di mesin pengembang).

### Crate yang diizinkan (selain ini, tanya dulu)
| Kategori | Crate |
|---|---|
| GUI | `gtk4`, `libadwaita` (alias `adw`), `glib-build-tools` (resource) |
| Data | `rusqlite` (fitur `bundled` **tidak** dipakai, pakai SQLite sistem), `serde`, `serde_json`, `toml` |
| Error/log | `thiserror`, `anyhow`, `tracing`, `tracing-subscriber`, `tracing-appender` |
| Sistem | `directories`, `x11rb` (fitur `randr`, `xfixes` bila perlu), `zbus`, `notify`, `libc` (hanya `prctl`/`setpriority`/`flock`), `nix` (opsional, alternatif libc) |
| Async | `tokio` (fitur minimum: `rt`, `macros`, `time`, `process`, `sync`, `net`, `io-util`, `signal`) |
| Utilitas | `url`, `blake3`, `clap`, `gettext-rs` (atau `rust-i18n` jika gettext menyulitkan; tanya) |
| Dev | `tempfile`, `assert_fs`, `pretty_assertions` |

Sebelum menambah crate: cek ukuran dan jumlah dependensi turunannya (mesin pengembang lemah). `cargo tree -d` untuk memeriksa duplikat.

## 6. Cara mengukur (dipakai di banyak fase)

- RAM proses: `ps -o rss= -p <PID>` (KB). Catat tiap menit lewat skrip saat soak test.
- CPU: `top -b -n 1 -p <PID>` atau `pidstat -p <PID> 5` (paket `sysstat`).
- Renderer GTK alternatif: `GSK_RENDERER=cairo ./vallentheme-gui` dan `GSK_RENDERER=ngl`.
- Log debug: `RUST_LOG=vallentheme=debug`.
- GTK: `GTK_DEBUG=interactive` untuk inspector (hanya untuk dev).

## 7. Hal yang dilarang

- Menambah fitur "sekalian".
- Menyalin kode dari proyek lain tanpa memeriksa lisensinya.
- Menonaktifkan lint/test agar hijau.
- Mengubah skema database tanpa migrasi.
- Memakai `sleep` sebagai pengganti sinkronisasi yang benar.
- Menyimpan data pengguna di luar direktori XDG.

## 8. Kapan harus berhenti dan bertanya

- Gate tidak bisa dipenuhi dengan pendekatan di dokumen.
- Butuh crate di luar daftar.
- Dua dokumen bertentangan.
- Perubahan akan mengubah perilaku yang sudah dijanjikan di fase sebelumnya.
- Ada risiko kehilangan data pengguna.

Gunakan format BLOKIR di `README.md`.

## 9. Gaya komunikasi agent

- Laporan singkat, faktual, ada angka ukur.
- Jangan klaim "sudah beres" tanpa menjalankan Gate.
- Bila gagal, tulis apa yang dicoba dan log relevan, bukan hanya "tidak bisa".
