//! Derived ribbon state and validated native-dialog parameters over the existing engine.
use crate::reader::Reader;
pub use joyword_layout::ViewMode;
use serde_json::{Value, json};

#[derive(Clone, Debug, Default)]
pub struct FormatState {
    pub character: Value,
    pub paragraph: Value,
    pub bullets: bool,
    pub numbered: bool,
}
impl FormatState {
    pub fn mixed(&self, key: &str) -> bool {
        self.character["mixed"]
            .as_array()
            .is_some_and(|v| v.iter().any(|x| x.as_str() == Some(key)))
    }
    pub fn active(&self, key: &str) -> bool {
        self.character[key].as_bool() == Some(true)
    }
}
impl Reader {
    pub fn formatting(&self) -> FormatState {
        let s = self.engine();
        let rows: Vec<Value> = joyword_engine::cmd::selected_paths(s)
            .iter()
            .filter_map(|path| s.doc.para(s.sel.focus.story, path))
            .filter_map(|p| {
                let mut resolved = s.doc.styles.resolve_para(&p.props);
                resolved.align = resolved.align.visual(resolved.bidi);
                serde_json::to_value(resolved).ok()
            })
            .collect();
        let mut paragraph = rows.first().cloned().unwrap_or(json!({}));
        let mut mixed = Vec::new();
        if let Some(fields) = paragraph.as_object_mut() {
            for (key, value) in fields.iter_mut() {
                if rows.iter().any(|r| &r[key] != value) {
                    *value = Value::Null;
                    mixed.push(key.clone());
                }
            }
        }
        if let Some(fields) = paragraph.as_object_mut() {
            fields.insert("mixed".into(), json!(mixed));
        }
        let numbering = serde_json::from_value(paragraph["numbering"].clone()).unwrap_or(None);
        let list = joyword_engine::cmd::para::list_buttons(&s.doc, numbering);
        FormatState {
            character: joyword_engine::cmd::format::state(s),
            paragraph,
            bullets: list.bullets,
            numbered: list.numbering,
        }
    }
    pub fn formatting_disabled(&self, command: &str) -> Option<&'static str> {
        if self.is_composing() {
            Some("Finish or cancel the composition first")
        } else if self.engine().view.read_mode && self.command_edits_document(command) {
            Some("Switch to Print Layout to edit")
        } else if self.engine().registry.get(command).is_none() {
            Some("Command unavailable")
        } else {
            self.engine().why_disabled(command)
        }
    }

    pub fn page_setup_values(&self) -> ([String; 6], bool, u32) {
        let s = self.engine();
        let page = joyword_engine::cmd::page::sect(s);
        let lengths = [
            page.page_w,
            page.page_h,
            page.margin_top,
            page.margin_bottom,
            page.margin_left,
            page.margin_right,
        ]
        .map(|length| s.format_length(length));
        (lengths, page.landscape, page.columns.count)
    }

    pub fn section_columns(&self) -> joyword_engine::doc::section::Columns {
        joyword_engine::cmd::page::sect(self.engine()).columns
    }

    pub fn header_footer_flags(&self) -> (bool, bool) {
        let session = self.engine();
        (
            joyword_engine::cmd::page::sect(session).title_page,
            session.doc.settings.even_odd_headers,
        )
    }

    pub fn outline_entries(&self) -> Vec<(String, Value)> {
        let doc = &self.engine().doc;
        joyword_layout::outline::levels(doc)
            .into_iter()
            .zip(&doc.body)
            .enumerate()
            .filter_map(|(block, (level, body))| {
                if !(1..=9).contains(&level) {
                    return None;
                }
                let text: String = body.as_para()?.plain_text().chars().take(120).collect();
                let at = joyword_engine::doc::Pos::body(block, 0);
                Some((format!("{level} · {text}"), json!({"anchor":at,"focus":at})))
            })
            .take(200)
            .collect()
    }
}

pub fn page_setup_parameters(
    values: &[String],
    units: joyword_geom::Unit,
) -> Result<Value, String> {
    let mut geometry = serde_json::Map::new();
    for (key, value) in ["width", "height", "top", "bottom", "left", "right"]
        .into_iter()
        .zip(values)
    {
        if !value.trim().is_empty() {
            let points = joyword_geom::parse_measure(value, units)
                .ok_or("请输入有效尺寸，例如 25 mm、2.5 cm 或 72 pt")?;
            geometry.insert(key.into(), json!(points));
        }
    }
    if let Some(value) = values.get(6).map(|v| v.trim()).filter(|v| !v.is_empty()) {
        let landscape = match value.to_ascii_lowercase().as_str() {
            "portrait" | "纵向" => false,
            "landscape" | "横向" => true,
            _ => return Err("请输入纵向或横向（portrait / landscape）".into()),
        };
        geometry.insert("landscape".into(), json!(landscape));
    }
    if let Some(value) = values.get(7).map(|v| v.trim()).filter(|v| !v.is_empty()) {
        let columns = value
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=12).contains(n))
            .ok_or("请输入 1–12 栏")?;
        geometry.insert("columns".into(), json!(columns));
    }
    Ok(json!({ "geometry": geometry }))
}

pub fn column_parameters(values: &[String], units: joyword_geom::Unit) -> Result<Value, String> {
    let [count, space, separator, widths, apply] = values else {
        return Err("Invalid columns form".into());
    };
    let count = count
        .trim()
        .parse::<u32>()
        .ok()
        .filter(|n| (1..=12).contains(n))
        .ok_or("请输入 1–12 栏 / Enter 1–12 columns")?;
    let gap = |value: &str| {
        joyword_geom::parse_measure(value, units)
            .filter(|n| (0.0..=300.0).contains(n))
            .ok_or("请输入 0–300 pt 的栏间距 / Enter a gap between 0 and 300 pt")
    };
    let separator = match separator.trim().to_ascii_lowercase().as_str() {
        "yes" | "是" => true,
        "no" | "否" => false,
        _ => return Err("请输入是或否 / Enter yes or no".into()),
    };
    let apply = match apply.trim().to_ascii_lowercase().as_str() {
        "section" | "本节" => "section",
        "document" | "整篇文档" => "document",
        "forward" | "插入点之后" => "forward",
        "selection" | "所选文字" => "selection",
        _ => return Err("请选择有效的应用范围 / Enter a valid scope".into()),
    };
    let mut args =
        json!({"count":count, "space":gap(space)?, "separator":separator, "apply":apply});
    if !widths.trim().is_empty() {
        let mut columns = Vec::new();
        for item in widths.split('|').take(13) {
            let (width, after) = item.split_once('/').unwrap_or((item, ""));
            let width = joyword_geom::parse_measure(width, units)
                .filter(|n| *n >= 18.0)
                .ok_or("栏宽至少为 18 pt / Column width must be at least 18 pt")?;
            let after = if after.trim().is_empty() {
                0.0
            } else {
                gap(after)?
            };
            columns.push((width, after));
        }
        if columns.len() != count as usize {
            return Err("栏宽条目数必须等于栏数 / Enter one width per column".into());
        }
        args["widths"] = json!(columns);
    }
    Ok(args)
}

pub fn font_families() -> Vec<String> {
    joyword_fonts::FontDb::global().families_in("zh-hans")
}

/// Blank mixed fields are omitted. Reject non-finite/out-of-range values before any edit.
pub fn optional_number(value: &str, min: f64, max: f64) -> Result<Option<f64>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let n: f64 = value
        .trim()
        .parse()
        .map_err(|_| "请输入有效数值；留空保持原值")?;
    if !n.is_finite() || !(min..=max).contains(&n) {
        return Err(format!("数值范围为 {min}–{max}"));
    }
    Ok(Some(n))
}
pub fn paragraph_parameters(values: &[String]) -> Result<Value, String> {
    if values.len() != 6 {
        return Err("Invalid paragraph form".into());
    }
    let mut props = serde_json::Map::new();
    for (i, key, min) in [
        (0, "indentLeft", -1584.),
        (1, "indentRight", -1584.),
        (2, "indentFirst", -1584.),
        (3, "spaceBefore", 0.),
        (4, "spaceAfter", 0.),
    ] {
        if let Some(n) = optional_number(&values[i], min, 1584.)? {
            props.insert(key.into(), json!(n));
            if i < 3 {
                // Explicit point indents take precedence over inherited character indents.
                props.insert(
                    ["indentLeftChars", "indentRightChars", "indentFirstChars"][i].into(),
                    json!(0.),
                );
            }
            if i == 3 {
                props.insert("spaceBeforeAuto".into(), json!(false));
            }
            if i == 4 {
                props.insert("spaceAfterAuto".into(), json!(false));
            }
        }
    }
    let line = values[5].trim();
    if !line.is_empty() {
        let (rule, value, max) = if let Some(v) = line.strip_prefix("exact:") {
            ("exactly", v, 1584.)
        } else if let Some(v) = line.strip_prefix("min:") {
            ("atLeast", v, 1584.)
        } else {
            ("multiple", line, 10.)
        };
        let n = optional_number(value, 0.5, max)?.ok_or("请输入行距")?;
        props.insert("lineSpacing".into(), json!({"rule":rule,"value":n}));
    }
    Ok(json!({"props":props}))
}
pub fn font_parameters(values: &[String]) -> Result<Value, String> {
    if values.len() != 4 {
        return Err("Invalid font form".into());
    }
    let mut props = serde_json::Map::new();
    for (i, key) in [(0, "font"), (1, "fontEa")] {
        let name = values[i].trim();
        if name.len() > 128 || name.chars().any(char::is_control) {
            return Err("字体名称无效".into());
        }
        if !name.is_empty() {
            props.insert(key.into(), json!(name));
            if i == 0 {
                props.insert("fontCs".into(), json!(name));
            }
        }
    }
    if let Some(n) = optional_number(&values[2], 1., 400.)? {
        props.insert("size".into(), json!(n));
    }
    let color = values[3].trim().trim_start_matches('#');
    if !color.is_empty() {
        if color != "auto" && !(color.len() == 6 && color.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err("颜色应为六位 RGB 或 auto".into());
        }
        props.insert("color".into(), json!(color));
    }
    Ok(json!({"props":props}))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "Test setup and assertions")]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    #[test]
    fn paragraph_forms_reject_bad_input_and_preserve_blank_mixed_values() {
        let fields = ["18", "", "-9", "6", "12", "exact:22"].map(String::from);
        let p = paragraph_parameters(&fields).unwrap();
        assert_eq!(
            p["props"]["lineSpacing"],
            json!({"rule":"exactly","value":22.})
        );
        assert!(p["props"].get("indentRight").is_none());
        for invalid in ["NaN", "inf", "-1", "2000", "bad"] {
            let mut f = fields.clone();
            f[3] = invalid.into();
            assert!(paragraph_parameters(&f).is_err());
        }
    }
    #[test]
    fn font_form_validates_before_edit_and_pending_typing_keeps_format() {
        for fields in [
            ["", "", "NaN", ""],
            ["", "", "0", ""],
            ["", "", "12", "orange"],
            ["bad\nfont", "", "12", "auto"],
        ] {
            assert!(font_parameters(&fields.map(String::from)).is_err());
        }
        let mut r = Reader::blank(&AtomicBool::new(false)).unwrap();
        r.edit_command(
            "format.set",
            &font_parameters(&["Source Serif 4", "SimSun", "20", "2459A6"].map(String::from))
                .unwrap(),
        )
        .unwrap();
        r.edit_command("format.bold", &json!({"value":true}))
            .unwrap();
        r.replace_text(None, "中文 mixed 😀").unwrap();
        let f = r.formatting();
        assert!(f.active("bold"));
        assert_eq!(f.character["size"].as_f64(), Some(20.));
        assert_eq!(f.character["color"], "2459A6");
        r.edit_command("edit.undo", &json!({})).unwrap();
        assert!(r.body_text().is_empty());
    }
    #[test]
    fn paragraph_mixed_patch_is_one_undo_and_blank_values_keep_each_indent() {
        let mut r = Reader::blank(&AtomicBool::new(false)).unwrap();
        r.edit_command("document.setText", &json!({"text":"first\n第二段"}))
            .unwrap();
        r.edit_command("para.indents", &json!({"left":24.}))
            .unwrap();
        r.edit_command("select.all", &json!({})).unwrap();
        assert!(r.formatting().paragraph["indentLeft"].is_null());
        let before = r.engine().undo_depth();
        r.edit_command(
            "para.set",
            &paragraph_parameters(&["", "", "", "", "18", ""].map(String::from)).unwrap(),
        )
        .unwrap();
        assert_eq!(r.engine().undo_depth(), before + 1);
        assert!(r.formatting().paragraph["indentLeft"].is_null());
        assert_eq!(r.formatting().paragraph["spaceAfter"].as_f64(), Some(18.));
        r.edit_command("edit.undo", &json!({})).unwrap();
        assert_ne!(r.formatting().paragraph["spaceAfter"].as_f64(), Some(18.));
    }
    #[test]
    fn mixed_ribbon_state_formatting_undo_and_docx_roundtrip() {
        let mut r = Reader::blank(&AtomicBool::new(false)).unwrap();
        r.edit_command(
            "document.setText",
            &json!({"text":"中文报告\nAlpha report"}),
        )
        .unwrap();
        r.edit_command("select.all", &json!({})).unwrap();
        r.edit_command("format.bold", &json!({})).unwrap();
        r.edit_command("format.size", &json!({"size":18})).unwrap();
        r.edit_command(
            "format.font",
            &json!({"name":"Source Serif 4","eastAsia":"SimSun"}),
        )
        .unwrap();
        r.edit_command("para.align", &json!({"value":"center"}))
            .unwrap();
        r.edit_command(
            "para.set",
            &paragraph_parameters(&["18", "0", "9", "6", "12", "1.5"].map(String::from)).unwrap(),
        )
        .unwrap();
        let all = r.formatting();
        assert!(all.active("bold"));
        assert_eq!(all.character["size"].as_f64(), Some(18.));
        assert_eq!(all.paragraph["align"], "center");
        r.edit_command("caret.docStart", &json!({})).unwrap();
        r.edit_command("select.word", &json!({})).unwrap();
        r.edit_command("format.bold", &json!({"value":false}))
            .unwrap();
        r.edit_command("select.all", &json!({})).unwrap();
        assert!(r.formatting().mixed("bold"));
        r.edit_command("edit.undo", &json!({})).unwrap();
        assert!(r.formatting().active("bold"));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("formatted.docx");
        let receipt = r.save_job(path.clone(), true).unwrap().run().unwrap();
        r.apply_save(receipt);
        let mut back = Reader::open(&path, &AtomicBool::new(false)).unwrap();
        back.edit_command("select.all", &json!({})).unwrap();
        let f = back.formatting();
        assert!(f.active("bold"));
        assert_eq!(f.character["fontEastAsia"], "SimSun");
        assert_eq!(f.paragraph["spaceAfter"], 12.);
        assert_eq!(
            f.paragraph["lineSpacing"],
            json!({"rule":"multiple","value":1.5})
        );
    }
    #[test]
    fn page_setup_applies_measurements_atomically_and_survives_reopen() -> Result<(), String> {
        let cancel = AtomicBool::new(false);
        let mut r = Reader::blank(&cancel)?;
        let before = r.engine().doc.last_section.clone();
        let args = page_setup_parameters(
            &["", "", "25", "1 in", "", "", "横向", "2"].map(String::from),
            joyword_geom::Unit::Millimeters,
        )?;
        r.execute("layout.pageSetup", &args)?;
        let page = &r.engine().doc.last_section;
        assert!(page.landscape);
        assert_eq!((page.page_w, page.page_h), (before.page_h, before.page_w));
        assert!((page.margin_top - 70.86614).abs() < 0.001);
        assert_eq!(page.margin_bottom, 72.0);
        assert_eq!(page.columns.count, 2);
        let edited = page.clone();
        let revision = r.engine().rev();
        assert!(
            r.execute("layout.pageSetup", &json!({"geometry":{"left":1500}}))
                .is_err()
        );
        assert_eq!(r.engine().rev(), revision);
        assert_eq!(r.engine().doc.last_section, edited);
        let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
        let path = dir.path().join("page-setup.docx");
        r.save_job(path.clone(), true)?.run()?;
        let reopened = Reader::open(&path, &cancel)?;
        assert_eq!(reopened.engine().doc.last_section.columns.count, 2);
        assert!((reopened.summary.pages[0].width - edited.page_w).abs() < 0.05);
        assert!((reopened.summary.pages[0].height - edited.page_h).abs() < 0.05);
        r.execute("edit.undo", &json!({}))?;
        assert_eq!(r.engine().doc.last_section, before);
        Ok(())
    }

    #[test]
    fn advanced_columns_and_even_section_break_survive_docx_roundtrip() -> Result<(), String> {
        let cancel = AtomicBool::new(false);
        let mut reader = Reader::blank(&cancel)?;
        reader.execute("text.insert", &json!({"text":"First"}))?;
        reader.execute("layout.break", &json!({"kind":"evenPage"}))?;
        reader.execute("text.insert", &json!({"text":"Second"}))?;
        let args = column_parameters(
            &["2", "36 pt", "是", "100 pt/20 pt | 120 pt", "本节"].map(String::from),
            joyword_geom::Unit::Millimeters,
        )?;
        reader.execute("layout.columns", &args)?;
        let columns = reader.section_columns();
        assert_eq!(columns.widths, [(100.0, 20.0), (120.0, 0.0)]);
        assert!(columns.separator);
        assert!(
            column_parameters(
                &["2", "36 pt", "yes", "100 pt", "section"].map(String::from),
                joyword_geom::Unit::Millimeters,
            )
            .is_err()
        );
        let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
        let path = dir.path().join("advanced-columns.docx");
        reader.save_job(path.clone(), true)?.run()?;
        let reopened = Reader::open(&path, &cancel)?;
        let sections = reopened.engine().doc.sections();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].1.columns.count, 1);
        assert_eq!(
            sections[1].1.start,
            joyword_engine::doc::section::SectionStart::EvenPage
        );
        assert_eq!(sections[1].1.columns, columns);
        reader.execute("edit.undo", &json!({}))?;
        assert_eq!(reader.section_columns().count, 1);
        Ok(())
    }
}
