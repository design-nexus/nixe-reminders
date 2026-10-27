# Reminders

Your task lists from Google Tasks, Microsoft To Do, Nextcloud, Fastmail, iCloud and this
computer, in one place, for Omarchy. Part of the [Nyx](https://github.com/design-nexus/nyx) apps.

- Today, Scheduled, Flagged, All and Completed across every list
- Type “Call Sam tomorrow at 9” to add a reminder with its due date
- Due dates and times, alerts, repeats, priority, notes, links, subtasks and tags (`#bills` when adding)
- Drag reminders into your own order
- Alerts as desktop notifications, with Complete and Snooze
- A repeating reminder moves to its next date when you complete it
- Works offline from the saved copy and catches up when you're back online
- Follows your Omarchy theme; works from the keyboard and tiled at half-screen

Accounts are added in **Settings → Online Accounts**. Tasks are kept by Evolution Data
Server; each list keeps what its service supports (Google Tasks has dates but no times,
alerts, repeats or priority; Microsoft To Do has dates but no times or alerts). Microsoft 365
and Outlook.com lists need the `evolution-ews` package, or [`evolution-ews-backends`](https://github.com/design-nexus/nyx-core/tree/main/packaging/evolution-ews-backends),
which is the same without the Evolution app.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/design-nexus/nyx-reminders/main/install.sh | bash
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
