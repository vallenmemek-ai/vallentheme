# 01 — Arsitektur

## 1. Struktur repo

```
vallentheme/
├─ Cargo.toml                     # [workspace], [workspace.lints], [workspace.dependencies]
├─ Cargo.lock                     # di-commit
├─ rust-toolchain.toml
├─ rustfmt.toml
├─ .cargo/config.toml             # jobs = 2, profil ringan
├─ crates/
│  ├─ vallentheme-core/           # tanpa GTK, tanpa X11
│  │  └─ src/{lib.rs, model.rs, db/, scan.rs, probe.rs, thumb.rs, config.rs,
│  │          paths.rs, perf/{mod.rs, policy.rs, profile.rs}, settings.rs, error.rs}
│  ├─ vallentheme-render/         # tanpa GTK
│  │  └─ src/{lib.rs, backend.rs (trait), x11/, mpv/{process.rs, ipc.rs, args.rs}, setter.rs}
│  ├─ vallentheme-daemon/         # binary
│  │  └─ src/{main.rs, dbus.rs, state.rs, supervisor.rs, monitors.rs, watch.rs, autostart.rs, restore.rs}
│  └─ vallentheme-gui/            # binary
│     └─ src/{main.rs, app.rs, window.rs, pages/{library,edit}.rs,
│             widgets/{card.rs, status_chip.rs, empty.rs, ...}, dbus_client.rs, thumbs.rs, i18n.rs}
├─ data/
│  ├─ style.css
│  ├─ icons/                      # ikon aplikasi + ikon symbolic yang dibundel
│  ├─ id.vallentheme.App.desktop
│  ├─ id.vallentheme.Daemon.desktop (autostart)
│  ├─ id.vallentheme.metainfo.xml
│  └─ vallentheme.gresource.xml
├─ po/                            # terjemahan
├─ tests/fixtures/                # video & foto contoh kecil
├─ docs/{PROGRESS.md, IDEAS.md, ARCH.md}
└─ scripts/{check.sh, soak.sh, setup-dev.sh}
```

## 2. Tanggung jawab crate

| Crate | Tugas | Tidak boleh |
|---|---|---|
| `core` | Model data, SQLite, scanner, ffprobe/thumbnail, config, **kebijakan kinerja (fungsi murni)** | Menyentuh GTK, X11, D-Bus |
| `render` | Trait `RenderBackend`, backend X11, supervisor/IPC mpv, setter foto desktop | Menyentuh DB |
| `daemon` | Layanan latar: D-Bus server, state machine, pantau monitor/window/baterai, restore | Menyentuh GTK |
| `gui` | Antarmuka GTK4/libadwaita, klien D-Bus | Mengontrol mpv/X11 langsung |

Aturan arah dependensi: `gui → core`, `daemon → core + render`, `render → core` (hanya tipe), `core → (tidak ada crate internal)`.

## 3. Model proses

```
┌────────────────────┐   D-Bus (session)   ┌─────────────────────┐   spawn + IPC   ┌───────┐
│ vallentheme-gui    │ ◄─────────────────► │ vallentheme-daemon  │ ──────────────► │  mpv  │ (1 per monitor)
│ (GTK4, DB, cache)  │                     │ (state, X11, perf)  │  JSON socket    └───────┘
└─────────┬──────────┘                     └──────────┬──────────┘
          │ baca/tulis                                │ baca/tulis
          ▼                                           ▼
   library.db + cache                      daemon-state.json (state terakhir)
```

- **GUI** adalah pemilik library (DB) dan thumbnail. Ia mengirim **permintaan lengkap** (path, jenis, pengaturan) ke daemon.
- **Daemon** tidak membaca DB. Ia menyimpan sendiri state terakhir per monitor di `daemon-state.json` supaya bisa **restore saat login tanpa GUI**.
- **mpv** dijalankan sebagai proses anak dan diawasi. Bila daemon mati, mpv ikut mati (`PR_SET_PDEATHSIG`).
- GUI boleh ditutup tanpa menghentikan wallpaper. Daemon hidup sampai diminta berhenti atau sesi berakhir.

## 4. State machine daemon (per monitor)

```
        Apply()                 mpv siap
Idle ─────────────► Starting ───────────────► Running ◄──────────┐
 ▲                     │ gagal                  │   │ aturan jeda │ aturan jeda hilang
 │ Stop()              ▼                        │   └──► Paused(reason) ┘
 │                  Failed(code) ◄──────────────┘ mpv crash berulang melewati batas
 │                     │ Apply() / Retry()
 └─────────────────────┘
                 Degraded: Running dengan batasan (hasil keputusan sistem kinerja)
```

- `Paused(reason)`: `Fullscreen`, `Maximized`, `Battery`, `ScreenLocked`, `UserRequest`, `SystemPowerSave`.
- `Degraded(level)`: level L1–L3 dari Fase 7.
- `Failed(code)`: berhenti mencoba ulang otomatis, kirim sinyal `Error`, tunggu aksi pengguna.

## 5. Alur penting

### 5.1 "Jadikan wallpaper"
1. GUI membangun `ApplyRequest` (id, path, jenis, `settings`, monitor target).
2. GUI memanggil `Apply` lewat D-Bus. Jika daemon belum jalan: GUI menyalakannya (`vallentheme-daemon --daemonize` atau lewat D-Bus activation), menunggu hingga 3 detik.
3. Daemon memvalidasi (file ada, bisa dibaca, jenis cocok), menyimpan state, lalu:
   - **Foto** → setter desktop (tanpa mpv).
   - **Video** → buat window X11 per monitor, jalankan mpv dengan argumen dari `args.rs`.
4. Daemon mengirim `StateChanged`. GUI menampilkan toast sukses/gagal.

### 5.2 Saat login
1. `~/.config/autostart/id.vallentheme.Daemon.desktop` menjalankan `vallentheme-daemon --restore`.
2. Daemon membaca `daemon-state.json`, memeriksa file masih ada, lalu menerapkan wallpaper per monitor.
3. Bila file hilang → jangan layar hitam; biarkan wallpaper desktop semula dan tulis log.

### 5.3 Saat keluar
1. `SIGTERM`/`SIGINT` → hentikan mpv dengan sopan (`quit` lewat IPC, lalu `SIGKILL` setelah 2 detik), tutup window, pulihkan setting desktop yang diubah, hapus socket/lock.

## 6. Thread dan async

| Tempat | Model |
|---|---|
| GUI | Main context GLib. Kerja berat di `gio::spawn_blocking`. Hasil kembali lewat `glib::spawn_future_local`. DB diakses dari satu thread DB khusus (aktor sederhana) agar tidak ada kunci tumpang-tindih |
| Daemon | `tokio` `current_thread` bila memadai. Satu task per monitor, satu task pemantau sistem, satu task D-Bus |
| Core | Pustaka sinkron. Worker pool kecil (`min(jumlah_core, 2)`) untuk thumbnail/ffprobe |

## 7. Lokasi file (XDG)

| Jenis | Path |
|---|---|
| Config | `~/.config/vallentheme/config.toml` |
| Database | `~/.local/share/vallentheme/library.db` |
| Cache thumbnail | `~/.cache/vallentheme/thumbs/` |
| Cache konversi | `~/.cache/vallentheme/optimized/` |
| State daemon | `~/.local/state/vallentheme/daemon-state.json` |
| Log | `~/.local/state/vallentheme/logs/` |
| Runtime (socket, lock) | `$XDG_RUNTIME_DIR/vallentheme/` |
| Unduhan bawaan | `~/Videos/vallentheme/` (bisa diubah) |

## 8. Taksonomi error

Kategori kode: `IO`, `DB`, `PROBE`, `THUMB`, `MPV`, `X11`, `DBUS`, `NET`, `CONFIG`, `PERM`, `UNSUPPORTED`.
Format: `VT-<KATEGORI>-<NNN>` (contoh `VT-MPV-002`). Katalog dan pesan ramah ada di Fase 9; kode didefinisikan sejak Fase 2 dan bertambah seiring fase.

## 9. Kompatibilitas

- Minimum: Debian 13 (GTK 4.18, libadwaita 1.7, mpv 0.40, ffmpeg 7).
- Jangan memakai API libadwaita yang lebih baru dari 1.5 tanpa pemeriksaan versi (kita memakai `AlertDialog`, `PreferencesDialog` (1.5+), `Breakpoint`, `ToolbarView`).
- Sesi non-X11 (Wayland): deteksi lewat `XDG_SESSION_TYPE` dan tampilkan pesan jelas. Jangan crash.

## 10. Di luar cakupan v0.1
Wayland (layer-shell/GNOME), Windows/macOS, format interaktif/web/Wallpaper Engine, akun/cloud/telemetri, editor video.
