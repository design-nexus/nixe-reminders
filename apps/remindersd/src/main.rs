//! nixe-remindersd: Reminders' background service. It turns due alerts from
//! task lists into desktop notifications (with Complete, Snooze and Dismiss)
//! and keeps `tasks.sqlite` current for Glimpse. No GTK.
//!
//! Alerts from calendars are Nixe Calendar's. While Evolution's own
//! `evolution-alarm-notify` runs, nothing is shown, so alerts aren't doubled.

mod notify;
mod tasks;

use chrono::{Duration, Local};
use nixe_eds::{Calendar, Kind, Registry, Reminders, View, Watch};
use nixe_reminders_core::{ListInfo, Task};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

const BUS_NAME: &str = "io.github.design_nexus.nixe.RemindersEngine";
const SNOOZE_MINUTES: i64 = 10;

const USAGE: &str = "Usage: nixe-remindersd [--write-tasks]\n\
\n\
  Shows reminder alerts as notifications and keeps tasks.sqlite current.\n\
  --write-tasks   write tasks.sqlite once and exit\n";

#[derive(Default)]
struct State {
    registry: Option<Registry>,
    _watch: Option<Watch>,
    reminders: Option<Reminders>,
    open: HashMap<String, Calendar>,
    views: HashMap<String, View>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static PENDING: Cell<bool> = const { Cell::new(false) };
}

fn main() {
    nixe_core::app::init(nixe_core::app::App { slug: "nixe-reminders", name: "Reminders" });
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{USAGE}");
        return;
    }
    if args.iter().any(|a| a == "--write-tasks") {
        let result = Registry::connect().map_err(|e| e.to_string()).and_then(|registry| {
            STATE.with(|s| s.borrow_mut().registry = Some(registry));
            write_tasks()
        });
        match result {
            Ok(n) => println!("{n} open tasks in {}", tasks_path().display()),
            Err(e) => {
                eprintln!("nixe-remindersd: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    let main_loop = glib::MainLoop::new(None, false);
    let quit = main_loop.clone();
    let _owner = gio::bus_own_name(
        gio::BusType::Session,
        BUS_NAME,
        gio::BusNameOwnerFlags::DO_NOT_QUEUE,
        |_, _| {},
        |_, _| start(),
        move |_, _| {
            eprintln!("nixe-remindersd: already running");
            quit.quit();
        },
    );
    main_loop.run();
}

fn tasks_path() -> std::path::PathBuf {
    nixe_core::paths::data_dir().join("tasks.sqlite")
}

fn start() {
    match Registry::connect() {
        Ok(registry) => {
            let watch = registry.watch(schedule);
            let reminders = Reminders::new(&registry);
            let notifier = Rc::new(notify::Notifier::new());
            reminders.connect_due(move |due, _snoozed| {
                if notify::other_notifier_running() {
                    return;
                }
                // Calendars' alerts are Nixe Calendar's to show.
                let tasks_only = |source: &str| STATE.with(|s| s.borrow().registry.as_ref().is_some_and(|r| r.source(source, Kind::TaskList).is_some()));
                for reminder in due.into_iter().filter(|r| tasks_only(&r.source)) {
                    notifier.show(reminder);
                }
            });
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.registry = Some(registry);
                s._watch = Some(watch);
                s.reminders = Some(reminders);
            });
            schedule();
            glib::timeout_add_seconds_local(3600, || {
                schedule();
                glib::ControlFlow::Continue
            });
        }
        Err(err) => {
            eprintln!("nixe-remindersd: {err}; trying again in 30 s");
            glib::timeout_add_seconds_local_once(30, start);
        }
    }
}

pub fn with_reminders(f: impl FnOnce(&Reminders)) {
    STATE.with(|s| {
        if let Some(r) = &s.borrow().reminders {
            f(r);
        }
    });
}

pub fn snooze_until() -> i64 {
    (Local::now() + Duration::minutes(SNOOZE_MINUTES)).timestamp()
}

/// Completes a task from a notification.
pub fn complete(list: &str, uid: &str) {
    let result = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let state = &mut *s;
        let Some(registry) = &state.registry else { return Err("Task data isn't available.".to_string()) };
        if !state.open.contains_key(list) {
            let open = Calendar::open(registry, list, Kind::TaskList, None).map_err(|e| e.to_string())?;
            state.open.insert(list.to_string(), open);
        }
        state.open[list].complete(uid).map_err(|e| e.to_string())
    });
    if let Err(e) = result {
        eprintln!("nixe-remindersd: couldn't complete the task: {e}");
    }
}

fn schedule() {
    if PENDING.with(|p| p.replace(true)) {
        return;
    }
    glib::timeout_add_seconds_local_once(2, || {
        PENDING.with(|p| p.set(false));
        if let Err(e) = write_tasks() {
            eprintln!("nixe-remindersd: tasks: {e}");
        }
    });
}

fn write_tasks() -> Result<usize, String> {
    let (lists, all) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let state = &mut *s;
        let Some(registry) = &state.registry else { return (Vec::new(), Vec::new()) };
        let sources = registry.sources(Kind::TaskList);
        let mut lists = Vec::new();
        let mut all = Vec::new();
        for src in &sources {
            if !state.open.contains_key(&src.uid) {
                match Calendar::open(registry, &src.uid, Kind::TaskList, None) {
                    Ok(open) => {
                        if let Ok(view) = open.watch("#t", schedule) {
                            state.views.insert(src.uid.clone(), view);
                        }
                        state.open.insert(src.uid.clone(), open);
                    }
                    Err(e) => {
                        eprintln!("nixe-remindersd: {}: {e}", src.label());
                        continue;
                    }
                }
            }
            if let Ok(items) = state.open[&src.uid].items("#t") {
                all.extend(items.iter().map(|i| Task::from_item(&src.uid, i)));
            }
            lists.push(ListInfo {
                uid: src.uid.clone(),
                name: src.label().to_string(),
                color: src.color.clone(),
                selected: src.selected,
                group: src.group.clone(),
                account: src.account.clone(),
                needs_sign_in: false,
                offline: false,
                read_only: false,
                caps: Default::default(),
            });
        }
        (lists, all)
    });
    let flags = nixe_reminders_core::Sidecar::open(&nixe_core::paths::data_dir().join("reminders.sqlite"))
        .and_then(|s| s.all())
        .unwrap_or_default();
    let now = Local::now().naive_local();
    tasks::write(&tasks_path(), &lists, &all, &flags, now).map_err(|e| e.to_string())
}
