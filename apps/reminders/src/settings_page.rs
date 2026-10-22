//! The settings card's groups.

use crate::prefs;
use crate::theme;
use crate::widgets::{self, Page};
use crate::window;
use gtk::prelude::*;

pub fn build(page: &Page) {
    window_group(page);
    lists(page);
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
