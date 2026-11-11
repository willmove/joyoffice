//! Dropdown choices share the same command dispatch as the ribbon and keyboard.
use crate::ribbon_catalog::{Tool, tool as t};
use joyoffice_ui::IconName as I;

pub fn tools(menu: &str) -> Vec<Tool> {
    match menu {
        "paste" => vec![
            t("paste-normal", "粘贴", "Paste", I::ClipboardPaste, "edit.paste"),
            t("paste-values", "粘贴值", "Paste Values", I::Clipboard, "edit.pasteSpecial").params("{\"what\":\"values\"}"),
            t("paste-formats", "粘贴格式", "Paste Formats", I::Paintbrush, "edit.pasteSpecial").params("{\"what\":\"formats\"}"),
            t("paste-special", "选择性粘贴…", "Paste Special…", I::Clipboard, "edit.pasteSpecial").dialog(),
        ],
        "underline" => vec![
            t("underline-single", "下划线", "Underline", I::Underline, "home.underline"),
            t("underline-double", "双下划线", "Double Underline", I::Underline, "home.doubleUnderline"),
        ],
        "borders" => vec![
            t("border-bottom", "下框线", "Bottom Border", I::TableCellsMerge, "home.borders").params("{\"preset\":\"bottom\"}"),
            t("border-all", "所有框线", "All Borders", I::Grid2x2, "home.borders").params("{\"preset\":\"all\"}"),
            t("border-outside", "外侧框线", "Outside Borders", I::Square, "home.borders").params("{\"preset\":\"outside\"}"),
            t("border-none", "无框线", "No Border", I::SquareDashed, "home.borders").params("{\"preset\":\"none\"}"),
            t("border-more", "其他边框…", "More Borders…", I::Grid2x2, "home.borders").dialog(),
        ],
        "fill" | "font-color" => {
            let command = if menu == "fill" { "home.fillColor" } else { "home.fontColor" };
            vec![
                t("color-black", "黑色", "Black", I::Circle, command).params("{\"color\":\"#202020\"}"),
                t("color-red", "红色", "Red", I::Circle, command).params("{\"color\":\"#C00000\"}"),
                t("color-green", "绿色", "Green", I::Circle, command).params("{\"color\":\"#008050\"}"),
                t("color-blue", "蓝色", "Blue", I::Circle, command).params("{\"color\":\"#1F5FC9\"}"),
                t("color-yellow", "黄色", "Yellow", I::Circle, command).params("{\"color\":\"#FFFF00\"}"),
                t("color-more", "其他颜色…", "More Colors…", I::Palette, command).dialog(),
            ]
        }
        "orientation" => vec![
            t("angle-up", "逆时针角度", "Angle Counterclockwise", I::RotateCcw, "home.orientation").params("{\"angle\":45}"),
            t("angle-down", "顺时针角度", "Angle Clockwise", I::RotateCw, "home.orientation").params("{\"angle\":-45}"),
            t("angle-vertical", "竖排文字", "Vertical Text", I::Type, "home.orientation").params("{\"angle\":\"vertical\"}"),
            t("angle-none", "水平文字", "Horizontal Text", I::Type, "home.orientation").params("{\"angle\":0}"),
            t("angle-more", "设置单元格对齐格式…", "Format Cell Alignment…", I::TextAlignCenter, "home.formatCells").dialog(),
        ],
        "merge" => vec![
            t("merge-center", "合并后居中", "Merge & Center", I::TableCellsMerge, "home.mergeCenter"),
            t("merge-across", "跨越合并", "Merge Across", I::TableCellsMerge, "home.mergeAcross"),
            t("merge-cells", "合并单元格", "Merge Cells", I::TableCellsMerge, "home.mergeCells"),
            t("unmerge", "取消单元格合并", "Unmerge Cells", I::TableCellsSplit, "home.unmergeCells"),
        ],
        "font-name" => vec![
            t("font-aptos", "Aptos", "Aptos", I::Type, "home.fontName").params("{\"name\":\"Aptos\"}"),
            t("font-dengxian", "等线", "DengXian", I::Type, "home.fontName").params("{\"name\":\"DengXian\"}"),
            t("font-calibri", "Calibri", "Calibri", I::Type, "home.fontName").params("{\"name\":\"Calibri\"}"),
            t("font-yahei", "微软雅黑", "Microsoft YaHei", I::Type, "home.fontName").params("{\"name\":\"Microsoft YaHei\"}"),
            t("font-other", "其他字体…", "More Fonts…", I::Type, "home.fontName").dialog(),
        ],
        "font-size" => [8, 9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72]
            .into_iter()
            .map(|size| {
                // Static labels/parameters keep callback identity stable across frames.
                let (label, params) = match size {
                    8 => ("8", "{\"size\":8}"),
                    9 => ("9", "{\"size\":9}"),
                    10 => ("10", "{\"size\":10}"),
                    11 => ("11", "{\"size\":11}"),
                    12 => ("12", "{\"size\":12}"),
                    14 => ("14", "{\"size\":14}"),
                    16 => ("16", "{\"size\":16}"),
                    18 => ("18", "{\"size\":18}"),
                    20 => ("20", "{\"size\":20}"),
                    24 => ("24", "{\"size\":24}"),
                    28 => ("28", "{\"size\":28}"),
                    36 => ("36", "{\"size\":36}"),
                    48 => ("48", "{\"size\":48}"),
                    _ => ("72", "{\"size\":72}"),
                };
                t(label, label, label, I::Type, "home.fontSize").params(params)
            })
            .collect(),
        "number-format" => vec![
            t("format-general", "常规", "General", I::Hash, "home.numberFormat").params("{\"format\":\"General\"}"),
            t("format-number", "数值", "Number", I::Hash, "home.numberFormat").params("{\"format\":\"Number\"}"),
            t("format-currency", "货币", "Currency", I::Banknote, "home.numberFormat").params("{\"format\":\"Currency\"}"),
            t("format-accounting", "会计专用", "Accounting", I::Banknote, "home.accounting"),
            t("format-date", "短日期", "Short Date", I::Calendar, "home.numberFormat").params("{\"format\":\"Short Date\"}"),
            t("format-percent", "百分比", "Percentage", I::Percent, "home.percent"),
            t("format-text", "文本", "Text", I::Type, "home.numberFormat").params("{\"format\":\"Text\"}"),
            t("format-more", "其他数字格式…", "More Number Formats…", I::Hash, "home.numberFormat").dialog(),
        ],
        "conditional" => vec![
            t("conditional-rule", "突出显示单元格规则…", "Highlight Cells Rules…", I::TableProperties, "home.conditionalFormat").dialog(),
            t("conditional-manage", "管理规则…", "Manage Rules…", I::List, "home.manageRules"),
            t("conditional-clear", "清除规则", "Clear Rules", I::Eraser, "home.clearRules"),
        ],
        "cell-styles" => vec![
            t("style-normal", "常规", "Normal", I::Table, "home.cellStyle").params("{\"name\":\"Normal\"}"),
            t("style-good", "好", "Good", I::Check, "home.cellStyle").params("{\"name\":\"Good\"}"),
            t("style-bad", "差", "Bad", I::X, "home.cellStyle").params("{\"name\":\"Bad\"}"),
            t("style-heading", "标题 1", "Heading 1", I::Type, "home.cellStyle").params("{\"name\":\"Heading 1\"}"),
            t("style-new", "新建单元格样式…", "New Cell Style…", I::Plus, "home.newCellStyle").dialog(),
        ],
        "insert-cells" => vec![
            t("insert-cell-dialog", "插入单元格…", "Insert Cells…", I::Plus, "home.insertCells").dialog(),
            t("insert-row", "插入工作表行", "Insert Sheet Rows", I::Rows3, "home.insertRows"),
            t("insert-column", "插入工作表列", "Insert Sheet Columns", I::Columns3, "home.insertColumns"),
            t("insert-sheet", "插入工作表", "Insert Sheet", I::Table, "home.insertSheet"),
        ],
        "delete-cells" => vec![
            t("delete-cell-dialog", "删除单元格…", "Delete Cells…", I::Minus, "home.deleteCells").dialog(),
            t("delete-row", "删除工作表行", "Delete Sheet Rows", I::Rows3, "home.deleteRows"),
            t("delete-column", "删除工作表列", "Delete Sheet Columns", I::Columns3, "home.deleteColumns"),
            t("delete-sheet", "删除工作表…", "Delete Sheet…", I::Table, "@delete-sheet"),
        ],
        "format-cells" => vec![
            t("row-height", "行高…", "Row Height…", I::MoveVertical, "home.rowHeight").dialog(),
            t("autofit-row", "自动调整行高", "AutoFit Row Height", I::MoveVertical, "home.autofitRowHeight"),
            t("column-width", "列宽…", "Column Width…", I::MoveHorizontal, "home.columnWidth").dialog(),
            t("autofit", "自动调整列宽", "AutoFit Column Width", I::MoveHorizontal, "home.autofitColumnWidth"),
            t("hide-rows", "隐藏行", "Hide Rows", I::EyeOff, "home.hideRows"),
            t("unhide-rows", "取消隐藏行", "Unhide Rows", I::Eye, "home.unhideRows"),
            t("hide-columns", "隐藏列", "Hide Columns", I::EyeOff, "home.hideColumns"),
            t("unhide-columns", "取消隐藏列", "Unhide Columns", I::Eye, "home.unhideColumns"),
            t("rename-sheet", "重命名工作表…", "Rename Sheet…", I::Table, "sheet.rename").dialog(),
            t("format-cells-dialog", "设置单元格格式…", "Format Cells…", I::TableProperties, "home.formatCells").dialog(),
        ],
        "autosum" => vec![
            t("sum", "求和", "Sum", I::Sigma, "formulas.autoSum"),
            t("average", "平均值", "Average", I::Sigma, "formulas.autoSum").params("{\"function\":\"AVERAGE\"}"),
            t("count", "计数", "Count Numbers", I::Hash, "formulas.autoSum").params("{\"function\":\"COUNT\"}"),
            t("max", "最大值", "Max", I::ArrowUp, "formulas.autoSum").params("{\"function\":\"MAX\"}"),
            t("min", "最小值", "Min", I::ArrowDown, "formulas.autoSum").params("{\"function\":\"MIN\"}"),
        ],
        "fill-menu" => vec![
            t("fill-down", "向下", "Down", I::ArrowDown, "edit.fillDown"),
            t("fill-right", "向右", "Right", I::ArrowRight, "edit.fillRight"),
            t("fill-up", "向上", "Up", I::ArrowUp, "edit.fillUp"),
            t("fill-left", "向左", "Left", I::ArrowLeft, "edit.fillLeft"),
            t("fill-series", "序列…", "Series…", I::ListOrdered, "edit.fillSeries").dialog(),
            t("fill-flash", "快速填充", "Flash Fill", I::Zap, "edit.flashFill"),
        ],
        "clear" => vec![
            t("clear-all", "全部清除", "Clear All", I::Eraser, "edit.clearAll"),
            t("clear-formats", "清除格式", "Clear Formats", I::Paintbrush, "edit.clearFormats"),
            t("clear-contents", "清除内容", "Clear Contents", I::Eraser, "edit.clearContents"),
            t("clear-comments", "清除批注", "Clear Comments", I::MessageSquareX, "edit.clearComments"),
            t("clear-links", "清除超链接", "Clear Hyperlinks", I::Link, "edit.clearHyperlinks"),
        ],
        "sort-filter" => vec![
            t("sort-ascending", "升序", "Sort A to Z", I::ArrowDownAZ, "data.sortAscending"),
            t("sort-descending", "降序", "Sort Z to A", I::ArrowDownZA, "data.sortDescending"),
            t("sort-custom", "自定义排序…", "Custom Sort…", I::ListOrdered, "data.sort").dialog(),
            t("sort-filter-toggle", "筛选", "Filter", I::Funnel, "data.filter"),
            t("sort-filter-clear", "清除筛选", "Clear Filter", I::FunnelX, "data.clearFilter"),
        ],
        "find-select" => vec![
            t("find", "查找…", "Find…", I::Search, "edit.find").dialog(),
            t("replace", "替换…", "Replace…", I::Replace, "edit.replace").dialog(),
            t("goto", "定位…", "Go To…", I::ArrowRight, "edit.goTo").dialog(),
            t("goto-special", "定位条件…", "Go To Special…", I::Scan, "edit.goToSpecial").dialog(),
        ],
        "charts" => vec![
            t("chart-column", "柱形图…", "Column…", I::ChartColumn, "insert.chart").dialog().params("{\"type\":\"column\"}"),
            t("chart-line", "折线图…", "Line…", I::ChartLine, "insert.chart").dialog().params("{\"type\":\"line\"}"),
            t("chart-pie", "饼图…", "Pie…", I::ChartPie, "insert.chart").dialog().params("{\"type\":\"pie\"}"),
            t("chart-bar", "条形图…", "Bar…", I::ChartBar, "insert.chart").dialog().params("{\"type\":\"bar\"}"),
            t("chart-scatter", "散点图…", "Scatter…", I::ChartScatter, "insert.chart").dialog().params("{\"type\":\"scatter\"}"),
            t("chart-more", "所有图表…", "All Charts…", I::ChartNoAxesCombined, "insert.chart").dialog(),
        ],
        "margins" => vec![
            t("margins-normal", "普通", "Normal", I::Scan, "pageLayout.margins").params("{\"preset\":\"normal\"}"),
            t("margins-wide", "宽", "Wide", I::Scan, "pageLayout.margins").params("{\"preset\":\"wide\"}"),
            t("margins-narrow", "窄", "Narrow", I::Scan, "pageLayout.margins").params("{\"preset\":\"narrow\"}"),
            t("margins-custom", "自定义页边距…", "Custom Margins…", I::Scan, "pageLayout.pageSetup").dialog(),
        ],
        "page-orientation" => vec![
            t("portrait", "纵向", "Portrait", I::File, "pageLayout.orientation").params("{\"orientation\":\"portrait\"}"),
            t("landscape", "横向", "Landscape", I::Files, "pageLayout.orientation").params("{\"orientation\":\"landscape\"}"),
        ],
        "page-size" => vec![
            t("paper-a4", "A4", "A4", I::File, "pageLayout.size").params("{\"paper\":\"A4\"}"),
            t("paper-a3", "A3", "A3", I::File, "pageLayout.size").params("{\"paper\":\"A3\"}"),
            t("paper-letter", "Letter", "Letter", I::File, "pageLayout.size").params("{\"paper\":\"Letter\"}"),
        ],
        "print-area" => vec![
            t("print-area-set", "设置打印区域", "Set Print Area", I::Printer, "pageLayout.printArea"),
            t("print-area-clear", "取消打印区域", "Clear Print Area", I::Eraser, "pageLayout.printArea").params("{\"clear\":true}"),
        ],
        "what-if" => vec![
            t("scenario-manager", "方案管理器…", "Scenario Manager…", I::List, "data.scenarioManager"),
            t("goal-seek", "单变量求解…", "Goal Seek…", I::Calculator, "data.goalSeek").dialog(),
            t("data-table", "模拟运算表…", "Data Table…", I::Table, "data.dataTable").dialog(),
        ],
        "notes" => vec![
            t("new-note", "新建注释…", "New Note…", I::StickyNote, "review.newNote").dialog(),
            t("show-note", "显示/隐藏注释…", "Show/Hide Note…", I::StickyNote, "review.showNote").dialog(),
            t("show-notes", "显示所有注释", "Show All Notes", I::StickyNote, "review.showAllNotes"),
            t("convert-notes", "转换为批注", "Convert to Comments", I::MessagesSquare, "review.convertToComments"),
        ],
        "freeze" => vec![
            t("freeze-selection", "冻结窗格", "Freeze Panes", I::Table, "view.freezePanes").params("{\"atSelection\":true}"),
            t("freeze-row", "冻结首行", "Freeze Top Row", I::Rows3, "view.freezeTopRow"),
            t("freeze-column", "冻结首列", "Freeze First Column", I::Columns3, "view.freezeFirstColumn"),
            t("unfreeze", "取消冻结窗格", "Unfreeze Panes", I::Table, "view.unfreezePanes"),
        ],
        _ => Vec::new(),
    }
}
