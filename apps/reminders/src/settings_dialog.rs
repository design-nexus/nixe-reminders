//! Settings as a card over the window.

use crate::settings_page;
use crate::widgets;
use crate::window;
use gtk::prelude::*;
use nyx_ui::settings_card::{self, About, Spec, Standard};

pub use nyx_ui::settings_card::fit;

const GROUPS: &[(&str, &str, &str)] = &[
    ("Lists", "view-list-symbolic", "Where new reminders go, completed tasks"),
    ("Alerts", "alarm-symbolic", "Alerts for new reminders"),
    ("Task lists", "view-list-bullet-symbolic", "Turn lists and accounts on or off"),
    ("Accounts", "system-users-symbolic", "Online accounts in Settings"),
];

const SPEC: Spec = Spec {
    groups: GROUPS,
    search_text: None,
    standard: Some(Standard {
        app: "reminders",
        about: About {
            name: "Reminders",
            icon: "io.github.design_nexus.nyx.Reminders",
            version: env!("CARGO_PKG_VERSION"),
            description: "Your task lists from every account in one place.",
            licence: "MIT",
            website: "https://github.com/design-nexus/nyx-reminders",
            issues: "https://github.com/design-nexus/nyx-reminders/issues",
            credits: &[],
        },
        shortcuts: window::SHORTCUTS,
        reduce_motion: || crate::prefs::get().reduce_motion,
        set_reduce_motion: |on| crate::prefs::update(|p| p.reduce_motion = on),
    }),
};

pub fn open() {
    let (Some(overlay), Some(window)) = (window::overlay(), window::window()) else { return };
    settings_card::open(&overlay, &window, SPEC, || {
        let page = widgets::page("settings");
        settings_page::build(&page);
        (page.root.upcast(), page.body)
    });
}
