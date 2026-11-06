//! Tipe error untuk lapisan render.

/// Hasil standar crate render.
pub type Result<T> = std::result::Result<T, RenderError>;

/// Kegagalan saat menghubungi X11 atau menjalankan proses media.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// Koneksi ke server X gagal.
    #[error("tidak bisa terhubung ke server X: {0}")]
    X11Connect(String),

    /// Permintaan X11 ditolak server.
    #[error("permintaan X11 gagal ({konteks}): {pesan}")]
    X11Request {
        /// Operasi yang sedang dilakukan.
        konteks: &'static str,
        /// Pesan dari pustaka X11.
        pesan: String,
    },

    /// `DISPLAY` tidak diatur atau sesi bukan X11.
    #[error("sesi ini bukan X11 (DISPLAY tidak tersedia)")]
    NotX11,

    /// Tidak ada monitor yang terdeteksi.
    #[error("tidak ada monitor yang terdeteksi")]
    NoMonitors,

    /// Proses anak gagal dijalankan.
    #[error("gagal menjalankan {program}: {sumber}")]
    Spawn {
        /// Nama program yang dicoba dijalankan.
        program: String,
        /// Kesalahan dari sistem.
        sumber: std::io::Error,
    },

    /// Pesan IPC mpv tidak bisa dikirim atau dibaca.
    #[error("komunikasi dengan mpv gagal: {0}")]
    Ipc(String),

    /// mpv tidak menjawab dalam batas waktu.
    #[error("mpv tidak menjawab dalam 2 detik")]
    IpcTimeout,

    /// Soket IPC mpv tertutup (mpv berhenti).
    #[error("soket mpv terputus; proses mpv kemungkinan berhenti")]
    IpcDisconnected,

    /// mpv berhenti merespons pemeriksaan kesehatan.
    #[error("mpv tidak merespons pemeriksaan kesehatan")]
    MpvHang,

    /// mpv gagal berulang kali sehingga pengawasan dihentikan.
    #[error("mpv gagal berulang kali: {0}")]
    MpvFailed(String),

    /// Setter wallpaper desktop gagal.
    #[error("gagal mengubah wallpaper {desktop}: {pesan}")]
    Setter {
        /// Nama desktop yang dideteksi.
        desktop: String,
        /// Keterangan kegagalan.
        pesan: String,
    },

    /// Desktop tidak dikenali sehingga setter tidak tersedia.
    #[error("desktop tidak dikenali; pengaturan wallpaper tidak didukung")]
    DesktopUnknown,
}

impl RenderError {
    /// Kode stabil untuk ditampilkan ke pengguna.
    pub fn code(&self) -> &'static str {
        match self {
            Self::X11Connect(_) => "VT-X11-001",
            Self::X11Request { .. } => "VT-X11-002",
            Self::NotX11 => "VT-X11-003",
            Self::NoMonitors => "VT-X11-004",
            Self::Spawn { .. } => "VT-MPV-001",
            Self::Ipc(_) => "VT-MPV-002",
            Self::IpcTimeout => "VT-MPV-003",
            Self::IpcDisconnected => "VT-MPV-004",
            Self::MpvHang => "VT-MPV-005",
            Self::MpvFailed(_) => "VT-MPV-006",
            Self::Setter { .. } => "VT-DESKTOP-001",
            Self::DesktopUnknown => "VT-DESKTOP-002",
        }
    }
}

/// Ubah kesalahan pustaka x11rb menjadi `RenderError`.
pub fn x11_err<E: std::fmt::Display>(konteks: &'static str, err: E) -> RenderError {
    RenderError::X11Request {
        konteks,
        pesan: err.to_string(),
    }
}
