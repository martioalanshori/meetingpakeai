//! Alat uji manual langkah 5: rekam mic + audio sistem (loopback) dengan pause/mute terjadwal.
//!
//!   cargo run --example record_both -- <detik> [folder] [pause_di pause_lama] [mute_di mute_lama]
//!
//! Contoh: `cargo run --example record_both -- 60 uji 10 20 40 10`
//!   → rekam 60 dtk timeline, pause di dtk 10 selama 20 dtk (dinding), mute mic di dtk timeline 40 selama 10 dtk.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use meeting_pake_ai_lib::audio::recorder::Recorder;
use meeting_pake_ai_lib::audio::writer::PartEvent;
use meeting_pake_ai_lib::audio::SAMPLE_RATE;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let num = |i: usize| a.get(i).and_then(|s| s.parse::<u64>().ok());
    let secs = num(0).unwrap_or(20);
    let dir = PathBuf::from(a.get(1).cloned().unwrap_or_else(|| "rekaman-uji".into()));
    let pause = num(2).zip(num(3));
    let mute = num(4).zip(num(5));

    let rec = match Recorder::start(
        &dir,
        Arc::new(|ev| {
            if let PartEvent::Finalized { channel, part_index, samples } = ev {
                println!("  part {}_{part_index:04} final: {samples} sampel", channel.as_str());
            }
        }),
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gagal mulai: {e}");
            std::process::exit(1);
        }
    };
    println!("merekam {secs} dtk timeline ke {} (Ctrl+C untuk batal)", dir.display());

    let wall = Instant::now();
    let mut paused_done = false;
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let t = rec.elapsed().as_secs();
        if let Some((at, len)) = pause {
            if !paused_done && t >= at {
                println!("[{t:>3}] PAUSE {len} dtk");
                rec.pause();
                std::thread::sleep(Duration::from_secs(len));
                rec.resume();
                println!("[{:>3}] RESUME", rec.elapsed().as_secs());
                paused_done = true;
            }
        }
        if let Some((at, len)) = mute {
            let want = t >= at && t < at + len;
            if want != rec.mic_muted() {
                println!("[{t:>3}] mic {}", if want { "MUTE" } else { "UNMUTE" });
                rec.set_mic_muted(want);
            }
        }
        let (m, s) = rec.levels();
        println!("[{t:>3}] mic {m:>6.1} dBFS | sistem {s:>6.1} dBFS");
        if t >= secs {
            break;
        }
    }
    match rec.stop() {
        Ok(r) => {
            let sec = |n: u64| n as f64 / SAMPLE_RATE as f64;
            println!(
                "selesai: timeline {:.3} dtk | mic {:.3} dtk | sistem {:.3} dtk | selisih {:.1} ms | waktu dinding {:.1} dtk",
                r.duration_ms as f64 / 1000.0,
                sec(r.mic_samples),
                sec(r.system_samples),
                (sec(r.mic_samples) - sec(r.system_samples)).abs() * 1000.0,
                wall.elapsed().as_secs_f64()
            );
        }
        Err(e) => eprintln!("gagal stop: {e}"),
    }
}
