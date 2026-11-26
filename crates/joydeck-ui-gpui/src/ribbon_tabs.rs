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

    pub(super) fn id(self) -> &'static str {
        match self {
            Self::File => "tab-file",
            Self::Home => "tab-home",
            Self::Insert => "tab-insert",
            Self::Draw => "tab-draw",
            Self::Design => "tab-design",
            Self::Transitions => "tab-transitions",
            Self::Animations => "tab-animations",
            Self::Show => "tab-show",
            Self::Record => "tab-record",
            Self::Review => "tab-review",
            Self::View => "tab-view",
            Self::Shape => "tab-shape",
            Self::Picture => "tab-picture",
            Self::TableDesign => "tab-table-design",
            Self::TableLayout => "tab-table-layout",
            Self::ChartDesign => "tab-chart-design",
            Self::ChartFormat => "tab-chart-format",
        }
    }

    pub(super) fn label(self, locale: joyoffice_ui::UiLocale) -> &'static str {
        let key = match self {
            Self::File => "tab.file",
            Self::Home => "tab.home",
            Self::Insert => "tab.insert",
            Self::Draw => "tab.draw",
            Self::Design => "tab.design",
            Self::Transitions => "tab.transitions",
            Self::Animations => "tab.animations",
            Self::Show => "tab.show",
            Self::Record => "tab.record",
            Self::Review => "tab.review",
            Self::View => "tab.view",
            Self::Shape => "tab.shape",
            Self::Picture => "tab.picture",
            Self::TableDesign => "tab.table_design",
            Self::TableLayout => "tab.table_layout",
            Self::ChartDesign => "tab.chart_design",
            Self::ChartFormat => "tab.chart_format",
        };
        crate::l10n::t(locale, key)
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
