//! Local help and command discovery. No account, network or telemetry is required.
use super::ribbon::Tool;
use super::*;
use gpui::component::WindowExt;
use joyoffice_ui::IconName as I;

impl JoyWord {
    pub(super) fn help_ribbon(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tr = |zh, en| self.shell.tr(zh, en);
        let tools = div()
            .flex()
            .child(
                self.ribbon_tool(
                    "help-topics",
                    Tool::new("帮助", "Help", I::CircleQuestionMark),
                    true,
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.show_help(false, w, cx))),
            )
            .child(
                self.ribbon_tool(
                    "help-feedback",
                    Tool::new("反馈", "Feedback", I::MessageSquare),
                    true,
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.show_feedback(w, cx))),
            )
            .child(
                self.ribbon_tool(
                    "help-training",
                    Tool::new("显示培训内容", "Show Training", I::GraduationCap),
                    true,
                    false,
                )
                .on_click(cx.listener(|v, _, w, cx| v.show_help(true, w, cx))),
            );
        div()
            .flex()
            .child(self.ribbon_centered_group("help-group", tr("帮助", "Help"), tools))
    }

    pub(super) fn show_help(&self, training: bool, w: &mut Window, cx: &mut Context<Self>) {
        let chinese = self.shell.locale == joyword_shell::l10n::Locale::ZhCn;
        let title = self.shell.tr(
            if training {
                "JoyWord 入门"
            } else {
                "JoyWord 帮助"
            },
            if training {
                "Getting Started"
            } else {
                "JoyWord Help"
            },
        );
        let lines = if chinese {
            [
                "1. 文件 → 新建或打开本地文档；Ctrl+S 保存，F12 另存为。",
                "2. 开始：字体、段落与样式；Ctrl+B / I / U 设置选区格式。",
                "3. 插入：表格、图片、页眉页脚；选择对象后显示对应格式选项卡。",
                "4. 绘图：选择笔后在页面拖动；Esc 返回选择，Ctrl+Z 撤销。",
                "5. 设计与布局：主题、页面背景、页边距、纸张和段落间距。",
                "6. 引用、邮件、审阅：目录、脚注、合并字段、批注和修订。",
                "7. Ctrl+F 查找，Ctrl+H 替换，Ctrl+K 搜索桌面支持的命令。",
                "灰色命令表示当前文档或桌面版本不可用。",
            ]
        } else {
            [
                "1. File → New or Open a local document; Ctrl+S saves, F12 saves a copy.",
                "2. Home: fonts, paragraphs and styles; Ctrl+B / I / U formats the selection.",
                "3. Insert: tables, pictures, headers and footers; select an object for its format tab.",
                "4. Draw: choose a pen and drag on the page; Esc selects, Ctrl+Z undoes.",
                "5. Design and Layout: themes, page backgrounds, margins, paper and spacing.",
                "6. References, Mailings and Review: contents, notes, merge fields and changes.",
                "7. Ctrl+F finds, Ctrl+H replaces, Ctrl+K searches supported desktop commands.",
                "Disabled commands are unavailable in the current document or desktop build.",
            ]
        };
        w.open_dialog(cx, move |d, _, _| {
            d.title(title).width(px(620.)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(lines.into_iter().map(|line| div().child(line))),
            )
        });
    }

    fn show_feedback(&self, w: &mut Window, cx: &mut Context<Self>) {
        let text = format!("JoyWord {}\n{}\nhttps://github.com/willmove/joyoffice/issues", env!("CARGO_PKG_VERSION"),
            self.shell.tr("请在项目问题页附上复现步骤、操作系统及截图。", "Include reproduction steps, operating system and screenshots in the project issue tracker."));
        w.open_dialog(cx, move |d, _, _| {
            d.title("JoyWord").width(px(560.)).child(text.clone())
        });
    }
}
