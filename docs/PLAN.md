# Reminders plan

Nyx Reminders: lists of things to do, with due dates, alerts, repeats and subtasks,
kept in Evolution Data Server task lists. That covers Google Tasks, CalDAV task lists
(Nextcloud, Fastmail, iCloud) and lists on this computer. Shared foundation:
[nyx-core docs/pim](https://github.com/design-nexus/nyx-core/blob/main/docs/pim/ARCHITECTURE.md).

| | |
| --- | --- |
| Packages | `nyx-reminders` (UI), `nyx-remindersd` (due alerts), `nyx-reminders-glimpse` |
| App id | `io.github.design_nexus.nyx.Reminders` |
| Data | EDS task lists (VTODO); `~/.local/share/nyx-reminders/reminders.sqlite` (sidecar), `tasks.sqlite` (read-only snapshot of open tasks for Glimpse) |
| Config | `~/.config/nyx-reminders/settings.toml` |

## Phases

### 1. MVP (done)
- [x] Sidebar (Music construction): Today, Scheduled, Flagged, All, Completed, then task lists grouped by account
- [x] List page: check off with Undo, quick add with `nyx-when` ("call Sam tomorrow at 9")
- [x] Detail: due date and time, alert, repeat, priority, notes, URL, subtasks, flag
- [x] Controls a list's backend can't store are hidden for that list; quick add says what it dropped
- [x] `nyx-remindersd`: due alerts through `EReminderWatcher` (task lists only), notifications with Complete / Snooze / Dismiss, `tasks.sqlite`
- [x] Completing a repeating task moves it to its next due date
- [x] Search; offline (EDS queue); banners as in Calendar
- [x] Control tools for Assistant (and Mail's "Remind me" via `reminders.create` with `source`)
- [ ] Drag to reorder (the sidecar keeps positions; no drag yet)

### 2. Advanced
- [ ] Tags and saved filters
- [ ] "Remind me" from Mail with a link back to the message
- [ ] Drag to reorder
- [ ] Import from the Assistant reminders ability; Assistant hands `reminders.*` to this app

## Out of scope

Hosted backends; a second task store; editing EDS source files directly.
