//! Resolve defined names and structured references at the formula cell. The graph
//! and evaluator use the same scope, relative addressing and table boundaries.
use super::*;

pub(super) enum Target {
    Expression { sheet: usize, expr: Expr },
    Table(Area),
}

pub(super) fn name_target(wb: &Workbook, sheet: usize, at: CellRef, name: &str) -> Option<Target> {
    let (lookup_sheet, bare) = match name.split_once('!') {
        Some((qualifier, bare)) => (wb.sheet_index(&joysheet_core::unquote_sheet(qualifier))?, bare),
        None => (sheet, name),
    };
    if let Some(definition) = wb.name(bare, lookup_sheet) {
        let mut expr = joysheet_formula::parse_name(&definition.formula).ok()?;
        if joysheet_formula::adjust::has_relative_refs(&expr) {
            expr = joysheet_formula::adjust::shift_relative_wrapping(expr, i64::from(at.row), i64::from(at.col));
        }
        return Some(Target::Expression { sheet: definition.scope.unwrap_or(sheet), expr });
    }
    let (sheet, index) = wb.table(bare)?;
    let range = wb.sheet(sheet)?.tables.get(index)?.data_range()?;
    Some(Target::Table(Area { sheet, range }))
}

pub(super) fn table_area(wb: &Workbook, sheet: usize, at: CellRef, reference: &StructRef) -> Result<Area, CellError> {
    let (sheet, index) = if reference.table.is_empty() {
        wb.sheet(sheet).and_then(|s| s.tables.iter().position(|t| t.range.contains(at))).map(|i| (sheet, i))
    } else {
        wb.table(&reference.table)
    }
    .ok_or(CellError::Ref)?;
    let table = wb.sheet(sheet).and_then(|s| s.tables.get(index)).ok_or(CellError::Ref)?;
    let items = &reference.specifiers;
    let (top, bottom) = if items.is_empty() {
        table.data_range().map(|r| (r.start.row, r.end.row)).ok_or(CellError::Ref)?
    } else if items.contains(&StructItem::ThisRow) {
        if !(table.range.start.row..=table.range.end.row).contains(&at.row) {
            return Err(CellError::Value);
        }
        (at.row, at.row)
    } else if items.contains(&StructItem::All) {
        (table.range.start.row, table.range.end.row)
    } else {
        let mut top = u32::MAX;
        let mut bottom = 0;
        for item in items {
            let span = match item {
                StructItem::Headers if table.header_row => Some((table.range.start.row, table.range.start.row)),
                StructItem::Totals if table.totals_row => Some((table.range.end.row, table.range.end.row)),
                StructItem::Data => table.data_range().map(|r| (r.start.row, r.end.row)),
                _ => return Err(CellError::Ref),
            };
            if let Some((first, last)) = span {
                top = top.min(first);
                bottom = bottom.max(last);
            }
        }
        if top > bottom {
            return Err(CellError::Ref);
        }
        (top, bottom)
    };
    let (left, right) = match &reference.col_start {
        None => (table.range.start.col, table.range.end.col),
        Some(first) => {
            let column = |name: &str| table.column_index(name).map(|i| table.range.start.col + i as u32).ok_or(CellError::Ref);
            let a = column(first)?;
            let b = reference.col_end.as_deref().map(column).transpose()?.unwrap_or(a);
            (a.min(b), a.max(b))
        }
    };
    Ok(Area { sheet, range: RangeRef::new(CellRef::new(top, left), CellRef::new(bottom, right)) })
}
