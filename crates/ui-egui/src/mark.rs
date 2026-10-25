//! MK Perth's mark, drawn in code: a dark gear holding a violet globe, its meridians and
//! parallels in white, a schedule's bars cascading across it. Worked out per pixel from the
//! shapes' geometry (4×4 samples a pixel), so one drawing gives the window's icon and the mark
//! in the app at any size.

use egui::{TextureHandle, TextureOptions};

/// The gear's teeth, and its radii in the mark's half-width: the teeth's tips, the rim, the
/// inside of the rim; then the white ring's inside, the globe's edge.
const TEETH: f32 = 12.0;
const TIP: f32 = 0.94;
const RIM: f32 = 0.80;
const RIM_IN: f32 = 0.67;
const GLOBE: f32 = 0.615;
/// The white lines' half-width, in the globe's radius.
const LINE: f32 = 0.024;
/// The white edge around the gear, in the mark's half-width: it shows the gear on a dark window.
const EDGE: f32 = 0.03;

const GEAR_LIGHT: [u8; 3] = [0x2A, 0x27, 0x30];
const GEAR_DARK: [u8; 3] = [0x1A, 0x19, 0x1E];
const WHITE: [u8; 3] = [0xFF, 0xFF, 0xFF];
/// The globe's violet: darker at the top right, lighter at the bottom left.
const GLOBE_DARK: [u8; 3] = [0x6A, 0x3A, 0x98];
const GLOBE_LIGHT: [u8; 3] = [0xA8, 0x74, 0xD8];

/// The bars, in the globe's radius: left, right, top, bottom and colour.
const BARS: [(f32, f32, f32, f32, [u8; 3]); 6] = [
    (-0.74, -0.06, -0.54, -0.41, [0x4E, 0x1F, 0x72]),
    (-0.36, 0.40, -0.36, -0.24, [0x8A, 0x52, 0xBC]),
    (0.02, 0.78, -0.19, -0.07, [0xB0, 0x7C, 0xDB]),
    (-0.86, -0.22, 0.10, 0.22, [0xD6, 0xBE, 0xEE]),
    (-0.46, 0.26, 0.27, 0.39, [0xC4, 0xA2, 0xE2]),
    (0.06, 0.66, 0.58, 0.70, [0xC8, 0xA6, 0xE4]),
];

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// The colour at (`x`, `y`), both from −1 to 1 across the mark, or nothing outside it.
fn at(x: f32, y: f32) -> Option<[u8; 3]> {
    let r = (x * x + y * y).sqrt();
    if r >= RIM_IN {
        // The gear, then a white edge around it as wide everywhere: on the rim's arc, along the
        // teeth's sides (an angle off a tooth, measured at this radius) and over their tips.
        let gear = |r: f32, off: f32| {
            let half = 0.24 - 0.10 * ((r - RIM) / (TIP - RIM)).max(0.0);
            r < TIP && (r < RIM || off <= half)
        };
        let turn = y.atan2(x) / std::f32::consts::TAU * TEETH;
        let off = (turn - turn.round()).abs();
        if gear(r, off) {
            // Shaded in two halves, lit from the left.
            return Some(if x < 0.0 { GEAR_LIGHT } else { GEAR_DARK });
        }
        let reach = EDGE / r.max(0.1) * TEETH / std::f32::consts::TAU;
        let edge = gear(r - EDGE, off) || gear(r, (off - reach).max(0.0)) || gear(r - EDGE * 0.7, (off - reach * 0.7).max(0.0));
        return edge.then_some(WHITE);
    }
    if r >= GLOBE {
        return Some(WHITE);
    }
    // The globe in its own radius.
    let (u, v) = (x / GLOBE, y / GLOBE);
    // The meridians (the middle one and an ellipse each side) and three parallels, the lowest
    // bowed as on a globe seen from a little above.
    let a = 0.47;
    let ellipse = ((u * u) / (a * a) + v * v).sqrt();
    let lines =
        u.abs() < LINE || ((ellipse - 1.0) * a).abs() < LINE || v.abs() < LINE || (v + 0.62).abs() < LINE || (v - (0.60 - 0.10 * u * u)).abs() < LINE;
    if lines {
        return Some(WHITE);
    }
    if let Some(&(.., c)) = BARS.iter().find(|&&(l, rr, t, b, _)| u >= l && u <= rr && v >= t && v <= b) {
        return Some(c);
    }
    Some(mix(GLOBE_DARK, GLOBE_LIGHT, (v - u) / 2.0 * 0.9 + 0.5))
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

    /// Clear in the corners, white on the ring, dark on the rim, a tooth at the top and none
    /// halfway between two.
    #[test]
    fn the_shapes_where_they_belong() {
        assert_eq!(at(-0.99, -0.99), None);
        assert_eq!(at(0.0, -(RIM_IN + GLOBE) / 2.0), Some(WHITE));
        assert_eq!(at(-0.73, 0.0), Some(GEAR_LIGHT));
        assert_eq!(at(0.0, -0.9), Some(GEAR_DARK), "a tooth at the top");
        assert_eq!(at(0.0, -(TIP + EDGE / 2.0)), Some(WHITE), "its white edge");
        let between = std::f32::consts::TAU / TEETH / 2.0 - std::f32::consts::FRAC_PI_2;
        assert_eq!(at(0.9 * between.cos(), 0.9 * between.sin()), None);
        let pixels = rgba(64);
        assert_eq!(pixels.len(), 64 * 64 * 4);
        assert_eq!(pixels[3], 0, "the corner is clear");
        assert_eq!(pixels[(32 * 64 + 32) * 4 + 3], 255, "the middle is covered");
    }
}
