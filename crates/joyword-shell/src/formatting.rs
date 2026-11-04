//! Derived ribbon state and validated native-dialog parameters over the existing engine.
use crate::reader::Reader;
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
        if let Some(fields) = paragraph.as_object_mut() {
            for (key, value) in fields.iter_mut() {
                if rows.iter().any(|r| &r[key] != value) {
                    *value = Value::Null;
                }
            }
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
        } else if self.engine().registry.get(command).is_none() {
            Some("Command unavailable")
        } else {
            self.engine().why_disabled(command)
        }
    }
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
}
