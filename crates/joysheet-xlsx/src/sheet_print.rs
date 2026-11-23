//! Shared worksheet print setup, page margins, print options, header/footer,
//! and manual page-break helpers across SpreadsheetML (`.xlsx`), BIFF8 (`.xls`),
//! and BIFF12 (`.xlsb`).
//!
//! Specifications:
//! - ECMA-376 Part 1 §18.3.1.64 (`pageMargins`), §18.3.1.65 (`pageSetup`),
//!   §18.3.1.70 (`printOptions`), §18.3.1.46 (`headerFooter`), §18.3.1.74 (`rowBreaks`).
//! - MS-XLS `Setup` (`0x00A1`), `PrintGridLines` (`0x002B`), `PrintHeaders` (`0x002A`),
//!   `HCenter` (`0x0083`), `VCenter` (`0x0084`), `LeftMargin`..`BottomMargin` (`0x0026`..`0x0029`),
//!   `Header` (`0x0014`), `Footer` (`0x0015`), `HorizontalPageBreaks` (`0x001B`),
//!   `VerticalPageBreaks` (`0x001A`), `WsBool` (`0x0081` bit `0x0100` `fFitToPage`).
//! - MS-XLSB `BrtPrintOptions` (`477`), `BrtMargins` (`476`), `BrtPageSetup` (`478`),
//!   `BrtBeginHeaderFooter` (`479`), `BrtBeginRwBrk`/`BrtBeginColBrk`/`BrtBrk` (`392`/`394`/`396`),
//!   `BrtWsProp` (`147` bit `0x0100` `fFitToPage`).

use std::fmt::Write as _;

use joysheet_core::{MAX_COLS, MAX_ROWS};
use joysheet_model::{Orientation, Sheet};

use crate::tables::{paper_id, paper_name};
use crate::xml::{El, esc};

/// Upper bound on manual row or column breaks imported per worksheet (Excel allows 1,026).
pub(crate) const MAX_PAGE_BREAKS: usize = 2048;
/// Maximum characters retained per header/footer section (Excel's limit is 255).
pub(crate) const MAX_HF_CHARS: usize = 255;

/// Accumulates order-independent print state (`fFitToPage` vs `Setup`/`BrtPageSetup`)
/// while reading a binary `.xls` or `.xlsb` worksheet.
#[derive(Default)]
pub(crate) struct BinaryPrint {
    pub fit_to_page: bool,
    pub fit_dims: Option<(u16, u16)>,
}

impl BinaryPrint {
    /// Captures `fFitToPage` (`0x0100`) from BIFF8 `WsBool` or BIFF12 `BrtWsProp`.
    pub(crate) fn apply_ws_flags(&mut self, flags: u16) {
        self.fit_to_page = flags & 0x0100 != 0;
    }

    /// Applies BIFF8 `Setup` (`0x00A1`) or BIFF12 `BrtPageSetup` (`478`) fields.
    ///
    /// Both MS-XLS `Setup` and MS-XLSB `BrtPageSetup` share the same bit meanings
    /// in `grbit`: bit 1 (`0x0002`) is `1` for Portrait and `0` for Landscape;
    /// bit 2 (`0x0004`) is `fNoPls` (ignore printer-dependent fields); bit 6 (`0x0040`)
    /// is `fNoOrient` (ignore orientation).
    pub(crate) fn apply_setup(&mut self, sheet: &mut Sheet, paper_size: u32, scale: u32, fit_w: u32, fit_h: u32, grbit: u16) {
        let no_pls = grbit & 0x0004 != 0;
        let no_orient = grbit & 0x0040 != 0;
        if !no_pls {
            if let Some(name) = paper_name(paper_size) {
                sheet.print.paper = name.to_string();
            }
            if (10..=400).contains(&scale) {
                sheet.print.scale = scale as u16;
            }
            if !no_orient {
                sheet.print.orientation = if grbit & 0x0002 == 0 { Orientation::Landscape } else { Orientation::Portrait };
            }
        }
        self.fit_dims = Some((fit_w.min(32767) as u16, fit_h.min(32767) as u16));
    }

    /// Resolves `sheet.print.fit_to` once all worksheet records have been read.
    pub(crate) fn finish(self, sheet: &mut Sheet) {
        if self.fit_to_page {
            sheet.print.fit_to = Some(self.fit_dims.unwrap_or((1, 1)));
        }
    }
}

/// Sets one margin slot (`0=left, 1=right, 2=top, 3=bottom, 4=header, 5=footer`),
/// clamping valid finite values to `[0.0, 50.0]` inches.
pub(crate) fn set_margin(sheet: &mut Sheet, idx: usize, inches: f64) {
    if inches.is_finite()
        && let Some(slot) = sheet.print.margins.get_mut(idx)
    {
        *slot = inches.clamp(0.0, 50.0) as f32;
    }
}

/// Applies all six margins `[left, right, top, bottom, header, footer]`.
pub(crate) fn apply_margins(sheet: &mut Sheet, margins: [f64; 6]) {
    for (idx, val) in margins.into_iter().enumerate() {
        set_margin(sheet, idx, val);
    }
}

/// Applies BIFF12 `BrtPrintOptions` (`477`) flags.
pub(crate) fn apply_biff12_print_options(sheet: &mut Sheet, flags: u16) {
    sheet.print.center_h = flags & 0x0001 != 0;
    sheet.print.center_v = flags & 0x0002 != 0;
    sheet.print.headings = flags & 0x0004 != 0;
    sheet.print.gridlines = flags & 0x0008 != 0;
}

/// Clamps header/footer text to Excel's 255-character limit.
pub(crate) fn clamp_hf_text(s: &str) -> String {
    s.chars().take(MAX_HF_CHARS).collect()
}

/// Appends a manual row break (`1..MAX_ROWS`) within the break budget.
pub(crate) fn push_row_break(sheet: &mut Sheet, row_id: u32) {
    if row_id > 0 && row_id < MAX_ROWS && sheet.print.row_breaks.len() < MAX_PAGE_BREAKS {
        sheet.print.row_breaks.push(row_id);
    }
}

/// Appends a manual column break (`1..MAX_COLS`) within the break budget.
pub(crate) fn push_col_break(sheet: &mut Sheet, col_id: u32) {
    if col_id > 0 && col_id < MAX_COLS && sheet.print.col_breaks.len() < MAX_PAGE_BREAKS {
        sheet.print.col_breaks.push(col_id);
    }
}

pub(crate) fn read_xml_print_options(e: &El, sheet: &mut Sheet) {
    sheet.print.gridlines = e.flag("gridLines", false);
    sheet.print.headings = e.flag("headings", false);
    sheet.print.center_h = e.flag("horizontalCentered", false);
    sheet.print.center_v = e.flag("verticalCentered", false);
}

pub(crate) fn read_xml_page_margins(e: &El, sheet: &mut Sheet) {
    for (i, k) in ["left", "right", "top", "bottom", "header", "footer"].iter().enumerate() {
        if let Some(v) = e.attr_f64(k) {
            set_margin(sheet, i, v);
        }
    }
}

pub(crate) fn read_xml_page_setup(e: &El, sheet: &mut Sheet, fit_to_page: bool) {
    if e.attr("orientation") == Some("landscape") {
        sheet.print.orientation = Orientation::Landscape;
    }
    if let Some(p) = e.attr_u32("paperSize").and_then(paper_name) {
        sheet.print.paper = p.to_string();
    }
    if let Some(s) = e.attr_u32("scale") {
        sheet.print.scale = s.clamp(10, 400) as u16;
    }
    if fit_to_page {
        let w = e.attr_u32("fitToWidth").unwrap_or(1).min(32767) as u16;
        let h = e.attr_u32("fitToHeight").unwrap_or(1).min(32767) as u16;
        sheet.print.fit_to = Some((w, h));
    }
}

pub(crate) fn read_xml_header_footer(e: &El, sheet: &mut Sheet) {
    let pr = &mut sheet.print;
    for (tag, slot) in [
        ("oddHeader", &mut pr.header),
        ("oddFooter", &mut pr.footer),
        ("evenHeader", &mut pr.even_header),
        ("evenFooter", &mut pr.even_footer),
        ("firstHeader", &mut pr.first_header),
        ("firstFooter", &mut pr.first_footer),
    ] {
        *slot = e.child(tag).map(|h| clamp_hf_text(&h.text)).unwrap_or_default();
    }
    pr.different_first = e.flag("differentFirst", false);
    pr.different_odd_even = e.flag("differentOddEven", false);
    pr.hf_scale_with_doc = e.flag("scaleWithDoc", true);
    pr.hf_align_with_margins = e.flag("alignWithMargins", true);
}

pub(crate) fn read_xml_breaks(e: &El, sheet: &mut Sheet, is_row: bool) {
    for b in e.kids("brk") {
        if let Some(id) = b.attr_u32("id") {
            if is_row {
                push_row_break(sheet, id);
            } else {
                push_col_break(sheet, id);
            }
        }
    }
}

pub(crate) fn write_xml_print_options(s: &mut String, sheet: &Sheet) {
    let p = &sheet.print;
    if p.gridlines || p.headings || p.center_h || p.center_v {
        s.push_str("<printOptions");
        for (a, v) in [("horizontalCentered", p.center_h), ("verticalCentered", p.center_v), ("headings", p.headings), ("gridLines", p.gridlines)] {
            if v {
                let _ = write!(s, " {a}=\"1\"");
            }
        }
        s.push_str("/>");
    }
}

pub(crate) fn write_xml_page_setup_and_breaks(s: &mut String, sheet: &Sheet, page_setup_rel: Option<&str>, fmt_num: impl Fn(f64) -> String) {
    let p = &sheet.print;
    let m = |i: usize| fmt_num(f64::from(p.margins.get(i).copied().unwrap_or(0.75)));
    let _ =
        write!(s, "<pageMargins left=\"{}\" right=\"{}\" top=\"{}\" bottom=\"{}\" header=\"{}\" footer=\"{}\"/>", m(0), m(1), m(2), m(3), m(4), m(5));
    s.push_str("<pageSetup");
    if let Some(id) = paper_id(&p.paper)
        && id != 1
    {
        let _ = write!(s, " paperSize=\"{id}\"");
    }
    if p.scale != 100 && (10..=400).contains(&p.scale) {
        let _ = write!(s, " scale=\"{}\"", p.scale);
    }
    if let Some((w, h)) = p.fit_to {
        let _ = write!(s, " fitToWidth=\"{w}\" fitToHeight=\"{h}\"");
    }
    let _ = write!(s, " orientation=\"{}\"", if p.orientation == Orientation::Landscape { "landscape" } else { "portrait" });
    if let Some(id) = page_setup_rel {
        let _ = write!(s, " r:id=\"{id}\"");
    }
    s.push_str("/>");
    if p.has_header_footer() || p.different_first || p.different_odd_even || !p.hf_scale_with_doc || !p.hf_align_with_margins {
        s.push_str("<headerFooter");
        for (attr, on, default) in [
            ("differentOddEven", p.different_odd_even, false),
            ("differentFirst", p.different_first, false),
            ("scaleWithDoc", p.hf_scale_with_doc, true),
            ("alignWithMargins", p.hf_align_with_margins, true),
        ] {
            if on != default {
                let _ = write!(s, " {attr}=\"{}\"", u8::from(on));
            }
        }
        s.push('>');
        for (tag, text) in [
            ("oddHeader", &p.header),
            ("oddFooter", &p.footer),
            ("evenHeader", &p.even_header),
            ("evenFooter", &p.even_footer),
            ("firstHeader", &p.first_header),
            ("firstFooter", &p.first_footer),
        ] {
            if !text.is_empty() {
                let _ = write!(s, "<{tag}>{}</{tag}>", esc(text));
            }
        }
        s.push_str("</headerFooter>");
    }
    for (tag, list, max) in [("rowBreaks", &p.row_breaks, 16383u32), ("colBreaks", &p.col_breaks, 1048575)] {
        let list: Vec<u32> = list.iter().copied().filter(|&b| b > 0).collect();
        if !list.is_empty() {
            let _ = write!(s, "<{tag} count=\"{0}\" manualBreakCount=\"{0}\">", list.len());
            for b in list {
                let _ = write!(s, "<brk id=\"{b}\" max=\"{max}\" man=\"1\"/>");
            }
            let _ = write!(s, "</{tag}>");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_print_handles_fit_to_page_order_and_landscape_bit() {
        let mut sheet = Sheet::new("Print");
        let mut bp = BinaryPrint::default();
        // Setup arrives before WsBool: paperSize=9 (A4), scale=80, fit=2x3, grbit=0 (Landscape, valid printer setup).
        bp.apply_setup(&mut sheet, 9, 80, 2, 3, 0);
        bp.apply_ws_flags(0x0100);
        bp.finish(&mut sheet);
        assert_eq!(sheet.print.paper, "A4");
        assert_eq!(sheet.print.scale, 80);
        assert_eq!(sheet.print.orientation, Orientation::Landscape);
        assert_eq!(sheet.print.fit_to, Some((2, 3)));

        // fNoPls (0x0004) ignores paper/scale/orientation while keeping fit dimensions.
        let mut sheet2 = Sheet::new("NoPls");
        let mut bp2 = BinaryPrint::default();
        bp2.apply_setup(&mut sheet2, 9, 55, 1, 0, 0x0004);
        bp2.apply_ws_flags(0x0100);
        bp2.finish(&mut sheet2);
        assert_eq!(sheet2.print.orientation, Orientation::Portrait);
        assert_eq!(sheet2.print.scale, 100);
        assert_eq!(sheet2.print.fit_to, Some((1, 0)));
    }
}
