//! Verifikasi PRD §21 #1, #3, #4, #5 terhadap API Groq memakai API key tersimpan (key tidak dicetak).
//!
//!   cargo run --example groq_probe -- [file.wav]

use meeting_pake_ai_lib::config::providers::ProvidersConfig;
use meeting_pake_ai_lib::{ai, ai_http, secrets};

const GROQ_URL: &str = "https://api.groq.com/openai/v1";

fn main() {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(run());
}

async fn run() {
    let key = secrets::get_key(ai::GROQ).ok().flatten().expect("API key belum tersimpan");
    let cfg = ProvidersConfig::default();
    let http = ai_http::build_client();

    // #1 model
    match ai_http::list_models(&http, GROQ_URL, Some(&key)).await {
        Ok(models) => {
            println!("== #1 model ==");
            println!("stt_model {} ada: {}", cfg.stt_model, models.contains(&cfg.stt_model));
            println!("llm_model {} ada: {}", cfg.llm_model, models.contains(&cfg.llm_model));
            let mut m = models.clone();
            m.sort();
            println!("semua model: {}", m.join(", "));
        }
        Err(e) => println!("list_models gagal: {e:?}"),
    }

    // #3 STT verbose_json
    if let Some(wav) = std::env::args().nth(1) {
        println!("== #3 STT ==");
        let bytes = std::fs::read(&wav).unwrap();
        let part = reqwest::multipart::Part::bytes(bytes).file_name("a.wav").mime_str("audio/wav").unwrap();
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", cfg.stt_model.clone())
            .text("language", "id")
            .text("response_format", "verbose_json")
            .text("timestamp_granularities[]", "segment")
            .text("temperature", "0");
        let resp = http
            .post(format!("{}/audio/transcriptions", GROQ_URL))
            .bearer_auth(&key)
            .multipart(form)
            .send()
            .await
            .unwrap();
        print_headers(&resp);
        let status = resp.status();
        let body: serde_json::Value = resp.json().await.unwrap_or_default();
        println!("HTTP {status}");
        println!("top-level keys: {:?}", body.as_object().map(|o| o.keys().collect::<Vec<_>>()));
        if let Some(seg) = body["segments"].get(0) {
            println!("segment[0] keys: {:?}", seg.as_object().map(|o| o.keys().collect::<Vec<_>>()));
            println!("segment[0]: {seg}");
        } else {
            println!("body: {body}");
        }
    }

    // #4 LLM: reasoning off + json_object
    println!("== #4 LLM ==");
    for (label, extra, json_mode) in [
        ("extra_body + json_object", true, true),
        ("tanpa extra_body, json_object", false, true),
        ("polos", false, false),
    ] {
        let mut body = serde_json::json!({
            "model": cfg.llm_model,
            "messages": [
                {"role": "system", "content": "Kembalikan HANYA objek JSON."},
                {"role": "user", "content": "Buat JSON {\"judul\": \"...\"} untuk meeting tentang anggaran Q4."}
            ],
            "temperature": 0.2,
            "max_tokens": 200
        });
        if extra {
            for (k, v) in &cfg.llm_extra_body {
                body[k] = v.clone();
            }
        }
        if json_mode {
            body["response_format"] = serde_json::json!({"type": "json_object"});
        }
        let resp = http
            .post(format!("{}/chat/completions", GROQ_URL))
            .bearer_auth(&key)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = resp.status();
        if label == "polos" {
            print_headers(&resp);
        }
        let v: serde_json::Value = resp.json().await.unwrap_or_default();
        println!("[{label}] HTTP {status}");
        if status.is_success() {
            println!("  content: {}", v["choices"][0]["message"]["content"]);
            println!("  usage: {}", v["usage"]);
        } else {
            println!("  error: {}", v["error"]);
        }
    }
}

fn print_headers(resp: &reqwest::Response) {
    for (n, v) in resp.headers() {
        let n = n.as_str();
        if n.starts_with("x-ratelimit") || n == "retry-after" {
            println!("  header {n}: {}", v.to_str().unwrap_or("?"));
        }
    }
}
