# FASE 5 — Daemon Produksi & Penerapan Wallpaper

**Baca dulu:** `panduan/01-arsitektur.md` (bagian 3–5), `panduan/06-referensi-teknis.md` (bagian 1–4, 8–9), hasil Fase 1 di `docs/ARCH.md`.

## Tujuan
Menerapkan wallpaper dengan **andal**: daemon yang tahan crash mpv, pulih sendiri, restore saat login, bersih saat dimatikan, dan terhubung ke GUI.

## Ruang lingkup
**Termasuk:** crate `render` (trait, backend X11, supervisor mpv, setter foto), crate `daemon` (D-Bus, state machine, monitor, restore, autostart), integrasi minimal GUI (status chip, aksi "Jadikan wallpaper" dari Library/menu kartu).
**Tidak termasuk:** halaman Edit penuh (Fase 6), sistem kinerja otomatis (Fase 7) — hanya jeda dasar sekarang (tidak ada).

## Langkah rinci

### 5.1 Trait backend (`render`)
```rust
pub trait RenderBackend: Send {
    fn monitors(&self) -> Result<Vec<MonitorInfo>, RenderError>;
    fn start(&mut self, monitor: &MonitorId, spec: &PlaySpec) -> Result<(), RenderError>;
    fn update(&mut self, monitor: &MonitorId, patch: &SettingsPatch) -> Result<(), RenderError>;
    fn pause(&mut self, monitor: &MonitorId) -> Result<(), RenderError>;
    fn resume(&mut self, monitor: &MonitorId) -> Result<(), RenderError>;
    fn stop(&mut self, monitor: &MonitorId) -> Result<(), RenderError>;
    fn events(&mut self) -> Receiver<BackendEvent>;   // MonitorsChanged, ProcessDied, ...
}
```
Implementasi: `X11Backend`. Backend lain (Wayland) belum ada tetapi trait tidak boleh bocor detail X11.

### 5.2 Supervisor mpv
- Menjalankan mpv sesuai `panduan/06` bagian 2 (argumen disusun fungsi murni `build_args(&PlaySpec) -> Vec<OsString>` yang **di-test unit**).
- Klien IPC: permintaan dengan `request_id`, timeout 2 detik, pembacaan peristiwa terpisah.
- **Health check** tiap 5 detik: `get_property time-pos`/`pause`. Tiga kegagalan beruntun → anggap hang → `kill`.
- **Restart dengan backoff**: 1s, 2s, 4s, 8s, 16s, lalu 30s. Bila ≥ 5 kegagalan dalam 2 menit → state `Failed(VT-MPV-00x)`, berhenti mencoba, kirim sinyal `Error`, tunggu `Apply`/`Retry` dari pengguna.
- Saat daemon keluar atau mati: mpv ikut mati (`PR_SET_PDEATHSIG`); pada keluar normal: `quit` via IPC, tunggu 2 detik, `SIGKILL`.
- Tangkap `stderr` mpv ke log (batasi ukuran) untuk diagnosis.

### 5.3 Backend X11
Sesuai temuan Fase 1: window DESKTOP per monitor, penanganan Caja, pemulihan setting. Tambahan:
- Pantau perubahan monitor (RandR): tambah/hapus/ubah geometri → buat/hancurkan/ubah ukuran window dan arahkan ulang mpv.
- Monitor baru muncul: terapkan wallpaper default (yang sama dengan monitor utama bila diatur "Semua monitor").
- Bila koneksi X putus: keluar bersih (sesi berakhir).

### 5.4 Wallpaper foto
`setter` sesuai `panduan/06` bagian 4. Catat nilai awal sebelum mengubah (simpan di `daemon-state.json` bagian `restore_on_stop`). `Stop` memulihkan nilai awal. Bila setter gagal: cadangan lewat mpv (render foto).

### 5.5 State daemon & restore
- `daemon-state.json` (atomik): per monitor → `ApplyRequest` terakhir, `restore_on_stop`, versi skema.
- `--restore`: baca state, validasi file, terapkan. File hilang → jangan layar hitam, biarkan desktop semula, log + sinyal `Error(VT-IO-00x)`.
- Autostart: GUI/daemon dapat menulis/menghapus `~/.config/autostart/id.vallentheme.Daemon.desktop` (nanti dikendalikan dari Pengaturan Fase 8; sekarang sediakan fungsinya dan flag `--install-autostart`/`--remove-autostart`).
- **Instance tunggal**: `flock` pada `$XDG_RUNTIME_DIR/vallentheme/daemon.lock`. Instance kedua keluar dengan pesan.

### 5.6 D-Bus (`zbus`)
Implementasikan antarmuka `panduan/06` bagian 1: `Apply`, `Stop`, `Pause`, `Resume`, `GetStatus`, `Quit`, sinyal `StateChanged`/`Error`, properti. (`Update`, `SetPerformance`, `GetDecisionLog`, `GetDeviceProfile` menyusul di Fase 6–7; sediakan stub yang mengembalikan "belum didukung" agar versi API stabil.)
- Aktivasi D-Bus: berkas service `id.vallentheme.Daemon.service` agar bus bisa menyalakan daemon sesuai kebutuhan (sertakan di `data/`).
- Validasi semua input (path ada, jenis valid, rentang pengaturan) dan balas error berkode, jangan panic.

### 5.7 Integrasi GUI
- `dbus_client.rs` (zbus, async di GLib via `glib::spawn_future_local` atau jembatan thread): koneksi, panggil `Apply`, berlangganan `StateChanged`/`Error`, deteksi hilangnya pemilik nama.
- `StatusChip` hidup mengikuti state; popover Mulai/Hentikan/Mulai ulang.
- Menu kartu **"Jadikan wallpaper"** aktif: kirim `Apply` dengan pengaturan tersimpan (bawaan bila belum ada); tombol/menu sementara hingga halaman Edit selesai.
- Penanda **"Dipakai"** pada kartu mengikuti `GetStatus`.
- Bila daemon belum jalan: GUI memicu aktivasi D-Bus dan menunggu maksimal 3 detik; gagal → dialog "Wallpaper gagal dijalankan" dengan kode.
- `mpv`/`ffmpeg` tidak ada → dialog yang sesuai (`panduan/05` bagian 2).

### 5.8 Sesi non-X11
Deteksi `XDG_SESSION_TYPE`. Wayland → daemon menolak `Apply` video dengan `VT-UNSUPPORTED-001`; GUI menampilkan dialog "Sesi ini belum didukung". Foto lewat setter desktop tetap boleh (GNOME/MATE/XFCE).

### 5.9 Skrip soak
Isi `scripts/soak.sh`: tiap 60 detik catat waktu, RSS dan CPU% daemon dan mpv ke `soak-<tanggal>.csv`. Tambahkan skenario berkala (ganti wallpaper tiap 10 menit lewat `busctl`/`gdbus call`).

## Kasus tepi
- mpv tidak terpasang / versi lama tanpa `--wid`.
- File sedang diputar dipindah/dihapus (mpv berhenti → daemon mendeteksi → state `Failed` dengan pesan jelas, bukan layar hitam; wallpaper desktop semula terlihat).
- Dua `Apply` beruntun cepat (serialisasi per monitor, yang terakhir menang, tanpa proses bocor).
- `Stop` saat `Starting`.
- Monitor dicabut/dipasang/diganti resolusi saat berjalan.
- Suspend/resume laptop, layar tidur, ganti workspace.
- Jalankan daemon saat DISPLAY belum siap (retry singkat, lalu keluar dengan pesan).
- `daemon-state.json` rusak (cadangkan, mulai bersih).
- Disk penuh saat menyimpan state (log, lanjut tanpa menyimpan).
- GUI ditutup, wallpaper tetap berjalan; GUI dibuka lagi → status sinkron.

## Tes
- Unit: `build_args`, parser IPC, mesin backoff, state machine (tabel transisi).
- Integrasi: **mpv palsu** (skrip/binary kecil yang meniru IPC dan bisa dipaksa crash/hang) untuk menguji supervisor tanpa video nyata.
- Manual terarah mengikuti Gate.

## Gate
- [ ] `kill -9` mpv → pulih < 3 detik; log mencatat penyebab
- [ ] `kill -9` daemon → tidak ada mpv/window yatim; start ulang bersih
- [ ] 5 crash beruntun cepat → state `Failed`, berhenti mencoba, GUI menampilkan pesan jelas
- [ ] Cabut/pasang monitor atau ganti resolusi saat berjalan: tidak crash
- [ ] Logout/login (atau reboot): wallpaper terakhir kembali sendiri
- [ ] `Stop` memulihkan wallpaper dan setting desktop semula
- [ ] Wayland/tanpa mpv → pesan jelas, tidak crash
- [ ] **Soak 4 jam**: memori daemon+mpv stabil (tidak naik terus), tidak ada proses yatim
- [ ] `scripts/check.sh` hijau

## Artefak
Crate `render` dan `daemon`, berkas service D-Bus, klien D-Bus GUI, `scripts/soak.sh`, hasil soak di `docs/PROGRESS.md`.

## Jangan lakukan
Membaca DB dari daemon. Memakai `sh -c`. Mengubah setting desktop tanpa mencatat nilai awal.
