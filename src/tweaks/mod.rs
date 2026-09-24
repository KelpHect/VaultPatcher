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
            (Value::Num(n), Control::Slider { decimals, unit, .. }) => {
                let mut s = format!("{n:.*}", *decimals as usize);
                if !unit.is_empty() {
                    let _ = write!(s, " {unit}");
                }
                s
            }
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
        decimals: u8,
        unit: &'static str,
    },
    Choice(&'static [Opt]),
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
            (Value::Num(n), Control::Slider { min, max, step, .. }) => {
                let snapped = if step > 0.0 { ((n - min) / step).round() * step + min } else { n };
                Value::Num((snapped.clamp(min, max) * 1e6).round() / 1e6)
            }
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
