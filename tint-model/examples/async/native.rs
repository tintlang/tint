// An async Rust function: it must return Result<T, E: Display>.
// In the browser the call returns a Promise; natively it is run to completion.
#[tint::export]
pub async fn slow_square(n: f64) -> Result<f64, String> {
    if n < 0.0 {
        return Err("negative input".to_string());
    }
    Ok(n * n)
}
