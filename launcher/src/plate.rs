use gpui::{rgba, Rgba};

/// `c` at opacity `a`.
pub fn alpha(c: u32, a: f32) -> Rgba {
    rgba(c << 8 | (a * 255.).round() as u32)
}
