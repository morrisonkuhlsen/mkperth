//! The typeface the interface is written in, and its size and style: egui's own (Ubuntu Light),
//! or one the system has, read through the app's services (no font is ever bundled). The size
//! scales every face, so text drawn at a fixed size grows with the rest.

use std::sync::Arc;

use egui::epaint::text::{FontData, FontDefinitions, FontFamily, FontTweak};

/// The face added before egui's own in the proportional family.
const FACE: &str = "mkperth-face";
/// The family emphasis is set in (summaries' rows, the calculation's dates): the system's bold
/// face first, the proportional family's after it for the glyphs it lacks.
const BOLD: &str = "mkperth-bold";
const BOLD_FACE: &str = "mkperth-bold-face";
/// The size, in points as spreadsheets count them: 10 is the usual, every face scaled by its
/// tenth; the usual ones offered, and the least and most.
pub const SIZES: [u8; 10] = [6, 7, 8, 9, 10, 11, 12, 14, 18, 24];
pub const MIN_SIZE: u8 = 6;
pub const MAX_SIZE: u8 = 24;

/// A size kept within the least and the most.
pub fn clamp_size(size: u8) -> u8 {
    size.clamp(MIN_SIZE, MAX_SIZE)
}

/// The interface's typeface: a system family (`None` for egui's own), its size in percent, and
/// whether it is set bold or italic (when the family has those faces).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontChoice {
    pub family: Option<String>,
    pub size: u8,
    pub bold: bool,
    pub italic: bool,
}

impl Default for FontChoice {
    fn default() -> Self {
        FontChoice { family: None, size: 10, bold: false, italic: false }
    }
}

impl FontChoice {
    /// `Segoe UI|12|bold|italic` (empty family for egui's own), for keeping between runs.
    pub fn to_line(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.family.as_deref().unwrap_or(""),
            self.size,
            if self.bold { "bold" } else { "" },
            if self.italic { "italic" } else { "" }
        )
    }

    /// The choice a line written by `to_line` keeps; the usual one where it doesn't read.
    pub fn from_line(line: &str) -> FontChoice {
        let mut parts = line.trim().split('|');
        let family = parts.next().filter(|f| !f.trim().is_empty()).map(|f| f.trim().to_string());
        // A size kept in percent before (90 to 130) is its tenth in points.
        let size = match parts.next().and_then(|s| s.parse::<u8>().ok()) {
            Some(s) if (MIN_SIZE..=MAX_SIZE).contains(&s) => s,
            Some(s) if (90..=130).contains(&s) => ((u16::from(s) + 5) / 10) as u8,
            _ => 10,
        };
        let bold = parts.next() == Some("bold");
        let italic = parts.next() == Some("italic");
        FontChoice { family, size, bold, italic }
    }
}

/// egui's fonts with the choice applied: the chosen face (from `bytes`, when given) first in the
/// proportional family, egui's after it for the glyphs it lacks; every face scaled to the size.
pub(crate) fn definitions(choice: &FontChoice, bytes: Option<Vec<u8>>, bold: Option<Vec<u8>>) -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    let scale = f32::from(clamp_size(choice.size)) / 10.0;
    for data in defs.font_data.values_mut() {
        let mut scaled = FontData::clone(data);
        scaled.tweak.scale *= scale;
        *data = Arc::new(scaled);
    }
    // A file egui can't read would stop it drawing anything: only a font that reads goes in.
    if let Some(bytes) = bytes.filter(|b| reads(b)) {
        defs.font_data.insert(FACE.into(), Arc::new(FontData::from_owned(bytes).tweak(FontTweak { scale, ..FontTweak::default() })));
        defs.families.entry(FontFamily::Proportional).or_default().insert(0, FACE.into());
    }
    let mut emphasis = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if let Some(bytes) = bold.filter(|b| reads(b)) {
        defs.font_data.insert(BOLD_FACE.into(), Arc::new(FontData::from_owned(bytes).tweak(FontTweak { scale, ..FontTweak::default() })));
        emphasis.insert(0, BOLD_FACE.into());
    }
    defs.families.insert(FontFamily::Name(BOLD.into()), emphasis);
    defs
}

/// Installs the fonts, with or without a bold face (`bold`, its file's bytes).
pub(crate) fn install(ctx: &egui::Context, choice: &FontChoice, bytes: Option<Vec<u8>>, bold: Option<Vec<u8>>) {
    let has_bold = bold.as_deref().is_some_and(reads);
    ctx.set_fonts(definitions(choice, bytes, bold));
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(BOLD), has_bold));
}

/// The bold face at `size`, when the system gave one.
pub(crate) fn bold(ctx: &egui::Context, size: f32) -> Option<egui::FontId> {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::unique(BOLD))).unwrap_or(false).then(|| egui::FontId::new(size, FontFamily::Name(BOLD.into())))
}

/// Text set bold: in the bold face, or, without one, drawn twice half a point apart.
pub(crate) fn strong(painter: &egui::Painter, at: egui::Pos2, align: egui::Align2, text: &str, size: f32, color: egui::Color32) {
    match bold(painter.ctx(), size) {
        Some(font) => {
            painter.text(at, align, text, font, color);
        }
        None => {
            for dx in [0.0, 0.5] {
                painter.text(at + egui::vec2(dx, 0.0), align, text, egui::FontId::proportional(size), color);
            }
        }
    }
}

/// Whether `bytes` are a font file's that egui's reader can open.
fn reads(bytes: &[u8]) -> bool {
    !bytes.is_empty() && skrifa::FontRef::from_index(bytes, 0).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_is_kept_as_a_line() {
        let c = FontChoice { family: Some("Segoe UI".into()), size: 12, bold: true, italic: false };
        assert_eq!(c.to_line(), "Segoe UI|12|bold|");
        assert_eq!(FontChoice::from_line(&c.to_line()), c);
        assert_eq!(FontChoice::from_line(""), FontChoice::default());
        assert_eq!(FontChoice::from_line("Arial|999|x|y"), FontChoice { family: Some("Arial".into()), ..FontChoice::default() });
        assert_eq!(FontChoice::from_line("|115||").size, 12, "a size kept in percent");
    }

    #[test]
    fn the_size_scales_every_face_and_the_chosen_one_comes_first() {
        let choice = FontChoice { size: 13, ..FontChoice::default() };
        let usual = FontDefinitions::default();
        let defs = definitions(&choice, None, None);
        for (name, data) in &usual.font_data {
            let scaled = defs.font_data.get(name).map(|d| d.tweak.scale);
            assert_eq!(scaled, Some(data.tweak.scale * 1.3), "{name}");
        }
        // No bytes, or bytes that aren't a font's, no face of its own: egui's stay.
        assert!(!defs.font_data.contains_key(FACE));
        assert!(!definitions(&choice, Some(b"not a font at all".to_vec()), None).font_data.contains_key(FACE));
        assert!(!definitions(&choice, Some(vec![0, 1, 0, 0]), None).font_data.contains_key(FACE));
    }
}
