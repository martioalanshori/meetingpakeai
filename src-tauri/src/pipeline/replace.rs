//! Ganti kata/frasa sebagai kata utuh tanpa membedakan huruf besar-kecil (langkah 52, feedback3 C3):
//! koreksi nama/istilah salah dengar di seluruh transkrip & notulen satu meeting.

fn lower(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Ganti setiap kemunculan `from` (kata utuh, case-insensitive) dengan `to`. Mengembalikan (teks, jumlah).
pub fn replace_words(text: &str, from: &str, to: &str) -> (String, usize) {
    let pat: Vec<char> = from.trim().chars().map(lower).collect();
    if pat.is_empty() {
        return (text.to_string(), 0);
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut count = 0;
    let mut i = 0;
    while i < chars.len() {
        let end = i + pat.len();
        let matches = end <= chars.len()
            && chars[i..end].iter().map(|&c| lower(c)).eq(pat.iter().copied())
            && (i == 0 || !chars[i - 1].is_alphanumeric())
            && (end == chars.len() || !chars[end].is_alphanumeric());
        if matches {
            out.push_str(to.trim());
            count += 1;
            i = end;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    (out, count)
}
