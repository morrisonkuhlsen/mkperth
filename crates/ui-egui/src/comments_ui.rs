//! The Comments tab of Details: an activity's threads of comments, as a spreadsheet keeps them.
//! Each thread in a card (who wrote it and when, the text, its replies), answered, resolved,
//! reopened or deleted; a new comment typed under them. Every change is a `comment.*` command.

use mkperth_engine::core::Project;
use mkperth_engine::i18n::tr;
use serde_json::{Value, json};

use crate::theme::{self, Tokens};

/// What the Comments tab is typing: a new comment, and a reply to the thread it names.
#[derive(Debug, Clone, Default)]
pub struct CommentForm {
    pub text: String,
    pub reply_to: Option<String>,
    pub reply: String,
}

/// Draws the tab for `activity`; the command to run, if something was asked for. New comments
/// and replies are signed `author` at `at`.
pub(crate) fn comments_tab(
    ui: &mut egui::Ui,
    t: &Tokens,
    project: &Project,
    activity: &str,
    form: &mut CommentForm,
    author: &str,
    at: &str,
) -> Option<(&'static str, Value)> {
    let mut pending = None;
    let roots: Vec<_> = project.comments.iter().filter(|c| c.activity == activity && c.reply_to.is_none()).collect();
    if roots.is_empty() {
        ui.label(egui::RichText::new(tr!("comments-none")).size(12.0).color(t.text_dim));
    }
    let signed = |ui: &mut egui::Ui, who: &str, when: &str| {
        ui.horizontal(|ui| {
            let who = if who.is_empty() { tr!("comments-someone") } else { who.to_string() };
            ui.label(egui::RichText::new(who).size(12.5).color(t.text).strong());
            ui.label(egui::RichText::new(when).size(11.5).color(t.text_dim));
        });
    };
    for root in roots {
        let frame = egui::Frame::new()
            .fill(if root.resolved { t.header_bg } else { t.paper })
            .stroke(egui::Stroke::new(1.0, if root.resolved { t.border } else { crate::splash::BRAND }))
            .corner_radius(0.0)
            .inner_margin(egui::Margin::same(10));
        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            if root.resolved {
                ui.set_opacity(0.6);
                ui.label(egui::RichText::new(tr!("comments-resolved")).size(11.0).color(t.green));
            }
            signed(ui, &root.author, &root.at);
            ui.label(egui::RichText::new(&root.text).size(12.5).color(t.text));
            for reply in project.comments.iter().filter(|c| c.reply_to.as_deref() == Some(root.id.as_str())) {
                ui.add_space(6.0);
                ui.indent(("reply", &reply.id), |ui| {
                    signed(ui, &reply.author, &reply.at);
                    ui.label(egui::RichText::new(&reply.text).size(12.5).color(t.text));
                    if theme::ghost(ui, &tr!("comments-delete-reply"), "", true) {
                        pending = Some(("comment.delete", json!({"id": reply.id})));
                    }
                });
            }
            ui.add_space(4.0);
            if form.reply_to.as_deref() == Some(root.id.as_str()) {
                let label = ui.label(egui::RichText::new(tr!("comments-reply-label")).size(11.5).color(t.text_dim)).id;
                ui.add(egui::TextEdit::multiline(&mut form.reply).desired_rows(2).desired_width(f32::INFINITY)).labelled_by(label);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!form.reply.trim().is_empty(), egui::Button::new(tr!("comments-send-reply"))).clicked() {
                        pending = Some(("comment.reply", json!({"id": root.id, "text": form.reply.trim(), "author": author, "at": at})));
                    }
                    if theme::ghost(ui, &tr!("comments-cancel"), "", true) {
                        form.reply_to = None;
                        form.reply.clear();
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    if theme::ghost(ui, &tr!("comments-reply"), "", true) {
                        form.reply_to = Some(root.id.clone());
                        form.reply.clear();
                    }
                    let (word, resolved) = if root.resolved { (tr!("comments-reopen"), false) } else { (tr!("comments-resolve"), true) };
                    if theme::ghost(ui, &word, "", true) {
                        pending = Some(("comment.resolve", json!({"id": root.id, "resolved": resolved})));
                    }
                    if theme::ghost(ui, &tr!("comments-delete"), &tr!("comments-delete-tip"), true) {
                        pending = Some(("comment.delete", json!({"id": root.id})));
                    }
                });
            }
        });
        ui.add_space(8.0);
    }
    ui.add_space(6.0);
    let label = ui.label(egui::RichText::new(tr!("comments-new")).size(12.0).color(t.text_dim)).id;
    ui.add(egui::TextEdit::multiline(&mut form.text).desired_rows(3).desired_width(f32::INFINITY).hint_text(tr!("comments-hint"))).labelled_by(label);
    if ui.add_enabled(!form.text.trim().is_empty(), egui::Button::new(tr!("comments-send"))).clicked() {
        pending = Some(("comment.add", json!({"activity": activity, "text": form.text.trim(), "author": author, "at": at})));
    }
    pending
}
