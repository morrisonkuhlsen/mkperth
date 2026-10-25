//! The Calendars pane: the project's calendars edited in the app. Which calendar, its working
//! week, its work periods, its region's holidays, and the days it takes off or works besides its
//! week. Every change is `calendar.edit`, undoable as any other.

use mkperth_engine::core::{Calendar, DEFAULT_PERIODS, Date, Period, Region};
use mkperth_engine::i18n::tr;
use serde_json::json;

use crate::icons::Icon;
use crate::theme;
use crate::{PerthApp, date_field, icon_button, show_date};

/// What the Calendars pane is editing: the calendar (empty for the project's own), the work
/// periods as typed, and the dates about to be added.
#[derive(Debug, Clone, Default)]
pub struct CalendarForm {
    pub calendar: String,
    /// The periods' text and the calendar and periods it was typed over (so another calendar, or
    /// an undo, shows its own).
    periods: Option<(String, Vec<Period>, String)>,
    pub holiday: Option<Date>,
    pub worked: Option<Date>,
}

/// `08:00–12:00, 13:00–17:00`.
pub fn periods_text(periods: &[Period]) -> String {
    let time = |m: u16| format!("{:02}:{:02}", m / 60, m % 60);
    periods.iter().map(|(a, b)| format!("{}–{}", time(*a), time(*b))).collect::<Vec<_>>().join(", ")
}

/// Periods typed as `08:00-12:00, 13:00-17:00` (a hyphen or a dash between the times, `8` for
/// `08:00`); `None` if any part isn't a time.
pub fn read_periods(text: &str) -> Option<Vec<Period>> {
    let time = |t: &str| -> Option<u16> {
        let (h, m) = t.trim().split_once(':').unwrap_or((t.trim(), "0"));
        let (h, m) = (h.trim().parse::<u16>().ok()?, m.trim().parse::<u16>().ok()?);
        (h <= 24 && m < 60).then(|| h * 60 + m).filter(|t| *t <= 1440)
    };
    text.split(',')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            let (a, b) = p.split_once(['-', '–', '—'])?;
            Some((time(a)?, time(b)?))
        })
        .collect()
}

/// The periods a calendar works on its first worked weekday (all its weekdays have the same
/// unless a file said otherwise); the default periods when it gives none.
fn day_periods(c: &Calendar) -> Vec<Period> {
    let first = c.week.iter().position(|w| *w).and_then(|d| c.periods.get(d)).filter(|p| !p.is_empty());
    first.cloned().unwrap_or_else(|| DEFAULT_PERIODS.to_vec())
}

impl PerthApp {
    pub(crate) fn calendars_pane(&mut self, ui: &mut egui::Ui) {
        if theme::pane_header(ui, Icon::Calendar, &tr!("pane-calendars"), &tr!("pane-calendars-tip"), |ui| {
            icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))
        }) {
            self.side = None;
        }
        let p = &self.session.project;
        // The calendar chosen, if it is still there; else the project's own.
        let own = self.calendar_form.calendar.is_empty() || self.calendar_form.calendar == p.calendar.name;
        let calendar = if own { Some(&p.calendar) } else { p.calendars.iter().find(|c| c.name == self.calendar_form.calendar) };
        let Some(calendar) = calendar.cloned() else {
            self.calendar_form.calendar.clear();
            return;
        };
        let names: Vec<String> = std::iter::once(p.calendar.name.clone()).chain(p.calendars.iter().map(|c| c.name.clone())).collect();
        let target = if own { None } else { Some(calendar.name.clone()) };
        let mut pending: Option<serde_json::Value> = None;
        let mut edit = |change: serde_json::Value| {
            let mut params = change;
            if let (Some(name), Some(map)) = (&target, params.as_object_mut()) {
                map.insert("calendar".into(), json!(name));
            }
            pending = Some(params);
        };
        let form = &mut self.calendar_form;
        egui::ScrollArea::vertical().id_salt("calendars-pane").auto_shrink(false).show(ui, |ui| {
            // Which calendar.
            theme::section(ui, &tr!("calendar-which"));
            let shown = if own { tr!("calendar-project", name = calendar.name.as_str()) } else { calendar.name.clone() };
            let combo = egui::ComboBox::from_id_salt("calendar-which").selected_text(shown).width(theme::fill(ui)).truncate().show_ui(ui, |ui| {
                for (k, name) in names.iter().enumerate() {
                    let text = if k == 0 { tr!("calendar-project", name = name.as_str()) } else { name.clone() };
                    let chosen = if k == 0 { String::new() } else { name.clone() };
                    if ui.selectable_label(form.calendar == chosen || (k == 0 && own), text).clicked() {
                        form.calendar = chosen;
                    }
                }
            });
            combo.response.on_hover_text(tr!("calendar-which"));

            // The week: seven toggles sharing the width.
            theme::section(ui, &tr!("calendar-week"));
            ui.horizontal(|ui| {
                let gap = 3.0;
                ui.spacing_mut().item_spacing.x = gap;
                let width = ((ui.available_width() - 6.0 * gap) / 7.0).floor().max(24.0);
                for day in 0..7 {
                    let on = calendar.week.get(day).copied().unwrap_or(false);
                    if ui.add_sized(egui::vec2(width, 24.0), egui::Button::selectable(on, tr!(&format!("weekday-{}", day + 1)))).clicked() {
                        let mut week = calendar.week;
                        if let Some(d) = week.get_mut(day) {
                            *d = !on;
                        }
                        edit(json!({"week": week}));
                    }
                }
            });

            theme::section(ui, &tr!("calendar-periods"));
            let periods = day_periods(&calendar);
            // The text typed for this calendar and these periods, else theirs.
            let typed = form.periods.as_ref().filter(|(name, was, _)| *name == calendar.name && *was == periods).map(|(_, _, text)| text.clone());
            let mut text = typed.unwrap_or_else(|| periods_text(&periods));
            let response = ui.add(egui::TextEdit::singleline(&mut text).desired_width(theme::fill(ui))).on_hover_text(tr!("calendar-periods-tip"));
            if response.lost_focus() {
                form.periods = None;
                match read_periods(&text) {
                    Some(read) if read != periods => edit(json!({"periods": read})),
                    Some(_) => {}
                    None => edit(json!({"periods": "unreadable"})),
                }
            } else if response.changed() {
                form.periods = Some((calendar.name.clone(), periods.clone(), text));
            }
            let minutes: u32 = periods.iter().map(|(a, b)| u32::from(b.saturating_sub(*a))).sum();
            theme::hint(ui, &format!("{} · {}", tr!("calendar-hours", hours = f64::from(minutes) / 60.0), tr!("calendar-periods-tip")));

            theme::section(ui, &tr!("calendar-region"));
            let mut brazil = calendar.region == Some(Region::Brazil);
            if ui.checkbox(&mut brazil, tr!("calendar-brazil")).on_hover_text(tr!("calendar-brazil-tip")).clicked() {
                edit(json!({"region": if brazil { json!("brazil") } else { json!(null) }}));
            }

            // The days off and the days worked besides the week, each with a field to add one.
            for (title, list, field, add, remove, id) in [
                (tr!("calendar-holidays"), &calendar.holidays, &mut form.holiday, "holidays_add", "holidays_remove", "calendar-holiday"),
                (tr!("calendar-worked"), &calendar.extra_days, &mut form.worked, "worked_add", "worked_remove", "calendar-worked"),
            ] {
                theme::section(ui, &title);
                if list.is_empty() {
                    theme::hint(ui, &tr!("detail-none"));
                }
                for d in list {
                    if theme::list_item(ui, None, &show_date(*d), "", &tr!("calendar-remove-day")) {
                        edit(json!({remove: [d.to_string()]}));
                    }
                }
                theme::row(ui, &tr!("notes-date"), |ui, label| {
                    if let Some(d) = date_field(ui, egui::Id::unique(id), label, *field, 96.0) {
                        *field = d;
                    }
                    if ui.add_enabled(field.is_some(), egui::Button::new(tr!("notes-add"))).clicked()
                        && let Some(d) = field.take()
                    {
                        edit(json!({add: [d.to_string()]}));
                    }
                });
            }
        });
        // A refusal (no working day left, periods that overlap) says why in the status bar.
        if let Some(params) = pending {
            self.run("calendar.edit", params);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_are_written_and_read_back() {
        assert_eq!(periods_text(&[(480, 720), (780, 1020)]), "08:00–12:00, 13:00–17:00");
        assert_eq!(read_periods("08:00–12:00, 13:00–17:00"), Some(vec![(480, 720), (780, 1020)]));
        assert_eq!(read_periods("8-12,13 - 17:30"), Some(vec![(480, 720), (780, 1050)]));
        assert_eq!(read_periods("22:00-24:00"), Some(vec![(1320, 1440)]));
        for bad in ["8", "8-", "a-b", "25:00-26:00", "08:60-09:00"] {
            assert_eq!(read_periods(bad), None, "{bad}");
        }
    }
}
