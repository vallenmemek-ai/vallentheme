# vallentheme

Wallpaper foto dan video (live wallpaper) untuk desktop Linux, dengan tampilan
gelap modern yang ringan.

- Bahasa: **Rust**
- GUI: **GTK4 + libadwaita**
- Target pertama: **Debian 13 + MATE + X11**
- Prioritas: **stabilitas > kinerja > fitur > kecantikan**

## Cara build

```bash
# 1. Dependensi sistem (lihat scripts/setup-dev.sh)
sudo apt install -y build-essential pkg-config libgtk-4-dev libadwaita-1-dev \
    libsqlite3-dev mpv ffmpeg

# 2. Build
cargo build --workspace

# 3. Pemeriksaan wajib
scripts/check.sh
```

## Struktur

| Path | Isi |
|---|---|
| `crates/vallentheme-core` | Model data, DB, scanner, thumbnail, konfigurasi, kebijakan kinerja |
| `crates/vallentheme-render` | Backend render: window X11 dan pengawasan mpv |
| `crates/vallentheme-daemon` | Layanan latar (D-Bus, state machine) |
| `crates/vallentheme-gui` | Antarmuka GTK4/libadwaita |
| `panduan/` | Aturan kerja, arsitektur, sistem desain, referensi teknis |
| `fase/` | Rencana kerja per fase beserta Gate |
| `docs/` | Progres, keputusan arsitektur, ide |

Rencana proyek lengkap ada di [`docs/RENCANA-PAKET.md`](docs/RENCANA-PAKET.md).

## Status

Lihat [`docs/PROGRESS.md`](docs/PROGRESS.md).

## Lisensi

Belum ditentukan (`LicenseRef-TBD`); keputusan lisensi diambil sebelum rilis.
