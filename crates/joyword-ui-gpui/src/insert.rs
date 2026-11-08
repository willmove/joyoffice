//! Insert ribbon, grounded in the native Word for Mac layout. All enabled entries
//! invoke the existing Session or version-bound forms; no imitation success paths.
use super::ribbon::Tool;
use super::*;
use gpui::component::{Icon, popover::Popover};
use joyoffice_ui::IconName;
use serde_json::{Value, json};

#[derive(Clone, Copy)]
enum InsertForm {
    Cover,
    Link,
    TextBox,
    DateTime,
    Field,
    Symbol,
    Equation,
}

impl InsertForm {
    fn command(self) -> &'static str {
        match self {
            Self::Cover => "insert.coverPage",
            Self::Link => "insert.link",
            Self::TextBox => "insert.textBox",
            Self::DateTime => "insert.dateTime",
            Self::Field => "insert.field",
            Self::Symbol => "insert.symbol",
            Self::Equation => "insert.equation",
        }
    }
}

#[derive(Clone)]
enum InsertAction {
    Command(&'static str, Value),
    Form(InsertForm),
    Advanced(&'static str, &'static str),
    Picture,
    Unavailable,
}

impl InsertAction {
    fn command(&self) -> Option<&'static str> {
        match self {
            Self::Command(id, _) | Self::Advanced(_, id) => Some(id),
            Self::Form(form) => Some(form.command()),
            Self::Picture => Some("insert.picture"),
            Self::Unavailable => None,
        }
    }
}

struct Entry {
    label: String,
    icon: IconName,
    action: InsertAction,
}

impl JoyWord {
    pub(super) fn insert_table_trigger(&self) -> Button {
        self.ribbon_tool(
            "insert-table-grid",
            Tool::new("表格", "Table", IconName::Table2),
            self.format_ready("insert.table"),
            true,
        )
    }

    fn insert_entry(
        &self,
        zh: &'static str,
        en: &'static str,
        icon: IconName,
        action: InsertAction,
    ) -> Entry {
        Entry {
            label: self.shell.tr(zh, en).to_owned(),
            icon,
            action,
        }
    }

    fn insert_enabled(&self, action: &InsertAction) -> bool {
        action
            .command()
            .is_some_and(|command| self.format_ready(command))
    }

    fn insert_menu(
        &self,
        id: &'static str,
        tool: Tool,
        items: Vec<Entry>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = cx.entity().downgrade();
        // Selection and presentation changes matter even when the document's
        // revision did not change (navigation, header/footer and composition).
        let target = self.canvas.reader.as_ref().map(|r| {
            (
                r.engine().document_id(),
                r.engine().rev(),
                r.presentation_revision(),
                r.engine().sel.clone(),
            )
        });
        let available: Vec<bool> = items
            .iter()
            .map(|item| self.insert_enabled(&item.action))
            .collect();
        let enabled = available.iter().any(|value| *value);
        let disabled_tip = self.shell.tr(
            "此功能尚未提供，或当前文档状态不可用",
            "Not available in this document or build",
        );
        Popover::new(id)
            .trigger(self.ribbon_tool((id, 0usize), tool, enabled, true))
            .content(move |_, _, cx| {
                let popup = cx.entity().downgrade();
                let tiles = matches!(id, "insert-pages" | "insert-links");
                let text_palette = id == "insert-text";
                let mut buttons: Vec<gpui::AnyElement> = items.iter().zip(&available).enumerate().map(|(index, (item, enabled))| {
                    let (view, popup, target, action) = (view.clone(), popup.clone(), target.clone(), item.action.clone());
                    let tile = tiles || (text_palette && index < 3);
                    let content = div().flex().w_full().items_center().gap(px(5.))
                        .when(tile, |el| el.flex_col().justify_center())
                        .child(Icon::new(item.icon).size(px(if tile { 26. } else { 16. })))
                        .child(div().text_size(px(if tile { 11. } else { 12. }))
                            .whitespace_normal()
                            .when(tile, |el| el.text_center().max_w(px(56.))).child(item.label.clone()));
                    Button::new((id, index + 2)).ghost().small().w_full().p_1()
                        .when(tile, |button| button.w(px(64.)).h(px(64.)))
                        .accessibility_label(item.label.clone()).child(content)
                        .tooltip(if *enabled { item.label.clone() } else { disabled_tip.to_owned() })
                        .disabled(!enabled)
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = view.update(cx, |v, cx| {
                                if v.canvas.reader.as_ref().map(|r| (r.engine().document_id(), r.engine().rev(), r.presentation_revision(), r.engine().sel.clone())) != target {
                                    v.shell.notice = Some(v.shell.tr("文档或选区已变化，请重新打开菜单。", "The document or selection changed. Reopen the menu.").into());
                                    cx.notify();
                                    return;
                                }
                                v.perform_insert(action.clone(), w, cx);
                            });
                        }).into_any_element()
                }).collect();
                let body = div().id((id, 1usize)).flex().max_h(px(380.));
                if tiles {
                    body.children(buttons)
                } else if text_palette {
                    body.children(buttons.drain(..3)).child(div().flex().flex_col().w(px(142.)).children(buttons))
                } else {
                    body.flex_col().w(px(226.)).overflow_y_scroll().children(buttons)
                }

            })
    }

    fn insert_direct(
        &self,
        id: &'static str,
        tool: Tool,
        action: InsertAction,
        cx: &mut Context<Self>,
    ) -> Button {
        self.ribbon_tool(id, tool, self.insert_enabled(&action), false)
            .on_click(cx.listener(move |v, _, w, cx| v.perform_insert(action.clone(), w, cx)))
    }

    fn insert_group(
        &self,
        label: &'static str,
        body: impl IntoElement,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("insert-group-{label}")))
            .flex()
            .flex_shrink_0()
            .h(px(66.))
            .px(px(7.))
            .items_center()
            .gap(px(1.))
            .border_r_1()
            .border_color(rgb(self.palette().border))
            .role(gpui::Role::Group)
            .aria_label(label)
            .child(body)
    }

    pub(super) fn insert_ribbon(&self, cx: &mut Context<Self>) -> gpui::Div {
        use IconName as I;
        use InsertAction::{Advanced, Command, Form, Picture, Unavailable};
        let tr = |zh, en| self.shell.tr(zh, en);
        let entries = |command: &'static str,
                       rows: &[(&'static str, &'static str, &'static str)],
                       key: &str,
                       icon| {
            rows.iter()
                .map(|(zh, en, value)| {
                    self.insert_entry(zh, en, icon, Command(command, json!({key:*value})))
                })
                .collect()
        };
        let (first, even) = self
            .canvas
            .reader
            .as_ref()
            .map_or((false, false), |r| r.header_footer_flags());
        let editing = self
            .canvas
            .reader
            .as_ref()
            .is_some_and(|r| r.engine().in_header_footer());
        let header = vec![
            self.insert_entry(
                "空白",
                "Blank",
                I::PanelTop,
                Command("insert.header", json!({"preset":"blank"})),
            ),
            self.insert_entry(
                "空白（三栏）",
                "Blank (Three Columns)",
                I::PanelTop,
                Command("insert.header", json!({"preset":"blankThree"})),
            ),
            self.insert_entry(
                "标题",
                "Title",
                I::FileText,
                Command("insert.header", json!({"preset":"title"})),
            ),
            self.insert_entry(
                "编辑页眉",
                "Edit Header",
                I::PanelTop,
                Command("insert.editHeader", json!({})),
            ),
            self.insert_entry(
                "首页不同",
                "Different First Page",
                I::File,
                Command("layout.differentFirstPage", json!({"value":!first})),
            ),
            self.insert_entry(
                "奇偶页不同",
                "Different Odd & Even Pages",
                I::Files,
                Command("layout.differentOddEven", json!({"value":!even})),
            ),
            self.insert_entry(
                "删除页眉",
                "Remove Header",
                I::Trash,
                Command("insert.removeHeader", json!({})),
            ),
        ];
        let mut footer = vec![
            self.insert_entry(
                "空白",
                "Blank",
                I::PanelBottom,
                Command("insert.footer", json!({"preset":"blank"})),
            ),
            self.insert_entry(
                "页码",
                "Page Number",
                I::Hash,
                Command("insert.footer", json!({"preset":"pageNumber"})),
            ),
            self.insert_entry(
                "编辑页脚",
                "Edit Footer",
                I::PanelBottom,
                Command("insert.editFooter", json!({})),
            ),
            self.insert_entry(
                "删除页脚",
                "Remove Footer",
                I::Trash,
                Command("insert.removeFooter", json!({})),
            ),
        ];
        if editing {
            footer.push(self.insert_entry(
                "关闭页眉页脚",
                "Close Header and Footer",
                I::X,
                Command("insert.closeHeader", json!({})),
            ));
        }
        let mut row = div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .py(px(4.))
            .child(self.insert_group(
                tr("页面", "Pages"),
                self.insert_menu(
                    "insert-pages",
                    Tool::new("页面", "Pages", I::FileText),
                    vec![
                        self.insert_entry(
                            "封面…",
                            "Cover Page…",
                            I::FileText,
                            Form(InsertForm::Cover),
                        ),
                        self.insert_entry(
                            "空白页",
                            "Blank Page",
                            I::FilePlus,
                            Command("insert.blankPage", json!({})),
                        ),
                        self.insert_entry(
                            "分页符",
                            "Page Break",
                            I::BetweenVerticalStart,
                            Command("insert.pageBreak", json!({})),
                        ),
                    ],
                    cx,
                ),
            ))
            .child(self.insert_group(tr("表格", "Tables"), self.table_grid(cx)));
        let illustrations = div()
            .flex()
            .items_center()
            .gap(px(1.))
            .child(self.insert_menu(
                "insert-pictures",
                Tool::new("图片", "Pictures", I::Image),
                vec![self.insert_entry("来自文件的图片…", "Picture from File…", I::Image, Picture)],
                cx,
            ))
            .child(self.insert_menu(
                "insert-shapes",
                Tool::new("形状", "Shapes", I::Shapes),
                entries(
                    "insert.shape",
                    &[
                        ("矩形", "Rectangle", "rectangle"),
                        ("圆角矩形", "Rounded Rectangle", "roundedRectangle"),
                        ("椭圆", "Oval", "ellipse"),
                        ("三角形", "Triangle", "triangle"),
                        ("直线", "Line", "line"),
                        ("向右箭头", "Right Arrow", "arrow"),
                    ],
                    "kind",
                    I::Shapes,
                ),
                cx,
            ))
            .child(
                self.insert_menu(
                    "insert-icons",
                    Tool::new("图标", "Icons", I::Sticker),
                    joyword_render::icon_lib::search("", None)
                        .iter()
                        .map(|icon| Entry {
                            label: icon.name.to_owned(),
                            icon: I::Sticker,
                            action: Command("insert.icon", json!({"id":icon.id})),
                        })
                        .collect(),
                    cx,
                ),
            )
            .child(self.insert_direct(
                "insert-3d",
                Tool::new("3D 模型", "3D Models", I::Box),
                Unavailable,
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(self.insert_menu(
                        "insert-smartart",
                        Tool::compact("SmartArt", "SmartArt", I::Network),
                        entries(
                            "insert.smartArt",
                            &[
                                ("基本列表", "Basic List", "basicList"),
                                ("流程", "Process", "process"),
                                ("循环", "Cycle", "cycle"),
                                ("层次结构", "Hierarchy", "hierarchy"),
                                ("棱锥图", "Pyramid", "pyramid"),
                                ("关系", "Radial", "radial"),
                                ("矩阵", "Matrix", "matrix"),
                                ("维恩图", "Venn", "venn"),
                            ],
                            "layout",
                            I::Network,
                        ),
                        cx,
                    ))
                    .child(self.insert_menu(
                        "insert-chart",
                        Tool::compact("图表", "Chart", I::ChartColumn),
                        entries(
                            "insert.chart",
                            &[
                                ("柱形图", "Column", "column"),
                                ("条形图", "Bar", "bar"),
                                ("折线图", "Line", "line"),
                                ("面积图", "Area", "area"),
                                ("饼图", "Pie", "pie"),
                                ("圆环图", "Doughnut", "doughnut"),
                                ("散点图", "Scatter", "scatter"),
                            ],
                            "type",
                            I::ChartColumn,
                        ),
                        cx,
                    ))
                    .child(self.insert_direct(
                        "insert-screenshot",
                        Tool::compact("屏幕截图", "Screenshot", I::Scan),
                        Unavailable,
                        cx,
                    )),
            );
        row = row
            .child(self.insert_group(tr("插图", "Illustrations"), illustrations))
            .child(self.insert_group(
                tr("媒体", "Media"),
                self.insert_direct(
                    "insert-media",
                    Tool::new("媒体", "Media", I::Video),
                    Unavailable,
                    cx,
                ),
            ))
            .child(self.insert_group(
                tr("链接", "Links"),
                self.insert_menu(
                    "insert-links",
                    Tool::new("链接", "Links", I::Link),
                    vec![
                        self.insert_entry("链接…", "Link…", I::Link, Form(InsertForm::Link)),
                        self.insert_entry(
                            "书签…",
                            "Bookmark…",
                            I::Bookmark,
                            Advanced("refs-bookmark", "insert.bookmark"),
                        ),
                        self.insert_entry(
                            "交叉引用…",
                            "Cross-reference…",
                            I::FileText,
                            Advanced("refs-crossref", "insert.crossReference"),
                        ),
                    ],
                    cx,
                ),
            ))
            .child(self.insert_group(
                tr("批注", "Comments"),
                self.insert_direct(
                    "insert-comment",
                    Tool::new("批注", "Comment", I::MessageSquarePlus),
                    Advanced("review-new", "review.newComment"),
                    cx,
                ),
            ))
            .child(
                self.insert_group(
                    tr("页眉和页脚", "Header & Footer"),
                    div()
                        .flex()
                        .gap(px(1.))
                        .child(self.insert_menu(
                            "insert-header",
                            Tool::new("页眉", "Header", I::PanelTop),
                            header,
                            cx,
                        ))
                        .child(self.insert_menu(
                            "insert-footer",
                            Tool::new("页脚", "Footer", I::PanelBottom),
                            footer,
                            cx,
                        ))
                        .child(self.insert_menu(
                            "insert-page-number",
                            Tool::new("页码", "Page Number", I::FileDigit),
                            vec![
                                self.insert_entry(
                                    "页面顶端",
                                    "Top of Page",
                                    I::PanelTop,
                                    Command(
                                        "insert.pageNumber",
                                        json!({"position":"top","align":"right"}),
                                    ),
                                ),
                                self.insert_entry(
                                    "页面底端",
                                    "Bottom of Page",
                                    I::PanelBottom,
                                    Command(
                                        "insert.pageNumber",
                                        json!({"position":"bottom","align":"center"}),
                                    ),
                                ),
                                self.insert_entry(
                                    "当前位置",
                                    "Current Position",
                                    I::Hash,
                                    Command("insert.pageNumber", json!({"position":"current"})),
                                ),
                            ],
                            cx,
                        )),
                ),
            )
            .child(self.insert_group(
                tr("文本", "Text"),
                self.insert_menu(
                    "insert-text",
                    Tool::new("文本", "Text", I::Type),
                    vec![
                        self.insert_entry(
                            "文本框…",
                            "Text Box…",
                            I::TextCursorInput,
                            Form(InsertForm::TextBox),
                        ),
                        self.insert_entry("艺术字", "WordArt", I::Type, Unavailable),
                        self.insert_entry(
                            "首字下沉",
                            "Drop Cap",
                            I::TextCursorInput,
                            Command("insert.dropCap", json!({"position":"dropped"})),
                        ),
                        self.insert_entry("域…", "Field…", I::FileText, Form(InsertForm::Field)),
                        self.insert_entry(
                            "日期和时间…",
                            "Date & Time…",
                            I::CalendarClock,
                            Form(InsertForm::DateTime),
                        ),
                        self.insert_entry("对象", "Object", I::Box, Unavailable),
                    ],
                    cx,
                ),
            ))
            .child(
                self.insert_group(
                    tr("符号", "Symbols"),
                    div()
                        .flex()
                        .gap(px(1.))
                        .child(self.insert_menu(
                            "insert-equations",
                            Tool::new("公式", "Equation", I::Radical),
                            vec![
                                self.insert_entry(
                                    "输入公式…",
                                    "Insert Equation…",
                                    I::Radical,
                                    Form(InsertForm::Equation),
                                ),
                                self.insert_entry(
                                    "勾股定理",
                                    "Pythagorean Theorem",
                                    I::Radical,
                                    Command(
                                        "insert.equation",
                                        json!({"builtin":"pythagoras","edit":false}),
                                    ),
                                ),
                                self.insert_entry(
                                    "一元二次方程",
                                    "Quadratic Formula",
                                    I::Radical,
                                    Command(
                                        "insert.equation",
                                        json!({"builtin":"quadratic","edit":false}),
                                    ),
                                ),
                            ],
                            cx,
                        ))
                        .child(self.insert_direct(
                            "insert-symbol",
                            Tool::new("高级符号", "Symbol", I::Omega),
                            Form(InsertForm::Symbol),
                            cx,
                        ))
                        .child(self.insert_direct(
                            "insert-number",
                            Tool::new("数字", "Number", I::Hash),
                            Unavailable,
                            cx,
                        )),
                ),
            );
        row.when(editing, |row| row.child(self.header_footer_ribbon(cx)))
    }

    fn perform_insert(&mut self, action: InsertAction, w: &mut Window, cx: &mut Context<Self>) {
        if !self.insert_enabled(&action) {
            return;
        }
        match action {
            InsertAction::Command(command, mut args) => {
                if matches!(command, "insert.editHeader" | "insert.editFooter") {
                    let page = self.canvas.reader.as_ref().map(|r| r.engine().page_hint);
                    if let Some(page) = page {
                        args["page"] = json!(page);
                    }
                }
                self.run_format(command, &args, w, cx);
            }
            InsertAction::Picture => self.pick_picture(false, w, cx),
            InsertAction::Advanced(id, _) => self.advanced_action(id, w, cx),
            InsertAction::Form(form) => self.insert_form(form, w, cx),
            InsertAction::Unavailable => {}
        }
    }

    fn insert_form(&mut self, form: InsertForm, w: &mut Window, cx: &mut Context<Self>) {
        let tr = |zh, en| self.shell.tr(zh, en);
        let (title, fields) = match form {
            InsertForm::Cover => (
                tr("插入封面", "Insert Cover Page"),
                vec![
                    (tr("标题", "Title"), String::new()),
                    (tr("副标题", "Subtitle"), String::new()),
                ],
            ),
            InsertForm::Link => (
                tr("插入链接", "Insert Link"),
                vec![
                    (tr("地址", "Address"), "https://".into()),
                    (
                        tr("显示文字（留空使用地址）", "Text to display (optional)"),
                        String::new(),
                    ),
                ],
            ),
            InsertForm::TextBox => (
                tr("插入文本框", "Insert Text Box"),
                vec![(tr("文字", "Text"), String::new())],
            ),
            InsertForm::DateTime => (
                tr("日期和时间", "Date & Time"),
                vec![
                    (
                        tr("格式（例如 yyyy-MM-dd）", "Format (e.g. yyyy-MM-dd)"),
                        "yyyy-MM-dd".into(),
                    ),
                    (
                        tr("自动更新（是 / 否）", "Update automatically (true / false)"),
                        "false".into(),
                    ),
                ],
            ),
            InsertForm::Field => (
                tr("插入域", "Insert Field"),
                vec![(
                    tr(
                        "域代码（例如 PAGE、DATE、AUTHOR）",
                        "Field code (e.g. PAGE, DATE, AUTHOR)",
                    ),
                    "PAGE".into(),
                )],
            ),
            InsertForm::Symbol => (
                tr("高级符号", "Symbol"),
                vec![
                    (
                        tr("符号或特殊字符", "Symbol or special character"),
                        "©".into(),
                    ),
                    (tr("字体（可选）", "Font (optional)"), String::new()),
                ],
            ),
            InsertForm::Equation => (
                tr("插入公式", "Insert Equation"),
                vec![(tr("LaTeX 公式", "LaTeX equation"), "x^2 + y^2 = z^2".into())],
            ),
        };
        self.fields_form(
            title,
            form.command(),
            fields,
            false,
            move |values| {
                let first = values.first().map(String::as_str).unwrap_or("").trim();
                if first.is_empty() || first.len() > 16_384 {
                    return Err(
                        "请填写有效内容（最多 16384 字节） / Enter valid content (max 16384 bytes)"
                            .into(),
                    );
                }
                let second = values.get(1).map(String::as_str).unwrap_or("").trim();
                Ok(match form {
                    InsertForm::Cover => json!({"title":first,"subtitle":second}),
                    InsertForm::Link => {
                        if second.is_empty() {
                            json!({"url":first})
                        } else {
                            json!({"url":first,"text":second})
                        }
                    }
                    InsertForm::TextBox => json!({"text":first}),
                    InsertForm::DateTime => {
                        let update = match second {
                            "是" | "true" => true,
                            "否" | "false" => false,
                            _ => return Err("请输入 是 / 否 或 true / false".into()),
                        };
                        json!({"format":first,"update":update})
                    }
                    InsertForm::Field => json!({"instr":first}),
                    InsertForm::Symbol => {
                        if second.is_empty() {
                            json!({"char":first})
                        } else {
                            json!({"char":first,"font":second})
                        }
                    }
                    InsertForm::Equation => json!({"latex":first,"edit":false}),
                })
            },
            w,
            cx,
        );
    }
}
