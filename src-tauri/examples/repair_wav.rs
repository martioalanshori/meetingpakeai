//! Alat uji manual: perbaiki header WAV part yang terputus (simulasi crash).
//!
//!   cargo run --example repair_wav -- <file.wav>

use std::path::PathBuf;

use meeting_pake_ai_lib::audio::writer::repair_wav_header;

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("argumen: <file.wav>"));
    match repair_wav_header(&path) {
        Ok(samples) => println!("header diperbaiki: {samples} sampel = {:.2} detik", samples as f64 / 16_000.0),
        Err(e) => {
            eprintln!("gagal: {e}");
            std::process::exit(1);
        }
    }
}
