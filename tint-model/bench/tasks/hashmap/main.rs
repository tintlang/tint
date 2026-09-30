use std::collections::HashMap;

fn main() {
    let mut m: HashMap<String, u64> = HashMap::new();
    m.insert("a".to_string(), 0);
    for i in 0..std::hint::black_box(100000u64) {
        m.insert(format!("k{i}"), i);
    }
    let mut total = 0u64;
    for i in 0..100000u64 {
        total += m[&format!("k{i}")];
    }
    println!("{total}");
}
