# Reminders

Your task lists from Google Tasks, Nextcloud, Fastmail, iCloud and this computer, in one
place, for Omarchy. Part of the [Nixe](https://github.com/design-nexus/nixe) apps.

- Today, Scheduled, Flagged, All and Completed across every list
- Type “Call Sam tomorrow at 9” to add a reminder with its due date
- Due dates and times, alerts, repeats, priority, notes, links and subtasks
- Alerts as desktop notifications, with Complete and Snooze
- A repeating reminder moves to its next date when you complete it
- Works offline from the saved copy and catches up when you're back online
- Follows your Omarchy theme; works from the keyboard and tiled at half-screen

Accounts are added in **Settings → Online Accounts**. Tasks are kept by Evolution Data
Server; each list keeps what its service supports (Google Tasks has dates but no times,
alerts, repeats or priority).

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/design-nexus/nixe-reminders/main/install.sh | bash
```

## Keys

| Key | Action |
| --- | --- |
| N | New reminder |
| J / K | Next / previous |
| Space | Complete |
| S | Flag |
| 1–4 | Today, Scheduled, Flagged, All |
| Ctrl+F | Find |
| F1 | All shortcuts |
