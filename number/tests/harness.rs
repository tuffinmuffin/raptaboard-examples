//! The Number plugin, run with the test harness the way the dashboard runs
//! it. `cargo test` here builds it for WebAssembly first.

// Tests written by Claude.

#![cfg(not(target_arch = "wasm32"))]

use raptaboard_plugin_test::{Harness, light_theme};

fn number() -> Harness {
    Harness::build(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn it_shows_the_value_with_its_unit() {
    let mut w = number();
    w.set("", 1.23456).unit("", "m");
    let drawn = w.render();
    assert!(drawn.has_text("1.2346 m"), "{:?}", drawn.texts());
    assert_eq!(drawn.summary(), Some("1.2346 m"));
    // Without a value: a dash.
    w.remove("");
    assert!(w.render().has_text("—"));
}

#[test]
fn its_settings_change_the_text() {
    let mut w = number();
    w.property("decimal_places", 2.0)
        .property("show_unit", false)
        .unit("", "m")
        .set("", -7.5);
    assert!(w.render().has_text("-7.50"));
    // An int topic reads as a number too.
    w.set("", 3i64);
    assert!(w.render().has_text("3.00"));
}

#[test]
fn fit_text_grows_with_the_widget() {
    let mut w = number();
    w.property("fit_text", true).set("", 42.0);
    let small = w.size(100.0, 50.0).render().text("42").unwrap().size;
    let big = w.size(400.0, 200.0).render().text("42").unwrap().size;
    assert!(big > 3.0 * small, "{small} then {big}");
    assert_eq!(
        w.property("fit_text", false)
            .set("", 42.0)
            .render()
            .text("42")
            .unwrap()
            .size,
        14.0
    );
}

#[test]
fn it_greys_while_disconnected_and_follows_the_theme() {
    let mut w = number();
    w.set("", 1.0);
    let connected = w.render().text("1").unwrap().color;
    let disconnected = w.connected(false).render().text("1").unwrap().color;
    assert_ne!(connected, disconnected);
    let light = w
        .connected(true)
        .theme(light_theme())
        .render()
        .text("1")
        .unwrap()
        .color;
    assert_eq!(light, light_theme().text);
}
