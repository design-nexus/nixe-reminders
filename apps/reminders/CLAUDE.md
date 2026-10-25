# Reminders — notes for working on this app

- GTK4 (`gtk4` 0.11, `v4_18`) + Rust, no libadwaita. Music's sidebar (smart lists with
  counts, then task lists by account), a list page with a quick-add line, and a detail
  panel at the right in the Inspector's place (`detail.rs`, 300 px, `theme_surface`).
- Tasks are EDS task lists (VTODO). `worker.rs` owns `nixe_eds::Registry` and every open
  list on one thread (own GLib main context; a `#t` view per list) and sends
  `worker::Update` to `window::apply`. Changes go through `actions.rs` → `worker::ask`.
  Never call EDS from the GTK thread.
- Lists' capabilities (`Caps`): Google Tasks keeps dates only, no alerts, repeats or
  priority; EDS's `task-date-only` / `task-no-alarm` are honoured too. The detail panel
  and quick add only offer what a list keeps, and say what they dropped.
- `crates/nixe-reminders-core`: `Task`, `Smart` lists, sorting, the subtask `tree`, and
  `Sidecar` (`~/.local/share/nixe-reminders/reminders.sqlite`: flagged, manual position,
  `source` link such as a Mail message). Rows of deleted tasks are pruned only for lists
  that loaded.
- Completing goes through `Calendar::complete`: a repeating task moves to its next due
  date. Complete and delete have Undo (an export of the task is put back).
- `apps/remindersd`: GTK-free user service. Task-list alerts → notifications with
  Complete, Snooze 10 min, Dismiss (calendar alerts are Nixe Calendar's); quiet while
  `evolution-alarm-notify` runs. Writes `tasks.sqlite` (every open task; schema in
  `tasks.rs`, a contract for Glimpse). The installer enables it without starting it;
  the app starts it on open. End to end: `tools/test-remindersd.sh`.
- Control tools (`control.rs`): `reminders.lists/today/list/search` read;
  `reminders.create` (with `source` for Mail's "Remind me"), `complete`, `edit`,
  `delete` change. Assistant needs them registered and the changing ones gated.
- Right-click menus (`menus.rs`, `widgets::popup_menu`; Menu key or Shift+F10 for the
  selected reminder): reminders (offering only what the list keeps) and task lists.
  Turning a list off is EDS's `enabled` (`Registry::set_enabled`); Settings → Task lists
  lists them all. New reminders' alerts follow prefs `timed_alert` and `day_alerts`.
- Checks: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`.
  Layout: `REMINDERS_SNAPSHOT=/tmp/r.png REMINDERS_SNAPSHOT_PAGE=all REMINDERS_SNAPSHOT_SELECT=1`
  under scratch `XDG_CONFIG_HOME`/`XDG_STATE_HOME`/`XDG_DATA_HOME`.
- Shared crates come from nixe-core on GitHub, pinned by `rev` (Cargo.toml and
  Cargo.lock). To build against unpushed nixe-core changes, push them first and bump the
  rev; never commit a `file://` source (CI refuses it, and the one-line install can't fetch it).
