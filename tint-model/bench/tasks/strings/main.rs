fn main() {
    let mut parts: Vec<String> = Vec::new();
    for i in 0..std::hint::black_box(100000) {
        parts.push(format!("item{i}"));
    }
    let joined = parts.join(",");
    let back: Vec<&str> = joined.split(',').collect();
    println!("{}", joined.len());
    println!("{}", back.len());
}
