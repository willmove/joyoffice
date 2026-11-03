//! Review changes and bounded object acceptance use the normal undo transaction.
use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ReviewAction {
    Add {
        #[serde(default)]
        id: Option<String>,
        author: String,
        text: String,
        shape: Option<ShapeId>,
    },
    Reply {
        index: usize,
        author: String,
        text: String,
    },
    Resolve {
        index: usize,
        resolved: bool,
    },
    Accessibility {
        description: String,
        decorative: bool,
    },
    ReadingOrder {
        shapes: Vec<ShapeId>,
    },
}
fn message(author: &str, text: &str) -> std::result::Result<(), String> {
    if author.trim().is_empty() || author.len() > 256 || text.trim().is_empty() || text.len() > 8192
    {
        return Err("comment author/text empty or outside budget".into());
    }
    Ok(())
}
pub(super) fn apply(
    st: &mut EditorSnapshot,
    action: &ReviewAction,
) -> std::result::Result<(), String> {
    let slide = Arc::make_mut(&mut Arc::make_mut(&mut st.model).slides[st.selection.page]);
    match action {
        ReviewAction::Add {
            id,
            author,
            text,
            shape,
        } => {
            message(author, text)?;
            if id.as_ref().is_some_and(|id| {
                id.is_empty() || id.len() > 128 || slide.comments.iter().any(|c| &c.id == id)
            }) {
                return Err("comment id is empty, duplicated or outside budget".into());
            }
            if slide.comments.len() >= 1024 || shape.is_some_and(|id| slide.shape(id).is_none()) {
                return Err("comment budget or anchor invalid".into());
            }
            slide.comments.push(Comment {
                id: id.clone().unwrap_or_default(),
                author: author.clone(),
                text: text.clone(),
                shape: *shape,
                ..Default::default()
            });
        }
        ReviewAction::Reply {
            index,
            author,
            text,
        } => {
            message(author, text)?;
            let comment = slide.comments.get_mut(*index).ok_or("comment missing")?;
            if comment.replies.len() >= 128 {
                return Err("reply budget exceeded".into());
            }
            comment.replies.push(Comment {
                author: author.clone(),
                text: text.clone(),
                ..Default::default()
            });
        }
        ReviewAction::Resolve { index, resolved } => {
            slide
                .comments
                .get_mut(*index)
                .ok_or("comment missing")?
                .resolved = *resolved;
        }
        ReviewAction::Accessibility {
            description,
            decorative,
        } => {
            if description.len() > 8192 || st.selection.shapes.is_empty() {
                return Err("select an object and use at most 8 KiB alt text".into());
            }
            for id in &st.selection.shapes {
                let shape = slide.shape_mut(*id).ok_or("shape missing")?;
                if shape.locked {
                    return Err("object is locked".into());
                }
                shape.descr = description.clone();
                shape.decorative = *decorative;
            }
        }
        ReviewAction::ReadingOrder { shapes } => {
            let unique: std::collections::BTreeSet<_> = shapes.iter().copied().collect();
            if unique.len() != shapes.len()
                || shapes.len() != slide.shapes.len()
                || shapes.iter().any(|id| slide.shape(*id).is_none())
            {
                return Err("reading order must contain every top-level object once".into());
            }
            for (rank, id) in shapes.iter().enumerate() {
                let shape = slide.shape_mut(*id).ok_or("shape missing")?;
                shape.tags.retain(|(k, _)| k != "JoyOffice.ReadingOrder");
                shape
                    .tags
                    .push(("JoyOffice.ReadingOrder".into(), rank.to_string()));
            }
        }
    }
    Ok(())
}
pub(super) fn accept(
    st: &mut EditorSnapshot,
    target: Option<ShapeId>,
    replacement: Option<&transfer::Transfer>,
) -> std::result::Result<(), String> {
    let page = st.selection.page;
    let at = target
        .map(|id| {
            st.model.slides[page]
                .shapes
                .iter()
                .position(|s| s.id == id)
                .ok_or("target object missing")
        })
        .transpose()?;
    if let Some(at) = at
        && st.model.slides[page].shapes[at].locked
    {
        return Err("object is locked".into());
    }
    if let Some(t) = replacement {
        if t.shapes.len() != 1 {
            return Err("accept exactly one object".into());
        }
        transfer::paste(st, t)?;
        let p = Arc::make_mut(&mut st.model);
        let slide = Arc::make_mut(&mut p.slides[page]);
        let mut shape = slide.shapes.pop().ok_or("accepted object missing")?;
        if let Some(x) = &mut shape.xfrm {
            x.x -= 18.;
            x.y -= 18.;
        }
        if let (Some(id), Some(at)) = (target, at) {
            shape.id = id;
            slide.shapes[at] = shape;
            st.selection.shapes = vec![id];
        } else {
            st.selection.shapes = vec![shape.id];
            slide.shapes.push(shape);
        }
    } else if let Some(id) = target {
        st.selection.shapes = vec![id];
        super::apply(st, Command::Delete)?;
    } else {
        return Err("accept requires a source or target object".into());
    }
    Ok(())
}
