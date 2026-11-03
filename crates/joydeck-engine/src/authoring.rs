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
#[serde(deny_unknown_fields)]
pub struct TextEdit {
    pub target: TextTarget,
    pub body: TextBody,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellRange {
    pub row: usize,
    pub col: usize,
    pub end_row: usize,
    pub end_col: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CellFormat {
    pub fill_rgb: Option<u32>,
    pub clear_fill: bool,
    pub borders: Option<CellBorders>,
    pub margins: Option<[f64; 4]>,
    pub margin_patch: Option<[Option<f64>; 4]>,
    pub anchor: Option<joydeck_model::text::Anchor>,
    pub vertical: Option<joydeck_model::text::TextDir>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellBorders {
    pub sides: [bool; 4],
    pub rgb: u32,
    pub width: f64,
    pub remove: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TableAction {
    Format {
        range: CellRange,
        format: CellFormat,
    },
    Style {
        name: String,
        first_row: bool,
        band_row: bool,
    },
    RowHeight {
        row: usize,
        height: f64,
    },
    ColWidth {
        col: usize,
        width: f64,
    },
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
            | Command::SetTexts { .. }
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
        Command::SetTexts { edits } => {
            if edits.is_empty() || edits.len() > 256 {
                return Err("text transaction requires 1–256 targets".into());
            }
            let mut bytes = 0usize;
            for (i, edit) in edits.iter().enumerate() {
                if edits[..i].iter().any(|prior| prior.target == edit.target) {
                    return Err("duplicate text target".into());
                }
                bytes = bytes.saturating_add(edit.body.text().len());
                if bytes > 1 << 20 {
                    return Err("text transaction exceeds 1 MiB".into());
                }
                set_body(p, page, &edit.target, edit.body.clone())?;
            }
            Ok(())
        }
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
                TableAction::Style {
                    name,
                    first_row,
                    band_row,
                } => {
                    if !table::table_styles().iter().any(|(id, _)| id == name)
                        && !p.table_styles.iter().any(|style| &style.id == name)
                    {
                        return Err("unknown table style".into());
                    }
                    t.style = name.clone();
                    t.first_row = *first_row;
                    t.band_row = *band_row;
                }
                TableAction::RowHeight { row, height } => {
                    if !height.is_finite() || !(4. ..=4000.).contains(height) {
                        return Err("row height must be 4–4000 pt".into());
                    }
                    t.rows.get_mut(*row).ok_or("missing row")?.height = *height;
                }
                TableAction::ColWidth { col, width } => {
                    if !width.is_finite() || !(4. ..=4000.).contains(width) {
                        return Err("column width must be 4–4000 pt".into());
                    }
                    *t.cols.get_mut(*col).ok_or("missing column")? = *width;
                }
                TableAction::Format { range, format } => {
                    if range.row > range.end_row
                        || range.col > range.end_col
                        || range.end_row >= t.n_rows()
                        || range.end_col >= t.n_cols()
                    {
                        return Err("cell range outside table".into());
                    }
                    for (row, data) in t.rows.iter().enumerate() {
                        for (col, cell) in data.cells.iter().enumerate() {
                            if cell.is_covered() {
                                continue;
                            }
                            let end_row = row.saturating_add(cell.row_span.max(1) as usize - 1);
                            let end_col = col.saturating_add(cell.grid_span.max(1) as usize - 1);
                            let intersects = row <= range.end_row
                                && end_row >= range.row
                                && col <= range.end_col
                                && end_col >= range.col;
                            if intersects
                                && (row < range.row
                                    || col < range.col
                                    || end_row > range.end_row
                                    || end_col > range.end_col)
                            {
                                return Err("select the complete merged cell".into());
                            }
                        }
                    }
                    if format.fill_rgb.is_some_and(|rgb| rgb > 0xffffff)
                        || format.margins.is_some_and(|v| {
                            v.iter()
                                .any(|n| !n.is_finite() || !(0. ..=720.).contains(n))
                        })
                        || format.borders.as_ref().is_some_and(|b| {
                            b.rgb > 0xffffff
                                || !b.width.is_finite()
                                || !(0. ..=100.).contains(&b.width)
                        })
                        || format.margin_patch.is_some_and(|v| {
                            v.iter()
                                .flatten()
                                .any(|n| !n.is_finite() || !(0. ..=720.).contains(n))
                        })
                    {
                        return Err("invalid cell color, border or margins".into());
                    }
                    for row in range.row..=range.end_row {
                        for col in range.col..=range.end_col {
                            let cell = t.cell_mut(row, col).ok_or("missing cell")?;
                            if cell.is_covered() {
                                continue;
                            }
                            if format.clear_fill {
                                cell.fill = None;
                            }
                            if let Some(rgb) = format.fill_rgb {
                                cell.fill = Some(Fill::solid(ColorRef::rgb(Rgba::rgb(
                                    (rgb >> 16) as u8,
                                    (rgb >> 8) as u8,
                                    rgb as u8,
                                ))));
                            }
                            if let Some(b) = &format.borders {
                                for (side, selected) in b.sides.iter().enumerate() {
                                    if *selected {
                                        cell.borders[side] = if b.remove {
                                            Some(Line {
                                                fill: Some(Fill::None),
                                                ..Default::default()
                                            })
                                        } else {
                                            Some(Line {
                                                fill: Some(Fill::solid(ColorRef::rgb(Rgba::rgb(
                                                    (b.rgb >> 16) as u8,
                                                    (b.rgb >> 8) as u8,
                                                    b.rgb as u8,
                                                )))),
                                                width: Some(b.width),
                                                ..Default::default()
                                            })
                                        };
                                    }
                                }
                            }
                            if let Some(v) = format.margins {
                                cell.margins = Some(v);
                            }
                            if let Some(patch) = format.margin_patch
                                && patch.iter().any(Option::is_some)
                            {
                                let mut margins = cell.margins.unwrap_or([7.2, 7.2, 3.6, 3.6]);
                                for (n, value) in margins.iter_mut().zip(patch) {
                                    if let Some(value) = value {
                                        *n = value;
                                    }
                                }
                                cell.margins = Some(margins);
                            }
                            if let Some(v) = format.anchor {
                                cell.anchor = Some(v);
                            }
                            if let Some(v) = format.vertical {
                                cell.vertical = Some(v);
                            }
                        }
                    }
                }
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
                if t.width() > 32768. || t.height() > 32768. {
                    return Err("table dimensions exceed coordinate budget".into());
                }
                x.w = t.width();
                x.h = t.height();
            }
            Ok(())
        }
        _ => Err("unsupported authoring command".into()),
    }
}
