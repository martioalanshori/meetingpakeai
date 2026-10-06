//! Thread capture per channel (PRD §7.1): tunggu event maks 100 ms, kuras semua paket, kirim ke sink.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

use super::devices::{self, OpenStream};
use super::{AudioError, Channel};

/// Timeout tunggu event, agar thread tetap bisa berhenti/pause walau loopback diam.
const WAIT_TIMEOUT_MS: u32 = 100;

/// Penerima sampel dari thread capture. Dipanggil di thread capture.
pub trait CaptureSink: Send + 'static {
    /// Satu paket sampel 16 kHz mono i16 (sudah nol jika WASAPI menandai `silent`).
    fn on_samples(&mut self, samples: &[i16]);
    /// Dipanggil tiap putaran loop (≈ ≤100 ms), juga saat tidak ada paket.
    fn on_tick(&mut self) {}
    /// Stream gagal di tengah jalan; thread berhenti setelah ini.
    fn on_error(&mut self, err: AudioError);
}

pub struct CaptureHandle {
    pub channel: Channel,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl CaptureHandle {
    /// Minta thread berhenti dan tunggu sampai selesai (sink ikut di-drop).
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Buka device di thread baru lalu mulai capture. Gagal buka → error dikembalikan langsung.
pub fn spawn<S: CaptureSink>(channel: Channel, sink: S) -> Result<CaptureHandle, AudioError> {
    let stop = Arc::new(AtomicBool::new(false));
    let stop_t = stop.clone();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), AudioError>>(1);
    let join = std::thread::Builder::new()
        .name(format!("capture-{}", channel.as_str()))
        .spawn(move || {
            // COM per thread.
            let _ = wasapi::initialize_mta();
            match devices::open_default(channel) {
                Ok(stream) => {
                    let _ = ready_tx.send(Ok(()));
                    run_loop(channel, stream, sink, &stop_t);
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                }
            }
        })?;
    match ready_rx.recv() {
        Ok(Ok(())) => Ok(CaptureHandle { channel, stop, join: Some(join) }),
        Ok(Err(e)) => {
            let _ = join.join();
            Err(e)
        }
        Err(_) => Err(AudioError::Wasapi("thread capture berhenti sebelum siap".into())),
    }
}

fn run_loop<S: CaptureSink>(channel: Channel, stream: OpenStream, mut sink: S, stop: &AtomicBool) {
    let mut bytes: Vec<u8> = Vec::new();
    let mut samples: Vec<i16> = Vec::new();
    let result = (|| -> Result<(), AudioError> {
        let err = |e| AudioError::from_wasapi(e, channel);
        while !stop.load(Ordering::SeqCst) {
            // Timeout bukan error: loopback tidak mengirim event saat tidak ada suara.
            let _ = stream.event.wait_for_event(WAIT_TIMEOUT_MS);
            loop {
                let frames = stream.capture.get_next_packet_size().map_err(err)?.unwrap_or(0) as usize;
                if frames == 0 {
                    break;
                }
                bytes.resize(frames * 2, 0);
                let (got, info) = stream.capture.read_from_device(&mut bytes).map_err(err)?;
                let got = got as usize;
                samples.clear();
                if info.flags.silent {
                    samples.resize(got, 0);
                } else {
                    samples.extend(bytes[..got * 2].as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)));
                }
                sink.on_samples(&samples);
            }
            sink.on_tick();
        }
        Ok(())
    })();
    let _ = stream.client.stop_stream();
    if let Err(e) = result {
        tracing::warn!("capture {} berhenti: {e}", channel.as_str());
        sink.on_error(e);
    }
}
