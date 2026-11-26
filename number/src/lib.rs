//! Number: shows a number topic as text, with its unit, in the middle of the
//! widget. The first example plugin (docs/spec.md "Plugin system").
//!
//! Settings: `decimal_places` (-1 for up to 4, trailing zeros dropped),
//! `font_size`, `fit_text` (as large as fits) and `show_unit`.

use raptaboard_plugin::prelude::*;

#[derive(Default)]
struct Number;

/// The text for a value: the number and, if wanted, its unit.
fn shown(value: Option<f64>, decimals: Option<usize>, unit: Option<String>) -> String {
    match value {
        None => "—".to_string(),
        Some(n) => {
            let text = format_number(n, decimals);
            match unit {
                Some(u) if !u.is_empty() => format!("{text} {u}"),
                _ => text,
            }
        }
    }
}

/// The widget's settings (plugin.toml's `[properties]`), read once a call:
/// everything after that is plain code, tested without the dashboard.
#[derive(Debug, Clone, PartialEq)]
struct Settings {
    /// Places after the point; `None` for up to 4, trailing zeros dropped.
    decimals: Option<usize>,
    font_size: f32,
    fit_text: bool,
    show_unit: bool,
}

impl Default for Settings {
    /// plugin.toml's defaults.
    fn default() -> Self {
        Settings {
            decimals: None,
            font_size: 14.0,
            fit_text: false,
            show_unit: true,
        }
    }
}

impl Settings {
    /// The settings the dashboard gives this widget.
    fn read() -> Settings {
        let places = property_number("decimal_places", -1.0);
        Settings {
            decimals: (places >= 0.0).then(|| places.min(10.0) as usize),
            font_size: property_number("font_size", 14.0) as f32,
            fit_text: property_bool("fit_text", false),
            show_unit: property_bool("show_unit", true),
        }
    }
}

/// The picture: `value` (and its `unit`) in the middle of the widget, as
/// large as fits with `fit_text`, greyed while disconnected. `measure` says
/// how big a text is at a size (the host's `measure-text` in the plugin).
fn picture(
    frame: &FrameInfo,
    settings: &Settings,
    value: Option<f64>,
    unit: Option<String>,
    measure: &dyn Fn(&str, f32) -> Size,
) -> Picture {
    let unit = unit.filter(|_| settings.show_unit);
    let text = shown(value, settings.decimals, unit);
    let mut size = settings.font_size;
    if settings.fit_text {
        // Measure at 100 px and scale to fill 90% of the width, at most
        // 80% of the height.
        let at_100 = measure(&text, 100.0);
        let by_width = frame.size.width * 0.9 / at_100.width.max(1.0) * 100.0;
        let by_height = frame.size.height * 0.8 / at_100.height.max(1.0) * 100.0;
        size = by_width.min(by_height).max(6.0);
    }
    let height = measure(&text, size).height;
    let mut picture = Picture::empty();
    picture.text(Text {
        pos: Point {
            x: frame.size.width / 2.0,
            y: (frame.size.height - height) / 2.0,
        },
        text: text.clone(),
        size,
        color: if frame.connected {
            frame.theme.text
        } else {
            frame.theme.weak_text
        },
        align: Align::Center,
        max_width: None,
        monospace: false,
    });
    picture.summary = Some(text);
    picture
}

/// The dashboard's side: read the settings and value from the host and hand
/// them to [`picture`].
impl Widget for Number {
    fn subscriptions(&self) -> Vec<Subscription> {
        vec![Subscription::Bound]
    }

    fn render(&mut self, frame: FrameInfo) -> Picture {
        let measure = |t: &str, size| host::measure_text(t, size, None, false);
        picture(
            &frame,
            &Settings::read(),
            number(&host::get("")),
            host::unit(""),
            &measure,
        )
    }
}

raptaboard_plugin::widget!(Number);

#[cfg(test)]
mod tests {
    // Tests written by Claude.
    use super::*;

    fn frame(width: f32, height: f32, connected: bool) -> FrameInfo {
        let grey = |v| Color {
            r: v,
            g: v,
            b: v,
            a: 255,
        };
        FrameInfo {
            size: Size { width, height },
            time_s: 0.0,
            connected,
            locked: false,
            theme: Theme {
                dark: true,
                text: grey(230),
                weak_text: grey(120),
                background: grey(0),
                card: grey(20),
                accent: grey(50),
                error: grey(60),
                warning: grey(70),
                info: grey(80),
            },
        }
    }

    /// Every character 0.6 of the size wide, one line the size high.
    fn measure(text: &str, size: f32) -> Size {
        Size {
            width: text.chars().count() as f32 * size * 0.6,
            height: size,
        }
    }

    fn line(p: &Picture) -> &Text {
        match &p.shapes[..] {
            [Shape::Text(t)] => t,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_value_shows_in_the_middle_with_its_unit_greyed_when_disconnected() {
        let two = Settings {
            decimals: Some(2),
            ..Settings::default()
        };
        let p = picture(
            &frame(200.0, 100.0, true),
            &two,
            Some(1.23456),
            Some("m".into()),
            &measure,
        );
        assert_eq!(p.summary.as_deref(), Some("1.23 m"));
        let t = line(&p);
        assert_eq!(
            (t.text.as_str(), t.size, t.align),
            ("1.23 m", 14.0, Align::Center)
        );
        // Centered: the middle across, half its height above the middle down.
        assert_eq!((t.pos.x, t.pos.y), (100.0, 43.0));
        assert_eq!(t.color.r, 230);
        let no_unit = Settings {
            show_unit: false,
            ..Settings::default()
        };
        let p = picture(
            &frame(200.0, 100.0, false),
            &no_unit,
            None,
            Some("m".into()),
            &measure,
        );
        assert_eq!(line(&p).text, "—");
        assert_eq!(line(&p).color.r, 120, "greyed");
    }

    #[test]
    fn fit_text_fills_the_width_or_the_height() {
        let fit = Settings {
            fit_text: true,
            ..Settings::default()
        };
        let size = |w, h| {
            line(&picture(
                &frame(w, h, true),
                &fit,
                Some(12.0),
                None,
                &measure,
            ))
            .size
        };
        // Wide: the height limits it (80% of 100). Narrow: "12" in 90% of 60.
        assert!((size(600.0, 100.0) - 80.0).abs() < 0.01);
        assert!((size(60.0, 100.0) - 45.0).abs() < 0.01);
        assert_eq!(size(1.0, 100.0), 6.0, "never smaller than 6");
    }

    #[test]
    fn text_has_the_unit_and_a_dash_without_a_value() {
        assert_eq!(shown(Some(1.23456), Some(2), Some("m".into())), "1.23 m");
        assert_eq!(shown(Some(2.5), None, None), "2.5");
        assert_eq!(shown(None, Some(2), Some("m".into())), "—");
    }
}
