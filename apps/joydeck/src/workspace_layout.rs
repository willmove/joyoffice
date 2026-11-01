//! Logical geometry shared by workspace composition and document fitting.
use super::*;
use joyoffice_ui::metrics;
#[derive(Clone, Copy, Debug)]
pub(super) struct WorkspaceLayout {
    pub rail: f32,
    pub pane: f32,
    pub padding: f32,
    pub ribbon: f32,
    pub notes: f32,
    pub stage_width: f32,
    pub stage_height: f32,
    pub compact: bool,
}
impl WorkspaceLayout {
    pub fn new(width: f32, height: f32, pane: bool, notes: bool, home: bool) -> Self {
        let compact = width < 1100. && home;
        let rail = if width >= 1320. {
            232.
        } else if width >= 1100. {
            212.
        } else if width >= 960. {
            184.
        } else {
            144.
        };
        let pane = if pane {
            if width >= 1100. { 288. } else { 260. }
        } else {
            0.
        };
        let padding = if width >= 1100. { 24. } else { 16. };
        let ribbon = if compact { 64. } else { 96. };
        let notes = if notes { 124. } else { 32. };
        Self {
            rail,
            pane,
            padding,
            ribbon,
            notes,
            compact,
            stage_width: (width - rail - pane).max(1.),
            stage_height: (height
                - metrics::HEADER
                - metrics::TAB
                - ribbon
                - metrics::STATUS
                - notes)
                .max(1.),
        }
    }
    pub fn fit(&self, width: f32, height: f32) -> f32 {
        ((self.stage_width - self.padding * 2.) / width)
            .min((self.stage_height - self.padding * 2.) / height)
            .max(0.01)
    }
}
impl Editor {
    pub(super) fn workspace_layout(&self, w: &Window) -> WorkspaceLayout {
        let pane = self.inspector_open
            || matches!(
                self.ribbon.as_str(),
                "文件"
                    | "设计"
                    | "表格"
                    | "图表"
                    | "图示"
                    | "动画"
                    | "切换"
                    | "幻灯片放映"
                    | "审阅"
                    | "媒体"
                    | "输出"
                    | "比较"
                    | "维护"
            );
        WorkspaceLayout::new(
            f32::from(w.viewport_size().width),
            f32::from(w.viewport_size().height),
            pane,
            self.show_notes,
            self.ribbon == "开始",
        )
    }
    pub(super) fn canvas_scale(&self, w: &Window) -> f32 {
        let scale = self
            .workspace_layout(w)
            .fit(self.session.deck.width, self.session.deck.height)
            * self.zoom;
        let mut a = self.session.deck.width.round() as u32;
        let mut b = self.session.deck.height.round() as u32;
        while b != 0 {
            let n = a % b;
            a = b;
            b = n;
        }
        let step = self.session.deck.width / a.max(1) as f32;
        if step <= 32. {
            ((self.session.deck.width * scale / step).floor().max(1.) * step)
                / self.session.deck.width
        } else {
            scale
        }
    }
}
#[cfg(test)]
mod tests {
    use super::WorkspaceLayout;
    #[test]
    fn fit_keeps_landscape_and_portrait_pages_inside_every_workspace_state() {
        for width in [800., 960., 1200., 1600.] {
            for pane in [false, true] {
                for notes in [false, true] {
                    for home in [false, true] {
                        let l = WorkspaceLayout::new(width, 700., pane, notes, home);
                        for (pw, ph) in [(960., 540.), (540., 960.), (10000., 10000.)] {
                            let scale = l.fit(pw, ph);
                            assert!(pw * scale <= l.stage_width - 2. * l.padding + 0.01);
                            assert!(ph * scale <= l.stage_height - 2. * l.padding + 0.01);
                        }
                    }
                }
            }
        }
    }
}
