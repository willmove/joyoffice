//! Editable drawing content, independent of any renderer or package implementation.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmartArtLayout {
    Process,
    Hierarchy,
    Cycle,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimelineLevel {
    Years,
    Quarters,
    Months,
    Days,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DrawingObject {
    SmartArt {
        layout: SmartArtLayout,
        nodes: Vec<String>,
    },
    WordArt {
        warp: String,
        shadow: bool,
    },
    Equation {
        expression: MathExpr,
    },
    Embedded {
        name: String,
        mime: String,
        /// Original file bytes, shared across history snapshots. Never executed by renderers.
        #[serde(with = "crate::b64::shared")]
        data: std::sync::Arc<Vec<u8>>,
    },
    Timeline {
        pivot: String,
        column: String,
        level: TimelineLevel,
        /// Inclusive Excel serial-date range; None means clear filter.
        selection: Option<(i32, i32)>,
        #[serde(default)]
        scroll_start: Option<i32>,
    },
}

/// Office Math's common built-up structures. Linear input is kept in Shape::text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MathExpr {
    Text(String),
    Row(Vec<MathExpr>),
    Fraction(Box<MathExpr>, Box<MathExpr>),
    Radical(Box<MathExpr>),
    Superscript(Box<MathExpr>, Box<MathExpr>),
    Subscript(Box<MathExpr>, Box<MathExpr>),
}

impl MathExpr {
    /// Bounded linear syntax: \frac{a}{b}, \sqrt{x}, x^{2}, x_{i}; nested groups allowed.
    pub fn parse(source: &str) -> Result<Self, String> {
        if source.chars().count() > 4096 {
            return Err("An equation is limited to 4096 characters".into());
        }
        let mut p = Parser { chars: source.chars().collect(), at: 0, nodes: 0 };
        let expr = p.row(false, 0)?;
        if p.at != p.chars.len() {
            return Err("Unexpected closing brace".into());
        }
        Ok(expr.normalized())
    }
    pub fn linear(&self) -> String {
        match self {
            Self::Text(t) => t.clone(),
            Self::Row(v) => v.iter().map(Self::linear).collect(),
            Self::Fraction(a, b) => format!("\\frac{{{}}}{{{}}}", a.linear(), b.linear()),
            Self::Radical(a) => format!("\\sqrt{{{}}}", a.linear()),
            Self::Superscript(a, b) => format!("{}^{{{}}}", a.linear(), b.linear()),
            Self::Subscript(a, b) => format!("{}_{{{}}}", a.linear(), b.linear()),
        }
    }
    /// Transparent Office Math containers and grouping braces share one canonical tree.
    pub fn normalized(self) -> Self {
        match self {
            Self::Row(v) => {
                let mut row = Vec::new();
                for e in v {
                    match e.normalized() {
                        Self::Row(v) => row.extend(v),
                        e => row.push(e),
                    }
                }
                if row.len() == 1 { row.remove(0) } else { Self::Row(row) }
            }
            Self::Fraction(a, b) => Self::Fraction(Box::new(a.normalized()), Box::new(b.normalized())),
            Self::Radical(a) => Self::Radical(Box::new(a.normalized())),
            Self::Superscript(a, b) => Self::Superscript(Box::new(a.normalized()), Box::new(b.normalized())),
            Self::Subscript(a, b) => Self::Subscript(Box::new(a.normalized()), Box::new(b.normalized())),
            e => e,
        }
    }
}
struct Parser {
    chars: Vec<char>,
    at: usize,
    nodes: usize,
}
impl Parser {
    fn row(&mut self, grouped: bool, depth: usize) -> Result<MathExpr, String> {
        if depth > 32 {
            return Err("Equation nesting exceeds 32 levels".into());
        }
        let mut row = Vec::new();
        while self.at < self.chars.len() && self.chars[self.at] != '}' {
            let mut atom = self.atom(depth + 1)?;
            while let Some(c @ ('^' | '_')) = self.chars.get(self.at).copied() {
                self.at += 1;
                let script = self.atom(depth + 1)?;
                atom = if c == '^' {
                    MathExpr::Superscript(Box::new(atom), Box::new(script))
                } else {
                    MathExpr::Subscript(Box::new(atom), Box::new(script))
                };
            }
            row.push(atom);
        }
        if grouped {
            if self.chars.get(self.at) != Some(&'}') {
                return Err("Missing closing brace".into());
            }
            self.at += 1;
        }
        Ok(MathExpr::Row(row))
    }
    fn atom(&mut self, depth: usize) -> Result<MathExpr, String> {
        self.nodes += 1;
        if self.nodes > 8192 || depth > 32 {
            return Err("Equation complexity limit exceeded".into());
        }
        let c = *self.chars.get(self.at).ok_or("Missing equation argument")?;
        self.at += 1;
        if c == '{' {
            return self.row(true, depth);
        }
        if matches!(c, '}' | '^' | '_') {
            return Err("Missing equation base or argument".into());
        }
        if c == '\\' {
            let start = self.at;
            while self.chars.get(self.at).is_some_and(|c| c.is_ascii_alphabetic()) {
                self.at += 1;
            }
            let name: String = self.chars[start..self.at].iter().collect();
            return match name.as_str() {
                "frac" => {
                    let a = self.atom(depth + 1)?;
                    let b = self.atom(depth + 1)?;
                    Ok(MathExpr::Fraction(Box::new(a), Box::new(b)))
                }
                "sqrt" => Ok(MathExpr::Radical(Box::new(self.atom(depth + 1)?))),
                "alpha" => Ok(MathExpr::Text("α".into())),
                "beta" => Ok(MathExpr::Text("β".into())),
                "pi" => Ok(MathExpr::Text("π".into())),
                "sum" => Ok(MathExpr::Text("∑".into())),
                "int" => Ok(MathExpr::Text("∫".into())),
                "infty" => Ok(MathExpr::Text("∞".into())),
                _ => Err(format!("Unsupported equation keyword: \\{name}")),
            };
        }
        Ok(MathExpr::Text(c.to_string()))
    }
}
