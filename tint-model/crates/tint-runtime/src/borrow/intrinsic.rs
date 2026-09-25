// borrow/intrinsic.rs
use crate::resources::Resource;

pub fn fft(_buf: &mut Resource) {
    // TODO
}

pub fn blur(_img: &mut Resource, _radius: f32) {
    // TODO
}

pub fn sobel(_img: &mut Resource) {
    // TODO
}

pub fn boolean_union(_a: &mut Resource, _b: &mut Resource) {
    // TODO
}

// registry
pub fn call_intrinsic(name: &str, args: &mut [Resource]) {
    match name {
        "fft" => fft(&mut args[0]),
        "blur" => blur(&mut args[0], 4.0),
        "sobel" => sobel(&mut args[0]),
        "boolean_union" => {
            // TODO: two-resource op
            let (a, b) = args.split_at_mut(1);
            boolean_union(&mut a[0], &mut b[0]);
        }

        _ => {}
    }
}
