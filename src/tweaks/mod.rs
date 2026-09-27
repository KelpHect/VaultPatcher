//! Data model for config tweaks.
//!
//! A game describes its tweaks as static tables (see `games::bl2::tweaks`).
//! Each tweak pairs a UI control with a binding that knows how to read and
//! write the value in the game's ini files, so pages never deal with ini keys
//! directly and new tweaks are a single table entry.

pub mod config;

use std::fmt::Write as _;

pub use config::ConfigSet;

use crate::core::ini::parse_bool;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Num(f64),
    /// One of the tweak's `Control::Choice` option values.
    Choice(&'static str),
    /// Something in the file that none of our options describe.
    Unknown(String),
}

impl Value {
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn display(&self, control: &Control) -> String {
        match (self, control) {
            (Value::Bool(b), _) => if *b { "On" } else { "Off" }.into(),
            (Value::Num(n), c @ Control::Slider { .. }) => c.slider_text(*n),
            (Value::Num(n), _) => format!("{n}"),
            (Value::Choice(v), Control::Choice(options)) => options
                .iter()
                .find(|o| o.value == *v)
                .map(|o| o.label.to_string())
                .unwrap_or_else(|| v.to_string()),
            (Value::Choice(v), _) => v.to_string(),
            (Value::Unknown(s), _) => format!("Custom ({s})"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Opt {
    pub value: &'static str,
    pub label: &'static str,
}

pub const fn opt(value: &'static str, label: &'static str) -> Opt {
    Opt { value, label }
}

#[derive(Clone, Copy, Debug)]
pub enum Control {
    Toggle,
    Slider {
        min: f64,
        max: f64,
        step: f64,
        /// Digits written to the file (0 writes an integer). The UI shows
        /// as many decimals as the step needs.
        decimals: u8,
        unit: &'static str,
        /// Values shown as words instead of numbers (0 → "Unlimited").
        labels: &'static [(f64, &'static str)],
        /// The range the description recommends, drawn as a band on the rail.
        recommended: Option<(f64, f64)>,
    },
    Choice(&'static [Opt]),
}

impl Control {
    /// Decimals shown for a slider value: from the step (0.05 → 2, 0.1 → 1,
    /// 1 → 0), or the file's decimals for continuous sliders.
    pub fn shown_decimals(&self) -> usize {
        match *self {
            Control::Slider { step, .. } if step > 0.0 => step_decimals(step),
            Control::Slider { decimals, .. } => decimals as usize,
            _ => 0,
        }
    }

    /// Snaps `n` to the slider's step and keeps it in range.
    pub fn clamp_num(&self, n: f64) -> f64 {
        match *self {
            Control::Slider { min, max, step, .. } => {
                let snapped = if step > 0.0 { ((n - min) / step).round() * step + min } else { n };
                (snapped.clamp(min, max) * 1e6).round() / 1e6
            }
            _ => n,
        }
    }

    /// The word for `n` if the slider names it (a sentinel such as "Off").
    pub fn slider_label(&self, n: f64) -> Option<&'static str> {
        match *self {
            Control::Slider { labels, .. } => labels.iter().find(|(v, _)| (v - n).abs() < 1e-9).map(|(_, l)| *l),
            _ => None,
        }
    }

    /// `n` as a NumberBox shows it: the sentinel word, or the formatted
    /// number without its unit.
    pub fn number_text(&self, n: f64) -> String {
        self.slider_label(n).map_or_else(|| format_number(n, self.shown_decimals()), str::to_string)
    }

    /// `n` with its unit: "110°", "1.00×", "10,000", "62 fps", or a word.
    pub fn slider_text(&self, n: f64) -> String {
        match (self.slider_label(n), self) {
            (Some(label), _) => label.to_string(),
            (None, Control::Slider { unit, .. }) => with_unit(format_number(n, self.shown_decimals()), unit),
            (None, _) => format!("{n}"),
        }
    }

    /// Reads what someone typed into a NumberBox: a sentinel word, or a
    /// number with optional thousands separators and unit.
    pub fn parse_number(&self, text: &str) -> Option<f64> {
        let Control::Slider { unit, labels, .. } = *self else { return None };
        let text = text.trim();
        if let Some((v, _)) = labels.iter().find(|(_, l)| l.eq_ignore_ascii_case(text)) {
            return Some(*v);
        }
        let without_unit = if unit.is_empty() { text } else { text.trim_end_matches(unit) };
        let cleaned: String = without_unit
            .trim_end_matches(['×', 'x', 'X', '°', '%'])
            .chars()
            .filter(|c| !matches!(c, ',' | ' ' | '\u{a0}' | '\u{202f}'))
            .map(|c| if c == '\u{2212}' { '-' } else { c })
            .collect();
        cleaned.parse::<f64>().ok().filter(|n| n.is_finite())
    }
}

/// Decimals a step needs: 0.05 → 2, 0.1 → 1, 5 → 0.
pub fn step_decimals(step: f64) -> usize {
    let mut s = step.abs();
    let mut d = 0;
    while d < 6 && (s - s.round()).abs() > 1e-6 {
        s *= 10.0;
        d += 1;
    }
    d
}

/// A number with `decimals` places and thousands separators ("10,000").
pub fn format_number(n: f64, decimals: usize) -> String {
    let fixed = format!("{:.*}", decimals, n.abs());
    let (int, frac) = match fixed.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (fixed.as_str(), None),
    };
    let mut out = String::with_capacity(fixed.len() + 4);
    if n < 0.0 && fixed.chars().any(|c| c.is_ascii_digit() && c != '0') {
        out.push('-');
    }
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(f) = frac {
        let _ = write!(out, ".{f}");
    }
    out
}

/// Symbols hug the number ("110°", "100%", "1.00×"); words get a space
/// ("62 fps", "600 MB").
pub fn with_unit(number: String, unit: &str) -> String {
    match unit {
        "" => number,
        "°" | "%" | "×" => number + unit,
        _ => format!("{number} {unit}"),
    }
}

/// Two sliders that bound one range, shown together as a RangeSlider.
#[derive(Clone, Copy)]
pub struct RangePair {
    pub min: &'static str,
    pub max: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

/// Keeps a range ordered after one end moved: the moved end stops at the
/// other one, as the thumbs of a RangeSlider can't cross.
pub fn order_range(min: f64, max: f64, moved_min: bool) -> (f64, f64) {
    if min <= max {
        (min, max)
    } else if moved_min {
        (max, max)
    } else {
        (min, min)
    }
}

/// How a boolean is spelled in the file. `on`/`off` may be swapped to model
/// "Disable X" style toggles over an "enable X" key.
#[derive(Clone, Copy, Debug)]
pub struct BoolFmt {
    pub on: &'static str,
    pub off: &'static str,
}

pub const TF: BoolFmt = BoolFmt { on: "True", off: "False" };
pub const TF_UPPER: BoolFmt = BoolFmt { on: "TRUE", off: "FALSE" };
pub const TF_LOWER: BoolFmt = BoolFmt { on: "true", off: "false" };
pub const TF_INV: BoolFmt = BoolFmt { on: "False", off: "True" };
pub const TF_LOWER_INV: BoolFmt = BoolFmt { on: "false", off: "true" };

#[derive(Clone, Copy, Debug)]
pub struct Key {
    /// Logical file id declared by the game (e.g. "engine").
    pub file: &'static str,
    pub section: &'static str,
    pub key: &'static str,
}

pub const fn key(file: &'static str, section: &'static str, key: &'static str) -> Key {
    Key { file, section, key }
}

pub type ReadFn = fn(&ConfigSet) -> Option<Value>;
pub type WriteFn = fn(&mut ConfigSet, &Value);

#[derive(Clone, Copy)]
pub enum Binding {
    /// A single key. The first key is read; all keys are written.
    Keys(&'static [Key], BoolFmt),
    /// Hand-written logic for tweaks that aren't a plain key.
    Custom { read: ReadFn, write: WriteFn },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Impact {
    None,
    Low,
    Medium,
    High,
}


#[derive(Clone, Copy, Debug, Default)]
pub struct Flags {
    /// Community-reported but not universally reliable; shown with a warning.
    pub experimental: bool,
    /// The in-game options menu rewrites this key; lock the file to keep it.
    pub menu_managed: bool,
}

pub const NONE: Flags = Flags {
    experimental: false,
    menu_managed: false,
};
pub const MENU: Flags = Flags {
    experimental: false,
    menu_managed: true,
};
pub const EXPERIMENTAL: Flags = Flags {
    experimental: true,
    menu_managed: false,
};

#[derive(Clone, Copy)]
pub struct Tweak {
    pub id: &'static str,
    pub category: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub control: Control,
    pub binding: Binding,
    pub default: DefaultValue,
    pub impact: Impact,
    pub flags: Flags,
}

/// `Value` isn't const-constructible for every variant, so tables use this.
#[derive(Clone, Copy, Debug)]
pub enum DefaultValue {
    B(bool),
    N(f64),
    C(&'static str),
}

impl DefaultValue {
    pub fn to_value(self) -> Value {
        match self {
            DefaultValue::B(b) => Value::Bool(b),
            DefaultValue::N(n) => Value::Num(n),
            DefaultValue::C(c) => Value::Choice(c),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Category {
    pub id: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
}

#[derive(Clone, Copy)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub rarity: crate::theme::Rarity,
    pub description: &'static str,
    pub values: &'static [(&'static str, DefaultValue)],
}

impl Tweak {
    /// Names slider values that mean something special (0 → "Off").
    pub const fn labels(self, labels: &'static [(f64, &'static str)]) -> Tweak {
        match self.control {
            Control::Slider { min, max, step, decimals, unit, recommended, .. } => {
                Tweak { control: Control::Slider { min, max, step, decimals, unit, labels, recommended }, ..self }
            }
            _ => self,
        }
    }

    /// Marks the range the description recommends.
    pub const fn recommended(self, low: f64, high: f64) -> Tweak {
        match self.control {
            Control::Slider { min, max, step, decimals, unit, labels, .. } => {
                Tweak { control: Control::Slider { min, max, step, decimals, unit, labels, recommended: Some((low, high)) }, ..self }
            }
            _ => self,
        }
    }

    pub fn read(&self, config: &ConfigSet) -> Option<Value> {
        match self.binding {
            Binding::Keys(keys, fmt) => {
                let raw = config.get(keys.first()?)?;
                Some(self.parse_raw(raw, fmt))
            }
            Binding::Custom { read, .. } => read(config),
        }
    }

    pub fn write(&self, config: &mut ConfigSet, value: &Value) {
        match self.binding {
            Binding::Keys(keys, fmt) => {
                let Some(raw) = self.format_raw(value, fmt) else {
                    return;
                };
                // Mirror copies (e.g. the launcher's ini) are only kept in sync
                // when they exist; the first key is always written.
                for (i, k) in keys.iter().enumerate() {
                    if i == 0 || config.file(k.file).is_some_and(|f| f.exists) {
                        config.set(k, &raw);
                    }
                }
            }
            Binding::Custom { write, .. } => write(config, value),
        }
    }

    fn parse_raw(&self, raw: &str, fmt: BoolFmt) -> Value {
        match self.control {
            Control::Toggle => match (parse_bool(raw), parse_bool(fmt.on)) {
                (Some(v), Some(on)) => Value::Bool(v == on),
                _ => Value::Unknown(raw.to_string()),
            },
            Control::Slider { .. } => raw
                .trim()
                .trim_end_matches('f')
                .parse::<f64>()
                .map(Value::Num)
                .unwrap_or_else(|_| Value::Unknown(raw.to_string())),
            Control::Choice(options) => match_choice(options, raw),
        }
    }

    fn format_raw(&self, value: &Value, fmt: BoolFmt) -> Option<String> {
        Some(match (value, self.control) {
            (Value::Bool(b), _) => if *b { fmt.on } else { fmt.off }.to_string(),
            (Value::Num(n), Control::Slider { decimals: 0, .. }) => format!("{}", n.round() as i64),
            (Value::Num(n), _) => format!("{n:.6}"),
            (Value::Choice(c), _) => c.to_string(),
            (Value::Unknown(_), _) => return None,
        })
    }

    pub fn clamp(&self, value: Value) -> Value {
        match (value, self.control) {
            (Value::Num(n), c) => Value::Num(c.clamp_num(n)),
            (v, _) => v,
        }
    }
}

/// Matches a raw ini value against the option list. Numeric options compare
/// numerically so `2.000000` selects the `2` option.
pub fn match_choice(options: &'static [Opt], raw: &str) -> Value {
    let raw = raw.trim();
    for o in options {
        if o.value.eq_ignore_ascii_case(raw) {
            return Value::Choice(o.value);
        }
        if let (Ok(a), Ok(b)) = (o.value.parse::<f64>(), raw.parse::<f64>())
            && (a - b).abs() < 1e-6 {
                return Value::Choice(o.value);
            }
    }
    Value::Unknown(raw.to_string())
}

// ---- table builders ------------------------------------------------------------
// Game tweak tables are long; these keep each entry to the fields that vary.

#[allow(clippy::too_many_arguments)]
pub const fn toggle(
    id: &'static str,
    category: &'static str,
    label: &'static str,
    description: &'static str,
    keys: &'static [Key],
    fmt: BoolFmt,
    default: bool,
    impact: Impact,
    flags: Flags,
) -> Tweak {
    Tweak {
        id,
        category,
        label,
        description,
        control: Control::Toggle,
        binding: Binding::Keys(keys, fmt),
        default: DefaultValue::B(default),
        impact,
        flags,
    }
}

#[allow(clippy::too_many_arguments)]
pub const fn slider(
    id: &'static str,
    category: &'static str,
    label: &'static str,
    description: &'static str,
    keys: &'static [Key],
    range: (f64, f64, f64),
    decimals: u8,
    unit: &'static str,
    default: f64,
    impact: Impact,
    flags: Flags,
) -> Tweak {
    Tweak {
        id,
        category,
        label,
        description,
        control: Control::Slider {
            min: range.0,
            max: range.1,
            step: range.2,
            decimals,
            unit,
            labels: &[],
            recommended: None,
        },
        binding: Binding::Keys(keys, TF),
        default: DefaultValue::N(default),
        impact,
        flags,
    }
}

#[allow(clippy::too_many_arguments)]
pub const fn choice(
    id: &'static str,
    category: &'static str,
    label: &'static str,
    description: &'static str,
    keys: &'static [Key],
    options: &'static [Opt],
    default: &'static str,
    impact: Impact,
    flags: Flags,
) -> Tweak {
    Tweak {
        id,
        category,
        label,
        description,
        control: Control::Choice(options),
        binding: Binding::Keys(keys, TF),
        default: DefaultValue::C(default),
        impact,
        flags,
    }
}

#[allow(clippy::too_many_arguments)]
pub const fn custom(
    id: &'static str,
    category: &'static str,
    label: &'static str,
    description: &'static str,
    control: Control,
    read: ReadFn,
    write: WriteFn,
    default: DefaultValue,
    impact: Impact,
    flags: Flags,
) -> Tweak {
    Tweak {
        id,
        category,
        label,
        description,
        control,
        binding: Binding::Custom { read, write },
        default,
        impact,
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn slider_control(step: f64, unit: &'static str, labels: &'static [(f64, &'static str)]) -> Control {
        Control::Slider { min: -1.0, max: 100_000.0, step, decimals: 0, unit, labels, recommended: None }
    }

    #[test]
    fn decimals_come_from_the_step() {
        assert_eq!(step_decimals(1.0), 0);
        assert_eq!(step_decimals(250.0), 0);
        assert_eq!(step_decimals(0.5), 1);
        assert_eq!(step_decimals(0.1), 1);
        assert_eq!(step_decimals(0.05), 2);
        assert_eq!(step_decimals(0.001), 3);
    }

    #[test]
    fn numbers_get_separators_and_units() {
        assert_eq!(format_number(10_000.0, 0), "10,000");
        assert_eq!(format_number(7_000.0, 0), "7,000");
        assert_eq!(format_number(999.0, 0), "999");
        assert_eq!(format_number(1_234_567.5, 1), "1,234,567.5");
        assert_eq!(format_number(-0.0001, 2), "0.00");
        assert_eq!(format_number(-1.0, 0), "-1");
        assert_eq!(slider_control(1.0, "°", &[]).slider_text(110.0), "110°");
        assert_eq!(slider_control(0.05, "×", &[]).slider_text(1.0), "1.00×");
        assert_eq!(slider_control(5.0, "%", &[]).slider_text(100.0), "100%");
        assert_eq!(slider_control(1.0, "fps", &[]).slider_text(62.0), "62 fps");
        assert_eq!(slider_control(100.0, "MB", &[]).slider_text(1200.0), "1,200 MB");
        assert_eq!(slider_control(0.001, "", &[]).slider_text(0.012), "0.012");
        assert_eq!(Value::Num(10_000.0).display(&slider_control(5000.0, "", &[])), "10,000");
    }

    #[test]
    fn sentinel_values_show_as_words() {
        let c = slider_control(250.0, "", &[(0.0, "Unlimited")]);
        assert_eq!(c.slider_label(0.0), Some("Unlimited"));
        assert_eq!(c.slider_label(250.0), None);
        assert_eq!(c.slider_text(0.0), "Unlimited");
        assert_eq!(c.number_text(0.0), "Unlimited");
        assert_eq!(c.number_text(5000.0), "5,000");
        let lod = slider_control(1.0, "", &[(-1.0, "Force high")]);
        assert_eq!(lod.slider_text(-1.0), "Force high");
        assert_eq!(lod.slider_text(2.0), "2");
    }

    #[test]
    fn typed_numbers_parse() {
        let c = slider_control(250.0, "MB", &[(0.0, "Off")]);
        assert_eq!(c.parse_number("off"), Some(0.0));
        assert_eq!(c.parse_number(" 1,500 "), Some(1500.0));
        assert_eq!(c.parse_number("600 MB"), Some(600.0));
        assert_eq!(c.parse_number("\u{2212}1"), Some(-1.0));
        assert_eq!(c.parse_number("abc"), None);
        assert_eq!(c.parse_number(""), None);
        let fov = slider_control(1.0, "°", &[]);
        assert_eq!(fov.parse_number("108°"), Some(108.0));
        let mult = slider_control(0.05, "×", &[]);
        assert_eq!(mult.parse_number("1.25x"), Some(1.25));
        assert_eq!(Control::Toggle.parse_number("1"), None);
    }

    #[test]
    fn range_ends_do_not_cross() {
        assert_eq!(order_range(22.0, 62.0, true), (22.0, 62.0));
        assert_eq!(order_range(80.0, 62.0, true), (62.0, 62.0));
        assert_eq!(order_range(22.0, 10.0, false), (22.0, 22.0));
    }

    #[test]
    fn builders_attach_labels_and_ranges() {
        const T: Tweak = slider("t", "c", "T", "", &[], (0.0, 10.0, 1.0), 0, "", 0.0, Impact::None, NONE)
            .labels(&[(0.0, "Off")])
            .recommended(2.0, 4.0);
        let Control::Slider { labels, recommended, .. } = T.control else { panic!("not a slider") };
        assert_eq!(labels, &[(0.0, "Off")]);
        assert_eq!(recommended, Some((2.0, 4.0)));
    }
}
