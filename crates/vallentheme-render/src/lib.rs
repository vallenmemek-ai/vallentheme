//! Backend render vallentheme.
//!
//! Crate ini menyembunyikan detail X11 dan mpv di balik satu pintu. Crate ini
//! tidak boleh menyentuh basis data maupun GTK.
//!
//! Rujukan: `panduan/01-arsitektur.md` bagian 2, `panduan/06` bagian 2–3.

pub mod error;
pub mod mpv;
pub mod process;
pub mod setter;
pub mod x11;

pub use error::{RenderError, Result};

/// Versi crate, dipakai binary untuk mencetak identitas saat start.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    #[test]
    fn versi_tidak_kosong() {
        assert!(!super::VERSION.is_empty());
    }
}
