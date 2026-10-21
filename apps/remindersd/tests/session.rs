//! End to end on a private session bus with scratch XDG directories (run through
//! tools/test-remindersd.sh): a task due in 20 seconds with an alert goes into
//! EDS's local task list, nixe-remindersd runs, this test plays the desktop's
//! notification server, presses Complete, and checks the task is done.

use gio::prelude::*;
use nixe_eds::{Calendar, Draft, Kind, Registry, Scope, When};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const NODE: &str = r#"<node><interface name="org.freedesktop.Notifications">
  <method name="Notify"><arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="s" direction="in"/>
    <arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="as" direction="in"/>
    <arg type="a{sv}" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="out"/></method>
  <method name="GetCapabilities"><arg type="as" direction="out"/></method>
  <signal name="ActionInvoked"><arg type="u"/><arg type="s"/></signal>
</interface></node>"#;

/// What the fake server was asked to show: summary, body, actions.
type Shown = Rc<RefCell<Vec<(String, String, Vec<String>)>>>;

fn pump(until: Instant, mut done: impl FnMut() -> bool) -> bool {
    let ctx = glib::MainContext::default();
    while Instant::now() < until {
        while ctx.iteration(false) {}
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
#[ignore]
fn due_task_alert_can_be_completed_from_the_notification() {
    assert!(std::env::var("NIXE_EDS_SCRATCH").is_ok(), "run through tools/test-remindersd.sh");
    let conn = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
    let seen: Shown = Rc::default();
    let info = gio::DBusNodeInfo::for_xml(NODE).unwrap().lookup_interface("org.freedesktop.Notifications").unwrap();
    let sink = seen.clone();
    let _object = conn
        .register_object("/org/freedesktop/Notifications", &info)
        .method_call(move |_, _, _, _, method, params, invocation| match method {
            "Notify" => {
                let summary = params.child_value(3).get::<String>().unwrap_or_default();
                let body = params.child_value(4).get::<String>().unwrap_or_default();
                let actions = params.child_value(5).get::<Vec<String>>().unwrap_or_default();
                sink.borrow_mut().push((summary, body, actions));
                invocation.return_value(Some(&(7u32,).to_variant()));
            }
            _ => invocation.return_value(Some(&(vec!["body".to_string(), "actions".to_string()],).to_variant())),
        })
        .build()
        .unwrap();
    let _name = gio::bus_own_name_on_connection(&conn, "org.freedesktop.Notifications", gio::BusNameOwnerFlags::NONE, |_, _| {}, |_, _| {});

    let registry = Registry::connect().unwrap();
    let list = Calendar::open(&registry, "system-task-list", Kind::TaskList, None).unwrap();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let uid = list
        .create(&Draft {
            summary: Some("Call Sam".into()),
            description: Some("About the trip".into()),
            due: Some(Some(When::Time(now + 20))),
            alarms: Some(vec![0]),
            ..Draft::default()
        })
        .unwrap();

    let mut daemon = std::process::Command::new(env!("CARGO_BIN_EXE_nixe-remindersd")).spawn().unwrap();
    let notified = pump(Instant::now() + Duration::from_secs(90), || !seen.borrow().is_empty());
    assert!(notified, "a notification was asked for");
    {
        let seen = seen.borrow();
        let (summary, body, actions) = &seen[0];
        assert_eq!(summary, "Call Sam");
        assert!(body.contains("About the trip"), "{body}");
        assert!(actions.contains(&"complete".to_string()));
    }
    conn.emit_signal(None, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "ActionInvoked", Some(&(7u32, "complete").to_variant()))
        .unwrap();
    let done = pump(Instant::now() + Duration::from_secs(15), || list.get(&uid, None).is_ok_and(|t| t.is_done()));
    let tasks_file = std::path::PathBuf::from(std::env::var("XDG_DATA_HOME").unwrap()).join("nixe-reminders/tasks.sqlite");
    let wrote = tasks_file.exists();
    let _ = daemon.kill();
    let _ = daemon.wait();
    list.remove(&uid, None, Scope::All).unwrap();
    assert!(done, "Complete in the notification completed the task");
    assert!(wrote, "tasks.sqlite was written");
}
