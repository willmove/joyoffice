//! Section geometry and breaks on the Layout ribbon.
use super::*;
use joyword_shell::formatting::page_setup_parameters;
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
