//! Parsing & validasi output LLM (PRD §10.5).

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub tugas: String,
    pub penanggung_jawab: Option<String>,
    pub tenggat: Option<String>,
}

/// Hasil final (FINAL / MERGE).
#[derive(Debug, Clone, Serialize)]
pub struct FinalNotes {
    pub judul: String,
    pub ringkasan: String,
    pub keputusan: Vec<String>,
    pub action_items: Vec<ActionItem>,
    pub topik: Vec<String>,
}

/// Hasil per bagian (CHUNK / merge perantara).
#[derive(Debug, Clone, Serialize)]
pub struct PartialNotes {
    pub ringkasan_bagian: String,
    pub keputusan: Vec<String>,
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
        ringkasan,
        keputusan: string_list(v.get("keputusan"))?,
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
        keputusan: string_list(v.get("keputusan"))?,
        action_items: action_items(v.get("action_items"))?,
        topik: string_list(v.get("topik"))?,
    })
}
