//! Desktop Insert ribbon using the Windows Word grouping and JoyWord commands.
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
    WordArt(&'static str),
    Signature,
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
            Self::WordArt(_) => "insert.wordArt",
            Self::Signature => "insert.signatureLine",
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
            Tool::tile("表格", "Table", IconName::Table2).width(48.),
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
        let target = self.ribbon_target();
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
                let shapes = id == "insert-shapes";
                let symbols = id == "insert-symbol";
                let buttons = items.iter().zip(&available).enumerate().map(|(index, (item, enabled))| {
                    let (view, popup, target, action) = (view.clone(), popup.clone(), target.clone(), item.action.clone());
                    let symbol = symbols && matches!(&action, InsertAction::Command("insert.symbol", _));
                    let tile = shapes || symbol;
                    let content = div().flex().w_full().items_center().gap(px(5.))
                        .when(tile, |el| el.justify_center())
                        .when(symbol, |el| el.child(div().text_size(px(18.)).child(item.label.clone())))
                        .when(!symbol, |el| el.child(Icon::new(item.icon).size(px(if shapes { 21. } else { 16. }))))
                        .when(!tile, |el| el.child(div().text_size(px(12.)).child(item.label.clone())));
                    Button::new((id, index + 2)).ghost().small().w_full().p_1()
                        .when(tile, |button| button.w(px(36.)).h(px(34.)))
                        .accessibility_label(item.label.clone()).child(content)
                        .tooltip(if *enabled { item.label.clone() } else { disabled_tip.to_owned() })
                        .disabled(!enabled)
                        .on_click(move |_, w, cx| {
                            let _ = popup.update(cx, |p, cx| p.dismiss(w, cx));
                            let _ = view.update(cx, |v, cx| {
                                if v.ribbon_target() != target {
                                    v.shell.notice = Some(v.shell.tr("文档或选区已变化，请重新打开菜单。", "The document or selection changed. Reopen the menu.").into());
                                    cx.notify();
                                    return;
                                }
                                v.perform_insert(action.clone(), w, cx);
                            });
                        })
                });
                div().id((id, 1usize)).flex().max_h(px(380.))
                    .when(shapes || symbols, |body| body.w(px(120.)).flex_wrap().gap_1())
                    .when(!shapes && !symbols, |body| body.flex_col().w(px(226.)).overflow_y_scroll())
                    .children(buttons)
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
        let english = self.shell.locale != joyword_shell::l10n::Locale::ZhCn;
        let width = |zh, en| if english { en } else { zh };
        let stack = || div().flex().flex_col();
        let row = || div().flex().items_center().gap(px(2.));
        let pages = stack()
            .child(self.insert_menu(
                "insert-cover-page",
                Tool::compact("封面", "Cover Page", I::FileText).width(width(76., 102.)),
                vec![self.insert_entry(
                    "自定义封面…",
                    "Custom Cover Page…",
                    I::FileText,
                    Form(InsertForm::Cover),
                )],
                cx,
            ))
            .child(self.insert_direct(
                "insert-blank-page",
                Tool::compact("空白页", "Blank Page", I::FilePlus).width(width(76., 102.)),
                Command("insert.blankPage", json!({})),
                cx,
            ))
            .child(
                self.insert_direct(
                    "insert-page-break",
                    Tool::compact("分页", "Page Break", I::BetweenVerticalStart)
                        .width(width(76., 102.)),
                    Command("insert.pageBreak", json!({})),
                    cx,
                ),
            );
        let illustrations = row()
            .child(self.insert_menu(
                "insert-pictures",
                Tool::tile("图片", "Pictures", I::Image).width(52.),
                vec![
                    self.insert_entry("此设备…", "This Device…", I::Monitor, Picture),
                    self.insert_entry("库存图片…", "Stock Images…", I::Images, Unavailable),
                    self.insert_entry("联机图片…", "Online Pictures…", I::Globe, Unavailable),
                ],
                cx,
            ))
            .child(
                self.insert_menu(
                    "insert-shapes",
                    Tool::tile("形状", "Shapes", I::Shapes).width(48.),
                    [
                        ("矩形", "Rectangle", "rectangle", I::Square),
                        (
                            "圆角矩形",
                            "Rounded Rectangle",
                            "roundedRectangle",
                            I::RectangleHorizontal,
                        ),
                        ("椭圆", "Oval", "ellipse", I::Circle),
                        ("三角形", "Triangle", "triangle", I::Triangle),
                        ("直线", "Line", "line", I::Minus),
                        ("向右箭头", "Right Arrow", "arrow", I::ArrowRight),
                    ]
                    .into_iter()
                    .map(|(zh, en, kind, icon)| {
                        self.insert_entry(
                            zh,
                            en,
                            icon,
                            Command("insert.shape", json!({"kind":kind})),
                        )
                    })
                    .collect(),
                    cx,
                ),
            )
            .child(
                self.insert_menu(
                    "insert-icons",
                    Tool::tile("图标", "Icons", I::Sticker).width(44.),
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
                Tool::tile("3D 模型", "3D Models", I::Box).width(56.),
                Unavailable,
                cx,
            ))
            .child(self.insert_menu(
                "insert-smartart",
                Tool::tile("SmartArt", "SmartArt", I::Network).width(62.),
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
                Tool::tile("图表", "Chart", I::ChartColumn).width(46.),
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
                Tool::tile("屏幕截图", "Screenshot", I::Scan).width(width(57., 79.)),
                Unavailable,
                cx,
            ));
        let media = self.insert_direct(
            "insert-media",
            Tool::tile("联机视频", "Online Video", I::Video).width(width(55., 62.)),
            Unavailable,
            cx,
        );
        let links = stack().children(
            [
                (
                    "insert-link",
                    "链接",
                    "Link",
                    I::Link,
                    Form(InsertForm::Link),
                ),
                (
                    "insert-bookmark",
                    "书签",
                    "Bookmark",
                    I::Bookmark,
                    Advanced("refs-bookmark", "insert.bookmark"),
                ),
                (
                    "insert-cross-reference",
                    "交叉引用",
                    "Cross-reference",
                    I::Files,
                    Advanced("refs-crossref", "insert.crossReference"),
                ),
            ]
            .into_iter()
            .map(|(id, zh, en, icon, action)| {
                self.insert_direct(
                    id,
                    Tool::compact(zh, en, icon).width(width(76., 110.)),
                    action,
                    cx,
                )
            }),
        );
        let comments = self.insert_direct(
            "insert-comment",
            Tool::tile("批注", "Comment", I::MessageSquarePlus).width(width(48., 68.)),
            Advanced("review-new", "review.newComment"),
            cx,
        );
        let headers = row()
            .child(self.insert_menu(
                "insert-header",
                Tool::tile("页眉", "Header", I::PanelTop).width(50.),
                header,
                cx,
            ))
            .child(self.insert_menu(
                "insert-footer",
                Tool::tile("页脚", "Footer", I::PanelBottom).width(50.),
                footer,
                cx,
            ))
            .child(self.insert_menu(
                "insert-page-number",
                Tool::tile("页码", "Page Number", I::FileDigit),
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
            ));
        let text = row()
            .child(self.insert_menu(
                "insert-text-box",
                Tool::tile("文本框", "Text Box", I::TextCursorInput),
                vec![self.insert_entry(
                    "插入文本框…",
                    "Insert Text Box…",
                    I::TextCursorInput,
                    Form(InsertForm::TextBox),
                )],
                cx,
            ))
            .child(
                stack()
                    .child(
                        self.insert_menu(
                            "insert-quick-parts",
                            Tool::compact("文档部件", "Quick Parts", I::FileText)
                                .width(width(94., 106.)),
                            vec![self.insert_entry(
                                "域…",
                                "Field…",
                                I::FileText,
                                Form(InsertForm::Field),
                            )],
                            cx,
                        ),
                    )
                    .child(
                        self.insert_menu(
                            "insert-word-art",
                            Tool::compact("艺术字", "WordArt", I::TypeOutline)
                                .width(width(94., 106.)),
                            [
                                ("填充", "Solid", "solid"),
                                ("轮廓", "Outlined", "outlined"),
                                ("阴影", "Shadowed", "shadowed"),
                            ]
                            .into_iter()
                            .map(|(zh, en, style)| {
                                self.insert_entry(
                                    zh,
                                    en,
                                    I::TypeOutline,
                                    Form(InsertForm::WordArt(style)),
                                )
                            })
                            .collect(),
                            cx,
                        ),
                    )
                    .child(
                        self.insert_menu(
                            "insert-drop-cap",
                            Tool::compact("首字下沉", "Drop Cap", I::TextInitial)
                                .width(width(94., 106.)),
                            entries(
                                "insert.dropCap",
                                &[
                                    ("无", "None", "none"),
                                    ("下沉", "Dropped", "dropped"),
                                    ("悬挂", "In Margin", "margin"),
                                ],
                                "position",
                                I::TextInitial,
                            ),
                            cx,
                        ),
                    ),
            )
            .child(
                stack()
                    .child(
                        self.insert_direct(
                            "insert-signature",
                            Tool::compact("签名行", "Signature Line", I::Signature)
                                .width(width(90., 107.)),
                            Form(InsertForm::Signature),
                            cx,
                        ),
                    )
                    .child(
                        self.insert_direct(
                            "insert-date-time",
                            Tool::compact("日期和时间", "Date & Time", I::CalendarClock)
                                .width(width(90., 107.)),
                            Form(InsertForm::DateTime),
                            cx,
                        ),
                    )
                    .child(self.insert_direct(
                        "insert-object",
                        Tool::compact("对象", "Object", I::Box).width(width(90., 107.)),
                        Unavailable,
                        cx,
                    )),
            );
        let symbols = stack()
            .child(self.insert_menu(
                "insert-equations",
                Tool::compact("公式", "Equation", I::Radical).width(width(70., 96.)),
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
            .child(
                self.insert_menu(
                    "insert-symbol",
                    Tool::compact("符号", "Symbol", I::Omega).width(width(70., 96.)),
                    ["©", "®", "™", "°", "±", "€"]
                        .into_iter()
                        .map(|symbol| {
                            self.insert_entry(
                                symbol,
                                symbol,
                                I::Omega,
                                Command("insert.symbol", json!({"char":symbol})),
                            )
                        })
                        .chain([self.insert_entry(
                            "其他符号…",
                            "More Symbols…",
                            I::Omega,
                            Form(InsertForm::Symbol),
                        )])
                        .collect(),
                    cx,
                ),
            )
            .child(self.insert_direct(
                "insert-number",
                Tool::compact("编号", "Number", I::Hash).width(width(70., 96.)),
                Unavailable,
                cx,
            ));
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .child(self.ribbon_centered_group("insert-group-pages", tr("页面", "Pages"), pages))
            .child(self.ribbon_centered_group(
                "insert-group-tables",
                tr("表格", "Tables"),
                self.table_grid(cx),
            ))
            .child(self.ribbon_centered_group(
                "insert-group-illustrations",
                tr("插图", "Illustrations"),
                illustrations,
            ))
            .child(self.ribbon_centered_group("insert-group-media", tr("媒体", "Media"), media))
            .child(self.ribbon_centered_group("insert-group-links", tr("链接", "Links"), links))
            .child(self.ribbon_centered_group(
                "insert-group-comments",
                tr("批注", "Comments"),
                comments,
            ))
            .child(self.ribbon_centered_group(
                "insert-group-header-footer",
                tr("页眉和页脚", "Header & Footer"),
                headers,
            ))
            .child(self.ribbon_centered_group("insert-group-text", tr("文本", "Text"), text))
            .child(self.ribbon_centered_group(
                "insert-group-symbols",
                tr("符号", "Symbols"),
                symbols,
            ))
            .when(editing, |row| {
                row.child(self.ribbon_centered_group(
                    "insert-group-header-footer-editing",
                    tr("页眉页脚编辑", "Header & Footer Editing"),
                    self.header_footer_ribbon(cx),
                ))
            })
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
            InsertForm::WordArt(_) => (
                tr("插入艺术字", "Insert WordArt"),
                vec![(tr("文字", "Text"), String::new())],
            ),
            InsertForm::Signature => (
                tr("插入签名行", "Insert Signature Line"),
                vec![
                    (tr("签名人", "Signer"), String::new()),
                    (tr("职务", "Title"), String::new()),
                ],
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
                if (first.is_empty() && !matches!(form, InsertForm::Signature))
                    || first.len() > 16_384
                {
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
                    InsertForm::WordArt(style) => json!({"text":first,"style":style}),
                    InsertForm::Signature => json!({"signer":first,"title":second}),
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
