// resources/types.rs

#[derive(Debug, Clone)]
pub enum Resource {
    Buffer(Vec<u8>),
    Image {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    TensorF32(Vec<f32>, Vec<usize>), // data + shape
    VecI32(Vec<i32>),
}
