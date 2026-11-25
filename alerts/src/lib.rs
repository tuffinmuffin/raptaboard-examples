//! Alerts: the robot's active alerts (WPILib's `Alert` class), as a circle
//! with the three signs in it, and optionally their text. The second example
//! plugin (docs/spec.md "Plugin system"): it draws what the built-in Alerts
//! widget draws, from a table rather than one topic.
//!
//! Each alert group is a table whose `.type` is `"Alerts"`, holding three
//! string[] topics, `errors`, `warnings` and `infos`, each listing the text of
//! the alerts of that level active now, newest first.
//!
//! The circle has a red ✕ (errors), a yellow ! (warnings) and a blue i
//! (infos); each sign lights while there's an alert of its level, with a count
//! when there's more than one, and the ring takes the color of the most
//! urgent. While an error or warning is active, the most urgent sign grows
//! large in the middle and the other two shrink below it. With `show_text` the
//! alerts' text is listed beside the circle or below it (`text_position`).
//!
//! Bound to one group's table, or with `all_groups` to every group on the
//! server. A new error pulses the circle for `pulse_seconds` (`pulse`), and
//! plays `tone` (`sound`). While the connection is down it keeps showing the
//! last alerts it saw, greyed.

use std::collections::{BTreeSet, HashSet};

use raptaboard_plugin::prelude::*;

/// The `.type` of an alert group's table.
const ALERTS_TYPE: &str = "Alerts";
/// The pulse's length when `pulse_seconds` isn't set, in seconds.
const DEFAULT_PULSE_S: f64 = 5.0;
/// The fewest seconds between two tones, so a flickering alert doesn't sound
/// over and over.
const TONE_GAP_S: f64 = 3.0;
/// Pulses a second.
const PULSE_RATE: f64 = 2.0;
/// How often the picture is drawn again while it pulses, in seconds.
const PULSE_FRAME_S: f64 = 1.0 / 30.0;
/// Size of the alerts' text, in pixels.
const TEXT_SIZE: f32 = 13.0;
/// Space between the circle and the text, in pixels.
const GAP: f32 = 6.0;

/// How urgent an alert is. Ordered most urgent first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    /// All three, most urgent first.
    const ALL: [Severity; 3] = [Severity::Error, Severity::Warning, Severity::Info];

    /// The topic under a group's table that lists this level's alerts.
    fn key(self) -> &'static str {
        match self {
            Severity::Error => "errors",
            Severity::Warning => "warnings",
            Severity::Info => "infos",
        }
    }

    /// Its name for people, plural when `n` isn't 1.
    fn name(self, n: usize) -> &'static str {
        match (self, n == 1) {
            (Severity::Error, true) => "error",
            (Severity::Error, false) => "errors",
            (Severity::Warning, true) => "warning",
            (Severity::Warning, false) => "warnings",
            (Severity::Info, true) => "info",
            (Severity::Info, false) => "infos",
        }
    }

    /// The level named `name` (`"error"`, `"warning"` or `"info"`).
    fn from_name(name: &str) -> Option<Severity> {
        Severity::ALL.into_iter().find(|s| s.name(1) == name)
    }

    /// As a tab mark's level.
    fn level(self) -> Level {
        match self {
            Severity::Error => Level::Error,
            Severity::Warning => Level::Warning,
            Severity::Info => Level::Info,
        }
    }

    /// Its color in the dashboard's theme.
    fn color(self, theme: &Theme) -> Color {
        match self {
            Severity::Error => theme.error,
            Severity::Warning => theme.warning,
            Severity::Info => theme.info,
        }
    }
}

/// One active alert.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveAlert {
    /// The group: its table's last name part, `""` for the widget's own.
    group: String,
    severity: Severity,
    text: String,
}

/// The active alerts in one group, errors first, each level newest first;
/// `None` if none of its lists has a value. `get` reads a topic: `prefix`
/// and the list's key (`errors`).
fn read_group(
    prefix: &str,
    group: &str,
    get: &impl Fn(&str) -> Option<NtValue>,
) -> Option<Vec<ActiveAlert>> {
    let mut found = false;
    let mut alerts = Vec::new();
    for severity in Severity::ALL {
        if let Some(NtValue::StrArray(texts)) = get(&format!("{prefix}{}", severity.key())) {
            found = true;
            alerts.extend(texts.into_iter().map(|text| ActiveAlert {
                group: group.to_string(),
                severity,
                text,
            }));
        }
    }
    found.then_some(alerts)
}

/// How many of `alerts` there are of each level, errors first.
fn counts(alerts: &[ActiveAlert]) -> [usize; 3] {
    let mut n = [0; 3];
    for a in alerts {
        n[a.severity as usize] += 1;
    }
    n
}

/// Whether the alerts a [`Watch`] gives are the robot's now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Freshness {
    /// What the server has now.
    Current,
    /// Nothing published now (yet, or any more): the last alerts seen.
    NotPublishedNow,
    /// Disconnected: the last alerts seen.
    Disconnected,
    /// Connected, and nothing has been published since watching began.
    NeverPublished,
}

/// What a [`Watch`] says to show this time.
#[derive(Debug, Clone, PartialEq)]
struct Seen {
    alerts: Vec<ActiveAlert>,
    freshness: Freshness,
    /// An error is active that wasn't the last time (never on the first look).
    new_error: bool,
}

/// Watches the alerts look after look: which errors are new, and the last
/// alerts seen, kept while they're not published or the connection is down.
#[derive(Debug, Clone, Default)]
struct Watch {
    last: Vec<ActiveAlert>,
    /// The errors active last look, by group and text.
    errors: HashSet<(String, String)>,
    started: bool,
}

impl Watch {
    /// Take a look: `current` is what's published now, `connected` whether the
    /// connection is up.
    fn update(&mut self, current: Option<Vec<ActiveAlert>>, connected: bool) -> Seen {
        let stale = |last: &[ActiveAlert], freshness| Seen {
            alerts: last.to_vec(),
            freshness,
            new_error: false,
        };
        match (current, connected) {
            (Some(now), true) => {
                let errors: HashSet<(String, String)> = now
                    .iter()
                    .filter(|a| a.severity == Severity::Error)
                    .map(|a| (a.group.clone(), a.text.clone()))
                    .collect();
                let new_error = self.started && errors.iter().any(|e| !self.errors.contains(e));
                self.errors = errors;
                self.started = true;
                self.last.clone_from(&now);
                Seen {
                    alerts: now,
                    freshness: Freshness::Current,
                    new_error,
                }
            }
            (None, true) if !self.last.is_empty() => stale(&self.last, Freshness::NotPublishedNow),
            (None, true) => stale(&[], Freshness::NeverPublished),
            (_, false) => stale(&self.last, Freshness::Disconnected),
        }
    }
}

/// A rectangle in the widget, in pixels from its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Area {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

impl Area {
    fn width(&self) -> f32 {
        self.x1 - self.x0
    }
    fn height(&self) -> f32 {
        self.y1 - self.y0
    }
}

/// Where the circle and the text go in `area`: the circle's area and the
/// text's (if it's shown).
fn layout(area: Area, show_text: bool, position: &str) -> (Area, Option<Area>) {
    if !show_text {
        return (area, None);
    }
    let beside = match position {
        "right" => true,
        "below" => false,
        _ => area.width() >= area.height() * 1.6,
    };
    if beside {
        let side = area.height().min(area.width() * 0.4);
        let split = area.x0 + side;
        (
            Area { x1: split, ..area },
            Some(Area {
                x0: split + GAP,
                ..area
            }),
        )
    } else {
        let side = area.width().min(area.height() * 0.45);
        let split = area.y0 + side;
        (
            Area { y1: split, ..area },
            Some(Area {
                y0: split + GAP,
                ..area
            }),
        )
    }
}

/// Where each sign goes in a circle of radius 1 (center at 0, y down), and
/// its size: the three the same unless an error or warning is active; then
/// the most urgent grows large in the middle and the other two shrink along
/// the bottom, the more urgent on the left.
fn sign_places(counts: [usize; 3]) -> [(Severity, (f32, f32), f32); 3] {
    let worst = [Severity::Error, Severity::Warning]
        .into_iter()
        .find(|s| counts[*s as usize] > 0);
    let Some(worst) = worst else {
        return [
            (Severity::Error, (-0.4, -0.25), 0.42),
            (Severity::Warning, (0.4, -0.25), 0.42),
            (Severity::Info, (0.0, 0.45), 0.42),
        ];
    };
    let mut others = Severity::ALL.into_iter().filter(|s| *s != worst);
    let (left, right) = (
        others.next().unwrap_or(Severity::Info),
        others.next().unwrap_or(Severity::Info),
    );
    [
        (worst, (0.0, -0.15), 0.72),
        (left, (-0.5, 0.5), 0.26),
        (right, (0.5, 0.5), 0.26),
    ]
}

/// `c` with its opacity times `f` (0 to 1).
fn fade(c: Color, f: f32) -> Color {
    Color {
        a: (f32::from(c.a) * f.clamp(0.0, 1.0)).round() as u8,
        ..c
    }
}

fn point(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn stroke(width: f32, color: Color) -> Stroke {
    Stroke { width, color }
}

fn dot(center: Point, radius: f32, color: Color) -> Shape {
    Shape::Circle(Circle {
        center,
        radius,
        stroke: None,
        fill: Some(color),
    })
}

fn line(start: Point, end: Point, stroke: Stroke) -> Shape {
    Shape::Line(Line { start, end, stroke })
}

fn text(pos: Point, text: String, size: f32, color: Color, max_width: Option<f32>) -> Shape {
    Shape::Text(Text {
        pos,
        text,
        size,
        color,
        align: Align::Left,
        max_width,
        monospace: false,
    })
}

/// A level's sign at (`x`, `y`), `size` across: ✕ for errors, ! for
/// warnings, i for infos (a bar and a dot).
fn sign(shapes: &mut Vec<Shape>, severity: Severity, x: f32, y: f32, size: f32, color: Color) {
    if severity == Severity::Error {
        let h = size * 0.8 / 2.0;
        let s = stroke((size * 0.8 * 0.18).max(1.5), color);
        shapes.push(line(point(x - h, y - h), point(x + h, y + h), s));
        shapes.push(line(point(x - h, y + h), point(x + h, y - h), s));
        return;
    }
    let h = size / 2.0;
    let width = (size * 0.2).max(1.5);
    let r = width * 0.75;
    let (top, bottom, dot_y) = if severity == Severity::Info {
        (y - h * 0.35, y + h, y - h + r)
    } else {
        (y - h, y + h * 0.35, y + h - r)
    };
    shapes.push(line(point(x, top), point(x, bottom), stroke(width, color)));
    shapes.push(dot(point(x, dot_y), r, color));
}

/// The circle in `area` (centered, as big as fits): the three signs, lit by
/// `counts`, with counts over 1 beside them, and the ring in the most urgent
/// color. `pulse` (0 to 1) fills it red; `dim` greys it all.
fn circle(
    shapes: &mut Vec<Shape>,
    theme: &Theme,
    area: Area,
    counts: [usize; 3],
    pulse: f32,
    dim: bool,
) {
    let r = area.width().min(area.height()) / 2.0 - 2.0;
    if r <= 2.0 {
        return;
    }
    let (cx, cy) = ((area.x0 + area.x1) / 2.0, (area.y0 + area.y1) / 2.0);
    let off = fade(theme.weak_text, 0.5);
    let lit = |s: Severity| {
        if counts[s as usize] == 0 {
            off
        } else if dim {
            theme.weak_text
        } else {
            s.color(theme)
        }
    };
    if pulse > 0.0 {
        shapes.push(dot(
            point(cx, cy),
            r,
            fade(theme.error, 0.15 + 0.35 * pulse),
        ));
    }
    let ring = Severity::ALL
        .into_iter()
        .find(|s| counts[*s as usize] > 0)
        .map_or(off, lit);
    shapes.push(Shape::Circle(Circle {
        center: point(cx, cy),
        radius: r,
        stroke: Some(stroke((r * 0.08).clamp(1.5, 4.0), ring)),
        fill: None,
    }));
    for (severity, (dx, dy), size) in sign_places(counts) {
        let (x, y, size) = (cx + dx * r, cy + dy * r, size * r);
        let color = lit(severity);
        sign(shapes, severity, x, y, size, color);
        let n = counts[severity as usize];
        if n > 1 {
            shapes.push(text(
                point(x + size * 0.55, y + size * 0.55),
                n.to_string(),
                (size * 0.6).max(8.0),
                color,
                None,
            ));
        }
    }
}

/// The alerts listed in `area`, each with its sign, long text wrapped; the
/// group's name first when there's more than one group. `note` heads the
/// list. What doesn't fit is counted at the end.
fn list(
    shapes: &mut Vec<Shape>,
    theme: &Theme,
    area: Area,
    alerts: &[ActiveAlert],
    note: Option<&str>,
    dim: bool,
) {
    let mut y = area.y0;
    let line_height = TEXT_SIZE * 1.3;
    if let Some(note) = note {
        let width = area.width().max(10.0);
        shapes.push(text(
            point(area.x0, y),
            note.to_string(),
            TEXT_SIZE,
            theme.weak_text,
            Some(width),
        ));
        y += host::measure_text(note, TEXT_SIZE, Some(width), false).height + 4.0;
    }
    if alerts.is_empty() && note.is_none() {
        shapes.push(text(
            point(area.x0, y),
            "No active alerts".into(),
            TEXT_SIZE,
            theme.weak_text,
            None,
        ));
        return;
    }
    let groups: HashSet<&str> = alerts.iter().map(|a| a.group.as_str()).collect();
    let text_x = area.x0 + TEXT_SIZE + 4.0;
    let width = (area.x1 - text_x).max(10.0);
    let color = if dim { theme.weak_text } else { theme.text };
    for (i, a) in alerts.iter().enumerate() {
        let shown = if groups.len() > 1 {
            format!("{}: {}", a.group, a.text)
        } else {
            a.text.clone()
        };
        let height = host::measure_text(&shown, TEXT_SIZE, Some(width), false)
            .height
            .max(line_height);
        // Not enough room for this one: say how many more there are.
        if y + height > area.y1 && i > 0 {
            shapes.push(text(
                point(text_x, y),
                format!("+{} more", alerts.len() - i),
                TEXT_SIZE,
                theme.weak_text,
                None,
            ));
            return;
        }
        let sign_color = if dim {
            theme.weak_text
        } else {
            a.severity.color(theme)
        };
        sign(
            shapes,
            a.severity,
            area.x0 + TEXT_SIZE / 2.0,
            y + line_height / 2.0,
            TEXT_SIZE * 0.8,
            sign_color,
        );
        shapes.push(text(point(text_x, y), shown, TEXT_SIZE, color, Some(width)));
        y += height + 2.0;
    }
}

/// What a screen reader says for the widget, and the tests read: its counts
/// and whether they're current.
fn summary(counts: [usize; 3], freshness: Freshness) -> String {
    format!(
        "{} {}, {} {}, {} {}{}",
        counts[0],
        Severity::Error.name(counts[0]),
        counts[1],
        Severity::Warning.name(counts[1]),
        counts[2],
        Severity::Info.name(counts[2]),
        match freshness {
            Freshness::Disconnected => " (disconnected)",
            Freshness::NotPublishedNow => " (not published)",
            _ => "",
        },
    )
}

/// The least urgent level shown, from `min_level`.
fn min_level() -> Severity {
    Severity::from_name(&property_string("min_level", "info")).unwrap_or(Severity::Info)
}

#[derive(Default)]
struct Alerts {
    /// With `all_groups`: the alert groups' tables seen, like
    /// "/SmartDashboard/Alerts".
    tables: BTreeSet<String>,
    watch: Watch,
    /// When the pulse ends (the frame's time, seconds).
    pulse_until: f64,
    /// When the last tone played, if one has.
    last_tone: Option<f64>,
}

impl Alerts {
    /// What's published now, errors first across groups; `None` if nothing is.
    fn current(&self) -> Option<Vec<ActiveAlert>> {
        let get = |topic: &str| host::get(topic);
        if !property_bool("all_groups", false) {
            // The widget's own table: its keys, the group's name not needed.
            return read_group("", "", &get);
        }
        let mut found = false;
        let mut all = Vec::new();
        for table in &self.tables {
            let group = table.rsplit('/').next().unwrap_or(table);
            if let Some(alerts) = read_group(&format!("{table}/"), group, &get) {
                found = true;
                all.extend(alerts);
            }
        }
        all.sort_by_key(|a| a.severity);
        found.then_some(all)
    }
}

impl Widget for Alerts {
    fn subscriptions(&self) -> Vec<Subscription> {
        if property_bool("all_groups", false) {
            vec![Subscription::TablesOfType(ALERTS_TYPE.into())]
        } else {
            Severity::ALL
                .iter()
                .map(|s| Subscription::Key(s.key().into()))
                .collect()
        }
    }

    fn update(&mut self, samples: Vec<Sample>) {
        // Every group's `.type` comes with `all_groups`: that's how they're found.
        for s in samples {
            if let Some(table) = s.topic.strip_suffix("/.type") {
                match s.value {
                    Some(NtValue::Str(t)) if t == ALERTS_TYPE => {
                        self.tables.insert(table.to_string());
                    }
                    _ => {
                        self.tables.remove(table);
                    }
                }
            }
        }
    }

    fn render(&mut self, frame: FrameInfo) -> Picture {
        let now = frame.time_s;
        let all_groups = property_bool("all_groups", false);
        let seen = self.watch.update(self.current(), frame.connected);
        // A new error: pulse, and maybe a tone.
        if seen.new_error {
            if property_bool("pulse", true) {
                self.pulse_until = now + property_number("pulse_seconds", DEFAULT_PULSE_S);
            }
            if property_bool("sound", false)
                && self.last_tone.is_none_or(|last| now - last >= TONE_GAP_S)
            {
                self.last_tone = Some(now);
                if let Err(e) = host::play_sound(&property_string("tone", "beep")) {
                    host::log(&format!("no sound: {e}"));
                }
            }
        }
        let (note, dim) = match seen.freshness {
            Freshness::Current => (None, false),
            Freshness::NotPublishedNow => (Some("Not published now: the last alerts seen"), true),
            Freshness::NeverPublished if all_groups => (Some("No alert groups published"), false),
            Freshness::NeverPublished => (Some("Alerts not published"), false),
            Freshness::Disconnected => (Some("Robot disconnected: the last alerts seen"), true),
        };
        let lowest = min_level();
        let shown: Vec<ActiveAlert> = seen
            .alerts
            .into_iter()
            .filter(|a| a.severity <= lowest)
            .collect();
        let counts = counts(&shown);
        // The pulse stops once the errors are gone, not only when its time is up.
        if counts[Severity::Error as usize] == 0 {
            self.pulse_until = 0.0;
        }
        let mut picture = Picture::empty();
        let pulse = if now < self.pulse_until {
            picture.redraw_in_s = Some(PULSE_FRAME_S);
            (0.5 - 0.5 * (now * PULSE_RATE * std::f64::consts::TAU).cos()) as f32
        } else {
            0.0
        };
        let whole = Area {
            x0: 0.0,
            y0: 0.0,
            x1: frame.size.width,
            y1: frame.size.height,
        };
        let (circle_area, text_area) = layout(
            whole,
            property_bool("show_text", true),
            &property_string("text_position", "auto"),
        );
        circle(
            &mut picture.shapes,
            &frame.theme,
            circle_area,
            counts,
            pulse,
            dim,
        );
        if let Some(area) = text_area {
            list(&mut picture.shapes, &frame.theme, area, &shown, note, dim);
        }
        picture.summary = Some(summary(counts, seen.freshness));
        picture
    }

    /// The most urgent level above info shown: errors and warnings mark the
    /// tab, infos don't.
    fn tab_mark(&self) -> Option<Level> {
        let lowest = min_level();
        self.current()
            .unwrap_or_default()
            .iter()
            .map(|a| a.severity)
            .filter(|s| *s <= lowest)
            .min()
            .filter(|s| *s != Severity::Info)
            .map(Severity::level)
    }
}

raptaboard_plugin::widget!(Alerts);

#[cfg(test)]
mod tests {
    // Tests written by Claude.
    use super::*;

    fn alert(severity: Severity, text: &str) -> ActiveAlert {
        ActiveAlert {
            group: String::new(),
            severity,
            text: text.into(),
        }
    }

    #[test]
    fn a_group_reads_in_level_order() {
        let get = |topic: &str| match topic {
            "/A/errors" => Some(NtValue::StrArray(vec!["new".into(), "old".into()])),
            "/A/infos" => Some(NtValue::StrArray(vec!["hi".into()])),
            _ => None,
        };
        let a = read_group("/A/", "A", &get).unwrap();
        let texts: Vec<&str> = a.iter().map(|a| a.text.as_str()).collect();
        assert_eq!(texts, ["new", "old", "hi"]);
        assert_eq!(counts(&a), [2, 0, 1]);
        assert!(read_group("/B/", "B", &get).is_none());
    }

    #[test]
    fn a_watch_sees_new_errors_only_and_keeps_the_last_alerts() {
        let mut w = Watch::default();
        assert!(
            !w.update(Some(vec![alert(Severity::Error, "x")]), true)
                .new_error,
            "already up on the first look"
        );
        assert!(
            !w.update(
                Some(vec![
                    alert(Severity::Error, "x"),
                    alert(Severity::Warning, "w")
                ]),
                true
            )
            .new_error
        );
        assert!(
            w.update(Some(vec![alert(Severity::Error, "y")]), true)
                .new_error
        );
        let gone = w.update(None, true);
        assert_eq!(
            (gone.freshness, gone.alerts.len()),
            (Freshness::NotPublishedNow, 1)
        );
        let down = w.update(None, false);
        assert_eq!(down.freshness, Freshness::Disconnected);
        assert_eq!(
            Watch::default().update(None, true).freshness,
            Freshness::NeverPublished
        );
    }

    #[test]
    fn the_most_urgent_sign_grows() {
        let size = |counts, s| {
            sign_places(counts)
                .into_iter()
                .find(|p| p.0 == s)
                .unwrap()
                .2
        };
        assert!(size([1, 2, 0], Severity::Error) > 2.0 * size([1, 2, 0], Severity::Warning));
        assert!(size([0, 1, 1], Severity::Warning) > 2.0 * size([0, 1, 1], Severity::Error));
        assert_eq!(
            size([0, 0, 3], Severity::Info),
            size([0, 0, 3], Severity::Error)
        );
    }

    #[test]
    fn text_goes_beside_a_wide_widget_and_below_a_tall_one() {
        let area = |w, h| Area {
            x0: 0.0,
            y0: 0.0,
            x1: w,
            y1: h,
        };
        let (c, t) = layout(area(400.0, 100.0), true, "auto");
        assert!(t.unwrap().x0 > c.x1);
        let (c, t) = layout(area(100.0, 300.0), true, "auto");
        assert!(t.unwrap().y0 > c.y1);
        assert_eq!(layout(area(10.0, 10.0), false, "auto").1, None);
    }

    #[test]
    fn the_summary_matches_the_built_in_widgets() {
        assert_eq!(
            summary([1, 0, 2], Freshness::Current),
            "1 error, 0 warnings, 2 infos"
        );
        assert_eq!(
            summary([0, 1, 1], Freshness::Disconnected),
            "0 errors, 1 warning, 1 info (disconnected)"
        );
    }

    #[test]
    fn fading_scales_only_the_opacity() {
        let c = color(10, 20, 30);
        assert_eq!(fade(c, 0.5), Color { a: 128, ..c });
    }
}
