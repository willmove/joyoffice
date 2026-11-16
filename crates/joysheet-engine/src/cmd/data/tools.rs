use super::*;

// ---------------------------------------------------------------- tools

pub(super) fn remove_duplicates(s: &mut Session, p: &Json) -> Result<Json> {
    let (sheet, r) = data_target(s, p, "data.removeDuplicates")?;
    let d = s.doc()?;
    let sh = d.wb.sheet(sheet).ok_or(EngineError::NoDocument)?;
    // Rows and columns past the data are blank: only the part with data is compared (whole
    // columns compared a million blank rows and reported them as removed duplicates).
    let r = sh.used_range().and_then(|u| r.intersection(&u)).unwrap_or(RangeRef::cell(r.start));
    let header = bool_param(p, "header").unwrap_or_else(|| guess_header(&d.wb, sh, r));
    let cols: Vec<u32> = p
        .get("columns")
        .and_then(Json::as_array)
        .map(|a| a.iter().filter_map(|c| col_param(Some(c), r.start.col)).collect())
        .unwrap_or_else(|| (r.start.col..=r.end.col).collect());
    // With no column to compare, every row key is empty: the first row would survive and every
    // other row would be deleted as a duplicate. A column outside the range compares data the
    // user didn't select. Excel refuses both.
    if cols.is_empty() || cols.iter().any(|c| !(r.start.col..=r.end.col).contains(c)) {
        return Err(bad("data.removeDuplicates", "`columns` must name at least one column of the range"));
    }
    let start = r.start.row + header as u32;
    let mut removed = 0;
    edit(s, |cx| {
        let Some(sh) = cx.wb.sheet(sheet) else { return Ok(()) };
        let mut seen = std::collections::HashSet::new();
        let mut keep: Vec<u32> = Vec::new();
        // Cells are compared by the text they show, as in Excel: the same date in two formats
        // isn't a duplicate, while 1.01 and 1.02 shown as 1.0 are.
        let shown = |c: CellRef| crate::display::format(&sh.value(c), cx.wb.styles.get(sh.style_id(c)).num_fmt.as_str(), &cx.wb).text;
        for row in start..=r.end.row {
            let key: Vec<String> = cols.iter().map(|c| shown(CellRef::new(row, *c)).to_lowercase()).collect();
            if seen.insert(key) {
                keep.push(row);
            } else {
                removed += 1;
            }
        }
        let pictures: Vec<_> = keep
            .iter()
            .enumerate()
            .flat_map(|(i, row)| {
                (r.start.col..=r.end.col)
                    .filter_map(move |col| sh.cell_pictures.get(&CellRef::new(*row, col)).map(|p| (CellRef::new(start + i as u32, col), p.clone())))
            })
            .collect();
        // Rows a filter keeps are compacted to `start + i`, so the notes and hyperlinks that
        // describe them move with them; the ones on removed rows are dropped with the row.
        let notes: Vec<_> = keep
            .iter()
            .enumerate()
            .flat_map(|(i, row)| {
                (r.start.col..=r.end.col)
                    .filter_map(move |col| sh.comments.get(&CellRef::new(*row, col)).map(|cm| (CellRef::new(start + i as u32, col), cm.clone())))
            })
            .collect();
        let links: Vec<_> = keep
            .iter()
            .enumerate()
            .flat_map(|(i, row)| {
                (r.start.col..=r.end.col)
                    .filter_map(move |col| sh.hyperlinks.get(&CellRef::new(*row, col)).map(|h| (CellRef::new(start + i as u32, col), h.clone())))
            })
            .collect();
        let rows: Vec<Vec<Option<Cell>>> =
            keep.iter().map(|row| (r.start.col..=r.end.col).map(|c| sh.cell(CellRef::new(*row, c)).cloned()).collect()).collect();
        let shm = cx.sheet_mut(sheet)?;
        for row in start..=r.end.row {
            for c in r.start.col..=r.end.col {
                shm.remove_cell(CellRef::new(row, c));
            }
        }
        for row in start..=r.end.row {
            for c in r.start.col..=r.end.col {
                let at = CellRef::new(row, c);
                shm.comments.remove(&at);
                shm.hyperlinks.remove(&at);
            }
        }
        for (i, cells) in rows.into_iter().enumerate() {
            for (j, cell) in cells.into_iter().enumerate() {
                if let Some(cell) = cell {
                    shm.set_cell(CellRef::new(start + i as u32, r.start.col + j as u32), cell);
                }
            }
        }
        shm.cell_pictures.extend(pictures);
        for (at, cm) in notes {
            shm.comments.insert(at, cm);
        }
        for (at, h) in links {
            shm.hyperlinks.insert(at, h);
        }
        for c in RangeRef::new(CellRef::new(start, r.start.col), r.end).iter() {
            cx.changed.push((sheet, c));
        }
        Ok(())
    })?;
    let remaining = (r.end.row + 1 - start) as usize - removed;
    Ok(json!({"removed": removed, "remaining": remaining}))
}

pub(super) fn text_to_columns(s: &mut Session, p: &Json) -> Result<Json> {
    let r = target_range(s, p)?;
    let sheet = target_sheet(s, p)?;
    if r.count() > 2_000_000 {
        return Err(bad("data.textToColumns", "selection too large"));
    }
    let formats = p.get("columnFormats").and_then(Json::as_object);
    if formats.is_some_and(|f| {
        f.len() > 16_384
            || f.iter().any(|(k, v)| {
                k.parse::<usize>().is_err()
                    || !matches!(
                        v.as_str(),
                        Some("general" | "text" | "skip" | "dateMDY" | "dateDMY" | "dateYMD" | "dateMYD" | "dateDYM" | "dateYDM")
                    )
            })
    }) {
        return Err(bad("data.textToColumns", "invalid column formats"));
    }
    let dest = cell_param_on(s, p, "destination", sheet)?.unwrap_or(r.start);
    edit(s, |cx| {
        let Some(sh) = cx.wb.sheet(sheet) else { return Ok(()) };
        let mut out: Vec<(CellRef, String, String)> = Vec::new();
        for (i, row) in (r.start.row..=r.end.row).enumerate() {
            let text = sh.value(CellRef::new(row, r.start.col)).display();
            let parts = crate::text_columns::split(&text, p);
            let mut column = 0u32;
            for (j, part) in parts.into_iter().enumerate() {
                let format = formats.and_then(|f| f.get(&j.to_string())).and_then(Json::as_str).unwrap_or("general");
                if format == "skip" {
                    continue;
                }
                let row = dest
                    .row
                    .checked_add(i as u32)
                    .filter(|r| *r < joysheet_core::MAX_ROWS)
                    .ok_or_else(|| bad("data.textToColumns", "destination exceeds worksheet"))?;
                let col = dest
                    .col
                    .checked_add(column)
                    .filter(|c| *c < joysheet_core::MAX_COLS)
                    .ok_or_else(|| bad("data.textToColumns", "destination exceeds worksheet"))?;
                column += 1;
                out.push((CellRef::new(row, col), part, format.to_string()));
                if out.len() > 2_000_000 {
                    return Err(bad("data.textToColumns", "output too large"));
                }
            }
        }
        for (c, t, format) in out {
            let date = crate::text_columns::date_value(&t, &format, cx.wb.date_system);
            let mut cell = if format == "text" {
                Some(Cell::value(Value::Text(t.into())))
            } else if format.starts_with("date") {
                Some(Cell::value(date.map(Value::number).unwrap_or_else(|| Value::Text(t.into()))))
            } else {
                crate::cmd::edit::input_to_cell(&t, sheet, c, &mut cx.wb)?
            };
            if let Some(cell) = &mut cell {
                let style = cx.wb.sheet(sheet).map(|s| s.style_id(c)).unwrap_or_default();
                if format == "text" || date.is_some() {
                    cell.style = cx.wb.styles.derive(style, |s| s.num_fmt = NumFmt::new(if format == "text" { "@" } else { "yyyy-mm-dd" }));
                }
            }
            let shm = cx.sheet_mut(sheet)?;
            match cell {
                Some(x) => shm.set_cell(c, x),
                None => {
                    shm.remove_cell(c);
                }
            }
            cx.changed.push((sheet, c));
        }
        Ok(())
    })?;
    ok()
}

#[cfg(test)]
pub(super) use crate::text_columns::split_delimited;
