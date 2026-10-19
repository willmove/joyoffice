//! Preferred column widths and the table's horizontal origin.
//!
//! ECMA-376 5th edition: `tblLayout` §17.4.52 (omitted means `auto`),
//! `tblW` §17.4.63, `tcW` §17.4.71, `jc` §17.4.28, `tblInd` §17.4.50.
//! Percentage table widths are fiftieths of the page text extent. Percentage
//! cell widths are fiftieths of the table width. This is not Word's content-
//! measured autofit: columns are not grown to non-breaking text.

use crate::block::{Table, TableAlign, TableLayoutKind, TableWidth, VerticalMerge};
use crate::props::Measure;

/// Column widths and the origin from the content left, both in twips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableMetrics {
    pub columns: Vec<i64>,
    /// Twips from the content left. Centre and right may be negative when a
    /// fixed table is wider than the content box.
    pub origin: i64,
}

/// Resolves `table` against `content_width_twips`.
///
/// `None` means the grid is not twips, a percentage is not positive, the
/// indent is negative, or a product overflows. An omitted `tblLayout` follows
/// §17.4.52 and shrinks the preferred widths when they are wider than the
/// content box. Explicit `fixed` does not shrink. Nothing here grows a table
/// to fit unwrapped text.
pub fn table_metrics(table: &Table, content_width_twips: i64) -> Option<TableMetrics> {
    if content_width_twips <= 0 || table.grid.is_empty() {
        return None;
    }
    let grid = grid_twips(table)?;
    let indent = match table.indent {
        Some(value) if value < 0 => return None,
        Some(value) => value,
        None => 0,
    };
    let pct = pct_columns(table, grid.len())?;
    let target = preferred_width(table.width, content_width_twips, &grid, pct.is_some())?;
    let mut columns = assign_columns(&grid, target, pct.as_deref())?;
    let shrink = !matches!(table.layout, Some(TableLayoutKind::Fixed));
    let sum = sum_twips(&columns)?;
    if shrink && sum > content_width_twips {
        scale_to(&mut columns, content_width_twips)?;
    }
    let width = sum_twips(&columns)?;
    if width <= 0 {
        return None;
    }
    let origin = match table.alignment {
        None | Some(TableAlign::Left) => indent,
        Some(TableAlign::Center) => content_width_twips.checked_sub(width)? / 2,
        Some(TableAlign::Right) => content_width_twips.checked_sub(width)?,
    };
    Some(TableMetrics { columns, origin })
}

fn grid_twips(table: &Table) -> Option<Vec<i64>> {
    let mut columns = Vec::with_capacity(table.grid.len());
    for measure in &table.grid {
        match measure {
            Measure::Twips(value) if *value >= 0 => columns.push(*value),
            _ => return None,
        }
    }
    Some(columns)
}

/// First span-1, non-continue percentage in document order. Later percentages
/// on the same column are ignored. A multi-column percentage is stored but
/// does not replace the grid.
fn pct_columns(table: &Table, columns: usize) -> Option<Option<Vec<Option<i64>>>> {
    let mut found = vec![None; columns];
    let mut any = false;
    for row in &table.rows {
        let mut cursor = row.grid_before as usize;
        for cell in &row.cells {
            let span = cell.grid_span as usize;
            if span == 1
                && cursor < columns
                && cell.vertical_merge != VerticalMerge::Continue
                && let Some(Measure::PercentFiftieths(value)) = cell.width
            {
                if value <= 0 {
                    return None;
                }
                if found[cursor].is_none() {
                    found[cursor] = Some(value);
                    any = true;
                }
            }
            cursor = cursor.saturating_add(span);
        }
    }
    if any { Some(Some(found)) } else { Some(None) }
}

fn preferred_width(
    width: Option<TableWidth>,
    content: i64,
    grid: &[i64],
    has_pct: bool,
) -> Option<i64> {
    let target = match width {
        Some(TableWidth::Pct(value)) => {
            if value <= 0 {
                return None;
            }
            let width = content.checked_mul(value)?.checked_div(5000)?;
            if width <= 0 { None } else { Some(width) }
        }
        Some(TableWidth::Dxa(value)) if value > 0 => Some(value),
        Some(TableWidth::Dxa(_)) | Some(TableWidth::Auto) | None => {
            if has_pct {
                Some(content)
            } else {
                let sum = sum_twips(grid)?;
                if sum <= 0 { None } else { Some(sum) }
            }
        }
    }?;
    Some(target)
}

fn assign_columns(grid: &[i64], target: i64, pct: Option<&[Option<i64>]>) -> Option<Vec<i64>> {
    let Some(pct) = pct else {
        let mut columns = grid.to_vec();
        scale_to(&mut columns, target)?;
        return Some(columns);
    };
    let mut columns = vec![0i64; grid.len()];
    let mut pct_index = Vec::new();
    for (index, value) in pct.iter().enumerate() {
        if let Some(value) = value {
            let width = target.checked_mul(*value)?.checked_div(5000)?;
            if width < 0 {
                return None;
            }
            columns[index] = width;
            pct_index.push(index);
        }
    }
    if pct_index.is_empty() {
        let mut columns = grid.to_vec();
        scale_to(&mut columns, target)?;
        return Some(columns);
    }
    let sum_pct = sum_twips(
        &pct_index
            .iter()
            .map(|index| columns[*index])
            .collect::<Vec<_>>(),
    )?;
    if sum_pct >= target {
        let mut values: Vec<i64> = pct_index.iter().map(|index| columns[*index]).collect();
        scale_to(&mut values, target)?;
        for (index, column) in pct_index.iter().enumerate() {
            columns[*column] = values[index];
        }
        return Some(columns);
    }
    let remainder = target.checked_sub(sum_pct)?;
    let non: Vec<usize> = (0..grid.len())
        .filter(|index| pct[*index].is_none())
        .collect();
    if non.is_empty() {
        let last = *pct_index.last()?;
        columns[last] = columns[last].checked_add(remainder)?;
        return Some(columns);
    }
    let mut values: Vec<i64> = non.iter().map(|index| grid[*index]).collect();
    if sum_twips(&values)? == 0 {
        columns[*non.last()?] = remainder;
    } else {
        scale_to(&mut values, remainder)?;
        for (index, column) in non.iter().enumerate() {
            columns[*column] = values[index];
        }
    }
    Some(columns)
}

fn sum_twips(values: &[i64]) -> Option<i64> {
    values
        .iter()
        .try_fold(0i64, |total, value| total.checked_add(*value))
}

/// Scales `values` so they sum to `target`. The last entry absorbs rounding.
fn scale_to(values: &mut [i64], target: i64) -> Option<()> {
    if values.is_empty() || target <= 0 {
        return None;
    }
    let sum = sum_twips(values)?;
    if sum <= 0 {
        return None;
    }
    if sum == target {
        return Some(());
    }
    let last = values.len() - 1;
    let mut assigned = 0i64;
    for (index, value) in values.iter_mut().enumerate() {
        if index == last {
            let rest = target.checked_sub(assigned)?;
            if rest < 0 {
                return None;
            }
            *value = rest;
        } else {
            let next = value.checked_mul(target)?.checked_div(sum)?;
            assigned = assigned.checked_add(next)?;
            *value = next;
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{BlockSeq, Cell, CellBorders, Row, Table};
    use crate::id::IdAllocator;

    fn bare(grid: Vec<i64>, width: Option<TableWidth>, layout: Option<TableLayoutKind>) -> Table {
        let mut ids = IdAllocator::default();
        Table {
            id: ids.node(),
            grid: grid.into_iter().map(Measure::Twips).collect(),
            rows: vec![Row {
                id: ids.node(),
                grid_before: 0,
                grid_after: 0,
                cells: vec![],
                header: false,
                cant_split: false,
                height: None,
                preserved: None,
            }],
            style: None,
            width,
            layout,
            alignment: None,
            indent: None,
            cell_margin: None,
            preserved: None,
            source: None,
        }
    }

    #[test]
    fn omitted_layout_shrinks_only_when_wider_than_the_content() {
        let narrow = bare(vec![2400, 2400], None, None);
        let metrics = table_metrics(&narrow, 8306).unwrap();
        assert_eq!(metrics.columns, vec![2400, 2400]);
        assert_eq!(metrics.origin, 0);

        let wide = bare(vec![5000, 5000], None, None);
        let metrics = table_metrics(&wide, 8306).unwrap();
        assert_eq!(metrics.columns.iter().sum::<i64>(), 8306);

        let fixed = bare(vec![5000, 5000], None, Some(TableLayoutKind::Fixed));
        let metrics = table_metrics(&fixed, 8306).unwrap();
        assert_eq!(metrics.columns, vec![5000, 5000]);
    }

    #[test]
    fn percentage_table_width_uses_the_text_extent_and_fixed_does_not_shrink() {
        let table = bare(
            vec![1000, 1000],
            Some(TableWidth::Pct(2500)),
            Some(TableLayoutKind::Fixed),
        );
        let metrics = table_metrics(&table, 8000).unwrap();
        assert_eq!(metrics.columns.iter().sum::<i64>(), 4000);
    }

    #[test]
    fn a_span_one_percentage_sets_that_column_against_an_automatic_table() {
        let mut table = bare(vec![1000, 3000], Some(TableWidth::Auto), None);
        let mut ids = IdAllocator::default();
        let _ = ids.node();
        table.rows[0].cells.push(Cell {
            id: ids.node(),
            grid_span: 1,
            vertical_merge: VerticalMerge::None,
            width: Some(Measure::PercentFiftieths(2500)),
            borders: CellBorders::default(),
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: BlockSeq::from_vec(vec![]),
            preserved: None,
        });
        table.rows[0].cells.push(Cell {
            id: ids.node(),
            grid_span: 1,
            vertical_merge: VerticalMerge::None,
            width: None,
            borders: CellBorders::default(),
            shading: None,
            margin: None,
            vertical_align: None,
            blocks: BlockSeq::from_vec(vec![]),
            preserved: None,
        });
        let metrics = table_metrics(&table, 8000).unwrap();
        assert_eq!(metrics.columns[0], 4000);
        assert_eq!(metrics.columns.iter().sum::<i64>(), 8000);
    }

    #[test]
    fn centre_ignores_indent_and_a_negative_indent_is_refused() {
        let mut table = bare(vec![2000], None, Some(TableLayoutKind::Fixed));
        table.alignment = Some(TableAlign::Center);
        table.indent = Some(500);
        let metrics = table_metrics(&table, 8000).unwrap();
        assert_eq!(metrics.origin, 3000);

        table.alignment = Some(TableAlign::Left);
        table.indent = Some(-1);
        assert!(table_metrics(&table, 8000).is_none());
    }
}
