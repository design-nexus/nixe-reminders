# Reminders testing

## Unit

- Smart list membership (today, overdue, scheduled) across time zones and midnight.
- Capability gating per backend.
- Sidecar cleanup when a task disappears.

## EDS (private session)

`dbus-run-session` with scratch XDG directories and EDS's local task list: VTODO round
trips for subtasks, repeat, alerts and completion; registry missing or slow.

## Live (only with Ken's go-ahead)

Read-only against the connected Google Tasks list; writes only to a scratch list.

## UI

Real-window screenshots at 1120 and 800 px, with the Omarchy theme copied into a
scratch `XDG_STATE_HOME`.

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```
