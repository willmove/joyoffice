//! Slide organization, multi-selection and drag/drop share the document transaction path.
use super::*;
use joydeck_engine::{Command, WorkspaceAction};
use joydeck_model::SlideId;

#[derive(Clone)]
pub(super) struct SlideDrag {
    owner: EntityId,
    generation: u64,
    pub(super) slides: Vec<SlideId>,
}
pub(super) struct SlideDragPreview(pub(super) usize);
impl Render for SlideDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_2()
            .bg(rgb(0xffffff))
            .border_1()
            .border_color(rgb(0xd95a1a))
            .child(format!("移动 {} 张幻灯片", self.0))
    }
}
impl Viewer {
    pub(super) fn selected_slides(&self) -> Vec<SlideId> {
        let Some(e) = &self.editor else { return vec![] };
        let p = e.session.model();
        let chosen: Vec<_> = p
            .slides
            .iter()
            .filter(|s| self.slide_selection.contains(&s.id))
            .map(|s| s.id)
            .collect();
        if chosen.is_empty() {
            p.slides
                .get(self.page)
                .map(|s| vec![s.id])
                .unwrap_or_default()
        } else {
            chosen
        }
    }
    pub(super) fn slide_selected(&self, page: usize) -> bool {
        self.editor
            .as_ref()
            .and_then(|e| e.session.model().slides.get(page))
            .is_some_and(|s| self.slide_selection.contains(&s.id) || page == self.page)
    }
    pub(super) fn select_slide(
        &mut self,
        page: usize,
        event: &ClickEvent,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.composing() || self.loading {
            return;
        }
        self.commit_notes(w, cx);
        let Some(e) = &self.editor else { return };
        let p = e.session.model();
        let Some(slide) = p.slides.get(page) else {
            return;
        };
        let id = slide.id;
        let mods = event.modifiers();
        let mut selected = self.selected_slides();
        let anchor = self
            .slide_anchor
            .or_else(|| p.slides.get(self.page).map(|s| s.id));
        if mods.shift {
            let start = anchor.and_then(|id| p.slide_index(id)).unwrap_or(page);
            selected = p.slides[start.min(page)..=start.max(page)]
                .iter()
                .map(|s| s.id)
                .collect();
        } else if mods.control || mods.platform {
            if selected.contains(&id) {
                selected.retain(|n| *n != id);
            } else {
                selected.push(id);
            }
        } else {
            selected = vec![id];
        }
        self.select(page, cx);
        self.slide_selection = selected;
        self.slide_anchor = if mods.shift { anchor } else { Some(id) };
        self.focus.focus(w, cx);
        cx.notify();
    }
    pub(super) fn slide_drag(&self, page: usize, cx: &Context<Self>) -> SlideDrag {
        let id = self
            .editor
            .as_ref()
            .and_then(|e| e.session.model().slides.get(page))
            .map(|s| s.id);
        let selected = self.selected_slides();
        let slides = match id {
            Some(id) if selected.contains(&id) => selected,
            Some(id) => vec![id],
            None => vec![],
        };
        SlideDrag {
            owner: cx.entity_id(),
            generation: self.generation,
            slides,
        }
    }
    pub(super) fn drop_slides(
        &mut self,
        drag: &SlideDrag,
        page: Option<usize>,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if drag.owner != cx.entity_id() || drag.generation != self.generation {
            self.message = "文稿已变化，请重新拖动".into();
            cx.notify();
            return;
        }
        let before = page.and_then(|page| {
            self.editor
                .as_ref()?
                .session
                .model()
                .slides
                .get(page)
                .map(|s| s.id)
        });
        if before.is_some_and(|id| drag.slides.contains(&id)) {
            return;
        }
        self.dispatch(
            Command::Workspace {
                action: WorkspaceAction::ReorderSlides {
                    slides: drag.slides.clone(),
                    before,
                },
            },
            w,
            cx,
        );
        self.slide_selection = drag.slides.clone();
        cx.notify();
    }
    pub(super) fn organize_dialog(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &self.editor else { return };
        let count = e.session.model().slides.len();
        let slides = self.selected_slides();
        self.fields_dialog(
            "移动选中的幻灯片",
            vec![(
                "移到此页之前（末尾填写总页数 + 1）",
                (self.page + 1).to_string(),
            )],
            w,
            cx,
            move |s, v, w, cx| {
                let page: usize = v[0].trim().parse().map_err(|_| "请输入页码")?;
                if !(1..=count + 1).contains(&page) {
                    return Err("页码超出范围".into());
                }
                let before = s
                    .editor
                    .as_ref()
                    .and_then(|e| e.session.model().slides.get(page - 1))
                    .map(|s| s.id);
                if before.is_some_and(|id| slides.contains(&id)) {
                    return Err("目标位置在当前选区内".into());
                }
                s.dispatch(
                    Command::Workspace {
                        action: WorkspaceAction::ReorderSlides {
                            slides: slides.clone(),
                            before,
                        },
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }
    pub(super) fn create_slide_section(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let slides = self.selected_slides();
        self.fields_dialog(
            "将选中页组成节",
            vec![("节名称", String::new())],
            w,
            cx,
            move |s, v, w, cx| {
                s.dispatch(
                    Command::Workspace {
                        action: WorkspaceAction::Section {
                            name: v[0].clone(),
                            slides: slides.clone(),
                        },
                    },
                    w,
                    cx,
                );
                Ok(())
            },
        );
    }

    pub(super) fn toggle_hidden_slides(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let slides = self.selected_slides();
        let hidden = self.editor.as_ref().is_some_and(|e| {
            slides.iter().any(|id| {
                e.session
                    .model()
                    .slide(*id)
                    .is_some_and(|slide| !slide.hidden)
            })
        });
        self.dispatch(
            Command::Workspace {
                action: WorkspaceAction::HideSlides { slides, hidden },
            },
            w,
            cx,
        );
    }
}
