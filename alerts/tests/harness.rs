//! The Alerts plugin, run with the test harness the way the dashboard runs
//! it. `cargo test` here builds it for WebAssembly first.

// Tests written by Claude.

#![cfg(not(target_arch = "wasm32"))]

use raptaboard_plugin_test::{Harness, Level, NtValue};

/// An Alerts widget on one group, with a warning and an info up.
fn alerts() -> Harness {
    let mut w = Harness::build(env!("CARGO_MANIFEST_DIR"));
    w.size(512.0, 256.0);
    w
}

fn group(w: &mut Harness, errors: Vec<&str>, warnings: Vec<&str>, infos: Vec<&str>) {
    w.set_all(vec![
        ("errors", errors),
        ("warnings", warnings),
        ("infos", infos),
    ]);
}

#[test]
fn it_counts_by_level_and_lists_the_text() {
    let mut w = alerts();
    group(
        &mut w,
        vec!["Encoder lost", "CAN fault"],
        vec!["Battery low"],
        vec![],
    );
    let drawn = w.render();
    assert_eq!(drawn.summary(), Some("2 errors, 1 warning, 0 infos"));
    for text in ["Encoder lost", "CAN fault", "Battery low", "2"] {
        assert!(drawn.has_text(text), "{text}: {:?}", drawn.texts());
    }
    // Errors first, in the robot's order.
    let texts = drawn.texts();
    let at = |t: &str| texts.iter().position(|x| *x == t).unwrap();
    assert!(at("Encoder lost") < at("CAN fault") && at("CAN fault") < at("Battery low"));
    // min_level hides the less urgent.
    w.property("min_level", "error");
    group(&mut w, vec!["Encoder lost"], vec!["Battery low"], vec![]);
    assert_eq!(w.render().summary(), Some("1 error, 0 warnings, 0 infos"));
}

#[test]
fn a_new_error_pulses_and_sounds_once_but_not_on_the_first_look() {
    let mut w = alerts();
    w.property("sound", true).property("tone", "chirp");
    group(&mut w, vec!["Already up"], vec![], vec![]);
    let first = w.render();
    assert!(w.take_sounds().is_empty(), "already up on the first look");
    assert_eq!(first.redraw_in_s(), None);
    group(&mut w, vec!["Brownout", "Already up"], vec![], vec![]);
    assert!(w.advance(1.0).render().redraw_in_s().is_some(), "pulsing");
    assert_eq!(w.take_sounds(), ["chirp"]);
    // Another new error within 3 s: no second tone.
    group(
        &mut w,
        vec!["Arm fault", "Brownout", "Already up"],
        vec![],
        vec![],
    );
    w.advance(1.0).render();
    assert!(w.take_sounds().is_empty());
    // The errors gone: the pulse stops before its time is up.
    group(&mut w, vec![], vec![], vec![]);
    assert_eq!(w.advance(0.5).render().redraw_in_s(), None);
    // A warning never asks for attention.
    group(&mut w, vec![], vec!["Hot motor"], vec![]);
    w.advance(5.0).render();
    assert!(w.take_sounds().is_empty());
}

#[test]
fn the_sound_is_off_unless_set() {
    let mut w = alerts();
    group(&mut w, vec![], vec![], vec![]);
    w.render();
    group(&mut w, vec!["Brownout"], vec![], vec![]);
    assert!(w.render().redraw_in_s().is_some(), "it still pulses");
    assert!(w.take_sounds().is_empty());
}

#[test]
fn the_tab_mark_is_the_most_urgent_above_info() {
    let mut w = alerts();
    group(&mut w, vec![], vec![], vec!["Vision up"]);
    assert_eq!(w.tab_mark(), None);
    group(&mut w, vec![], vec!["Battery low"], vec!["Vision up"]);
    assert_eq!(w.tab_mark(), Some(Level::Warning));
    group(&mut w, vec!["Brownout"], vec!["Battery low"], vec![]);
    assert_eq!(w.tab_mark(), Some(Level::Error));
}

#[test]
fn every_group_comes_from_tables_of_its_type() {
    let mut w = alerts();
    w.property("all_groups", true);
    let subs = format!("{:?}", w.subscriptions());
    assert!(subs.contains("TablesOfType(\"Alerts\")"), "{subs}");
    w.set_all(vec![
        (
            "/SmartDashboard/Alerts/.type",
            NtValue::Str("Alerts".into()),
        ),
        (
            "/SmartDashboard/Alerts/errors",
            NtValue::StrArray(vec!["Brownout".into()]),
        ),
        ("/Arm/.type", NtValue::Str("Alerts".into())),
        ("/Arm/warnings", NtValue::StrArray(vec!["Arm hot".into()])),
    ]);
    let drawn = w.render();
    assert_eq!(drawn.summary(), Some("1 error, 1 warning, 0 infos"));
    // Two groups: each alert says which.
    assert!(drawn.has_text("Alerts: Brownout"), "{:?}", drawn.texts());
    assert!(drawn.has_text("Arm: Arm hot"));
    // A group whose table goes away is dropped.
    w.remove("/Arm/.type").remove("/Arm/warnings");
    assert_eq!(w.render().summary(), Some("1 error, 0 warnings, 0 infos"));
}

#[test]
fn it_keeps_the_last_alerts_greyed_when_disconnected() {
    let mut w = alerts();
    assert!(w.render().has_text("Alerts not published"));
    group(&mut w, vec!["Brownout"], vec![], vec![]);
    assert!(w.render().has_text("Brownout"));
    w.remove("errors")
        .remove("warnings")
        .remove("infos")
        .connected(false);
    let drawn = w.render();
    assert_eq!(
        drawn.summary(),
        Some("1 error, 0 warnings, 0 infos (disconnected)")
    );
    assert!(drawn.has_text("Robot disconnected: the last alerts seen"));
    assert!(drawn.has_text("Brownout"));
    w.connected(true);
    group(&mut w, vec![], vec![], vec![]);
    assert!(w.render().has_text("No active alerts"));
}

#[test]
fn what_doesnt_fit_is_counted() {
    let mut w = alerts();
    let many: Vec<String> = (1..=20).map(|i| format!("Fault {i}")).collect();
    w.size(300.0, 120.0)
        .set("errors", NtValue::StrArray(many))
        .set("warnings", vec![] as Vec<&str>);
    let drawn = w.render();
    let more = drawn.texts().into_iter().find(|t| t.ends_with("more"));
    assert!(
        more.is_some_and(|t| t.starts_with('+')),
        "{:?}",
        drawn.texts()
    );
    // Without text: only the circle.
    w.property("show_text", false);
    w.set("errors", vec!["Brownout"]);
    let drawn = w.render();
    assert!(!drawn.has_text("Brownout"));
    assert!(!drawn.shapes().is_empty());
}
