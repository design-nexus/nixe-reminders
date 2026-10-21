//! `reminders.sqlite`: what a task list can't store, by (list uid, task uid).
//! Rows whose task is gone are pruned when every list has loaded.

use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Local extras for one task.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    pub flagged: bool,
    /// Manual order within lists; `None` sorts by due date.
    pub position: Option<i64>,
    /// A link to what the task came from (a Mail message, `nixe-mail:` URI).
    pub source: Option<String>,
}

pub struct Sidecar {
    db: Connection,
}

pub type Key = (String, String);

impl Sidecar {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let db = Connection::open(path)?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version < 1 {
            db.execute_batch(
                "CREATE TABLE IF NOT EXISTS meta (
                   list TEXT NOT NULL, uid TEXT NOT NULL,
                   flagged INTEGER NOT NULL DEFAULT 0, position INTEGER, source TEXT,
                   PRIMARY KEY (list, uid));
                 PRAGMA user_version = 1;",
            )?;
        }
        Ok(Self { db })
    }

    pub fn all(&self) -> rusqlite::Result<HashMap<Key, Meta>> {
        let mut stmt = self.db.prepare("SELECT list, uid, flagged, position, source FROM meta")?;
        let rows = stmt.query_map([], |r| {
            Ok(((r.get(0)?, r.get(1)?), Meta { flagged: r.get(2)?, position: r.get(3)?, source: r.get(4)? }))
        })?;
        rows.collect()
    }

    pub fn get(&self, key: &Key) -> rusqlite::Result<Meta> {
        Ok(self
            .db
            .query_row("SELECT flagged, position, source FROM meta WHERE list = ?1 AND uid = ?2", params![key.0, key.1], |r| {
                Ok(Meta { flagged: r.get(0)?, position: r.get(1)?, source: r.get(2)? })
            })
            .optional()?
            .unwrap_or_default())
    }

    pub fn set(&self, key: &Key, meta: &Meta) -> rusqlite::Result<()> {
        if *meta == Meta::default() {
            self.db.execute("DELETE FROM meta WHERE list = ?1 AND uid = ?2", params![key.0, key.1])?;
        } else {
            self.db.execute(
                "INSERT INTO meta (list, uid, flagged, position, source) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT (list, uid) DO UPDATE SET flagged = ?3, position = ?4, source = ?5",
                params![key.0, key.1, meta.flagged, meta.position, meta.source],
            )?;
        }
        Ok(())
    }

    /// Writes positions 0.. for `order` (a list after a drag).
    pub fn reorder(&mut self, order: &[Key]) -> rusqlite::Result<()> {
        let tx = self.db.transaction()?;
        for (position, key) in order.iter().enumerate() {
            tx.execute(
                "INSERT INTO meta (list, uid, position) VALUES (?1, ?2, ?3)
                 ON CONFLICT (list, uid) DO UPDATE SET position = ?3",
                params![key.0, key.1, position as i64],
            )?;
        }
        tx.commit()
    }

    /// Forgets rows for tasks that no longer exist in `lists` (the lists that
    /// loaded; others are left alone, so an offline list keeps its flags).
    pub fn prune(&self, lists: &HashSet<String>, existing: &HashSet<Key>) -> rusqlite::Result<usize> {
        let gone: Vec<Key> = self.all()?.into_keys().filter(|k| lists.contains(&k.0) && !existing.contains(k)).collect();
        for key in &gone {
            self.db.execute("DELETE FROM meta WHERE list = ?1 AND uid = ?2", params![key.0, key.1])?;
        }
        Ok(gone.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(list: &str, uid: &str) -> Key {
        (list.to_string(), uid.to_string())
    }

    #[test]
    fn flags_order_and_prune() {
        let dir = std::env::temp_dir().join(format!("nixe-reminders-sidecar-{}", std::process::id()));
        let path = dir.join("reminders.sqlite");
        let mut s = Sidecar::open(&path).unwrap();
        s.set(&key("l", "a"), &Meta { flagged: true, ..Meta::default() }).unwrap();
        s.set(&key("l", "b"), &Meta { source: Some("nixe-mail:1".into()), ..Meta::default() }).unwrap();
        s.reorder(&[key("l", "b"), key("l", "a"), key("m", "c")]).unwrap();
        let all = s.all().unwrap();
        assert!(all[&key("l", "a")].flagged);
        assert_eq!(all[&key("l", "a")].position, Some(1));
        assert_eq!(all[&key("l", "b")].source.as_deref(), Some("nixe-mail:1"));

        // Clearing everything removes the row.
        s.set(&key("m", "c"), &Meta::default()).unwrap();
        assert_eq!(s.get(&key("m", "c")).unwrap(), Meta::default());

        // "b" was deleted from list l; list m didn't load, so nothing there is touched.
        s.set(&key("m", "d"), &Meta { flagged: true, ..Meta::default() }).unwrap();
        let lists: HashSet<String> = ["l".to_string()].into();
        let existing: HashSet<Key> = [key("l", "a")].into();
        assert_eq!(s.prune(&lists, &existing).unwrap(), 1);
        let all = s.all().unwrap();
        assert!(all.contains_key(&key("l", "a")) && all.contains_key(&key("m", "d")) && !all.contains_key(&key("l", "b")));

        // Reopening keeps the data.
        drop(s);
        assert!(Sidecar::open(&path).unwrap().get(&key("l", "a")).unwrap().flagged);
        let _ = std::fs::remove_dir_all(dir);
    }
}
