//! Membaca part WAV satu channel sebagai satu stream 16 kHz mono (PRD §8.1 langkah 1).
//! Jumlah sampel dihitung dari panjang file (aman untuk part hasil recovery).

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;

struct Part {
    path: PathBuf,
    data_offset: u64,
    samples: u64,
    /// Indeks sampel global pertama part ini.
    start: u64,
}

pub struct PartReader {
    parts: Vec<Part>,
    total: u64,
}

fn data_offset(f: &mut File) -> std::io::Result<u64> {
    let mut header = [0u8; 12];
    f.seek(SeekFrom::Start(0))?;
    f.read_exact(&mut header)?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Ok(44);
    }
    let mut pos = 12u64;
    let mut chunk = [0u8; 8];
    while pos < 4096 {
        f.seek(SeekFrom::Start(pos))?;
        f.read_exact(&mut chunk)?;
        if &chunk[0..4] == b"data" {
            return Ok(pos + 8);
        }
        let size = u32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]) as u64;
        pos += 8 + size + (size % 2);
    }
    Ok(44)
}

impl PartReader {
    /// `paths` harus urut sesuai `part_index`. File yang hilang dilewati.
    pub fn open(paths: &[PathBuf]) -> std::io::Result<Self> {
        let mut parts = Vec::with_capacity(paths.len());
        let mut total = 0u64;
        for p in paths {
            let mut f = match File::open(p) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!("part hilang dilewati ({e})");
                    continue;
                }
            };
            let len = f.metadata()?.len();
            let off = data_offset(&mut f).unwrap_or(44);
            let samples = len.saturating_sub(off) / 2;
            parts.push(Part { path: p.clone(), data_offset: off, samples, start: total });
            total += samples;
        }
        Ok(Self { parts, total })
    }

    pub fn total_samples(&self) -> u64 {
        self.total
    }

    /// Baca sampel [start, start+len) lintas part.
    pub fn read_range(&self, start: u64, len: u64) -> std::io::Result<Vec<i16>> {
        let end = (start + len).min(self.total);
        let mut out = Vec::with_capacity(end.saturating_sub(start) as usize);
        let mut pos = start;
        for part in &self.parts {
            if pos >= end {
                break;
            }
            let part_end = part.start + part.samples;
            if pos >= part_end {
                continue;
            }
            let from = pos - part.start;
            let n = (end.min(part_end) - pos) as usize;
            let mut f = File::open(&part.path)?;
            f.seek(SeekFrom::Start(part.data_offset + from * 2))?;
            let mut buf = vec![0u8; n * 2];
            f.read_exact(&mut buf)?;
            out.extend(buf.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)));
            pos += n as u64;
        }
        Ok(out)
    }

    /// Iterasi frame berurutan berukuran `frame` sampel (frame terakhir yang tidak penuh di-pad nol).
    pub fn for_each_frame(&self, frame: usize, mut f: impl FnMut(&[i16])) -> std::io::Result<()> {
        let mut buf: Vec<i16> = Vec::with_capacity(frame * 2);
        let mut bytes = vec![0u8; 64 * 1024];
        for part in &self.parts {
            let mut r = BufReader::new(File::open(&part.path)?);
            r.seek(SeekFrom::Start(part.data_offset))?;
            let mut remaining = part.samples * 2;
            while remaining > 0 {
                let want = (bytes.len() as u64).min(remaining) as usize;
                r.read_exact(&mut bytes[..want])?;
                remaining -= want as u64;
                for b in bytes[..want].as_chunks::<2>().0 {
                    buf.push(i16::from_le_bytes(*b));
                    if buf.len() == frame {
                        f(&buf);
                        buf.clear();
                    }
                }
            }
        }
        if !buf.is_empty() {
            buf.resize(frame, 0);
            f(&buf);
        }
        Ok(())
    }
}
