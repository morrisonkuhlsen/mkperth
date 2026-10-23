//! A month of days to pick a date from, as spreadsheets open one over a date cell: the month and
//! year between arrows, the weekdays from Monday, the days (the chosen one lit, today in the
//! accent).

use mkperth_engine::core::Date;
use mkperth_engine::i18n::tr;

use crate::icon_button;
use crate::icons::Icon;
use crate::theme::Tokens;

/// The month before or after `(year, month)`.
fn step(year: i32, month: u32, forward: bool) -> (i32, u32) {
    match (forward, month) {
        (true, 12) => (year + 1, 1),
        (true, m) => (year, m + 1),
        (false, 1) => (year - 1, 12),
        (false, m) => (year, m - 1),
    }
}

/// The month `id` shows (the chosen date's until an arrow moves it), and the day clicked.
pub(crate) fn month_grid(ui: &mut egui::Ui, id: egui::Id, chosen: Date, today: Option<Date>) -> Option<Date> {
    let t = Tokens::get(ui.ctx());
    let key = id.with("month");
    let (y, m, _) = chosen.ymd();
    let (mut year, mut month) = ui.data(|d| d.get_temp::<(i32, u32)>(key)).unwrap_or((y, m));
    ui.horizontal(|ui| {
        if icon_button(ui, Icon::ChevronLeft, &tr!("date-previous-month")) {
            (year, month) = step(year, month, false);
        }
        let name = format!("{} {year}", tr!(&format!("month-{month}")));
        ui.add_sized(egui::vec2(150.0, 20.0), egui::Label::new(egui::RichText::new(name).size(13.0).color(t.text).strong()));
        if icon_button(ui, Icon::ChevronRight, &tr!("date-next-month")) {
            (year, month) = step(year, month, true);
        }
    });
    ui.data_mut(|d| d.insert_temp(key, (year, month)));
    let first = Date::from_ymd(year, month, 1)?;
    let mut picked = None;
    egui::Grid::new(id.with("days")).spacing(egui::vec2(2.0, 2.0)).show(ui, |ui| {
        for w in 1..=7 {
            ui.label(egui::RichText::new(tr!(&format!("weekday-{w}"))).size(11.0).color(t.text_dim));
        }
        ui.end_row();
        // Monday first: the days before the 1st are blank.
        let lead = first.weekday().index();
        for _ in 0..lead {
            ui.label("");
        }
        let (mut column, mut day) = (lead, Some(first));
        while let Some(d) = day.filter(|d| d.ymd().1 == month) {
            let mut text = egui::RichText::new(d.ymd().2.to_string()).size(12.0);
            if today == Some(d) {
                text = text.color(t.accent).strong();
            }
            if ui.add(egui::Button::selectable(d == chosen, text).min_size(egui::vec2(30.0, 22.0))).clicked() {
                picked = Some(d);
            }
            column += 1;
            if column == 7 {
                ui.end_row();
                column = 0;
            }
            day = d.succ();
        }
    });
    picked
}

/// Forgets the month `id` was showing, so it opens again on its date.
pub(crate) fn forget(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|d| d.remove::<(i32, u32)>(id.with("month")));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_step_over_the_year() {
        assert_eq!(step(2026, 12, true), (2027, 1));
        assert_eq!(step(2027, 1, false), (2026, 12));
        assert_eq!(step(2026, 5, true), (2026, 6));
    }
}
