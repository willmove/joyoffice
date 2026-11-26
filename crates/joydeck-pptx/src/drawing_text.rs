//! ECMA-376 DrawingML text bodies, paragraphs, breaks, fields and edit-ready runs.

use joydeck_model::text::{Paragraph, Run, RunKind, RunProps, TextBody};

use crate::read::{Imp, Part, dml as input};
use crate::write::{Exp, Out, dml as output};
use crate::xml::{A, El, W};

const MAX_PARAGRAPHS: usize = 100_000;
const MAX_RUNS: usize = 10_000;

pub(crate) fn read_body(imp: &mut Imp, part: &Part, element: &El) -> TextBody {
    let mut body = TextBody::default();
    for child in element.elements() {
        match child.local() {
            "bodyPr" => body.body = input::body_pr(child),
            "lstStyle" => body.list_style = input::list_style(imp, part, child),
            "p" if body.paragraphs.len() < MAX_PARAGRAPHS => {
                body.paragraphs.push(read_paragraph(imp, part, child));
            }
            _ => {}
        }
    }
    body
}

pub(crate) fn read_paragraph(imp: &mut Imp, part: &Part, element: &El) -> Paragraph {
    let mut paragraph = Paragraph::default();
    for child in element.elements() {
        match child.local() {
            "pPr" => {
                let (level, properties, defaults) = input::ppr(imp, part, child);
                paragraph.level = level.unwrap_or_default();
                paragraph.props = properties;
                paragraph.def_props = defaults.unwrap_or_default();
            }
            "endParaRPr" => paragraph.end_props = input::rpr(imp, part, child),
            "r" | "br" | "fld" | "m" if paragraph.runs.len() < MAX_RUNS => {
                read_run(imp, part, child, &mut paragraph.runs);
            }
            _ => {}
        }
    }
    paragraph
}

fn read_run(imp: &mut Imp, part: &Part, element: &El, runs: &mut Vec<Run>) {
    if element.is("m") {
        let mut text = Vec::new();
        element.find_all("t", &mut text);
        runs.push(Run {
            text: text.into_iter().map(El::text).collect(),
            props: RunProps::default(),
            kind: RunKind::Math {
                omml: element
                    .elements()
                    .map(|child| part.keep_inner(child))
                    .collect(),
            },
        });
        return;
    }
    let props = element
        .child("rPr")
        .map(|child| input::rpr(imp, part, child))
        .unwrap_or_default();
    let text = element.child("t").map(El::text).unwrap_or_default();
    match element.local() {
        "r" => {
            for (index, piece) in text.split(['\n', '\u{b}']).enumerate() {
                if index > 0 && runs.len() < MAX_RUNS {
                    runs.push(Run {
                        text: String::new(),
                        props: props.clone(),
                        kind: RunKind::Break,
                    });
                }
                if runs.len() >= MAX_RUNS {
                    break;
                }
                if !piece.is_empty() {
                    runs.push(Run {
                        text: piece.to_owned(),
                        props: props.clone(),
                        kind: RunKind::Text,
                    });
                }
            }
        }
        "br" => runs.push(Run {
            text: String::new(),
            props,
            kind: RunKind::Break,
        }),
        "fld" => runs.push(Run {
            text,
            props,
            kind: RunKind::Field {
                field: element.attr("type").unwrap_or_default().to_owned(),
            },
        }),
        _ => {}
    }
}

pub(crate) fn write_body(
    xml: &mut W,
    exp: &mut Exp,
    part: &mut Out,
    tag: &str,
    body: &TextBody,
    math: bool,
) {
    xml.open0(tag);
    output::body_pr(xml, &body.body);
    output::list_style(xml, exp, part, "a:lstStyle", &body.list_style);
    if body.paragraphs.is_empty() {
        xml.empty0("a:p");
    }
    for paragraph in &body.paragraphs {
        write_paragraph(xml, exp, part, paragraph, math);
    }
    xml.close(tag);
}

pub(crate) fn write_paragraph(
    xml: &mut W,
    exp: &mut Exp,
    part: &mut Out,
    paragraph: &Paragraph,
    math: bool,
) {
    xml.open0("a:p");
    let defaults = (paragraph.def_props != RunProps::default()).then_some(&paragraph.def_props);
    if paragraph.level != 0 || !output::is_default_ppr(&paragraph.props) || defaults.is_some() {
        output::ppr(
            xml,
            exp,
            part,
            "a:pPr",
            Some(paragraph.level),
            &paragraph.props,
            defaults,
        );
    }
    for run in &paragraph.runs {
        match &run.kind {
            RunKind::Break => line_break(xml, exp, part, &run.props),
            RunKind::Field { field } => {
                let kind = (!field.is_empty()
                    && field
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-'))
                .then_some(field.as_str());
                xml.open(
                    "a:fld",
                    A::new().a("id", part.next_field_id()).o("type", kind),
                );
                output::rpr(xml, exp, part, "a:rPr", &run.props);
                xml.elt("a:t", &run.text);
                xml.close("a:fld");
            }
            RunKind::Math { omml } if math && !omml.is_empty() => {
                xml.open("a14:m", A::new().a("xmlns:a14", crate::opc::NS_A14));
                xml.raw(omml);
                xml.close("a14:m");
            }
            _ => {
                for (index, piece) in run.text.split(['\n', '\u{b}']).enumerate() {
                    if index != 0 {
                        line_break(xml, exp, part, &run.props);
                    }
                    if index == 0 || !piece.is_empty() {
                        xml.open0("a:r");
                        output::rpr(xml, exp, part, "a:rPr", &run.props);
                        xml.elt("a:t", piece);
                        xml.close("a:r");
                    }
                }
            }
        }
    }
    if paragraph.end_props != RunProps::default() {
        output::rpr(xml, exp, part, "a:endParaRPr", &paragraph.end_props);
    }
    xml.close("a:p");
}

fn line_break(xml: &mut W, exp: &mut Exp, part: &mut Out, props: &RunProps) {
    xml.open0("a:br");
    output::rpr(xml, exp, part, "a:rPr", props);
    xml.close("a:br");
}

pub(crate) fn has_math(body: &TextBody) -> bool {
    body.paragraphs
        .iter()
        .flat_map(|paragraph| &paragraph.runs)
        .any(|run| matches!(&run.kind, RunKind::Math { omml } if !omml.is_empty()))
}
