//! The window's colours, light and dark. Neutral ink for now: the brand's colour is the
//! maintainer's to choose (#18).

use egui::Color32;

#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub background: Color32,
    pub stripe: Color32,
    pub header: Color32,
    pub selection: Color32,
    pub grid: Color32,
    pub grid_soft: Color32,
    pub text: Color32,
    pub text_soft: Color32,
    pub bar: Color32,
    pub late: Color32,
    pub critical: Color32,
    pub summary: Color32,
    pub non_working: Color32,
    pub warning: Color32,
}

impl Tokens {
    pub fn get(ctx: &egui::Context) -> Tokens {
        if ctx.theme() == egui::Theme::Dark { Tokens::DARK } else { Tokens::LIGHT }
    }

    pub const LIGHT: Tokens = Tokens {
        background: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        stripe: Color32::from_rgb(0xF6, 0xF7, 0xF9),
        header: Color32::from_rgb(0xEC, 0xEE, 0xF2),
        selection: Color32::from_rgb(0xDD, 0xE6, 0xF7),
        grid: Color32::from_rgb(0xD5, 0xD9, 0xE0),
        grid_soft: Color32::from_rgb(0xEC, 0xEE, 0xF2),
        text: Color32::from_rgb(0x14, 0x16, 0x1C),
        text_soft: Color32::from_rgb(0x5E, 0x66, 0x73),
        bar: Color32::from_rgb(0x3D, 0x6F, 0xB6),
        late: Color32::from_rgb(0xA9, 0xB1, 0xBE),
        critical: Color32::from_rgb(0xC9, 0x3C, 0x3C),
        summary: Color32::from_rgb(0x14, 0x16, 0x1C),
        non_working: Color32::from_rgba_premultiplied(0x14, 0x16, 0x1C, 0x0D),
        warning: Color32::from_rgb(0xA8, 0x6A, 0x00),
    };

    pub const DARK: Tokens = Tokens {
        background: Color32::from_rgb(0x1A, 0x1C, 0x22),
        stripe: Color32::from_rgb(0x20, 0x23, 0x2A),
        header: Color32::from_rgb(0x26, 0x29, 0x31),
        selection: Color32::from_rgb(0x2C, 0x3B, 0x55),
        grid: Color32::from_rgb(0x34, 0x38, 0x42),
        grid_soft: Color32::from_rgb(0x26, 0x29, 0x31),
        text: Color32::from_rgb(0xE8, 0xEA, 0xEE),
        text_soft: Color32::from_rgb(0x9A, 0xA1, 0xAD),
        bar: Color32::from_rgb(0x6B, 0x9B, 0xE0),
        late: Color32::from_rgb(0x5A, 0x60, 0x6C),
        critical: Color32::from_rgb(0xF0, 0x6A, 0x6A),
        summary: Color32::from_rgb(0xE8, 0xEA, 0xEE),
        non_working: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x30),
        warning: Color32::from_rgb(0xE0, 0xA8, 0x3C),
    };
}
