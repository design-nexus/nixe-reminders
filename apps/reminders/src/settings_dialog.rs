//! Settings as a card over the window.

use crate::settings_page;
use crate::widgets;
use crate::window;
use gtk::prelude::*;
use nyx_ui::settings_card::{self, Spec};

pub use nyx_ui::settings_card::fit;

const GROUPS: &[(&str, &str, &str)] = &[
    ("Reminders window", "applications-graphics-symbolic", "Theme, glow, motion"),
    ("Lists", "view-list-symbolic", "Where new reminders go, completed tasks"),
    ("Alerts", "alarm-symbolic", "Alerts for new reminders"),
    ("Task lists", "view-list-bullet-symbolic", "Turn lists and accounts on or off"),
    ("Accounts", "system-users-symbolic", "Online accounts in Settings"),
    ("Keyboard", "input-keyboard-symbolic", "Shortcuts"),
];

const SPEC: Spec = Spec { groups: GROUPS, search_text: None };

pub fn open() {
    let (Some(overlay), Some(window)) = (window::overlay(), window::window()) else { return };
    settings_card::open(&overlay, &window, SPEC, || {
        let page = widgets::page("settings");
        settings_page::build(&page);
        (page.root.upcast(), page.body)
    });
}
