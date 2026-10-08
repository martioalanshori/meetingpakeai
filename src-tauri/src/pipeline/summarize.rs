//! Ringkasan map-reduce (PRD §8.6, §10).

use async_trait::async_trait;

use crate::error::{AppError, ErrorCode};
use crate::llm::parse::{self, FinalNotes, PartialNotes};
use crate::llm::{prompts, ChatMessage};
use crate::queue::state::{StepError, StepResult};

const MAX_TOKENS_CHUNK: u32 = 1200;
const MAX_TOKENS_FINAL: u32 = 1500;
const MIN_WORDS: usize = 20;

/// Pemanggil LLM yang sudah menangani rate limiter, retry, dan pencatatan pemakaian.
#[async_trait]
pub trait LlmCaller: Send + Sync {
    async fn call(&self, messages: Vec<ChatMessage>, max_tokens: u32) -> StepResult<String>;
    /// Progres step `summarizing`: request selesai / total request.
    fn progress(&self, done: usize, total: usize);
}

pub struct SummarizeInput {
    /// Satu baris per segment tampil: `[HH:MM:SS] <Label>: <teks>`.
    pub lines: Vec<String>,
    pub word_count: usize,
    pub label_saya: String,
    pub label_peserta: String,
    /// Tanggal meeting `YYYY-MM-DD`.
    pub tanggal: String,
}

pub enum SummaryOutcome {
    /// Total kata < 20 → tanpa LLM, `summaries.status = 'empty'`.
    Empty,
    Notes(FinalNotes),
}

/// Estimasi token = ceil(jumlah karakter / 3).
pub fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(3)
}

fn invalid_output() -> StepError {
    StepError::Failed(AppError::new(ErrorCode::LlmInvalidOutput))
}

/// Panggil lalu parse; gagal parse → satu kali retry dengan pesan perbaikan (PRD §10.5).
async fn call_parsed<T>(
    caller: &dyn LlmCaller,
    mut messages: Vec<ChatMessage>,
    max_tokens: u32,
    parse: fn(&str) -> Result<T, String>,
) -> StepResult<T> {
    let raw = caller.call(messages.clone(), max_tokens).await?;
    match parse(&raw) {
        Ok(v) => Ok(v),
        Err(err) => {
            tracing::warn!("output LLM tidak valid, retry: {err}");
            messages.push(ChatMessage::assistant(raw));
            messages.push(ChatMessage::user(prompts::retry(&err)));
            let raw2 = caller.call(messages, max_tokens).await?;
            parse(&raw2).map_err(|e| {
                tracing::warn!("output LLM tetap tidak valid: {e}");
                invalid_output()
            })
        }
    }
}

/// Potong baris jadi kelompok ≤ `max_tokens` (baris tidak pernah dipotong).
fn split_lines(lines: &[String], max_tokens: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in lines {
        let candidate_len = estimate_tokens(&cur) + estimate_tokens(line) + 1;
        if !cur.is_empty() && candidate_len > max_tokens {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push('\n');
        }
        cur.push_str(line);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Kelompokkan JSON parsial agar tiap kelompok ≤ `max_tokens`.
fn group_partials(partials: &[String], max_tokens: usize) -> Vec<Vec<String>> {
    let mut groups: Vec<Vec<String>> = Vec::new();
    let mut size = 0;
    for p in partials {
        let t = estimate_tokens(p) + 1;
        match groups.last_mut() {
            Some(g) if size + t <= max_tokens => {
                g.push(p.clone());
                size += t;
            }
            _ => {
                groups.push(vec![p.clone()]);
                size = t;
            }
        }
    }
    groups
}

fn json_array(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

pub async fn summarize(caller: &dyn LlmCaller, input: &SummarizeInput, max_chunk_tokens: usize) -> StepResult<SummaryOutcome> {
    if input.word_count < MIN_WORDS {
        return Ok(SummaryOutcome::Empty);
    }
    let system = ChatMessage::system(prompts::system(&input.label_saya, &input.label_peserta));
    let transcript = input.lines.join("\n");

    // Single pass.
    if estimate_tokens(&transcript) <= max_chunk_tokens {
        caller.progress(0, 1);
        let user = ChatMessage::user(prompts::final_prompt(
            &input.tanggal,
            &input.label_saya,
            &input.label_peserta,
            &transcript,
        ));
        let notes = call_parsed(caller, vec![system, user], MAX_TOKENS_FINAL, parse::parse_final).await?;
        caller.progress(1, 1);
        tracing::info!("ringkasan single pass: 1 request");
        return Ok(SummaryOutcome::Notes(notes));
    }

    // Map: CHUNK per bagian.
    let chunks = split_lines(&input.lines, max_chunk_tokens);
    let n = chunks.len();
    let mut total = n + 1;
    let mut done = 0;
    caller.progress(done, total);
    let mut partials: Vec<String> = Vec::with_capacity(n);
    for (i, text) in chunks.iter().enumerate() {
        let user =
            ChatMessage::user(prompts::chunk(&input.tanggal, i + 1, n, text, &input.label_saya, &input.label_peserta));
        let p: PartialNotes = call_parsed(caller, vec![system.clone(), user], MAX_TOKENS_CHUNK, parse::parse_partial).await?;
        partials.push(serde_json::to_string(&p).map_err(|e| StepError::Failed(AppError::from(e)))?);
        done += 1;
        caller.progress(done, total);
    }

    // Reduce: merge bertingkat sampai muat satu request, lalu merge final.
    loop {
        let joined = json_array(&partials);
        let groups = group_partials(&partials, max_chunk_tokens);
        if estimate_tokens(&joined) <= max_chunk_tokens || groups.len() == partials.len() {
            let user = ChatMessage::user(prompts::merge(&input.tanggal, &joined, false));
            let notes = call_parsed(caller, vec![system.clone(), user], MAX_TOKENS_FINAL, parse::parse_final).await?;
            done += 1;
            caller.progress(done, total.max(done));
            tracing::info!("ringkasan map-reduce: {done} request");
            return Ok(SummaryOutcome::Notes(notes));
        }
        total += groups.len();
        let mut next = Vec::with_capacity(groups.len());
        for g in groups {
            let user = ChatMessage::user(prompts::merge(&input.tanggal, &json_array(&g), true));
            let p: PartialNotes = call_parsed(caller, vec![system.clone(), user], MAX_TOKENS_FINAL, parse::parse_partial).await?;
            next.push(serde_json::to_string(&p).map_err(|e| StepError::Failed(AppError::from(e)))?);
            done += 1;
            caller.progress(done, total);
        }
        partials = next;
    }
}
