// Plain Rust. Anything marked #[tint::export] is callable from app.tn.
use std::collections::HashMap;

#[derive(tint::IntoTint, tint::FromTint)]
pub struct Stats {
    pub count: f64,
    pub mean: f64,
}

#[tint::export]
pub fn stats(values: Vec<f64>) -> Stats {
    let count = values.len() as f64;
    let mean = if count == 0.0 { 0.0 } else { values.iter().sum::<f64>() / count };
    Stats { count, mean }
}

#[tint::export]
pub fn word_count(text: String) -> HashMap<String, f64> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        *counts.entry(word.to_lowercase()).or_insert(0.0) += 1.0;
    }
    counts
}

#[tint::export]
pub fn to_num(text: String) -> Result<f64, String> {
    text.trim().parse::<f64>().map_err(|e| format!("not a number: {e}"))
}
