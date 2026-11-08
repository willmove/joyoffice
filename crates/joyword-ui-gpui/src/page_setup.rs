//! Section geometry and breaks on the Layout ribbon.
use super::*;
use joyword_shell::formatting::{column_parameters, page_setup_parameters};

impl JoyWord {
    pub(super) fn open_columns(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(reader) = self.canvas.reader.as_ref() else {
            return;
        };
        let engine = reader.engine();
        let columns = reader.section_columns();
        let units = engine.units();
        let widths = columns
            .widths
            .iter()
            .map(|(width, space)| format!("{width} pt/{space} pt"))
            .collect::<Vec<_>>()
            .join(" | ");
        let tr = |zh, en| self.shell.tr(zh, en);
        self.fields_form(
            tr("更多分栏", "More columns"),
            "layout.columns",
            vec![
                (
                    tr("栏数（1–12）", "Columns (1–12)"),
                    columns.count.to_string(),
                ),
                (
                    tr("等宽栏间距", "Equal-column gap"),
                    format!("{} pt", columns.space),
                ),
                (
                    tr("分隔线（是 / 否）", "Separator line (yes / no)"),
                    if columns.separator {
                        tr("是", "Yes")
                    } else {
                        tr("否", "No")
                    }
                    .into(),
                ),
                (
                    tr(
                        "不等宽（如 100 pt/20 pt | 120 pt）",
                        "Unequal widths (e.g. 100 pt/20 pt | 120 pt)",
                    ),
                    widths,
                ),
                (
                    tr(
                        "应用于（本节 / 整篇文档 / 插入点之后 / 所选文字）",
                        "Apply to (section / document / forward / selection)",
                    ),
                    tr("本节", "section").into(),
                ),
            ],
            true,
            move |values| column_parameters(values, units),
            w,
            cx,
        );
    }

    pub(super) fn open_page_setup(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let Some(reader) = self.canvas.reader.as_ref() else {
            return;
        };
        let tr = |zh, en| self.shell.tr(zh, en);
        let units = reader.engine().units();
        let (lengths, landscape, columns) = reader.page_setup_values();
        let mut fields: Vec<_> = [
            tr("纸张宽度", "Page width"),
            tr("纸张高度", "Page height"),
            tr("上边距", "Top margin"),
            tr("下边距", "Bottom margin"),
            tr("左边距", "Left margin"),
            tr("右边距", "Right margin"),
        ]
        .into_iter()
        .zip(lengths)
        .collect();
        let orientation = if landscape {
            tr("横向", "Landscape")
        } else {
            tr("纵向", "Portrait")
        };
        fields.push((
            tr("方向（纵向 / 横向）", "Orientation (portrait / landscape)"),
            orientation.into(),
        ));
        fields.push((tr("栏数（1–12）", "Columns (1–12)"), columns.to_string()));
        self.fields_form(
            tr("页面设置", "Page setup"),
            "layout.pageSetup",
            fields,
            true,
            move |values| page_setup_parameters(values, units),
            w,
            cx,
        );
    }
}
