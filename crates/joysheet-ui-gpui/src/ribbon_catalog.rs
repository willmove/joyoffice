//! Ribbon structure transcribed from the locally installed Excel UI, 2026-11-11.
//! Commands are JoySheet's own public engine protocol; no Office assets are used.
use joyoffice_ui::IconName as I;

pub const TABS: &[(&str, &str, &str)] = &[
    ("ribbon-file", "文件", "File"),
    ("ribbon-home", "开始", "Home"),
    ("drawing-workflows", "插入", "Insert"),
    ("ribbon-draw", "绘图", "Draw"),
    ("print-workflows", "页面布局", "Page Layout"),
    ("analysis-workflows", "公式", "Formulas"),
    ("data-workflows", "数据", "Data"),
    ("review-workflows", "审阅", "Review"),
    ("window-panel", "视图", "View"),
    ("ribbon-help", "帮助", "Help"),
    ("automation-workflows", "自动化", "Automate"),
];

#[derive(Clone, Copy, Debug)]
pub struct Tool {
    pub id: &'static str,
    pub zh: &'static str,
    pub en: &'static str,
    pub icon: I,
    pub command: &'static str,
    pub dialog: bool,
    pub menu: bool,
    pub params: &'static str,
    pub compact: bool,
}
pub fn tool(id: &'static str, zh: &'static str, en: &'static str, icon: I, command: &'static str) -> Tool {
    Tool { id, zh, en, icon, command, dialog: false, menu: false, params: "{}", compact: false }
}
impl Tool {
    pub fn dialog(mut self) -> Self {
        self.dialog = true;
        self
    }
    pub fn menu(mut self) -> Self {
        self.menu = true;
        self
    }
    pub fn params(mut self, params: &'static str) -> Self {
        self.params = params;
        self
    }
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}
pub struct Group {
    pub zh: &'static str,
    pub en: &'static str,
    pub tools: Vec<Vec<Tool>>,
    pub rows: bool,
    pub launcher: Option<&'static str>,
}
fn group(zh: &'static str, en: &'static str, tools: Vec<Vec<Tool>>) -> Group {
    Group { zh, en, tools, rows: false, launcher: None }
}
impl Group {
    fn rows(mut self) -> Self {
        self.rows = true;
        self
    }
    fn launcher(mut self, command: &'static str) -> Self {
        self.launcher = Some(command);
        self
    }
}

pub fn groups(tab: usize) -> Vec<Group> {
    let t = tool;
    match tab {
        1 => vec![
            group(
                "剪贴板",
                "Clipboard",
                vec![
                    vec![t("paste", "粘贴", "Paste", I::ClipboardPaste, "paste").menu()],
                    vec![
                        t("cut", "剪切", "Cut", I::Scissors, "edit.cut"),
                        t("copy", "复制", "Copy", I::Copy, "edit.copy"),
                        t("format-painter", "格式刷", "Format Painter", I::Paintbrush, "@format-painter"),
                    ],
                ],
            ),
            group(
                "字体",
                "Font",
                vec![
                    vec![
                        t("font-name", "字体", "Font", I::Type, "font-name").menu(),
                        t("font-size", "字号", "Size", I::Type, "font-size").menu(),
                        t("font-grow", "增大字号", "Increase Font Size", I::AArrowUp, "home.increaseFontSize").compact(),
                        t("font-shrink", "减小字号", "Decrease Font Size", I::AArrowDown, "home.decreaseFontSize").compact(),
                    ],
                    vec![
                        t("bold", "加粗", "Bold", I::Bold, "home.bold").compact(),
                        t("italic", "倾斜", "Italic", I::Italic, "home.italic").compact(),
                        t("underline", "下划线", "Underline", I::Underline, "underline").menu().compact(),
                        t("borders", "边框", "Borders", I::Grid2x2, "borders").menu().compact(),
                        t("fill", "填充颜色", "Fill Color", I::PaintBucket, "fill").menu().compact(),
                        t("font-color", "字体颜色", "Font Color", I::Baseline, "font-color").menu().compact(),
                    ],
                ],
            )
            .rows()
            .launcher("home.formatCells"),
            group(
                "对齐方式",
                "Alignment",
                vec![
                    vec![
                        t("align-top", "顶端对齐", "Top Align", I::AlignStartVertical, "home.alignTop").compact(),
                        t("align-middle", "垂直居中", "Middle Align", I::AlignCenterVertical, "home.alignMiddle").compact(),
                        t("align-bottom", "底端对齐", "Bottom Align", I::AlignEndVertical, "home.alignBottom").compact(),
                        t("orientation", "方向", "Orientation", I::RotateCcw, "orientation").menu().compact(),
                        t("wrap", "自动换行", "Wrap Text", I::TextWrap, "home.wrapText"),
                    ],
                    vec![
                        t("align-left", "左对齐", "Align Left", I::TextAlignStart, "home.alignLeft").compact(),
                        t("align-center", "居中", "Center", I::TextAlignCenter, "home.alignCenter").compact(),
                        t("align-right", "右对齐", "Align Right", I::TextAlignEnd, "home.alignRight").compact(),
                        t("indent-less", "减少缩进量", "Decrease Indent", I::ListIndentDecrease, "home.decreaseIndent").compact(),
                        t("indent-more", "增加缩进量", "Increase Indent", I::ListIndentIncrease, "home.increaseIndent").compact(),
                        t("merge", "合并后居中", "Merge & Center", I::TableCellsMerge, "merge").menu(),
                    ],
                ],
            )
            .rows()
            .launcher("home.formatCells"),
            group(
                "数字",
                "Number",
                vec![
                    vec![t("number-format", "常规", "General", I::Hash, "number-format").menu()],
                    vec![
                        t("currency", "会计数字格式", "Accounting", I::Banknote, "home.accounting").compact(),
                        t("percent", "百分比样式", "Percent", I::Percent, "home.percent").compact(),
                        t("comma", "千位分隔样式", "Comma Style", I::Quote, "home.comma").compact(),
                        t("decimal-increase", "增加小数位数", "Increase Decimal", I::DecimalsArrowLeft, "home.increaseDecimal").compact(),
                        t("decimal", "减少小数位数", "Decrease Decimal", I::DecimalsArrowRight, "home.decreaseDecimal").compact(),
                    ],
                ],
            )
            .rows()
            .launcher("home.numberFormat"),
            group(
                "样式",
                "Styles",
                vec![
                    vec![t("conditional-format", "条件格式", "Conditional Formatting", I::TableProperties, "conditional").menu()],
                    vec![t("format-as-table", "套用表格格式", "Format as Table", I::Table, "home.formatAsTable").dialog()],
                    vec![t("cell-styles", "单元格样式", "Cell Styles", I::PanelsTopLeft, "cell-styles").menu()],
                ],
            ),
            group(
                "单元格",
                "Cells",
                vec![
                    vec![t("insert-cells", "插入", "Insert", I::TableRowsSplit, "insert-cells").menu()],
                    vec![t("delete-cells", "删除", "Delete", I::TableRowsSplit, "delete-cells").menu()],
                    vec![t("format-panel", "格式", "Format", I::TableProperties, "format-cells").menu()],
                ],
            ),
            group(
                "编辑",
                "Editing",
                vec![
                    vec![
                        t("autosum", "自动求和", "AutoSum", I::Sigma, "autosum").menu(),
                        t("fill-menu", "填充", "Fill", I::ArrowDownToLine, "fill-menu").menu(),
                        t("clear-menu", "清除", "Clear", I::Eraser, "clear").menu(),
                    ],
                    vec![t("home-sort", "排序和筛选", "Sort & Filter", I::ListFilter, "sort-filter").menu()],
                    vec![t("home-find", "查找和选择", "Find & Select", I::Search, "find-select").menu()],
                ],
            ),
        ],
        2 => vec![
            group(
                "表格",
                "Tables",
                vec![
                    vec![t("insert-pivot", "数据透视表", "PivotTable", I::TableProperties, "insert.pivotTable").dialog()],
                    vec![t(
                        "insert-pivot-recommended",
                        "推荐的数据透视表",
                        "Recommended PivotTables",
                        I::TableProperties,
                        "insert.recommendedPivotTables",
                    )],
                    vec![t("insert-table", "表格", "Table", I::Table, "insert.table").dialog()],
                ],
            ),
            group(
                "插图",
                "Illustrations",
                vec![
                    vec![t("insert-picture", "图片", "Pictures", I::Image, "insert.picture").dialog()],
                    vec![
                        t("insert-shape", "形状", "Shapes", I::Shapes, "insert.shape").dialog(),
                        t("insert-icons", "图标", "Icons", I::Shapes, "insert.icons").dialog(),
                        t("insert-smartart", "SmartArt", "SmartArt", I::Workflow, "insert.smartArt").dialog(),
                    ],
                ],
            ),
            group(
                "图表",
                "Charts",
                vec![
                    vec![t("recommended-charts", "推荐的图表", "Recommended Charts", I::ChartNoAxesCombined, "insert.recommendedCharts")],
                    vec![t("insert-chart", "插入图表", "Insert Chart", I::ChartColumn, "charts").menu()],
                    vec![t("insert-pivot-chart", "数据透视图", "PivotChart", I::ChartColumn, "insert.pivotChart").dialog()],
                ],
            ),
            group(
                "迷你图",
                "Sparklines",
                vec![
                    vec![t("spark-line", "折线", "Line", I::ChartLine, "insert.sparkline").dialog().params("{\"type\":\"line\"}")],
                    vec![t("spark-column", "柱形", "Column", I::ChartColumn, "insert.sparkline").dialog().params("{\"type\":\"column\"}")],
                    vec![t("spark-winloss", "盈亏", "Win/Loss", I::ChartNoAxesColumn, "insert.sparkline").dialog().params("{\"type\":\"winLoss\"}")],
                ],
            ),
            group(
                "筛选器",
                "Filters",
                vec![
                    vec![t("insert-slicer", "切片器", "Slicer", I::ListFilter, "insert.slicer").dialog()],
                    vec![t("insert-timeline", "日程表", "Timeline", I::Calendar, "insert.timeline").dialog()],
                ],
            ),
            group("链接", "Links", vec![vec![t("insert-link", "链接", "Link", I::Link, "insert.link").dialog()]]),
            group("批注", "Comments", vec![vec![t("insert-comment", "批注", "Comment", I::MessageSquarePlus, "review.newComment").dialog()]]),
            group(
                "文本",
                "Text",
                vec![
                    vec![t("insert-text", "文本框", "Text Box", I::TextCursorInput, "insert.textBox").dialog()],
                    vec![
                        t("header-footer", "页眉和页脚", "Header & Footer", I::PanelTop, "pageLayout.headerFooter").dialog(),
                        t("word-art", "艺术字", "WordArt", I::Type, "insert.wordArt").dialog(),
                        t("insert-object", "对象", "Object", I::Box, "insert.object").dialog(),
                    ],
                ],
            ),
            group(
                "符号",
                "Symbols",
                vec![
                    vec![t("insert-equation", "公式", "Equation", I::Pi, "insert.equation").dialog()],
                    vec![t("insert-symbol", "符号", "Symbol", I::Omega, "insert.symbol").dialog()],
                ],
            ),
        ],
        3 => vec![
            group("输入模式", "Input Mode", vec![vec![t("draw-mode", "绘图", "Draw", I::PenTool, "@pen")]]),
            group(
                "撤消",
                "Undo",
                vec![vec![t("draw-undo", "撤消", "Undo", I::Undo2, "edit.undo"), t("draw-redo", "重做", "Redo", I::Redo2, "edit.redo")]],
            ),
            group(
                "绘图工具",
                "Drawing Tools",
                vec![
                    vec![t("draw-select", "选择", "Select", I::MousePointer2, "@select")],
                    vec![t("draw-eraser", "橡皮擦", "Eraser", I::Eraser, "@eraser")],
                    vec![t("draw-black", "黑色笔", "Black Pen", I::PenTool, "@pen").params("{\"color\":\"#202020\",\"width\":2}")],
                    vec![t("draw-red", "红色笔", "Red Pen", I::PenTool, "@pen").params("{\"color\":\"#C00000\",\"width\":2}")],
                    vec![t("draw-blue", "蓝色笔", "Blue Pen", I::PenTool, "@pen").params("{\"color\":\"#1F5FC9\",\"width\":2}")],
                    vec![t("draw-highlight", "荧光笔", "Highlighter", I::Highlighter, "@pen").params("{\"color\":\"#FFD966\",\"width\":8}")],
                ],
            ),
            group("转换", "Convert", vec![vec![t("ink-to-shape", "将墨迹转换为形状", "Ink to Shape", I::Shapes, "draw.inkToShape")]]),
        ],
        4 => vec![
            group(
                "主题",
                "Themes",
                vec![
                    vec![t("theme-gallery", "主题", "Themes", I::PanelsTopLeft, "pageLayout.theme").dialog()],
                    vec![
                        t("theme-colors", "颜色", "Colors", I::Palette, "pageLayout.themeColors").dialog(),
                        t("theme-fonts", "字体", "Fonts", I::Type, "pageLayout.themeFonts").dialog(),
                    ],
                ],
            ),
            group(
                "页面设置",
                "Page Setup",
                vec![
                    vec![t("page-margins", "页边距", "Margins", I::Scan, "margins").menu()],
                    vec![t("page-orientation", "纸张方向", "Orientation", I::Files, "page-orientation").menu()],
                    vec![t("page-size", "纸张大小", "Size", I::File, "page-size").menu()],
                    vec![t("print-area", "打印区域", "Print Area", I::Printer, "print-area").menu()],
                    vec![t("page-breaks", "分隔符", "Breaks", I::BetweenHorizontalStart, "pageLayout.breaks").dialog()],
                    vec![t("page-background", "背景", "Background", I::Image, "pageLayout.background").dialog()],
                    vec![t("print-titles", "打印标题", "Print Titles", I::Table, "pageLayout.printTitles").dialog()],
                ],
            )
            .launcher("pageLayout.pageSetup"),
            group(
                "调整为合适大小",
                "Scale to Fit",
                vec![vec![
                    t("page-width", "宽度：自动", "Width: Automatic", I::MoveHorizontal, "pageLayout.scaleToFit").dialog(),
                    t("page-height", "高度：自动", "Height: Automatic", I::MoveVertical, "pageLayout.scaleToFit").dialog(),
                    t("page-scale", "缩放比例：100%", "Scale: 100%", I::Percent, "pageLayout.scaleToFit").dialog(),
                ]],
            ),
            group(
                "工作表选项",
                "Sheet Options",
                vec![
                    vec![
                        t("layout-grid-view", "查看网格线", "View Gridlines", I::Square, "view.gridlines"),
                        t("layout-grid-print", "打印网格线", "Print Gridlines", I::Square, "pageLayout.printGridlines"),
                    ],
                    vec![
                        t("layout-heading-view", "查看标题", "View Headings", I::Square, "view.headings"),
                        t("layout-heading-print", "打印标题", "Print Headings", I::Square, "pageLayout.printHeadings"),
                    ],
                ],
            ),
            group(
                "排列",
                "Arrange",
                vec![
                    vec![t("bring-forward", "上移一层", "Bring Forward", I::BringToFront, "arrange.bringForward").dialog()],
                    vec![t("send-backward", "下移一层", "Send Backward", I::SendToBack, "arrange.sendBackward").dialog()],
                    vec![t("selection-pane", "选择窗格", "Selection Pane", I::MousePointer2, "@objects")],
                    vec![
                        t("align-objects", "对齐", "Align", I::AlignStartVertical, "@objects"),
                        t("rotate-object", "旋转", "Rotate", I::RotateCw, "arrange.rotate").dialog(),
                    ],
                ],
            ),
        ],
        5 => vec![
            group(
                "函数库",
                "Function Library",
                vec![
                    vec![t("insert-function", "插入函数", "Insert Function", I::SquareFunction, "formulas.insertFunction").dialog()],
                    vec![t("formula-autosum", "自动求和", "AutoSum", I::Sigma, "autosum").menu()],
                    vec![t("functions-recent", "最近使用的函数", "Recently Used", I::Clock, "@functions:Recent")],
                    vec![t("functions-financial", "财务", "Financial", I::Banknote, "@functions:Financial")],
                    vec![t("functions-logical", "逻辑", "Logical", I::CircleQuestionMark, "@functions:Logical")],
                    vec![t("functions-text", "文本", "Text", I::Type, "@functions:Text")],
                    vec![t("functions-date", "日期和时间", "Date & Time", I::Calendar, "@functions:DateTime")],
                    vec![t("functions-lookup", "查找与引用", "Lookup & Reference", I::Search, "@functions:Lookup")],
                    vec![t("functions-math", "数学和三角函数", "Math & Trig", I::Calculator, "@functions:MathTrig")],
                    vec![t("functions-more", "其他函数", "More Functions", I::Ellipsis, "@functions:All")],
                ],
            ),
            group(
                "定义的名称",
                "Defined Names",
                vec![
                    vec![t("name-manager", "名称管理器", "Name Manager", I::Tags, "formulas.nameManager")],
                    vec![
                        t("define-name", "定义名称", "Define Name", I::Tag, "formulas.defineName").dialog(),
                        t("use-name", "用于公式", "Use in Formula", I::Tag, "formulas.pasteName").dialog(),
                        t("names-from-selection", "根据所选内容创建", "Create from Selection", I::Table, "formulas.createFromSelection").dialog(),
                    ],
                ],
            ),
            group(
                "公式审核",
                "Formula Auditing",
                vec![
                    vec![
                        t("trace-precedents", "追踪引用单元格", "Trace Precedents", I::ArrowUpRight, "formulas.tracePrecedents"),
                        t("trace-dependents", "追踪从属单元格", "Trace Dependents", I::ArrowDownRight, "formulas.traceDependents"),
                        t("trace-remove", "移去箭头", "Remove Arrows", I::Eraser, "formulas.removeArrows"),
                    ],
                    vec![
                        t("show-formulas", "显示公式", "Show Formulas", I::SquareFunction, "formulas.showFormulas"),
                        t("formula-errors", "错误检查", "Error Checking", I::TriangleAlert, "formulas.errorChecking"),
                        t("formula-evaluate", "公式求值", "Evaluate Formula", I::Search, "formulas.evaluateFormula"),
                    ],
                    vec![t("watch-window", "监视窗口", "Watch Window", I::Glasses, "formulas.watchWindow").dialog()],
                ],
            ),
            group(
                "计算",
                "Calculation",
                vec![
                    vec![t("calculation-options", "计算选项", "Calculation Options", I::Calculator, "formulas.calculationOptions").dialog()],
                    vec![
                        t("calculate", "开始计算", "Calculate Now", I::Calculator, "formulas.calculateNow"),
                        t("calculate-sheet", "计算工作表", "Calculate Sheet", I::Table, "formulas.calculateSheet"),
                    ],
                ],
            ),
        ],
        6 => vec![
            group(
                "获取和转换数据",
                "Get & Transform Data",
                vec![
                    vec![t("get-data", "获取数据", "Get Data", I::Database, "data.getData").dialog()],
                    vec![
                        t("from-csv", "从文本/CSV", "From Text/CSV", I::FileText, "data.fromTextCsv").dialog(),
                        t("from-table", "来自表格/区域", "From Table/Range", I::Table, "data.fromTableRange"),
                        t("recent-sources", "最近使用的源", "Recent Sources", I::Clock, "data.queriesConnections"),
                    ],
                ],
            ),
            group(
                "查询和连接",
                "Queries & Connections",
                vec![
                    vec![t("refresh-all", "全部刷新", "Refresh All", I::RefreshCw, "data.refreshAll")],
                    vec![t("queries", "查询和连接", "Queries & Connections", I::Database, "data.queriesConnections")],
                ],
            ),
            group(
                "排序和筛选",
                "Sort & Filter",
                vec![
                    vec![
                        t("sort-up", "升序", "Sort A to Z", I::ArrowDownAZ, "data.sortAscending"),
                        t("sort-down", "降序", "Sort Z to A", I::ArrowDownZA, "data.sortDescending"),
                    ],
                    vec![t("sort-dialog", "排序", "Sort", I::ListOrdered, "data.sort").dialog()],
                    vec![t("filter", "筛选", "Filter", I::Funnel, "data.filter")],
                    vec![
                        t("clear-filter", "清除", "Clear", I::FunnelX, "data.clearFilter"),
                        t("reapply-filter", "重新应用", "Reapply", I::RefreshCw, "data.reapply"),
                        t("advanced-filter", "高级", "Advanced", I::ListFilter, "data.advancedFilter").dialog(),
                    ],
                ],
            ),
            group(
                "数据工具",
                "Data Tools",
                vec![
                    vec![t("text-to-columns", "分列", "Text to Columns", I::Columns3, "data.textToColumns").dialog()],
                    vec![t("flash-fill", "快速填充", "Flash Fill", I::Zap, "edit.flashFill")],
                    vec![t("remove-duplicates", "删除重复值", "Remove Duplicates", I::CopyX, "data.removeDuplicates").dialog()],
                    vec![t("validation", "数据验证", "Data Validation", I::ListChecks, "data.validation").dialog()],
                    vec![t("consolidate", "合并计算", "Consolidate", I::Combine, "data.consolidate").dialog()],
                ],
            ),
            group(
                "预测",
                "Forecast",
                vec![
                    vec![t("what-if", "模拟分析", "What-If Analysis", I::TableProperties, "what-if").menu()],
                    vec![t("forecast", "预测工作表", "Forecast Sheet", I::ChartLine, "data.forecastSheet").dialog()],
                ],
            ),
            group(
                "分级显示",
                "Outline",
                vec![
                    vec![t("group", "组合", "Group", I::Group, "data.group").dialog()],
                    vec![t("ungroup", "取消组合", "Ungroup", I::Ungroup, "data.ungroup").dialog()],
                    vec![t("subtotal", "分类汇总", "Subtotal", I::Table, "data.subtotal").dialog()],
                    vec![
                        t("show-detail", "显示明细数据", "Show Detail", I::Plus, "data.showDetail").dialog(),
                        t("hide-detail", "隐藏明细数据", "Hide Detail", I::Minus, "data.hideDetail").dialog(),
                    ],
                ],
            ),
            group(
                "分析",
                "Analysis",
                vec![vec![
                    t("analysis", "数据分析", "Data Analysis", I::ChartNoAxesCombined, "data.analysisToolPak").dialog(),
                    t("solver", "规划求解", "Solver", I::Calculator, "data.solver").dialog(),
                ]],
            ),
        ],
        7 => vec![
            group(
                "校对",
                "Proofing",
                vec![
                    vec![t("spelling", "拼写检查", "Spelling", I::SpellCheck, "review.spelling")],
                    vec![t("thesaurus", "同义词库", "Thesaurus", I::BookOpen, "review.thesaurus").dialog()],
                    vec![t("workbook-statistics", "工作簿统计信息", "Workbook Statistics", I::ChartColumn, "review.workbookStatistics")],
                ],
            ),
            group(
                "辅助功能",
                "Accessibility",
                vec![vec![t("accessibility", "检查辅助功能", "Check Accessibility", I::Accessibility, "review.checkAccessibility")]],
            ),
            group(
                "批注",
                "Comments",
                vec![
                    vec![t("new-comment", "新建批注", "New Comment", I::MessageSquarePlus, "review.newComment").dialog()],
                    vec![t("delete-comment", "删除", "Delete", I::MessageSquareX, "review.deleteComment").dialog()],
                    vec![
                        t("previous-comment", "上一条批注", "Previous Comment", I::ArrowLeft, "review.previousComment"),
                        t("next-comment", "下一条批注", "Next Comment", I::ArrowRight, "review.nextComment"),
                    ],
                    vec![t("show-comments", "显示批注", "Show Comments", I::MessagesSquare, "@comments")],
                ],
            ),
            group("注释", "Notes", vec![vec![t("notes-menu", "注释", "Notes", I::StickyNote, "notes").menu()]]),
            group(
                "保护",
                "Protect",
                vec![
                    vec![t("protect-sheet", "保护工作表", "Protect Sheet", I::LockKeyhole, "review.protectSheet").dialog()],
                    vec![t("protect-workbook", "保护工作簿", "Protect Workbook", I::LockKeyhole, "review.protectWorkbook").dialog()],
                    vec![t("allow-edit-ranges", "允许编辑区域", "Allow Edit Ranges", I::TableProperties, "review.allowEditRanges").dialog()],
                    vec![t("unprotect-sheet", "撤销工作表保护", "Unprotect Sheet", I::LockKeyholeOpen, "review.unprotectSheet").dialog()],
                ],
            ),
        ],
        8 => vec![
            group(
                "工作簿视图",
                "Workbook Views",
                vec![
                    vec![t("view-normal", "普通", "Normal", I::Table, "view.normal")],
                    vec![t("view-page-break", "分页预览", "Page Break Preview", I::PanelsTopLeft, "view.pageBreakPreview")],
                    vec![t("view-page-layout", "页面布局", "Page Layout", I::File, "view.pageLayout")],
                    vec![t("custom-views", "自定义视图", "Custom Views", I::PanelsTopLeft, "view.customViews").dialog()],
                ],
            ),
            group(
                "显示",
                "Show",
                vec![vec![
                    t("gridlines", "网格线", "Gridlines", I::Square, "view.gridlines"),
                    t("headings", "标题", "Headings", I::Square, "view.headings"),
                    t("formula-bar-toggle", "编辑栏", "Formula Bar", I::Square, "@formula-bar"),
                ]],
            ),
            group(
                "缩放",
                "Zoom",
                vec![
                    vec![t("zoom", "缩放", "Zoom", I::Search, "view.zoom").dialog()],
                    vec![t("zoom100", "100%", "100%", I::FileSearch, "view.zoom100")],
                    vec![t("zoom-selection", "缩放到选定区域", "Zoom to Selection", I::Scan, "view.zoomToSelection")],
                ],
            ),
            group(
                "窗口",
                "Window",
                vec![
                    vec![t("new-window", "新建窗口", "New Window", I::AppWindow, "view.newWindow")],
                    vec![t("arrange-all", "全部重排", "Arrange All", I::PanelsTopLeft, "view.arrangeAll").dialog()],
                    vec![t("freeze", "冻结窗格", "Freeze Panes", I::Table, "freeze").menu()],
                    vec![
                        t("split", "拆分", "Split", I::PanelLeft, "view.split"),
                        t("hide-window", "隐藏", "Hide", I::EyeOff, "view.hideWindow"),
                        t("unhide-window", "取消隐藏", "Unhide", I::Eye, "view.unhideWindow"),
                    ],
                    vec![t("switch-windows", "切换窗口", "Switch Windows", I::AppWindow, "@windows")],
                ],
            ),
        ],
        9 => vec![group(
            "帮助",
            "Help",
            vec![
                vec![t("help", "帮助", "Help", I::CircleQuestionMark, "@help")],
                vec![t("training", "显示培训内容", "Show Training", I::GraduationCap, "@help")],
                vec![t("command-search", "搜索命令", "Search Commands", I::Search, "@search")],
            ],
        )],
        10 => vec![
            group(
                "脚本",
                "Scripts",
                vec![
                    vec![t("script-details", "所有脚本", "All Scripts", I::FileCode, "@scripts")],
                    vec![
                        t("script-record", "录制操作", "Record Actions", I::Circle, "@record"),
                        t("script-new", "新建脚本", "New Script", I::FilePlus, "automate.newScript").dialog(),
                    ],
                ],
            ),
            group("控制", "Control", vec![vec![t("desktop-control", "桌面控制", "Desktop Control", I::AppWindow, "@control")]]),
        ],
        _ => Vec::new(),
    }
}
