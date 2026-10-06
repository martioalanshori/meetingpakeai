//! Penulisan part WAV per channel (PRD §7.3): `<channel>_<NNNN>.wav`, maks 60 detik, flush tiap 1 detik.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use hound::{SampleFormat, WavSpec, WavWriter};

use super::{Channel, SAMPLE_RATE};

/// Maks sampel per part: 60 detik.
pub const PART_MAX_SAMPLES: u64 = 60 * SAMPLE_RATE as u64;
/// Interval flush header + data ke disk: 1 detik.
const FLUSH_EVERY_SAMPLES: u64 = SAMPLE_RATE as u64;

pub const WAV_SPEC: WavSpec = WavSpec {
    channels: 1,
    sample_rate: SAMPLE_RATE,
    bits_per_sample: 16,
    sample_format: SampleFormat::Int,
};

/// Kejadian part, untuk dicatat ke tabel `recording_parts` oleh pemanggil.
#[derive(Debug, Clone)]
pub enum PartEvent {
    /// Part baru dibuka (`finalized = 0`).
    Opened { channel: Channel, part_index: u32, path: PathBuf },
    /// Part ditutup dengan header final (`finalized = 1`).
    Finalized { channel: Channel, part_index: u32, samples: u64 },
}

pub type PartEventSink = Box<dyn FnMut(PartEvent) + Send>;

pub struct PartWriter {
    dir: PathBuf,
    channel: Channel,
    part_index: u32,
    writer: Option<WavWriter<BufWriter<File>>>,
    part_samples: u64,
    since_flush: u64,
    total_samples: u64,
    on_event: PartEventSink,
}

impl PartWriter {
    pub fn new(dir: &Path, channel: Channel, on_event: PartEventSink) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
            channel,
            part_index: 0,
            writer: None,
            part_samples: 0,
            since_flush: 0,
            total_samples: 0,
            on_event,
        })
    }

    pub fn part_file_name(channel: Channel, part_index: u32) -> String {
        format!("{}_{:04}.wav", channel.as_str(), part_index)
    }

    /// Total sampel yang sudah ditulis di semua part.
    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    fn open_next(&mut self) -> std::io::Result<()> {
        self.part_index += 1;
        let path = self.dir.join(Self::part_file_name(self.channel, self.part_index));
        let writer = WavWriter::create(&path, WAV_SPEC).map_err(hound_err)?;
        self.writer = Some(writer);
        self.part_samples = 0;
        self.since_flush = 0;
        (self.on_event)(PartEvent::Opened { channel: self.channel, part_index: self.part_index, path });
        Ok(())
    }

    fn finalize_current(&mut self) -> std::io::Result<()> {
        if let Some(w) = self.writer.take() {
            w.finalize().map_err(hound_err)?;
            (self.on_event)(PartEvent::Finalized {
                channel: self.channel,
                part_index: self.part_index,
                samples: self.part_samples,
            });
        }
        Ok(())
    }

    pub fn write(&mut self, samples: &[i16]) -> std::io::Result<()> {
        let mut rest = samples;
        while !rest.is_empty() {
            if self.writer.is_none() {
                self.open_next()?;
            }
            let room = (PART_MAX_SAMPLES - self.part_samples) as usize;
            let (now, later) = rest.split_at(room.min(rest.len()));
            let w = self.writer.as_mut().expect("writer terbuka");
            {
                let mut iw = w.get_i16_writer(now.len() as u32);
                for &s in now {
                    iw.write_sample(s);
                }
                iw.flush().map_err(hound_err)?;
            }
            let n = now.len() as u64;
            self.part_samples += n;
            self.total_samples += n;
            self.since_flush += n;
            if self.since_flush >= FLUSH_EVERY_SAMPLES {
                w.flush().map_err(hound_err)?;
                self.since_flush = 0;
            }
            if self.part_samples >= PART_MAX_SAMPLES {
                self.finalize_current()?;
            }
            rest = later;
        }
        Ok(())
    }

    /// Tutup part terakhir (dipanggil saat Stop).
    pub fn finish(mut self) -> std::io::Result<u64> {
        self.finalize_current()?;
        Ok(self.total_samples)
    }
}

fn hound_err(e: hound::Error) -> std::io::Error {
    match e {
        hound::Error::IoError(io) => io,
        other => std::io::Error::other(other.to_string()),
    }
}

/// Perbaiki header WAV part yang tidak di-finalize (crash). PRD §7.3:
/// ukuran RIFF dan `data` = sisa file setelah header, buang byte ganjil terakhir.
/// Mengembalikan jumlah sampel (16-bit mono) di file.
pub fn repair_wav_header(path: &Path) -> std::io::Result<u64> {
    let mut f = OpenOptions::new().read(true).write(true).open(path)?;
    let file_len = f.metadata()?.len();
    let data_offset = find_data_chunk_offset(&mut f)?.unwrap_or(44);
    if file_len < data_offset {
        return Ok(0);
    }
    let mut data_len = file_len - data_offset;
    if data_len % 2 == 1 {
        data_len -= 1;
        f.set_len(data_offset + data_len)?;
    }
    let data_len32 = u32::try_from(data_len).unwrap_or(u32::MAX);
    let riff_len = u32::try_from(data_offset + data_len - 8).unwrap_or(u32::MAX);
    f.seek(SeekFrom::Start(4))?;
    f.write_all(&riff_len.to_le_bytes())?;
    f.seek(SeekFrom::Start(data_offset - 4))?;
    f.write_all(&data_len32.to_le_bytes())?;
    f.flush()?;
    Ok(data_len / 2)
}

/// Offset awal isi chunk `data` (setelah id + ukuran). `None` jika tidak ketemu.
fn find_data_chunk_offset(f: &mut File) -> std::io::Result<Option<u64>> {
    let mut header = [0u8; 12];
    f.seek(SeekFrom::Start(0))?;
    if f.read_exact(&mut header).is_err() || &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Ok(None);
    }
    let mut pos = 12u64;
    let mut chunk = [0u8; 8];
    // Header part hanya berisi beberapa chunk kecil; batasi pencarian.
    while pos < 4096 {
        f.seek(SeekFrom::Start(pos))?;
        if f.read_exact(&mut chunk).is_err() {
            return Ok(None);
        }
        if &chunk[0..4] == b"data" {
            return Ok(Some(pos + 8));
        }
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as u64;
        pos += 8 + size + (size % 2);
    }
    Ok(None)
}
