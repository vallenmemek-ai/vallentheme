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

## 3. Keputusan render X11 (Fase 1)

Diuji pada sesi nyata `:0`, 1366×768, MATE 1.26.4, marco (compositor aktif),
caja 1.26.4, Intel GM45 (OpenGL 2.1).

### 3.1 Metode yang dipakai

**Window bertipe DESKTOP per monitor + `mpv --wid`.** Terbukti berjalan: video
tampil sebagai wallpaper, tidak muncul di taskbar/pager, ikut semua workspace.

Atom yang dipasang dan diverifikasi dengan `xprop`:

```
_NET_WM_WINDOW_TYPE = _NET_WM_WINDOW_TYPE_DESKTOP
_NET_WM_STATE       = _NET_WM_STATE_BELOW, _NET_WM_STATE_STICKY,
                      _NET_WM_STATE_SKIP_TASKBAR, _NET_WM_STATE_SKIP_PAGER
_NET_WM_DESKTOP     = 4294967295 (semua workspace)
WM_CLASS            = "vallentheme", "Vallentheme"
```

### 3.2 Temuan penting: `background_pixel` tidak cukup

Dengan compositor aktif (marco), isi window tidak otomatis terisi dari
`background_pixel`. Window harus **dilukis sendiri** (`poly_fill_rectangle`)
dan dilukis ulang setiap menerima `Expose`. Tanpa itu window tampak kosong
walaupun `xwininfo` menyatakan `IsViewable`.

### 3.3 Konflik dengan Caja (ikon desktop) — **BLOKIR**

Window desktop Caja berkedalaman **24-bit** (opaque). Karena itu:

- Window wallpaper kita (di bawah, sesuai `_NET_WM_STATE_BELOW`) **tertutup
  total** oleh window desktop Caja. Diukur: 0 piksel wallpaper terlihat.
- Bila window kita dinaikkan di atas window Caja, wallpaper terlihat
  (214.548 piksel terukur), tetapi **ikon desktop Caja ikut tertutup**.
- Opsi `gsettings set org.mate.background draw-background false` (opsi b di
  `panduan/06` bagian 3) **sudah diuji dan tidak menyelesaikan masalah**:
  Caja tetap melukis warna solid (bukan transparan), wallpaper tetap 0 piksel.

Fresco (referensi, v1.1.45) menyelesaikan ini dengan **cermin ikon**
(`caja_mirror.rs`): latar Caja di-set ke warna kunci (`#010101`, shading solid),
wallpaper dinaikkan di atas Caja, lalu isi window desktop Caja disalin ke atas
wallpaper memakai ekstensi X **Composite + Damage** dengan chroma-key.

Keputusan sementara v0.1: naikkan window wallpaper tepat di atas window desktop
Caja (best-effort, dicek ulang saat tumpukan berubah) dan tampilkan pemberitahuan
bahwa ikon desktop disembunyikan selama wallpaper berjalan. Cermin ikon diusulkan
untuk v0.2. Butuh persetujuan pemilik proyek (lihat BLOKIR di `docs/PROGRESS.md`).

Catatan: di mesin pengembang `show-desktop-icons` bernilai `false`, jadi konflik
ini tidak muncul pada pengujian harian.

### 3.4 Hardware decoding: tidak tersedia untuk H.264/VP9

`vainfo` pada GM45 hanya melaporkan **MPEG-2** (`VAProfileMPEG2Simple/Main`).
Tidak ada `VAProfileH264*` maupun VP9. Akibatnya, apa pun nilai `--hwdec`,
mpv selalu berakhir di `hwdec-current = "no"` (software).

Konsekuensi untuk Fase 7: `hwdec=auto-safe` **bukan** jaminan hemat CPU; harus
diverifikasi lewat `hwdec-current`, bukan diasumsikan dari `auto-safe`.

### 3.5 Opsi mpv final untuk mesin ini

| Opsi | Nilai | Alasan |
|---|---|---|
| `--vo` | `gpu` | 4K: 77,5% CPU vs `x11` 91,5% CPU |
| `--hwdec` | `auto-safe` | aman; jatuh ke software, tidak error |
| `--wid` | XID window desktop | wajib |
| `--panscan` / `--keepaspect` | `1.0` / `yes` | mode "Penuhi" |
| `--framedrop` | `vo` | jaga audio/timing |

### 3.6 Tabel pengukuran

Kondisi: sesi nyata, panel MATE + plank berjalan, tanpa aplikasi berat.
Durasi 60 detik per sel, CPU = persen dari **satu** core, RSS = memori mpv.
Data mentah: `docs/ukur-fase1.csv`.

| Video | hwdec diminta | hwdec aktual | vo | CPU% | RSS (MB) | frame drop |
|---|---|---|---|---|---|---|
| 1080p H.264 24fps | no | no | gpu | 40,3 | 169 | 0 |
| 1080p H.264 24fps | vaapi | no | gpu | 40,4 | 175 | 0 |
| 1080p H.264 24fps | auto-safe | no | gpu | 39,1 | 201 | 0 |
| 4K H.264 24fps | auto-safe | no | gpu | 77,5 | 315 | 0 |
| 4K H.264 24fps | no | no | x11 | 91,5 | 199 | 0 |
| 1080p VP9 30fps | auto-safe | no | gpu | 55,2 | 179 | 0 |
| Foto 4K (mpv) | auto-safe | no | gpu | 1,0 | 158 | 0 |

Kesimpulan: 4K tetap lancar (0 frame drop) di mesin 2 core ini. Jangan batasi
kualitas secara default.

### 3.7 Ketahanan

| Uji | Hasil |
|---|---|
| `kill -9` pada program induk | mpv ikut mati (`PR_SET_PDEATHSIG`), tidak ada proses yatim |
| `kill -9` pada mpv | dipulihkan otomatis dalam **2543 ms** |
| Window sisa setelah keluar | tidak ada |
| Sesi tanpa `DISPLAY` | `RenderError::NotX11` (kode `VT-X11-003`), bukan panic |

### 3.8 Deteksi fullscreen/maximize

Memantau `_NET_ACTIVE_WINDOW` dan `_NET_CLIENT_LIST_STACKING` pada window root
lewat `PropertyChangeMask`.

- **Fullscreen**: terdeteksi, latensi **6,0 ms** (diukur dengan `wmctrl`).
- **Maximize**: **tidak** terdeteksi lewat root saja — perubahan `_NET_WM_STATE`
  terjadi pada window klien, bukan root. Fase 5/7 harus berlangganan
  `PropertyNotify` pada window aktif itu sendiri.


## 4. Keputusan daftar library

*(diisi di Fase 4)*
