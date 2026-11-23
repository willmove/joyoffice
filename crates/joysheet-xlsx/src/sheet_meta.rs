//! First-party worksheet protection, password hash, and bounded hyperlink codec
//! shared across XLSX (ECMA-376 §18.3.1.85 / §18.3.1.48), BIFF8 `.xls`
//! ([MS-XLS] `Protect` `0x0012` / `Password` `0x0013` / `FeatHdr` `0x0867`
//! `EnhancedProtection`), and BIFF12 `.xlsb` ([MS-XLSB] `BrtSheetProtection`
//! `535` / `BrtSheetProtectionIso` `678` / `BrtHLink` `494`).

use std::fmt::Write as _;

use joysheet_core::RangeRef;
use joysheet_model::{Hyperlink, PasswordHash, Sheet, SheetProtection};

use crate::package::Rel;
use crate::read::Ctx;
use crate::write::Rels;
use crate::xml::{El, esc_attr};

/// Most cells that receive hyperlinks on a single worksheet.
pub(crate) const MAX_HYPERLINK_CELLS: u64 = 10_000;
/// Most combined target + tooltip bytes replicated across cells on one worksheet.
pub(crate) const MAX_HYPERLINK_BYTES: u64 = 8 * 1024 * 1024;

// ========================================================================= passwords

/// `base` preceded by `prefix` in camelCase: `("workbook", "hashValue")` -> `"workbookHashValue"`.
pub(crate) fn password_attr(prefix: &str, base: &str) -> String {
    let mut chars = base.chars();
    match chars.next() {
        Some(first) if !prefix.is_empty() => format!("{prefix}{}{}", first.to_ascii_uppercase(), chars.as_str()),
        _ => base.to_string(),
    }
}

/// Fail-closed placeholder hash that never matches any password, ensuring
/// unreadable or unsupported hashes keep the protected item locked.
pub(crate) fn unmatchable_password() -> PasswordHash {
    PasswordHash::Iterated { algorithm: String::new(), hash: String::new(), salt: String::new(), spin_count: 0 }
}

/// Reads an OOXML protection password hash (iterated SHA or legacy 16-bit XOR).
pub(crate) fn read_password(e: &El, prefix: &str) -> Option<PasswordHash> {
    let attr = |base: &str| e.attr(&password_attr(prefix, base));
    if attr("algorithmName").is_some() || attr("hashValue").is_some() {
        return Some(PasswordHash::Iterated {
            algorithm: attr("algorithmName").unwrap_or("").to_string(),
            hash: attr("hashValue").unwrap_or("").to_string(),
            salt: attr("saltValue").unwrap_or("").to_string(),
            spin_count: e.attr_u32(&password_attr(prefix, "spinCount")).unwrap_or(0),
        });
    }
    attr("password").map(|hex| match u16::from_str_radix(hex.trim(), 16) {
        Ok(hash) => PasswordHash::Legacy(hash),
        Err(_) => unmatchable_password(),
    })
}

/// Formats an OOXML protection password hash as XML attributes.
pub(crate) fn write_password_attrs(hash: &PasswordHash, prefix: &str) -> String {
    let name = |base: &str| password_attr(prefix, base);
    match hash {
        PasswordHash::Legacy(code) => format!(" {}=\"{code:04X}\"", name("password")),
        PasswordHash::Iterated { algorithm, hash, salt, spin_count } => format!(
            " {}=\"{}\" {}=\"{}\" {}=\"{}\" {}=\"{spin_count}\"",
            name("algorithmName"),
            esc_attr(algorithm),
            name("hashValue"),
            esc_attr(hash),
            name("saltValue"),
            esc_attr(salt),
            name("spinCount")
        ),
    }
}

// ========================================================================= sheet protection

/// Normalized permission booleans (`true` = user action allowed on protected sheet).
#[derive(Clone, Copy, Debug)]
struct Permissions {
    select_locked: bool,
    select_unlocked: bool,
    format_cells: bool,
    format_columns: bool,
    format_rows: bool,
    insert_columns: bool,
    insert_rows: bool,
    delete_columns: bool,
    delete_rows: bool,
    sort: bool,
    autofilter: bool,
}

impl Permissions {
    /// Excel's legacy default when a worksheet is protected without granular flags.
    const LEGACY_DEFAULT: Self = Self {
        select_locked: true,
        select_unlocked: true,
        format_cells: false,
        format_columns: false,
        format_rows: false,
        insert_columns: false,
        insert_rows: false,
        delete_columns: false,
        delete_rows: false,
        sort: false,
        autofilter: false,
    };

    /// Decodes MS-XLS `EnhancedProtection` (2.5.104), where `1` = allowed.
    fn from_biff8_enhanced(bits: u32) -> Self {
        let on = |mask: u32| bits & mask != 0;
        Self {
            format_cells: on(0x0004),
            format_columns: on(0x0008),
            format_rows: on(0x0010),
            insert_columns: on(0x0020),
            insert_rows: on(0x0040),
            delete_columns: on(0x0100),
            delete_rows: on(0x0200),
            select_locked: on(0x0400),
            sort: on(0x0800),
            autofilter: on(0x1000),
            select_unlocked: on(0x4000),
        }
    }

    /// Decodes MS-XLSB `BrtSheetProtection` (2.4.792) 16 DWORD flags (`1` = allowed).
    fn from_biff12_flags(flags: [u32; 16]) -> Self {
        Self {
            format_cells: flags[3] != 0,
            format_columns: flags[4] != 0,
            format_rows: flags[5] != 0,
            insert_columns: flags[6] != 0,
            insert_rows: flags[7] != 0,
            delete_columns: flags[9] != 0,
            delete_rows: flags[10] != 0,
            select_locked: flags[11] != 0,
            sort: flags[12] != 0,
            autofilter: flags[13] != 0,
            select_unlocked: flags[15] != 0,
        }
    }

    fn into_model(self, password: Option<PasswordHash>) -> SheetProtection {
        SheetProtection {
            password,
            legacy_password_hash: None,
            select_locked: self.select_locked,
            select_unlocked: self.select_unlocked,
            format_cells: self.format_cells,
            format_columns: self.format_columns,
            format_rows: self.format_rows,
            insert_columns: self.insert_columns,
            insert_rows: self.insert_rows,
            delete_columns: self.delete_columns,
            delete_rows: self.delete_rows,
            sort: self.sort,
            autofilter: self.autofilter,
        }
    }
}

/// Accumulates worksheet protection records while scanning BIFF8 or BIFF12 streams.
#[derive(Default)]
pub(crate) struct BinaryProtection {
    protected: bool,
    password: Option<PasswordHash>,
    permissions: Option<Permissions>,
    iso_hash_seen: bool,
}

impl BinaryProtection {
    /// BIFF8 `PROTECT` (`0x0012`): enables or disables worksheet protection.
    pub(crate) fn set_protected(&mut self, protected: bool) {
        self.protected = protected;
    }

    /// BIFF8 `PASSWORD` (`0x0013`): records a non-zero 16-bit legacy password hash.
    pub(crate) fn set_legacy_password(&mut self, hash: u16) {
        if hash != 0 {
            self.password = Some(PasswordHash::Legacy(hash));
        }
    }

    /// BIFF8 `FeatHdr` (`0x0867`, `isf == ISFPROTECTION`): granular allowed flags.
    pub(crate) fn apply_biff8_enhanced(&mut self, bits: u32) {
        self.permissions = Some(Permissions::from_biff8_enhanced(bits));
    }

    /// BIFF12 `BrtSheetProtection` (`535`): password + 16 DWORD flags.
    pub(crate) fn apply_biff12_protection(&mut self, password: u16, flags: [u32; 16]) {
        self.protected = flags[0] != 0;
        if self.protected {
            if password != 0 {
                self.password = Some(PasswordHash::Legacy(password));
            }
            self.permissions = Some(Permissions::from_biff12_flags(flags));
        } else {
            self.password = None;
            self.permissions = None;
        }
    }

    /// BIFF12 `BrtSheetProtectionIso` (`678`): marks an ISO iterated password hash
    /// so the sheet fails closed instead of silently dropping password protection.
    pub(crate) fn mark_iso_password(&mut self) {
        self.iso_hash_seen = true;
    }

    pub(crate) fn finish(self, sheet: &mut Sheet) {
        if !self.protected {
            return;
        }
        let password = self.password.or_else(|| self.iso_hash_seen.then(unmatchable_password));
        let perms = self.permissions.unwrap_or(Permissions::LEGACY_DEFAULT);
        sheet.protection = Some(perms.into_model(password));
    }
}

/// Reads OOXML `<sheetProtection>` (attributes are inverted restriction flags: `0` = allowed).
pub(crate) fn read_xml_protection(e: &El) -> Option<SheetProtection> {
    if !e.flag("sheet", false) {
        return None;
    }
    let perms = Permissions {
        select_locked: !e.flag("selectLockedCells", false),
        select_unlocked: !e.flag("selectUnlockedCells", false),
        format_cells: !e.flag("formatCells", true),
        format_columns: !e.flag("formatColumns", true),
        format_rows: !e.flag("formatRows", true),
        insert_columns: !e.flag("insertColumns", true),
        insert_rows: !e.flag("insertRows", true),
        delete_columns: !e.flag("deleteColumns", true),
        delete_rows: !e.flag("deleteRows", true),
        sort: !e.flag("sort", true),
        autofilter: !e.flag("autoFilter", true),
    };
    Some(perms.into_model(read_password(e, "")))
}

/// Writes OOXML `<sheetProtection .../>`.
pub(crate) fn write_xml_protection(s: &mut String, p: &SheetProtection) {
    s.push_str("<sheetProtection sheet=\"1\" objects=\"1\" scenarios=\"1\"");
    for (attr, allowed) in [
        ("formatCells", p.format_cells),
        ("formatColumns", p.format_columns),
        ("formatRows", p.format_rows),
        ("insertColumns", p.insert_columns),
        ("insertRows", p.insert_rows),
        ("deleteColumns", p.delete_columns),
        ("deleteRows", p.delete_rows),
        ("sort", p.sort),
        ("autoFilter", p.autofilter),
    ] {
        if allowed {
            let _ = write!(s, " {attr}=\"0\"");
        }
    }
    if !p.select_locked {
        s.push_str(" selectLockedCells=\"1\"");
    }
    if !p.select_unlocked {
        s.push_str(" selectUnlockedCells=\"1\"");
    }
    if let Some(hash) = &p.password {
        s.push_str(&write_password_attrs(hash, ""));
    }
    s.push_str("/>");
}

// ========================================================================= hyperlinks

/// Resolves an OOXML/XLSB hyperlink record against the worksheet's relationships.
pub(crate) fn resolve_hyperlink(rel_id: Option<&str>, location: Option<&str>, tooltip: Option<&str>, rels: &[Rel]) -> Option<Hyperlink> {
    let mut target = rel_id.filter(|id| !id.is_empty()).and_then(|id| rels.iter().find(|r| r.id == id)).map(|r| r.target.clone()).unwrap_or_default();
    let external = !target.is_empty();
    if let Some(loc) = location.filter(|l| !l.is_empty()) {
        if target.is_empty() {
            target = loc.trim_start_matches('#').to_string();
        } else {
            target = format!("{target}#{loc}");
        }
    }
    if target.is_empty() {
        return None;
    }
    Some(Hyperlink { target, tooltip: tooltip.map(str::to_string), external })
}

/// Shared cell/byte budget enforcing `MAX_HYPERLINK_CELLS` and `MAX_HYPERLINK_BYTES`
/// identically for XLSX and XLSB hyperlink imports.
#[derive(Default)]
pub(crate) struct HyperlinkBudget {
    cells: u64,
    bytes: u64,
    exhausted: bool,
}

impl HyperlinkBudget {
    pub(crate) fn insert_range(
        &mut self,
        sheet: &mut Sheet,
        sheet_name: &str,
        range: RangeRef,
        link: Hyperlink,
        warn: &mut dyn FnMut(String),
    ) -> bool {
        if self.exhausted {
            return false;
        }
        let link_bytes = (link.target.len() + link.tooltip.as_ref().map_or(0, String::len)) as u64;
        for cell in range.iter() {
            if self.cells >= MAX_HYPERLINK_CELLS {
                self.exhausted = true;
                warn(format!("sheet \"{sheet_name}\": hyperlinks past the first {MAX_HYPERLINK_CELLS} cells were dropped"));
                return false;
            }
            if self.bytes.saturating_add(link_bytes) > MAX_HYPERLINK_BYTES {
                self.exhausted = true;
                warn(format!("sheet \"{sheet_name}\": hyperlink targets past {MAX_HYPERLINK_BYTES} bytes were dropped"));
                return false;
            }
            self.cells += 1;
            self.bytes += link_bytes;
            sheet.hyperlinks.insert(cell, link.clone());
        }
        true
    }
}

/// Reads OOXML `<hyperlinks>` into `sheet.hyperlinks` under the shared budget.
pub(crate) fn read_xml_hyperlinks(cx: &mut Ctx<'_>, name: &str, e: &El, rels: &[Rel], sheet: &mut Sheet) {
    let mut budget = HyperlinkBudget::default();
    for h in e.kids("hyperlink") {
        let Some(range) = h.attr("ref").and_then(RangeRef::parse) else { continue };
        let Some(link) = resolve_hyperlink(h.attr("id"), h.attr("location"), h.attr("tooltip"), rels) else { continue };
        if !budget.insert_range(sheet, name, range, link, &mut |msg| cx.warn(msg)) {
            return;
        }
    }
}

/// Writes OOXML `<hyperlinks>` and populates external hyperlink relationships.
pub(crate) fn write_xml_hyperlinks(s: &mut String, sheet: &Sheet, rels: &mut Rels) {
    if sheet.hyperlinks.is_empty() {
        return;
    }
    s.push_str("<hyperlinks>");
    for (cell, h) in &sheet.hyperlinks {
        let t = h.target.trim();
        if t.is_empty() {
            continue;
        }
        let external = h.external
            || t.contains("://")
            || t.to_ascii_lowercase().starts_with("mailto:")
            || t.starts_with("\\\\")
            || t.to_ascii_lowercase().starts_with("file:");
        let _ = write!(s, "<hyperlink ref=\"{}\"", cell.a1());
        if external {
            let (url, loc) = match t.split_once('#') {
                Some((u, l)) if !u.is_empty() => (u, Some(l)),
                _ => (t, None),
            };
            let id = rels.add_external("hyperlink", url);
            let _ = write!(s, " r:id=\"{id}\"");
            if let Some(l) = loc {
                let _ = write!(s, " location=\"{}\"", esc_attr(l));
            }
        } else {
            let clean = t.trim_start_matches('#');
            let _ = write!(s, " location=\"{}\" display=\"{}\"", esc_attr(clean), esc_attr(clean));
        }
        if let Some(tt) = &h.tooltip {
            let _ = write!(s, " tooltip=\"{}\"", esc_attr(tt));
        }
        s.push_str("/>");
    }
    s.push_str("</hyperlinks>");
}
