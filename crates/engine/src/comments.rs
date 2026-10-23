//! Comments on activities, in threads: a comment opens one, replies answer it, and it is resolved
//! once dealt with. Each change is a command with undo. The engine reads no clock and knows no
//! one: the caller says who writes and when.

use mkperth_core::Comment;
use mkperth_i18n::tr;
use serde_json::Value;

use crate::{EngineError, Session};

/// The most comments a project may hold.
const MAX_COMMENTS: usize = 100_000;
/// The longest a comment may be, and an author or a time, in characters.
const MAX_TEXT: usize = 10_000;
const MAX_SHORT: usize = 200;

fn bad(command: &str, param: &str) -> EngineError {
    EngineError::Value(command.into(), param.into())
}

fn text(params: &Value, command: &str, name: &str, max: usize) -> Result<Option<String>, EngineError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.chars().count() <= max => Ok(Some(s.trim().to_string())),
        Some(_) => Err(bad(command, name)),
    }
}

fn required(params: &Value, command: &str, name: &str, max: usize) -> Result<String, EngineError> {
    text(params, command, name, max)?.filter(|s| !s.is_empty()).ok_or_else(|| EngineError::Param(command.into(), name.into()))
}

/// The comment `id`'s place in the list, and the thread it is in (the comment that opened it).
fn find(comments: &[Comment], id: &str) -> Result<(usize, String), EngineError> {
    let k = comments.iter().position(|c| c.id == id).ok_or_else(|| EngineError::UnknownComment(id.to_string()))?;
    let root = comments.get(k).map(|c| c.reply_to.clone().unwrap_or_else(|| c.id.clone())).unwrap_or_default();
    Ok((k, root))
}

/// `C` and the first number after the ones taken.
fn next_id(comments: &[Comment]) -> String {
    let highest = comments.iter().filter_map(|c| c.id.strip_prefix('C').and_then(|n| n.parse::<u64>().ok())).max().unwrap_or(0);
    format!("C{}", highest.saturating_add(1))
}

impl Session {
    /// The comment commands; `None` when `id` isn't one of them.
    pub(crate) fn comment_command(&mut self, id: &str, params: &Value) -> Option<Result<(), EngineError>> {
        Some(match id {
            "comment.add" => self.add_comment(params, false),
            "comment.reply" => self.add_comment(params, true),
            "comment.edit" => self.edit_comment(params),
            "comment.resolve" => self.resolve_comment(params),
            "comment.delete" => self.delete_comment(params),
            _ => return None,
        })
    }

    /// A comment on `activity` (`comment.add`), or a reply in the thread of comment `id`
    /// (`comment.reply`): its `text`, its `author` and the time it was written (`at`).
    fn add_comment(&mut self, params: &Value, reply: bool) -> Result<(), EngineError> {
        let c: &str = if reply { "comment.reply" } else { "comment.add" };
        let body = required(params, c, "text", MAX_TEXT)?;
        let author = text(params, c, "author", MAX_SHORT)?.unwrap_or_default();
        let at = text(params, c, "at", MAX_SHORT)?.unwrap_or_default();
        let (activity, reply_to) = if reply {
            let id = required(params, c, "id", MAX_SHORT)?;
            let (k, root) = find(&self.project.comments, &id)?;
            (self.project.comments.get(k).map(|x| x.activity.clone()).unwrap_or_default(), Some(root))
        } else {
            (required(params, c, "activity", MAX_SHORT)?, None)
        };
        let label =
            if reply { tr!("undo-reply-comment", activity = activity.as_str()) } else { tr!("undo-add-comment", activity = activity.as_str()) };
        self.change(label, |p| {
            if p.comments.len() >= MAX_COMMENTS {
                return Err(bad(c, "text"));
            }
            if !p.activities.iter().any(|a| a.id == activity) {
                return Err(EngineError::UnknownActivity(activity.clone()));
            }
            // A reply reopens a resolved thread, as a spreadsheet's does.
            if let Some(root) = &reply_to
                && let Some(first) = p.comments.iter_mut().find(|x| x.id == *root)
            {
                first.resolved = false;
            }
            let id = next_id(&p.comments);
            p.comments.push(Comment { id, activity: activity.clone(), author, at, text: body, reply_to, resolved: false });
            Ok(())
        })
    }

    /// The comment `id`'s new `text`.
    fn edit_comment(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "comment.edit";
        let id = required(params, C, "id", MAX_SHORT)?;
        let body = required(params, C, "text", MAX_TEXT)?;
        let (k, _) = find(&self.project.comments, &id)?;
        let activity = self.project.comments.get(k).map(|x| x.activity.clone()).unwrap_or_default();
        self.change(tr!("undo-edit-comment", activity = activity.as_str()), |p| {
            if let Some(x) = p.comments.get_mut(k) {
                x.text = body;
            }
            Ok(())
        })
    }

    /// Resolves the thread of comment `id` (or reopens it, with `resolved: false`).
    fn resolve_comment(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "comment.resolve";
        let id = required(params, C, "id", MAX_SHORT)?;
        let resolved = match params.get("resolved") {
            None | Some(Value::Null) => true,
            Some(v) => v.as_bool().ok_or_else(|| bad(C, "resolved"))?,
        };
        let (_, root) = find(&self.project.comments, &id)?;
        let activity = self.project.comments.iter().find(|x| x.id == root).map(|x| x.activity.clone()).unwrap_or_default();
        let label = if resolved {
            tr!("undo-resolve-comment", activity = activity.as_str())
        } else {
            tr!("undo-reopen-comment", activity = activity.as_str())
        };
        self.change(label, |p| {
            if let Some(first) = p.comments.iter_mut().find(|x| x.id == root) {
                first.resolved = resolved;
            }
            Ok(())
        })
    }

    /// Deletes the comment `id`; the comment that opened a thread takes its replies with it.
    fn delete_comment(&mut self, params: &Value) -> Result<(), EngineError> {
        const C: &str = "comment.delete";
        let id = required(params, C, "id", MAX_SHORT)?;
        let (k, _) = find(&self.project.comments, &id)?;
        let activity = self.project.comments.get(k).map(|x| x.activity.clone()).unwrap_or_default();
        self.change(tr!("undo-delete-comment", activity = activity.as_str()), |p| {
            p.comments.retain(|x| x.id != id && x.reply_to.as_deref() != Some(id.as_str()));
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run(s: &mut Session, id: &str, params: Value) {
        if let Err(e) = s.run(id, &params) {
            panic!("{id} {params}: {e}");
        }
    }

    #[test]
    fn threads_of_comments() {
        let mut s = Session::default();
        run(&mut s, "comment.add", json!({"activity": "A10", "text": "Is the survey team booked?", "author": "Ana", "at": "2026-10-23 09:00"}));
        run(&mut s, "comment.reply", json!({"id": "C1", "text": "Yes, for the 3rd.", "author": "Bia", "at": "2026-10-23 09:30"}));
        // A reply to a reply joins the same thread.
        run(&mut s, "comment.reply", json!({"id": "C2", "text": "Thanks!", "author": "Ana"}));
        let c = &s.project.comments;
        assert_eq!(c.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["C1", "C2", "C3"]);
        assert_eq!((c[1].reply_to.as_deref(), c[2].reply_to.as_deref(), c[2].activity.as_str()), (Some("C1"), Some("C1"), "A10"));
        // Resolved from any comment in it; a reply reopens it.
        run(&mut s, "comment.resolve", json!({"id": "C3"}));
        assert!(s.project.comments[0].resolved);
        run(&mut s, "comment.reply", json!({"id": "C1", "text": "One more thing."}));
        assert!(!s.project.comments[0].resolved);
        run(&mut s, "comment.edit", json!({"id": "C2", "text": "Yes, for the 4th."}));
        assert_eq!(s.project.comments[1].text, "Yes, for the 4th.");
        // A renamed activity keeps its comments; a deleted thread takes its replies.
        run(&mut s, "activity.edit", json!({"id": "A10", "new_id": "A11"}));
        assert!(s.project.comments.iter().all(|x| x.activity == "A11"));
        run(&mut s, "comment.add", json!({"activity": "A20", "text": "Late?"}));
        run(&mut s, "comment.delete", json!({"id": "C1"}));
        assert_eq!(s.project.comments.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), ["C5"]);
        run(&mut s, "edit.undo", Value::Null);
        assert_eq!(s.project.comments.len(), 5);
        // A deleted activity takes its comments.
        run(&mut s, "activity.delete", json!({"id": "A20"}));
        assert!(s.project.comments.iter().all(|x| x.activity == "A11"));
        // Refused: nothing said, no such activity or comment, a resolution that isn't yes or no.
        let before = s.project.clone();
        for (id, params) in [
            ("comment.add", json!({"activity": "A11", "text": "  "})),
            ("comment.add", json!({"activity": "ZZ", "text": "x"})),
            ("comment.reply", json!({"id": "C99", "text": "x"})),
            ("comment.resolve", json!({"id": "C1", "resolved": "maybe"})),
            ("comment.edit", json!({"id": "C1", "text": "x".repeat(MAX_TEXT + 1)})),
        ] {
            assert!(s.run(id, &params).is_err(), "{id} {params}");
            assert_eq!(s.project, before);
        }
    }
}
