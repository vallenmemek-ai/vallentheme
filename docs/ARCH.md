# ARCH — Arsitektur & Catatan Teknis

Dokumen ini mencatat keputusan teknis nyata hasil pengukuran di mesin, termasuk
perbedaan dari `panduan/`. Bila hasil uji berbeda dari dokumen rencana, **yang
berlaku adalah catatan di sini**, dan perbedaannya dilaporkan lewat BLOKIR.

## 1. Lingkungan pengembangan

Diukur pada 2026-11-06.

| Komponen | Versi / Nilai |
|---|---|
| Distribusi | Debian GNU/Linux 13 (trixie) |
| Kernel | lihat `uname -r` |
| Rust | rustc 1.99.0 (b940084d7 2026-09-28) |
| Cargo | 1.99.0 (56f54df78 2026-08-27) |
| GTK | 4.18.6 |
| libadwaita | 1.7.6 (≥ 1.5, syarat terpenuhi) |
| SQLite | 3.46.1 |
| mpv | 0.40.0 |
| FFmpeg | 7.1.5-0+deb13u1 |
| yt-dlp | 2025.04.30 |
| CPU | 2 core (Core 2 Duo) |
| RAM | 3,8 GB total, ~1,5 GB tersedia |
| GPU | Intel GM45 (OpenGL 2.1) |
| Disk | 289 GB, 216 GB bebas |
| Sesi saat pengembangan | headless (tanpa `DISPLAY`), uji GUI lewat `Xvfb` |

Catatan: mesin pengembang lemah (2 core, RAM < 4 GB). Build memakai
`jobs = 2` di `.cargo/config.toml`. Jangan menambah dependensi berat tanpa
mengukur dampaknya.

## 2. Keputusan fondasi (Fase 0)

- **Workspace 4 crate** dengan arah dependensi `gui → core`,
  `daemon → core + render`, `render → core`.
- **Lint ketat sejak awal**: `unsafe_code = "forbid"` (ditimpa per modul bila
  benar-benar perlu), `clippy::unwrap_used/expect_used/panic = deny`.
  Blok test memberi `#![allow(...)]` lokal agar tetap ringkas.
- **`panic = "unwind"`** di profil rilis: daemon harus bisa menangkap panic
  thread, bukan langsung abort.
- **Config**: TOML dengan `#[serde(default)]`, validasi yang mengoreksi nilai
  di luar rentang, penyimpanan atomik (`*.tmp` → `fsync` → `rename`), dan
  cadangan `config.toml.bak` bila berkas rusak.
- **Logging**: `tracing` ganda (stderr + berkas harian), pemangkasan otomatis
  maksimal 5 berkas per komponen. `WorkerGuard` wajib disimpan pemanggil.
- **Path**: seluruh lokasi XDG ditentukan di satu tempat (`core::paths`), folder
  runtime diberi izin `0700`.

## 3. Keputusan render X11

*(diisi di Fase 1)*

## 4. Keputusan daftar library

*(diisi di Fase 4)*
