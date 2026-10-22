//! Changes to tasks, run on the EDS worker: add, complete (with Undo), change,
//! delete (with Undo), and the parse of a quick-add line.

use crate::prefs;
use crate::window;
use crate::worker;
use chrono::{Local, NaiveDateTime, NaiveTime};
use nixe_eds::{Draft, Scope};
use nixe_reminders_core::{Caps, Due, Task};

/// What a quick-add line says, fitted to what the list can keep.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quick {
    pub title: String,
    pub due: Option<Due>,
    pub repeat: Option<String>,
    /// Parts the list can't keep (shown as a note).
    pub dropped: Vec<&'static str>,
}

pub fn parse_quick(text: &str, now: NaiveDateTime, caps: Caps, default_day: Option<chrono::NaiveDate>) -> Quick {
    let parse = nixe_when::parse(text, now);
    let mut dropped = Vec::new();
    let date = parse.date.or(default_day);
    let mut due = match (date, parse.start) {
        (Some(day), Some(time)) => Some(Due::At(day.and_time(time))),
        (Some(day), None) => Some(Due::Day(day)),
        (None, _) => None,
    };
    if caps.date_only
        && let Some(Due::At(t)) = due
    {
        due = Some(Due::Day(t.date()));
        dropped.push("time");
    }
    let mut repeat = parse.repeat.clone();
    if repeat.is_some() && !caps.repeat {
        repeat = None;
        dropped.push("repeat");
    }
    let title = if parse.title.trim().is_empty() { text.trim().to_string() } else { parse.title.trim().to_string() };
    Quick { title, due, repeat, dropped }
}

/// Adds a task. `parent` makes it a subtask.
pub fn add(
    list: String,
    quick: Quick,
    parent: Option<String>,
    notes: Option<String>,
    after: impl FnOnce(Result<String, String>) + 'static,
) {
    let alarms = quick.due.map(|due| match due {
        // A due time reminds at that time; a due day at the usual hour.
        Due::At(_) => vec![0],
        Due::Day(_) => vec![-(prefs::get().default_hour as i64 * 60)],
    });
    let caps = window::list_caps(&list);
    let draft = Draft {
        summary: Some(quick.title),
        due: quick.due.map(|d| Some(d.when())),
        repeat: quick.repeat.map(Some),
        parent: parent.map(Some),
        description: notes,
        alarms: alarms.filter(|_| caps.alerts),
        ..Draft::default()
    };
    worker::ask(move || worker::with_list(&list, |l| l.create(&draft)), after);
}

/// Marks a task done (a repeating one moves to its next due date) or not done.
pub fn set_done(task: &Task, done: bool) {
    let task = task.clone();
    let title = task.title.clone();
    let repeats = task.rule.is_some();
    worker::ask(
        move || {
            worker::with_list(&task.list, |l| {
                let backup = l.export(&task.uid)?;
                if done {
                    l.complete(&task.uid)?;
                } else {
                    l.modify(&task.uid, None, Scope::All, &Draft { done: Some(false), ..Draft::default() })?;
                }
                Ok((task.list.clone(), backup))
            })
        },
        move |result| match result {
            Ok((list, backup)) if done => {
                let message =
                    if repeats { format!("“{title}” moved to its next date.") } else { format!("Completed “{title}”.") };
                window::toast_undo(&message, move || restore(list.clone(), backup.clone()));
            }
            Ok(_) => {}
            Err(err) => window::toast(&err),
        },
    );
}

/// Applies changes to a task.
pub fn save(task: &Task, draft: Draft) {
    if draft == Draft::default() {
        return;
    }
    let task = task.clone();
    worker::ask(
        move || worker::with_list(&task.list, |l| l.modify(&task.uid, None, Scope::All, &draft)),
        |result| {
            if let Err(err) = result {
                window::toast(&err);
            }
        },
    );
}

/// Deletes a task and its subtasks, with Undo.
pub fn delete(task: &Task, subtasks: Vec<String>) {
    let task = task.clone();
    let title = task.title.clone();
    worker::ask(
        move || {
            worker::with_list(&task.list, |l| {
                let mut backups = Vec::new();
                for uid in subtasks.iter().chain(std::iter::once(&task.uid)) {
                    backups.push(l.export(uid)?);
                    l.remove(uid, None, Scope::All)?;
                }
                Ok((task.list.clone(), backups))
            })
        },
        move |result| match result {
            Ok((list, backups)) => window::toast_undo(&format!("Deleted “{title}”."), move || {
                for backup in backups.iter().rev() {
                    restore(list.clone(), backup.clone());
                }
            }),
            Err(err) => window::toast(&err),
        },
    );
}

fn restore(list: String, ics: String) {
    worker::ask(
        move || worker::with_list(&list, |l| l.import_all(&ics)),
        |result| {
            if let Err(err) = result {
                window::toast(&err);
            }
        },
    );
}

/// "Today", "Tomorrow 9 AM", "Fri", "Oct 30", "Oct 30, 2027".
pub fn due_text(due: Due, now: NaiveDateTime) -> String {
    let today = now.date();
    let day = due.day();
    let days = (day - today).num_days();
    let date = match days {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        -1 => "Yesterday".to_string(),
        2..=6 => day.format("%a").to_string(),
        _ if day.format("%Y").to_string() == today.format("%Y").to_string() => day.format("%b %-d").to_string(),
        _ => day.format("%b %-d, %Y").to_string(),
    };
    match due {
        Due::Day(_) => date,
        Due::At(t) => format!("{date} {}", time_text(t.time())),
    }
}

pub fn time_text(t: NaiveTime) -> String {
    if prefs::twelve_hour() {
        let h = chrono::Timelike::hour(&t) % 12;
        let h = if h == 0 { 12 } else { h };
        let m = chrono::Timelike::minute(&t);
        let m = if m == 0 { String::new() } else { format!(":{m:02}") };
        format!("{h}{m} {}", if chrono::Timelike::hour(&t) < 12 { "AM" } else { "PM" })
    } else {
        t.format("%H:%M").to_string()
    }
}

pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(day: u32, h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }

    #[test]
    fn quick_add_fits_the_list() {
        let full = Caps::default();
        let q = parse_quick("Call Sam tomorrow at 9", at(21, 10), full, None);
        assert_eq!((q.title.as_str(), q.due), ("Call Sam", Some(Due::At(at(22, 9)))));
        let google = Caps { date_only: true, alerts: false, repeat: false, priority: false };
        let q = parse_quick("Water plants every monday at 8", at(21, 10), google, None);
        assert_eq!(q.due, Some(Due::Day(NaiveDate::from_ymd_opt(2026, 10, 26).unwrap())));
        assert_eq!(q.repeat, None);
        assert_eq!(q.dropped, vec!["time", "repeat"]);
        // In Today, an undated line is due today.
        let q = parse_quick("Buy milk", at(21, 10), full, Some(at(21, 0).date()));
        assert_eq!(q.due, Some(Due::Day(at(21, 0).date())));
        assert_eq!(q.title, "Buy milk");
    }

    #[test]
    fn due_texts() {
        let now = at(21, 10);
        let d = |day| NaiveDate::from_ymd_opt(2026, 10, day).unwrap();
        assert_eq!(due_text(Due::Day(d(21)), now), "Today");
        assert_eq!(due_text(Due::Day(d(22)), now), "Tomorrow");
        assert_eq!(due_text(Due::Day(d(20)), now), "Yesterday");
        assert_eq!(due_text(Due::Day(d(23)), now), "Fri");
        assert_eq!(due_text(Due::Day(d(30)), now), "Oct 30");
        assert_eq!(due_text(Due::Day(NaiveDate::from_ymd_opt(2027, 1, 2).unwrap()), now), "Jan 2, 2027");
    }
}
