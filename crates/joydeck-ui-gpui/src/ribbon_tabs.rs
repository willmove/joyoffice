//! JoyDeck editor tab order and selection transitions.

use joydeck_model::ShapeKind;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum RibbonTab {
    File,
    #[default]
    Home,
    Insert,
    Draw,
    Design,
    Transitions,
    Animations,
    Show,
    Record,
    Review,
    View,
    Shape,
    Picture,
    TableDesign,
    TableLayout,
    ChartDesign,
    ChartFormat,
}

impl RibbonTab {
    pub(super) const STANDARD: [Self; 11] = [
        Self::File,
        Self::Home,
        Self::Insert,
        Self::Draw,
        Self::Design,
        Self::Transitions,
        Self::Animations,
        Self::Show,
        Self::Record,
        Self::Review,
        Self::View,
    ];

    pub(super) fn contextual(selection: Option<&ShapeKind>) -> &'static [Self] {
        match selection {
            Some(ShapeKind::Picture { .. }) => &[Self::Picture],
            Some(ShapeKind::Chart(_)) => &[Self::ChartDesign, Self::ChartFormat],
            Some(ShapeKind::Table(_)) => &[Self::TableDesign, Self::TableLayout],
            Some(_) => &[Self::Shape],
            None => &[],
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::File => "文件",
            Self::Home => "开始",
            Self::Insert => "插入",
            Self::Draw => "绘图",
            Self::Design => "设计",
            Self::Transitions => "切换",
            Self::Animations => "动画",
            Self::Show => "幻灯片放映",
            Self::Record => "录制",
            Self::Review => "审阅",
            Self::View => "视图",
            Self::Shape => "形状格式",
            Self::Picture => "图片格式",
            Self::TableDesign => "表格设计",
            Self::TableLayout => "表格布局",
            Self::ChartDesign => "图表设计",
            Self::ChartFormat => "格式",
        }
    }

    pub(super) fn visible(self, backstage: bool, contextual: &[Self]) -> Self {
        if backstage {
            Self::File
        } else if self != Self::File
            && (Self::STANDARD.contains(&self) || contextual.contains(&self))
        {
            self
        } else {
            Self::Home
        }
    }

    pub(super) fn press(self, selected: bool, folded: bool, double: bool) -> TabAction {
        if selected {
            if double {
                return TabAction::SetFolded(!folded);
            }
            if folded {
                return TabAction::SetFolded(false);
            }
            return TabAction::Stay;
        }
        if self == Self::File {
            TabAction::OpenFile
        } else {
            TabAction::Open(self)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TabAction {
    Stay,
    SetFolded(bool),
    OpenFile,
    Open(RibbonTab),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contextual_tabs_are_scoped_to_selection_and_file_takes_precedence() {
        use RibbonTab::*;
        let tabs = RibbonTab::contextual(Some(&ShapeKind::Shape));
        assert_eq!(tabs, &[Shape]);
        assert_eq!(Shape.visible(false, tabs), Shape);
        assert_eq!(ChartDesign.visible(false, tabs), Home);
        assert_eq!(Shape.visible(true, tabs), File);
        assert_eq!(Shape.visible(false, &[]), Home);
    }

    #[test]
    fn selected_tab_reopens_a_folded_strip_and_double_activation_toggles_it() {
        use RibbonTab::*;
        assert_eq!(Home.press(true, false, false), TabAction::Stay);
        assert_eq!(Home.press(true, true, false), TabAction::SetFolded(false));
        assert_eq!(Home.press(true, false, true), TabAction::SetFolded(true));
        assert_eq!(Insert.press(false, true, false), TabAction::Open(Insert));
        assert_eq!(File.press(false, false, false), TabAction::OpenFile);
    }
}
