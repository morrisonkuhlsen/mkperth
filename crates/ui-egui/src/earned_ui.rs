//! The Earned value pane: how it is measured and estimated, where the project stands at the data
//! date (BAC, PV, EV, AC, the variances and indices, the estimates), the S-curve, and the
//! financial periods with their closes stored. Each change is a command.

use mkperth_engine::core::{Date, EtcTechnique, EvTechnique};
use mkperth_engine::i18n::tr;
use mkperth_engine::sched::{Earned, PeriodPoint};
use serde_json::{Value, json};

use crate::costs_ui::{money_text, read_money};
use crate::icons::Icon;
use crate::theme::{self, Tokens};
use crate::{PerthApp, icon_button, show_date};

/// The S-curve's series colours, light and dark: planned, earned, actual. Steps of the
/// reference categorical palette, checked for colour-blind separation on both surfaces.
fn series(dark: bool) -> [egui::Color32; 3] {
    let c = |r, g, b| egui::Color32::from_rgb(r, g, b);
    if dark {
        [c(0x39, 0x87, 0xE5), c(0x19, 0x9E, 0x70), c(0xD9, 0x59, 0x26)]
    } else {
        [c(0x2A, 0x78, 0xD6), c(0x1B, 0xAF, 0x7A), c(0xEB, 0x68, 0x34)]
    }
}

const TECHNIQUES: [EvTechnique; 6] =
    [EvTechnique::Physical, EvTechnique::Duration, EvTechnique::Units, EvTechnique::ZeroHundred, EvTechnique::FiftyFifty, EvTechnique::Custom(0)];

fn technique_name(t: EvTechnique) -> String {
    tr!(&format!("ev-technique-{}", t.code()))
}

fn etc_name(t: EtcTechnique) -> String {
    tr!(&format!("ev-etc-{}", t.code()))
}

/// An index to two places, in the language's decimal mark; a dash when it can't be worked out.
fn index_text(x: Option<f64>) -> String {
    x.map_or_else(|| "—".to_string(), |x| mkperth_engine::i18n::decimal(&format!("{x:.2}")))
}

/// What the S-curve draws.
struct Curve<'a> {
    planned: &'a [(Date, i64)],
    points: &'a [PeriodPoint],
    /// The data date and the figures then.
    now: Option<(Date, Earned)>,
    budget: i64,
    currency: &'a str,
}

/// The S-curve: the planned value summed day by day, the earned value and the actual cost at
/// each stored close and at the data date, the budget at completion as a faint line; a legend
/// above it, each line named at its end, the figures under the pointer.
fn s_curve(ui: &mut egui::Ui, t: &Tokens, c: &Curve) {
    let [plan_c, earn_c, act_c] = series(t.dark);
    // The legend: a mark for each, and its name.
    ui.horizontal(|ui| {
        for (color, name, square) in [(plan_c, tr!("ev-pv-short"), false), (earn_c, tr!("ev-ev-short"), false), (act_c, tr!("ev-ac-short"), true)] {
            let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            if square {
                ui.painter().rect_filled(r.shrink(1.0), 0.0, color);
            } else {
                ui.painter().circle_filled(r.center(), 4.0, color);
            }
            ui.label(egui::RichText::new(name).size(11.5).color(t.text_dim));
            ui.add_space(6.0);
        }
    });
    let dates = c.planned.iter().map(|(d, _)| *d).chain(c.points.iter().map(|p| p.finish)).chain(c.now.map(|(d, _)| d));
    let (Some(first), Some(last)) = (dates.clone().min(), dates.max()) else {
        theme::hint(ui, &tr!("ev-curve-empty"));
        return;
    };
    let top = c
        .planned
        .last()
        .map(|(_, v)| *v)
        .into_iter()
        .chain([c.budget])
        .chain(c.points.iter().flat_map(|p| [p.earned.unwrap_or(0), p.actual.unwrap_or(0)]))
        .chain(c.now.map(|(_, e)| e.actual.max(e.earned)))
        .max()
        .unwrap_or(1)
        .max(1);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 170.0), egui::Sense::hover());
    // Room on the right for the lines' names at their ends.
    let names = [tr!("ev-pv-short"), tr!("ev-ev-short"), tr!("ev-ac-short")];
    let room =
        names.iter().map(|n| ui.painter().layout_no_wrap(n.clone(), egui::FontId::proportional(10.5), t.text).size().x).fold(0.0f32, f32::max) + 10.0;
    let plot = egui::Rect::from_min_max(rect.min + egui::vec2(0.0, 14.0), rect.max - egui::vec2(room, 18.0));
    let painter = ui.painter_at(rect);
    let span = (last.days() - first.days()).max(1) as f32;
    let x = |d: Date| plot.left() + (d.days() - first.days()) as f32 / span * plot.width();
    let y = |v: i64| plot.bottom() - v as f32 / top as f32 * plot.height();
    // The axes, recessive: the baseline, and the budget at completion faint across.
    painter.hline(plot.x_range(), plot.bottom(), egui::Stroke::new(1.0, t.border));
    if c.budget > 0 {
        painter.hline(plot.x_range(), y(c.budget), egui::Stroke::new(1.0, t.border.gamma_multiply(0.7)));
        // Named at its left, clear of the lines' names at the right.
        painter.text(
            egui::pos2(plot.left(), y(c.budget) - 2.0),
            egui::Align2::LEFT_BOTTOM,
            format!("BAC {}", money_text(c.budget, c.currency)),
            egui::FontId::proportional(10.5),
            t.text_dim,
        );
    }
    painter.text(plot.left_bottom() + egui::vec2(0.0, 3.0), egui::Align2::LEFT_TOP, show_date(first), egui::FontId::proportional(10.5), t.text_dim);
    painter.text(plot.right_bottom() + egui::vec2(0.0, 3.0), egui::Align2::RIGHT_TOP, show_date(last), egui::FontId::proportional(10.5), t.text_dim);
    // The planned line, a step a day.
    let line: Vec<egui::Pos2> = c.planned.iter().map(|(d, v)| egui::pos2(x(*d), y(*v))).collect();
    if line.len() > 1 {
        painter.add(egui::Shape::line(line.clone(), egui::Stroke::new(2.0, plan_c)));
    }
    // The data date, dashed.
    if let Some((d, _)) = c.now {
        let at = x(d);
        painter.add(egui::Shape::dashed_line(
            &[egui::pos2(at, plot.top()), egui::pos2(at, plot.bottom())],
            egui::Stroke::new(1.0, t.text_dim),
            3.0,
            3.0,
        ));
    }
    // Earned and actual: the stored closes, then the data date.
    // A close stored for the period the data date is in is the data date's point: drawn there.
    let closed: Vec<&PeriodPoint> = c.points.iter().filter(|p| c.now.is_none_or(|(d, _)| p.finish < d)).collect();
    let mut earned: Vec<(Date, i64)> = closed.iter().filter_map(|p| p.earned.map(|v| (p.finish, v))).collect();
    let mut actual: Vec<(Date, i64)> = closed.iter().filter_map(|p| p.actual.map(|v| (p.finish, v))).collect();
    if let Some((d, e)) = c.now {
        earned.push((d, e.earned));
        actual.push((d, e.actual));
    }
    for (list, color, square) in [(&earned, earn_c, false), (&actual, act_c, true)] {
        let pts: Vec<egui::Pos2> = list.iter().map(|(d, v)| egui::pos2(x(*d), y(*v))).collect();
        if pts.len() > 1 {
            painter.add(egui::Shape::line(pts.clone(), egui::Stroke::new(2.0, color)));
        }
        for p in &pts {
            // A ring of the surface around each mark keeps overlapping ones apart.
            if square {
                painter.rect_filled(egui::Rect::from_center_size(*p, egui::vec2(10.0, 10.0)), 0.0, t.paper);
                painter.rect_filled(egui::Rect::from_center_size(*p, egui::vec2(8.0, 8.0)), 0.0, color);
            } else {
                painter.circle_filled(*p, 5.0, t.paper);
                painter.circle_filled(*p, 4.0, color);
            }
        }
    }
    // Each line named at its end.
    let ends = [
        (line.last().copied(), tr!("ev-pv-short")),
        (earned.last().map(|(d, v)| egui::pos2(x(*d), y(*v))), tr!("ev-ev-short")),
        (actual.last().map(|(d, v)| egui::pos2(x(*d), y(*v))), tr!("ev-ac-short")),
    ];
    let mut used: Vec<f32> = Vec::new();
    for (end, name) in ends {
        let Some(p) = end else { continue };
        // Nudged apart when two ends meet.
        let mut at = p.y;
        while used.iter().any(|u| (u - at).abs() < 11.0) {
            at -= 11.0;
        }
        used.push(at);
        painter.text(egui::pos2(p.x + 6.0, at), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(10.5), t.text);
    }
    // Under the pointer: the day, and the figures by then.
    if let Some(pointer) = response.hover_pos().filter(|p| plot.x_range().contains(p.x)) {
        let day = Date::from_days(first.days() + ((pointer.x - plot.left()) / plot.width() * span).round() as i32).unwrap_or(first);
        painter.vline(pointer.x, plot.y_range(), egui::Stroke::new(1.0, t.text_dim.gamma_multiply(0.5)));
        let by = |list: &[(Date, i64)]| list.iter().take_while(|(d, _)| *d <= day).last().map(|(_, v)| *v);
        let mut lines = vec![show_date(day)];
        for (name, v) in [(tr!("ev-pv-short"), by(c.planned)), (tr!("ev-ev-short"), by(&earned)), (tr!("ev-ac-short"), by(&actual))] {
            if let Some(v) = v {
                lines.push(format!("{name}: {}", money_text(v, c.currency)));
            }
        }
        response.on_hover_text(lines.join("\n"));
    }
}

/// Where the project stands at `at`: the measures, then the S-curve.
fn measures(ui: &mut egui::Ui, t: &Tokens, p: &mkperth_engine::core::Project, s: &mkperth_engine::sched::Schedule, at: Date, points: &[PeriodPoint]) {
    let ev = &p.earned_value;
    let e = mkperth_engine::sched::project_earned(p, s, at);
    let money = |m: i64| money_text(m, &p.currency);
    let sign = |m: i64| if m < 0 { t.red } else { t.text };
    let index = |x: Option<f64>| if x.is_some_and(|x| x < 1.0) { t.red } else { t.text };
    theme::section(ui, &tr!("ev-now-at", date = show_date(at).as_str()));
    theme::fact(ui, &tr!("ev-bac"), &money(e.budget), t.text);
    theme::fact(ui, &tr!("ev-pv"), &money(e.planned), t.text);
    theme::fact(ui, &tr!("ev-ev"), &money(e.earned), t.text);
    theme::fact(ui, &tr!("ev-ac"), &money(e.actual), t.text);
    theme::fact(ui, &tr!("ev-cv"), &money(e.cost_variance()), sign(e.cost_variance()));
    theme::fact(ui, &tr!("ev-sv"), &money(e.schedule_variance()), sign(e.schedule_variance()));
    theme::fact(ui, &tr!("ev-cpi"), &index_text(e.cpi()), index(e.cpi()));
    theme::fact(ui, &tr!("ev-spi"), &index_text(e.spi()), index(e.spi()));
    theme::fact(ui, &tr!("ev-etc-value"), &money(e.etc(ev.etc)), t.text);
    theme::fact(ui, &tr!("ev-eac"), &money(e.eac(ev.etc)), t.text);
    theme::fact(ui, &tr!("ev-vac"), &money(e.vac(ev.etc)), sign(e.vac(ev.etc)));
    theme::fact(ui, &tr!("ev-tcpi"), &index_text(e.tcpi()), if e.tcpi().is_some_and(|x| x > 1.0) { t.red } else { t.text });

    theme::section(ui, &tr!("ev-curve"));
    let planned = mkperth_engine::sched::planned_curve(p, s);
    let curve = Curve { planned: &planned, points, now: p.data_date.map(|d| (d, e)), budget: e.budget, currency: &p.currency };
    s_curve(ui, t, &curve);
}

impl PerthApp {
    pub(crate) fn earned_pane(&mut self, ui: &mut egui::Ui) {
        let t = Tokens::get(ui.ctx());
        if theme::pane_header(ui, Icon::Frequencies, &tr!("pane-earned"), &tr!("pane-earned-tip"), |ui| {
            icon_button(ui, Icon::ChevronRight, &tr!("pane-close"))
        }) {
            self.side = None;
        }
        let p = &self.session.project;
        let ev = &p.earned_value;
        let mut pending: Vec<(&str, Value)> = Vec::new();
        let schedule = self.session.schedule.as_ref().ok();
        // Measured at the data date, else today.
        let at = p.data_date.or(self.options.today);
        egui::ScrollArea::vertical().id_salt("earned-pane").auto_shrink(false).show(ui, |ui| {
            // How it is measured and estimated.
            theme::section(ui, &tr!("ev-measure"));
            theme::row(ui, &tr!("ev-technique"), |ui, label| {
                let mut chosen = ev.technique;
                egui::ComboBox::from_id_salt("ev-technique")
                    .selected_text(technique_name(chosen))
                    .width(theme::fill(ui))
                    .truncate()
                    .show_ui(ui, |ui| {
                        for x in TECHNIQUES {
                            let on = std::mem::discriminant(&chosen) == std::mem::discriminant(&x);
                            if ui.selectable_label(on, technique_name(x)).clicked() {
                                chosen = x;
                            }
                        }
                    })
                    .response
                    .labelled_by(label);
                if std::mem::discriminant(&chosen) != std::mem::discriminant(&ev.technique) {
                    let share = if let EvTechnique::Custom(s) = ev.technique { s as f64 / 100.0 } else { 50.0 };
                    pending.push(("project.options", json!({"ev_technique": chosen.code(), "ev_share": share})));
                }
            });
            if let EvTechnique::Custom(share) = ev.technique {
                theme::row(ui, &tr!("ev-share"), |ui, label| {
                    let mut percent = share as f64 / 100.0;
                    let r = ui.add(egui::DragValue::new(&mut percent).range(0.0..=100.0).suffix(" %").speed(1.0)).labelled_by(label);
                    if r.drag_stopped() || (r.changed() && !r.dragged()) {
                        pending.push(("project.options", json!({"ev_technique": "custom", "ev_share": percent})));
                    }
                });
            }
            theme::row(ui, &tr!("ev-etc"), |ui, label| {
                let mut chosen = ev.etc;
                egui::ComboBox::from_id_salt("ev-etc")
                    .selected_text(etc_name(chosen))
                    .width(theme::fill(ui))
                    .truncate()
                    .show_ui(ui, |ui| {
                        for x in EtcTechnique::ALL {
                            ui.selectable_value(&mut chosen, x, etc_name(x));
                        }
                    })
                    .response
                    .labelled_by(label);
                if chosen != ev.etc {
                    pending.push(("project.options", json!({"etc_technique": chosen.code()})));
                }
            });

            let Some(s) = schedule else { return };
            let points = mkperth_engine::sched::period_points(p, s);
            match at {
                None => {
                    theme::section(ui, &tr!("ev-now"));
                    theme::hint(ui, &tr!("ev-no-date"));
                }
                Some(at) => measures(ui, &t, p, s, at, &points),
            }
            // The financial periods, each with whether its close is stored.
            theme::section(ui, &tr!("ev-periods"));
            if ev.periods.is_empty() {
                theme::hint(ui, &tr!("ev-periods-none"));
            }
            for q in &ev.periods {
                let stored = points.iter().any(|x| x.period == q.name && x.earned.is_some());
                let detail = format!(
                    "{} – {}{}",
                    show_date(q.start),
                    show_date(q.finish),
                    if stored { format!(" · {}", tr!("ev-stored")) } else { String::new() }
                );
                if theme::list_item(ui, None, &q.name, &detail, &tr!("ev-period-delete", name = q.name.as_str())) {
                    pending.push(("period.delete", json!({"name": q.name})));
                }
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                let (from, to) = (p.start, s.finish.max(p.data_date.unwrap_or(p.start)));
                if ui.button(tr!("ev-months")).on_hover_text(tr!("ev-months-tip")).clicked() {
                    pending.push(("period.generate", json!({"from": from.to_string(), "to": to.to_string(), "every": "month"})));
                }
                if ui.button(tr!("ev-weeks")).on_hover_text(tr!("ev-weeks-tip")).clicked() {
                    pending.push(("period.generate", json!({"from": from.to_string(), "to": to.to_string(), "every": "week"})));
                }
                let ready = p.data_date.is_some() && !ev.periods.is_empty();
                if ui.add_enabled(ready, egui::Button::new(tr!("ev-store"))).on_hover_text(tr!("ev-store-tip")).clicked() {
                    pending.push(("period.store", json!({})));
                }
            });
        });
        for (command, params) in pending {
            self.run(command, params);
        }
    }
}

/// Details' percent complete: the share of the work done, as judged, typed in place.
pub(crate) fn percent_field(ui: &mut egui::Ui, id: &str, percent: Option<i64>) -> Option<Value> {
    let mut out = None;
    theme::row(ui, &tr!("detail-percent-complete"), |ui, label| {
        let key = egui::Id::unique("percent-complete").with(id);
        let shown = percent.map_or_else(String::new, |p| {
            mkperth_engine::i18n::decimal(format!("{:.2}", p as f64 / 100.0).trim_end_matches('0').trim_end_matches('.'))
        });
        let mut text = ui.data(|d| d.get_temp::<(String, String)>(key)).filter(|(was, _)| *was == shown).map_or(shown.clone(), |(_, x)| x);
        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(60.0).hint_text("0")).labelled_by(label);
        ui.label(egui::RichText::new("%").size(12.5));
        if r.lost_focus() {
            ui.data_mut(|d| d.remove::<(String, String)>(key));
            if text != shown {
                out = Some(match text.trim() {
                    "" => json!({"percent_complete": null}),
                    typed => json!({"percent_complete": read_money(typed).unwrap_or(-1.0)}),
                });
            }
        } else if r.changed() {
            let typed = text.clone();
            ui.data_mut(|d| d.insert_temp(key, (shown, typed)));
        }
    });
    out
}
