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

impl Widget for Number {
    fn subscriptions(&self) -> Vec<Subscription> {
        vec![Subscription::Bound]
    }

    fn render(&mut self, frame: FrameInfo) -> Picture {
        let places = property_number("decimal_places", -1.0);
        let decimals = (places >= 0.0).then(|| places.min(10.0) as usize);
        let unit = property_bool("show_unit", true)
            .then(|| host::unit(""))
            .flatten();
        let value = number(&host::get(""));
        let text = shown(value, decimals, unit);

        let mut size = property_number("font_size", 14.0) as f32;
        if property_bool("fit_text", false) {
            // Measure at 100 px and scale to fill 90% of the width, at most
            // 80% of the height.
            let at_100 = host::measure_text(&text, 100.0, None, false);
            let by_width = frame.size.width * 0.9 / at_100.width.max(1.0) * 100.0;
            let by_height = frame.size.height * 0.8 / at_100.height.max(1.0) * 100.0;
            size = by_width.min(by_height).max(6.0);
        }

        let mut picture = Picture::empty();
        let mut line = centered_text(&frame, &text, size);
        if !frame.connected {
            line.color = frame.theme.weak_text;
        }
        picture.text(line);
        picture.summary = Some(text);
        picture
    }
}

raptaboard_plugin::widget!(Number);

#[cfg(test)]
mod tests {
    // Tests written by Claude.
    use super::*;

    #[test]
    fn text_has_the_unit_and_a_dash_without_a_value() {
        assert_eq!(shown(Some(1.23456), Some(2), Some("m".into())), "1.23 m");
        assert_eq!(shown(Some(2.5), None, None), "2.5");
        assert_eq!(shown(None, Some(2), Some("m".into())), "—");
    }
}
