# FASE 0 — Persiapan & Fondasi Repo

**Baca dulu:** `panduan/00-aturan-agent.md`, `panduan/01-arsitektur.md` (bagian 1–2, 7).

## Tujuan
Lingkungan bangun yang bersih, cepat, dan bisa diulang. Semua alat kualitas (lint, format, test, log, config) siap **sebelum** ada fitur.

## Ruang lingkup
**Termasuk:** dependensi sistem, workspace Cargo, konfigurasi build ringan, skrip cek, logging, loader config, struktur docs.
**Tidak termasuk:** GUI, database, mpv, X11 (semua di fase berikutnya).

## Langkah rinci

### 0.1 Dependensi sistem
```bash
sudo apt update
sudo apt install -y build-essential pkg-config git \
  libgtk-4-dev libadwaita-1-dev libsqlite3-dev \
  mpv i965-va-driver vainfo ffmpeg sysstat
# yt-dlp sudah ada di mesin pengembang
```
Verifikasi dan **catat versinya** di `docs/ARCH.md`:
```bash
pkg-config --modversion gtk4 libadwaita-1 sqlite3
mpv --version | head -1
ffmpeg -version | head -1
rustc --version && cargo --version
vainfo 2>&1 | head -10
```
Bila `libadwaita` < 1.5 → **BLOKIR** (kita butuh `AlertDialog`/`PreferencesDialog`).

### 0.2 Inisialisasi repo
- `git init`, branch `main`, `.gitignore` (target/, *.db, .env, dll.).
- Buat struktur di `panduan/01-arsitektur.md` bagian 1 (folder kosong boleh berisi `.gitkeep`).
- `README.md` singkat (nama, tujuan, cara build) dan `LICENSE` **sementara** berisi catatan "lisensi belum ditentukan" (keputusan lisensi ada di Fase 9; tanyakan pemilik proyek).

### 0.3 Workspace Cargo
`Cargo.toml` root:
```toml
[workspace]
resolver = "2"
members = ["crates/vallentheme-core", "crates/vallentheme-render",
           "crates/vallentheme-daemon", "crates/vallentheme-gui"]

[workspace.package]
version = "0.0.1"
edition = "2021"
license = "LicenseRef-TBD"
rust-version = "1.80"

[workspace.lints.rust]
unsafe_code = "forbid"
unused_must_use = "deny"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "warn"
dbg_macro = "deny"

[profile.dev]
opt-level = 1
debug = 1

[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 4
strip = true
panic = "unwind"      # jangan abort; daemon harus bisa menangkap panic thread
```
Tiap crate: `[lints] workspace = true`. Crate yang butuh `unsafe` kecil (render: `pre_exec`) menimpa `unsafe_code = "deny"` dengan `#![allow(unsafe_code)]` hanya di modul terkait, dengan komentar alasan.

`.cargo/config.toml`:
```toml
[build]
jobs = 2

[term]
verbose = false
```
`rust-toolchain.toml`: `channel = "stable"`, komponen `rustfmt`, `clippy`.
`rustfmt.toml`: `edition = "2021"`, `max_width = 100`.

### 0.4 Crate kerangka
- `core`: modul kosong `error.rs` (enum `Error` dengan `thiserror`, kode `VT-...`), `paths.rs`, `config.rs`.
- `render`, `daemon`, `gui`: `main.rs`/`lib.rs` minimal yang mencetak versi lewat `tracing` dan keluar bersih.
- Satu test dummy per crate agar `cargo test` jalan.

### 0.5 Paths (XDG)
Di `core::paths`: fungsi mengembalikan semua lokasi di `panduan/01-arsitektur.md` bagian 7 memakai crate `directories`, termasuk membuat direktori bila belum ada (izin `0700` untuk runtime dir). Test: gunakan `tempfile` + override env `XDG_*`.

### 0.6 Logging
- `tracing-subscriber` + `tracing-appender`: keluaran ke stderr (dev) dan file `~/.local/state/vallentheme/logs/<nama>.log` dengan rotasi harian, simpan maks 5 file. Ukuran total dibatasi (~10 MB).
- Level default `info`, bisa diubah `RUST_LOG`.
- Fungsi `init_logging(component: &str)` di `core` dipanggil oleh tiap binary.
- Jangan mencatat data sensitif (URL dengan token, isi clipboard).

### 0.7 Config
`core::config`: struct `Config` (lihat contoh di `panduan/06-referensi-teknis.md` bagian 10) dengan `serde`, nilai `Default`, `load()` (toleran field hilang/asing), `validate()` (klem rentang), `save()` (atomik).
Test: file kosong, file rusak (harus diganti default + log peringatan, file lama dicadangkan `config.toml.bak`), field asing, nilai di luar rentang.

### 0.8 Skrip
- `scripts/check.sh` (`set -euo pipefail`): `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- `scripts/setup-dev.sh`: menjalankan perintah apt di 0.1 (hanya mencetak, tidak otomatis `sudo` tanpa persetujuan).
- `scripts/soak.sh`: kerangka (diisi Fase 5): mencatat RSS dan CPU proses tiap menit ke CSV.

### 0.9 Dokumen
Buat `docs/PROGRESS.md` (kosong dengan templat), `docs/IDEAS.md`, `docs/ARCH.md` (ringkasan arsitektur, versi alat, **catatan mesin pengembang**: Core 2 Duo, 3,8 GB RAM, GM45).

## Kasus tepi
- Disk hampir penuh saat build: periksa `df -h`; `cargo clean` bila perlu.
- RAM habis saat kompilasi: pastikan `jobs = 2`; bila masih OOM, aktifkan swap sementara dan catat. Jangan kurangi kualitas lint.
- Build pertama lama (10–20 menit) itu normal.

## Tes
- `scripts/check.sh` hijau.
- Test config dan paths lulus.

## Gate
- [ ] `cargo build --workspace` sukses dari nol (`cargo clean` dulu)
- [ ] `scripts/check.sh` hijau
- [ ] `pkg-config --modversion gtk4 libadwaita-1` menampilkan versi; libadwaita ≥ 1.5
- [ ] Menjalankan tiap binary menulis log ke direktori state dan keluar bersih (kode 0)
- [ ] Loader config lolos semua kasus tepi di 0.7
- [ ] `docs/ARCH.md` memuat versi alat dan spek mesin

## Artefak
Repo ber-workspace, skrip, dokumen dasar, commit `chore: fondasi repo`.

## Jangan lakukan
Menambah dependensi GUI/DB sekarang; menulis fitur apa pun.
