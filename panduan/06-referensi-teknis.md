# 06 — Referensi Teknis

Kumpulan detail teknis yang dirujuk fase-fase. Bila hasil uji di mesin nyata berbeda dari tulisan di sini, **catat di `docs/ARCH.md`** dan ikuti kenyataan (lalu laporkan lewat BLOKIR bila mengubah desain).

## 1. D-Bus daemon

- Bus: **session**. Nama: `id.vallentheme.Daemon`. Path: `/id/vallentheme/Daemon`. Interface: `id.vallentheme.Daemon1`.
- Semua payload kompleks berupa **string JSON** (versi skema ada di field `v`).

### Metode
| Metode | Argumen | Hasil | Keterangan |
|---|---|---|---|
| `Apply` | `s request_json` | `s result_json` | Terapkan wallpaper (lihat `ApplyRequest`) |
| `Update` | `s patch_json` | `s result_json` | Ubah pengaturan wallpaper aktif tanpa mulai ulang |
| `Stop` | `s monitor` (kosong = semua) | `b` | Hentikan dan pulihkan wallpaper desktop |
| `Pause` | `s reason` | `b` | Jeda manual |
| `Resume` | — | `b` | Lanjutkan |
| `GetStatus` | — | `s status_json` | Status per monitor |
| `SetPerformance` | `s perf_json` | `b` | Terapkan konfigurasi kinerja |
| `GetDecisionLog` | `u limit` | `s json` | Riwayat keputusan kinerja |
| `GetDeviceProfile` | — | `s json` | Profil perangkat hasil deteksi |
| `Quit` | — | — | Keluar bersih |

### Sinyal
`StateChanged(s status_json)`, `Error(s code, s message)`, `DecisionMade(s entry_json)`.

### Properti
`Version` (s), `State` (s: `idle|running|paused|degraded|failed`), `Session` (s: `x11|wayland|unknown`).

### Contoh `ApplyRequest`
```json
{
  "v": 1,
  "wallpaper_id": 42,
  "path": "/home/user/Videos/raiden.mp4",
  "kind": "video",
  "monitor": "all",
  "settings": {
    "fill": "cover", "rotation": 0, "zoom": 0.0, "pan_x": 0.0, "pan_y": 0.0,
    "transition": "none", "mute": true, "volume": 50, "power": "default"
  }
}
```

### Contoh `status_json`
```json
{
  "v": 1,
  "state": "running",
  "monitors": [
    {"name": "HDMI-1", "geometry": [0,0,1920,1080], "state": "running",
     "wallpaper_id": 42, "level": 0, "paused_reason": null, "hwdec": "vaapi"}
  ]
}
```

## 2. mpv

### Argumen dasar (video)
```
mpv
  --no-config
  --wid=<XID>
  --input-ipc-server=<runtime>/mpv-<monitor>.sock
  --idle=yes --force-window=yes
  --loop-file=inf
  --no-osc --no-osd-bar --osd-level=0
  --no-input-default-bindings --no-input-cursor --cursor-autohide=no
  --really-quiet
  --keep-open=yes
  --hwdec=auto-safe
  --vo=gpu                    # fallback --vo=x11 bila GL gagal (lihat Fase 1)
  --framedrop=vo
  --no-audio | --mute=yes --volume=<0..100>
  --panscan=<0|1> --keepaspect=<yes|no> --video-unscaled=<yes|no>
  --video-rotate=<0|90|180|270>
  --video-zoom=<log2> --video-pan-x=<f> --video-pan-y=<f>
  --vf=fps=<N>                # hanya bila ada batas fps (Fase 7)
  -- <path>
```

### Pemetaan "Metode pengisian" → mpv
| Mode | Opsi |
|---|---|
| Penuhi (cover) | `--panscan=1.0 --keepaspect=yes` |
| Pas (fit) | `--panscan=0.0 --keepaspect=yes` (latar hitam) |
| Tengah (center) | `--video-unscaled=yes` |
| Regang (stretch) | `--keepaspect=no` |
| Ubin | Tidak untuk video |

### Properti mpv yang dibaca lewat IPC (Fase 5/7)
`time-pos`, `pause`, `hwdec-current`, `estimated-vf-fps`, `vo-drop-frame-count`, `decoder-frame-drop-count`, `video-params/w`, `video-params/h`, `container-fps`.
**Tidak ada properti `cpu`**; beban CPU dibaca dari `/proc` (bagian 8).

### Protokol IPC
JSON per baris lewat soket unix. Permintaan: `{"command": ["get_property", "pause"], "request_id": 7}`. Balasan: `{"request_id":7,"error":"success","data":false}`. Peristiwa tidak berkaitan permintaan datang tanpa `request_id` (`{"event":"end-file",...}`). Selalu cocokkan balasan dengan `request_id`, beri timeout 2 detik, dan tangani soket tertutup (mpv mati).

### Menjaga mpv mati bersama daemon
Pada `pre_exec` anak: `libc::prctl(PR_SET_PDEATHSIG, SIGKILL)` (modul kecil dengan `unsafe` dan komentar). Atur juga grup proses sendiri agar bisa dibunuh bersama anak-anaknya.

## 3. X11 (x11rb)

Atom: `_NET_WM_WINDOW_TYPE`, `_NET_WM_WINDOW_TYPE_DESKTOP`, `_NET_WM_STATE`, `_NET_WM_STATE_BELOW`, `_NET_WM_STATE_STICKY`, `_NET_WM_STATE_SKIP_TASKBAR`, `_NET_WM_STATE_SKIP_PAGER`, `_NET_WM_DESKTOP` (0xFFFFFFFF = semua desktop), `_NET_ACTIVE_WINDOW`, `_NET_CLIENT_LIST_STACKING`, `_NET_WM_STATE_FULLSCREEN`, `_NET_WM_STATE_MAXIMIZED_VERT/HORZ`, `_NET_WM_OPACITY`/`_NET_WM_WINDOW_OPACITY`.

Langkah dasar membuat window wallpaper per monitor:
1. RandR `get_monitors` → geometri tiap monitor.
2. Buat window anak root dengan geometri monitor, tanpa dekorasi.
3. Set `_NET_WM_WINDOW_TYPE = [_NET_WM_WINDOW_TYPE_DESKTOP]`, `_NET_WM_STATE = [BELOW, STICKY, SKIP_TASKBAR, SKIP_PAGER]`, `_NET_WM_DESKTOP = 0xFFFFFFFF`.
4. Petakan window, `lower_window`, kirim XID ke mpv (`--wid`).
5. Pantau `RRScreenChangeNotify`/`RRNotify` untuk monitor berubah.
6. Pantau `PropertyNotify` pada root untuk `_NET_ACTIVE_WINDOW` dan `_NET_CLIENT_LIST_STACKING` (deteksi fullscreen/maximize).

Bila Caja (MATE) menutupi window: coba berurutan (catat hasil di Fase 1):
a. `lower_window` ulang setelah Caja siap.
b. Mematikan lukisan latar Caja: `gsettings set org.mate.background draw-background false` (catat nilai awal, pulihkan saat berhenti).
c. Memakai `override_redirect` + `lower_window`.
d. Teknik "xwinwrap" (reparent ke window root/desktop yang sudah ada).

## 4. Setter foto desktop

| Desktop | Cara |
|---|---|
| MATE | `gsettings set org.mate.background picture-filename '<path>'` dan `picture-options` ∈ `none, wallpaper, centered, scaled, stretched, zoom, spanned` |
| GNOME/Cinnamon | `gsettings set org.gnome.desktop.background picture-uri 'file://<path>'` (+ `picture-uri-dark`), `picture-options` |
| XFCE | `xfconf-query -c xfce4-desktop -p /backdrop/screen0/monitor<N>/workspace0/last-image -s <path>` |
| Lainnya (cadangan) | Render foto lewat window mpv yang sama seperti video |

Pemetaan fill → `picture-options`: Penuhi=`zoom`, Pas=`scaled`, Tengah=`centered`, Regang=`stretched`, Ubin=`wallpaper`.
Gunakan `Command::new("gsettings").args([...])` tanpa shell. Catat nilai awal sebelum mengubah, pulihkan bila pengguna menekan Stop.
Deteksi desktop: `XDG_CURRENT_DESKTOP` (dipisah `:`), cadangan `DESKTOP_SESSION`.

## 5. FFprobe & FFmpeg

```
ffprobe -v error -print_format json -show_format -show_streams -- <file>
```
Ambil: `streams[].codec_type`, `codec_name`, `width`, `height`, `avg_frame_rate` (pecahan "24000/1001"), `format.duration`, `format.size`. Video tanpa stream audio → `has_audio=false`. Beri timeout 15 detik.

Thumbnail video (480 px, frame di ~10% durasi, minimal 1 detik):
```
ffmpeg -hide_banner -loglevel error -y -ss <t> -i <file> -frames:v 1 -vf scale=480:-2 -q:v 4 -- <out.jpg>
```
Thumbnail foto: `ffmpeg -hide_banner -loglevel error -y -i <file> -frames:v 1 -vf scale=480:-2 -q:v 4 -- <out.jpg>`.
Frame pratinjau besar (Edit): lebar 960, sama caranya.

Optimalkan saat impor (Fase 7):
```
ffmpeg -hide_banner -loglevel error -y -i <in> \
  -vf "scale=-2:'min(<H>,ih)',fps=<F>" \
  -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p \
  -c:a aac -b:a 96k -movflags +faststart -progress pipe:1 -nostats -- <out.mp4>
```
(`<H>` = 720 atau 1080, `<F>` = batas fps; hanya turunkan, jangan naikkan.)

## 6. yt-dlp (Fase 8)

```
yt-dlp --no-playlist --newline --no-exec --restrict-filenames \
  --max-filesize <N>M \
  -f "bv*[height<=?<H>]+ba/b[height<=?<H>]/bv*+ba/b" \
  --merge-output-format mp4 \
  --progress-template "download:%(progress._percent_str)s" \
  -o "<dir>/%(title).100B [%(id)s].%(ext)s" \
  -- <url>
```
Baca progres dari stdout per baris. Hentikan dengan `SIGTERM` lalu bersihkan file `.part`.

## 7. Skema database (SQLite, WAL)

```sql
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA synchronous = NORMAL;

CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);          -- schema_version

CREATE TABLE folders (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  recursive INTEGER NOT NULL DEFAULT 1,
  added_at INTEGER NOT NULL
);

CREATE TABLE wallpapers (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL CHECK (kind IN ('image','video')),
  title TEXT NOT NULL,
  width INTEGER, height INTEGER,
  fps REAL, duration_ms INTEGER, codec TEXT,
  has_audio INTEGER NOT NULL DEFAULT 0,
  size_bytes INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL,
  thumb_path TEXT,
  favorite INTEGER NOT NULL DEFAULT 0,
  missing INTEGER NOT NULL DEFAULT 0,
  sort_index INTEGER NOT NULL,
  added_at INTEGER NOT NULL,
  last_used_at INTEGER,
  deleted_at INTEGER,                       -- soft delete untuk "Urungkan"
  settings_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_wp_kind ON wallpapers(kind);
CREATE INDEX idx_wp_folder ON wallpapers(folder_id);
CREATE INDEX idx_wp_added ON wallpapers(added_at DESC);
CREATE INDEX idx_wp_used ON wallpapers(last_used_at DESC);
CREATE INDEX idx_wp_fav ON wallpapers(favorite) WHERE favorite = 1;
```
Waktu disimpan sebagai detik Unix (UTC). Migrasi: file/fungsi bernomor (`m001_init`, `m002_...`), dijalankan dalam transaksi, versi diperbarui di `meta`.

## 8. Metrik sistem (tanpa crate berat)

- CPU proses daemon+mpv: baca `/proc/<pid>/stat` (utime+stime) dua kali dengan selang, bagi dengan `CLK_TCK` dan jumlah core.
- CPU total: `/proc/stat` baris `cpu`.
- RAM tersedia: `/proc/meminfo` → `MemAvailable`.
- Baterai: `/sys/class/power_supply/BAT*/{capacity,status}`; sumber AC: `/sys/class/power_supply/AC*/online` atau `ADP*/online`.
- GPU/driver (informasi saja): `/dev/dri/`, keluaran `vainfo` bila ada.
- Layar terkunci: sinyal D-Bus `ActiveChanged` dari `org.mate.ScreenSaver`, `org.gnome.ScreenSaver`, atau `org.freedesktop.ScreenSaver` (coba berurutan, abaikan yang tidak ada).

## 9. Autostart

`~/.config/autostart/id.vallentheme.Daemon.desktop`:
```
[Desktop Entry]
Type=Application
Name=vallentheme
Exec=vallentheme-daemon --restore
X-GNOME-Autostart-enabled=true
NoDisplay=true
```

## 10. Config (`config.toml`, contoh)

```toml
version = 1

[general]
language = "system"          # system | id | en
autostart = true
run_in_background = true

[ui]
last_folder = "all"
sort = "mine"                # mine | newest | recent | name | size | resolution | favorite
reduce_motion = "system"     # system | on | off

[performance]
mode = "auto"                # auto | original | balanced | saver | custom
hwdec = "auto"               # auto | vaapi | nvdec | none
fps_cap = 0                  # 0 = tanpa batas
max_render_height = 0        # 0 = asli
pause_on_fullscreen = true
pause_on_maximized = true
pause_on_battery = true
battery_threshold = 30
pause_on_lock = true
pause_on_system_saver = true
nice = "normal"              # normal | low | lowest
optimize_on_import = false
optimize_target_height = 1080

[storage]
download_dir = ""            # kosong = ~/Videos/vallentheme
cache_limit_mb = 2048
```
Loader wajib: toleran terhadap field hilang (pakai bawaan), mengabaikan field tak dikenal dengan peringatan log, memvalidasi rentang, dan menyimpan ulang secara atomik.

## 11. Pemeriksaan lingkungan saat start (GUI & daemon)

`mpv` ada? `ffmpeg`/`ffprobe` ada? `XDG_SESSION_TYPE`? `DISPLAY` valid? Versi GTK/libadwaita? Hasil dicatat di log dan, bila ada yang kritis hilang, ditampilkan dialog yang sesuai (lihat `05-gui-pengaturan-dialog-status.md`).
