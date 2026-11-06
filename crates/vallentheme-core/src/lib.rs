//! Pustaka inti vallentheme.
//!
//! Crate ini tidak boleh menyentuh GTK, X11, maupun D-Bus. Isinya adalah
//! model data, akses berkas menurut XDG, konfigurasi pengguna, dan logging.

pub mod config;
pub mod db;
pub mod db_actor;
pub mod download;
pub mod error;
pub mod logging;
pub mod model;
pub mod paths;
pub mod perf;
pub mod probe;
pub mod scan;
pub mod search;
pub mod thumb;
pub mod url;

pub use error::{Error, Result};
