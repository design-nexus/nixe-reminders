//! The settings card's groups.

use crate::prefs;
use crate::theme;
use crate::widgets::{self, Page};
use crate::window;
use crate::worker;
use gtk::prelude::*;

pub fn build(page: &Page) {
    window_group(page);
    lists(page);
    alerts(page);
    task_lists(page);
    accounts(page);
    keyboard(page);
}

fn window_group(page: &Page) {
    let group = page.group("Reminders window");
    group.add(&nixe_ui::widgets::start_row());
    let prefs = prefs::get();
    let options: Vec<(String, String)> = theme::all().iter().map(|t| (t.id.clone(), t.name.clone())).collect();
    let (theme_row, theme_dd) =
        widgets::choice_row("Theme", "A bundled theme, or one of your own.", options, &prefs.theme, |id| {
            prefs::update(|p| {
                p.theme = id;
                p.mode = prefs::ThemeMode::Theme;
            });
            theme::apply();
        });
    theme_dd.set_sensitive(prefs.mode == prefs::ThemeMode::Theme || !theme::omarchy_available());
    if theme::omarchy_available() {
        let dd = theme_dd.clone();
        let (row, _) = widgets::switch_row(
            "Follow Omarchy theme",
            "Match the desktop's colors and update when the Omarchy theme changes.",
            prefs.mode == prefs::ThemeMode::Omarchy,
            move |on| {
                prefs::update(|p| p.mode = if on { prefs::ThemeMode::Omarchy } else { prefs::ThemeMode::Theme });
                dd.set_sensitive(!on);
                theme::apply();
            },
        );
        group.add(&row);
    }
    group.add(&theme_row);
    let (row, _) = widgets::switch_row("Glow", "Soft accent glow around focused and selected elements.", prefs.glow, |on| {
        prefs::update(|p| p.glow = on);
        theme::apply();
    });
    group.add(&row);
    let (row, _) = widgets::switch_row("Reduce motion", "Turn off transitions in this window.", prefs.reduce_motion, |on| {
        prefs::update(|p| p.reduce_motion = on);
        theme::apply();
    });
    group.add(&row);
}

fn lists(page: &Page) {
    let group = page.group("Lists");
    let prefs = prefs::get();
    let lists: Vec<(String, String)> = window::lists().into_iter().filter(|l| !l.read_only).map(|l| (l.uid, l.name)).collect();
    if !lists.is_empty() {
        let (row, _) = widgets::choice_row(
            "New reminders go to",
            "The list for reminders added in Today, Scheduled and the other smart lists.",
            lists,
            &window::default_list(),
            |uid| prefs::update(|p| p.default_list = uid),
        );
        group.add(&row);
    }
    let (row, _) =
        widgets::switch_row("Show completed", "List done tasks under the open ones in each list.", prefs.show_completed, |on| {
            prefs::update(|p| p.show_completed = on);
            window::refresh_page();
        });
    group.add(&row);
    let hours: Vec<(String, String)> = (5..=12).map(|h| (h.to_string(), format!("{h}:00"))).collect();
    let (row, _) = widgets::choice_row(
        "Remind at",
        "The time an alert goes off for a reminder due on a day without a time.",
        hours,
        &prefs.default_hour.to_string(),
        |hour| prefs::update(|p| p.default_hour = hour.parse().unwrap_or(9)),
    );
    group.add(&row);
}

fn alerts(page: &Page) {
    let group = page.group("Alerts");
    group.note("For new reminders, in lists that keep alerts. Change one reminder's alert in its details.");
    let prefs = prefs::get();
    let mut timed = vec![("none".to_string(), "None".to_string())];
    timed.extend(
        [
            (0, "At the time"),
            (5, "5 minutes before"),
            (15, "15 minutes before"),
            (30, "30 minutes before"),
            (60, "1 hour before"),
        ]
        .iter()
        .map(|(m, l)| (m.to_string(), l.to_string())),
    );
    let (row, _) = widgets::choice_row(
        "Reminders with a time",
        "When the alert goes off for “call Sam at 3”.",
        timed,
        &prefs.timed_alert.map(|m| m.to_string()).unwrap_or_else(|| "none".into()),
        |m| prefs::update(|p| p.timed_alert = m.parse().ok()),
    );
    group.add(&row);
    let (row, _) = widgets::switch_row(
        "Reminders due on a day",
        "An alert at the Remind at time for reminders with a date but no time.",
        prefs.day_alerts,
        |on| prefs::update(|p| p.day_alerts = on),
    );
    group.add(&row);
}

/// Task lists switched on or off, by account (off: not loaded or synced, in
/// every app; switch on again here).
fn task_lists(page: &Page) {
    let group = page.group("Task lists");
    group.note("Turn lists off to stop loading and syncing them, here and in other apps.");
    fill_lists(&group.list);
}

fn fill_lists(list: &gtk::Box) {
    let list = list.clone();
    worker::all_lists(move |all| {
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let mut groups: Vec<(String, Vec<(nixe_eds::Source, bool)>)> = Vec::new();
        for (source, on) in all {
            let label = source.group.clone().unwrap_or_else(|| "On this computer".into());
            match groups.iter_mut().find(|(g, _)| *g == label) {
                Some((_, items)) => items.push((source, on)),
                None => groups.push((label, vec![(source, on)])),
            }
        }
        for (label, items) in groups {
            let uids: Vec<String> = items.iter().map(|(s, _)| s.uid.clone()).collect();
            let any_on = items.iter().any(|(_, on)| *on);
            let l = list.clone();
            let (row, _) = widgets::switch_row(&label, &format!("{} lists", items.len()), any_on, move |on| {
                let (count, done) = (uids.len(), std::rc::Rc::new(std::cell::Cell::new(0)));
                for uid in &uids {
                    let (l, done) = (l.clone(), done.clone());
                    worker::set_enabled(uid.clone(), on, move |r| {
                        if let Err(e) = r {
                            window::toast(&e);
                        }
                        done.set(done.get() + 1);
                        if done.get() == count {
                            fill_lists(&l);
                        }
                    });
                }
            });
            row.add_css_class("account-row");
            list.append(&row);
            for (source, on) in items {
                let (uid, l) = (source.uid.clone(), list.clone());
                let (row, _) = widgets::switch_row(source.label(), "", on, move |on| {
                    let l = l.clone();
                    worker::set_enabled(uid.clone(), on, move |r| {
                        if let Err(e) = r {
                            window::toast(&e);
                        }
                        fill_lists(&l);
                    });
                });
                row.add_css_class("nested-row");
                list.append(&row);
            }
        }
    });
}

fn accounts(page: &Page) {
    let group = page.group("Accounts");
    group.note("Task lists come from the accounts in Settings → Online Accounts, and from this computer.");
    let open = widgets::icon_button("go-next-symbolic", "Open Settings → Online Accounts");
    open.connect_clicked(|_| window::open_online_accounts());
    group.add(&widgets::row(
        "Online accounts",
        "Add, sign in to or remove Google, Nextcloud and other accounts with task lists.",
        Some(open.upcast_ref()),
    ));
}

fn keyboard(page: &Page) {
    let group = page.group("Keyboard");
    for (keys, what) in window::SHORTCUTS {
        group.add(&widgets::row(what, "", Some(widgets::key_caps(keys).upcast_ref())));
    }
}
