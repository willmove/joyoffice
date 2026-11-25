//! Text bodies: body properties, paragraphs, runs, list styles.

use serde::{Deserialize, Serialize};

use crate::style::{ColorRef, Fill, Line};
pub use crate::text_body::{
    LevelStyle, ListNumbering, ListStyle, Paragraph, Run, RunKind, TextBody,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Anchor {
    Top,
    Middle,
    Bottom,
    Justified,
    Distributed,
}

impl Anchor {
    pub fn xml(self) -> &'static str {
        match self {
            Anchor::Top => "t",
            Anchor::Middle => "ctr",
            Anchor::Bottom => "b",
            Anchor::Justified => "just",
            Anchor::Distributed => "dist",
        }
    }
    pub fn from_xml(s: &str) -> Option<Self> {
        Some(match s {
            "t" => Anchor::Top,
            "ctr" => Anchor::Middle,
            "b" => Anchor::Bottom,
            "just" => Anchor::Justified,
            "dist" => Anchor::Distributed,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AutoFit {
    /// Do not autofit.
    None,
    /// Shrink text on overflow: font scale and line-spacing reduction (0–1). `font_scale` is
    /// `None` for a file's `<a:normAutofit/>` with no `fontScale`: the schema default 100 %,
    /// which PowerPoint draws as it is and only fits again when the text changes (#724).
    Shrink {
        #[serde(skip_serializing_if = "Option::is_none")]
        font_scale: Option<f64>,
        line_reduction: f64,
    },
    /// Resize shape to fit text.
    Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextDir {
    Horizontal,
    Vertical,
    Vertical270,
    Stacked,
    EaVertical,
    /// `mongolianVert`: turned like `vert`, lines running from left to right.
    MongolianVertical,
    /// `wordArtVertRtl`: stacked, lines running from right to left.
    StackedRtl,
}

impl TextDir {
    pub fn xml(self) -> &'static str {
        match self {
            TextDir::Horizontal => "horz",
            TextDir::Vertical => "vert",
            TextDir::Vertical270 => "vert270",
            TextDir::Stacked => "wordArtVert",
            TextDir::EaVertical => "eaVert",
            TextDir::MongolianVertical => "mongolianVert",
            TextDir::StackedRtl => "wordArtVertRtl",
        }
    }
    pub fn from_xml(s: &str) -> Option<Self> {
        Some(match s {
            "horz" => TextDir::Horizontal,
            "vert" => TextDir::Vertical,
            "mongolianVert" => TextDir::MongolianVertical,
            "vert270" => TextDir::Vertical270,
            "wordArtVert" => TextDir::Stacked,
            "wordArtVertRtl" => TextDir::StackedRtl,
            "eaVert" => TextDir::EaVertical,
            _ => return None,
        })
    }
}

/// `bodyPr`: insets in points, anchoring, wrapping, autofit, columns, direction, warp.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BodyProps {
    pub inset_l: Option<f64>,
    pub inset_t: Option<f64>,
    pub inset_r: Option<f64>,
    pub inset_b: Option<f64>,
    pub anchor: Option<Anchor>,
    pub anchor_ctr: Option<bool>,
    pub wrap: Option<bool>,
    pub autofit: Option<AutoFit>,
    pub vert: Option<TextDir>,
    /// Extra text rotation in degrees.
    pub rot: Option<f64>,
    pub columns: Option<u32>,
    pub col_spacing: Option<f64>,
    /// WordArt transform (`prstTxWarp`), e.g. `textArchUp`.
    pub warp: Option<String>,
    /// The warp's adjust values (`prstTxWarp/avLst`: `adj` or `adj1`, `adj2`…, raw guide values);
    /// empty = the preset's defaults.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warp_adj: Vec<f64>,
    pub rtl_col: Option<bool>,
    /// Keep text upright when the shape rotates.
    pub upright: Option<bool>,
    /// Space before the first paragraph and after the last count too (`spcFirstLastPara`).
    pub space_first_last: Option<bool>,
}

impl BodyProps {
    pub fn inherit(&mut self, p: &BodyProps) {
        self.inset_l = self.inset_l.or(p.inset_l);
        self.inset_t = self.inset_t.or(p.inset_t);
        self.inset_r = self.inset_r.or(p.inset_r);
        self.inset_b = self.inset_b.or(p.inset_b);
        self.anchor = self.anchor.or(p.anchor);
        self.anchor_ctr = self.anchor_ctr.or(p.anchor_ctr);
        self.wrap = self.wrap.or(p.wrap);
        self.autofit = self.autofit.or(p.autofit);
        self.vert = self.vert.or(p.vert);
        self.rot = self.rot.or(p.rot);
        self.columns = self.columns.or(p.columns);
        self.col_spacing = self.col_spacing.or(p.col_spacing);
        if self.warp.is_none() {
            self.warp.clone_from(&p.warp);
            self.warp_adj.clone_from(&p.warp_adj);
        }
        self.rtl_col = self.rtl_col.or(p.rtl_col);
        self.upright = self.upright.or(p.upright);
        self.space_first_last = self.space_first_last.or(p.space_first_last);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    Left,
    Center,
    Right,
    Justify,
    /// Justified with kashida (tatweel) elongation for Arabic (`justLow`).
    JustLow,
    Distributed,
}

impl Align {
    pub fn xml(self) -> &'static str {
        match self {
            Align::Left => "l",
            Align::Center => "ctr",
            Align::Right => "r",
            Align::Justify => "just",
            Align::JustLow => "justLow",
            Align::Distributed => "dist",
        }
    }
    pub fn from_xml(s: &str) -> Option<Self> {
        Some(match s {
            "l" => Align::Left,
            "ctr" => Align::Center,
            "r" => Align::Right,
            "just" | "thaiDist" => Align::Justify,
            "justLow" => Align::JustLow,
            "dist" => Align::Distributed,
            _ => return None,
        })
    }
}

/// Spacing as a percentage of the line (1.0 = single) or in points.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "v")]
pub enum Spacing {
    Pct(f64),
    Pts(f64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Bullet {
    None,
    Char {
        char: String,
    },
    /// `arabicPeriod`, `arabicParenR`, `romanUcPeriod`, `romanLcPeriod`, `alphaUcPeriod`,
    /// `alphaLcParenR`, `alphaLcPeriod`, `circleNumDbPlain`…
    AutoNum {
        scheme: String,
        #[serde(default = "start_one")]
        start_at: u32,
    },
    Picture {
        media: crate::MediaId,
    },
}

fn start_one() -> u32 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabStop {
    pub pos: f64,
    /// `l`, `ctr`, `r`, `dec`.
    pub align: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ParaProps {
    pub align: Option<Align>,
    /// Left margin (points).
    pub margin_left: Option<f64>,
    pub margin_right: Option<f64>,
    /// First-line indent relative to the margin (negative = hanging).
    pub indent: Option<f64>,
    pub line_spacing: Option<Spacing>,
    pub space_before: Option<Spacing>,
    pub space_after: Option<Spacing>,
    pub bullet: Option<Bullet>,
    pub bullet_font: Option<String>,
    pub bullet_font_follow_text: Option<bool>,
    /// `None` = follow text colour.
    pub bullet_color: Option<ColorRef>,
    pub bullet_color_follow_text: Option<bool>,
    /// Bullet size as a fraction of the text size.
    pub bullet_size: Option<f64>,
    pub bullet_size_follow_text: Option<bool>,
    pub tabs: Option<Vec<TabStop>>,
    pub default_tab: Option<f64>,
    pub rtl: Option<bool>,
    pub font_align: Option<String>,
    pub east_asian_line_break: Option<bool>,
    /// `latinLnBrk`: Latin words may break inside (after `/`, `\`); off by default.
    pub latin_line_break: Option<bool>,
    /// `hangingPunct`: closing East Asian punctuation may hang past the end of a line.
    pub hanging_punct: Option<bool>,
}

impl ParaProps {
    pub fn inherit(&mut self, p: &ParaProps) {
        self.align = self.align.or(p.align);
        self.margin_left = self.margin_left.or(p.margin_left);
        self.margin_right = self.margin_right.or(p.margin_right);
        self.indent = self.indent.or(p.indent);
        self.line_spacing = self.line_spacing.or(p.line_spacing);
        self.space_before = self.space_before.or(p.space_before);
        self.space_after = self.space_after.or(p.space_after);
        if self.bullet.is_none() {
            self.bullet.clone_from(&p.bullet);
        }
        inherit_marker(
            &mut self.bullet_font,
            &mut self.bullet_font_follow_text,
            &p.bullet_font,
            p.bullet_font_follow_text,
        );
        inherit_marker(
            &mut self.bullet_color,
            &mut self.bullet_color_follow_text,
            &p.bullet_color,
            p.bullet_color_follow_text,
        );
        inherit_marker(
            &mut self.bullet_size,
            &mut self.bullet_size_follow_text,
            &p.bullet_size,
            p.bullet_size_follow_text,
        );
        if self.tabs.is_none() {
            self.tabs.clone_from(&p.tabs);
        }
        self.default_tab = self.default_tab.or(p.default_tab);
        self.rtl = self.rtl.or(p.rtl);
        if self.font_align.is_none() {
            self.font_align.clone_from(&p.font_align);
        }
        self.east_asian_line_break = self.east_asian_line_break.or(p.east_asian_line_break);
        self.latin_line_break = self.latin_line_break.or(p.latin_line_break);
        self.hanging_punct = self.hanging_punct.or(p.hanging_punct);
    }
}

fn inherit_marker<T: Clone>(
    value: &mut Option<T>,
    follow: &mut Option<bool>,
    parent: &Option<T>,
    parent_follow: Option<bool>,
) {
    if value.is_some() {
        *follow = None;
    } else if *follow != Some(true) {
        *follow = parent_follow.filter(|follows| *follows);
        if follow.is_none() {
            value.clone_from(parent);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Caps {
    None,
    Small,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Strike {
    None,
    Single,
    Double,
}

/// A hyperlink or action on text or a shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Action {
    Url {
        url: String,
    },
    Slide {
        slide: crate::SlideId,
    },
    NextSlide,
    PreviousSlide,
    FirstSlide,
    LastSlide,
    EndShow,
    LastViewed,
    /// Run a custom show. `returns` is PowerPoint's "Show and return".
    CustomShow {
        name: String,
        #[serde(default = "custom_show_returns", skip_serializing_if = "is_true")]
        returns: bool,
    },
    Program {
        path: String,
    },
    PlayMedia,
}

/// Can a link from a (possibly untrusted) deck be opened outside it, by the slide show or by
/// whoever opens an exported file? Only web pages and mail: `file:`, `javascript:`, relative paths
/// and other schemes (OS protocol handlers, network shares) are not.
pub fn safe_url(url: &str) -> bool {
    let u = url.trim();
    let Some((scheme, rest)) = u.split_once(':') else {
        return false;
    };
    match scheme.to_ascii_lowercase().as_str() {
        "http" | "https" => rest.starts_with("//") && rest.len() > 2,
        "mailto" => !rest.is_empty(),
        _ => false,
    }
}

fn custom_show_returns() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hyperlink {
    pub action: Action,
    #[serde(default)]
    pub tooltip: String,
    #[serde(default)]
    pub highlight_click: bool,
    /// Sound played when the action runs (`a:snd`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<crate::MediaId>,
    /// `endSnd`: stop the sound that is already playing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stop_sound: bool,
    /// Zoom (Insert ▸ Zoom) with Return to Zoom: in the show, after this slide is shown and
    /// advanced past, go back to the slide with the zoom. Saved to .pptx as a plain slide link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoom_return: Option<crate::SlideId>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RunProps {
    /// Latin font family, or a theme reference `+mj-lt` / `+mn-lt`.
    pub font: Option<String>,
    pub font_ea: Option<String>,
    pub font_cs: Option<String>,
    pub font_sym: Option<String>,
    /// Size in points.
    pub size: Option<f64>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    /// `none`, `sng`, `dbl`, `heavy`, `dotted`, `dash`, `wavy`…
    pub underline: Option<String>,
    pub underline_color: Option<ColorRef>,
    pub strike: Option<Strike>,
    /// Baseline offset as a fraction of the size (0.3 = superscript 30%).
    pub baseline: Option<f64>,
    pub caps: Option<Caps>,
    /// Character spacing in points.
    pub spacing: Option<f64>,
    /// Kern at and above this size (points); 0 = off.
    pub kern: Option<f64>,
    pub fill: Option<Fill>,
    pub outline: Option<Line>,
    pub highlight: Option<ColorRef>,
    pub link: Option<Hyperlink>,
    /// Action on mouse over (Insert ▸ Action ▸ Mouse Over on selected text, `a:hlinkMouseOver`).
    pub hover: Option<Hyperlink>,
    pub lang: Option<String>,
    pub shadow: Option<crate::style::Shadow>,
    /// `a:innerShdw` on the run. Outer shadow stays in `shadow`.
    pub inner_shadow: Option<crate::style::Shadow>,
    pub glow: Option<crate::style::Glow>,
    pub reflection: Option<crate::style::Reflection>,
    /// Soft-edge radius in points (`a:softEdge`).
    pub soft_edge: Option<f64>,
    /// Spelling/grammar errors are not flagged.
    pub no_proof: Option<bool>,
}

impl RunProps {
    pub fn inherit(&mut self, p: &RunProps) {
        macro_rules! take {
            ($($f:ident),*) => { $( if self.$f.is_none() { self.$f.clone_from(&p.$f); } )* };
        }
        take!(
            font,
            font_ea,
            font_cs,
            font_sym,
            size,
            bold,
            italic,
            underline,
            underline_color,
            strike,
            baseline,
            caps,
            spacing,
            kern,
            fill,
            outline,
            highlight,
            link,
            hover,
            lang,
            shadow,
            inner_shadow,
            glow,
            reflection,
            soft_edge,
            no_proof
        );
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Test fixtures use explicit failure assertions"
)]
mod tests {
    use super::*;
    use joydeck_color::Rgba;

    #[test]
    fn explicit_bullet_follow_text_resets_block_inheritance() {
        let parent = ParaProps {
            bullet_font: Some("Inter".into()),
            bullet_color: Some(ColorRef::rgb(Rgba::rgb(0, 0, 0))),
            bullet_size: Some(2.0),
            ..Default::default()
        };
        let mut child = ParaProps {
            bullet_font_follow_text: Some(true),
            bullet_color_follow_text: Some(true),
            bullet_size_follow_text: Some(true),
            ..Default::default()
        };

        child.inherit(&parent);

        assert_eq!(child.bullet_font, None);
        assert_eq!(child.bullet_color, None);
        assert_eq!(child.bullet_size, None);

        let mut inherited_reset = ParaProps::default();
        inherited_reset.inherit(&ParaProps {
            bullet_font_follow_text: Some(true),
            bullet_color_follow_text: Some(true),
            bullet_size_follow_text: Some(true),
            ..Default::default()
        });
        assert_eq!(inherited_reset.bullet_font_follow_text, Some(true));
        assert_eq!(inherited_reset.bullet_color_follow_text, Some(true));
        assert_eq!(inherited_reset.bullet_size_follow_text, Some(true));

        let mut explicit_false = ParaProps {
            bullet_font_follow_text: Some(false),
            bullet_color_follow_text: Some(false),
            bullet_size_follow_text: Some(false),
            ..Default::default()
        };
        explicit_false.inherit(&parent);
        assert_eq!(explicit_false.bullet_font, parent.bullet_font);
        assert_eq!(explicit_false.bullet_color, parent.bullet_color);
        assert_eq!(explicit_false.bullet_size, parent.bullet_size);
    }
}
