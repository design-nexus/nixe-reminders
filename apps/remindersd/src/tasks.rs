//! `tasks.sqlite`: every open task, for Glimpse and other readers that can't
//! reach EDS. Rewritten whole and swapped in atomically.
//!
//! Schema (`user_version` 1), a contract recorded in the suite manifest:
//!
//! ```sql
//! tasks(list, list_name, uid, title, notes, parent,
//!       due_day TEXT,      -- YYYY-MM-DD or NULL
//!       due_at INTEGER,    -- Unix seconds when the task has a due time, else NULL
//!       overdue INTEGER, flagged INTEGER, priority INTEGER)
//! meta(key PRIMARY KEY, value)   -- written_at (Unix seconds), utc_offset (local
//!                                  seconds east of UTC then; for sandboxed readers)
//! ```

use chrono::NaiveDateTime;
use nixe_reminders_core::{Due, ListInfo, Meta, Task, timestamp};
use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::Path;

pub fn write(
    path: &Path,
    lists: &[ListInfo],
    tasks: &[Task],
    meta: &HashMap<(String, String), Meta>,
    now: NaiveDateTime,
) -> rusqlite::Result<usize> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = path.with_extension("sqlite.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut count = 0;
    {
        let mut db = Connection::open(&tmp)?;
        db.execute_batch(
            "PRAGMA user_version = 1;
             CREATE TABLE tasks (
               list TEXT NOT NULL, list_name TEXT NOT NULL, uid TEXT NOT NULL, title TEXT NOT NULL,
               notes TEXT NOT NULL, parent TEXT, due_day TEXT, due_at INTEGER,
               overdue INTEGER NOT NULL, flagged INTEGER NOT NULL, priority INTEGER);
             CREATE INDEX tasks_due ON tasks(due_day);
             CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        let tx = db.transaction()?;
        {
            let mut insert = tx.prepare(
                "INSERT INTO tasks (list, list_name, uid, title, notes, parent, due_day, due_at, overdue, flagged, priority)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for t in tasks.iter().filter(|t| !t.done) {
                let list_name = lists.iter().find(|l| l.uid == t.list).map(|l| l.name.as_str()).unwrap_or("");
                let flagged = meta.get(&t.key()).is_some_and(|m| m.flagged);
                let due_at = match t.due {
                    Some(Due::At(at)) => Some(timestamp(at)),
                    _ => None,
                };
                insert.execute(params![
                    t.list,
                    list_name,
                    t.uid,
                    t.title,
                    t.notes,
                    t.parent,
                    t.due.map(|d| d.day().to_string()),
                    due_at,
                    t.overdue(now),
                    flagged,
                    t.priority,
                ])?;
                count += 1;
            }
            tx.execute("INSERT INTO meta (key, value) VALUES ('written_at', ?1)", params![timestamp(now).to_string()])?;
            let offset = chrono::Local::now().offset().local_minus_utc();
            tx.execute("INSERT INTO meta (key, value) VALUES ('utc_offset', ?1)", params![offset.to_string()])?;
        }
        tx.commit()?;
    }
    std::fs::rename(&tmp, path).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn task(uid: &str, due: Option<Due>, done: bool) -> Task {
        Task {
            list: "l".into(),
            uid: uid.into(),
            title: uid.into(),
            notes: String::new(),
            url: None,
            due,
            done,
            priority: None,
            parent: None,
            rule: None,
            alarms: Vec::new(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn writes_open_tasks() {
        let dir = std::env::temp_dir().join(format!("nixe-remindersd-test-{}", std::process::id()));
        let path = dir.join("tasks.sqlite");
        let d = |day| NaiveDate::from_ymd_opt(2026, 10, day).unwrap();
        let now = d(21).and_hms_opt(12, 0, 0).unwrap();
        let tasks = vec![
            task("late", Some(Due::Day(d(20))), false),
            task("later", Some(Due::At(d(22).and_hms_opt(9, 0, 0).unwrap())), false),
            task("done", None, true),
        ];
        let mut meta = HashMap::new();
        meta.insert(("l".to_string(), "later".to_string()), Meta { flagged: true, ..Meta::default() });
        assert_eq!(write(&path, &[], &tasks, &meta, now).unwrap(), 2);
        let db = Connection::open(&path).unwrap();
        let rows: Vec<(String, bool, bool, Option<i64>)> = db
            .prepare("SELECT uid, overdue, flagged, due_at FROM tasks ORDER BY uid")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows[0].0, "late");
        assert!(rows[0].1 && !rows[0].2 && rows[0].3.is_none());
        assert_eq!(rows[1].0, "later");
        assert!(!rows[1].1 && rows[1].2 && rows[1].3.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
