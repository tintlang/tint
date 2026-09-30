use std::hint::black_box;

fn main() {
    let size: i64 = black_box(200);
    let mut inside = 0;
    for y in 0..size {
        for x in 0..size {
            let cr = 2.0 * x as f64 / size as f64 - 1.5;
            let ci = 2.0 * y as f64 / size as f64 - 1.0;
            let (mut zr, mut zi) = (0.0f64, 0.0f64);
            let mut it = 0;
            while it < 50 && zr * zr + zi * zi <= 4.0 {
                let t = zr * zr - zi * zi + cr;
                zi = 2.0 * zr * zi + ci;
                zr = t;
                it += 1;
            }
            if it == 50 {
                inside += 1;
            }
        }
    }
    println!("{}", inside);
}
