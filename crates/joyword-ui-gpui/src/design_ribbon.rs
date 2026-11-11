//! Document formatting and page background over the existing engine commands.
use super::ribbon::Tool;
use super::*;
use joyoffice_ui::IconName as I;
use joyword_shell::authoring::DOCUMENT_THEMES;
use serde_json::json;

fn color(text: &str) -> Result<String, String> {
    let text = text.trim().trim_start_matches('#');
    if text.len() == 6 && text.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(text.to_ascii_uppercase())
    } else {
        Err("请输入 6 位 RGB 颜色，例如 315FA8 / Enter a six-digit RGB color".into())
    }
}

impl JoyWord {
    pub(super) fn design_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let themes = |key: &str| {
            DOCUMENT_THEMES
                .iter()
                .map(|(name, _, _, _)| (name.to_string(), json!({key:*name})))
                .collect()
        };
        let theme = self.format_choices(
            "design-themes-menu",
            self.ribbon_tool(
                "design-themes",
                Tool::new("主题", "Themes", I::Palette),
                self.format_ready("design.theme"),
                true,
            ),
            "design.theme",
            themes("name"),
            cx,
        );
        let mut gallery = div().flex().items_center().gap(px(2.));
        for (name, zh, en, accent, italic) in [
            ("default", "默认", "Default", 0x156082, false),
            ("basic", "基本", "Basic", 0x262626, false),
            ("lines", "线条", "Lines", 0x156082, false),
            ("shaded", "阴影", "Shaded", 0x156082, false),
            ("casual", "休闲", "Casual", 0x156082, true),
            ("centered", "居中", "Centered", 0x156082, false),
            ("minimalist", "简约", "Minimalist", 0x404040, false),
            ("title", "标题", "Title", 0x156082, false),
        ] {
            let label = tr(zh, en);
            gallery = gallery.child(
                Button::new(SharedString::from(format!("design-style-{name}")))
                    .ghost()
                    .xsmall()
                    .w(px(104.))
                    .h(px(64.))
                    .p(px(4.))
                    .border_1()
                    .border_color(rgb(self.palette().border))
                    .accessibility_label(label)
                    .tooltip(label)
                    .disabled(!self.format_ready("design.styleSet"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .text_color(rgb(accent))
                                    .when(italic, |v| v.italic())
                                    .child(tr("标题", "Title")),
                            )
                            .child(div().text_size(px(8.)).child(tr("副标题", "Subtitle")))
                            .child(div().text_size(px(8.)).child(tr("正文文字", "Body text")))
                            .child(div().text_size(px(9.)).mt(px(4.)).child(label)),
                    )
                    .on_click(cx.listener(move |v, _, w, cx| {
                        v.run_format("design.styleSet", &json!({"name":name}), w, cx)
                    })),
            );
        }
        let colors = self.format_choices(
            "design-colors-menu",
            self.ribbon_tool(
                "design-colors",
                Tool::new("颜色", "Colors", I::SwatchBook),
                self.format_ready("design.themeColors"),
                true,
            ),
            "design.themeColors",
            themes("name"),
            cx,
        );
        let fonts = self.home_menu(
            "design-fonts-menu",
            self.ribbon_tool(
                "design-fonts",
                Tool::new("字体", "Fonts", I::Type),
                self.format_ready("design.themeFonts"),
                true,
            ),
            vec![
                (
                    "design-fonts-aptos",
                    "Aptos Display / Aptos",
                    self.format_ready("design.themeFonts"),
                    |v, w, cx| {
                        v.run_format(
                            "design.themeFonts",
                            &json!({"heading":"Aptos Display","body":"Aptos"}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "design-fonts-calibri",
                    "Calibri Light / Calibri",
                    self.format_ready("design.themeFonts"),
                    |v, w, cx| {
                        v.run_format(
                            "design.themeFonts",
                            &json!({"heading":"Calibri Light","body":"Calibri"}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "design-fonts-chinese",
                    "等线 / 宋体",
                    self.format_ready("design.themeFonts"),
                    |v, w, cx| {
                        v.run_format(
                            "design.themeFonts",
                            &json!({"heading":"DengXian","body":"SimSun"}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "design-fonts-custom",
                    tr("自定义字体…", "Customize Fonts…"),
                    self.format_ready("design.themeFonts"),
                    Self::design_fonts_form,
                ),
            ],
            cx,
        );
        let spacing = self.format_choices(
            "design-spacing-menu",
            self.ribbon_tool(
                "design-spacing",
                Tool::compact("段落间距", "Paragraph Spacing", I::List).width(122.),
                self.format_ready("design.paragraphSpacing"),
                true,
            ),
            "design.paragraphSpacing",
            [
                ("默认", "Default", "default"),
                ("无段落间距", "No Paragraph Space", "none"),
                ("紧凑", "Compact", "compact"),
                ("紧密", "Tight", "tight"),
                ("开放", "Open", "open"),
                ("宽松", "Relaxed", "relaxed"),
                ("双倍", "Double", "double"),
            ]
            .into_iter()
            .map(|(zh, en, value)| (tr(zh, en).into(), json!({"value":value})))
            .collect(),
            cx,
        );
        let options = div()
            .flex()
            .flex_col()
            .child(spacing)
            .child(
                self.ribbon_tool(
                    "design-effects",
                    Tool::compact("效果", "Effects", I::Sparkles).width(122.),
                    false,
                    true,
                )
                .tooltip(tr(
                    "主题效果尚未接入页面渲染",
                    "Theme effects are not rendered yet",
                )),
            )
            .child(
                self.ribbon_tool(
                    "design-default",
                    Tool::compact("设为默认值", "Set as Default", I::Check).width(122.),
                    false,
                    false,
                )
                .tooltip(tr(
                    "文档设计默认值尚未接入持久化",
                    "Document design defaults are not persisted yet",
                )),
            );
        let watermark = self.home_menu(
            "design-watermark-menu",
            self.ribbon_tool(
                "design-watermark",
                Tool::new("水印", "Watermark", I::Stamp),
                self.format_ready("design.watermark"),
                true,
            ),
            vec![
                (
                    "watermark-confidential",
                    tr("机密", "CONFIDENTIAL"),
                    self.format_ready("design.watermark"),
                    |v, w, cx| {
                        v.run_format(
                            "design.watermark",
                            &json!({"text":v.shell.tr("机密","CONFIDENTIAL"),"diagonal":true}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "watermark-draft",
                    tr("草稿", "DRAFT"),
                    self.format_ready("design.watermark"),
                    |v, w, cx| {
                        v.run_format(
                            "design.watermark",
                            &json!({"text":v.shell.tr("草稿","DRAFT"),"diagonal":true}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "watermark-custom",
                    tr("自定义水印…", "Custom Watermark…"),
                    self.format_ready("design.watermark"),
                    Self::design_watermark_form,
                ),
                (
                    "watermark-remove",
                    tr("删除水印", "Remove Watermark"),
                    self.format_ready("design.watermark"),
                    |v, w, cx| v.run_format("design.watermark", &json!({"remove":true}), w, cx),
                ),
            ],
            cx,
        );
        let page_color = self.format_choices(
            "design-page-color-menu",
            self.ribbon_tool(
                "design-page-color",
                Tool::new("页面颜色", "Page Color", I::PaintBucket),
                self.format_ready("design.pageColor"),
                true,
            ),
            "design.pageColor",
            [
                (tr("无颜色", "No Color"), None),
                (tr("浅蓝", "Light Blue"), Some("DCE6F1")),
                (tr("浅黄", "Light Yellow"), Some("FFF2CC")),
                (tr("浅绿", "Light Green"), Some("E2F0D9")),
                (tr("浅灰", "Light Gray"), Some("E7E6E6")),
                (tr("白色", "White"), Some("FFFFFF")),
            ]
            .into_iter()
            .map(|(label, color)| (label.into(), json!({"color":color})))
            .collect(),
            cx,
        );
        let borders = self.home_menu(
            "design-page-borders-menu",
            self.ribbon_tool(
                "design-page-borders",
                Tool::new("页面边框", "Page Borders", I::Square),
                self.format_ready("design.pageBorders"),
                true,
            ),
            vec![
                (
                    "page-border-box",
                    tr("方框", "Box"),
                    self.format_ready("design.pageBorders"),
                    |v, w, cx| {
                        v.run_format(
                            "design.pageBorders",
                            &json!({"kind":"box","width":1.}),
                            w,
                            cx,
                        )
                    },
                ),
                (
                    "page-border-none",
                    tr("无", "None"),
                    self.format_ready("design.pageBorders"),
                    |v, w, cx| v.run_format("design.pageBorders", &json!({"kind":"none"}), w, cx),
                ),
                (
                    "page-border-custom",
                    tr("边框和底纹…", "Borders and Shading…"),
                    self.format_ready("design.pageBorders"),
                    Self::design_border_form,
                ),
            ],
            cx,
        );
        div()
            .flex()
            .flex_shrink_0()
            .child(
                self.ribbon_centered_group(
                    "design-formatting",
                    tr("文档格式", "Document Formatting"),
                    div()
                        .flex()
                        .items_center()
                        .gap(px(3.))
                        .child(theme)
                        .child(gallery)
                        .child(colors)
                        .child(fonts)
                        .child(options),
                ),
            )
            .child(
                self.ribbon_centered_group(
                    "design-background",
                    tr("页面背景", "Page Background"),
                    div()
                        .flex()
                        .child(watermark)
                        .child(page_color)
                        .child(borders),
                ),
            )
    }

    fn design_fonts_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let settings = self
            .canvas
            .reader
            .as_ref()
            .map(|r| &r.engine().doc.settings);
        let heading = settings.map_or(String::new(), |s| s.major_font.clone());
        let body = settings.map_or(String::new(), |s| s.minor_font.clone());
        self.fields_form(
            "主题字体 / Theme Fonts",
            "design.themeFonts",
            vec![
                ("标题字体 / Heading font", heading),
                ("正文字体 / Body font", body),
            ],
            false,
            |values| {
                let names: Vec<_> = values.iter().map(|v| v.trim()).collect();
                if names
                    .iter()
                    .any(|v| v.is_empty() || v.len() > 128 || v.chars().any(char::is_control))
                {
                    return Err("字体名称需要 1–128 字节 / Font names must be 1–128 bytes".into());
                }
                Ok(json!({"heading":names[0],"body":names[1]}))
            },
            w,
            cx,
        );
    }

    fn design_watermark_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        let wm = self
            .canvas
            .reader
            .as_ref()
            .and_then(|r| r.engine().doc.settings.watermark.as_ref());
        let fields = vec![
            ("文字 / Text", wm.map_or(String::new(), |w| w.text.clone())),
            (
                "颜色 / Color (RRGGBB)",
                wm.map_or("BFBFBF".into(), |w| w.color.hex()),
            ),
            (
                "方向 / Direction (diagonal / horizontal)",
                wm.map_or("diagonal".into(), |w| {
                    if w.diagonal { "diagonal" } else { "horizontal" }.into()
                }),
            ),
        ];
        self.fields_form(
            "文字水印 / Text Watermark",
            "design.watermark",
            fields,
            false,
            |v| {
                if v[0].trim().is_empty() || v[0].chars().count() > 200 {
                    return Err("水印需要 1–200 个字符 / Watermarks need 1–200 characters".into());
                }
                let diagonal = match v[2].trim() {
                    "diagonal" => true,
                    "horizontal" => false,
                    _ => return Err("方向请输入 diagonal 或 horizontal".into()),
                };
                Ok(json!({"text":v[0],"color":color(&v[1])?,"diagonal":diagonal}))
            },
            w,
            cx,
        );
    }

    fn design_border_form(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.fields_form("页面边框 / Page Borders", "design.pageBorders", vec![
            ("线型 / Style (single / double / dotted / dashed)","single".into()),
            ("线宽 / Width (pt)","1".into()), ("颜色 / Color (RRGGBB)","315FA8".into()),
            ("应用于 / Apply to (section / document)","section".into())], false, |v| {
            if !["single","double","dotted","dashed","thick","triple","dotDash","wave"].contains(&v[0].trim()) { return Err("无效线型 / Invalid border style".into()); }
            if !["section","document"].contains(&v[3].trim()) { return Err("应用范围应为 section 或 document".into()); }
            Ok(json!({"style":v[0].trim(),"width":joyword_shell::authoring::required_number(&v[1],0.25,6.)?,"color":color(&v[2])?,"applyTo":v[3].trim()}))
        }, w, cx);
    }
}
