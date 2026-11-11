//! Native authoring queries and bounded import, over the single editor Session.
use crate::formatting::optional_number;
use crate::reader::Reader;
pub use joyword_engine::cmd::citations::STYLES as CITATION_STYLES;
pub use joyword_engine::cmd::design::THEMES as DOCUMENT_THEMES;
pub use joyword_engine::cmd::draw::{DrawMode, PenSettings, lasso_selection};
pub use joyword_engine::cmd::format::enclose_target;
pub use joyword_engine::cmd::phonetic::state as phonetic_state;
pub use joyword_engine::cmd::{table_mouse, table_resize};
pub use joyword_engine::doc::para::Wrap;
pub use joyword_engine::doc::{InlineObject, Path as DocPath, Pos, StoryRef};
pub use joyword_layout::hit::ObjectHit;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs::File, io::Read, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleInfo {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub based_on: Option<String>,
    pub next: Option<String>,
    pub builtin: bool,
    pub quick: bool,
    pub hidden: bool,
    pub chr: Value,
    pub para: Value,
    pub resolved: Value,
}
#[derive(Clone, Debug, Serialize)]
pub struct TableInfo {
    pub rows: usize,
    pub cols: usize,
    pub row: usize,
    pub col: usize,
    pub props: Value,
    pub cell: Value,
}
#[derive(Clone, Debug, Serialize)]
pub struct PictureInfo {
    pub pos: Pos,
    pub width: f32,
    pub height: f32,
    pub crop: [f32; 4],
    pub alt: String,
    pub float: Value,
}
impl Reader {
    /// Screen-only shading for visible merge fields. Inspect only paragraphs
    /// placed on this page, so a large document is not rescanned on every paint.
    pub fn merge_field_rects(&self, page: usize) -> Vec<joyword_geom::Rect> {
        use joyword_layout::Placed;
        let Some(page_items) = self.layout.pages.get(page) else {
            return Vec::new();
        };
        let mut paragraphs = Vec::new();
        for item in &page_items.items {
            if let Placed::Lines { story, path, .. } = item
                && !paragraphs.contains(&(*story, path.clone()))
            {
                paragraphs.push((*story, path.clone()));
            }
        }
        let mut rects = Vec::new();
        for (story, path) in paragraphs {
            let Some(p) = self.session.doc.para(story, &path) else {
                continue;
            };
            for off in p.object_offsets() {
                if let Some(InlineObject::Field { instr, .. }) = p.object_at(off)
                    && instr.split_whitespace().next().is_some_and(|name| {
                        ["MERGEFIELD", "ADDRESSBLOCK", "GREETINGLINE"]
                            .iter()
                            .any(|field| name.eq_ignore_ascii_case(field))
                    })
                {
                    let a = Pos {
                        story,
                        path: path.clone(),
                        off,
                    };
                    let b = Pos {
                        off: off + '\u{fffc}'.len_utf8(),
                        ..a.clone()
                    };
                    rects.extend(
                        self.layout
                            .selection_rects(&self.session.doc, &a, &b, page)
                            .into_iter()
                            .filter_map(|(p, r)| (p == page).then_some(r)),
                    );
                    if rects.len() >= 512 {
                        rects.truncate(512);
                        return rects;
                    }
                }
            }
        }
        rects
    }
    pub fn styles(&self) -> Vec<StyleInfo> {
        self.session
            .doc
            .styles
            .styles
            .iter()
            .map(|s| StyleInfo {
                id: s.id.clone(),
                name: s.name.clone(),
                kind: match s.kind {
                    joyword_engine::doc::styles::StyleKind::Paragraph => "paragraph",
                    joyword_engine::doc::styles::StyleKind::Character => "character",
                    joyword_engine::doc::styles::StyleKind::Table => "table",
                    _ => "numbering",
                }
                .into(),
                based_on: s.based_on.clone(),
                next: s.next.clone(),
                builtin: s.builtin,
                quick: s.quick,
                hidden: s.hidden,
                chr: serde_json::to_value(&s.chr).unwrap_or_default(),
                para: serde_json::to_value(&s.para).unwrap_or_default(),
                resolved: serde_json::to_value(
                    self.session.doc.styles.resolve_char(
                        (s.kind != joyword_engine::doc::styles::StyleKind::Character)
                            .then_some(s.id.as_str()),
                        &joyword_engine::doc::CharProps {
                            style: (s.kind == joyword_engine::doc::styles::StyleKind::Character)
                                .then_some(s.id.clone()),
                            ..Default::default()
                        },
                    ),
                )
                .unwrap_or_default(),
            })
            .collect()
    }
    pub fn table_info(&self) -> Option<TableInfo> {
        let (path, row, col) = self.session.sel.focus.path.cell()?;
        let table = self
            .session
            .doc
            .table(self.session.sel.focus.story, &path)?;
        Some(TableInfo {
            rows: table.rows.len(),
            cols: table.grid.len(),
            row,
            col,
            props: serde_json::to_value(&table.props).ok()?,
            cell: serde_json::to_value(&table.rows.get(row)?.cells.get(col)?.props).ok()?,
        })
    }
    pub fn picture_info(&self) -> Option<PictureInfo> {
        let (pos, obj) = joyword_engine::cmd::objects::object_selection(&self.session)?;
        let InlineObject::Image {
            w,
            h,
            crop,
            alt,
            float,
            ..
        } = obj
        else {
            return None;
        };
        Some(PictureInfo {
            pos,
            width: *w,
            height: *h,
            crop: *crop,
            alt: alt.clone(),
            float: serde_json::to_value(float).ok()?,
        })
    }
    pub fn select_object(&mut self, pos: Pos) -> Result<(), String> {
        let end = Pos {
            off: pos.off + '\u{fffc}'.len_utf8(),
            ..pos.clone()
        };
        self.select_positions(pos, end)
    }
}

pub fn required_number(s: &str, min: f64, max: f64) -> Result<f64, String> {
    optional_number(s, min, max)?.ok_or("请填写数值".into())
}
pub fn integer(s: &str, min: u32, max: u32) -> Result<u32, String> {
    let n = required_number(s, f64::from(min), f64::from(max))?;
    if n.fract() != 0. {
        return Err("请输入整数".into());
    }
    Ok(n as u32)
}
pub fn style_name(s: &str) -> Result<String, String> {
    let s = s.trim();
    if s.is_empty() || s.len() > 128 || s.chars().any(char::is_control) {
        return Err("样式名称需要 1–128 字节且不能含控制字符".into());
    }
    Ok(s.into())
}
pub fn insert_table_params(v: &[String]) -> Result<Value, String> {
    if v.len() != 2 {
        return Err("Invalid table form".into());
    }
    let rows = integer(&v[0], 1, 100)?;
    let cols = integer(&v[1], 1, 32)?;
    if rows * cols > 1000 {
        return Err("单次插入最多 1000 个单元格".into());
    }
    Ok(json!({"rows":rows,"cols":cols}))
}
pub const IMAGE_BYTES: u64 = 32 << 20;
pub const IMAGE_PIXELS: u64 = 16_777_216;
pub fn import_image(path: &Path) -> Result<Value, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    image_bytes(&bytes)
}
pub fn image_bytes(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() as u64 > IMAGE_BYTES {
        return Err("图片文件超过 32 MiB".into());
    }
    let pic = joyword_geom::picture::probe(bytes).map_err(|e| format!("无法读取图片：{e:?}"))?;
    if u64::from(pic.width) * u64::from(pic.height) > IMAGE_PIXELS || !pic.fits(128 << 20) {
        return Err("图片超过 16 MP 或解码内存预算".into());
    }
    pic.decode(128 << 20)
        .map_err(|e| format!("图片解码失败：{e:?}"))?;
    Ok(json!({"data":joyword_engine::cmd::insert::base64_encode(bytes)}))
}
pub fn picture_crop(v: &[String]) -> Result<Value, String> {
    if v.len() != 4 {
        return Err("Invalid crop form".into());
    }
    let n: Vec<f64> = v
        .iter()
        .map(|x| required_number(x, 0., 95.).map(|n| n / 100.))
        .collect::<Result<_, _>>()?;
    if n[0] + n[2] > 0.95 || n[1] + n[3] > 0.95 {
        return Err("每个方向至少保留 5% 图片".into());
    }
    Ok(json!({"left":n[0],"top":n[1],"right":n[2],"bottom":n[3]}))
}

#[cfg(test)]
#[path = "authoring_tests.rs"]
#[allow(clippy::unwrap_used, reason = "Test setup and assertions")]
mod tests;
