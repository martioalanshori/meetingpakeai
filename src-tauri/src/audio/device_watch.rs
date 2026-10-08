//! Pantau perubahan default device Windows (IMMNotificationClient) selama merekam.
//! Ganti output/mic lewat ikon volume tidak membuat stream lama invalid, jadi tanpa ini
//! loopback tetap menempel ke device lama dan suara peserta hilang diam-diam.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

use wasapi::{DeviceEnumerator, DeviceEventCallbacks, Direction, Role};

use super::Channel;

pub struct DeviceWatcher {
    rx: mpsc::Receiver<Channel>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl DeviceWatcher {
    /// Daftarkan callback di thread COM sendiri. Gagal → `None` (rekaman tetap jalan tanpa pemantauan).
    pub fn start() -> Option<Self> {
        let (tx, rx) = mpsc::channel::<Channel>();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_t = stop.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel::<bool>(1);
        let join = std::thread::Builder::new()
            .name("device-watch".into())
            .spawn(move || {
                let _ = wasapi::initialize_mta();
                let registration = DeviceEnumerator::new().and_then(|enumerator| {
                    let mut callbacks = DeviceEventCallbacks::new();
                    // Callback dipanggil dari thread audio Windows: cukup kirim ke channel lalu kembali.
                    callbacks.set_default_device_callback(move |direction, role, id| {
                        if !matches!(role, Role::Console) {
                            return;
                        }
                        let channel = match direction {
                            Direction::Capture => Channel::Mic,
                            Direction::Render => Channel::System,
                        };
                        tracing::info!("default device {} berubah: {}", channel.as_str(), id.as_deref().unwrap_or("-"));
                        let _ = tx.send(channel);
                    });
                    enumerator.register_notification_callback(callbacks)
                });
                let registration = match registration {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("notifikasi device gagal didaftarkan: {e}");
                        let _ = ready_tx.send(false);
                        return;
                    }
                };
                let _ = ready_tx.send(true);
                while !stop_t.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(200));
                }
                drop(registration);
            })
            .ok()?;
        if ready_rx.recv().unwrap_or(false) {
            Some(Self { rx, stop, join: Some(join) })
        } else {
            let _ = join.join();
            None
        }
    }

    /// Channel yang default device-nya berubah sejak panggilan terakhir.
    pub fn changes(&self) -> Vec<Channel> {
        self.rx.try_iter().collect()
    }
}

impl Drop for DeviceWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
