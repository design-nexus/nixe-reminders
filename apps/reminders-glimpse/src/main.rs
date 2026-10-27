//! Glimpse plugin for Reminders. It reads `tasks.sqlite` (written by
//! nyx-remindersd; Glimpse mounts it read-only into the sandbox) and never
//! reaches the task lists themselves.
//!
//! `reminders` lists what's open (soonest due first), `today` what's due today
//! or overdue, `flagged` the flagged ones; other words search titles and notes.
//! `remind me to …` offers to add a reminder. Results act through the host:
//!
//! `{"api_version": 1, "app": "reminders", "op": "open", "list": "UID"}`
//! `{"api_version": 1, "app": "reminders", "op": "add", "text": "Call Sam tomorrow at 9"}`
//!
//! The sandbox has no time zone, so "today" uses the offset remindersd
//! recorded (`meta.utc_offset`).

use anyhow::{Context, Result, bail};
use glimpse_plugin_sdk::{Action, Hit, Request, serve};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::{Value, json};
use std::path::PathBuf;

fn main() {
    if let Err(error) = serve(&["search", "preview"], answer) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

fn database() -> PathBuf {
    if let Some(path) = std::env::var_os("NYX_REMINDERS_DB") {
        return PathBuf::from(path);
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    data.unwrap_or_else(|| PathBuf::from(".local/share")).join("nyx-reminders/tasks.sqlite")
}

fn open() -> Result<Connection> {
    let path = database();
    if !path.exists() {
        bail!("Reminders hasn't written its list yet.");
    }
    Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .context("the reminders couldn't be read")
}

#[derive(Debug, PartialEq, Eq)]
enum Asked {
    Open,
    Today,
    Flagged,
    Search(String),
    Add(String),
}

fn asked(query: &str) -> Option<Asked> {
    let q = query.trim();
    let lower = q.to_lowercase();
    for prefix in ["remind me to ", "remind me ", "reminder "] {
        if lower.starts_with(prefix) && q.len() > prefix.len() {
            return Some(Asked::Add(q[prefix.len()..].trim().to_string()));
        }
    }
    match lower.as_str() {
        "reminders" | "todo" | "to do" | "to-do" => Some(Asked::Open),
        "today" | "due" | "overdue" | "reminders today" => Some(Asked::Today),
        "flagged" | "reminders flagged" => Some(Asked::Flagged),
        _ => {
            let words = lower.strip_prefix("reminders ").unwrap_or(&lower).trim();
            (words.chars().count() >= 3).then(|| Asked::Search(words.to_string()))
        }
    }
}

#[derive(Clone, Debug)]
struct Task {
    list: String,
    list_name: String,
    uid: String,
    title: String,
    notes: String,
    due_day: Option<String>,
    due_at: Option<i64>,
    flagged: bool,
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn offset(db: &Connection) -> i64 {
    db.query_row("SELECT value FROM meta WHERE key = 'utc_offset'", [], |r| r.get::<_, String>(0))
        .optional()
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Days since 1970-01-01 → YYYY-MM-DD.
fn ymd(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
}

fn tasks(db: &Connection, filter: &str, arg: Option<&str>, limit: usize) -> Result<Vec<Task>> {
    let sql = format!(
        "SELECT list, list_name, uid, title, notes, due_day, due_at, flagged FROM tasks WHERE {filter}
         ORDER BY due_day IS NULL, due_day, due_at, title COLLATE NOCASE LIMIT ?2"
    );
    let mut stmt = db.prepare(&sql)?;
    let rows = stmt.query_map(params![arg, limit as i64], |r| {
        Ok(Task {
            list: r.get(0)?,
            list_name: r.get(1)?,
            uid: r.get(2)?,
            title: r.get(3)?,
            notes: r.get(4)?,
            due_day: r.get(5)?,
            due_at: r.get(6)?,
            flagged: r.get::<_, i64>(7)? != 0,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn due_text(t: &Task, today: &str, offset: i64) -> String {
    let Some(day) = &t.due_day else { return String::new() };
    let time = t.due_at.map(|at| {
        let secs = (at + offset).rem_euclid(86_400);
        format!(" {:02}:{:02}", secs / 3600, secs % 3600 / 60)
    });
    let word = if day.as_str() < today {
        format!("Overdue · {day}")
    } else if day == today {
        "Today".to_string()
    } else {
        day.clone()
    };
    format!("{word}{}", time.unwrap_or_default())
}

fn hits(db: &Connection, what: Asked, now: i64, limit: usize) -> Result<Vec<Hit>> {
    let offset = offset(db);
    let today = ymd((now + offset).div_euclid(86_400));
    let found = match what {
        Asked::Add(text) => {
            return Ok(vec![Hit {
                id: "add".into(),
                title: format!("Add reminder: {text}"),
                subtitle: "To the list new reminders go to; a time in the words sets when".into(),
                action: Action::Activate {
                    token: json!({"api_version": 1, "app": "reminders", "op": "add", "text": text}).to_string(),
                },
            }]);
        }
        Asked::Open => tasks(db, "parent IS NULL AND ?1 IS NULL", None, limit)?,
        Asked::Today => tasks(db, "due_day <= ?1", Some(&today), limit)?,
        Asked::Flagged => tasks(db, "flagged = 1 AND ?1 IS NULL", None, limit)?,
        Asked::Search(words) => {
            let pattern = format!("%{}%", words.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
            tasks(db, "(title LIKE ?1 ESCAPE '\\' OR notes LIKE ?1 ESCAPE '\\')", Some(&pattern), limit)?
        }
    };
    Ok(found
        .iter()
        .map(|t| {
            let due = due_text(t, &today, offset);
            let mut subtitle = t.list_name.clone();
            if !due.is_empty() {
                subtitle = format!("{due} · {subtitle}");
            }
            Hit {
                id: format!("{}\u{1f}{}", t.list, t.uid),
                title: if t.flagged { format!("★ {}", t.title) } else { t.title.clone() },
                subtitle,
                action: Action::Activate {
                    token: json!({"api_version": 1, "app": "reminders", "op": "open", "list": t.list}).to_string(),
                },
            }
        })
        .collect())
}

fn answer(request: &Request) -> Result<Value> {
    match request.op.as_str() {
        "search" => {
            let query = request.args["query"].as_str().unwrap_or("");
            let limit = request.args["limit"].as_i64().unwrap_or(10).clamp(1, 10) as usize;
            let Some(what) = asked(query) else { return Ok(json!([])) };
            // Adding needs nothing from the database.
            if let Asked::Add(_) = what {
                let db = Connection::open_in_memory()?;
                return Ok(serde_json::to_value(hits(&db, what, now(), limit)?)?);
            }
            let db = open()?;
            Ok(serde_json::to_value(hits(&db, what, now(), limit)?)?)
        }
        "preview" => {
            let id = request.args["result_id"].as_str().unwrap_or("");
            let Some((list, uid)) = id.split_once('\u{1f}') else { bail!("nothing to show") };
            let db = open()?;
            let offset = offset(&db);
            let today = ymd((now() + offset).div_euclid(86_400));
            let found = tasks(&db, "list || char(31) || uid = ?1", Some(&format!("{list}\u{1f}{uid}")), 1)?;
            let Some(t) = found.first() else { bail!("that reminder is done or gone") };
            let mut lines = vec![t.title.clone(), t.list_name.clone()];
            let due = due_text(t, &today, offset);
            if !due.is_empty() {
                lines.push(due);
            }
            if !t.notes.is_empty() {
                lines.push(String::new());
                lines.push(t.notes.chars().take(1500).collect());
            }
            Ok(json!({ "text": lines.join("\n") }))
        }
        other => bail!("unsupported operation {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_was_asked() {
        assert_eq!(asked("Remind me to call Sam tomorrow at 9"), Some(Asked::Add("call Sam tomorrow at 9".into())));
        assert_eq!(asked("reminders"), Some(Asked::Open));
        assert_eq!(asked("today"), Some(Asked::Today));
        assert_eq!(asked("reminders milk"), Some(Asked::Search("milk".into())));
        assert_eq!(asked("mi"), None);
        assert_eq!(ymd(20_747), "2026-10-21");
    }

    #[test]
    fn task_queries() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE tasks (list TEXT, list_name TEXT, uid TEXT, title TEXT, notes TEXT, parent TEXT, due_day TEXT,
               due_at INTEGER, overdue INTEGER, flagged INTEGER, priority INTEGER);
             CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);
             INSERT INTO meta VALUES ('utc_offset', '-14400');
             INSERT INTO tasks VALUES ('l', 'Home', 'a', 'Buy milk', '2%', NULL, '2026-10-20', NULL, 1, 0, NULL);
             INSERT INTO tasks VALUES ('l', 'Home', 'b', 'Call Sam', '', NULL, '2026-10-21', 1792591200, 0, 1, NULL);
             INSERT INTO tasks VALUES ('l', 'Home', 'c', 'Someday: learn Rust', '', NULL, NULL, NULL, 0, 0, NULL);
             INSERT INTO tasks VALUES ('l', 'Home', 'd', 'Oat milk', '', 'a', '2026-10-25', NULL, 0, 0, NULL);",
        )
        .unwrap();
        // 2026-10-21 09:00 local (UTC-4).
        let now = 20_747 * 86_400 + 13 * 3600;
        let today = hits(&db, Asked::Today, now, 10).unwrap();
        assert_eq!(today.iter().map(|h| h.title.as_str()).collect::<Vec<_>>(), ["Buy milk", "★ Call Sam"]);
        assert_eq!(today[0].subtitle, "Overdue · 2026-10-20 · Home");
        assert_eq!(today[1].subtitle, "Today 10:00 · Home");
        assert_eq!(hits(&db, Asked::Open, now, 10).unwrap().len(), 3, "subtasks only under their parent");
        assert_eq!(hits(&db, Asked::Search("milk".into()), now, 10).unwrap().len(), 2);
        assert_eq!(hits(&db, Asked::Search("2%".into()), now, 10).unwrap().len(), 1, "wildcards are literal");
        assert_eq!(hits(&db, Asked::Flagged, now, 10).unwrap()[0].title, "★ Call Sam");
        let add = hits(&db, Asked::Add("water plants".into()), now, 10).unwrap();
        let Action::Activate { token } = &add[0].action else { panic!() };
        assert!(token.contains("\"op\":\"add\"") && token.contains("water plants"));
    }
}
