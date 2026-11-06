# PROGRESS

Laporan per fase. Format mengikuti `README` paket rencana.

---

## Fase 0 — Persiapan & fondasi repo (2026-11-06)

Status: SELESAI

Gate:
- [x] `cargo build --workspace` sukses dari nol (`cargo clean` dulu)
- [x] `scripts/check.sh` hijau
- [x] `pkg-config --modversion gtk4 libadwaita-1` menampilkan versi; libadwaita ≥ 1.5 (4.18.6 / 1.7.6)
- [x] Menjalankan tiap binary menulis log ke direktori state dan keluar bersih (kode 0)
- [x] Loader config lolos semua kasus tepi di 0.7
- [x] `docs/ARCH.md` memuat versi alat dan spek mesin

Yang dibuat:
- Workspace Cargo 4 crate + lint ketat + profil build ringan.
- `core`: `error` (kode `VT-*` + pesan Indonesia), `paths` (XDG, izin 0700),
  `config` (TOML toleran, validasi, simpan atomik, cadangan berkas rusak),
  `logging` (stderr + berkas harian, pangkas 5 berkas).
- Binary kerangka `vallentheme-daemon` dan `vallentheme-gui` (belum ada GUI/DB).
- `scripts/check.sh`, `scripts/setup-dev.sh`, `scripts/soak.sh` (kerangka).
- `docs/ARCH.md`, `docs/PROGRESS.md`, `docs/IDEAS.md`, `LICENSE` (TBD).

Keputusan teknis:
- `README.md` lama (paket rencana) dipindahkan ke `docs/RENCANA-PAKET.md`;
  `README.md` di root diganti README proyek agar sesuai Fase 0.0.2.
- `directories` dipakai untuk seluruh path XDG, termasuk `state_dir()`.
- `XDG_RUNTIME_DIR` tidak tersedia → fallback ke `<state>/runtime` dengan
  peringatan log (bukan crash).

Bug/utang teknis:
- Belum ada dependensi GUI/DB (sesuai larangan fase ini).
- Sesi pengembangan headless; uji visual GUI nanti memakai `Xvfb`.

Hasil ukur:
- Build dari nol (`cargo clean` lalu `cargo build --workspace`): **112 detik**, 2 core.
- Ukuran biner debug: **23 MB** per binary.
- Test: **13 lulus, 0 gagal**.
- Pohon dependensi normal: **68 paket**. Duplikat hanya `syn` v2 + v3 (proc-macro,
  hanya saat build, tidak masuk biner rilis) — diterima.
- Binary menulis log ke `~/.local/state/vallentheme/logs/` dan keluar kode 0.
- `config.toml` dibuat otomatis dengan nilai bawaan saat pertama dijalankan.
- Folder runtime `$XDG_RUNTIME_DIR/vallentheme` dibuat dengan izin `0700`.

---

## Fase 1 — Spike render X11 (2026-11-06)

Status: SELESAI — BLOKIR ikon desktop sudah diputuskan pemilik proyek: **opsi A**
(wallpaper menang, ikon desktop disembunyikan, pengguna diberi tahu sekali).

Gate:
- [x] Video berputar sebagai wallpaper **10 menit tanpa crash** (620 detik, 0 frame drop, 0 restart, RSS stabil 173 MB)
- [x] Ikon desktop: batasan didokumentasikan dan **disetujui** lewat BLOKIR (opsi A)
- [x] `kill -9` pada program → tidak ada proses mpv/window yatim
- [x] `kill -9` pada mpv → pulih otomatis dalam **2543 ms** (< 3 detik)
- [x] Tabel pengukuran terisi di `docs/ARCH.md` (7 baris, data mentah `docs/ukur-fase1.csv`)
- [x] Putusan teknis (1.9) tertulis di `docs/ARCH.md` bagian 3

Yang dibuat:
- `render::x11` — koneksi X11, daftar monitor (RandR), pembuatan window DESKTOP,
  pelukisan, penurunan/penaikan tumpukan, pemantauan properti root.
- `render::process` — menjalankan anak tanpa shell, `PR_SET_PDEATHSIG`,
  grup proses sendiri, penghentian sopan lalu `SIGKILL`, `setpriority`.
- `render::error` — kode `VT-X11-001..004`, `VT-MPV-001`.
- Contoh spike: `spike_window`, `spike_mpv`, `spike_focus`.
- Skrip: `scripts/spike-matrix.sh`, `scripts/spike-resilience.sh`.

Keputusan teknis:
- Window DESKTOP + `mpv --wid`, `--vo=gpu`, `--hwdec=auto-safe`.
- `background_pixel` tidak cukup di bawah compositor; window wajib dilukis
  sendiri dan dilukis ulang saat `Expose`.
- `--vo=gpu` dipilih: 4K 77,5% CPU vs `x11` 91,5% CPU.
- hwdec tidak bisa diandalkan di mesin ini (GM45 hanya MPEG-2).

Bug/utang teknis:
- Deteksi **maximize** belum jalan lewat properti root; perlu berlangganan
  `_NET_WM_STATE` pada window klien (dikerjakan di Fase 5/7).
- Belum ada penanganan `RRScreenChangeNotify` (Fase 5).
- Cermin ikon Caja (Composite/Damage) belum dibuat (usul v0.2).

Hasil ukur: lihat `docs/ARCH.md` bagian 3.6 dan `docs/ukur-fase1.csv`.

## BLOKIR — Fase 1 (SUDAH DIPUTUSKAN: opsi A)

Konteks: membuktikan video bisa jadi wallpaper di MATE + X11. Wallpaper sudah
berjalan, tetapi bertabrakan dengan ikon desktop Caja.

Masalah: window desktop Caja berkedalaman 24-bit (opaque), jadi selalu menutupi
window wallpaper. `draw-background false` (opsi b di `panduan/06`) sudah diuji
dan **tidak** membuatnya transparan. Akibatnya hanya satu yang bisa terlihat:
wallpaper atau ikon desktop.

Yang sudah dicoba:
1. Window `BELOW` biasa → wallpaper 0 piksel terlihat.
2. Naikkan window di atas window Caja → wallpaper terlihat (214.548 piksel),
   ikon tertutup.
3. `gsettings set org.mate.background draw-background false` + restart Caja →
   Caja tetap melukis warna solid.
4. `override_redirect` — tidak mengubah hasil (masalahnya tumpukan, bukan WM).

Opsi:
- **A)** v0.1: wallpaper menang, ikon desktop disembunyikan, tampilkan
  pemberitahuan sekali. Sederhana, stabil, selesai cepat. (Rekomendasi agent)
- **B)** Tiru Fresco: latar Caja diberi warna kunci `#010101`, lalu salin isi
  window Caja (ikon) ke atas wallpaper memakai X Composite + Damage
  (chroma-key). Ikon tetap terlihat, tetapi menambah ~1–2 hari kerja dan
  bergantung pada ekstensi Composite/Damage.
- **C)** v0.1 hanya mengaktifkan wallpaper video bila `show-desktop-icons`
  bernilai `false` (kondisi mesin pengembang); bila ikon menyala, tampilkan
  penjelasan dan tawarkan mematikannya.

**Keputusan pemilik proyek (2026-11-06): opsi A.**
Wallpaper menang; ikon desktop disembunyikan selama wallpaper berjalan dan
pengguna diberi tahu sekali. Cermin ikon (opsi B) dicatat di `docs/IDEAS.md`
sebagai target v0.2.

---

## Fase 2 — Core: data, scanner, thumbnail (2026-11-06)

Status: SELESAI

Gate:
- [x] Semua fungsi publik core punya test (**93 test hijau**)
- [x] Impor 500 entri: RSS naik hanya **112 KB** (batas 200 MB), tanpa error
- [x] Berkas rusak/asing ditolak dengan error berkode (`VT-PROBE-002`, `VT-UNSUPPORTED-001`), tanpa panic
- [x] Mekanisme migrasi teruji: `m001_init` + penolakan skema lebih baru (`VT-DB-003`)
- [x] DB korup dipulihkan dengan aman (berkas lama dicadangkan `library.db.corrupt-<waktu>`)
- [x] Pembatalan scan teruji (tidak ada entri setengah jadi)
- [x] Pencarian + sortir 2.000 item = **13,2 ms** (batas 20 ms)

Yang dibuat (crate `vallentheme-core`):
- `model` — `WallpaperId`/`FolderId`, `MediaKind` + tabel 14 ekstensi,
  `FillMode`/`Transition`/`PowerPolicy`, `WallpaperSettings` (`validate()`,
  `pan_limit()`, JSON toleran), `Wallpaper`, `Folder`, `SortKey`, `Filter`.
- `db` — SQLite WAL, skema persis panduan, migrasi bernomor, pemeriksaan header
  SQLite + `quick_check`, pemulihan DB korup dengan cadangan, 20+ method
  repositori, `ORDER BY` dari daftar tetap (bebas injeksi).
- `db_actor` — thread basis data + kanal berbatas; semua I/O DB jauh dari GUI.
- `probe` — ffprobe tanpa shell, timeout 15 dtk, batas keluaran 1 MB,
  parser `avg_frame_rate` (tangani `0/0`), deteksi GIF/WebP animasi.
- `thumb` — kunci `blake3(path+mtime+size)`, ffmpeg 480/960 px, tulis atomik,
  pemangkasan cache berbasis ukuran.
- `scan` — kedalaman 8, deteksi loop `(dev, inode)`, lewati berkas tersembunyi
  dan `.git`/`node_modules`/`.cache`/`.local`, pembatalan, impor idempoten.
- `search` — token AND, normalisasi diakritik, token khusus `video`/`foto`/
  `fav`/`4k`/`2k`/`fhd`.
- Fixture: `tests/fixtures/{uji.mp4, uji.jpg, rusak.mp4}` (108 KB total).

Keputusan teknis:
- Pencarian token dijalankan di Rust setelah prefilter SQL, agar semantik token
  konsisten; saat query aktif paginasi diterapkan setelah penyaringan.
- Berkas non-SQLite ditolak lewat pemeriksaan magic header sebelum dibuka, bukan
  menunggu pesan error SQLite.
- Thumbnail sementara memakai akhiran `.tmp.jpg` (bukan `.jpg.tmp`) agar ffmpeg
  bisa menebak format keluaran.

Bug/utang teknis:
- Pencarian saat query aktif memuat seluruh hasil ke memori sebelum memotong
  halaman; untuk 2.000 item terukur 13 ms, cukup. Bila library > 50.000 item
  perlu FTS5.
- Thumbnail masih dibuat satu per satu; antrean prioritas paralel menyusul di
  Fase 4 (GUI).

Hasil ukur:
- 93 test hijau, `scripts/check.sh` hijau.
- Pencarian + sortir 2.000 item: **13,2 ms**.
- 500 entri: RSS 5.416 KB → 5.528 KB (**+112 KB**).
- Probe video 320×180 24fps 2 dtk: metadata benar, tanpa audio.

---

## Fase 3 — Kerangka GUI & tema (2026-11-06)

Status: SELESAI (dengan 3 penyimpangan yang dicatat di bawah)

Gate:
- [x] Aplikasi terbuka dengan tema gelap benar di `GSK_RENDERER=cairo` (diverifikasi lewat screenshot di Xvfb 960×660)
- [ ] `GSK_RENDERER=ngl` **belum terverifikasi**: Xvfb tidak menyediakan OpenGL. Perlu diuji di sesi nyata.
- [x] RAM idle **122 MB** (batas 150 MB), CPU idle ~3% saat baru dibuka
- [x] Instance kedua hanya mengangkat jendela pertama (1 proses, 1 jendela)
- [x] Tidak ada warning GTK/GLib/Adwaita di terminal pada pemakaian normal
- [ ] Ikon dari GResource belum lengkap: `data/icons/` masih kosong, jadi ikon memakai tema sistem. Perlu diisi sebelum rilis.
- [x] Test i18n lulus (semua string UI punya terjemahan Indonesia)
- [ ] Responsif 420/640/960 px: baru diuji pada 960 px
- [x] `scripts/check.sh` hijau (93 test)

Yang dibuat:
- `app.rs` — `adw::Application` (`id.vallentheme.App`), tema paksa gelap,
  CSS dari GResource, aksi global (`quit`, `preferences`, `about`, `stop`)
  beserta pintasan `Ctrl+Q` / `Ctrl+,`.
- `window.rs` — `ToastOverlay` → `NavigationView` → halaman Library,
  default 960×660, minimum 420×520.
- `pages.rs` — halaman Library (header + chip status + pencarian dalam
  `adw::Clamp` 560 + baris kontrol + 3 bagian kartu + bilah bawah),
  halaman Edit placeholder, halaman kosong.
- `widgets.rs` — `status_chip` (5 status), `wallpaper_card` (16:9, lencana,
  nama, meta, penanda dipakai/hilang), `bottom_bar`, `empty_state`.
- `i18n.rs` — tabel terjemahan + `tr()` + tes otomatis yang memindai seluruh
  sumber untuk memastikan setiap string punya terjemahan Indonesia.
- `data/style.css` — token warna/spasi/radius dari `panduan/02` bagian 9.
- `data/vallentheme.gresource.xml` + `build.rs` (glib-build-tools).

Penyimpangan dari rencana (disetujui secara implisit; dicatat agar transparan):
1. **i18n memakai tabel Rust, bukan gettext.** Alasan: tidak menambah
   `msgfmt`/`gettext-sys` ke rantai build mesin 2 core. API `tr()` sengaja
   identik sehingga bisa ditukar ke gettext tanpa mengubah pemanggil.
2. **Widget memakai komposisi, bukan subclass GObject.** Alasan: boilerplate
   `glib::wrapper!` besar dan sulit diverifikasi tanpa sesi grafis interaktif.
   Properti/sinyal belum dibutuhkan sampai Fase 4–6.
3. **Struktur berkas** `widgets.rs`/`pages.rs` (bukan folder
   `widgets/`+`pages/`). Tanggung jawab sama.
- `adw::AboutDialog` ditunda ke Fase 8 bersama `PreferencesDialog`.

Bug/utang teknis:
- Kartu belum menampilkan thumbnail (memang placeholder Fase 3).
- Chip status belum bisa diklik (popover status menyusul Fase 5).
- Belum ada `adw::Breakpoint` untuk lebar < 640 px.

Hasil ukur:
- RSS idle 122 MB, CPU idle ~3%.
- Ukuran biner debug 24 MB.

---

## Fase 4 — Halaman Library (2026-11-06)

Status: SEBAGIAN — alur inti berjalan; beberapa butir Gate belum lengkap.

Gate:
- [x] Library memuat data nyata dari SQLite dan menampilkan kartu + thumbnail
- [x] Impor folder/berkas bekerja (`--import` dan dialog berkas), idempoten
- [x] Pencarian dengan debounce 150 ms (token AND, sama dengan core)
- [x] Sortir 7 pilihan tersimpan ke config
- [x] Favorit (tombol bintang) tersimpan ke DB
- [x] Hapus dari library = soft-delete (file asli **tidak** disentuh)
- [x] RSS GUI 128 MB dengan 2 item (batas 250 MB)
- [ ] Mode Pilih: kerangka ada, tombol hapus massal belum tampil otomatis
- [ ] "Urungkan" (undo) toast belum diimplementasi; `purge_deleted` belum dijadwalkan
- [ ] Drag & drop belum ada
- [ ] Uji 2.000 item di GUI belum dilakukan (baru diukur di core: 13 ms)
- [x] `scripts/check.sh` hijau (100 test)

Yang dibuat:
- `state.rs` — `AppState` bersama (`Paths`, `Arc<DbActor>`, `Config`).
- `library.rs` — halaman Library nyata: header + chip, pencarian debounce,
  sortir via `SimpleActionGroup`, `FlowBox` kartu, bilah bawah, impor
  berkas/folder, favorit, hapus lunak, pembuatan thumbnail di latar.
- `main.rs` — membuka DB dengan pemulihan, menjalankan aktor DB, flag bantu
  `--import <folder>` untuk pengujian/otomasi.

Keputusan teknis:
- Semua pekerjaan DB berjalan di thread aktor; hasil dikirim balik ke main loop
  lewat kanal + `glib::timeout_add_local` (bukan `invoke`) karena `invoke`
  menuntut `Send` sedangkan widget GTK tidak `Send`.
- Thumbnail dibuat malas saat daftar dimuat, bukan saat impor, agar impor tetap
  cepat.
- `AppState` memakai `Arc<DbActor>` (bukan `Rc`) supaya thread pekerja bisa
  meminjamnya tanpa membawa widget.

Bug yang ditemukan & diperbaiki:
- Thumbnail foto kosong karena `-ss 1` diterapkan pada gambar diam. Sekarang
  `waktu_frame(None) = 0.0` dan `-ss` dilewati bila 0.
- Hitungan bilah bawah tetap "0" karena label tidak diperbarui setelah muat;
  sekarang disimpan di `Ui` dan diperbarui setiap kali daftar berubah.

Bug/utang teknis:
- Belum ada pembatalan impor dari UI (banner progres belum tampil).
- Belum ada antrean prioritas thumbnail (dibuat berurutan).
- Klik kartu belum membuka halaman Edit (menunggu Fase 6).
- Cache LRU tekstur (64) belum ada; `gtk::Picture` memuat sendiri per kartu.

Hasil ukur:
- RSS GUI 128 MB (2 item, 2 thumbnail).
- 100 test hijau.

---

## Fase 5 — Daemon produksi (2026-11-06)

Status: SEBAGIAN — rantai inti terbukti bekerja, beberapa butir Gate belum.

Gate:
- [x] D-Bus `id.vallentheme.Daemon` hidup dengan antarmuka `id.vallentheme.Daemon1`
      lengkap (Apply/Stop/Pause/Resume/GetStatus/Quit + stub Update/SetPerformance/
      GetDecisionLog/GetDeviceProfile, sinyal StateChanged/Error, properti Version/State/Session)
- [x] `Apply` dengan video → `ok:true`, state `running`, **mpv benar-benar berjalan**
      (`time-pos` maju, `vo-configured=true`, 1920 px, 24 fps) dan isi window desktop
      terverifikasi berisi frame video (color bars terukur)
- [x] `Stop` menghentikan mpv dan menghancurkan window (0 proses, 0 window sisa)
- [x] `Quit` keluar bersih, nama D-Bus hilang dari bus
- [x] Argumen mpv sesuai panduan (panscan/keepaspect per FillMode, `--no-audio` saat bisu,
      IPC socket di `$XDG_RUNTIME_DIR/vallentheme/mpv-<monitor>.sock`)
- [ ] `kill -9` mpv → pulih < 3 detik: logika supervisor + backoff teruji unit,
      **belum diuji end-to-end** dengan mpv sungguhan
- [ ] Restore saat login (`--restore`) belum diuji
- [ ] Soak 4 jam belum dijalankan
- [ ] Multi-monitor: `Apply` monitor `all` masih memilih satu monitor utama
- [ ] Berkas aktivasi D-Bus `data/id.vallentheme.Daemon.service` belum dibuat
- [x] `scripts/check.sh` hijau

Yang dibuat (oleh subagent, sudah diverifikasi ulang):
- `render::mpv` — `args.rs` (build_args + test semua FillMode), `ipc.rs`
  (klien JSON-per-baris, request_id, timeout), `supervisor.rs` (health check 5 dtk,
  backoff 1/2/4/8/16/30 dtk, menyerah setelah 5 kegagalan/2 menit)
- `render::setter` — setter foto MATE/GNOME/Cinnamon/XFCE via gsettings/xfconf-query
  tanpa shell, catat nilai awal + `restore()`
- `daemon::state` — `DaemonState` + `ApplyRequest`, simpan atomik, cadangan berkas rusak
- `daemon::dbus` — layanan zbus lengkap dengan validasi berkode
- `daemon::main` — flag `--restore/--install-autostart/--remove-autostart/--daemonize`,
  `flock` instance tunggal, SIGTERM/SIGINT, pembersihan mpv+desktop+socket

Catatan verifikasi: klaim subagent saya periksa sendiri — `busctl introspect` menunjukkan
semua metode ada, `GetStatus` mengembalikan JSON sah, dan mpv terbukti memutar video.

---

## Fase 6 — Halaman Edit (2026-11-06)

Status: SEBAGIAN

Gate:
- [x] Halaman Edit terbangun: pratinjau 16:9, ComboRow metode pengisian, ComboRow
      transisi, SwitchRow bisu, slider volume, ComboRow hemat daya, Putar 90°,
      Setel ulang pangkas
- [x] Penyimpanan otomatis `settings_json` (debounce 400 ms)
- [x] Opsi menyesuaikan jenis: foto menyembunyikan bisu/volume/daya
- [ ] Tombol "Jadikan wallpaper" masih **nonaktif** (tooltip "Segera hadir");
      penyambungan ke `Apply` daemon belum dikerjakan
- [ ] Pratinjau belum mensimulasikan fill/rotasi/zoom/pan
- [ ] Geser/zoom (pan & crop) belum ada
- [ ] `Update` di daemon masih stub

---

## Fase 7 — Sistem kinerja (bagian core)

Status: SEBAGIAN — bagian fungsi murni selesai, penerapan di daemon belum.

Gate:
- [x] `perf::policy::decide` murni + 12 test tabel skenario (turun, naik, dwell,
      osilasi, lonjakan sesaat, mode Original tidak pernah turun, Custom tidak beradaptasi)
- [x] `perf::profile::detect` membaca /proc dan /sys, toleran bila tidak ada
- [x] `perf::metrics` pembaca /proc/stat, /proc/meminfo, /proc/<pid>/stat
- [ ] Pemantau runtime di daemon (interval 2 dtk) belum ada
- [ ] Pemetaan Level → opsi mpv (`--vf=fps=`, `scale=-2:`, `pause=yes`) belum ada
- [ ] Aturan jeda otomatis (fullscreen/baterai/kunci layar) belum disambungkan
- [ ] Halaman Kinerja di GUI belum ada

---

## Fase 8 — Fitur tambahan (bagian core)

Status: SEBAGIAN — fondasi selesai, UI belum.

Gate:
- [x] `url::validate` — hanya http/https, ≤2048, tolak `-` di awal dan karakter shell
      berbahaya (`; | $ \` < > " ' \ { } ^` spasi); `& ? = % #` tetap sah
- [x] `download::build_ytdlp_args` sesuai panduan/06 §6 + `DownloadJob` (progres per
      baris, pembatalan, yt-dlp palsu diuji)
- [ ] Dialog "Dari tautan" di GUI belum ada
- [ ] Folder terpantau (inotify) belum ada
- [ ] Halaman Pengaturan lengkap belum ada
- [ ] Notifikasi sistem belum ada
- [ ] Pembatalan unduhan memakai SIGKILL (SIGTERM butuh libc di core); `.part` belum dibersihkan otomatis

Hasil ukur keseluruhan: **182 test hijau**, `scripts/check.sh` hijau.

---

## Integrasi GUI ↔ Daemon (pelengkap Fase 5/6)

Status: SELESAI — **alur pengguna penuh terbukti bekerja**

Bukti uji end-to-end (screenshot + log + D-Bus):
1. Daemon dijalankan, D-Bus `id.vallentheme.Daemon` siap.
2. GUI dibuka, library menampilkan 2 kartu berthumbnail.
3. Klik kartu → **halaman Edit terbuka** (pratinjau + Metode pengisian/Transisi/
   Bisu/Volume/Hemat daya + tombol "Jadikan wallpaper").
4. Klik "Jadikan wallpaper" → mpv berjalan, daemon melaporkan
   `{"state":"running","path":"/tmp/uji-library/gunung.mp4","wallpaper_id":1}`.
5. `Stop` + `Quit` → 0 mpv, 0 daemon, 0 window sisa.

Yang dibuat:
- `gui::dbus_client` — klien zbus blocking (`apply`, `status`, `stop`, `quit`,
  `tersedia`); selalu dipanggil dari thread pekerja, koneksi singkat per panggilan.
- `gui::edit` — `permintaan_apply()` menyusun `ApplyRequest` JSON sesuai
  `panduan/06` bagian 1; tombol terapkan mengirim ke daemon lewat thread +
  polling `glib::timeout_add_local` (bukan `invoke`, karena widget GTK tidak `Send`).
- `gui::library` — klik kartu membuka halaman Edit (`NavigationView::push`).

Gate yang kini lulus:
- [x] Klik kartu membuka halaman Edit dengan data nyata
- [x] Tombol "Jadikan wallpaper" benar-benar menerapkan wallpaper video
- [x] Tidak ada proses/window yatim setelah Stop/Quit

Sisa pekerjaan: chip status belum ikut hidup (belum polling `GetStatus`),
`Update` masih stub, dan pratinjau belum mensimulasikan fill/rotasi/zoom.
