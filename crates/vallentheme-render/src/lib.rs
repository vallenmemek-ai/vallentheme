//! Backend render vallentheme.
//!
//! Crate ini menyembunyikan detail X11 dan mpv di balik trait `RenderBackend`.
//! Crate ini tidak boleh menyentuh basis data maupun GTK.

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
