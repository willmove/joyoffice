//! Explicit subset of the documented v1 command parameters, translated into v2 edits.
use joydeck_engine::{
    Command, ContentAction, EditorSnapshot, ReviewAction, Session, TableAction, TextTarget,
    WorkspaceAction,
};
use joydeck_model::{Presentation, ShapeId};
use serde_json::Value;
pub const SUPPORTED: &[&str] = &[
    "page.add",
    "page.copy",
    "page.delete",
    "page.visibility",
    "page.reorder",
    "page.size",
    "object.delete",
    "table.edit",
    "document.design",
    "review.edit",
    "transaction.batch",
];
fn number(p: &Value, name: &str) -> Result<usize, String> {
    p[name]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("missing index {name}"))
}
fn n(v: &Value) -> Result<usize, String> {
    v.as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or("invalid index".into())
}
fn f(v: &Value) -> Result<f64, String> {
    v.as_f64()
        .filter(|n| n.is_finite())
        .ok_or("invalid number".into())
}
fn text(v: &Value) -> Result<String, String> {
    v.as_str().map(str::to_owned).ok_or("missing text".into())
}
pub fn parse(p: &Value, session: &Session) -> Result<Command, String> {
    if p["command"] != "transaction.batch" {
        return parse_edit(p, session.model());
    }
    let mut snapshot = session.snapshot();
    let mut commands = Vec::new();
    collect_batch(p, &mut snapshot, &mut commands, 0)?;
    Ok(Command::Batch { commands })
}
fn collect_batch(
    p: &Value,
    snapshot: &mut EditorSnapshot,
    commands: &mut Vec<Command>,
    depth: usize,
) -> Result<(), String> {
    if depth > 0 && p.get("expectedRevision").is_some() {
        return Err("nested revision guards are not supported; use the outer request guard".into());
    }
    if depth > 16 {
        return Err("legacy batch depth exceeds 16".into());
    }
    if p["command"] != "transaction.batch" {
        let edits = match parse_edit(p, &snapshot.model)? {
            Command::Batch { commands } => commands,
            edit => vec![edit],
        };
        if commands.len() + edits.len() > 256 {
            return Err("flattened batch exceeds 256 commands".into());
        }
        for edit in edits {
            snapshot.stage_edit(edit.clone())?;
            commands.push(edit);
        }
        return Ok(());
    }
    if p.as_object().is_none_or(|p| {
        p.keys()
            .any(|k| !matches!(k.as_str(), "command" | "commands" | "expectedRevision"))
    }) {
        return Err("unknown legacy command field".into());
    }
    let values = p["commands"]
        .as_array()
        .filter(|v| !v.is_empty() && v.len() <= 256)
        .ok_or("legacy batch requires 1–256 commands")?;
    for value in values {
        collect_batch(value, snapshot, commands, depth + 1)?;
    }
    Ok(())
}
fn table_shape(slide: &joydeck_model::Slide, object: &Value) -> Result<ShapeId, String> {
    if let Some(legacy) = object.as_str() {
        let mut matches = slide.shapes.iter().filter(|shape| {
            shape
                .tags
                .iter()
                .any(|(key, value)| key == "JoyOffice.LegacyId" && value == legacy)
        });
        if let Some(shape) = matches.next() {
            if matches.next().is_some() {
                return Err("ambiguous legacy table id".into());
            }
            return Ok(shape.id);
        }
    }
    let id = object
        .as_str()
        .and_then(|id| id.parse::<u32>().ok())
        .or_else(|| object.as_u64().and_then(|id| u32::try_from(id).ok()))
        .map(ShapeId)
        .ok_or("missing legacy table id")?;
    slide
        .shapes
        .iter()
        .find(|shape| shape.id == id)
        .map(|shape| shape.id)
        .ok_or("table object outside page".into())
}
fn parse_edit(p: &Value, model: &Presentation) -> Result<Command, String> {
    let id = p["command"].as_str().ok_or("missing legacy command")?;
    let allowed = match id {
        "page.add" => vec![],
        "page.copy" | "page.delete" | "page.visibility" => vec!["pages"],
        "page.reorder" => vec!["pages", "target"],
        "page.size" => vec!["width", "height"],
        "object.delete" => vec!["page", "objects"],
        "table.edit" => vec!["page", "object", "action"],
        "document.design" | "review.edit" => vec!["action"],
        _ => {
            return Err(
                "legacy command is outside the advertised subset; original state unchanged".into(),
            );
        }
    };
    if p.as_object().is_none_or(|p| {
        p.keys()
            .any(|k| k != "command" && k != "expectedRevision" && !allowed.contains(&k.as_str()))
    }) {
        return Err("unknown legacy command field".into());
    }
    let pages = || -> Result<Vec<joydeck_model::SlideId>, String> {
        let pages = p["pages"].as_array().ok_or("missing pages")?;
        let ids = pages
            .iter()
            .map(|v| {
                model
                    .slides
                    .get(n(v)?)
                    .map(|s| s.id)
                    .ok_or("page outside document".into())
            })
            .collect::<Result<Vec<_>, String>>()?;
        if ids.is_empty()
            || ids.iter().collect::<std::collections::BTreeSet<_>>().len() != ids.len()
        {
            return Err("empty or duplicate pages".into());
        }
        Ok(ids)
    };
    let workspace = |action| Command::Workspace { action };
    let at = |page: usize, shapes, edit| Command::At {
        page: page + 1,
        shapes,
        edit: Box::new(edit),
    };
    Ok(match id {
        "page.add" => Command::NewSlide,
        "page.copy" => workspace(WorkspaceAction::DuplicateSlides { slides: pages()? }),
        "page.delete" => workspace(WorkspaceAction::DeleteSlides { slides: pages()? }),
        "page.visibility" => Command::Batch {
            commands: pages()?
                .into_iter()
                .map(|id| {
                    workspace(WorkspaceAction::HideSlides {
                        slides: vec![id],
                        hidden: model.slide(id).is_some_and(|s| !s.hidden),
                    })
                })
                .collect(),
        },
        "page.reorder" => {
            let ids = pages()?;
            let target = number(p, "target")?;
            if target > model.slides.len() {
                return Err("page gap outside document".into());
            }
            let before = model.slides[target..]
                .iter()
                .find(|s| !ids.contains(&s.id))
                .map(|s| s.id);
            workspace(WorkspaceAction::ReorderSlides {
                slides: ids,
                before,
            })
        }
        "page.size" => Command::Content {
            action: ContentAction::SlideSize {
                width: f(&p["width"])?,
                height: f(&p["height"])?,
            },
        },
        "object.delete" => {
            let page = number(p, "page")?;
            let slide = model.slides.get(page).ok_or("page outside document")?;
            let objects = p["objects"].as_array().ok_or("missing objects")?;
            let ids = objects
                .iter()
                .map(|v| {
                    slide
                        .shapes
                        .get(n(v)?)
                        .map(|s| s.id)
                        .ok_or("object index outside page".into())
                })
                .collect::<Result<Vec<_>, String>>()?;
            if ids.is_empty() {
                return Err("select at least one object".into());
            }
            at(page, ids, Command::Delete)
        }
        "table.edit" => {
            let page = number(p, "page")?;
            let slide = model.slides.get(page).ok_or("page outside document")?;
            let shape = table_shape(slide, &p["object"])?;
            let action = &p["action"];
            let value = &action["value"];
            if action
                .as_object()
                .is_none_or(|a| a.keys().any(|k| k != "type" && k != "value"))
            {
                return Err("unknown table action field".into());
            }
            let kind = action["type"].as_str().ok_or("missing table action type")?;
            let count = match kind {
                "rowHeight" | "columnWidth" | "split" => Some(2),
                "paste" => Some(3),
                _ => None,
            };
            if count.is_some_and(|n| value.as_array().is_none_or(|v| v.len() != n)) {
                return Err("invalid table action arguments".into());
            }
            if kind == "merge"
                && value.as_object().is_none_or(|v| {
                    v.len() != 4
                        || v.keys()
                            .any(|k| !["top", "left", "bottom", "right"].contains(&k.as_str()))
                })
            {
                return Err("invalid merge fields".into());
            }
            let action = match kind {
                "insertRow" => TableAction::InsertRow { at: n(value)? },
                "deleteRow" => TableAction::DeleteRow { at: n(value)? },
                "insertColumn" => TableAction::InsertCol { at: n(value)? },
                "deleteColumn" => TableAction::DeleteCol { at: n(value)? },
                "rowHeight" => TableAction::RowHeight {
                    row: n(&value[0])?,
                    height: f(&value[1])?,
                },
                "columnWidth" => TableAction::ColWidth {
                    col: n(&value[0])?,
                    width: f(&value[1])?,
                },
                "paste" => TableAction::PasteTsv {
                    row: n(&value[0])?,
                    col: n(&value[1])?,
                    text: text(&value[2])?,
                },
                "merge" => TableAction::Merge {
                    row: number(value, "top")?,
                    col: number(value, "left")?,
                    end_row: number(value, "bottom")?,
                    end_col: number(value, "right")?,
                },
                "split" => TableAction::Split {
                    row: n(&value[0])?,
                    col: n(&value[1])?,
                },
                _ => {
                    return Err(
                        "legacy table style/rich-run/distribution action requires schema 2".into(),
                    );
                }
            };
            at(page, vec![shape], Command::Table { action })
        }
        "document.design" => {
            let action = &p["action"];
            let value = &action["value"];
            if action
                .as_object()
                .is_none_or(|a| a.keys().any(|k| k != "type" && k != "value"))
                || value.as_array().is_none_or(|v| v.len() != 2)
            {
                return Err("invalid design action fields".into());
            }
            match action["type"].as_str().ok_or("missing design action type")? {
                "notes" => {
                    let page = n(&value[0])?;
                    if page >= model.slides.len() {
                        return Err("page outside document".into());
                    }
                    at(page, vec![], Command::SetText { target: TextTarget::Notes, body: Box::new(joydeck_model::TextBody::from_text(&text(&value[1])?)) })
                }

                _ => return Err("legacy theme/layout/field design action requires schema 2; no values discarded".into()),
            }
        }
        "review.edit" => {
            let action = &p["action"];
            let kind = action["type"].as_str().ok_or("missing review type")?;
            let keys = match kind {
                "add" => vec!["id", "slide", "object", "author", "text"],
                "reply" => vec!["id", "author", "text"],
                "resolve" => vec!["id", "resolved"],
                "accessibility" => vec!["object", "alt", "decorative"],
                _ => return Err("unsupported legacy review action".into()),
            };
            if action
                .as_object()
                .is_none_or(|a| a.keys().any(|k| k != "type" && !keys.contains(&k.as_str())))
            {
                return Err("unknown review action field".into());
            }
            let find_object = |id: &str| {
                model
                    .slides
                    .iter()
                    .enumerate()
                    .find_map(|(page, s)| {
                        s.shapes
                            .iter()
                            .find(|s| {
                                s.name == id
                                    || s.id.0.to_string() == id
                                    || s.tags
                                        .iter()
                                        .any(|(k, v)| k == "JoyOffice.LegacyId" && v == id)
                            })
                            .map(|s| (page, s.id))
                    })
                    .ok_or("review object anchor missing")
            };
            match kind {
                "add" => {
                    let slide_id = text(&action["slide"])?;
                    if !action["object"].is_null() && !action["object"].is_string() {
                        return Err("review object must be a stable id or null".into());
                    }
                    let page = model
                        .slides
                        .iter()
                        .position(|s| {
                            s.name == slide_id
                                || s.id.0.to_string() == slide_id
                                || s.tags.iter().any(|(k, v)| {
                                    k == "JoyOffice.LegacyId"
                                        && Some(v.as_str()) == action["slide"].as_str()
                                })
                        })
                        .ok_or("review page anchor missing")?;
                    let shape = action["object"]
                        .as_str()
                        .map(|id| find_object(id).map(|(_, id)| id))
                        .transpose()?;
                    at(
                        page,
                        vec![],
                        Command::Review {
                            action: ReviewAction::Add {
                                id: Some(text(&action["id"])?),
                                author: text(&action["author"])?,
                                text: text(&action["text"])?,
                                shape,
                            },
                        },
                    )
                }
                "reply" | "resolve" => {
                    let id = text(&action["id"])?;
                    let (page, index) = model
                        .slides
                        .iter()
                        .enumerate()
                        .find_map(|(page, s)| {
                            s.comments
                                .iter()
                                .position(|c| c.id == id)
                                .map(|i| (page, i))
                        })
                        .ok_or("review thread id missing")?;
                    let action = if kind == "reply" {
                        ReviewAction::Reply {
                            index,
                            author: text(&action["author"])?,
                            text: text(&action["text"])?,
                        }
                    } else {
                        ReviewAction::Resolve {
                            index,
                            resolved: action["resolved"].as_bool().ok_or("resolved missing")?,
                        }
                    };
                    at(page, vec![], Command::Review { action })
                }
                "accessibility" => {
                    let (page, id) = find_object(&text(&action["object"])?)?;
                    at(
                        page,
                        vec![id],
                        Command::Review {
                            action: ReviewAction::Accessibility {
                                description: text(&action["alt"])?,
                                decorative: action["decorative"]
                                    .as_bool()
                                    .ok_or("decorative missing")?,
                            },
                        },
                    )
                }
                _ => return Err("unsupported legacy review action".into()),
            }
        }
        _ => return Err("unsupported legacy command".into()),
    })
}
