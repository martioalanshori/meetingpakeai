//! Ukur akurasi transkrip (langkah 45, feedback3 G8): bandingkan transkrip dengan teks rujukan yang diketik manual.
//!
//!   cargo run --example wer -- <rujukan.txt> <hipotesis.txt | id-meeting> [folder_data]
//!
//! - `hipotesis.txt`: teks transkrip apa adanya (mis. hasil Salin tab Transkrip; `[HH:MM:SS]` dibuang).
//! - `id-meeting`: transkrip dibaca dari database aplikasi (`%APPDATA%\com.meetingpakeai.desktop` atau
//!   `folder_data`), urut waktu, tanpa segment yang difilter/duplikat.
//!
//! Hasil: WER (kata salah/hilang/sisipan dibagi jumlah kata rujukan), rincian, dan persentase kata rujukan
//! yang hilang sama sekali (deletion) — indikator "kalimat hilang".

use std::path::PathBuf;

use meeting_pake_ai_lib::db::{repo_segments, Db};
use meeting_pake_ai_lib::pipeline::filter::normalize;

fn words(text: &str) -> Vec<String> {
    // Buang stempel waktu "[00:01:02]" lalu normalisasi sama dengan filter (huruf kecil, tanpa tanda baca).
    let no_ts: String = text
        .lines()
        .map(|l| {
            let l = l.trim();
            if l.starts_with('[') {
                l.find(']').map_or(l, |i| &l[i + 1..])
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    normalize(&no_ts).split_whitespace().map(str::to_string).collect()
}

/// Levenshtein per kata → (substitusi, hapus, sisip).
fn align(r: &[String], h: &[String]) -> (usize, usize, usize) {
    let (n, m) = (r.len(), h.len());
    // d[i][j] = (biaya, sub, del, ins)
    let mut prev: Vec<(usize, usize, usize, usize)> = (0..=m).map(|j| (j, 0, 0, j)).collect();
    for i in 1..=n {
        let mut cur = vec![(i, 0, i, 0); m + 1];
        for j in 1..=m {
            let same = r[i - 1] == h[j - 1];
            let sub = prev[j - 1];
            let sub = (sub.0 + usize::from(!same), sub.1 + usize::from(!same), sub.2, sub.3);
            let del = prev[j];
            let del = (del.0 + 1, del.1, del.2 + 1, del.3);
            let ins = cur[j - 1];
            let ins = (ins.0 + 1, ins.1, ins.2, ins.3 + 1);
            cur[j] = [sub, del, ins].into_iter().min_by_key(|x| x.0).unwrap_or(sub);
        }
        prev = cur;
    }
    let (_, s, d, i) = prev[m];
    (s, d, i)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("pakai: cargo run --example wer -- <rujukan.txt> <hipotesis.txt | id-meeting> [folder_data]");
        std::process::exit(2);
    }
    let reference = std::fs::read_to_string(&args[0]).expect("rujukan tidak bisa dibaca");
    let hyp_path = PathBuf::from(&args[1]);
    let hypothesis = if hyp_path.is_file() {
        std::fs::read_to_string(&hyp_path).expect("hipotesis tidak bisa dibaca")
    } else {
        let dir = args.get(2).map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(std::env::var("APPDATA").expect("APPDATA")).join("com.meetingpakeai.desktop")
        });
        let db = Db::open(&dir.join("app.sqlite")).expect("database tidak bisa dibuka");
        let segs = repo_segments::list_visible(&db.conn(), &args[1]).expect("transkrip tidak ditemukan");
        segs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join(" ")
    };

    let r = words(&reference);
    let h = words(&hypothesis);
    if r.is_empty() {
        eprintln!("rujukan kosong");
        std::process::exit(2);
    }
    let (s, d, i) = align(&r, &h);
    let n = r.len() as f64;
    println!("Kata rujukan     : {}", r.len());
    println!("Kata transkrip   : {}", h.len());
    println!("Salah (subst.)   : {s}");
    println!("Hilang (del.)    : {d}  ({:.1}% kata rujukan)", d as f64 / n * 100.0);
    println!("Sisipan (ins.)   : {i}");
    println!("WER              : {:.1}%", (s + d + i) as f64 / n * 100.0);
}
