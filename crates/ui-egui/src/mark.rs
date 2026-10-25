//! MK Perth's mark, drawn in code: a hexagon seen as a solid, its band open at the lower left so
//! that it reads as a P, around a cube; each in three shades, lit from the upper left. Worked
//! out per pixel from the shapes' geometry (4×4 samples a pixel), so one drawing gives the
//! window's icon and the mark in the app at any size.

use egui::{TextureHandle, TextureOptions};

/// The hexagon's outer and inner radii and the cube's, in the mark's half-width; the hexagon
/// stands on a corner, as an isometric solid does.
const OUTER: f32 = 0.96;
const INNER: f32 = 0.56;
const CUBE: f32 = 0.31;
/// The white cut between the P's stem and its top, in the mark's half-width.
const CUT: f32 = 0.035;
/// How far down the P's stem goes, at its outer edge: past the hexagon's lower-left corner.
const STEM: f32 = 0.74;

/// The three shades: faces turned up, to the left and to the right.
const TOP: [u8; 3] = [0x3B, 0x5B, 0xDB];
const LEFT: [u8; 3] = [0x6E, 0x9B, 0xF2];
const RIGHT: [u8; 3] = [0x36, 0x2A, 0x8A];

/// Inside a hexagon of radius `r` standing on a corner.
fn in_hexagon(x: f32, y: f32, r: f32) -> bool {
    let c = r * 3f32.sqrt() / 2.0;
    x.abs() <= c && y.abs() <= r - x.abs() / 3f32.sqrt()
}

/// The face a point is on, by its direction from the centre: up between the two upper corners,
/// left between the upper-left and lower-left ones, right from the upper-right to the bottom;
/// between the bottom and the lower left, none.
fn face(x: f32, y: f32) -> Option<[u8; 3]> {
    let angle = y.atan2(x).to_degrees();
    match angle {
        a if (-150.0..-30.0).contains(&a) => Some(TOP),
        a if (-30.0..90.0).contains(&a) => Some(RIGHT),
        a if (90.0..150.0).contains(&a) => None,
        _ => Some(LEFT),
    }
}

/// The colour at (`x`, `y`), both from −1 to 1 across the mark, or nothing outside it.
fn at(x: f32, y: f32) -> Option<[u8; 3]> {
    // The cube in the middle, its three faces meeting at its centre.
    if in_hexagon(x, y, CUBE) {
        let angle = y.atan2(x).to_degrees();
        return Some(if (-150.0..-30.0).contains(&angle) {
            TOP
        } else if (-30.0..90.0).contains(&angle) {
            RIGHT
        } else {
            LEFT
        });
    }
    // A white cut along the line to the upper-left corner parts the stem from the top.
    let (dx, dy) = ((-150f32).to_radians().cos(), (-150f32).to_radians().sin());
    if (x * dy - y * dx).abs() < CUT && x * dx + y * dy > 0.0 {
        return None;
    }
    // The stem: the band's left side, down past the corner, its foot slanting as the hexagon's
    // lower edges do.
    let (outer, inner) = (OUTER * 3f32.sqrt() / 2.0, INNER * 3f32.sqrt() / 2.0);
    let slope = 1.0 / 3f32.sqrt();
    // Its top is the cut: the line from the centre to the upper-left corner.
    if (-outer..=-inner).contains(&x) && y >= x * slope && y <= STEM + (x + outer) * slope {
        return Some(LEFT);
    }
    if !in_hexagon(x, y, OUTER) || in_hexagon(x, y, INNER) {
        return None;
    }
    face(x, y)
}

/// The mark as RGBA pixels, `size` on each side, transparent around it.
pub fn rgba(size: u32) -> Vec<u8> {
    const N: u32 = 4;
    let size = size.clamp(1, 1024);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    let scale = 2.0 / size as f32;
    for py in 0..size {
        for px in 0..size {
            let (mut sum, mut hits) = ([0u32; 3], 0u32);
            for sy in 0..N {
                for sx in 0..N {
                    let x = (px as f32 + (sx as f32 + 0.5) / N as f32) * scale - 1.0;
                    let y = (py as f32 + (sy as f32 + 0.5) / N as f32) * scale - 1.0;
                    if let Some(c) = at(x, y) {
                        for (s, v) in sum.iter_mut().zip(c) {
                            *s += u32::from(v);
                        }
                        hits += 1;
                    }
                }
            }
            // Colour averaged over the samples it covers; coverage as alpha.
            let avg = |s: u32| s.checked_div(hits).unwrap_or(0) as u8;
            out.extend([avg(sum[0]), avg(sum[1]), avg(sum[2]), (hits * 255 / (N * N)) as u8]);
        }
    }
    out
}

/// The mark as a texture, made once per context.
pub(crate) fn texture(ctx: &egui::Context) -> TextureHandle {
    let id = egui::Id::unique("mkperth-mark");
    if let Some(texture) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return texture;
    }
    const SIZE: usize = 256;
    let pixels = rgba(SIZE as u32);
    let image = egui::ColorImage::from_rgba_unmultiplied([SIZE, SIZE], &pixels);
    let texture = ctx.load_texture("mkperth-mark", image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

/// The mark as the window's icon.
pub fn icon() -> egui::IconData {
    const SIZE: u32 = 256;
    egui::IconData { rgba: rgba(SIZE), width: SIZE, height: SIZE }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Clear in the corners and in the hole, the band's three shades where they belong, the
    /// opening at the lower left, the cube in the middle.
    #[test]
    fn the_shapes_where_they_belong() {
        assert_eq!(at(-0.99, -0.99), None);
        assert_eq!(at(0.0, -0.8), Some(TOP), "the band's top");
        assert_eq!(at(-0.75, 0.0), Some(LEFT), "the stem");
        assert_eq!(at(0.75, 0.0), Some(RIGHT), "the bowl's right");
        assert_eq!(at(-0.35, 0.7), None, "the opening");
        assert_eq!(at(-0.75, 0.6), Some(LEFT), "the stem past the corner");
        assert_eq!(at(0.0, 0.45), None, "the hole");
        assert_eq!((at(0.0, -0.2), at(-0.15, 0.1), at(0.15, 0.1)), (Some(TOP), Some(LEFT), Some(RIGHT)), "the cube");
        let pixels = rgba(64);
        assert_eq!(pixels.len(), 64 * 64 * 4);
        assert_eq!(pixels[3], 0, "the corner is clear");
        assert_eq!(pixels[(32 * 64 + 32) * 4 + 3], 255, "the middle is covered");
    }
}
