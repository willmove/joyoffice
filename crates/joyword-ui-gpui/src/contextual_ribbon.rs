//! Selection-dependent table and picture ribbons, using the common command widgets.
use super::ribbon::Tool;
use super::*;
use gpui::component::checkbox::Checkbox;
use joyoffice_ui::IconName as I;
use serde_json::json;

impl JoyWord {
    pub(super) fn picture_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let stack = || div().flex().flex_col();
        let corrections = self.format_choices(
            "picture-corrections-menu",
            self.ribbon_tool(
                "picture-corrections",
                Tool::new("校正", "Corrections", I::Sun).width(52.),
                self.format_ready("picture.corrections"),
                true,
            ),
            "picture.corrections",
            [
                ("正常", "Normal", 0., 0., 0.),
                ("明亮", "Bright", 20., 0., 0.),
                ("暗淡", "Dark", -20., 0., 0.),
                ("高对比度", "High Contrast", 0., 40., 0.),
                ("柔化", "Soften", 0., 0., -50.),
                ("锐化", "Sharpen", 0., 0., 50.),
            ]
            .into_iter()
            .map(|(zh, en, b, c, s)| {
                (
                    tr(zh, en).into(),
                    json!({"brightness":b,"contrast":c,"sharpen":s}),
                )
            })
            .collect(),
            cx,
        );
        let colors = self.format_choices(
            "picture-color-menu",
            self.ribbon_tool(
                "picture-color",
                Tool::compact("颜色", "Color", I::Palette).width(100.),
                self.format_ready("picture.color"),
                true,
            ),
            "picture.color",
            [
                ("原色", "Original", "saturation"),
                ("灰度", "Grayscale", "grayscale"),
                ("褐色", "Sepia", "sepia"),
                ("冲蚀", "Washout", "washout"),
                ("黑白", "Black and White", "blackAndWhite"),
            ]
            .into_iter()
            .map(|(zh, en, mode)| (tr(zh, en).into(), json!({"mode":mode,"saturation":100.})))
            .collect(),
            cx,
        );
        let effects = self.format_choices(
            "picture-effects-menu",
            self.ribbon_tool(
                "picture-effects",
                Tool::compact("艺术效果", "Artistic Effects", I::Paintbrush).width(100.),
                self.format_ready("picture.effects"),
                true,
            ),
            "picture.effects",
            [
                ("无", "None", "none"),
                ("模糊", "Blur", "blur"),
                ("锐化", "Sharpen", "sharpen"),
                ("反色", "Invert", "invert"),
                ("色调分离", "Posterize", "posterize"),
                ("像素化", "Pixelate", "pixelate"),
            ]
            .into_iter()
            .map(|(zh, en, effect)| (tr(zh, en).into(), json!({"effect":effect})))
            .collect(),
            cx,
        );
        let transparency = self.format_choices(
            "picture-transparency-menu",
            self.ribbon_tool(
                "picture-transparency",
                Tool::compact("透明度", "Transparency", I::Blend).width(100.),
                self.format_ready("picture.transparency"),
                true,
            ),
            "picture.transparency",
            [0, 15, 30, 50, 65, 80, 95]
                .into_iter()
                .map(|percent| (format!("{percent}%"), json!({"percent":percent})))
                .collect(),
            cx,
        );
        let compress = self.format_choices(
            "picture-compress-menu",
            self.ribbon_tool(
                "picture-compress",
                Tool::icon("压缩图片", "Compress Pictures", I::Minimize2).width(28.),
                self.format_ready("picture.compress"),
                false,
            ),
            "picture.compress",
            [1024, 1600, 2200]
                .into_iter()
                .map(|pixels| {
                    (
                        format!("{} {pixels} px", tr("最长边", "Longest side")),
                        json!({"maxPixels":pixels}),
                    )
                })
                .collect(),
            cx,
        );
        let adjust = div()
            .flex()
            .child(self.ribbon_command(
                "picture-remove-background",
                Tool::new("删除背景", "Remove Background", I::ImageOff).width(62.),
                "picture.removeBackground",
                json!({"tolerance":30.}),
                cx,
            ))
            .child(corrections)
            .child(stack().child(colors).child(effects).child(transparency))
            .child(
                stack()
                    .child(compress)
                    .child(
                        self.ribbon_tool(
                            "replace-picture",
                            Tool::icon("更改图片", "Change Picture", I::ImagePlus).width(28.),
                            self.format_ready("picture.change"),
                            true,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.pick_picture(true, w, cx))),
                    )
                    .child(self.ribbon_command(
                        "reset-picture",
                        Tool::icon("重置图片", "Reset Picture", I::RotateCcw).width(28.),
                        "picture.reset",
                        json!({}),
                        cx,
                    )),
            );
        let mut gallery = div().flex().gap(px(3.));
        for (style, zh, en, icon) in [
            ("simpleFrame", "简单框架", "Simple Frame", I::Image),
            ("thickFrame", "粗框架", "Thick Frame", I::Square),
            ("rounded", "圆角", "Rounded", I::RectangleHorizontal),
            ("softEdge", "柔化边缘", "Soft Edge", I::Blend),
            ("shadow", "阴影", "Shadow", I::Copy),
        ] {
            gallery = gallery.child(self.ribbon_command(
                match style {
                    "simpleFrame" => "picture-style-simple",
                    "thickFrame" => "picture-style-thick",
                    "rounded" => "picture-style-rounded",
                    "softEdge" => "picture-style-soft",
                    _ => "picture-style-shadow",
                },
                Tool::new(zh, en, icon).width(84.),
                "picture.style",
                json!({"style":style}),
                cx,
            ));
        }
        let border = self.format_choices(
            "picture-border-menu",
            self.ribbon_tool(
                "picture-border",
                Tool::compact("图片边框", "Picture Border", I::Square).width(106.),
                self.format_ready("picture.border"),
                true,
            ),
            "picture.border",
            [
                ("黑色", "Black", "000000"),
                ("蓝色", "Blue", "315FA8"),
                ("红色", "Red", "C00000"),
                ("白色", "White", "FFFFFF"),
            ]
            .into_iter()
            .map(|(zh, en, color)| (tr(zh, en).into(), json!({"color":color,"width":2})))
            .collect(),
            cx,
        );
        let picture_effects = self.format_choices(
            "picture-style-effects-menu",
            self.ribbon_tool(
                "picture-style-effects",
                Tool::compact("图片效果", "Picture Effects", I::Layers).width(106.),
                self.format_ready("picture.style"),
                true,
            ),
            "picture.style",
            [
                ("阴影", "Shadow", "shadow"),
                ("柔化边缘", "Soft Edge", "softEdge"),
                ("简单框架", "Simple Frame", "simpleFrame"),
            ]
            .into_iter()
            .map(|(zh, en, style)| (tr(zh, en).into(), json!({"style":style})))
            .collect(),
            cx,
        );
        let styles =
            div()
                .flex()
                .child(gallery)
                .child(
                    stack()
                        .child(border)
                        .child(picture_effects)
                        .child(self.ribbon_tool(
                            "picture-layout",
                            Tool::compact("图片版式", "Picture Layout", I::Network).width(106.),
                            false,
                            true,
                        )),
                );
        let alt = self
            .ribbon_tool(
                "picture-alt",
                Tool::new("替代文字", "Alt Text", I::Accessibility).width(64.),
                self.format_ready("object.altText"),
                false,
            )
            .on_click(cx.listener(|v, _, w, cx| v.picture_alt(w, cx)));
        let position = self.format_choices(
            "picture-position-menu",
            self.ribbon_tool(
                "picture-position",
                Tool::new("位置", "Position", I::PanelsTopLeft).width(48.),
                self.format_ready("arrange.position"),
                true,
            ),
            "arrange.position",
            [
                ("左上", "Top Left", "topLeft"),
                ("中上", "Top Center", "topCenter"),
                ("右上", "Top Right", "topRight"),
                ("左中", "Middle Left", "middleLeft"),
                ("正中", "Middle Center", "middleCenter"),
                ("右中", "Middle Right", "middleRight"),
                ("左下", "Bottom Left", "bottomLeft"),
                ("中下", "Bottom Center", "bottomCenter"),
                ("右下", "Bottom Right", "bottomRight"),
            ]
            .into_iter()
            .map(|(zh, en, preset)| (tr(zh, en).into(), json!({"preset":preset})))
            .collect(),
            cx,
        );
        let wrap = self.format_choices(
            "picture-wrap-menu",
            self.ribbon_tool(
                "picture-wrap",
                Tool::new("环绕文字", "Wrap Text", I::TextWrap).width(56.),
                self.format_ready("arrange.wrap"),
                true,
            ),
            "arrange.wrap",
            [
                ("嵌入型", "In Line with Text", "inline"),
                ("四周型", "Square", "square"),
                ("紧密型", "Tight", "tight"),
                ("穿越型", "Through", "through"),
                ("上下型", "Top and Bottom", "topAndBottom"),
                ("衬于文字下方", "Behind Text", "behindText"),
                ("浮于文字上方", "In Front of Text", "inFrontOfText"),
            ]
            .into_iter()
            .map(|(zh, en, wrap)| (tr(zh, en).into(), json!({"wrap":wrap})))
            .collect(),
            cx,
        );
        let rotate = self.format_choices(
            "picture-rotate-menu",
            self.ribbon_tool(
                "picture-rotate",
                Tool::compact("旋转", "Rotate", I::RotateCw).width(78.),
                self.format_ready("arrange.rotate"),
                true,
            ),
            "arrange.rotate",
            [
                ("向右旋转 90°", "Rotate Right 90°", "right"),
                ("向左旋转 90°", "Rotate Left 90°", "left"),
                ("水平翻转", "Flip Horizontal", "flipHorizontal"),
                ("垂直翻转", "Flip Vertical", "flipVertical"),
            ]
            .into_iter()
            .map(|(zh, en, direction)| (tr(zh, en).into(), json!({"direction":direction})))
            .collect(),
            cx,
        );
        let align = self.format_choices(
            "picture-align-menu",
            self.ribbon_tool(
                "picture-align",
                Tool::compact("对齐", "Align", I::TextAlignStart).width(78.),
                self.format_ready("arrange.align"),
                true,
            ),
            "arrange.align",
            [
                ("左对齐", "Align Left", "left"),
                ("居中", "Align Center", "center"),
                ("右对齐", "Align Right", "right"),
                ("顶端对齐", "Align Top", "top"),
                ("垂直居中", "Align Middle", "middle"),
                ("底端对齐", "Align Bottom", "bottom"),
            ]
            .into_iter()
            .map(|(zh, en, value)| (tr(zh, en).into(), json!({"value":value})))
            .collect(),
            cx,
        );
        let arrange = div()
            .flex()
            .child(position)
            .child(wrap)
            .child(
                stack()
                    .child(self.ribbon_tool(
                        "picture-forward",
                        Tool::compact("上移一层", "Bring Forward", I::BringToFront).width(92.),
                        false,
                        true,
                    ))
                    .child(self.ribbon_tool(
                        "picture-backward",
                        Tool::compact("下移一层", "Send Backward", I::SendToBack).width(92.),
                        false,
                        true,
                    ))
                    .child(
                        self.ribbon_tool(
                            "picture-selection-pane",
                            Tool::compact("选择窗格", "Selection Pane", I::MousePointer2)
                                .width(92.),
                            self.format_ready("arrange.selectionPane"),
                            false,
                        )
                        .on_click(cx.listener(|v, _, _, cx| v.open_advanced("对象", cx))),
                    ),
            )
            .child(
                stack()
                    .child(align)
                    .child(self.ribbon_tool(
                        "picture-group",
                        Tool::compact("组合", "Group", I::Group).width(78.),
                        false,
                        true,
                    ))
                    .child(rotate),
            );
        let size = div()
            .flex()
            .child(
                self.ribbon_tool(
                    "picture-crop",
                    Tool::new("裁剪", "Crop", I::Crop).width(48.),
                    self.format_ready("picture.crop"),
                    true,
                )
                .on_click(cx.listener(|v, _, w, cx| v.picture_crop_form(w, cx))),
            )
            .child(
                self.ribbon_tool(
                    "picture-size",
                    Tool::new("大小", "Size", I::Scaling).width(48.),
                    self.format_ready("picture.size"),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.picture_size(w, cx))),
            );
        div()
            .flex()
            .flex_shrink_0()
            .child(self.ribbon_centered_group("picture-adjust-group", tr("调整", "Adjust"), adjust))
            .child(self.ribbon_centered_group(
                "picture-style-group",
                tr("图片样式", "Picture Styles"),
                styles,
            ))
            .child(self.ribbon_centered_group(
                "picture-accessibility-group",
                tr("辅助功能", "Accessibility"),
                alt,
            ))
            .child(self.ribbon_centered_group(
                "picture-arrange-group",
                tr("排列", "Arrange"),
                arrange,
            ))
            .child(self.ribbon_centered_group("picture-size-group", tr("大小", "Size"), size))
    }
    pub(super) fn table_design_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let table = self.canvas.reader.as_ref().and_then(|r| r.table_info());
        let mut options = div().flex().justify_between().w(px(160.));
        for column in [
            [
                ("headerRow", "标题行", "Header Row"),
                ("totalRow", "汇总行", "Total Row"),
                ("bandedRows", "镶边行", "Banded Rows"),
            ],
            [
                ("firstColumn", "第一列", "First Column"),
                ("lastColumn", "最后一列", "Last Column"),
                ("bandedColumns", "镶边列", "Banded Columns"),
            ],
        ] {
            options = options.child(div().flex().flex_col().children(column.into_iter().map(
                |(key, zh, en)| {
                    let checked = table.as_ref().is_some_and(|t| t.props["look"][key] == true);
                    Checkbox::new(SharedString::from(format!("table-look-{key}")))
                        .label(tr(zh, en))
                        .xsmall()
                        .h(px(23.))
                        .text_size(px(12.))
                        .checked(checked)
                        .disabled(!self.format_ready("table.look"))
                        .on_click(cx.listener(move |v, on, w, cx| {
                            v.run_format("table.look", &json!({key:on}), w, cx)
                        }))
                },
            )));
        }
        let styles = self.style_catalog.iter().filter(|s| s.kind == "table");
        let mut gallery = div().flex().items_center().gap(px(3.));
        for style in styles.clone().take(7) {
            let id = style.id.clone();
            let resolved = self
                .canvas
                .reader
                .as_ref()
                .and_then(|r| r.engine().doc.styles.table_style(&id));
            let border = resolved
                .as_ref()
                .and_then(|s| s.parts.borders)
                .and_then(|b| b.bottom.or(b.top).or(b.left).or(b.right))
                .and_then(|b| b.color)
                .map(|c| (u32::from(c.0) << 16) | (u32::from(c.1) << 8) | u32::from(c.2));
            let preview = div()
                .flex()
                .flex_col()
                .w(px(60.))
                .h(px(40.))
                .children((0..4).map(|row| {
                    let fill = resolved
                        .as_ref()
                        .and_then(|s| {
                            if row == 0 {
                                s.parts.header_fill.or(s.parts.fill)
                            } else if row % 2 == 0 {
                                s.parts.band_fill.or(s.parts.fill)
                            } else {
                                s.parts.fill
                            }
                        })
                        .map(|c| (u32::from(c.0) << 16) | (u32::from(c.1) << 8) | u32::from(c.2))
                        .unwrap_or(0xffffff);
                    div().flex().h(px(10.)).children((0..3).map(move |_| {
                        div()
                            .w(px(20.))
                            .h(px(10.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(fill))
                            .when_some(border, |v, color| v.border_1().border_color(rgb(color)))
                            .child(div().w(px(10.)).h(px(1.)).bg(rgb(if fill < 0x808080 {
                                0xffffff
                            } else {
                                0x555555
                            })))
                    }))
                }));
            gallery = gallery.child(
                Button::new(SharedString::from(format!("table-style-{id}")))
                    .ghost()
                    .xsmall()
                    .w(px(68.))
                    .h(px(60.))
                    .p(px(3.))
                    .border_1()
                    .border_color(rgb(self.palette().border))
                    .tooltip(style.name.clone())
                    .accessibility_label(style.name.clone())
                    .disabled(!self.format_ready("table.style"))
                    .selected(table.as_ref().is_some_and(|t| t.props["style"] == id))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(preview)
                            .child(
                                div()
                                    .w(px(62.))
                                    .truncate()
                                    .text_size(px(10.))
                                    .child(style.name.clone()),
                            ),
                    )
                    .on_click(cx.listener(move |v, _, w, cx| {
                        v.run_format("table.style", &json!({"style":id}), w, cx)
                    })),
            );
        }
        gallery = gallery.child(
            self.format_choices(
                "table-style-all-menu",
                self.ribbon_tool(
                    "table-style-all",
                    Tool::new("更多", "More", I::ChevronDown).width(24.),
                    self.format_ready("table.style"),
                    false,
                ),
                "table.style",
                styles
                    .map(|s| (s.name.clone(), json!({"style":s.id})))
                    .collect(),
                cx,
            ),
        );
        let shading = self.format_choices(
            "table-shading-menu",
            self.ribbon_tool(
                "table-shading",
                Tool::new("底纹", "Shading", I::PaintBucket),
                self.format_ready("table.shading"),
                true,
            ),
            "table.shading",
            [
                (tr("无颜色", "No Color"), None),
                (tr("浅蓝", "Light Blue"), Some("DCE6F1")),
                (tr("浅黄", "Light Yellow"), Some("FFF2CC")),
                (tr("浅绿", "Light Green"), Some("E2F0D9")),
                (tr("深蓝", "Dark Blue"), Some("315FA8")),
            ]
            .into_iter()
            .map(|(label, color)| (label.into(), json!({"color":color})))
            .collect(),
            cx,
        );
        let borders = self.format_choices(
            "table-borders-menu",
            self.ribbon_tool(
                "table-borders",
                Tool::new("边框", "Borders", I::Grid2x2),
                self.format_ready("table.borders"),
                true,
            ),
            "table.borders",
            [
                ("所有框线", "All Borders", "all"),
                ("外侧框线", "Outside Borders", "outside"),
                ("内部框线", "Inside Borders", "inside"),
                ("无框线", "No Border", "none"),
                ("上框线", "Top Border", "top"),
                ("下框线", "Bottom Border", "bottom"),
                ("左框线", "Left Border", "left"),
                ("右框线", "Right Border", "right"),
            ]
            .into_iter()
            .map(|(zh, en, kind)| (tr(zh, en).into(), json!({"kind":kind})))
            .collect(),
            cx,
        );
        let border_settings = self
            .ribbon_tool(
                "table-border-settings",
                Tool::new("边框样式", "Border Styles", I::Square).width(68.),
                self.format_ready("table.borders"),
                true,
            )
            .on_click(cx.listener(|v, _, w, cx| v.table_border_form(w, cx)));
        let painter = self
            .ribbon_tool(
                "table-border-painter",
                Tool::new("边框刷", "Border Painter", I::Paintbrush),
                false,
                false,
            )
            .tooltip(tr(
                "逐边绘制边框尚未接入桌面",
                "Painting individual table borders is not available yet",
            ));
        div()
            .flex()
            .flex_shrink_0()
            .child(self.ribbon_centered_group(
                "table-style-options",
                tr("表格样式选项", "Table Style Options"),
                options,
            ))
            .child(self.ribbon_centered_group(
                "table-styles",
                tr("表格样式", "Table Styles"),
                div().flex().child(gallery).child(shading),
            ))
            .child(
                self.ribbon_centered_group(
                    "table-border-group",
                    tr("边框", "Borders"),
                    div()
                        .flex()
                        .child(border_settings)
                        .child(borders)
                        .child(painter),
                ),
            )
    }

    pub(super) fn table_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let stack = || div().flex().flex_col();
        let select = self.home_menu(
            "table-select-actions",
            self.ribbon_tool(
                "table-select",
                Tool::new("选择", "Select", I::MousePointer2).width(48.),
                self.format_ready("table.selectTable"),
                true,
            ),
            vec![
                (
                    "select-cell",
                    tr("选择单元格", "Select Cell"),
                    self.format_ready("table.selectCell"),
                    |v, w, cx| v.run_format("table.selectCell", &json!({}), w, cx),
                ),
                (
                    "select-row",
                    tr("选择行", "Select Row"),
                    self.format_ready("table.selectRow"),
                    |v, w, cx| v.run_format("table.selectRow", &json!({}), w, cx),
                ),
                (
                    "select-column",
                    tr("选择列", "Select Column"),
                    self.format_ready("table.selectColumn"),
                    |v, w, cx| v.run_format("table.selectColumn", &json!({}), w, cx),
                ),
                (
                    "select-table",
                    tr("选择表格", "Select Table"),
                    self.format_ready("table.selectTable"),
                    |v, w, cx| v.run_format("table.selectTable", &json!({}), w, cx),
                ),
            ],
            cx,
        );
        let table = div()
            .flex()
            .child(select)
            .child(
                self.ribbon_tool(
                    "table-gridlines",
                    Tool::new("查看网格线", "View Gridlines", I::Table),
                    false,
                    false,
                )
                .tooltip(tr(
                    "无边框单元格网格尚未接入屏幕叠加",
                    "Borderless cell grid overlays are not available yet",
                )),
            )
            .child(
                self.ribbon_tool(
                    "table-properties",
                    Tool::new("属性", "Properties", I::Settings2).width(48.),
                    self.format_ready("table.properties"),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.table_form(w, cx))),
            );
        let draw = div()
            .flex()
            .child(self.ribbon_tool(
                "table-draw",
                Tool::new("绘制表格", "Draw Table", I::PencilRuler).width(60.),
                false,
                false,
            ))
            .child(self.ribbon_tool(
                "table-eraser",
                Tool::new("橡皮擦", "Eraser", I::Eraser).width(52.),
                false,
                false,
            ));
        let delete = self.home_menu(
            "table-delete-menu",
            self.ribbon_tool(
                "table-delete",
                Tool::new("删除", "Delete", I::TableCellsMerge).width(48.),
                self.format_ready("table.deleteTable"),
                true,
            ),
            vec![
                (
                    "delete-row",
                    tr("删除行", "Delete Rows"),
                    self.format_ready("table.deleteRow"),
                    |v, w, cx| v.run_format("table.deleteRow", &json!({}), w, cx),
                ),
                (
                    "delete-column",
                    tr("删除列", "Delete Columns"),
                    self.format_ready("table.deleteColumn"),
                    |v, w, cx| v.run_format("table.deleteColumn", &json!({}), w, cx),
                ),
                (
                    "delete-table",
                    tr("删除表格", "Delete Table"),
                    self.format_ready("table.deleteTable"),
                    |v, w, cx| v.run_format("table.deleteTable", &json!({}), w, cx),
                ),
            ],
            cx,
        );
        let mut rows = div().flex().child(delete);
        for (id, zh, en, icon, command) in [
            (
                "row-above",
                "在上方插入行",
                "Insert Above",
                I::ArrowUpToLine,
                "table.insertRowAbove",
            ),
            (
                "row-below",
                "在下方插入行",
                "Insert Below",
                I::ArrowDownToLine,
                "table.insertRowBelow",
            ),
            (
                "column-left",
                "在左侧插入列",
                "Insert Left",
                I::ArrowLeftToLine,
                "table.insertColumnLeft",
            ),
            (
                "column-right",
                "在右侧插入列",
                "Insert Right",
                I::ArrowRightToLine,
                "table.insertColumnRight",
            ),
        ] {
            rows = rows.child(self.ribbon_command(
                id,
                Tool::new(zh, en, icon).width(52.),
                command,
                json!({}),
                cx,
            ));
        }
        let merge = div()
            .flex()
            .child(self.ribbon_command(
                "merge-cells",
                Tool::new("合并单元格", "Merge Cells", I::TableCellsMerge).width(58.),
                "table.merge",
                json!({}),
                cx,
            ))
            .child(
                self.ribbon_tool(
                    "split-cells",
                    Tool::new("拆分单元格", "Split Cells", I::TableCellsSplit).width(58.),
                    self.format_ready("table.split"),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.table_split_form(w, cx))),
            )
            .child(self.ribbon_command(
                "split-table",
                Tool::new("拆分表格", "Split Table", I::Rows2).width(58.),
                "table.splitTable",
                json!({}),
                cx,
            ));
        let autofit = self.format_choices(
            "table-autofit-menu",
            self.ribbon_tool(
                "table-autofit",
                Tool::new("自动调整", "AutoFit", I::BetweenHorizontalStart).width(64.),
                self.format_ready("table.autofit"),
                true,
            ),
            "table.autofit",
            [
                ("根据内容调整表格", "AutoFit Contents", "contents"),
                ("根据窗口调整表格", "AutoFit Window", "window"),
                ("固定列宽", "Fixed Column Width", "fixed"),
            ]
            .into_iter()
            .map(|(zh, en, mode)| (tr(zh, en).into(), json!({"mode":mode})))
            .collect(),
            cx,
        );
        let size = div()
            .flex()
            .child(autofit)
            .child(
                stack()
                    .child(
                        self.ribbon_tool(
                            "table-height",
                            Tool::compact("高度…", "Height…", I::ArrowUpDown).width(86.),
                            self.format_ready("table.rowHeight"),
                            false,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.table_dimension_form(false, w, cx))),
                    )
                    .child(
                        self.ribbon_tool(
                            "table-width",
                            Tool::compact("宽度…", "Width…", I::ArrowLeftRight).width(86.),
                            self.format_ready("table.columnWidth"),
                            false,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.table_dimension_form(true, w, cx))),
                    ),
            )
            .child(
                stack()
                    .child(self.ribbon_command(
                        "distribute-rows",
                        Tool::compact("分布行", "Distribute Rows", I::Rows3).width(84.),
                        "table.distributeRows",
                        json!({}),
                        cx,
                    ))
                    .child(self.ribbon_command(
                        "distribute-cols",
                        Tool::compact("分布列", "Distribute Columns", I::Columns3).width(84.),
                        "table.distributeColumns",
                        json!({}),
                        cx,
                    )),
            );
        let mut align = stack();
        for (row, values) in [
            ["topLeft", "topCenter", "topRight"],
            ["centerLeft", "center", "centerRight"],
            ["bottomLeft", "bottomCenter", "bottomRight"],
        ]
        .into_iter()
        .enumerate()
        {
            align = align.child(div().flex().children(values.into_iter().enumerate().map(
                |(col, value)| {
                    self.ribbon_command(
                        [
                            "cell-top-left",
                            "cell-top-center",
                            "cell-top-right",
                            "cell-center-left",
                            "cell-center",
                            "cell-center-right",
                            "cell-bottom-left",
                            "cell-bottom-center",
                            "cell-bottom-right",
                        ][row * 3 + col],
                        Tool::icon(
                            [
                                "左上对齐",
                                "顶端居中",
                                "右上对齐",
                                "左中对齐",
                                "居中",
                                "右中对齐",
                                "左下对齐",
                                "底端居中",
                                "右下对齐",
                            ][row * 3 + col],
                            value,
                            [I::TextAlignStart, I::TextAlignCenter, I::TextAlignEnd][col],
                        )
                        .width(23.),
                        "table.cellAlign",
                        json!({"value":value}),
                        cx,
                    )
                },
            )));
        }
        let alignment = div()
            .flex()
            .child(align)
            .child(self.ribbon_command(
                "table-text-direction",
                Tool::new("文字方向", "Text Direction", I::TextCursorInput).width(56.),
                "table.textDirection",
                json!({}),
                cx,
            ))
            .child(
                self.ribbon_tool(
                    "cell-margins",
                    Tool::new("单元格边距", "Cell Margins", I::Table).width(58.),
                    self.format_ready("table.cellMargins"),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.cell_margins_form(w, cx))),
            );
        let data = div()
            .flex()
            .child(
                self.ribbon_tool(
                    "table-sort",
                    Tool::new("排序", "Sort", I::ArrowDownAZ).width(48.),
                    self.format_ready("table.sort"),
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.table_sort_form(w, cx))),
            )
            .child(
                stack()
                    .child(self.ribbon_command(
                        "table-repeat-header",
                        Tool::compact("重复标题行", "Repeat Header Rows", I::Table).width(116.),
                        "table.repeatHeader",
                        json!({}),
                        cx,
                    ))
                    .child(self.ribbon_command(
                        "table-to-text",
                        Tool::compact("转换为文本", "Convert to Text", I::Type).width(116.),
                        "table.toText",
                        json!({"separator":"tab"}),
                        cx,
                    ))
                    .child(
                        self.ribbon_tool(
                            "table-formula",
                            Tool::compact("公式", "Formula", I::Sigma).width(116.),
                            self.format_ready("table.formula"),
                            false,
                        )
                        .on_click(cx.listener(|v, _, w, cx| v.table_formula_form(w, cx))),
                    ),
            );
        div()
            .flex()
            .flex_shrink_0()
            .child(self.ribbon_centered_group("table-layout-table", tr("表", "Table"), table))
            .child(self.ribbon_centered_group("table-layout-draw", tr("绘图", "Draw"), draw))
            .child(self.ribbon_centered_group(
                "table-layout-rows",
                tr("行和列", "Rows & Columns"),
                rows,
            ))
            .child(self.ribbon_centered_group("table-layout-merge", tr("合并", "Merge"), merge))
            .child(self.ribbon_centered_group(
                "table-layout-size",
                tr("单元格大小", "Cell Size"),
                size,
            ))
            .child(self.ribbon_centered_group(
                "table-layout-alignment",
                tr("对齐方式", "Alignment"),
                alignment,
            ))
            .child(self.ribbon_centered_group("table-layout-data", tr("数据", "Data"), data))
    }

    fn table_split_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form("拆分单元格 / Split Cells","table.split",vec![("列数 / Columns","2".into()),("行数 / Rows","1".into())],false,|s|Ok(json!({"columns":joyword_shell::authoring::integer(&s[0],1,16)?,"rows":joyword_shell::authoring::integer(&s[1],1,32)?,"mergeFirst":true})),w,cx);
    }
    fn table_dimension_form(&mut self, width: bool, w: &mut Window, cx: &mut Context<Self>) {
        let (command, key, label) = if width {
            (
                "table.columnWidth",
                "width",
                "列宽（磅） / Column width (pt)",
            )
        } else {
            ("table.rowHeight", "height", "行高（磅） / Row height (pt)")
        };
        self.fields_form(
            "单元格大小 / Cell Size",
            command,
            vec![(label, String::new())],
            false,
            move |v| Ok(json!({key:joyword_shell::authoring::required_number(&v[0],if width{6.}else{1.},1584.)?})),
            w,
            cx,
        );
    }
    fn cell_margins_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form(
            "单元格边距 / Cell Margins",
            "table.cellMargins",
            vec![
                ("上（磅） / Top (pt)", String::new()),
                ("左（磅） / Left (pt)", String::new()),
                ("下（磅） / Bottom (pt)", String::new()),
                ("右（磅） / Right (pt)", String::new()),
            ],
            true,
            |v| {
                let mut patch = json!({});
                for (value, key) in v.iter().zip(["top", "left", "bottom", "right"]) {
                    if !value.trim().is_empty() {
                        patch[key] =
                            json!(joyword_shell::authoring::required_number(value, 0., 144.)?);
                    }
                }
                Ok(patch)
            },
            w,
            cx,
        );
    }
    fn table_sort_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form("表格排序 / Sort Table","table.sort",vec![("列（从 1 开始） / Column (1-based)","1".into()),("方向 / Order (ascending / descending)","ascending".into()),("标题行 / Header row (yes / no)","yes".into())],false,|v|{
            if !["ascending","descending"].contains(&v[1].trim())||!["yes","no"].contains(&v[2].trim()){return Err("请输入 ascending / descending 和 yes / no".into());}
            Ok(json!({"column":joyword_shell::authoring::integer(&v[0],1,63)?-1,"descending":v[1].trim()=="descending","header":v[2].trim()=="yes"}))
        },w,cx);
    }
    fn table_formula_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form(
            "表格公式 / Table Formula",
            "table.formula",
            vec![("公式 / Formula", "=SUM(ABOVE)".into())],
            false,
            |v| {
                if v[0].len() > 1024 || !v[0].trim().starts_with('=') {
                    return Err("请输入以 = 开头的公式 / A formula must start with =".into());
                }
                Ok(json!({"formula":v[0].trim()}))
            },
            w,
            cx,
        );
    }
    fn table_border_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form("表格边框 / Table Borders","table.borders",vec![("范围 / Kind (all / outside / inside / top / bottom / left / right / none)","all".into()),("宽度（磅） / Width (pt)","0.5".into()),("颜色 / Color (RRGGBB)","315FA8".into())],false,|v|{
            if !["all","outside","inside","top","bottom","left","right","none"].contains(&v[0].trim()){return Err("无效边框范围 / Invalid border kind".into());}
            let c=v[2].trim().trim_start_matches('#');
            if c.len()!=6||!c.bytes().all(|b|b.is_ascii_hexdigit()){return Err("颜色需要 6 位 RGB / A six-digit RGB color is required".into());}
            Ok(json!({"kind":v[0].trim(),"width":joyword_shell::authoring::required_number(&v[1],0.25,6.)?,"color":c}))
        },w,cx);
    }
}
