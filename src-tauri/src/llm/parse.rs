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
    /// Jenis meeting yang dikenali LLM (template otomatis); hanya kunci template yang dikenal.
    pub jenis: Option<String>,
    /// Nama satu-satunya peserta lain jika disebut di meeting (1:1); dipakai sebagai label channel system.
    pub nama_peserta_lain: Option<String>,
}

/// Hasil per bagian (CHUNK / merge perantara).
#[derive(Debug, Clone, Serialize)]
pub struct PartialNotes {
    pub ringkasan_bagian: String,
    pub keputusan: Vec<String>,
    pub action_items: Vec<ActionItem>,
    pub topik: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nama_peserta_lain: Option<String>,
}

/// Nama peserta: dibuang jika kosong, terlalu panjang (> 40 karakter), atau berisi lebih dari 4 kata.
fn person_name(v: Option<&Value>) -> Option<String> {
    opt_string(v).filter(|n| n.chars().count() <= 40 && n.split_whitespace().count() <= 4)
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
        jenis: opt_string(v.get("jenis"))
            .map(|j| j.to_lowercase())
            .filter(|j| crate::llm::prompts::is_template(j)),
        nama_peserta_lain: person_name(v.get("nama_peserta_lain")),
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
        nama_peserta_lain: person_name(v.get("nama_peserta_lain")),
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
        assert_eq!(n.keputusan, vec!["A"]);
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
    fn parse_final_jenis_hanya_kunci_dikenal() {
        assert_eq!(parse_final(r#"{"judul":"J","ringkasan":"R","jenis":"Standup"}"#).unwrap().jenis.as_deref(), Some("standup"));
        assert_eq!(parse_final(r#"{"judul":"J","ringkasan":"R","jenis":"rapat"}"#).unwrap().jenis, None);
    }

    #[test]
    fn parse_final_nama_peserta_lain() {
        let n = parse_final(r#"{"judul":"J","ringkasan":"R","nama_peserta_lain":" Ucup "}"#).unwrap();
        assert_eq!(n.nama_peserta_lain.as_deref(), Some("Ucup"));
        let n = parse_final(r#"{"judul":"J","ringkasan":"R","nama_peserta_lain":null}"#).unwrap();
        assert_eq!(n.nama_peserta_lain, None);
        let long = r#"{"judul":"J","ringkasan":"R","nama_peserta_lain":"tim marketing dan tim sales klien"}"#;
        assert_eq!(parse_final(long).unwrap().nama_peserta_lain, None);
    }

    #[test]
    fn parse_final_item_tanpa_tugas_dibuang() {
        let raw = r#"{"judul":"J","ringkasan":"R","action_items":[{"tugas":""},{"tugas":"Ok"}]}"#;
        assert_eq!(parse_final(raw).unwrap().action_items.len(), 1);
    }
}
