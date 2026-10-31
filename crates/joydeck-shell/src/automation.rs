//! Synchronous, framework-independent dispatcher. No listener is created by this module.
use crate::{document::Result, edit::Command, review, workspace::SharedSession as Session};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};
pub const VERSION: &str = "joydeck-control-1";
pub const LIMIT: usize = 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub protocol_version: String,
    pub request_id: String,
    pub method: String,
    pub target: Option<u64>,
    #[serde(default)]
    pub params: Value,
}
#[derive(Clone, Debug, Serialize)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}
pub struct Dispatcher {
    pub remote: Option<PathBuf>,
    pub session: Session,
    read: Vec<PathBuf>,
    write: Vec<PathBuf>,
    completed: BTreeMap<String, (Vec<u8>, Value)>,
    completed_bytes: usize,
}
impl Dispatcher {
    pub fn new(read: Vec<PathBuf>, write: Vec<PathBuf>) -> Result<Self> {
        let roots = |paths: Vec<PathBuf>| {
            paths
                .into_iter()
                .map(|p| {
                    p.canonicalize().map_err(|e| e.to_string()).and_then(|p| {
                        if p.is_dir() {
                            Ok(p)
                        } else {
                            Err("root必须是目录".into())
                        }
                    })
                })
                .collect::<Result<Vec<_>>>()
        };
        Ok(Self {
            remote: None,
            session: Session::new(),
            read: roots(read)?,
            write: roots(write)?,
            completed: BTreeMap::new(),
            completed_bytes: 0,
        })
    }
    pub fn path(&self, p: &str, write: bool) -> Result<PathBuf> {
        let path = Path::new(p);
        if !path.is_absolute() {
            return Err("需要绝对路径".into());
        }
        // Parent traversal, dangling links and symlink output targets must never bypass roots.
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err("路径含父级跳转".into());
        }
        let canonical = if write {
            if path.symlink_metadata().is_ok() {
                return Err("输出必须是不存在的新文件".into());
            }
            path.parent()
                .ok_or("缺少父目录")?
                .canonicalize()
                .map_err(|e| e.to_string())?
                .join(path.file_name().ok_or("缺少文件名")?)
        } else {
            path.canonicalize().map_err(|e| e.to_string())?
        };
        if !(if write { &self.write } else { &self.read })
            .iter()
            .any(|root| canonical.starts_with(root))
        {
            return Err("路径超出已授权roots".into());
        }
        Ok(canonical)
    }
    pub fn dispatch(&mut self, r: Request) -> Value {
        if let Some(path) = &self.remote {
            return crate::control::remote(path, &r).unwrap_or_else(
                |e| json!({"requestId":r.request_id,"error":{"code":"remote-error","message":e}}),
            );
        }
        let id = r.request_id.clone();
        let err =
            |code, message: String| json!({"requestId":id,"error":{"code":code,"message":message}});
        if r.protocol_version != VERSION || id.is_empty() || id.len() > 128 {
            return err("invalid-request", "协议版本或requestId无效".into());
        }
        let Ok(bytes) = serde_json::to_vec(&r) else {
            return err("invalid-request", "无效JSON".into());
        };
        if bytes.len() > LIMIT {
            return err("limit", "请求超过1MiB".into());
        }
        if let Some((old, response)) = self.completed.get(&id) {
            return if old == &bytes {
                response.clone()
            } else {
                err("id-conflict", "相同ID不得使用不同参数".into())
            };
        }
        if self.completed.len() >= 1024
            || self.completed_bytes + bytes.len() + 4 * 1024 * 1024 > 16 * 1024 * 1024
        {
            return err("limit", "会话去重记录或16MiB预算已满；请启动新会话".into());
        }
        let result = self
            .execute(&r)
            .map(|v| json!({"requestId":id,"result":v}))
            .unwrap_or_else(|e| err("operation-failed", e));
        let result =
            if serde_json::to_vec(&result).map_or(usize::MAX, |b| b.len()) > 4 * 1024 * 1024 {
                err("limit", "响应超过4MiB；请缩小文稿或使用导出".into())
            } else {
                result
            };
        if matches!(
            r.method.as_str(),
            "engine.commands" | "document.inspect" | "document.check" | "document.compare"
        ) {
            return result;
        }
        self.completed_bytes += bytes.len() + serde_json::to_vec(&result).map_or(0, |b| b.len());
        self.completed.insert(id, (bytes, result.clone()));
        result
    }
    fn execute(&mut self, r: &Request) -> Result<Value> {
        self.session.sync();
        let p = &r.params;
        let string = |key: &str| {
            p[key]
                .as_str()
                .ok_or_else(|| format!("缺少字符串参数{key}"))
        };
        if !matches!(
            r.method.as_str(),
            "engine.commands" | "document.new" | "document.open"
        ) && r.target != Some(self.session.id)
        {
            return Err("target与当前文稿ID不匹配".into());
        }
        match r.method.as_str() {
            "engine.commands" => Ok(describe()),
            "document.new" | "document.open" => {
                if self.session.dirty() && p["dirtyPolicy"] != "discard" {
                    return Err("文稿有未保存修改；需显式dirtyPolicy=discard".into());
                }
                let deck = if r.method == "document.open" {
                    Some(crate::document::open(
                        &self.path(string("path")?, false)?,
                        &AtomicBool::new(false),
                    )?)
                } else {
                    None
                };
                self.session = deck
                    .map(|d| Session::from_deck(d, true))
                    .unwrap_or_default();
                Ok(json!({"documentId":self.session.id}))
            }
            "document.inspect" => {
                let mut deck = (*self.session.deck).clone();
                deck.source = None;
                deck.resources.clear();
                for a in deck.playback.assets.values_mut() {
                    std::sync::Arc::make_mut(a).bytes.clear();
                }
                Ok(
                    json!({"documentId":self.session.id,"revision":self.session.revision,"dirty":self.session.dirty(),"deck":deck,"assetBytesOmitted":true}),
                )
            }
            "document.check" => Ok(json!(review::check(&self.session.deck))),
            "document.compare" | "document.acceptObject" => {
                let other = crate::document::open(
                    &self.path(string("path")?, false)?,
                    &AtomicBool::new(false),
                )?;
                let report = crate::compare::compare(&self.session.deck, &other)?;
                if r.method == "document.compare" {
                    return Ok(json!(report));
                }
                if p["baseSha256"] != report.base_sha256
                    || p["referenceSha256"] != report.reference_sha256
                {
                    return Err("比较版本已改变，请重新比较".into());
                }
                let index = p["index"].as_u64().ok_or("缺少index")? as usize;
                let command =
                    crate::compare::accept_object(&self.session.deck, &other, &report, index)?;
                self.session.execute(self.session.id, command)?;
                Ok(json!({"revision":self.session.revision}))
            }
            "command.execute" => {
                let c = match string("command")? {
                    "page.add" => Command::AddPage,
                    "review.edit" => Command::Review(
                        serde_json::from_value(p["action"].clone()).map_err(|e| e.to_string())?,
                    ),
                    "object.add" => Command::AddObject(
                        p["page"].as_u64().ok_or("缺少page")? as usize,
                        serde_json::from_value(p["object"].clone()).map_err(|e| e.to_string())?,
                    ),
                    _ => return Err("不支持的自动化命令".into()),
                };
                self.session.execute(self.session.id, c)?;
                Ok(json!({"revision":self.session.revision}))
            }
            "document.undo" => {
                self.session.undo();
                Ok(json!({"revision":self.session.revision}))
            }
            "document.redo" => {
                self.session.redo();
                Ok(json!({"revision":self.session.revision}))
            }
            "document.save" => {
                let path = self.path(string("path")?, true)?;
                crate::edit::save_new(&path, &self.session.deck)?;
                self.session.mark_saved(self.session.deck.clone());
                Ok(json!({"saved":true}))
            }
            "document.export" => {
                let path = self.path(string("path")?, true)?;
                let format = string("format")?;
                let d = &self.session.deck;
                let cancel = AtomicBool::new(false);
                let pages = crate::export::pages(d, p["range"].as_str().unwrap_or(""), false)?;
                match format {
                    "vtt" => {
                        let options = serde_json::from_value::<crate::video::Options>(
                            p.get("video").cloned().unwrap_or_else(|| json!({})),
                        )
                        .map_err(|e| e.to_string())?;
                        let text = crate::video::captions(d, &pages, &options)?;
                        crate::export::atomic(&path, text.as_bytes(), None, &cancel)?;
                    }
                    "mp4" => {
                        if p["acceptLosses"] != true {
                            return Err(
                                "静态页MP4不含音轨/动画/转场/嵌入媒体；需acceptLosses=true".into(),
                            );
                        }
                        let options = serde_json::from_value::<crate::video::Options>(
                            p.get("video").cloned().unwrap_or_else(|| json!({})),
                        )
                        .map_err(|e| e.to_string())?;
                        let report = crate::video::export(&path, d, &pages, &options, &cancel)?;
                        return Ok(json!({"exported":true,"format":"mp4","video":report}));
                    }
                    "pdf" => crate::output::vector_pdf(&path, d, &pages, &cancel)?,
                    "pptx" | "potx" | "ppsx" => {
                        if p["acceptLosses"] != true {
                            return Err("公开格式输出需acceptLosses=true".into());
                        }
                        crate::output::presentation(&path, d, format, &cancel)?;
                    }
                    "outline" | "rtf" | "gif" => {
                        crate::output::other(&path, d, &pages, format, &cancel)?
                    }
                    "png" => {
                        if pages.len() != 1 {
                            return Err("PNG导出需要单页range".into());
                        }
                        let image = crate::export::raster(d, pages[0], 144, false, &cancel)?;
                        let bytes =
                            crate::export::encoded(&image, crate::export::ImageFormat::Png, 144)?;
                        crate::export::atomic(&path, &bytes, None, &cancel)?;
                    }
                    _ => return Err("未知输出格式".into()),
                }
                Ok(json!({"exported":true,"format":format}))
            }
            _ => Err("unsupported：该方法未实现（无桌面远程/异步jobs）".into()),
        }
    }
}
pub fn describe() -> Value {
    json!({"protocolVersion":VERSION,"backend":"headless","methods":["engine.commands","document.new","document.open","document.inspect","document.check","document.compare","document.acceptObject","command.execute","document.undo","document.redo","document.save","document.export"],"commands":["page.add","object.add","review.edit"],"limits":{"requestBytes":LIMIT,"operationRecords":1024,"replayBytes":16777216,"responseBytes":4194304},"execution":"serial-synchronous; optional authenticated loopback bridge; no asynchronous jobs"})
}
