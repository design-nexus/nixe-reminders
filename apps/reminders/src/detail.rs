//! The selected task's details, in a panel at the right (the Inspector's
//! place). Every change saves at once; only fields the list can keep show.

use crate::actions::{self, Quick};
use crate::widgets;
use crate::window;
use chrono::{Datelike, NaiveDate, NaiveTime};
use gtk::prelude::*;
use nyx_eds::Draft;
use nyx_reminders_core::{Due, Level, Meta, Task};
use std::rc::Rc;

pub const WIDTH: i32 = 300;

const REPEATS: &[(&str, &str)] = &[
    ("", "Never"),
    ("FREQ=DAILY", "Every day"),
    ("FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", "Every weekday"),
    ("FREQ=WEEKLY", "Every week"),
    ("FREQ=WEEKLY;INTERVAL=2", "Every 2 weeks"),
    ("FREQ=MONTHLY", "Every month"),
    ("FREQ=YEARLY", "Every year"),
];

fn section(panel: &gtk::Box, title: &str) {
    let label = widgets::label(&title.to_uppercase(), "detail-heading");
    label.set_xalign(0.0);
    panel.append(&label);
}

fn parse_time(text: &str) -> Option<NaiveTime> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    nyx_when::parse(&format!("at {t}"), actions::now()).start
}

/// Builds the panel for `task` into `panel` (emptied first).
pub fn fill(panel: &gtk::Box, task: &Task, meta: &Meta, subtasks: &[Task]) {
    while let Some(child) = panel.first_child() {
        panel.remove(&child);
    }
    let caps = window::list_caps(&task.list);
    let read_only = window::lists().iter().find(|l| l.uid == task.list).is_none_or(|l| l.read_only);
    let task = Rc::new(task.clone());

    let title = gtk::Entry::new();
    title.add_css_class("detail-title");
    title.update_property(&[gtk::accessible::Property::Label("Title")]);
    title.set_text(&task.title);
    title.set_sensitive(!read_only);
    {
        let t = task.clone();
        let save = move |e: &gtk::Entry| {
            let text = e.text().trim().to_string();
            if !text.is_empty() && text != t.title {
                actions::save(&t, Draft { summary: Some(text), ..Draft::default() });
            }
        };
        let s = save.clone();
        title.connect_activate(move |e| s(e));
        let focus = gtk::EventControllerFocus::new();
        let e = title.clone();
        focus.connect_leave(move |_| save(&e));
        title.add_controller(focus);
    }
    panel.append(&title);

    let notes = gtk::TextView::new();
    notes.add_css_class("detail-notes");
    notes.update_property(&[gtk::accessible::Property::Label("Notes")]);
    notes.set_wrap_mode(gtk::WrapMode::WordChar);
    notes.buffer().set_text(&task.notes);
    notes.set_editable(!read_only);
    {
        let t = task.clone();
        let focus = gtk::EventControllerFocus::new();
        let n = notes.clone();
        focus.connect_leave(move |_| {
            let b = n.buffer();
            let text = b.text(&b.start_iter(), &b.end_iter(), false).to_string();
            if text != t.notes {
                actions::save(&t, Draft { description: Some(text), ..Draft::default() });
            }
        });
        notes.add_controller(focus);
    }
    let notes_scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::External)
        .min_content_height(90)
        .max_content_height(220)
        .propagate_natural_height(true)
        .child(&notes)
        .build();
    notes_scroll.add_css_class("detail-notes-box");
    panel.append(&notes_scroll);

    if read_only {
        let note = widgets::label("This list is read only.", "dim");
        note.set_xalign(0.0);
        panel.append(&note);
        return;
    }

    // Due
    section(panel, "Due");
    let due_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let day = task.due.map(|d| d.day()).unwrap_or_else(|| actions::now().date());
    let date_button = gtk::MenuButton::new();
    let this_year = actions::now().date().year() == day.year();
    let label = if task.due.is_none() {
        "No date".to_string()
    } else if this_year {
        day.format("%a, %b %-d").to_string()
    } else {
        day.format("%a, %b %-d, %Y").to_string()
    };
    date_button.set_label(&label);
    date_button.set_hexpand(true);
    let calendar = gtk::Calendar::new();
    if let Ok(dt) = glib::DateTime::from_local(day.year(), day.month() as i32, day.day() as i32, 0, 0, 0.0) {
        calendar.select_day(&dt);
    }
    let pop = gtk::Popover::new();
    let pop_box = widgets::vbox(6);
    pop_box.append(&calendar);
    let clear = gtk::Button::with_label("No date");
    pop_box.append(&clear);
    pop.set_child(Some(&pop_box));
    date_button.set_popover(Some(&pop));
    due_row.append(&date_button);
    let time = gtk::Entry::new();
    time.set_placeholder_text(Some("Time"));
    time.update_property(&[gtk::accessible::Property::Label("Due time")]);
    time.set_width_chars(8);
    if let Some(Due::At(t)) = task.due {
        time.set_text(&actions::time_text(t.time()));
    }
    time.set_visible(!caps.date_only && task.due.is_some());
    due_row.append(&time);
    panel.append(&due_row);
    {
        let (t, p, time_field) = (task.clone(), pop.clone(), time.clone());
        calendar.connect_day_selected(move |cal| {
            let picked = cal.date();
            let Some(day) = NaiveDate::from_ymd_opt(picked.year(), picked.month() as u32, picked.day_of_month() as u32) else {
                return;
            };
            let due = match parse_time(&time_field.text()).filter(|_| !caps.date_only) {
                Some(at) => Due::At(day.and_time(at)),
                None => Due::Day(day),
            };
            p.popdown();
            actions::save(&t, Draft { due: Some(Some(due.when())), ..Draft::default() });
        });
        let (t, p) = (task.clone(), pop.clone());
        clear.connect_clicked(move |_| {
            p.popdown();
            actions::save(&t, Draft { due: Some(None), alarms: Some(Vec::new()), ..Draft::default() });
        });
        let t = task.clone();
        let save_time = move |e: &gtk::Entry| {
            let Some(day) = t.due.map(|d| d.day()) else { return };
            let due = match parse_time(&e.text()) {
                Some(at) => Due::At(day.and_time(at)),
                None if e.text().trim().is_empty() => Due::Day(day),
                None => {
                    window::toast("Enter a time like 2:30 PM or 14:30.");
                    return;
                }
            };
            if Some(due) != t.due {
                actions::save(&t, Draft { due: Some(Some(due.when())), ..Draft::default() });
            }
        };
        let s = save_time.clone();
        time.connect_activate(move |e| s(e));
        let focus = gtk::EventControllerFocus::new();
        let e = time.clone();
        focus.connect_leave(move |_| save_time(&e));
        time.add_controller(focus);
    }

    if caps.repeat && task.due.is_some() {
        let options: Vec<(String, String)> = REPEATS.iter().map(|(r, l)| (r.to_string(), l.to_string())).collect();
        let current = task.rule.clone().unwrap_or_default();
        let mut options = options;
        if !current.is_empty() && !options.iter().any(|(r, _)| *r == current) {
            options.push((current.clone(), "Custom".into()));
        }
        let dd = widgets::dropdown(&options, &current);
        let t = task.clone();
        dd.connect_selected_notify(move |d| {
            let Some((rule, _)) = options.get(d.selected() as usize).cloned() else { return };
            if rule != t.rule.clone().unwrap_or_default() {
                actions::save(&t, Draft { repeat: Some(Some(rule).filter(|r| !r.is_empty())), ..Draft::default() });
            }
        });
        panel.append(&labeled("Repeat", &dd));
    }

    if caps.alerts && task.due.is_some() {
        let date_only = matches!(task.due, Some(Due::Day(_)));
        let mut options = vec![("none".to_string(), "No alert".to_string())];
        if date_only {
            for hour in [8i64, 9, 12, 17, 20] {
                options.push((
                    (-hour * 60).to_string(),
                    format!(
                        "On the day at {}",
                        actions::time_text(NaiveTime::from_hms_opt(hour as u32, 0, 0).unwrap_or_default())
                    ),
                ));
            }
        } else {
            for (m, l) in [(0i64, "At the due time"), (10, "10 min before"), (60, "1 hour before"), (1440, "1 day before")] {
                options.push((m.to_string(), l.to_string()));
            }
        }
        let current = task.alarms.first().map(|m| m.to_string()).unwrap_or_else(|| "none".into());
        if !options.iter().any(|(id, _)| *id == current) {
            options.push((current.clone(), "Custom".into()));
        }
        let dd = widgets::dropdown(&options, &current);
        let t = task.clone();
        dd.connect_selected_notify(move |d| {
            let Some((id, _)) = options.get(d.selected() as usize).cloned() else { return };
            let alarms: Vec<i64> = id.parse().ok().into_iter().collect();
            if alarms != t.alarms {
                actions::save(&t, Draft { alarms: Some(alarms), ..Draft::default() });
            }
        });
        panel.append(&labeled("Alert", &dd));
    }

    if caps.priority {
        section(panel, "Priority");
        let current = match task.priority_level() {
            Some(Level::High) => "high",
            Some(Level::Medium) => "medium",
            Some(Level::Low) => "low",
            None => "none",
        };
        let t = task.clone();
        let seg = widgets::segmented(
            &widgets::opts(&[("none", "None"), ("low", "Low"), ("medium", "Medium"), ("high", "High")]),
            current,
            move |id| {
                let value = match id.as_str() {
                    "high" => Some(Level::High.value()),
                    "medium" => Some(Level::Medium.value()),
                    "low" => Some(Level::Low.value()),
                    _ => None,
                };
                if value != t.priority {
                    actions::save(&t, Draft { priority: Some(value), ..Draft::default() });
                }
            },
        );
        panel.append(&seg);
    }

    section(panel, "More");
    let flag = gtk::Switch::new();
    flag.set_active(meta.flagged);
    flag.set_halign(gtk::Align::End);
    {
        let t = task.clone();
        flag.connect_active_notify(move |s| window::set_flag(&t, s.is_active()));
    }
    panel.append(&labeled("Flagged", &flag));
    let link = gtk::Entry::new();
    link.set_placeholder_text(Some("https://…"));
    link.set_text(task.url.as_deref().unwrap_or(""));
    {
        let t = task.clone();
        let save = move |e: &gtk::Entry| {
            let text = e.text().trim().to_string();
            if Some(text.as_str()) != t.url.as_deref().or(Some("")) {
                actions::save(&t, Draft { url: Some(Some(text).filter(|u| !u.is_empty())), ..Draft::default() });
            }
        };
        let s = save.clone();
        link.connect_activate(move |e| s(e));
        let focus = gtk::EventControllerFocus::new();
        let e = link.clone();
        focus.connect_leave(move |_| save(&e));
        link.add_controller(focus);
    }
    panel.append(&labeled("Link", &link));
    if caps.tags {
        let tags = gtk::Entry::new();
        tags.set_placeholder_text(Some("home, bills"));
        tags.set_text(&task.tags.join(", "));
        tags.set_tooltip_text(Some("Tags, separated by commas (also #tag when adding)"));
        let t = task.clone();
        let save = move |e: &gtk::Entry| {
            let mut picked: Vec<String> = Vec::new();
            for tag in e.text().split([',', ' ']).map(|t| t.trim().trim_start_matches('#')).filter(|t| !t.is_empty()) {
                if !picked.iter().any(|p| p.eq_ignore_ascii_case(tag)) {
                    picked.push(tag.to_string());
                }
            }
            if picked != t.tags {
                actions::save(&t, Draft { categories: Some(picked), ..Draft::default() });
            }
        };
        let s = save.clone();
        tags.connect_activate(move |e| s(e));
        let focus = gtk::EventControllerFocus::new();
        let e = tags.clone();
        focus.connect_leave(move |_| save(&e));
        tags.add_controller(focus);
        panel.append(&labeled("Tags", &tags));
    }
    if let Some(source) = meta.source.as_deref() {
        let open = widgets::icon_button("mail-unread-symbolic", "Open the message this came from");
        let uri = source.to_string();
        open.connect_clicked(move |_| {
            let _ = gtk::gio::AppInfo::launch_default_for_uri(&uri, gtk::gio::AppLaunchContext::NONE);
        });
        panel.append(&labeled("From", &open));
    }

    // Subtasks
    if task.parent.is_none() {
        section(panel, "Subtasks");
        for sub in subtasks {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let check = gtk::CheckButton::new();
            check.set_active(sub.done);
            let s = sub.clone();
            check.connect_toggled(move |c| actions::set_done(&s, c.is_active()));
            row.append(&check);
            let label = widgets::label(&sub.title, if sub.done { "done" } else { "" });
            label.set_xalign(0.0);
            label.set_hexpand(true);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&label);
            panel.append(&row);
        }
        let add = gtk::Entry::new();
        add.set_placeholder_text(Some("Add a subtask"));
        add.update_property(&[gtk::accessible::Property::Label("Add a subtask")]);
        let t = task.clone();
        add.connect_activate(move |e| {
            let text = e.text().trim().to_string();
            if text.is_empty() {
                return;
            }
            e.set_text("");
            let quick = actions::parse_quick(&text, actions::now(), caps, None);
            let quick = Quick { title: quick.title, due: quick.due, repeat: None, tags: quick.tags, dropped: Vec::new() };
            actions::add(t.list.clone(), quick, Some(t.uid.clone()), None, |result| {
                if let Err(err) = result {
                    window::toast(&err);
                }
            });
        });
        panel.append(&add);
    }

    let tools = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    tools.set_halign(gtk::Align::End);
    tools.set_margin_top(12);
    let delete = widgets::icon_button("user-trash-symbolic", "Delete (Delete)");
    delete.add_css_class("destructive");
    let t = task.clone();
    let subs: Vec<String> = subtasks.iter().map(|s| s.uid.clone()).collect();
    delete.connect_clicked(move |_| {
        window::select(None);
        actions::delete(&t, subs.clone());
    });
    tools.append(&delete);
    panel.append(&tools);
}

fn labeled(text: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.add_css_class("detail-row");
    let label = widgets::label(text, "detail-label");
    label.set_xalign(0.0);
    label.set_width_chars(8);
    row.append(&label);
    widget.set_hexpand(true);
    widget.as_ref().update_relation(&[gtk::accessible::Relation::LabelledBy(&[label.upcast_ref()])]);
    row.append(widget);
    row
}
