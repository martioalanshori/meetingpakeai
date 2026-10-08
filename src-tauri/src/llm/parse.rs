//! Parsing & validasi output LLM (PRD §10.5).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub tugas: String,
    pub penanggung_jawab: Option<String>,
    pub tenggat: Option<String>,
    /// Waktu di transkrip `HH:MM:SS` tempat tugas dibahas (langkah 42).
    #[serde(default)]
    pub sumber: Option<String>,
}

/// Keputusan + waktu sumbernya. Format lama (string saja) tetap diterima.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub teks: String,
    #[serde(default)]
    pub sumber: Option<String>,
}

/// Hasil final (FINAL / MERGE).
#[derive(Debug, Clone, Serialize)]
pub struct FinalNotes {
    pub judul: String,
    /// Intisari 3 poin (langkah 50).
    pub intisari: Vec<String>,
    pub ringkasan: String,
    pub keputusan: Vec<Decision>,
    /// Belum diputuskan / pertanyaan terbuka (langkah 50).
    pub pertanyaan_terbuka: Vec<Decision>,
    pub action_items: Vec<ActionItem>,
    pub topik: Vec<String>,
}

/// Hasil per bagian (CHUNK / merge perantara).
#[derive(Debug, Clone, Serialize)]
pub struct PartialNotes {
    pub ringkasan_bagian: String,
    pub keputusan: Vec<Decision>,
    pub pertanyaan_terbuka: Vec<Decision>,
    pub action_items: Vec<ActionItem>,
    pub topik: Vec<String>,
}

/// Langkah 1–2: buang `<think>…</think>` (termasuk yang tidak tertutup), ambil `{` pertama s/d `}` terakhir.
pub fn extract_json(raw: &str) -> Result<Value, String> {
    let mut s = raw.to_string();
    while let Some(start) = s.find("<think>") {
        let end = match s[start..].find("</think>") {
            Some(rel) => start + rel + "</think>".len(),
            // Tidak tertutup: hapus sampai `{` pertama setelahnya.
            None => s[start..].find('{').map_or(s.len(), |rel| start + rel),
        };
        s.replace_range(start..end, "");
    }
    let open = s.find('{').ok_or("tidak ada objek JSON")?;
    let close = s.rfind('}').ok_or("objek JSON tidak lengkap")?;
    if close < open {
        return Err("objek JSON tidak lengkap".into());
    }
    serde_json::from_str(&s[open..=close]).map_err(|e| format!("JSON tidak valid: {e}"))
}

fn opt_string(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    }
}

fn string_list(v: Option<&Value>) -> Result<Vec<String>, String> {
    match v {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items
            .iter()
            .filter_map(|i| match i {
                Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                _ => None,
            })
            .collect()),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(vec![s.trim().to_string()]),
        Some(_) => Err("field daftar bukan array string".into()),
    }
}

/// `keputusan`: array string (format lama) atau objek `{teks, sumber}`.
fn decisions(v: Option<&Value>) -> Result<Vec<Decision>, String> {
    match v {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items
            .iter()
            .filter_map(|i| match i {
                Value::String(s) if !s.trim().is_empty() => Some(Decision { teks: s.trim().to_string(), sumber: None }),
                Value::Object(_) => {
                    let teks = opt_string(i.get("teks")).or_else(|| opt_string(i.get("text")))?;
                    Some(Decision { teks, sumber: opt_string(i.get("sumber")) })
                }
                _ => None,
            })
            .collect()),
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(vec![Decision { teks: s.trim().to_string(), sumber: None }]),
        Some(_) => Err("keputusan bukan array".into()),
    }
}

/// `HH:MM:SS` / `MM:SS` (boleh diawali `[`) → ms.
pub fn timestamp_ms(s: &str) -> Option<i64> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    let parts: Vec<i64> = s.split(':').map(|p| p.trim().parse::<i64>().ok()).collect::<Option<Vec<_>>>()?;
    let secs = match parts.as_slice() {
        [h, m, sec] => h * 3600 + m * 60 + sec,
        [m, sec] => m * 60 + sec,
        _ => return None,
    };
    (secs >= 0).then_some(secs * 1000)
}

fn action_items(v: Option<&Value>) -> Result<Vec<ActionItem>, String> {
    match v {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items
            .iter()
            .filter_map(|i| {
                let tugas = opt_string(i.get("tugas"))?;
                Some(ActionItem {
                    tugas,
                    penanggung_jawab: opt_string(i.get("penanggung_jawab")),
                    tenggat: opt_string(i.get("tenggat")),
                    sumber: opt_string(i.get("sumber")),
                })
            })
            .collect()),
        Some(_) => Err("action_items bukan array".into()),
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

pub fn parse_final(raw: &str) -> Result<FinalNotes, String> {
    let v = extract_json(raw)?;
    let judul = opt_string(v.get("judul")).ok_or("judul kosong")?;
    let ringkasan = opt_string(v.get("ringkasan")).ok_or("ringkasan kosong")?;
    Ok(FinalNotes {
        judul: truncate_chars(&judul, 100),
        intisari: string_list(v.get("intisari")).unwrap_or_default().into_iter().take(5).collect(),
        ringkasan,
        keputusan: decisions(v.get("keputusan"))?,
        pertanyaan_terbuka: decisions(v.get("pertanyaan_terbuka")).unwrap_or_default(),
        action_items: action_items(v.get("action_items"))?,
        topik: string_list(v.get("topik"))?.into_iter().take(8).collect(),
    })
}

pub fn parse_partial(raw: &str) -> Result<PartialNotes, String> {
    let v = extract_json(raw)?;
    let ringkasan_bagian = opt_string(v.get("ringkasan_bagian"))
        .or_else(|| opt_string(v.get("ringkasan")))
        .unwrap_or_default();
    Ok(PartialNotes {
        ringkasan_bagian,
        keputusan: decisions(v.get("keputusan"))?,
        pertanyaan_terbuka: decisions(v.get("pertanyaan_terbuka")).unwrap_or_default(),
        action_items: action_items(v.get("action_items"))?,
        topik: string_list(v.get("topik"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_final_lengkap() {
        let raw = r#"{"judul":"Rapat","ringkasan":"Isi.","keputusan":["A"],
            "action_items":[{"tugas":"Kirim","penanggung_jawab":"Budi","tenggat":null}],"topik":["x","y"]}"#;
        let n = parse_final(raw).unwrap();
        assert_eq!(n.judul, "Rapat");
        assert_eq!(n.keputusan[0].teks, "A");
        assert_eq!(n.action_items.len(), 1);
        assert_eq!(n.action_items[0].penanggung_jawab.as_deref(), Some("Budi"));
        assert_eq!(n.action_items[0].tenggat, None);
    }

    #[test]
    fn parse_final_membuang_think_dan_teks_sekitar() {
        let raw = "<think>menimbang…</think>Berikut hasilnya:\n```json\n{\"judul\":\"J\",\"ringkasan\":\"R\"}\n```";
        let n = parse_final(raw).unwrap();
        assert_eq!(n.judul, "J");
        assert!(n.keputusan.is_empty() && n.action_items.is_empty() && n.topik.is_empty());
    }

    #[test]
    fn parse_final_think_tidak_tertutup() {
        let n = parse_final("<think>bocor {\"judul\":\"J\",\"ringkasan\":\"R\"}").unwrap();
        assert_eq!(n.ringkasan, "R");
    }

    #[test]
    fn parse_final_judul_kosong_ditolak() {
        assert!(parse_final(r#"{"judul":"  ","ringkasan":"R"}"#).is_err());
        assert!(parse_final("bukan json").is_err());
    }

    #[test]
    fn parse_final_batas_judul_dan_topik() {
        let judul = "a".repeat(150);
        let topik: Vec<String> = (0..12).map(|i| format!("t{i}")).collect();
        let raw = serde_json::json!({ "judul": judul, "ringkasan": "R", "topik": topik }).to_string();
        let n = parse_final(&raw).unwrap();
        assert_eq!(n.judul.chars().count(), 100);
        assert_eq!(n.topik.len(), 8);
    }

    #[test]
    fn parse_final_item_tanpa_tugas_dibuang() {
        let raw = r#"{"judul":"J","ringkasan":"R","action_items":[{"tugas":""},{"tugas":"Ok"}]}"#;
        assert_eq!(parse_final(raw).unwrap().action_items.len(), 1);
    }
}
