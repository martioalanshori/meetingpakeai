//! Alat uji manual langkah 4: rekam mikrofon default ke part WAV 60 detik.
//!
//!   cargo run --example record_mic -- <detik> [folder_output]
//!
//! Contoh: `cargo run --example record_mic -- 300 rekaman-uji` → rekaman-uji/mic_0001.wav … mic_0005.wav

use std::path::PathBuf;
use std::time::{Duration, Instant};

use meeting_pake_ai_lib::audio::capture::{self, CaptureSink};
use meeting_pake_ai_lib::audio::writer::{PartEvent, PartWriter};
use meeting_pake_ai_lib::audio::{AudioError, Channel, SAMPLE_RATE};

struct WriterSink {
    writer: Option<PartWriter>,
    peak: i16,
}

impl CaptureSink for WriterSink {
    fn on_samples(&mut self, samples: &[i16]) {
        if let Some(w) = self.writer.as_mut() {
            if let Err(e) = w.write(samples) {
                eprintln!("gagal menulis: {e}");
            }
        }
        self.peak = samples.iter().fold(self.peak, |p, s| p.max(s.saturating_abs()));
    }

    fn on_error(&mut self, err: AudioError) {
        eprintln!("capture error: {err}");
    }
}

impl Drop for WriterSink {
    fn drop(&mut self) {
        if let Some(w) = self.writer.take() {
            match w.finish() {
                Ok(total) => println!(
                    "selesai: {total} sampel = {:.2} detik, puncak {:.1} dBFS",
                    total as f64 / SAMPLE_RATE as f64,
                    if self.peak == 0 { -90.0 } else { 20.0 * (self.peak as f64 / 32768.0).log10() }
                ),
                Err(e) => eprintln!("gagal menutup part: {e}"),
            }
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let secs: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(10);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "rekaman-uji".into()));

    let writer = PartWriter::new(
        &dir,
        Channel::Mic,
        Box::new(|ev| match ev {
            PartEvent::Opened { path, .. } => println!("part dibuka: {}", path.display()),
            PartEvent::Finalized { part_index, samples, .. } => {
                println!("part {part_index:04} final: {samples} sampel")
            }
        }),
    )
    .expect("buat folder output");

    println!("merekam mic {secs} detik ke {} …", dir.display());
    let started = Instant::now();
    let handle = match capture::spawn(Channel::Mic, WriterSink { writer: Some(writer), peak: 0 }) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("gagal membuka mic: {e}");
            std::process::exit(1);
        }
    };
    std::thread::sleep(Duration::from_secs(secs));
    handle.stop();
    println!("waktu dinding: {:.2} detik", started.elapsed().as_secs_f64());
}
