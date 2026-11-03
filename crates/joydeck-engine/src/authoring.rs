//! R3 authoring transactions. UI composition state never enters the persistent snapshot.
use super::*;
use joydeck_model::table::Table;
use std::result::Result;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TextTarget {
    Shape {
        shape: ShapeId,
    },
    Cell {
        shape: ShapeId,
        row: usize,
        col: usize,
    },
    Notes,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TableAction {
    InsertRow {
        at: usize,
    },
    InsertCol {
        at: usize,
    },
    DeleteRow {
        at: usize,
    },
    DeleteCol {
        at: usize,
    },
    Merge {
        row: usize,
        col: usize,
        end_row: usize,
        end_col: usize,
    },
    Split {
        row: usize,
        col: usize,
    },
    PasteTsv {
        row: usize,
        col: usize,
        text: String,
    },
    Fill {
        row: usize,
        col: usize,
        rgb: u32,
    },
}
pub fn body(p: &Presentation, page: usize, target: &TextTarget) -> Result<TextBody, String> {
    let slide = p.slides.get(page).ok_or("no active page")?;
    if matches!(target, TextTarget::Notes) {
        return Ok(slide.notes.clone());
    }
    let id = match target {
        TextTarget::Shape { shape } | TextTarget::Cell { shape, .. } => *shape,
        _ => return Err("invalid target".into()),
    };
    let s = slide
        .shapes
        .iter()
        .find(|s| s.id == id)
        .ok_or("top-level text target missing")?;
    if s.locked {
        return Err("object is locked".into());
    }
    match (target, &s.kind) {
        (TextTarget::Cell { row, col, .. }, ShapeKind::Table(t)) => Ok(t
            .cell(*row, *col)
            .filter(|c| !c.is_covered())
            .ok_or("cell is missing or merged/covered")?
            .text
            .clone()),
        (TextTarget::Shape { .. }, ShapeKind::Shape | ShapeKind::Connector { .. }) => {
            Ok(s.text.clone().unwrap_or_default())
        }
        _ => Err("target does not contain editable text".into()),
    }
}
pub fn set_body(
    p: &mut Presentation,
    page: usize,
    target: &TextTarget,
    body: TextBody,
) -> Result<(), String> {
    self::body(p, page, target)?;
    text::validate(&body)?;
    let slide = Arc::make_mut(&mut p.slides[page]);
    match target {
        TextTarget::Notes => slide.notes = body,
        TextTarget::Shape { shape } => {
            slide
                .shapes
                .iter_mut()
                .find(|s| s.id == *shape)
                .ok_or("missing shape")?
                .text = Some(body)
        }
        TextTarget::Cell { shape, row, col } => {
            let s = slide
                .shapes
                .iter_mut()
                .find(|s| s.id == *shape)
                .ok_or("missing table")?;
            if let ShapeKind::Table(t) = &mut s.kind {
                t.cell_mut(*row, *col).ok_or("missing cell")?.text = body;
            }
        }
    }
    Ok(())
}
pub fn apply(st: &mut EditorSnapshot, cmd: &Command) -> Option<Result<(), String>> {
    if !matches!(
        cmd,
        Command::SetText { .. }
            | Command::EditText { .. }
            | Command::InsertTable { .. }
            | Command::Table { .. }
    ) {
        return None;
    }
    Some(run(st, cmd))
}
fn run(st: &mut EditorSnapshot, cmd: &Command) -> Result<(), String> {
    let page = st.selection.page;
    let p = Arc::make_mut(&mut st.model);
    match cmd {
        Command::SetText { target, body } => set_body(p, page, target, body.as_ref().clone()),
        Command::EditText {
            target,
            start,
            end,
            text: value,
        } => {
            let mut b = body(p, page, target)?;
            let range = text::utf16_range(&b.text(), *start..*end).ok_or("invalid UTF-16 range")?;
            text::replace(&mut b, range, value)?;
            set_body(p, page, target, b)
        }
        Command::InsertTable {
            x,
            y,
            rows,
            cols,
            width,
            row_height,
        } => {
            if !(1..=100).contains(rows) || !(1..=100).contains(cols) {
                return Err("table budget is 100 rows/columns".into());
            }
            for v in [*x, *y, *width, *row_height] {
                checked(v)?;
            }
            if *width < 10. || *row_height < 4. || row_height * *rows as f64 > 32768. {
                return Err("invalid table dimensions".into());
            }
            let id = ShapeId(next_id(p)?);
            Arc::make_mut(&mut p.slides[page]).shapes.push(Shape {
                id,
                name: "Table".into(),
                xfrm: Some(Xfrm::new(*x, *y, *width, row_height * *rows as f64)),
                kind: ShapeKind::Table(Table::new(*rows, *cols, *width, *row_height)),
                ..Default::default()
            });
            st.selection.shapes = vec![id];
            Ok(())
        }
        Command::Table { action } => {
            if st.selection.shapes.len() != 1 {
                return Err("select one table".into());
            }
            let s = Arc::make_mut(&mut p.slides[page])
                .shapes
                .iter_mut()
                .find(|s| s.id == st.selection.shapes[0])
                .ok_or("missing table")?;
            if s.locked {
                return Err("object is locked".into());
            }
            let ShapeKind::Table(t) = &mut s.kind else {
                return Err("select a table".into());
            };
            if t.n_rows() > 100 || t.n_cols() > 100 {
                return Err("large table editing exceeds budget".into());
            }
            match action {
                TableAction::InsertRow { at } if *at <= t.n_rows() && t.n_rows() < 100 => {
                    t.insert_row(*at)
                }
                TableAction::InsertCol { at } if *at <= t.n_cols() && t.n_cols() < 100 => {
                    t.insert_col(*at)
                }
                TableAction::DeleteRow { at } if *at < t.n_rows() && t.n_rows() > 1 => {
                    t.delete_row(*at);
                }
                TableAction::DeleteCol { at } if *at < t.n_cols() && t.n_cols() > 1 => {
                    t.delete_col(*at);
                }
                TableAction::Merge {
                    row,
                    col,
                    end_row,
                    end_col,
                } => {
                    if !t.merge(*row, *col, *end_row, *end_col) {
                        return Err("invalid merge".into());
                    }
                }
                TableAction::Split { row, col } => {
                    if !t.split(*row, *col) {
                        return Err("cell is not a merge anchor".into());
                    }
                }
                TableAction::Fill { row, col, rgb } => {
                    if *rgb > 0xffffff {
                        return Err("invalid RGB".into());
                    }
                    t.cell_mut(*row, *col).ok_or("missing cell")?.fill = Some(Fill::solid(
                        ColorRef::rgb(Rgba::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8)),
                    ));
                }
                TableAction::PasteTsv {
                    row,
                    col,
                    text: value,
                } => {
                    if value.len() > 65536 {
                        return Err("TSV exceeds 64 KiB".into());
                    }
                    let value = value.replace("\r\n", "\n").replace('\r', "\n");
                    let rows: Vec<Vec<&str>> = value
                        .strip_suffix('\n')
                        .unwrap_or(&value)
                        .split('\n')
                        .map(|r| r.split('\t').collect())
                        .collect();
                    if rows.len() > t.n_rows().saturating_sub(*row)
                        || rows
                            .iter()
                            .any(|r| r.len() > t.n_cols().saturating_sub(*col))
                    {
                        return Err("paste rectangle exceeds table".into());
                    }
                    for (dr, r) in rows.iter().enumerate() {
                        for (dc, value) in r.iter().enumerate() {
                            let c = t
                                .cell_mut(row + dr, col + dc)
                                .filter(|c| !c.is_covered())
                                .ok_or("paste intersects merged cells")?;
                            let end = c.text.text().len();
                            text::replace(&mut c.text, 0..end, value)?;
                        }
                    }
                }
                _ => return Err("table boundary or budget reached".into()),
            }
            if let Some(x) = &mut s.xfrm {
                x.w = t.width();
                x.h = t.height();
            }
            Ok(())
        }
        _ => Err("unsupported authoring command".into()),
    }
}
