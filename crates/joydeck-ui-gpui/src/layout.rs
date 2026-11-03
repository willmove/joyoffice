//! Logical geometry shared by workspace composition and document fitting.
use joyoffice_ui::metrics;
#[derive(Clone, Copy, Debug)]
pub(crate) struct WorkspaceLayout {
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
