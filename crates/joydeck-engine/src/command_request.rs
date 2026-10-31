//! Version-one machine parameters for the registered editing commands.
//! Conversion is pure; validation and commit use the same editing kernel as UI.
use crate::{
    design_commands,
    document::{Object, Result},
    edit::Command,
    motion, playback, review, table,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "command", deny_unknown_fields)]
pub enum CommandRequest {
    #[serde(rename = "page.add")]
    AddPage {},
    #[serde(rename = "page.copy")]
    CopyPages { pages: Vec<usize> },
    #[serde(rename = "page.delete")]
    DeletePages { pages: Vec<usize> },
    #[serde(rename = "page.reorder")]
    ReorderPages { pages: Vec<usize>, target: usize },
    #[serde(rename = "page.visibility")]
    HidePages { pages: Vec<usize> },
    #[serde(rename = "page.size")]
    PageSize { width: f32, height: f32 },
    #[serde(rename = "object.add")]
    AddObject { page: usize, object: Box<Object> },
    #[serde(rename = "object.update")]
    UpdateObjects { page: usize, objects: Vec<Object> },
    #[serde(rename = "object.delete")]
    DeleteObjects { page: usize, objects: Vec<usize> },
    #[serde(rename = "object.reorder")]
    ReorderObjects {
        page: usize,
        objects: Vec<usize>,
        target: usize,
    },
    #[serde(rename = "review.edit")]
    Review { action: review::Action },
    #[serde(rename = "table.edit")]
    Table {
        page: usize,
        object: String,
        action: table::Action,
    },
    #[serde(rename = "document.design")]
    Design { action: design_commands::Action },
    #[serde(rename = "presentation.motion")]
    Motion { presentation: motion::Presentation },
    #[serde(rename = "media.edit")]
    Media { playback: playback::Playback },
    #[serde(rename = "transaction.batch")]
    Batch { commands: Vec<CommandRequest> },
}
impl CommandRequest {
    pub fn into_command(self) -> Result<Command> {
        self.convert(0)
    }
    fn convert(self, depth: usize) -> Result<Command> {
        if depth > 16 {
            return Err("复合命令嵌套超限".into());
        }
        Ok(match self {
            Self::AddPage {} => Command::AddPage,
            Self::CopyPages { pages } => Command::DuplicatePages(pages),
            Self::DeletePages { pages } => Command::DeletePages(pages),
            Self::ReorderPages { pages, target } => Command::ReorderPages(pages, target),
            Self::HidePages { pages } => Command::HidePages(pages),
            Self::PageSize { width, height } => Command::PageSize(width, height),
            Self::AddObject { page, object } => Command::AddObject(page, *object),
            Self::UpdateObjects { page, objects } => {
                if objects.is_empty() {
                    return Err("对象选择为空".into());
                }
                Command::ReplaceMany(page, objects)
            }
            Self::DeleteObjects { page, mut objects } => {
                objects.sort_unstable();
                if objects.is_empty() || objects.windows(2).any(|v| v[0] == v[1]) {
                    return Err("对象选择为空或重复".into());
                }
                if objects.len() > 1000 {
                    return Err("批量删除超过1000个对象".into());
                }
                Command::Batch(
                    objects
                        .into_iter()
                        .rev()
                        .map(|i| Command::DeleteObject(page, i))
                        .collect(),
                )
            }
            Self::ReorderObjects {
                page,
                objects,
                target,
            } => Command::ReorderObjects(page, objects, target),
            Self::Review { action } => Command::Review(action),
            Self::Table {
                page,
                object,
                action,
            } => Command::Table(page, object, action),
            Self::Design { action } => Command::Design(action),
            Self::Motion { presentation } => Command::Motion(presentation),
            Self::Media { playback } => Command::Playback(playback),
            Self::Batch { commands } => {
                if commands.is_empty() || commands.len() > 1000 {
                    return Err("复合命令须包含1–1000条命令".into());
                }
                Command::Batch(
                    commands
                        .into_iter()
                        .map(|c| c.convert(depth + 1))
                        .collect::<Result<_>>()?,
                )
            }
        })
    }
}
