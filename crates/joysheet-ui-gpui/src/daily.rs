use crate::desktop::{After, Desktop};
use gpui_kit::component::button::Button;
use gpui_kit::{prelude::*, *};
use serde_json::json;
impl Desktop {
    pub fn daily_panel(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let group = self.daily_group;
        let actions: &[(&str, &str)] = match group {
            1 => &[
                ("单元格格式…", "home.formatCells"),
                ("文字方向", "home.orientation"),
                ("字体", "home.fontName"),
                ("字号", "home.fontSize"),
                ("文字颜色", "home.fontColor"),
                ("填充颜色", "home.fillColor"),
                ("数字格式", "home.numberFormat"),
                ("边框", "home.borders"),
                ("左对齐", "home.alignLeft"),
                ("居中", "home.alignCenter"),
                ("右对齐", "home.alignRight"),
                ("顶部对齐", "home.alignTop"),
                ("垂直居中", "home.alignMiddle"),
                ("底部对齐", "home.alignBottom"),
                ("自动换行", "home.wrapText"),
                ("缩小字体填充", "home.shrinkToFit"),
                ("合并居中", "home.mergeCenter"),
                ("取消合并", "home.unmergeCells"),
                ("自动列宽", "home.autofitColumnWidth"),
                ("列宽", "home.columnWidth"),
                ("行高", "home.rowHeight"),
            ],
            2 => &[
                ("添加工作表", "home.insertSheet"),
                ("重命名工作表", "sheet.rename"),
                ("移动或复制工作表", "sheet.move"),
                ("隐藏工作表", "sheet.hide"),
                ("取消隐藏工作表", "sheet.unhide"),
                ("删除工作表…", "home.deleteSheet"),
                ("插入行", "home.insertRows"),
                ("删除行", "home.deleteRows"),
                ("插入列", "home.insertColumns"),
                ("删除列", "home.deleteColumns"),
                ("隐藏行", "home.hideRows"),
                ("显示行", "home.unhideRows"),
                ("隐藏列", "home.hideColumns"),
                ("显示列", "home.unhideColumns"),
            ],
            3 => &[("当前工作簿新视图", "view.newWindow")],
            4 => &[
                ("排序…", "data.sort"),
                ("开启/关闭筛选", "data.filter"),
                ("按列筛选…", "data.filterBy"),
                ("清除筛选", "data.clearFilter"),
                ("查找…", "edit.find"),
                ("替换…", "edit.replace"),
                ("去除重复项…", "data.removeDuplicates"),
                ("填充所选区域…", "range.fill"),
            ],
            5 => &[
                ("创建表格", "insert.table"),
                ("表格样式", "table.style"),
                ("汇总行", "table.totalRow"),
                ("转为区域", "table.convertToRange"),
                ("条件格式规则", "home.conditionalFormat"),
                ("查看条件格式规则", "home.manageRules"),
                ("清除条件格式", "home.clearRules"),
                ("数据验证", "data.validation"),
                ("查找无效数据", "data.circleInvalid"),
            ],
            6 => &[
                ("建立分组", "data.group"),
                ("取消分组", "data.ungroup"),
                ("折叠分组", "data.hideDetail"),
                ("展开分组", "data.showDetail"),
                ("显示大纲层级", "data.showLevels"),
                ("拆分窗格（冻结语义）", "view.split"),
                ("冻结首行", "view.freezeTopRow"),
                ("取消冻结", "view.unfreezePanes"),
            ],
            7 => &[
                ("导入 CSV / TSV / JSON", "data.getData"),
                ("从当前表或区域导入", "data.fromTableRange"),
                ("查询与连接清单", "data.queriesConnections"),
            ],
            10 => &[("插入图表", "insert.chart"), ("图表类型", "chart.changeType"), ("切换行列", "chart.switchRowColumn"), ("图表标题", "chart.set")],
            11 => &[
                ("插入图片", "insert.picture"),
                ("插入形状", "insert.shape"),
                ("插入文本框", "insert.textBox"),
                ("插入迷你图", "insert.sparkline"),
                ("插入复选框", "insert.checkbox"),
            ],
            12 => &[
                ("位置与尺寸", "object.move"),
                ("对象锚定方式", "object.setAnchorMode"),
                ("文本框内容", "shape.setText"),
                ("形状填充", "chart.shapeFill"),
                ("形状边线", "chart.shapeOutline"),
                ("上移一层", "arrange.bringForward"),
                ("下移一层", "arrange.sendBackward"),
                ("删除对象", "object.delete"),
            ],
            13 => &[
                ("页面设置", "pageLayout.pageSetup"),
                ("打印预览", "file.printPreview"),
                ("系统打印…", "file.print"),
                ("导出 PDF（指定路径）", "file.exportPdf"),
            ],
            14 => &[
                ("创建透视表", "insert.pivotTable"),
                ("透视字段布局", "pivot.fields"),
                ("透视表清单", "pivot.list"),
                ("汇总与显示方式", "pivot.valueSettings"),
                ("透视报表布局", "pivot.layout"),
                ("透视筛选", "pivot.filter"),
                ("透视排序", "pivot.sort"),
                ("计算字段", "pivot.calculatedField"),
                ("刷新透视表", "pivot.refresh"),
                ("更改透视源", "pivot.changeSource"),
                ("数据分析工具库", "data.analysisToolPak"),
                ("单变量求解", "data.goalSeek"),
                ("规划求解", "data.solver"),
                ("保存方案", "data.scenarioAdd"),
                ("方案清单", "data.scenarioManager"),
                ("应用方案", "data.scenarioShow"),
                ("方案摘要", "data.scenarioSummary"),
            ],
            15 => &[
                ("新建批注", "review.newComment"),
                ("新建笔记", "review.newNote"),
                ("上一批注", "review.previousComment"),
                ("下一批注", "review.nextComment"),
                ("全部笔记", "review.showAllNotes"),
                ("笔记转批注", "review.convertToComments"),
                ("保护工作表", "review.protectSheet"),
                ("取消工作表保护", "review.unprotectSheet"),
                ("工作簿结构保护", "review.protectWorkbook"),
                ("允许编辑区域", "review.allowEditRanges"),
                ("定义名称", "formulas.defineName"),
                ("名称管理", "formulas.nameManager"),
                ("工作簿统计", "review.workbookStatistics"),
                ("检查辅助功能", "review.checkAccessibility"),
            ],
            _ => &[],
        };
        let mut panel = div()
            .absolute()
            .right_0()
            .top(px(164.))
            .bottom_0()
            .w(px(280.))
            .p_4()
            .bg(rgb(if self.preferences.dark { 0x20332a } else { 0xffffff }))
            .shadow_lg()
            .flex()
            .flex_col()
            .gap_2()
            .id("daily-panel")
            .occlude()
            .overflow_y_scroll()
            .child(div().id("close-daily").cursor_pointer().child("关闭 / Close ×").on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 0;
                cx.notify();
            })));
        for &(label, command) in actions {
            panel = panel.child(div().id(command).p_2().rounded_md().cursor_pointer().hover(|s| s.bg(rgb(0xe3f2e7))).child(label).on_click(
                cx.listener(move |this, _, window, cx| {
                    this.daily_group = 0;
                    if group == 2
                        || group >= 4
                        || [
                            "home.fontName",
                            "home.fontSize",
                            "home.fontColor",
                            "home.fillColor",
                            "home.numberFormat",
                            "home.borders",
                            "home.columnWidth",
                            "home.rowHeight",
                            "home.orientation",
                            "home.formatCells",
                        ]
                        .contains(&command)
                    {
                        this.open_command(command, window, cx);
                    } else {
                        this.commit(After::Command(command.into(), json!({})), window, cx);
                    }
                }),
            ));
        }
        if group >= 10 {
            panel = panel.child(Button::new("open-object-selection").label("对象多选与排列…").on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 22;
                cx.notify();
            })));
            panel =
                panel.child(div().id("object-properties").p_2().cursor_pointer().child("已选对象属性 →").on_click(cx.listener(|this, _, _, cx| {
                    this.daily_group = 12;
                    cx.notify();
                })));
            panel =
                panel.child(div().id("objects-tab").p_2().cursor_pointer().child("图片、形状与对象 →").on_click(cx.listener(|this, _, _, cx| {
                    this.daily_group = 11;
                    cx.notify();
                })));
        }
        if group >= 4 {
            panel = panel.child(div().id("import-tab").p_2().cursor_pointer().child("导入与连接 →").on_click(cx.listener(|this, _, _, cx| {
                this.daily_group = 7;
                cx.notify();
            })));
            panel =
                panel.child(div().id("outline-tab").p_2().cursor_pointer().child("分组、大纲与窗格 →").on_click(cx.listener(|this, _, _, cx| {
                    this.daily_group = 6;
                    cx.notify();
                })));
            panel = panel.child(div().id("data-tools-tab").p_2().cursor_pointer().child("表格、条件格式与验证 →").on_click(cx.listener(
                |this, _, _, cx| {
                    this.daily_group = 5;
                    cx.notify();
                },
            )));
        }
        if group == 5 {
            panel = panel.child(div().id("validation-list").p_2().cursor_pointer().child("选择验证列表中的值 →").on_click(cx.listener(
                |this, _, _, cx| {
                    this.daily_group = 9;
                    cx.notify();
                },
            )));
        }
        if group == 9
            && let Some(view) = self.view()
            && let Some(sheet) = view.sheet()
            && let Some(rule) = sheet.validations.iter().find(|r| r.ranges.iter().any(|range| range.contains(view.selection.active)))
        {
            for (i, value) in joysheet_engine::cmd::data::list_items(&view.workbook, view.sheet, rule).into_iter().take(200).enumerate() {
                panel = panel.child(div().id(("validation-item", i)).p_2().cursor_pointer().child(value.clone()).on_click(cx.listener(
                    move |this, _, window, cx| {
                        this.daily_group = 0;
                        this.begin_edit(Some(value.clone()), true, window, cx);
                        this.commit(After::None, window, cx);
                    },
                )));
            }
        }
        if group == 3 {
            for (mode, label, only) in [
                ("tiled", "平铺 / Tiled", false),
                ("horizontal", "水平 / Horizontal", false),
                ("vertical", "垂直 / Vertical", false),
                ("cascade", "层叠 / Cascade", false),
                ("tiled", "仅当前工作簿平铺 / Active workbook", true),
            ] {
                panel = panel.child(Button::new(SharedString::from(format!("arrange-{mode}-{only}"))).label(label).on_click(cx.listener(
                    move |this, _, _, cx| {
                        this.daily_group = 0;
                        this.issue("view.arrangeAll", json!({"arrange":mode,"activeWorkbook":only}), After::None, cx);
                    },
                )));
            }
            for view in &self.snapshot.windows {
                let id = view.window;
                let title = view.title.clone();
                panel = panel.child(div().id(("switch-window", id)).p_2().cursor_pointer().child(title).on_click(cx.listener(
                    move |this, _, window, cx| {
                        this.daily_group = 0;
                        this.commit(After::Command("window.activate".into(), json!({"window":id})), window, cx);
                    },
                )));
            }
        }
        if group == 8 {
            panel = panel.child("操作结果 / Results（最多 200 项）");
            for (index, (label, reference)) in self.data_results.iter().cloned().enumerate() {
                panel = panel.child(div().id(("data-result", index)).p_2().cursor_pointer().child(label).on_click(cx.listener(
                    move |this, _, window, cx| {
                        if let Some(reference) = &reference {
                            this.daily_group = 0;
                            this.commit(After::Command("edit.goTo".into(), json!({"reference":reference})), window, cx);
                        }
                    },
                )));
            }
        }
        if group == 15 {
            panel = panel.child(self.comments_list(cx));
        }
        panel
    }
}
