//! Klien IPC mpv: JSON per baris lewat soket Unix.
//!
//! Setiap permintaan membawa `request_id`; balasan dicocokkan dengan ID itu,
//! sementara peristiwa tanpa ID diabaikan. Batas waktu bawaan 2 detik sesuai
//! `panduan/06-referensi-teknis.md` bagian 2.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::error::{RenderError, Result};

/// Batas waktu bawaan untuk satu permintaan IPC.
pub const BATAS_WAKTU: Duration = Duration::from_secs(2);

/// Hasil klasifikasi satu baris dari mpv.
#[derive(Debug, Clone, PartialEq)]
pub enum BarisBalasan {
    /// Balasan untuk `request_id` yang ditunggu.
    Cocok(Value),
    /// Peristiwa asinkron (tidak punya `request_id`).
    Peristiwa,
    /// Balasan untuk permintaan lain atau objek tanpa arti.
    Lain,
}

/// Klasifikasikan satu baris JSON dari mpv.
///
/// Baris kosong dianggap [`BarisBalasan::Lain`], sedangkan JSON rusak menjadi
/// [`RenderError::Ipc`] agar pemanggil bisa mencatat penyebabnya.
pub fn klasifikasi_baris(baris: &str, request_id: u64) -> Result<BarisBalasan> {
    let teks = baris.trim();
    if teks.is_empty() {
        return Ok(BarisBalasan::Lain);
    }
    let nilai: Value = serde_json::from_str(teks)
        .map_err(|err| RenderError::Ipc(format!("balasan mpv bukan JSON: {err}")))?;

    match nilai.get("request_id").and_then(Value::as_u64) {
        Some(id) if id == request_id => Ok(BarisBalasan::Cocok(nilai)),
        Some(_) => Ok(BarisBalasan::Lain),
        None => {
            if nilai.get("event").is_some() {
                Ok(BarisBalasan::Peristiwa)
            } else {
                Ok(BarisBalasan::Lain)
            }
        }
    }
}

/// Koneksi IPC ke satu instans mpv.
pub struct Ipc {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: u64,
    batas: Duration,
}

impl Ipc {
    /// Hubungkan ke soket mpv, menunggu sampai `timeout` bila soket belum siap.
    pub fn connect(path: &Path, timeout: Duration) -> Result<Self> {
        let batas = Instant::now() + timeout;
        let stream = loop {
            match UnixStream::connect(path) {
                Ok(stream) => break stream,
                Err(err) if Instant::now() < batas => {
                    if err.kind() != std::io::ErrorKind::NotFound
                        && err.kind() != std::io::ErrorKind::ConnectionRefused
                    {
                        return Err(RenderError::Ipc(format!(
                            "tidak bisa menghubungi soket mpv {}: {err}",
                            path.display()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(err) => {
                    return Err(RenderError::Ipc(format!(
                        "soket mpv {} tidak siap: {err}",
                        path.display()
                    )))
                }
            }
        };

        stream
            .set_read_timeout(Some(timeout))
            .map_err(|err| RenderError::Ipc(err.to_string()))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|err| RenderError::Ipc(err.to_string()))?;
        let pembaca = stream
            .try_clone()
            .map_err(|err| RenderError::Ipc(err.to_string()))?;

        Ok(Self {
            writer: stream,
            reader: BufReader::new(pembaca),
            next_id: 1,
            batas: timeout,
        })
    }

    /// Baca satu properti mpv, misalnya `time-pos` atau `pause`.
    pub fn get_property(&mut self, nama: &str) -> Result<Value> {
        self.permintaan(json!(["get_property", nama]))
    }

    /// Tulis satu properti mpv.
    pub fn set_property(&mut self, nama: &str, nilai: Value) -> Result<()> {
        self.permintaan(json!(["set_property", nama, nilai]))?;
        Ok(())
    }

    /// Kirim perintah tanpa menunggu balasan (dipakai untuk `quit`).
    pub fn kirim_tanpa_balas(&mut self, perintah: Value) -> Result<()> {
        let pesan = json!({ "command": perintah, "request_id": self.next_id });
        self.next_id = self.next_id.wrapping_add(1);
        self.tulis_baris(&pesan)
    }

    /// Kirim perintah dan tunggu balasan yang cocok.
    fn permintaan(&mut self, perintah: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let pesan = json!({ "command": perintah, "request_id": id });
        self.tulis_baris(&pesan)?;

        let batas = Instant::now() + self.batas;
        loop {
            if Instant::now() >= batas {
                return Err(RenderError::IpcTimeout);
            }

            let mut baris = String::new();
            match self.reader.read_line(&mut baris) {
                Ok(0) => return Err(RenderError::IpcDisconnected),
                Ok(_) => {}
                Err(err)
                    if err.kind() == std::io::ErrorKind::WouldBlock
                        || err.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return Err(RenderError::IpcTimeout)
                }
                Err(err) => return Err(RenderError::Ipc(err.to_string())),
            }

            match klasifikasi_baris(&baris, id)? {
                BarisBalasan::Cocok(nilai) => {
                    let status = nilai
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("success");
                    if status != "success" {
                        return Err(RenderError::Ipc(format!("mpv menolak perintah: {status}")));
                    }
                    return Ok(nilai.get("data").cloned().unwrap_or(Value::Null));
                }
                BarisBalasan::Peristiwa | BarisBalasan::Lain => continue,
            }
        }
    }

    /// Tulis satu objek JSON diikuti baris baru.
    fn tulis_baris(&mut self, pesan: &Value) -> Result<()> {
        let teks = serde_json::to_string(pesan)
            .map_err(|err| RenderError::Ipc(format!("tidak bisa menyusun IPC: {err}")))?;
        self.writer
            .write_all(teks.as_bytes())
            .and_then(|_| self.writer.write_all(b"\n"))
            .and_then(|_| self.writer.flush())
            .map_err(|err| {
                if err.kind() == std::io::ErrorKind::BrokenPipe
                    || err.kind() == std::io::ErrorKind::ConnectionReset
                {
                    RenderError::IpcDisconnected
                } else {
                    RenderError::Ipc(err.to_string())
                }
            })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn balasan_cocok_dikenali() {
        let hasil =
            klasifikasi_baris(r#"{"request_id":7,"error":"success","data":false}"#, 7).unwrap();
        match hasil {
            BarisBalasan::Cocok(nilai) => {
                assert_eq!(nilai["error"], "success");
                assert_eq!(nilai["data"], false);
            }
            lain => panic!("harus cocok, dapat {lain:?}"),
        }
    }

    #[test]
    fn peristiwa_tanpa_request_id_diabaikan() {
        let hasil = klasifikasi_baris(
            r#"{"event":"end-file","reason":"eof","playlist_entry_id":1}"#,
            7,
        )
        .unwrap();
        assert_eq!(hasil, BarisBalasan::Peristiwa);
    }

    #[test]
    fn balasan_id_lain_bukan_untuk_kita() {
        let hasil = klasifikasi_baris(r#"{"request_id":8,"error":"success","data":1}"#, 7).unwrap();
        assert_eq!(hasil, BarisBalasan::Lain);
    }

    #[test]
    fn objek_tanpa_id_dan_tanpa_event_diabaikan() {
        let hasil = klasifikasi_baris(r#"{"error":"success"}"#, 7).unwrap();
        assert_eq!(hasil, BarisBalasan::Lain);
    }

    #[test]
    fn baris_kosong_diabaikan() {
        assert_eq!(klasifikasi_baris("   ", 1).unwrap(), BarisBalasan::Lain);
    }

    #[test]
    fn json_rusak_menghasilkan_error_berkode() {
        let err = klasifikasi_baris("{bukan json", 1).unwrap_err();
        assert_eq!(err.code(), "VT-MPV-002");
    }

    #[test]
    fn request_id_negatif_tidak_dianggap_cocok() {
        let hasil = klasifikasi_baris(r#"{"request_id":-7,"event":"x"}"#, 7).unwrap();
        assert_eq!(hasil, BarisBalasan::Peristiwa);
    }
}
