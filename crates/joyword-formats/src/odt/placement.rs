//! ODF 1.3 graphic positioning and wrapping (§17.21, §20.298–299, §20.397–402).
use joyword_model::para::{Anchor, Float, FloatAlign, Wrap, WrapText};

use super::writer::pt;
use super::xml::{Attrs, get, length};

pub(super) fn supported_property(key: &str) -> bool {
    matches!(
        key,
        "fo:margin"
            | "fo:margin-left"
            | "fo:margin-right"
            | "fo:margin-top"
            | "fo:margin-bottom"
            | "style:wrap"
            | "style:run-through"
            | "style:wrap-contour"
            | "style:wrap-contour-mode"
            | "style:horizontal-rel"
            | "style:vertical-rel"
            | "style:horizontal-pos"
            | "style:vertical-pos"
    )
}

pub(super) fn margins(attrs: &Attrs, group: &str) -> [Option<f32>; 4] {
    let all = get(attrs, &format!("fo:{group}")).and_then(length);
    ["top", "left", "bottom", "right"].map(|side| {
        get(attrs, &format!("fo:{group}-{side}"))
            .and_then(length)
            .or(all)
            .map(|v| v.clamp(0.0, 1584.0))
    })
}

pub(super) fn read(frame: &Attrs, props: &Attrs) -> Float {
    let value = |key| get(props, key).unwrap_or("");
    let [top, left, bottom, right] = margins(props, "margin").map(|v| v.unwrap_or(0.0));
    if !matches!(
        get(frame, "text:anchor-type"),
        Some("paragraph" | "char" | "page" | "frame")
    ) {
        return Float {
            effect: [left, top, right, bottom],
            ..Float::default()
        };
    }
    let align = |v| match v {
        "left" | "top" => Some(FloatAlign::Start),
        "center" | "middle" => Some(FloatAlign::Center),
        "right" | "bottom" => Some(FloatAlign::End),
        "inside" => Some(FloatAlign::Inside),
        "outside" => Some(FloatAlign::Outside),
        _ => None,
    };
    let wrap = match value("style:wrap") {
        "none" => Wrap::TopAndBottom,
        "run-through" if value("style:run-through") == "background" => Wrap::BehindText,
        "run-through" => Wrap::InFrontOfText,
        _ if value("style:wrap-contour") == "true" => match value("style:wrap-contour-mode") {
            "full" => Wrap::Through,
            _ => Wrap::Tight,
        },
        _ => Wrap::Square,
    };
    let offset = |key| {
        get(frame, key)
            .and_then(length)
            .unwrap_or(0.0)
            .clamp(-1584.0, 1584.0)
    };
    Float {
        wrap,
        wrap_text: match value("style:wrap") {
            "left" => WrapText::Left,
            "right" => WrapText::Right,
            "dynamic" => WrapText::Largest,
            _ => WrapText::BothSides,
        },
        h_rel: match value("style:horizontal-rel") {
            "page" => Anchor::Page,
            "page-content" => Anchor::Margin,
            "page-start-margin" => Anchor::LeftMargin,
            "page-end-margin" => Anchor::RightMargin,
            "char" => Anchor::Character,
            _ => Anchor::Column,
        },
        v_rel: match value("style:vertical-rel") {
            "page" => Anchor::Page,
            "page-content" => Anchor::Margin,
            "char" | "line" => Anchor::Line,
            _ => Anchor::Paragraph,
        },
        h_align: align(value("style:horizontal-pos")),
        v_align: align(value("style:vertical-pos")),
        z: get(frame, "draw:z-index")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        x: offset("svg:x"),
        y: offset("svg:y"),
        dist: (left + right) / 2.0,
        dist_top: top,
        dist_bottom: bottom,
        ..Float::default()
    }
}

pub(super) fn write(f: &Float) -> (String, String) {
    let attrs = |[left, top, right, bottom]: [f32; 4]| {
        format!(
            " fo:margin-left=\"{}\" fo:margin-top=\"{}\" fo:margin-right=\"{}\" fo:margin-bottom=\"{}\"",
            pt(left),
            pt(top),
            pt(right),
            pt(bottom)
        )
    };
    if f.wrap == Wrap::Inline {
        return (
            " text:anchor-type=\"as-char\"".into(),
            if f.effect == [0.0; 4] {
                String::new()
            } else {
                attrs(f.effect)
            },
        );
    }
    let (wrap, extra) = match f.wrap {
        Wrap::TopAndBottom => ("none", ""),
        Wrap::BehindText => ("run-through", " style:run-through=\"background\""),
        Wrap::InFrontOfText => ("run-through", " style:run-through=\"foreground\""),
        _ => {
            let sides = match f.wrap_text {
                WrapText::BothSides => "parallel",
                WrapText::Left => "left",
                WrapText::Right => "right",
                WrapText::Largest => "dynamic",
            };
            let contour = match f.wrap {
                Wrap::Tight => " style:wrap-contour=\"true\" style:wrap-contour-mode=\"outside\"",
                Wrap::Through => " style:wrap-contour=\"true\" style:wrap-contour-mode=\"full\"",
                _ => "",
            };
            (sides, contour)
        }
    };
    let anchor = if f.h_rel == Anchor::Character || f.v_rel == Anchor::Line {
        "char"
    } else {
        "paragraph"
    };
    let horizontal = match f.h_rel {
        Anchor::Page => "page",
        Anchor::Margin => "page-content",
        Anchor::LeftMargin => "page-start-margin",
        Anchor::RightMargin => "page-end-margin",
        Anchor::Character => "char",
        _ => "paragraph",
    };
    let vertical = match f.v_rel {
        Anchor::Page => "page",
        Anchor::Margin => "page-content",
        Anchor::Line => "line",
        _ => "paragraph",
    };
    let hpos = f.h_align.map_or("from-left", |a| a.ooxml(true));
    let vpos = match f.v_align {
        Some(FloatAlign::Start) => "top",
        Some(FloatAlign::Center) => "middle",
        Some(FloatAlign::End) => "bottom",
        _ => "from-top",
    };
    let margin = attrs([f.dist, f.dist_top, f.dist, f.dist_bottom]);
    (
        format!(
            " text:anchor-type=\"{anchor}\" svg:x=\"{}\" svg:y=\"{}\"",
            pt(f.x),
            pt(f.y)
        ),
        format!(
            " style:wrap=\"{wrap}\"{extra} style:horizontal-pos=\"{hpos}\" style:horizontal-rel=\"{horizontal}\" style:vertical-pos=\"{vpos}\" style:vertical-rel=\"{vertical}\"{margin}"
        ),
    )
}
