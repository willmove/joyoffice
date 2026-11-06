use super::*;
use serde_json::json;

impl JoyWord {
    pub(super) fn header_footer_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let reader = self.canvas.reader.as_ref();
        let editing = reader.is_some_and(|reader| reader.engine().in_header_footer());
        let (first, even) = reader.map_or((false, false), |reader| reader.header_footer_flags());
        let tr = |zh, en| self.shell.tr(zh, en);
        div()
            .flex()
            .gap_1()
            .children(
                [
                    ("edit-header", tr("页眉", "Header"), "insert.editHeader"),
                    ("edit-footer", tr("页脚", "Footer"), "insert.editFooter"),
                ]
                .into_iter()
                .map(|(id, label, command)| {
                    Button::new(id)
                        .ghost()
                        .xsmall()
                        .label(label)
                        .disabled(!self.format_ready(command))
                        .on_click(cx.listener(move |v, _, w, cx| {
                            let page = v.canvas.reader.as_ref().and_then(|reader| {
                                let session = reader.engine();
                                session.in_header_footer().then_some(session.page_hint)
                            });
                            let params =
                                page.map_or_else(|| json!({}), |page| json!({"page":page}));
                            v.run_format(command, &params, w, cx);
                        }))
                }),
            )
            .when(editing, |group| {
                group
                    .child(self.author_button(
                        "insert-page-number",
                        tr("在光标处插入页码", "Page number at caret"),
                        "insert.pageNumber",
                        json!({"position":"current"}),
                        cx,
                    ))
                    .child(self.author_button(
                        "close-header-footer",
                        tr("关闭页眉页脚", "Close header and footer"),
                        "insert.closeHeader",
                        json!({}),
                        cx,
                    ))
            })
            .when(!editing, |group| {
                group.children(
                    [
                        (
                            "first-page",
                            tr("首页不同", "Different first page"),
                            "layout.differentFirstPage",
                            first,
                        ),
                        (
                            "odd-even",
                            tr("奇偶页不同", "Different odd/even pages"),
                            "layout.differentOddEven",
                            even,
                        ),
                    ]
                    .into_iter()
                    .map(|(id, label, command, active)| {
                        self.author_button(id, label, command, json!({"value":!active}), cx)
                            .selected(active)
                    }),
                )
            })
    }
}
