//! Section geometry and breaks on the Layout ribbon.
use super::*;
use joyword_shell::formatting::{column_parameters, page_setup_parameters};
use serde_json::json;

impl JoyWord {
    pub(super) fn page_setup_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        div()
            .flex()
            .items_start()
            .gap_1()
            .px_3()
            .child(
                Button::new("page-setup")
                    .ghost()
                    .xsmall()
                    .label(tr("页面设置…", "Page setup…"))
                    .disabled(!self.format_ready("layout.pageSetup"))
                    .on_click(cx.listener(|v, _, w, cx| v.open_page_setup(w, cx))),
            )
            .child(
                Button::new("advanced-columns")
                    .ghost()
                    .xsmall()
                    .label(tr("更多分栏…", "More columns…"))
                    .disabled(!self.format_ready("layout.columns"))
                    .on_click(cx.listener(|v, _, w, cx| v.open_columns(w, cx))),
            )
            .child(self.author_button("paper-a4", "A4", "layout.size", json!({"name":"A4"}), cx))
            .child(self.author_button(
                "paper-letter",
                "Letter",
                "layout.size",
                json!({"name":"Letter"}),
                cx,
            ))
            .child(self.author_button(
                "page-break",
                tr("分页符", "Page break"),
                "layout.break",
                json!({"kind":"page"}),
                cx,
            ))
            .child(self.author_button(
                "section-break",
                tr("下一页分节符", "Next page section break"),
                "layout.break",
                json!({"kind":"nextPage"}),
                cx,
            ))
            .child(
                self.choices(
                    "more-breaks",
                    tr("其他分隔符", "More breaks").into(),
                    "layout.break",
                    [
                        (tr("分栏符", "Column break"), "column"),
                        (tr("换行符", "Text wrapping break"), "textWrapping"),
                        (tr("连续分节符", "Continuous section break"), "continuous"),
                        (tr("偶数页分节符", "Even page section break"), "evenPage"),
                        (tr("奇数页分节符", "Odd page section break"), "oddPage"),
                    ]
                    .into_iter()
                    .map(|(label, kind)| (label.into(), json!({"kind":kind})))
                    .collect(),
                    cx,
                ),
            )
    }

    fn open_columns(&mut self, w: &mut Window, cx: &mut Context<Self>) {
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

    fn open_page_setup(&mut self, w: &mut Window, cx: &mut Context<Self>) {
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
