//! Independent editable table model. Covered slots are empty and point to a unique rectangular anchor by topology.
use crate::{
    document::{Deck, Object, Result},
    text::{Run, Style},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub rows: Vec<f32>,
    pub columns: Vec<f32>,
    pub cells: Vec<Cell>,
    pub style: TableStyle,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub text: String,
    pub runs: Vec<Run>,
    pub span: [usize; 2],
    pub style: CellStyle,
}
impl Default for Cell {
    fn default() -> Self {
        Self {
            text: String::new(),
            runs: vec![],
            span: [1, 1],
            style: CellStyle::default(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellStyle {
    pub fill: Option<String>,
    pub color: Option<String>,
    pub family: Option<String>,
    pub size: Option<f32>,
    pub bold: Option<bool>,
    pub align: Option<String>,
    pub border: Option<String>,
    pub border_width: Option<f32>,
    pub padding: Option<f32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableStyle {
    pub header: bool,
    pub banded: bool,
    pub border: String,
    pub border_width: f32,
    pub header_fill: String,
    pub body_fill: String,
    pub band_fill: String,
    pub color: String,
    pub header_color: String,
    pub family: String,
    pub size: f32,
    pub padding: f32,
    pub align: String,
}
impl Default for TableStyle {
    fn default() -> Self {
        Self {
            header: true,
            banded: true,
            border: "#d8dfec".into(),
            border_width: 1.,
            header_fill: "@accent".into(),
            body_fill: "#ffffff".into(),
            band_fill: "#f2f4fa".into(),
            color: "#243247".into(),
            header_color: "#ffffff".into(),
            family: "@body".into(),
            size: 18.,
            padding: 6.,
            align: "left".into(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Range {
    pub top: usize,
    pub left: usize,
    pub bottom: usize,
    pub right: usize,
}
impl Range {
    pub fn cell(row: usize, col: usize) -> Self {
        Self {
            top: row,
            left: col,
            bottom: row,
            right: col,
        }
    }
    pub fn between(a: (usize, usize), b: (usize, usize)) -> Self {
        Self {
            top: a.0.min(b.0),
            left: a.1.min(b.1),
            bottom: a.0.max(b.0),
            right: a.1.max(b.1),
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum Action {
    SetCell(usize, usize, String, Vec<Run>),
    Paste(usize, usize, String),
    InsertRow(usize),
    InsertColumn(usize),
    DeleteRow(usize),
    DeleteColumn(usize),
    Merge(Range),
    Split(usize, usize),
    RowHeight(usize, f32),
    ColumnWidth(usize, f32),
    DistributeRows,
    DistributeColumns,
    Style(TableStyle),
    CellStyle(Range, CellStyle),
}
#[derive(Clone)]
pub struct Resolved {
    pub fill: String,
    pub text: Style,
    pub border: String,
    pub border_width: f32,
    pub padding: f32,
    pub align: String,
}
fn alignment(s: &str) -> bool {
    matches!(s, "left" | "center" | "right")
}
fn token(s: &str) -> bool {
    crate::document::color(s) || matches!(s, "@accent" | "@background")
}
fn family(s: &str) -> bool {
    !s.trim().is_empty()
        && s.len() <= 256
        && !s.chars().any(char::is_control)
        && (!s.starts_with('@') || s == "@body")
}
impl Table {
    pub fn new(rows: usize, cols: usize) -> Result<Self> {
        if rows == 0 || cols == 0 || rows > 128 || cols > 64 || rows * cols > 4096 {
            return Err("表格须1–128行/1–64列且≤4096格".into());
        }
        Ok(Self {
            rows: vec![44.; rows],
            columns: vec![112.; cols],
            cells: vec![Cell::default(); rows * cols],
            style: TableStyle::default(),
        })
    }
    pub fn index(&self, row: usize, col: usize) -> Result<usize> {
        if row >= self.rows.len() || col >= self.columns.len() {
            return Err("单元格已不存在".into());
        }
        Ok(row * self.columns.len() + col)
    }
    pub fn owner(&self, row: usize, col: usize) -> Result<(usize, usize)> {
        self.index(row, col)?;
        for (i, c) in self.cells.iter().enumerate() {
            let r = i / self.columns.len();
            let k = i % self.columns.len();
            if c.span[0] > 0 && r <= row && k <= col && r + c.span[0] > row && k + c.span[1] > col {
                return Ok((r, k));
            }
        }
        Err("单元格合并引用失效".into())
    }
    /// View selections include every merged cell that intersects the rectangle.
    pub fn selection_range(&self, a: (usize, usize), b: (usize, usize)) -> Result<Range> {
        self.index(a.0, a.1)?;
        self.index(b.0, b.1)?;
        let mut range = Range::between(a, b);
        loop {
            let before = range;
            for (i, c) in self.cells.iter().enumerate().filter(|(_, c)| c.span[0] > 0) {
                let r = i / self.columns.len();
                let col = i % self.columns.len();
                let bottom = r + c.span[0] - 1;
                let right = col + c.span[1] - 1;
                if r <= range.bottom
                    && bottom >= range.top
                    && col <= range.right
                    && right >= range.left
                {
                    range.top = range.top.min(r);
                    range.left = range.left.min(col);
                    range.bottom = range.bottom.max(bottom);
                    range.right = range.right.max(right);
                }
            }
            if range == before {
                return Ok(range);
            }
        }
    }
    pub fn bounds(&self, row: usize, col: usize) -> Result<[f32; 4]> {
        let (r, c) = self.owner(row, col)?;
        let cell = &self.cells[self.index(r, c)?];
        Ok([
            self.columns[..c].iter().sum(),
            self.rows[..r].iter().sum(),
            self.columns[c..c + cell.span[1]].iter().sum(),
            self.rows[r..r + cell.span[0]].iter().sum(),
        ])
    }
    pub fn validate(&self) -> Result<()> {
        if self.rows.is_empty()
            || self.columns.is_empty()
            || self.rows.len() > 128
            || self.columns.len() > 64
            || self.cells.len() != self.rows.len() * self.columns.len()
            || self.cells.len() > 4096
            || self
                .rows
                .iter()
                .chain(&self.columns)
                .any(|v| !v.is_finite() || *v < 8. || *v > 4096.)
        {
            return Err("表格行列/尺寸/预算无效（最小8pt）".into());
        }
        let mut occupied = vec![false; self.cells.len()];
        let mut chars = 0;
        for (i, cell) in self.cells.iter().enumerate() {
            crate::text::validate(&cell.text, &cell.runs)?;
            chars += cell.text.chars().count();
            if cell.text.len() > 40000 || chars > 100000 {
                return Err("表格文本超出预算".into());
            }
            let s = &cell.style;
            if s.border.as_ref().is_some_and(|v| !token(v))
                || s.border_width
                    .is_some_and(|v| !v.is_finite() || !(0. ..=10.).contains(&v))
                || s.padding
                    .is_some_and(|v| !v.is_finite() || !(0. ..=24.).contains(&v))
                || s.fill.as_ref().is_some_and(|v| !token(v))
                || s.color.as_ref().is_some_and(|v| !token(v))
                || s.family.as_ref().is_some_and(|v| !family(v))
                || s.size
                    .is_some_and(|v| !v.is_finite() || !(1. ..=512.).contains(&v))
                || s.align.as_ref().is_some_and(|v| !alignment(v))
            {
                return Err("单元格样式无效".into());
            }
            if cell.span == [0, 0] {
                if !cell.text.is_empty() || !cell.runs.is_empty() {
                    return Err("覆盖单元格不能含隐藏文字".into());
                }
                continue;
            }
            let row = i / self.columns.len();
            let col = i % self.columns.len();
            if cell.span[0] == 0
                || cell.span[1] == 0
                || row
                    .checked_add(cell.span[0])
                    .is_none_or(|n| n > self.rows.len())
                || col
                    .checked_add(cell.span[1])
                    .is_none_or(|n| n > self.columns.len())
            {
                return Err("表格合并越界".into());
            }
            for r in row..row + cell.span[0] {
                for c in col..col + cell.span[1] {
                    let k = self.index(r, c)?;
                    if occupied[k] || (k != i && self.cells[k].span != [0, 0]) {
                        return Err("表格合并重叠或覆盖失效".into());
                    }
                    occupied[k] = true;
                }
            }
        }
        if occupied.iter().any(|b| !*b) {
            return Err("表格存在无主覆盖单元格".into());
        }
        let s = &self.style;
        if ![
            &s.border,
            &s.header_fill,
            &s.body_fill,
            &s.band_fill,
            &s.color,
            &s.header_color,
        ]
        .iter()
        .all(|v| token(v))
            || !s.border_width.is_finite()
            || !(0. ..=10.).contains(&s.border_width)
            || !s.size.is_finite()
            || !(1. ..=512.).contains(&s.size)
            || !s.padding.is_finite()
            || !(0. ..=24.).contains(&s.padding)
            || !family(&s.family)
            || !alignment(&s.align)
        {
            return Err("表格主题/边框样式无效".into());
        }
        Ok(())
    }
    fn range(&self, r: Range) -> Result<()> {
        if r.top > r.bottom || r.left > r.right {
            return Err("非法单元格选区".into());
        }
        self.index(r.bottom, r.right)?;
        Ok(())
    }
    fn full_range(&self, r: Range) -> Result<()> {
        self.range(r)?;
        for row in r.top..=r.bottom {
            for col in r.left..=r.right {
                let (a, b) = self.owner(row, col)?;
                let c = &self.cells[self.index(a, b)?];
                if a < r.top
                    || b < r.left
                    || a + c.span[0] > r.bottom + 1
                    || b + c.span[1] > r.right + 1
                {
                    return Err("选区不能穿过已有合并区域，请扩展选区或先拆分".into());
                }
            }
        }
        Ok(())
    }
    pub fn apply(&mut self, a: Action) -> Result<()> {
        let mut draft = self.clone();
        draft.apply_inner(a)?;
        draft.validate()?;
        *self = draft;
        Ok(())
    }
    fn apply_inner(&mut self, a: Action) -> Result<()> {
        match a {
            Action::SetCell(r, c, text, runs) => {
                let (r, c) = self.owner(r, c)?;
                let i = self.index(r, c)?;
                self.cells[i].text = text;
                self.cells[i].runs = runs;
            }
            Action::Paste(r, c, value) => {
                let value = value.replace("\r\n", "\n").replace('\r', "\n");
                let value = value.strip_suffix('\n').unwrap_or(&value);
                let rows: Vec<Vec<_>> =
                    value.split('\n').map(|r| r.split('\t').collect()).collect();
                let width = rows[0].len();
                if rows.iter().any(|r| r.len() != width)
                    || rows.len() * width > 4096
                    || value.len() > 400000
                {
                    return Err("TSV必须为有界矩形".into());
                }
                self.index(
                    r.checked_add(rows.len() - 1).ok_or("TSV行越界")?,
                    c.checked_add(width - 1).ok_or("TSV列越界")?,
                )?;
                for (dy, row) in rows.iter().enumerate() {
                    for (dx, text) in row.iter().enumerate() {
                        let i = self.index(r + dy, c + dx)?;
                        if self.cells[i].span != [1, 1]
                            && !(rows.len() == 1 && width == 1 && self.cells[i].span[0] > 0)
                        {
                            return Err("TSV粘贴与合并区域冲突，请先拆分".into());
                        }
                        self.cells[i].text = (*text).into();
                        self.cells[i].runs.clear();
                    }
                }
            }
            Action::Merge(r) => {
                self.full_range(r)?;
                if r.top == r.bottom && r.left == r.right {
                    return Ok(());
                }
                let mut text = String::new();
                let mut runs = Vec::<Run>::new();
                for row in r.top..=r.bottom {
                    for col in r.left..=r.right {
                        let i = self.index(row, col)?;
                        if !self.cells[i].text.is_empty() {
                            if !text.is_empty() {
                                let start = text.len();
                                text.push('\n');
                                if let Some(last) = runs.last_mut() {
                                    last.end = start + 1;
                                }
                            }
                            let offset = text.len();
                            text.push_str(&self.cells[i].text);
                            for run in &self.cells[i].runs {
                                let mut run = run.clone();
                                run.start += offset;
                                run.end += offset;
                                runs.push(run);
                            }
                        }
                        if row != r.top || col != r.left {
                            self.cells[i] = Cell {
                                span: [0, 0],
                                ..Cell::default()
                            };
                        }
                    }
                }
                let i = self.index(r.top, r.left)?;
                self.cells[i].text = text;
                self.cells[i].runs = runs;
                self.cells[i].span = [r.bottom - r.top + 1, r.right - r.left + 1];
            }
            Action::Split(r, c) => {
                let (r, c) = self.owner(r, c)?;
                let i = self.index(r, c)?;
                let span = self.cells[i].span;
                self.cells[i].span = [1, 1];
                for y in r..r + span[0] {
                    for x in c..c + span[1] {
                        if y != r || x != c {
                            let i = self.index(y, x)?;
                            self.cells[i] = Cell::default();
                        }
                    }
                }
            }
            Action::InsertRow(at) => self.insert(true, at)?,
            Action::InsertColumn(at) => self.insert(false, at)?,
            Action::DeleteRow(at) => self.delete(true, at)?,
            Action::DeleteColumn(at) => self.delete(false, at)?,
            Action::RowHeight(r, h) => *self.rows.get_mut(r).ok_or("行不存在")? = h,
            Action::ColumnWidth(c, w) => *self.columns.get_mut(c).ok_or("列不存在")? = w,
            Action::DistributeRows => {
                let h = self.rows.iter().sum::<f32>() / self.rows.len() as f32;
                self.rows.fill(h);
            }
            Action::DistributeColumns => {
                let w = self.columns.iter().sum::<f32>() / self.columns.len() as f32;
                self.columns.fill(w);
            }
            Action::Style(style) => self.style = style,
            Action::CellStyle(r, style) => {
                self.full_range(r)?;
                for row in r.top..=r.bottom {
                    for col in r.left..=r.right {
                        let i = self.index(row, col)?;
                        if self.cells[i].span[0] > 0 {
                            self.cells[i].style = style.clone();
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn insert(&mut self, row: bool, at: usize) -> Result<()> {
        let old_rows = self.rows.len();
        let old_cols = self.columns.len();
        let count = if row { old_rows } else { old_cols };
        if at > count {
            return Err("插入位置越界".into());
        }
        let mut merged = vec![];
        for (i, c) in self.cells.iter().enumerate() {
            if c.span == [0, 0] {
                continue;
            }
            let mut r = i / old_cols;
            let mut k = i % old_cols;
            let mut c = c.clone();
            let pos = if row { r } else { k };
            if pos >= at {
                if row { r += 1 } else { k += 1 }
            } else if pos + if row { c.span[0] } else { c.span[1] } > at {
                if row { c.span[0] += 1 } else { c.span[1] += 1 }
            }
            merged.push((r, k, c));
        }
        if row {
            self.rows.insert(at, 44.);
        } else {
            self.columns.insert(at, 112.);
        }
        self.rebuild(merged)
    }
    fn delete(&mut self, row: bool, at: usize) -> Result<()> {
        let count = if row {
            self.rows.len()
        } else {
            self.columns.len()
        };
        if at >= count || count == 1 {
            return Err("至少保留一行/列".into());
        }
        let old_cols = self.columns.len();
        let mut anchors = vec![];
        for (i, c) in self.cells.iter().enumerate() {
            if c.span == [0, 0] {
                continue;
            }
            let (mut r, mut k) = (i / old_cols, i % old_cols);
            let mut c = c.clone();
            let pos = if row { r } else { k };
            let span = if row { c.span[0] } else { c.span[1] };
            if pos > at {
                if row { r -= 1 } else { k -= 1 }
            } else if pos <= at && pos + span > at {
                if span == 1 {
                    continue;
                }
                if row { c.span[0] -= 1 } else { c.span[1] -= 1 }
            }
            anchors.push((r, k, c));
        }
        if row {
            self.rows.remove(at);
        } else {
            self.columns.remove(at);
        }
        self.rebuild(anchors)
    }
    fn rebuild(&mut self, anchors: Vec<(usize, usize, Cell)>) -> Result<()> {
        if self.rows.len() > 128
            || self.columns.len() > 64
            || self.rows.len() * self.columns.len() > 4096
        {
            return Err("表格数量超限".into());
        }
        self.cells = vec![Cell::default(); self.rows.len() * self.columns.len()];
        for (r, c, cell) in anchors {
            let i = self.index(r, c)?;
            for y in r..r + cell.span[0] {
                for x in c..c + cell.span[1] {
                    let k = self.index(y, x)?;
                    self.cells[k] = Cell {
                        span: [0, 0],
                        ..Cell::default()
                    };
                }
            }
            self.cells[i] = cell;
        }
        Ok(())
    }
    pub fn next(&self, r: usize, c: usize, reverse: bool) -> Option<(usize, usize)> {
        let (r, c) = self.owner(r, c).ok()?;
        let i = self.index(r, c).ok()?;
        let anchors: Vec<_> = self
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.span[0] > 0)
            .map(|(i, _)| i)
            .collect();
        let at = anchors.iter().position(|v| *v == i)?;
        let next = if reverse { at.checked_sub(1)? } else { at + 1 };
        anchors
            .get(next)
            .map(|i| (i / self.columns.len(), i % self.columns.len()))
    }
}
pub fn object(rows: usize, cols: usize) -> Result<Object> {
    let table = Table::new(rows, cols)?;
    let mut o = crate::identity::object("table");
    o.width = table.columns.iter().sum();
    o.height = table.rows.iter().sum();
    o.table = Some(table);
    Ok(o)
}
pub fn validate(o: &Object) -> Result<()> {
    match (&o.table, o.kind.as_str()) {
        (Some(t), "table") => {
            t.validate()?;
            if (t.columns.iter().sum::<f32>() - o.width).abs() > 0.01
                || (t.rows.iter().sum::<f32>() - o.height).abs() > 0.01
            {
                return Err("表格尺寸与行列不一致".into());
            }
        }
        (None, "table") | (Some(_), _) => return Err("表格对象与数据不匹配".into()),
        _ => {}
    }
    Ok(())
}
pub fn resize(o: &mut Object) {
    if let Some(t) = &mut o.table {
        let w = t.columns.iter().sum::<f32>();
        let h = t.rows.iter().sum::<f32>();
        for c in &mut t.columns {
            *c *= o.width / w;
        }
        for r in &mut t.rows {
            *r *= o.height / h;
        }
    }
}
pub fn scale_content(o: &mut Object, factor: f32) {
    if let Some(t) = &mut o.table {
        t.style.size *= factor;
        t.style.padding *= factor;
        t.style.border_width *= factor;
        for c in &mut t.cells {
            if let Some(v) = &mut c.style.size {
                *v *= factor;
            }
            if let Some(v) = &mut c.style.padding {
                *v *= factor;
            }
            if let Some(v) = &mut c.style.border_width {
                *v *= factor;
            }
            for r in &mut c.runs {
                r.style.size *= factor;
            }
        }
    }
}
pub fn theme_value(deck: &Deck, page: usize, value: &str, font: bool) -> String {
    if let Some(slot) = value.strip_prefix('@') {
        let theme = deck
            .slides
            .get(page)
            .and_then(|s| s.layout.as_ref())
            .and_then(|id| deck.design.layouts.get(id))
            .and_then(|l| deck.design.masters.get(&l.master))
            .and_then(|m| deck.design.themes.get(&m.theme))
            .or_else(|| deck.design.themes.values().next());
        return theme
            .and_then(|t| {
                if font {
                    t.fonts.get(slot)
                } else {
                    t.colors.get(slot)
                }
            })
            .cloned()
            .unwrap_or_else(|| {
                if font {
                    "sans-serif".into()
                } else if slot == "background" {
                    "#ffffff".into()
                } else {
                    "#6d5ce7".into()
                }
            });
    }
    value.into()
}
pub fn resolve(deck: &Deck, page: usize, t: &Table, r: usize, c: usize) -> Resolved {
    let cell = &t.cells[r * t.columns.len() + c];
    let s = &t.style;
    let header = s.header && r == 0;
    let fill = cell.style.fill.as_ref().unwrap_or(if header {
        &s.header_fill
    } else if s.banded && r.is_multiple_of(2) {
        &s.band_fill
    } else {
        &s.body_fill
    });
    Resolved {
        fill: theme_value(deck, page, fill, false),
        text: Style {
            family: theme_value(
                deck,
                page,
                cell.style.family.as_ref().unwrap_or(&s.family),
                true,
            ),
            size: cell.style.size.unwrap_or(s.size),
            bold: cell.style.bold.unwrap_or(header),
            italic: false,
            color: theme_value(
                deck,
                page,
                cell.style.color.as_ref().unwrap_or(if header {
                    &s.header_color
                } else {
                    &s.color
                }),
                false,
            ),
        },
        border: theme_value(
            deck,
            page,
            cell.style.border.as_ref().unwrap_or(&s.border),
            false,
        ),
        border_width: cell.style.border_width.unwrap_or(s.border_width),
        padding: cell.style.padding.unwrap_or(s.padding),
        align: cell.style.align.clone().unwrap_or_else(|| s.align.clone()),
    }
}
pub fn cell_object(deck: &Deck, page: usize, o: &Object, r: usize, c: usize) -> Result<Object> {
    let t = o.table.as_ref().ok_or("不是表格")?;
    let (r, c) = t.owner(r, c)?;
    let [x, y, w, h] = t.bounds(r, c)?;
    let style = resolve(deck, page, t, r, c);
    let cell = &t.cells[t.index(r, c)?];
    let mut text = crate::identity::object("text");
    text.id = format!("{}:cell:{r}:{c}", o.id);
    text.x = o.x + x + style.padding;
    text.y = o.y + y + style.padding;
    text.width = (w - 2. * style.padding).max(1.);
    text.height = (h - 2. * style.padding).max(1.);
    text.text = cell.text.clone();
    text.font_size = style.text.size;
    text.fill = style.text.color.clone();
    text.opacity = o.opacity;
    text.text_align = style.align;
    text.runs = if cell.runs.is_empty() && !cell.text.is_empty() {
        vec![Run {
            start: 0,
            end: cell.text.len(),
            style: style.text,
        }]
    } else {
        cell.runs.clone()
    };
    Ok(text)
}
pub fn expand(deck: &Deck, page: usize, objects: &[Object]) -> Result<Vec<Object>> {
    let mut out = vec![];
    for o in objects {
        let Some(t) = &o.table else {
            out.push(o.clone());
            continue;
        };
        for (i, cell) in t.cells.iter().enumerate() {
            if cell.span == [0, 0] {
                continue;
            }
            let (r, c) = (i / t.columns.len(), i % t.columns.len());
            let [x, y, w, h] = t.bounds(r, c)?;
            let style = resolve(deck, page, t, r, c);
            let mut bg = crate::identity::object("rect");
            bg.id = format!("{}:bg:{i}", o.id);
            bg.x = o.x + x;
            bg.y = o.y + y;
            bg.width = w;
            bg.height = h;
            bg.fill = style.fill;
            bg.stroke = style.border;
            bg.stroke_width = style.border_width;
            bg.opacity = o.opacity;
            let mut text = cell_object(deck, page, o, r, c)?;
            for item in [&mut bg, &mut text] {
                let dx = item.x + item.width / 2. - o.x - o.width / 2.;
                let dy = item.y + item.height / 2. - o.y - o.height / 2.;
                let (s, co) = o.rotation.to_radians().sin_cos();
                item.x = o.x + o.width / 2. + dx * co - dy * s - item.width / 2.;
                item.y = o.y + o.height / 2. + dx * s + dy * co - item.height / 2.;
                item.rotation = o.rotation;
            }
            out.push(bg);
            out.push(text);
        }
    }
    Ok(out)
}
pub fn hit(o: &Object, x: f32, y: f32) -> Option<(usize, usize)> {
    let t = o.table.as_ref()?;
    let (dx, dy) = (x - o.x - o.width / 2., y - o.y - o.height / 2.);
    let (s, c) = (-o.rotation).to_radians().sin_cos();
    let (x, y) = (
        dx * c - dy * s + o.width / 2.,
        dx * s + dy * c + o.height / 2.,
    );
    if x < 0. || y < 0. || x > o.width || y > o.height {
        return None;
    }
    let mut edge = 0.;
    let col = t.columns.iter().position(|w| {
        edge += w;
        edge >= x
    })?;
    edge = 0.;
    let row = t.rows.iter().position(|h| {
        edge += h;
        edge >= y
    })?;
    t.owner(row, col).ok()
}
pub fn freeze(deck: &Deck, page: usize, o: &mut Object) {
    if let Some(t) = &mut o.table {
        let s = &mut t.style;
        for value in [
            &mut s.border,
            &mut s.header_fill,
            &mut s.body_fill,
            &mut s.band_fill,
            &mut s.color,
            &mut s.header_color,
        ] {
            *value = theme_value(deck, page, value, false);
        }
        s.family = theme_value(deck, page, &s.family, true);
        for cell in &mut t.cells {
            if let Some(v) = &mut cell.style.fill {
                *v = theme_value(deck, page, v, false);
            }
            if let Some(v) = &mut cell.style.color {
                *v = theme_value(deck, page, v, false);
            }
            if let Some(v) = &mut cell.style.border {
                *v = theme_value(deck, page, v, false);
            }
            if let Some(v) = &mut cell.style.family {
                *v = theme_value(deck, page, v, true);
            }
        }
    }
}
