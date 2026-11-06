//! Pustaka inti vallentheme.
//!
//! Crate ini tidak boleh menyentuh GTK, X11, maupun D-Bus. Isinya adalah
//! model data, akses berkas menurut XDG, konfigurasi pengguna, dan logging.

pub mod config;
pub mod error;
pub mod logging;
pub mod model;
pub mod paths;

pub use error::{Error, Result};
