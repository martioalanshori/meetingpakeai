//! Alat uji manual langkah 8: tes 5 detik onboarding (mic + loopback + nada tes).
//!
//!   cargo run --example audio_test -- <folder_sementara>

use std::path::PathBuf;

use meeting_pake_ai_lib::audio::test_tone;
use meeting_pake_ai_lib::windows_integration::{self, mic_permission};

fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&dir).unwrap();
    println!("izin mikrofon: {}", mic_permission::check().as_str());
    let tone = dir.join("test_tone.wav");
    test_tone::write_tone(&tone).unwrap();
    let mut n = 0;
    let res = test_tone::run(
        || println!("putar nada: {}", windows_integration::play_wav_async(&tone)),
        |m, s| {
            n += 1;
            if n % 5 == 0 {
                println!("  mic {m:>6.1} | sistem {s:>6.1}");
            }
        },
    )
    .unwrap();
    println!("{res:?}");
}
