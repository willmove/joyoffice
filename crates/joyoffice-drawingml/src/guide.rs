//! DrawingML geometry guides, ECMA-376 Part 1 §20.1.9.11. The same
//! operations serve compiled presets and named guides in imported shapes.

use std::collections::HashMap;

pub const MAX_GUIDES: usize = 10_000;
const MAX_MAGNITUDE: f64 = 1.0e15;

pub fn finite(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(-MAX_MAGNITUDE, MAX_MAGNITUDE)
    } else {
        0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Value,
    Product,
    Sum,
    AddDivide,
    Select,
    Absolute,
    Direction,
    CosineOfDirection,
    SineOfDirection,
    Cosine,
    Sine,
    Tangent,
    Maximum,
    Minimum,
    Length,
    Pin,
    SquareRoot,
}

impl Operation {
    pub fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "val" => Self::Value,
            "*/" => Self::Product,
            "+-" => Self::Sum,
            "+/" => Self::AddDivide,
            "?:" => Self::Select,
            "abs" => Self::Absolute,
            "at2" => Self::Direction,
            "cat2" => Self::CosineOfDirection,
            "sat2" => Self::SineOfDirection,
            "cos" => Self::Cosine,
            "sin" => Self::Sine,
            "tan" => Self::Tangent,
            "max" => Self::Maximum,
            "min" => Self::Minimum,
            "mod" => Self::Length,
            "pin" => Self::Pin,
            "sqrt" => Self::SquareRoot,
            _ => return None,
        })
    }

    pub fn apply(self, [x, y, z]: [f64; 3]) -> f64 {
        use Operation::*;
        let divide = |n| if z == 0.0 { 0.0 } else { n / z };
        let radians = || (y / 60_000.).to_radians();
        finite(match self {
            Value => x,
            Product => divide(x * y),
            Sum => x + y - z,
            AddDivide => divide(x + y),
            Select => {
                if x > 0. {
                    y
                } else {
                    z
                }
            }
            Absolute => x.abs(),
            Direction => y.atan2(x).to_degrees() * 60_000.,
            CosineOfDirection => x * z.atan2(y).cos(),
            SineOfDirection => x * z.atan2(y).sin(),
            Cosine => x * radians().cos(),
            Sine => x * radians().sin(),
            Tangent => x * radians().tan(),
            Maximum => x.max(y),
            Minimum => x.min(y),
            Length => x.hypot(y).hypot(z),
            Pin => {
                if y < x {
                    x
                } else {
                    y.min(z)
                }
            }
            SquareRoot => x.max(0.0).sqrt(),
        })
    }
}

/// Built-in distances are resolved without allocating entries for each new shape.
#[derive(Clone, Copy, Debug)]
pub struct GuideSpace {
    pub width: f64,
    pub height: f64,
}

impl GuideSpace {
    pub fn new(width: f64, height: f64) -> Self {
        Self {
            width: finite(width),
            height: finite(height),
        }
    }

    pub fn builtin(self, key: &str) -> Option<f64> {
        Builtin::parse(key).map(|variable| variable.evaluate(self))
    }
}

/// Named shape distances and angles in a compiled or dynamically read guide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    Constant(i32),
    Width(u8),
    Height(u8),
    Short(u8),
    Long,
}

impl Builtin {
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "l" | "t" => Self::Constant(0),
            "r" | "w" => Self::Width(1),
            "b" | "h" => Self::Height(1),
            "hc" => Self::Width(2),
            "vc" => Self::Height(2),
            "ss" => Self::Short(1),
            "ls" => Self::Long,
            "cd2" => Self::Constant(10_800_000),
            "cd4" => Self::Constant(5_400_000),
            "cd8" => Self::Constant(2_700_000),
            "3cd4" => Self::Constant(16_200_000),
            "3cd8" => Self::Constant(8_100_000),
            "5cd8" => Self::Constant(13_500_000),
            "7cd8" => Self::Constant(18_900_000),
            _ => {
                let divisor = |s: &str| match s {
                    "2" => Some(2),
                    "3" => Some(3),
                    "4" => Some(4),
                    "5" => Some(5),
                    "6" => Some(6),
                    "8" => Some(8),
                    "10" => Some(10),
                    "12" => Some(12),
                    "16" => Some(16),
                    "32" => Some(32),
                    _ => None,
                };
                if let Some(n) = name.strip_prefix("wd") {
                    Self::Width(divisor(n)?)
                } else if let Some(n) = name.strip_prefix("hd") {
                    Self::Height(divisor(n)?)
                } else {
                    Self::Short(divisor(name.strip_prefix("ssd")?)?)
                }
            }
        })
    }
    pub fn evaluate(self, space: GuideSpace) -> f64 {
        let GuideSpace { width, height } = space;
        match self {
            Self::Constant(v) => f64::from(v),
            Self::Width(d) => width / f64::from(d.max(1)),
            Self::Height(d) => height / f64::from(d.max(1)),
            Self::Short(d) => width.min(height) / f64::from(d.max(1)),
            Self::Long => width.max(height),
        }
    }
}

pub struct Guides {
    space: GuideSpace,
    values: HashMap<Box<str>, f64>,
    additional_names: usize,
}

impl Guides {
    pub fn new(w: f64, h: f64) -> Self {
        Self {
            space: GuideSpace::new(w, h),
            values: HashMap::new(),
            additional_names: 0,
        }
    }
    pub fn resolve(&self, token: &str) -> Option<f64> {
        let key = token.trim();
        self.values
            .get(key)
            .copied()
            .or_else(|| self.space.builtin(key))
            .or_else(|| key.parse().ok().map(finite))
    }
    pub fn get(&self, token: &str) -> f64 {
        self.resolve(token).unwrap_or(0.)
    }

    pub fn eval(&self, expression: &str) -> f64 {
        let mut tokens = expression.split_whitespace();
        let Some(op) = tokens.next().and_then(Operation::parse) else {
            return 0.0;
        };
        let mut values = [0.; 3];
        for (slot, token) in values.iter_mut().zip(tokens) {
            *slot = self.get(token);
        }
        op.apply(values)
    }

    pub fn set(&mut self, key: &str, value: f64) {
        // 47 built-in bindings also occupied slots in existing Deck documents.
        // A user guide may shadow a built-in, including a zero-valued one.
        if self.additional_names < MAX_GUIDES - 47 {
            if let Some(slot) = self.values.get_mut(key) {
                *slot = finite(value);
            } else {
                self.additional_names += usize::from(self.space.builtin(key).is_none());
                self.values.insert(key.into(), finite(value));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_references_and_division_boundaries() {
        let mut guides = Guides::new(160.0, 80.0);
        guides.set("v", 32.0);
        for (formula, expected) in [
            ("val wd16", 10.),
            ("*/ v 3 2", 48.),
            ("+- v 5 7", 30.),
            ("+/ 8 12 5", 4.),
            ("?: 0 10 7", 7.),
            ("abs -2", 2.),
            ("at2 0 1", 5_400_000.),
            ("cat2 10 3 4", 6.),
            ("sat2 10 3 4", 8.),
            ("cos 10 cd2", -10.),
            ("sin 10 cd4", 10.),
            ("tan 9 cd8", 9.),
            ("max 3 2", 3.),
            ("min 3 2", 2.),
            ("mod 2 3 6", 7.),
            ("pin 2 8 6", 6.),
            ("sqrt 25", 5.),
            ("*/ 2 5 0", 0.),
            ("sqrt -1", 0.),
            ("val hd8", 10.),
            ("val ssd32", 2.5),
            ("val ls", 160.),
            ("val 7cd8", 18_900_000.),
            ("unknown 1", 0.),
        ] {
            assert!((guides.eval(formula) - expected).abs() < 1e-8, "{formula}");
        }
        assert_eq!(guides.resolve("unbound"), None);
        guides.set("w", guides.eval("*/ w 1 2"));
        assert_eq!(guides.eval("val w"), 80.);
        assert_eq!(guides.eval("val wd2"), 80.);
        guides.set("v", guides.eval("+- v w 2"));
        assert_eq!(guides.get("v"), 110.);
        assert_eq!(guides.get("1e100"), MAX_MAGNITUDE);
        assert_eq!(guides.get("NaN"), 0.);
    }

    #[test]
    fn imported_binding_limit_includes_builtins_and_accepts_shadows() {
        let mut guides = Guides::new(20., 40.);
        guides.set("w", 22.);
        for i in 0..MAX_GUIDES {
            guides.set(&format!("v{i}"), i as f64);
        }
        assert_eq!(guides.get("w"), 22.);
        assert_eq!(guides.resolve("v9952"), Some(9952.));
        assert_eq!(guides.resolve("v9953"), None);
    }
}
