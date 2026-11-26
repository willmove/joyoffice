//! Themes: colour scheme, font scheme and format scheme (the fill/line/effect "style matrix").

use joydeck_color::{ColorScheme, ColorTransform, Rgba, SchemeSlot};
use serde::{Deserialize, Serialize};

use crate::style::{
    ColorRef, Effects, Fill, Glow, Gradient, GradientShape, GradientStop, Line, Shadow,
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FontSet {
    pub latin: String,
    pub ea: String,
    pub cs: String,
    /// Per-script typefaces (`a:font script="Arab" typeface="…"`), in document order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<ScriptFont>,
}

/// A theme font for one script (ISO 15924 tag, e.g. `Arab`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScriptFont {
    pub script: String,
    pub typeface: String,
}

impl FontSet {
    /// The typeface the theme gives `script` (`"Arab"`), if any.
    pub fn script(&self, script: &str) -> Option<&str> {
        self.scripts
            .iter()
            .find(|f| f.script.eq_ignore_ascii_case(script) && !f.typeface.is_empty())
            .map(|f| f.typeface.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FontScheme {
    pub name: String,
    /// Headings.
    pub major: FontSet,
    /// Body.
    pub minor: FontSet,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FormatScheme {
    pub name: String,
    /// Subtle, moderate, intense (style index 1–3).
    pub fills: Vec<Fill>,
    pub lines: Vec<Line>,
    pub effects: Vec<Effects>,
    /// Background fills (index 1001–1003).
    pub bg_fills: Vec<Fill>,
}

impl FormatScheme {
    /// Style references address one-based shape and background fill lists.
    pub(crate) fn fill(&self, idx: u32) -> Option<&Fill> {
        if idx >= 1001 {
            self.bg_fills.get((idx - 1001) as usize)
        } else {
            self.fills.get(idx.checked_sub(1)? as usize)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Theme {
    pub name: String,
    pub colors: ColorScheme,
    pub fonts: FontScheme,
    pub format: FormatScheme,
    /// Raw theme XML extras kept for round-trip (object defaults, extra colour lists).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_extra: Option<String>,
}

impl Default for Theme {
    fn default() -> Self {
        builtin_themes()
            .into_iter()
            .next()
            .unwrap_or_else(|| Theme {
                name: "现代商务".into(),
                colors: ColorScheme {
                    name: "现代商务".into(),
                    colors: [Rgba::BLACK; 12],
                },
                fonts: FontScheme::default(),
                format: FormatScheme::default(),
                raw_extra: None,
            })
    }
}

impl Theme {
    /// Resolve a theme font reference (`+mj-lt`, `+mn-ea`…) to a family name.
    pub fn font(&self, name: &str) -> String {
        match name {
            "+mj-lt" => self.fonts.major.latin.clone(),
            "+mn-lt" => self.fonts.minor.latin.clone(),
            "+mj-ea" => self.fonts.major.ea.clone(),
            "+mn-ea" => self.fonts.minor.ea.clone(),
            "+mj-cs" => self.fonts.major.cs.clone(),
            "+mn-cs" => self.fonts.minor.cs.clone(),
            other => other.to_string(),
        }
    }
}

fn hex(s: &str) -> Rgba {
    Rgba::from_hex(s).unwrap_or(Rgba::BLACK)
}

fn scheme(name: &str, c: [&str; 12]) -> ColorScheme {
    ColorScheme {
        name: name.into(),
        colors: c.map(hex),
    }
}

fn ph() -> ColorRef {
    ColorRef::scheme(SchemeSlot::PhClr)
}

/// Our default format scheme: flat solid fills, a soft gradient, and a gentle shadow for "intense".
pub fn default_format() -> FormatScheme {
    let grad = |a: i32, b: i32| {
        Fill::Gradient(Gradient {
            tile: [0.0; 4],
            stops: vec![
                GradientStop {
                    pos: 0.0,
                    color: ph()
                        .with(ColorTransform::LumMod(a))
                        .with(ColorTransform::SatMod(103000)),
                },
                GradientStop {
                    pos: 1.0,
                    color: ph().with(ColorTransform::LumMod(b)),
                },
            ],
            shape: GradientShape::Linear {
                angle: 90.0,
                scaled: false,
            },
            rotate_with_shape: true,
            flip: None,
        })
    };
    FormatScheme {
        name: "DeckCraft".into(),
        fills: vec![Fill::solid(ph()), grad(110000, 92000), grad(105000, 80000)],
        lines: vec![
            Line::solid(ph(), 0.75),
            Line::solid(ph(), 1.0),
            Line::solid(ph(), 1.5),
        ],
        effects: vec![
            Effects::default(),
            Effects::default(),
            Effects {
                outer_shadow: Some(Shadow {
                    color: ColorRef::rgb(Rgba::BLACK).with(ColorTransform::Alpha(40000)),
                    blur: 6.0,
                    dist: 3.0,
                    dir: 90.0,
                    inner: false,
                    sx: 1.0,
                    sy: 1.0,
                    kx: 0.0,
                    ky: 0.0,
                    align: "t".into(),
                    rotate_with_shape: false,
                }),
                ..Default::default()
            },
        ],
        bg_fills: vec![
            Fill::solid(ph()),
            Fill::solid(
                ph().with(ColorTransform::Tint(95000))
                    .with(ColorTransform::SatMod(170000)),
            ),
            grad(102000, 85000),
        ],
    }
}

/// Design ▸ Variants: four colour variations of `t`, built from its built-in colours when it is
/// a built-in theme (so picking one variant after another doesn't drift), else from its own.
/// 1: as designed; 2 and 3: the accents rotated; 4: dark (light and dark colours swapped).
pub fn theme_variants(t: &Theme) -> Vec<ColorScheme> {
    let base = builtin_themes()
        .into_iter()
        .find(|b| b.name == t.name)
        .map_or_else(|| t.colors.clone(), |b| b.colors);
    let rotate = |by: usize| {
        let mut c = base.clone();
        let accents: Vec<_> = base
            .colors
            .get(4..10)
            .map(<[_]>::to_vec)
            .unwrap_or_default();
        for (i, slot) in c.colors.iter_mut().skip(4).take(6).enumerate() {
            if let Some(a) = accents.get((i + by) % accents.len().max(1)) {
                *slot = *a;
            }
        }
        c
    };
    let mut dark = base.clone();
    dark.colors.swap(0, 1);
    dark.colors.swap(2, 3);
    let mut out = vec![base.clone(), rotate(1), rotate(3), dark];
    for (i, c) in out.iter_mut().enumerate().skip(1) {
        c.name = format!("{} {}", base.name, i + 1);
    }
    out
}

/// Curated built-in themes (shown in Design ▸ Themes), covering the 10 most popular
/// mainstream presentation scenarios (corporate, dark tech, minimalist, official,
/// consulting, academic, neo-Chinese, eco/ESG, warm creative, and black-gold executive).
pub fn builtin_themes() -> Vec<Theme> {
    let t = |name: &str, colors: ColorScheme, major: &str, minor: &str| Theme {
        name: name.into(),
        colors,
        fonts: FontScheme {
            name: name.into(),
            major: FontSet {
                latin: major.into(),
                ..Default::default()
            },
            minor: FontSet {
                latin: minor.into(),
                ..Default::default()
            },
        },
        format: default_format(),
        raw_extra: None,
    };
    vec![
        // 1. 现代商务 — 通用企业汇报、年度总结、商业计划书（经典信任蓝 + 高对比数据色）
        t(
            "现代商务",
            scheme(
                "现代商务",
                [
                    "1A1D21", "FFFFFF", "1E3A5F", "F1F5F9", "1D63DC", "0EA5A4", "F59E0B", "E11D48",
                    "6366F1", "10B981", "1D63DC", "6366F1",
                ],
            ),
            "Inter",
            "Inter",
        ),
        // 2. 深空科技 — AI / 大数据 / 产品发布会、科技路演（深空暗底 + 霓虹青紫）
        t(
            "深空科技",
            scheme(
                "深空科技",
                [
                    "F8FAFC", "0B1120", "E2E8F0", "162033", "38BDF8", "818CF8", "2DD4BF", "FB7185",
                    "FBBF24", "34D399", "38BDF8", "818CF8",
                ],
            ),
            "Inter",
            "Inter",
        ),
        // 3. 极简留白 — Swiss 极简演讲、作品集、产品概念提案（大面积留白 + 炭黑 + 焦点单色）
        t(
            "极简留白",
            scheme(
                "极简留白",
                [
                    "18181B", "FFFFFF", "3F3F46", "F4F4F5", "18181B", "52525B", "2563EB", "EA580C",
                    "059669", "7C3AED", "2563EB", "7C3AED",
                ],
            ),
            "Helvetica Neue",
            "Inter",
        ),
        // 4. 党政赤金 — 党政汇报、党建总结、政企正式报告（中国红 + 正统金 + 沉稳藏青）
        t(
            "党政赤金",
            scheme(
                "党政赤金",
                [
                    "1F1A17", "FFFFFF", "7F1D1D", "FDF6F2", "C81E1E", "D97706", "991B1B", "1E3A5F",
                    "B45309", "4B5563", "C81E1E", "7F1D1D",
                ],
            ),
            "Noto Serif SC",
            "Noto Sans SC",
        ),
        // 5. 咨询藏青 — 管理咨询、财务分析、高管决策简报（深海藏青 + 变革青绿）
        t(
            "咨询藏青",
            scheme(
                "咨询藏青",
                [
                    "0F1E2E", "FFFFFF", "13344D", "EDF3F7", "0F4C81", "0D9488", "D97706", "DC2626",
                    "475569", "2563EB", "0F4C81", "0D9488",
                ],
            ),
            "Merriweather",
            "Open Sans",
        ),
        // 6. 学术青蓝 — 毕业答辩、开题报告、科研学术会议（严谨学院蓝 + 护眼浅蓝底）
        t(
            "学术青蓝",
            scheme(
                "学术青蓝",
                [
                    "16202A", "FAFCFF", "1E3A5F", "EBF1F8", "1E40AF", "0F766E", "B91C1C", "4338CA",
                    "A16207", "475569", "1E40AF", "4338CA",
                ],
            ),
            "Georgia",
            "Source Sans 3",
        ),
        // 7. 新中式韵 — 东方美学、文旅推广、文创与文化展示（月白宣纸 + 远山黛 + 朱砂石青）
        t(
            "新中式韵",
            scheme(
                "新中式韵",
                [
                    "23201D", "FAF7F2", "3E4A52", "EFE9DF", "3B5B6E", "B83B2A", "C69749", "4D7C6F",
                    "7A5C43", "6D5D7B", "3B5B6E", "B83B2A",
                ],
            ),
            "Playfair Display",
            "Lato",
        ),
        // 8. 生态青绿 — ESG 报告、双碳新能源、健康医疗与环保主题（森林绿 + 生态青 + 暖麦金）
        t(
            "生态青绿",
            scheme(
                "生态青绿",
                [
                    "13261B", "FCFEFC", "1F4430", "EDF5EF", "15803D", "0D9488", "65A30D", "CA8A04",
                    "0284C7", "B45309", "15803D", "0D9488",
                ],
            ),
            "Montserrat",
            "Open Sans",
        ),
        // 9. 暖阳陶土 — 消费品营销、文创品牌、生活方式创意提案（燕麦暖底 + 陶土红棕 + 橄榄绿）
        t(
            "暖阳陶土",
            scheme(
                "暖阳陶土",
                [
                    "261B15", "FFFBF6", "4A3224", "F5ECE2", "C2592B", "D9822B", "7A8B4A", "9E3B4B",
                    "2F6F7E", "7C5C48", "C2592B", "9E3B4B",
                ],
            ),
            "Poppins",
            "Nunito Sans",
        ),
        // 10. 黑金奢雅 — 高端发布会、金融资管、年终盛典与轻奢路演（曜石黑底 + 香槟金）
        t(
            "黑金奢雅",
            scheme(
                "黑金奢雅",
                [
                    "F5F3EE", "121316", "E5DFD3", "1D1F25", "D4AF37", "E6C975", "94A3B8", "38BDF8",
                    "E07A5F", "52B788", "D4AF37", "E6C975",
                ],
            ),
            "Playfair Display",
            "Inter",
        ),
    ]
}

/// Named colour schemes for Design ▸ Variants ▸ Colors.
pub fn builtin_color_schemes() -> Vec<ColorScheme> {
    let mut v: Vec<ColorScheme> = builtin_themes().into_iter().map(|t| t.colors).collect();
    v.push(scheme(
        "Grayscale",
        [
            "000000", "FFFFFF", "000000", "F8F8F8", "DDDDDD", "B2B2B2", "969696", "808080",
            "5F5F5F", "4D4D4D", "5F5F5F", "919191",
        ],
    ));
    v.push(scheme(
        "Ocean",
        [
            "000000", "FFFFFF", "13293D", "E8F1F2", "006494", "247BA0", "1B98E0", "6CC1E8",
            "13293D", "0B3954", "1B98E0", "6CC1E8",
        ],
    ));
    v.push(scheme(
        "Sunset",
        [
            "2B2118", "FFFFFF", "6F1A07", "FFF3E6", "FF9F1C", "FFBF69", "CB997E", "E76F51",
            "F4A261", "2A9D8F", "E76F51", "2A9D8F",
        ],
    ));
    v.push(scheme(
        "Forest",
        [
            "1B1B1B", "FFFFFF", "283618", "FEFAE0", "606C38", "DDA15E", "BC6C25", "283618",
            "8A9A5B", "4F772D", "606C38", "BC6C25",
        ],
    ));
    v.push(scheme(
        "Berry",
        [
            "1F0A1D", "FFFFFF", "4A1942", "F7E8F3", "893168", "C6426E", "EB6A8F", "4A1942",
            "7D7ABC", "56A3A6", "893168", "56A3A6",
        ],
    ));
    v
}

/// Named font pairs for Design ▸ Variants ▸ Fonts.
pub fn builtin_font_schemes() -> Vec<FontScheme> {
    let f = |name: &str, major: &str, minor: &str| FontScheme {
        name: name.into(),
        major: FontSet {
            latin: major.into(),
            ..Default::default()
        },
        minor: FontSet {
            latin: minor.into(),
            ..Default::default()
        },
    };
    vec![
        f("Inter", "Inter", "Inter"),
        f("Classic", "Merriweather", "Open Sans"),
        f("Geometric", "Montserrat", "Lato"),
        f("Editorial", "Playfair Display", "Lato"),
        f("Humanist", "Merriweather", "Open Sans"),
        f("Friendly", "Poppins", "Nunito Sans"),
        f("Neutral", "Roboto", "Roboto"),
        f("Office Compatible", "Carlito", "Carlito"),
        f("Times / Arial", "Liberation Serif", "Liberation Sans"),
    ]
}

fn shadow(blur: f64, dist: f64, alpha: i32) -> Effects {
    Effects {
        outer_shadow: Some(Shadow {
            color: ColorRef::rgb(Rgba::BLACK).with(ColorTransform::Alpha(alpha)),
            blur,
            dist,
            dir: 90.0,
            inner: false,
            sx: 1.0,
            sy: 1.0,
            kx: 0.0,
            ky: 0.0,
            align: "b".into(),
            rotate_with_shape: false,
        }),
        ..Default::default()
    }
}

fn glow(radius: f64) -> Effects {
    Effects {
        glow: Some(Glow {
            color: ph().with(ColorTransform::Alpha(60_000)),
            radius,
        }),
        ..Default::default()
    }
}

/// Named effect schemes for Design ▸ Effects. Each one replaces the theme's format scheme
/// (subtle, moderate and intense fills, lines and effects) and leaves the colour and font schemes.
pub fn builtin_format_schemes() -> Vec<FormatScheme> {
    let mut soft = default_format();
    soft.name = "Soft".into();
    soft.lines = vec![
        Line::solid(ph(), 0.5),
        Line::solid(ph(), 0.75),
        Line::solid(ph(), 1.0),
    ];
    soft.effects = vec![
        shadow(16.0, 1.0, 18_000),
        shadow(12.0, 2.0, 22_000),
        shadow(10.0, 3.0, 28_000),
    ];

    let mut crisp = default_format();
    crisp.name = "Crisp".into();
    crisp.fills = vec![Fill::solid(ph()), Fill::solid(ph()), Fill::solid(ph())];
    crisp.lines = vec![
        Line::solid(ph(), 1.0),
        Line::solid(ph(), 1.5),
        Line::solid(ph(), 2.25),
    ];
    crisp.effects = vec![
        Effects::default(),
        Effects::default(),
        shadow(0.0, 2.0, 55_000),
    ];

    let mut lift = default_format();
    lift.name = "Lift".into();
    lift.effects = vec![
        Effects::default(),
        shadow(8.0, 6.0, 28_000),
        shadow(12.0, 10.0, 35_000),
    ];

    let mut glow_scheme = default_format();
    glow_scheme.name = "Glow".into();
    glow_scheme.effects = vec![Effects::default(), glow(6.0), glow(12.0)];

    let mut plain = default_format();
    plain.name = "Plain".into();
    plain.fills = vec![Fill::solid(ph()), Fill::solid(ph()), Fill::solid(ph())];
    plain.lines = vec![
        Line::solid(ph(), 0.75),
        Line::solid(ph(), 0.75),
        Line::solid(ph(), 0.75),
    ];
    plain.effects = vec![Effects::default(), Effects::default(), Effects::default()];

    vec![default_format(), soft, crisp, lift, glow_scheme, plain]
}
