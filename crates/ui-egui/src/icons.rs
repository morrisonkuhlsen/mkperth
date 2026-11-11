//! MK Perth's icons, drawn in code (original artwork, nothing bundled). The system is the
//! house's, as MK Matrix draws its set (MIT OR Apache-2.0, the same owner; see NOTICE): line art
//! at one stroke weight on a 16×16 grid whose live area runs from 1.5 to 14.5, lines on half
//! units so a one-unit stroke covers whole pixels at 16 and 32 points, square corners and ends;
//! the icon in the text's colour, the part that is its action (the arrow, the new row, the
//! driving bar) in a quiet ink of its family, secondary lines muted. The scheduler's own icons
//! (the Gantt chart, the network, the critical path, levelling, PERT) are drawn the same way.

use std::f32::consts::{FRAC_PI_2, PI};

use egui::epaint::{PathShape, PathStroke};
use egui::{Color32, Painter, Pos2, Rect, Stroke, pos2, vec2};

use crate::theme::Tokens;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    /// MK Perth's mark (the brand is still open, #18).
    Mark,
    /// A Gantt chart: bars along a time line.
    Gantt,
    /// A calendar page.
    Calendar,
    /// The critical path: bars tied one after another.
    Critical,
    /// Activities as boxes linked by arrows.
    Network,
    Open,
    Save,
    Undo,
    Redo,
    /// Schedule: a play mark.
    Run,
    Table,
    Filter,
    /// A row added to a table: a new activity.
    InsertCase,
    /// A row taken out of a table: an activity deleted.
    Delete,
    /// Arrows out to two edges: the whole of it in view.
    Fit,
    /// Rows with a bullet each: the calculation's steps.
    List,
    Search,
    /// Light and dark: a crescent and a star.
    Contrast,
    Share,
    Minus,
    Plus,
    /// A flag on its staff: the chart's notes.
    Pin,
    /// Two people, one before the other: resources.
    People,
    /// Three even bars under a line: resources levelled within their limit.
    Level,
    /// Two links of a chain: activities tied one after another.
    Link,
    /// The two links apart, the break marked.
    Unlink,
    /// Lines with an arrow into them: under the summary above.
    Indent,
    /// Lines with an arrow out of them: out of its summary.
    Outdent,
    /// A speech bubble: a comment.
    Comment,
    /// A calendar page with today marked.
    Today,
    /// Cross hairs on a dot: go to the chosen activity.
    Locate,
    /// A bell curve over its axis, the mean marked: PERT's spread.
    Pert,
    /// Rows, every other one tinted: a striped table.
    Stripes,
    /// A screen on its stand with a play mark: presenting.
    Present,
    /// Bars of a chart: the budget.
    Budget,
    /// A banknote: costs.
    Costs,
    /// An S-curve over its axes: earned value.
    EarnedValue,
    /// A tag: codes and fields.
    Codes,
    ChevronUp,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    House,
    /// A new page.
    Page,
    Info,
    Close,
    /// The LaTeX name typeset as LaTeX sets it (raised A, lowered E): the name, not the
    /// LaTeX Project's logo artwork, whose terms don't allow it as a program's icon.
    Latex,
}

/// The stroke every line is drawn with, in grid units: one pixel at 16 points, two at 32.
const W: f32 = 1.0;

/// A painter scaled to the 16×16 design grid.
struct G<'a> {
    p: &'a Painter,
    r: Rect,
    s: f32,
}

impl G<'_> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        pos2(self.r.left() + x * self.s, self.r.top() + y * self.s)
    }
    fn width(&self, w: f32) -> f32 {
        (w * self.s).max(1.0)
    }
    /// An open polyline `w` units wide.
    fn line(&self, pts: &[(f32, f32)], c: Color32, w: f32) {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| self.pt(*x, *y)).collect();
        self.p.add(PathShape::line(v, PathStroke::new(self.width(w), c)));
    }
    /// An open polyline at the set's stroke.
    fn stroke(&self, pts: &[(f32, f32)], c: Color32) {
        self.line(pts, c, W);
    }
    /// A closed outline at the set's stroke.
    fn closed(&self, pts: &[(f32, f32)], c: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| self.pt(*x, *y)).collect();
        self.p.add(PathShape::closed_line(v, PathStroke::new(self.width(W), c)));
    }
    /// A rectangle's outline, centred on its lines.
    fn frame(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.closed(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], c);
    }
    /// A filled rectangle.
    fn block(&self, x0: f32, y0: f32, x1: f32, y1: f32, c: Color32) {
        self.p.rect_filled(Rect::from_min_max(self.pt(x0, y0), self.pt(x1, y1)), 0.0, c);
    }
    /// A filled convex polygon.
    fn poly(&self, pts: &[(f32, f32)], fill: Color32) {
        let v: Vec<Pos2> = pts.iter().map(|(x, y)| self.pt(*x, *y)).collect();
        self.p.add(PathShape::convex_polygon(v, fill, Stroke::NONE));
    }
    fn ring(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_stroke(self.pt(x, y), r * self.s, Stroke::new(self.width(W), c));
    }
    fn dot(&self, x: f32, y: f32, r: f32, c: Color32) {
        self.p.circle_filled(self.pt(x, y), r * self.s, c);
    }
    /// A cubic Bézier curve from `p0` to `p3`.
    fn curve(&self, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), c: Color32) {
        let pts: Vec<(f32, f32)> = (0..=20)
            .map(|i| {
                let t = i as f32 / 20.0;
                let u = 1.0 - t;
                let (a, b, d, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                (a * p0.0 + b * p1.0 + d * p2.0 + e * p3.0, a * p0.1 + b * p1.1 + d * p2.1 + e * p3.1)
            })
            .collect();
        self.stroke(&pts, c);
    }
    /// A solid arrowhead with its tip at (x, y), pointing along angle `a` (radians).
    fn head(&self, x: f32, y: f32, a: f32, c: Color32) {
        let (len, half) = (3.0, 2.0);
        let (dx, dy) = (a.cos(), a.sin());
        let (bx, by) = (x - dx * len, y - dy * len);
        self.poly(&[(x, y), (bx - dy * half, by + dx * half), (bx + dy * half, by - dx * half)], c);
    }
}

/// The quiet inks the icons' action parts are drawn in, one per family, as (light theme, dark
/// theme): the house's, as MK Matrix has them.
mod ink {
    use egui::Color32;
    pub(super) const TERRACOTTA: (Color32, Color32) = (Color32::from_rgb(0xA0, 0x52, 0x2D), Color32::from_rgb(0xE0, 0x8A, 0x5F));
    pub(super) const SLATE: (Color32, Color32) = (Color32::from_rgb(0x3E, 0x5C, 0x76), Color32::from_rgb(0x8F, 0xB0, 0xCC));
    pub(super) const MUSTARD: (Color32, Color32) = (Color32::from_rgb(0x9A, 0x6B, 0x12), Color32::from_rgb(0xE0, 0xB0, 0x4F));
    pub(super) const OLIVE: (Color32, Color32) = (Color32::from_rgb(0x6B, 0x7A, 0x3A), Color32::from_rgb(0xAE, 0xBB, 0x7A));
    pub(super) const OXBLOOD: (Color32, Color32) = (Color32::from_rgb(0x9B, 0x2D, 0x35), Color32::from_rgb(0xE5, 0x86, 0x8F));
}

impl Icon {
    /// The ink of this icon's family: the project and its files in terracotta; what the chart
    /// shows in slate; time, resources and money in mustard; the activities and their ties in
    /// olive; the critical path in oxblood. `None` for the interface's own (arrows, close, plus,
    /// minus), drawn in their main colour alone.
    fn ink(self, dark: bool) -> Option<Color32> {
        use Icon::*;
        let (light, night) = match self {
            Undo | Redo | Save | Open | Page | Share | Search | House | Info | Present | Run => ink::TERRACOTTA,
            Gantt | Network | Table | Fit | Stripes | Contrast | Locate | List | Filter => ink::SLATE,
            Calendar | Today | Level | People | Pert | Budget | Costs | EarnedValue => ink::MUSTARD,
            InsertCase | Delete | Link | Unlink | Indent | Outdent | Comment | Pin | Codes => ink::OLIVE,
            Critical => ink::OXBLOOD,
            _ => return None,
        };
        Some(if dark { night } else { light })
    }
}

/// Draws `icon` in `rect` in the theme's text colour, its action part in its family's ink.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, t: &Tokens) {
    paint_in(painter, rect, icon, t, t.text);
}

/// Draws `icon` in `c`, its action part in its family's ink: the toolbar's icons.
pub fn paint_in(painter: &Painter, rect: Rect, icon: Icon, t: &Tokens, c: Color32) {
    let a = icon.ink(t.dark).unwrap_or(c);
    draw(painter, rect, icon, c, a);
}

/// Draws `icon` in `c` alone, its action part included: the rails' and the title bar's icons.
pub fn paint_mono(painter: &Painter, rect: Rect, icon: Icon, c: Color32) {
    draw(painter, rect, icon, c, c);
}

/// An icon of `size` points in the layout.
pub fn show(ui: &mut egui::Ui, icon: Icon, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(size, size), egui::Sense::hover());
    paint(ui.painter(), rect, icon, &Tokens::get(ui.ctx()));
    response
}

fn draw(p: &Painter, rect: Rect, icon: Icon, c: Color32, a: Color32) {
    let side = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), vec2(side, side));
    let g = G { p, r, s: side / 16.0 };
    let muted = c.gamma_multiply(0.45);
    let wash = a.gamma_multiply(0.3);
    let (up, left, right) = (-FRAC_PI_2, PI, 0.0);
    // A page with a folded corner, the shape documents share.
    let page = |g: &G| {
        g.closed(&[(3.5, 1.5), (9.5, 1.5), (12.5, 4.5), (12.5, 14.5), (3.5, 14.5)], c);
        g.stroke(&[(9.5, 1.5), (9.5, 4.5), (12.5, 4.5)], c);
    };
    // A table of rows, its columns muted: activities.
    let rows = |g: &G| {
        g.frame(5.5, 2.5, 14.5, 13.5, c);
        g.stroke(&[(5.5, 6.17), (14.5, 6.17)], muted);
        g.stroke(&[(5.5, 9.83), (14.5, 9.83)], muted);
        g.stroke(&[(9.5, 2.5), (9.5, 13.5)], muted);
    };
    // A calendar page: its rings, its days.
    let calendar = |g: &G, header: bool| {
        if header {
            g.block(1.5, 2.5, 14.5, 5.5, a);
        } else {
            g.stroke(&[(1.5, 5.5), (14.5, 5.5)], c);
        }
        g.frame(1.5, 2.5, 14.5, 14.5, c);
        g.stroke(&[(4.5, 1.0), (4.5, 3.5)], c);
        g.stroke(&[(11.5, 1.0), (11.5, 3.5)], c);
    };
    // Lines of text, the second and third indented: the outline the arrows move in and out of.
    let outline = |g: &G| {
        for (i, y) in [2.5f32, 6.0, 9.5, 13.0].iter().enumerate() {
            let x0 = if i == 0 || i == 3 { 1.5 } else { 7.5 };
            g.stroke(&[(x0, *y), (14.5, *y)], c);
        }
    };
    let link = |g: &G, apart: f32| {
        g.closed(&[(2.4 - apart, 9.3 + apart), (5.3 - apart, 6.4 + apart), (7.7 - apart, 8.8 + apart), (4.8 - apart, 11.7 + apart)], c);
        g.closed(&[(8.3 + apart, 7.2 - apart), (11.2 + apart, 4.3 - apart), (13.6 + apart, 6.7 - apart), (10.7 + apart, 9.6 - apart)], c);
    };
    match icon {
        Icon::Mark => {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            p.image(crate::mark::texture(p.ctx()).id(), r, uv, Color32::WHITE);
        }
        Icon::Gantt => {
            g.stroke(&[(1.5, 1.5), (1.5, 14.5)], muted);
            g.frame(3.5, 2.5, 8.5, 5.5, c);
            g.block(6.5, 6.5, 12.5, 9.5, a);
            g.frame(9.5, 10.5, 14.5, 13.5, c);
        }
        Icon::Critical => {
            g.block(1.5, 2.5, 6.5, 5.5, a);
            g.block(6.5, 6.5, 10.5, 9.5, a);
            g.block(10.5, 10.5, 14.5, 13.5, a);
            g.stroke(&[(4.0, 5.5), (4.0, 8.0), (5.5, 8.0)], c);
            g.stroke(&[(8.5, 9.5), (8.5, 12.0), (9.5, 12.0)], c);
        }
        Icon::Network => {
            g.frame(1.5, 6.5, 5.5, 9.5, c);
            g.frame(10.5, 1.5, 14.5, 4.5, c);
            g.frame(10.5, 11.5, 14.5, 14.5, c);
            g.stroke(&[(5.5, 8.0), (7.5, 8.0), (7.5, 3.0), (8.5, 3.0)], a);
            g.stroke(&[(7.5, 8.0), (7.5, 13.0), (8.5, 13.0)], a);
            g.head(10.5, 3.0, right, a);
            g.head(10.5, 13.0, right, a);
        }
        Icon::Calendar => {
            calendar(&g, true);
            for (x, y) in [(4.5, 8.5), (8.0, 8.5), (11.5, 8.5), (4.5, 11.75), (8.0, 11.75)] {
                g.block(x - 0.75, y - 0.75, x + 0.75, y + 0.75, c);
            }
        }
        Icon::Today => {
            calendar(&g, false);
            for (x, y) in [(4.5, 8.5), (8.0, 8.5), (11.5, 8.5), (4.5, 11.75)] {
                g.block(x - 0.75, y - 0.75, x + 0.75, y + 0.75, muted);
            }
            g.block(9.5, 10.0, 12.5, 13.0, a);
        }
        Icon::Open => {
            g.stroke(&[(12.5, 6.5), (12.5, 4.5), (7.5, 4.5), (6.0, 2.5), (1.5, 2.5), (1.5, 13.5)], c);
            g.closed(&[(1.5, 13.5), (4.0, 7.5), (15.0, 7.5), (12.5, 13.5)], c);
            g.stroke(&[(4.5, 10.5), (12.0, 10.5)], a);
        }
        Icon::Save => {
            g.closed(&[(1.5, 1.5), (11.5, 1.5), (14.5, 4.5), (14.5, 14.5), (1.5, 14.5)], c);
            g.frame(4.5, 1.5, 10.5, 5.5, c);
            g.block(4.0, 9.0, 12.0, 14.5, a);
        }
        Icon::Undo | Icon::Redo => {
            // An arrow back along the top, turning down and round to the bottom.
            let flip = icon == Icon::Redo;
            let fx = |v: f32| if flip { 16.0 - v } else { v };
            let arc: Vec<(f32, f32)> = (0..=16)
                .map(|i| {
                    let k = -FRAC_PI_2 + PI * i as f32 / 16.0;
                    (fx(10.0 + 4.0 * k.cos()), 9.5 + 4.0 * k.sin())
                })
                .collect();
            let mut path = vec![(fx(4.5), 5.5)];
            path.extend(arc);
            path.push((fx(5.5), 13.5));
            g.stroke(&path, c);
            g.head(fx(1.5), 5.5, if flip { right } else { left }, a);
        }
        Icon::Run => g.poly(&[(4.5, 2.0), (13.5, 8.0), (4.5, 14.0)], a),
        Icon::Table => {
            g.block(1.5, 2.5, 14.5, 5.5, a);
            g.frame(1.5, 2.5, 14.5, 13.5, c);
            g.stroke(&[(5.5, 5.5), (5.5, 13.5)], muted);
            g.stroke(&[(1.5, 9.5), (14.5, 9.5)], muted);
        }
        Icon::Filter => {
            g.closed(&[(1.5, 2.5), (14.5, 2.5), (9.5, 8.5), (9.5, 14.0), (6.5, 12.5), (6.5, 8.5)], c);
            g.stroke(&[(4.0, 5.0), (12.0, 5.0)], a);
        }
        Icon::InsertCase => {
            rows(&g);
            g.stroke(&[(0.5, 8.0), (4.5, 8.0)], a);
            g.stroke(&[(2.5, 6.0), (2.5, 10.0)], a);
        }
        Icon::Delete => {
            rows(&g);
            g.stroke(&[(0.8, 6.3), (4.2, 9.7)], a);
            g.stroke(&[(4.2, 6.3), (0.8, 9.7)], a);
        }
        Icon::Fit => {
            g.stroke(&[(1.5, 3.5), (1.5, 12.5)], c);
            g.stroke(&[(14.5, 3.5), (14.5, 12.5)], c);
            g.stroke(&[(5.5, 8.0), (10.5, 8.0)], a);
            g.head(3.5, 8.0, left, a);
            g.head(12.5, 8.0, right, a);
        }
        Icon::List => {
            for y in [3.5, 8.0, 12.5] {
                g.block(1.5, y - 1.0, 3.5, y + 1.0, a);
                g.stroke(&[(5.5, y), (14.5, y)], c);
            }
        }
        Icon::Search => {
            g.ring(6.5, 6.5, 4.5, c);
            g.line(&[(9.8, 9.8), (14.5, 14.5)], c, 1.6);
        }
        Icon::Contrast => {
            // A crescent in line art, and a four-pointed star in the bite it leaves.
            let (outer, bite) = (((7.0, 9.0), 6.0), ((10.6, 5.6), 4.9));
            let arc = |((cx, cy), rad): ((f32, f32), f32), a0: f32, a1: f32| -> Vec<(f32, f32)> {
                (0..=24)
                    .map(|i| {
                        let k = a0 + (a1 - a0) * i as f32 / 24.0;
                        (cx + rad * k.cos(), cy + rad * k.sin())
                    })
                    .collect()
            };
            g.stroke(&arc(outer, 0.1523, 4.6172), c);
            g.stroke(&arc(bite, 3.6944, 1.0751), c);
            g.poly(&[(13.0, 0.75), (13.55, 2.25), (15.0, 2.8), (13.55, 3.35), (13.0, 4.85), (12.45, 3.35), (11.0, 2.8), (12.45, 2.25)], a);
        }
        Icon::Share => {
            g.stroke(&[(5.5, 6.5), (2.5, 6.5), (2.5, 14.5), (13.5, 14.5), (13.5, 6.5), (10.5, 6.5)], c);
            g.stroke(&[(8.0, 11.0), (8.0, 4.0)], a);
            g.head(8.0, 1.0, up, a);
        }
        Icon::Minus => g.line(&[(2.5, 8.0), (13.5, 8.0)], c, 1.2),
        Icon::Plus => {
            g.line(&[(8.0, 2.5), (8.0, 13.5)], c, 1.2);
            g.line(&[(2.5, 8.0), (13.5, 8.0)], c, 1.2);
        }
        Icon::Pin => {
            g.stroke(&[(3.5, 1.5), (3.5, 14.5)], c);
            g.poly(&[(3.5, 2.5), (13.5, 2.5), (11.0, 5.5), (13.5, 8.5), (3.5, 8.5)], wash);
            g.closed(&[(3.5, 2.5), (13.5, 2.5), (11.0, 5.5), (13.5, 8.5), (3.5, 8.5)], a);
        }
        Icon::People => {
            // The one behind, in the ink; the one before it, in the main colour.
            g.ring(11.5, 4.5, 2.0, a);
            g.stroke(&[(12.0, 9.0), (13.0, 9.0), (14.5, 10.5), (14.5, 13.5)], a);
            g.ring(6.0, 5.0, 2.5, c);
            g.stroke(&[(1.5, 14.5), (1.5, 12.0), (3.5, 10.0), (8.5, 10.0), (10.5, 12.0), (10.5, 14.5)], c);
        }
        Icon::Level => {
            g.stroke(&[(1.0, 5.5), (15.0, 5.5)], a);
            g.frame(2.5, 7.5, 5.5, 14.5, c);
            g.frame(6.5, 7.5, 9.5, 14.5, c);
            g.frame(10.5, 7.5, 13.5, 14.5, c);
        }
        Icon::Link => {
            link(&g, 0.0);
            g.stroke(&[(6.0, 10.0), (10.0, 6.0)], a);
        }
        Icon::Unlink => {
            link(&g, 0.8);
            g.stroke(&[(7.0, 4.0), (7.0, 1.5)], a);
            g.stroke(&[(9.0, 12.0), (9.0, 14.5)], a);
            g.stroke(&[(4.0, 7.0), (1.5, 7.0)], a);
            g.stroke(&[(12.0, 9.0), (14.5, 9.0)], a);
        }
        Icon::Indent => {
            outline(&g);
            g.head(5.5, 7.75, right, a);
        }
        Icon::Outdent => {
            outline(&g);
            g.head(1.5, 7.75, left, a);
        }
        Icon::Comment => {
            g.closed(&[(1.5, 2.5), (14.5, 2.5), (14.5, 10.5), (7.5, 10.5), (4.5, 13.5), (4.5, 10.5), (1.5, 10.5)], c);
            g.stroke(&[(4.5, 5.5), (11.5, 5.5)], a);
            g.stroke(&[(4.5, 7.5), (9.0, 7.5)], muted);
        }
        Icon::Locate => {
            g.ring(8.0, 8.0, 4.5, c);
            for (x0, y0, x1, y1) in [(8.0, 1.0, 8.0, 3.5), (8.0, 12.5, 8.0, 15.0), (1.0, 8.0, 3.5, 8.0), (12.5, 8.0, 15.0, 8.0)] {
                g.stroke(&[(x0, y0), (x1, y1)], c);
            }
            g.dot(8.0, 8.0, 1.6, a);
        }
        Icon::Pert => {
            g.stroke(&[(1.0, 14.5), (15.0, 14.5)], c);
            let bell: Vec<(f32, f32)> = (0..=24)
                .map(|i| {
                    let x = 1.5 + 13.0 * i as f32 / 24.0;
                    let z = (x - 8.0) / 2.6;
                    (x, 14.0 - 11.0 * (-0.5 * z * z).exp())
                })
                .collect();
            g.stroke(&bell, c);
            g.stroke(&[(8.0, 3.0), (8.0, 14.5)], a);
        }
        Icon::Stripes => {
            g.block(1.5, 5.17, 14.5, 7.83, wash);
            g.block(1.5, 10.5, 14.5, 13.5, wash);
            g.frame(1.5, 2.5, 14.5, 13.5, c);
            g.stroke(&[(1.5, 5.17), (14.5, 5.17)], muted);
            g.stroke(&[(1.5, 7.83), (14.5, 7.83)], muted);
            g.stroke(&[(1.5, 10.5), (14.5, 10.5)], muted);
        }
        Icon::Present => {
            g.frame(1.5, 2.5, 14.5, 11.5, c);
            g.stroke(&[(8.0, 11.5), (8.0, 14.5)], c);
            g.stroke(&[(5.0, 14.5), (11.0, 14.5)], c);
            g.poly(&[(6.5, 4.75), (10.5, 7.0), (6.5, 9.25)], a);
        }
        Icon::Budget => {
            g.stroke(&[(1.5, 14.5), (14.5, 14.5)], c);
            g.frame(2.5, 8.5, 5.5, 14.5, c);
            g.block(6.5, 3.5, 9.5, 14.5, a);
            g.frame(10.5, 6.5, 13.5, 14.5, c);
        }
        Icon::Costs => {
            g.frame(1.5, 4.5, 14.5, 11.5, c);
            g.ring(8.0, 8.0, 2.0, a);
            g.dot(4.0, 8.0, 0.75, muted);
            g.dot(12.0, 8.0, 0.75, muted);
        }
        Icon::EarnedValue => {
            g.stroke(&[(1.5, 1.5), (1.5, 14.5), (14.5, 14.5)], c);
            g.curve((3.0, 12.5), (8.0, 12.5), (8.0, 3.5), (14.0, 3.5), a);
            g.stroke(&[(9.5, 1.5), (9.5, 13.5)], muted);
        }
        Icon::Codes => {
            g.closed(&[(1.5, 1.5), (8.0, 1.5), (14.5, 8.0), (8.0, 14.5), (1.5, 8.0)], c);
            g.ring(5.0, 5.0, 1.25, a);
        }
        Icon::ChevronUp => g.line(&[(4.0, 10.0), (8.0, 6.0), (12.0, 10.0)], c, 1.2),
        Icon::ChevronDown => g.line(&[(4.0, 6.0), (8.0, 10.0), (12.0, 6.0)], c, 1.2),
        Icon::ChevronLeft => g.line(&[(10.0, 3.5), (5.5, 8.0), (10.0, 12.5)], c, 1.2),
        Icon::ChevronRight => g.line(&[(6.0, 3.5), (10.5, 8.0), (6.0, 12.5)], c, 1.2),
        Icon::House => {
            g.stroke(&[(1.0, 8.0), (8.0, 1.5), (15.0, 8.0)], c);
            g.stroke(&[(3.5, 6.0), (3.5, 14.5), (12.5, 14.5), (12.5, 6.0)], c);
            g.block(6.5, 10.0, 9.5, 14.5, a);
        }
        Icon::Page => {
            page(&g);
            g.stroke(&[(8.0, 7.5), (8.0, 12.5)], a);
            g.stroke(&[(5.5, 10.0), (10.5, 10.0)], a);
        }
        Icon::Info => {
            g.ring(8.0, 8.0, 6.5, c);
            g.line(&[(8.0, 7.0), (8.0, 11.5)], c, 1.3);
            g.dot(8.0, 4.75, 0.9, a);
        }
        Icon::Close => {
            g.line(&[(3.5, 3.5), (12.5, 12.5)], c, 1.2);
            g.line(&[(12.5, 3.5), (3.5, 12.5)], c, 1.2);
        }
        Icon::Latex => {
            let big = egui::FontId::proportional(7.0 * g.s);
            let small = egui::FontId::proportional(5.2 * g.s);
            // As \LaTeX sets it: A raised and tucked under the L's arm, E lowered.
            for (letter, font, x, dy) in
                [("L", &big, -0.6, 0.0), ("A", &small, 2.5, -1.8), ("T", &big, 5.2, 0.0), ("E", &big, 8.5, 1.6), ("X", &big, 11.8, 0.0)]
            {
                p.text(g.pt(x, 8.0 + dy), egui::Align2::LEFT_CENTER, letter, font.clone(), c);
            }
        }
    }
}

/// For tests: every icon.
#[cfg(test)]
pub(crate) const ALL: [Icon; 47] = [
    Icon::Mark,
    Icon::Gantt,
    Icon::Calendar,
    Icon::Critical,
    Icon::Network,
    Icon::Open,
    Icon::Save,
    Icon::Undo,
    Icon::Redo,
    Icon::Run,
    Icon::Table,
    Icon::Filter,
    Icon::InsertCase,
    Icon::Delete,
    Icon::Fit,
    Icon::List,
    Icon::Search,
    Icon::Contrast,
    Icon::Share,
    Icon::Minus,
    Icon::Plus,
    Icon::Pin,
    Icon::People,
    Icon::Level,
    Icon::Link,
    Icon::Unlink,
    Icon::Indent,
    Icon::Outdent,
    Icon::Comment,
    Icon::Today,
    Icon::Locate,
    Icon::Pert,
    Icon::Stripes,
    Icon::Present,
    Icon::Budget,
    Icon::Costs,
    Icon::EarnedValue,
    Icon::Codes,
    Icon::ChevronUp,
    Icon::ChevronDown,
    Icon::ChevronLeft,
    Icon::ChevronRight,
    Icon::House,
    Icon::Page,
    Icon::Info,
    Icon::Close,
    Icon::Latex,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_stay_in_their_square() {
        let ctx = egui::Context::default();
        let rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(32.0, 32.0));
        for icon in ALL {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                paint(&ui.ctx().layer_painter(egui::LayerId::debug()), rect, icon, &Tokens::light());
            });
            out.textures_delta.clear();
            let drawn: Vec<_> = out.shapes.into_iter().filter(|c| c.shape.visual_bounding_rect().is_positive()).collect();
            assert!(!drawn.is_empty(), "{icon:?} draws something");
            for clipped in drawn {
                let bounds = clipped.shape.visual_bounding_rect();
                assert!(rect.expand(2.0).contains_rect(bounds), "{icon:?} leaves its square: {bounds:?}");
            }
        }
    }
}
