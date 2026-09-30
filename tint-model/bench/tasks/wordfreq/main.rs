use std::collections::HashMap;
fn main() {
    let mut rng: u64 = 7; let mut words = Vec::new();
    for _ in 0..300000 { rng = (rng * 1664525 + 1013904223) % 4294967296; words.push(format!("w{}", rng % 5000)); }
    let text = words.join(" ");
    let mut counts: HashMap<&str, u64> = HashMap::new(); counts.insert("seed", 0);
    for w in text.split(' ') { *counts.entry(w).or_insert(0) += 1; }
    let (mut best, mut chars) = (0u64, 0u64);
    for (k, c) in &counts { if *c > best { best = *c; } chars += c * k.len() as u64; }
    println!("{} {} {}", counts.len(), best, chars);
}
