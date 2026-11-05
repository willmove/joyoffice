//! R5 views and output jobs over the existing Session; no second editable model.
use crate::reader::{MAX_INPUT_BYTES, Reader, publish_new};
use joyword_engine::{
    Session,
    doc::{Document, StoryRef},
};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub fn bounded_text(s: &str, max: usize) -> Result<String, String> {
    if s.len() > max || s.contains('\0') {
        return Err(format!("文本最多 {max} 字节，且不能含空字符"));
    }
    Ok(s.to_string())
}
pub fn read_local(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err("文件超过本次操作的大小限制".into());
    }
    Ok(bytes)
}
pub const IMPORTS: &[&str] = &[
    "docx", "docm", "dotx", "dotm", "doc", "dot", "txt", "md", "markdown", "html", "htm", "mht",
    "mhtml", "rtf", "odt", "tex", "latex", "xml",
];
pub const EXPORTS: &[&str] = &[
    "docx", "pdf", "txt", "md", "html", "rtf", "odt", "tex", "xml",
];
pub fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
impl Reader {
    /// Snapshot the current document for a copy export, including unsaved edits.
    pub fn export_snapshot(&self) -> Result<Document, String> {
        if self.is_composing() {
            return Err("请先完成或取消输入法组合".into());
        }
        Ok(self.session.doc.clone())
    }

    pub fn advanced_query(&mut self, id: &str) -> Result<Value, String> {
        if !matches!(
            id,
            "review.comments"
                | "review.changes"
                | "references.notes"
                | "insert.crossReference"
                | "link.targets"
                | "tools.macros"
                | "mailings.checkErrors"
        ) {
            return Err("Unsupported read query".into());
        }
        self.session.run(id, &json!({})).map_err(|e| e.to_string())
    }
    pub fn sources(&self) -> Value {
        json!(self.session.doc.sources)
    }
    pub fn merge_info(&self) -> Value {
        let m = &self.session.merge;
        json!({"fields":m.headers,"rows":m.rows,"total":m.source.len(),"records":m.rows.len(),"record":m.record+1,"preview":m.preview,"kind":m.kind})
    }
    pub fn return_to_body(&mut self) -> Result<(), String> {
        let pos = self.session.doc.end_of(StoryRef::Body);
        self.select_positions(pos.clone(), pos)
    }
    pub fn visit_note(&mut self, id: u32) -> Result<(), String> {
        if !self.session.doc.parts.get(&id).is_some_and(|p| {
            matches!(
                p.kind,
                joyword_engine::doc::PartKind::Footnote | joyword_engine::doc::PartKind::Endnote
            )
        }) {
            return Err("注释已不存在".into());
        }
        let pos = self.session.doc.start_of(StoryRef::Part(id));
        self.select_positions(pos.clone(), pos)
    }
    pub fn select_json(&mut self, start: &Value, end: &Value) -> Result<(), String> {
        let a = joyword_engine::cmd::parse_pos(start).ok_or("无效范围")?;
        let b = joyword_engine::cmd::parse_pos(end).unwrap_or_else(|| a.clone());
        self.select_positions(a, b)
    }
    pub fn export_job(&self, ext: &str, markup: bool) -> Result<OutputJob, String> {
        if self.is_composing() {
            return Err("请先完成或取消输入法组合".into());
        }
        if !EXPORTS.contains(&ext) {
            return Err("此格式尚未实现导出".into());
        }
        Ok(OutputJob {
            doc: self.session.doc.clone(),
            ext: ext.into(),
            markup,
            merge: None,
        })
    }
    pub fn merge_job(&self, current: bool) -> Result<OutputJob, String> {
        if self.is_composing() || self.session.merge.rows.is_empty() {
            return Err("请先选择收件人并完成输入".into());
        }
        if !current && self.session.merge.rows.len() > 1000 {
            return Err("开发包单次合并最多 1000 条记录".into());
        }
        Ok(OutputJob {
            doc: self.session.doc.clone(),
            ext: "docx".into(),
            markup: false,
            merge: Some((self.session.merge.clone(), current)),
        })
    }
    pub fn macro_steps(&self, name: &str) -> Result<Vec<(String, Value)>, String> {
        let steps = self.session.macros.get(name).ok_or("找不到宏")?;
        validate_macro(steps)?;
        Ok(steps.clone())
    }
    pub(crate) fn validate_macro_run(&self, args: &Value) -> Result<(), String> {
        if let Some(name) = args["run"].as_str() {
            self.macro_steps(name)?;
        }
        if let Some(define) = args.get("define") {
            let steps: Vec<_> = define["steps"]
                .as_array()
                .ok_or("宏需要步骤")?
                .iter()
                .map(|s| {
                    Ok((
                        s["command"].as_str().ok_or("无效宏命令")?.to_string(),
                        s.get("params").cloned().unwrap_or(json!({})),
                    ))
                })
                .collect::<Result<_, String>>()?;
            validate_macro(&steps)?;
        }
        Ok(())
    }
}

pub struct ArchivalPdf(Vec<u8>);

impl ArchivalPdf {
    pub fn prepare(doc: &Document, year: u16) -> Result<Self, String> {
        let bytes = joyword_pdf::export_archival(doc, year).map_err(|error| error.to_string())?;
        if bytes.len() > 256 << 20 {
            return Err("PDF/A output exceeds 256 MiB".into());
        }
        Ok(Self(bytes))
    }

    pub fn publish(&self, path: &Path, cancel: &AtomicBool) -> Result<(), String> {
        if extension(path) != "pdf" {
            return Err("PDF/A destination must end in .pdf".into());
        }
        crate::reader::publish_verified(path, &self.0, cancel)
    }
}

#[cfg(test)]
mod archival_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn archive_uses_unsaved_text_and_publishes_only_to_new_paths() -> Result<(), String> {
        let cancel = AtomicBool::new(false);
        let mut reader = Reader::blank(&cancel)?;
        reader.execute("text.insert", &json!({"text": "Unsaved archival snapshot"}))?;
        let revision = reader.engine().rev();
        let doc = reader.export_snapshot()?;
        assert!(
            doc.plain_text(StoryRef::Body)
                .contains("Unsaved archival snapshot")
        );
        let archive = ArchivalPdf::prepare(&doc, 2026)?;
        assert_eq!(reader.engine().rev(), revision);
        assert!(archive.0.starts_with(b"%PDF-1.7"));
        let directory = tempfile::tempdir().map_err(|e| e.to_string())?;
        let path = directory.path().join("archive.pdf");
        cancel.store(true, Ordering::Release);
        assert!(archive.publish(&path, &cancel).is_err());
        assert!(!path.exists());
        cancel.store(false, Ordering::Release);
        assert!(
            archive
                .publish(&path.with_extension("txt"), &cancel)
                .is_err()
        );
        archive.publish(&path, &cancel)?;
        assert!(archive.publish(&path, &cancel).is_err());
        assert_eq!(std::fs::read(&path).map_err(|e| e.to_string())?, archive.0);
        Ok(())
    }
}
pub fn validate_macro(steps: &[(String, Value)]) -> Result<(), String> {
    fn external(v: &Value) -> bool {
        match v {
            Value::Object(o) => o.iter().any(|(k, v)| {
                matches!(k.as_str(), "path" | "file" | "import") && v.is_string() || external(v)
            }),
            Value::Array(a) => a.iter().any(external),
            _ => false,
        }
    }
    if steps.len() > 256 || serde_json::to_vec(steps).map_err(|e| e.to_string())?.len() > 1 << 20 {
        return Err("宏最多 256 步 / 1 MiB".into());
    }
    for (id, args) in steps {
        let allowed = [
            "format.",
            "para.",
            "styles.",
            "insert.",
            "table.",
            "picture.",
            "arrange.",
            "references.",
            "text.",
            "caret.",
            "select.",
            "review.",
        ]
        .iter()
        .any(|p| id.starts_with(p));
        if !allowed
            || external(args)
            || matches!(
                id.as_str(),
                "review.compare" | "review.combine" | "references.researcher"
            )
        {
            return Err(format!("桌面宏不允许文件、外部服务或会话控制命令：{id}"));
        }
    }
    Ok(())
}
pub struct OutputJob {
    doc: Document,
    ext: String,
    markup: bool,
    merge: Option<(joyword_engine::cmd::mailings::MergeState, bool)>,
}
impl OutputJob {
    pub fn bytes(self) -> Result<Vec<u8>, String> {
        let mut doc = self.doc;
        if let Some((merge, current)) = self.merge {
            // Bound expansion before cloning a template per record. A counting writer
            // avoids allocating a second serialized copy merely to estimate its size.
            struct Budget {
                left: usize,
            }
            impl std::io::Write for Budget {
                fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                    if bytes.len() > self.left {
                        return Err(std::io::Error::other(
                            "邮件合并超过 32 MiB 展开预算，请减少记录范围或模板内容",
                        ));
                    }
                    self.left -= bytes.len();
                    Ok(bytes.len())
                }
                fn flush(&mut self) -> std::io::Result<()> {
                    Ok(())
                }
            }
            let count = if current { 1 } else { merge.rows.len().max(1) };
            serde_json::to_writer(
                Budget {
                    left: (32 << 20) / count,
                },
                &doc,
            )
            .map_err(|e| e.to_string())?;
            let mut s = Session::new(doc);
            s.merge = merge;
            s.run(
                "mailings.finish",
                &json!({"records":if current{"current"}else{"all"}}),
            )
            .map_err(|e| e.to_string())?;
            doc = s.doc;
        }
        let opts = joyword_engine::io::ExportOptions {
            markup: self.markup,
            comments: self.markup,
            ..Default::default()
        };
        joyword_engine::io::save_bytes_opts(&format!("copy.{}", self.ext), &doc, &opts)
    }
    pub fn publish(self, path: PathBuf) -> Result<PathBuf, String> {
        if extension(&path) != self.ext {
            return Err(format!("输出文件需要 .{} 扩展名", self.ext));
        }
        let bytes = self.bytes()?;
        if bytes.len() as u64 > 256 << 20 {
            return Err("输出超过 256 MiB 开发预算".into());
        }
        if extension(&path) == "docx" {
            Reader::from_bytes("copy.docx", &bytes, &AtomicBool::new(false))?;
        }
        publish_new(&path, &bytes)?;
        Ok(path)
    }
}
pub fn recipients(path: &Path) -> Result<Value, String> {
    let ext = extension(path);
    if !["csv", "tsv", "txt", "xlsx", "xlsm", "xls", "ods"].contains(&ext.as_str()) {
        return Err("请选择 CSV/TSV 或 Excel/OpenDocument 收件人文件".into());
    }
    let bytes = read_local(path, 16 << 20)?;
    Ok(
        json!({"data":joyword_engine::cmd::insert::base64_encode(&bytes),"path":path.file_name().and_then(|s|s.to_str()).unwrap_or("recipients.csv")}),
    )
}
pub fn comparison(path: &Path) -> Result<Value, String> {
    let bytes = read_local(path, MAX_INPUT_BYTES)?;
    Reader::from_bytes("revised.docx", &bytes, &AtomicBool::new(false))?;
    Ok(
        json!({"data":joyword_engine::cmd::insert::base64_encode(&bytes),"name":"revised.docx","showIn":"original","author":"比较文档"}),
    )
}
pub fn source_params(v: &[String]) -> Result<Value, String> {
    if v.len() != 7 || v[0].trim().is_empty() || v[2].trim().is_empty() {
        return Err("请填写来源标记和标题".into());
    }
    for s in v {
        bounded_text(s, 4096)?;
    }
    Ok(
        json!({"add":{"tag":v[0].trim(),"kind":v[1].trim(),"title":v[2],"author":v[3],"year":v[4],"publisher":v[5],"url":v[6]}}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn blank() -> Reader {
        Reader::blank(&AtomicBool::new(false)).unwrap()
    }
    fn run(r: &mut Reader, id: &str, args: Value) -> Value {
        r.execute(id, &args).unwrap()
    }
    fn reopen(r: &Reader) -> Reader {
        Reader::from_bytes(
            "roundtrip.docx",
            &r.export_job("docx", true).unwrap().bytes().unwrap(),
            &AtomicBool::new(false),
        )
        .unwrap()
    }
    #[test]
    fn references_notes_comments_sources_survive_docx() {
        let mut r = blank();
        run(&mut r, "text.insert", json!({"text":"报告标题"}));
        run(&mut r, "para.style", json!({"style":"Heading 1"}));
        run(&mut r, "text.newParagraph", json!({}));
        run(&mut r, "text.insert", json!({"text":"正文内容"}));
        run(
            &mut r,
            "references.sources",
            json!({"add":{"tag":"Source2026","kind":"book","title":"中文来源","author":"作者","year":"2026"}}),
        );
        run(&mut r, "references.citation", json!({"tag":"Source2026"}));
        run(&mut r, "references.footnote", json!({"text":"真实脚注"}));
        r.return_to_body().unwrap();
        let c = run(&mut r, "review.newComment", json!({"text":"审阅意见"}));
        run(
            &mut r,
            "review.reply",
            json!({"id":c["id"],"text":"已核对"}),
        );
        run(
            &mut r,
            "review.resolveComment",
            json!({"id":c["id"],"value":true}),
        );
        run(&mut r, "caret.docStart", json!({}));
        run(&mut r, "references.toc", json!({"title":"目录","levels":3}));
        let mut back = reopen(&r);
        assert_eq!(back.sources()[0]["tag"], "Source2026");
        assert_eq!(
            back.advanced_query("review.comments")
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(
            back.advanced_query("references.notes")
                .unwrap()
                .to_string()
                .contains("真实脚注")
        );
        assert!(back.body_text().contains("目录"));
        assert!(back.engine().doc.comments.values().any(|c| c.resolved));
    }
    #[test]
    fn tracking_roundtrip_projection_accept_reject_and_compare_bytes() {
        let mut r = blank();
        run(&mut r, "text.insert", json!({"text":"原始内容"}));
        run(&mut r, "review.trackChanges", json!({"value":true}));
        run(&mut r, "text.insert", json!({"text":"新增内容"}));
        let mut back = reopen(&r);
        assert!(
            !back
                .advanced_query("review.changes")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        let rev = back.engine().rev();
        let presentation = back.presentation_revision();
        run(&mut back, "review.markup", json!({"value":"noMarkup"}));
        assert_eq!(back.engine().rev(), rev);
        assert!(back.presentation_revision() > presentation);
        run(&mut back, "review.rejectAll", json!({"stopTracking":true}));
        assert_eq!(back.body_text(), "原始内容");
        let mut revised = blank();
        run(&mut revised, "text.insert", json!({"text":"原始内容修订"}));
        let bytes = revised.export_job("docx", false).unwrap().bytes().unwrap();
        run(
            &mut back,
            "review.compare",
            json!({"data":joyword_engine::cmd::insert::base64_encode(&bytes),"name":"revised.docx","showIn":"original"}),
        );
        assert!(
            !back
                .advanced_query("review.changes")
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        run(&mut back, "review.acceptAll", json!({"stopTracking":true}));
        assert_eq!(back.body_text(), "原始内容修订");
        assert!(
            back.execute("review.compare", &json!({"data":"invalid"}))
                .is_err()
        );
    }
    #[test]
    fn merge_snapshot_filters_preview_and_copy_do_not_replace_template() {
        let mut r = blank();
        run(&mut r, "mailings.start", json!({"kind":"letters"}));
        run(&mut r, "text.insert", json!({"text":"您好，"}));
        run(&mut r, "mailings.insertField", json!({"field":"姓名"}));
        run(
            &mut r,
            "mailings.recipients",
            json!({"csv":"姓名,城市\n张三,北京\n李四,上海\n王五,北京"}),
        );
        run(
            &mut r,
            "mailings.filterRecipients",
            json!({"rules":[{"field":"城市","op":"equal","value":"北京"}]}),
        );
        assert_eq!(r.merge_info()["records"], 2);
        assert_eq!(r.merge_info()["rows"].as_array().unwrap().len(), 2);
        let rev = r.engine().rev();
        let p = r.presentation_revision();
        run(&mut r, "mailings.preview", json!({"value":true}));
        assert!(r.engine().rev() >= rev);
        assert!(r.presentation_revision() > p);
        let id = r.engine().document_id();
        let before = joyword_docx::write(&r.engine().doc).unwrap();
        let job = r.merge_job(false).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("合并 结果.docx");
        job.publish(path.clone()).unwrap();
        let merged = Reader::open(&path, &AtomicBool::new(false)).unwrap();
        assert!(merged.body_text().contains("张三") && merged.body_text().contains("王五"));
        assert!(!merged.body_text().contains("李四"));
        assert_eq!(r.engine().document_id(), id);
        assert_eq!(joyword_docx::write(&r.engine().doc).unwrap(), before);
        assert!(r.merge_job(true).unwrap().publish(path.clone()).is_err());
        assert!(r.is_dirty());
    }
    #[test]
    fn exchange_real_formats_and_new_path_publication() {
        let mut r = blank();
        run(&mut r, "text.insert", json!({"text":"中文格式交换 Hello"}));
        for ext in ["txt", "md", "html", "rtf", "odt", "tex", "xml"] {
            let bytes = r.export_job(ext, false).unwrap().bytes().unwrap();
            assert!(!bytes.is_empty(), "{ext}");
            let back = Reader::from_bytes(&format!("copy.{ext}"), &bytes, &AtomicBool::new(false))
                .unwrap();
            assert!(
                back.body_text().contains("Hello"),
                "{ext}: {}",
                back.body_text()
            );
        }
        assert!(r.export_job("doc", false).is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copy.rtf");
        r.export_job("rtf", false)
            .unwrap()
            .publish(path.clone())
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        assert!(
            r.export_job("rtf", false)
                .unwrap()
                .publish(path.clone())
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    #[test]
    fn macros_rollback_failed_document_steps_and_refuse_external_io() {
        let mut r = blank();
        run(&mut r, "text.insert", json!({"text":"原文"}));
        assert!(r.execute("tools.macros",&json!({"define":{"name":"bad","steps":[{"command":"insert.picture","params":{"path":"secret.png"}}]}})).is_err());
        run(
            &mut r,
            "tools.macros",
            json!({"define":{"name":"rollback","steps":[{"command":"text.insert","params":{"text":"partial"}},{"command":"format.notRegistered"}]}}),
        );
        let before = r.body_text();
        let rev = r.engine().rev();
        assert!(
            r.execute("tools.macros", &json!({"run":"rollback"}))
                .is_err()
        );
        assert_eq!(r.body_text(), before);
        assert!(r.engine().rev() >= rev);
        run(&mut r, "tools.recordMacro", json!({"name":"hello"}));
        run(&mut r, "text.insert", json!({"text":"宏"}));
        run(&mut r, "tools.recordMacro", json!({}));
        run(&mut r, "tools.macros", json!({"run":"hello"}));
        assert!(r.body_text().ends_with("宏宏"));
        r.mark_text(None, "pinyin", None).unwrap();
        assert!(r.export_job("docx", true).is_err());
        assert!(r.cancel_composition());
    }
    #[test]
    fn bounded_imports_reject_bad_data_and_keep_live_reader() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("list.csv");
        std::fs::write(&file, "姓名\n张三").unwrap();
        assert!(recipients(&file).unwrap()["data"].is_string());
        assert!(read_local(&file, 2).is_err());
        assert!(comparison(&file).is_err());
        assert!(bounded_text("a\0b", 100).is_err());
        assert!(validate_macro(&vec![("text.insert".into(), json!({})); 257]).is_err());
    }

    #[test]
    fn merge_expansion_is_rejected_before_materializing_copies() {
        let mut r = blank();
        run(
            &mut r,
            "document.setText",
            json!({"text":"x".repeat(40_000)}),
        );
        run(
            &mut r,
            "mailings.recipients",
            json!({"fields":["Name"],"rows":vec![vec!["record"];1000]}),
        );
        assert!(r.merge_job(false).unwrap().bytes().is_err());
        run(
            &mut r,
            "mailings.recipients",
            json!({"fields":["Name"],"rows":vec![vec!["record"];1001]}),
        );
        assert!(r.merge_job(false).is_err());
        assert!(r.merge_job(true).unwrap().bytes().is_ok());
        assert_eq!(r.body_text().len(), 40_000);
    }
}
