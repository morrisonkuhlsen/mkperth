//! The splash screen: a small window of its own, without decorations, before the main one, as
//! MK Sigma and MK aeGIS have. In two halves: on Morrison Kühlsen's ink, MK Perth's mark, name,
//! motto and version, a bar that fills while it shows with the program's parts named in turn,
//! and the house's wordmark; beside them, artwork drawn in code in MK Perth's lilac: a schedule's
//! bars cascading down to a milestone. It closes by itself after a moment; a click or Esc closes
//! it sooner.

use egui::epaint::{Mesh, PathShape, Vertex};
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, TextureOptions, pos2, vec2};
use mkperth_engine::i18n::tr;

use crate::icons::{self, Icon};
use crate::theme::Tokens;

/// The window's size, in points, as MK aeGIS's.
pub const SIZE: [f32; 2] = [816.0, 480.0];
/// The height the layout's measures are given for; they scale with the window's.
const LAYOUT_HEIGHT: f32 = 400.0;
/// Seconds it stays.
const HOLD: f64 = 5.0;
/// The Morrison Kühlsen wordmark, white on clear (a brand asset: `docs/brand/`).
const MORRISON_KUHLSEN: &[u8] = include_bytes!("../../../docs/brand/morrison-kuhlsen.png");
/// The largest image side decoded, against a corrupt header.
const MAX_SIDE: usize = 4096;
/// Morrison Kühlsen's ink, and the text on it.
const INK: Color32 = Color32::from_rgb(0x14, 0x16, 0x1C);
const ON_INK: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const ON_INK_SOFT: Color32 = Color32::from_rgba_premultiplied(184, 184, 184, 184);
/// MK Perth's colour: lilac.
pub const BRAND: Color32 = Color32::from_rgb(0xB7, 0x9C, 0xED);
/// The artwork's lighter and deeper lilacs.
const LILAC_LIGHT: Color32 = Color32::from_rgb(0xDD, 0xCD, 0xFA);
const LILAC_DEEP: Color32 = Color32::from_rgb(0x7E, 0x5B, 0xC9);
const NAME: &str = "MK Perth";
const MOTTO: &str = "Festina lente";

/// The program's parts, named in turn as the bar fills.
fn components() -> [String; 12] {
    [
        tr!("splash-component-calendars"),
        tr!("splash-component-passes"),
        tr!("splash-component-floats"),
        tr!("splash-component-constraints"),
        tr!("splash-component-progress"),
        tr!("splash-component-wbs"),
        tr!("splash-component-perth"),
        tr!("splash-component-p6xml"),
        tr!("splash-component-engine"),
        tr!("splash-component-calculation"),
        tr!("splash-component-window"),
        tr!("splash-component-cli"),
    ]
}

#[derive(Debug, Default)]
pub struct SplashScreen {
    /// When it first showed, in egui's time.
    started: Option<f64>,
    closed: bool,
}

impl SplashScreen {
    /// Whether it asked its window to close.
    pub fn closed(&self) -> bool {
        self.closed
    }

    /// Draws it over the whole window; asks the window to close when its time is up.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = ctx.input(|i| i.time);
        let started = *self.started.get_or_insert(now);
        let progress = ((now - started) / HOLD).clamp(0.0, 1.0) as f32;
        // The whole window: the root Ui comes inset by a margin.
        let rect = ctx.content_rect();
        let response = ui.allocate_rect(ui.max_rect(), Sense::click());
        paint(&ui.painter().with_clip_rect(rect), &ctx, rect, progress);
        let done = progress >= 1.0 || response.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if done && !self.closed {
            self.closed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

fn paint(painter: &egui::Painter, ctx: &egui::Context, rect: Rect, progress: f32) {
    // The layout is drawn for 400 points of height and scales with the window.
    let k = rect.height() / LAYOUT_HEIGHT;
    // Two halves: the words on Morrison Kühlsen's ink, the artwork beside them.
    let half = rect.left() + rect.width() / 2.0;
    let words = Rect::from_min_max(rect.min, pos2(half, rect.bottom()));
    let art = Rect::from_min_max(pos2(half, rect.top()), rect.max);
    painter.rect_filled(rect, 0.0, INK);
    artwork(&painter.with_clip_rect(art), art, k);

    let left = words.left() + 36.0 * k;
    let width = words.width() - 72.0 * k;
    // The mark, in lilac, and the name.
    let mark = Rect::from_min_size(pos2(left, words.top() + 52.0 * k), vec2(46.0, 46.0) * k);
    let mut tokens = Tokens::dark();
    tokens.accent = BRAND;
    icons::paint(painter, mark, Icon::Mark, &tokens);
    painter.text(pos2(left + 58.0 * k, words.top() + 75.0 * k), Align2::LEFT_CENTER, NAME, FontId::proportional(34.0 * k), ON_INK);
    painter.text(pos2(left, words.top() + 128.0 * k), Align2::LEFT_CENTER, MOTTO, FontId::proportional(15.0 * k), ON_INK_SOFT);
    let tagline = painter.layout(tr!("splash-tagline"), FontId::proportional(12.5 * k), ON_INK_SOFT, width);
    let below = words.top() + 152.0 * k + tagline.size().y;
    painter.galley(pos2(left, words.top() + 152.0 * k), tagline, ON_INK_SOFT);
    painter.text(
        pos2(left, below + 10.0 * k),
        Align2::LEFT_TOP,
        tr!("splash-version", version = env!("CARGO_PKG_VERSION")),
        FontId::proportional(12.5 * k),
        ON_INK_SOFT,
    );

    // The parts loading, over the progress bar.
    let bar = Rect::from_min_size(pos2(left, words.bottom() - 96.0 * k), vec2(width, 3.0 * k));
    let components = components();
    let shown = ((progress * components.len() as f32) as usize).min(components.len().saturating_sub(1));
    let status = components.get(shown).map_or_else(String::new, |c| tr!("splash-loading", component = c));
    painter.text(bar.left_top() - vec2(0.0, 10.0 * k), Align2::LEFT_BOTTOM, status, FontId::proportional(12.0 * k), ON_INK_SOFT);
    painter.rect_filled(bar, 0.0, Color32::from_white_alpha(28));
    painter.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * progress, bar.height())), 0.0, BRAND);

    // The house: its wordmark and line at the foot.
    let foot = words.bottom() - 28.0 * k;
    if let Some(texture) = wordmark(ctx) {
        let logo_width = 104.0 * k;
        let size = vec2(logo_width, logo_width * texture.size_vec2().y / texture.size_vec2().x.max(1.0));
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), Rect::from_min_size(pos2(left, foot - size.y), size), uv, ON_INK);
    }
    painter.text(pos2(left + width, foot - 4.0 * k), Align2::RIGHT_BOTTOM, tr!("splash-house"), FontId::proportional(11.0 * k), ON_INK_SOFT);
}

/// The artwork, drawn in code: on a violet dusk, a schedule's bars cascade down to a milestone,
/// their relationships drawn as elbows, the critical ones in the brightest lilac.
fn artwork(painter: &egui::Painter, art: Rect, k: f32) {
    // The dusk: deep violet at the top left, lilac at the bottom right.
    let mut mesh = Mesh::default();
    for (pos, color) in [
        (art.left_top(), Color32::from_rgb(0x1C, 0x15, 0x33)),
        (art.right_top(), Color32::from_rgb(0x34, 0x24, 0x5E)),
        (art.right_bottom(), Color32::from_rgb(0x8A, 0x68, 0xD2)),
        (art.left_bottom(), Color32::from_rgb(0x44, 0x2D, 0x7C)),
    ] {
        mesh.vertices.push(Vertex { pos, uv: egui::epaint::WHITE_UV, color });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(mesh);
    // The time scale's faint lines: thirteen steps across, so the cascade and its milestone fit.
    let step = art.width() / 13.0;
    let mut x = art.left() + step / 2.0;
    while x < art.right() {
        painter.vline(x, art.y_range(), Stroke::new(1.0, Color32::from_white_alpha(10)));
        x += step;
    }
    // The bars: start and length in steps of the scale, critical or not; each row a step lower.
    let bars: [(f32, f32, bool); 10] = [
        (0.6, 3.0, true),
        (1.4, 2.2, false),
        (3.6, 2.6, true),
        (4.2, 1.6, false),
        (6.2, 2.4, true),
        (5.0, 3.4, false),
        (8.6, 1.8, true),
        (7.4, 2.0, false),
        (10.4, 1.6, true),
        (9.6, 1.2, false),
    ];
    let row = 30.0 * k;
    let top = art.top() + 58.0 * k;
    let at = |start: f32, n: usize| pos2(art.left() + start * step, top + n as f32 * row);
    let height = 11.0 * k;
    let mut critical_end: Option<Pos2> = None;
    for (n, &(start, length, critical)) in bars.iter().enumerate() {
        let min = at(start, n);
        let bar = Rect::from_min_size(min, vec2(length * step, height));
        // Each critical bar drives the next one: an elbow from its finish to that start.
        if critical {
            if let Some(from) = critical_end {
                let mid = pos2(from.x + 6.0 * k, from.y);
                let points = vec![from, mid, pos2(mid.x, bar.center().y), pos2(bar.left(), bar.center().y)];
                painter.add(PathShape::line(points, Stroke::new(1.4 * k, LILAC_LIGHT.gamma_multiply(0.55))));
            }
            critical_end = Some(pos2(bar.right(), bar.center().y));
        }
        let fill = if critical { BRAND } else { LILAC_DEEP.gamma_multiply(0.75) };
        painter.rect_filled(bar, 3.0 * k, fill);
        if !critical {
            // The float, as a thin line on to where the bar could finish.
            let float = pos2(bar.right() + 1.2 * step, bar.center().y);
            painter.line_segment([pos2(bar.right(), bar.center().y), float], Stroke::new(1.0 * k, LILAC_LIGHT.gamma_multiply(0.35)));
        }
    }
    // The finish: a milestone after the last critical bar.
    if let Some(end) = critical_end {
        let c = pos2(end.x + 18.0 * k, end.y + row);
        let r = 7.0 * k;
        painter.line_segment([end, pos2(c.x, end.y)], Stroke::new(1.4 * k, LILAC_LIGHT.gamma_multiply(0.55)));
        painter.line_segment([pos2(c.x, end.y), pos2(c.x, c.y - r)], Stroke::new(1.4 * k, LILAC_LIGHT.gamma_multiply(0.55)));
        let diamond = vec![pos2(c.x, c.y - r), pos2(c.x + r, c.y), pos2(c.x, c.y + r), pos2(c.x - r, c.y)];
        painter.add(PathShape::convex_polygon(diamond, LILAC_LIGHT, Stroke::NONE));
    }
    // Today, a dashed line across the schedule.
    let today = art.left() + 7.0 * step;
    painter.add(egui::Shape::dashed_line(
        &[pos2(today, top - 18.0 * k), pos2(today, art.bottom() - 30.0 * k)],
        Stroke::new(1.2 * k, LILAC_LIGHT.gamma_multiply(0.6)),
        5.0 * k,
        4.0 * k,
    ));
}

/// The Morrison Kühlsen wordmark, white on clear, as a texture, loaded once per context.
fn wordmark(ctx: &egui::Context) -> Option<TextureHandle> {
    let id = egui::Id::new("splash-morrison-kuhlsen");
    if let Some(texture) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(texture);
    }
    let image = decode_png(MORRISON_KUHLSEN).ok()?;
    let texture = ctx.load_texture("splash-morrison-kuhlsen", image, TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

/// A PNG as an egui image (RGBA), refusing absurd sizes.
fn decode_png(bytes: &[u8]) -> Result<egui::ColorImage, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let (w, h) = (reader.info().width as usize, reader.info().height as usize);
    if w > MAX_SIDE || h > MAX_SIDE {
        return Err(format!("{w}×{h} is too large"));
    }
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("no buffer size")?];
    let frame = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    pixels.truncate(frame.buffer_size());
    let rgba: Vec<u8> = match frame.color_type {
        png::ColorType::Rgba => pixels,
        png::ColorType::Rgb => pixels.as_chunks::<3>().0.iter().flat_map(|[r, g, b]| [*r, *g, *b, 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.as_chunks::<2>().0.iter().flat_map(|[g, a]| [*g, *g, *g, *a]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => return Err("indexed colour".into()),
    };
    if rgba.len() != w.saturating_mul(h).saturating_mul(4) {
        return Err(format!("{} bytes for {w}×{h}", rgba.len()));
    }
    Ok(egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wordmark_decodes() {
        let logo = decode_png(MORRISON_KUHLSEN).unwrap();
        assert!(logo.size[0] > logo.size[1] * 2, "a wide wordmark");
    }

    #[test]
    fn hostile_images_never_panic() {
        assert!(decode_png(&[]).is_err());
        for cut in [2, 60, MORRISON_KUHLSEN.len() / 2] {
            let _ = decode_png(MORRISON_KUHLSEN.get(..cut).unwrap());
        }
        let mut flipped = MORRISON_KUHLSEN.to_vec();
        for i in (0..flipped.len()).step_by(97) {
            if let Some(b) = flipped.get_mut(i) {
                *b ^= 0xff;
            }
            let _ = decode_png(&flipped);
        }
    }

    #[test]
    fn it_closes_on_esc_and_after_its_time() {
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(SIZE[0], SIZE[1]))
            .build_ui_state(|ui, s: &mut SplashScreen| s.ui(ui), SplashScreen::default());
        harness.run_steps(2);
        assert!(!harness.state().closed());
        harness.key_press(egui::Key::Escape);
        harness.run_steps(1);
        assert!(harness.state().closed());
        let mut waiting = egui_kittest::Harness::builder()
            .with_size(egui::vec2(SIZE[0], SIZE[1]))
            .with_step_dt(1.0)
            .build_ui_state(|ui, s: &mut SplashScreen| s.ui(ui), SplashScreen::default());
        waiting.run_steps(7);
        assert!(waiting.state().closed(), "gone after {HOLD} s");
    }
}
