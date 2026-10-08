//! Membuka stream WASAPI shared + event-driven untuk satu channel (PRD §7.1).
//! Format diminta langsung 16 kHz mono PCM16 dengan autoconvert Windows.

use wasapi::{
    AudioCaptureClient, AudioClient, DeviceEnumerator, Direction, Handle, SampleType, StreamMode,
    WaveFormat,
};

use super::{AudioError, Channel, SAMPLE_RATE};

/// 200 ms dalam satuan 100 ns.
const MIN_BUFFER_HNS: i64 = 2_000_000;

pub struct OpenStream {
    pub client: AudioClient,
    pub capture: AudioCaptureClient,
    pub event: Handle,
    pub device_name: String,
}

/// Buka default device: `mic` = capture (eConsole), `system` = render dalam mode loopback.
/// Harus dipanggil di thread yang sudah `wasapi::initialize_mta()`.
pub fn open_default(channel: Channel) -> Result<OpenStream, AudioError> {
    let err = |e| AudioError::from_wasapi(e, channel);
    let enumerator = DeviceEnumerator::new().map_err(err)?;
    let device_dir = match channel {
        Channel::Mic => Direction::Capture,
        Channel::System => Direction::Render,
    };
    let device = enumerator.get_default_device(&device_dir).map_err(err)?;
    let device_name = device.get_friendlyname().unwrap_or_else(|_| "?".into());
    let mut client = device.get_iaudioclient().map_err(err)?;

    let format = WaveFormat::new(16, 16, &SampleType::Int, SAMPLE_RATE as usize, 1, None);
    let (default_period, _min_period) = client.get_device_period().map_err(err)?;
    // Buffer ≥ 200 ms: thread capture yang sempat tertahan tidak langsung kehilangan audio.
    let mode = StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: default_period.max(MIN_BUFFER_HNS) };
    // Device render + Direction::Capture = loopback (AUDCLNT_STREAMFLAGS_LOOPBACK).
    client.initialize_client(&format, &Direction::Capture, &mode).map_err(err)?;
    let event = client.set_get_eventhandle().map_err(err)?;
    let capture = client.get_audiocaptureclient().map_err(err)?;
    client.start_stream().map_err(err)?;
    tracing::info!("{} stream dibuka: 16 kHz mono i16 (autoconvert)", channel.as_str());
    Ok(OpenStream { client, capture, event, device_name })
}
