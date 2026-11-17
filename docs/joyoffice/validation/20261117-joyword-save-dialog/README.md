# JoyWord 未保存更改弹框

2026-11-17；分支 `fix/joyword-unsaved-dialog`，基于 main `eb551e36622c32feaebcdb029d1b06abb26aea26`。本次在独立 worktree 开发，原工作区的 Ribbon 等未提交修改未动。

## 实现

- JoyWord 原顶部未保存提示条改为与 JoySheet `render_close_dialog` 相同的居中弹框：450 逻辑像素宽、24 像素内边距、圆角、阴影和背景遮罩，右下角按“取消／不保存／保存”排列。
- 标题为“保存更改？”。关闭文档提示“关闭文档前可以先保存。”；新建、打开或切换文档提示“继续前可以先保存此文档。”。保留英文与当前 JoyWord 主题，保存按钮使用 JoyWord 主题强调色。
- 文件页、文档编辑页及退出请求共用此弹框。焦点进入弹框，Tab 留在框内，Esc 取消；背景的键盘、菜单和拖入操作不能覆盖待处理决定。保存期间禁用操作，失败时继续保留修改和错误提示，成功并确认文档已无修改后才继续原操作。
- 原文件替换的既有元数据确认在弹框内显示，保留“另存副本／确认替换原文件”和返回上一级选择的取消入口。

未改变 DOCX 保存格式、编辑引擎、依赖版本或 JoySheet 代码。

## 自动验证

本机 macOS ARM64、仓库 Rust 1.98.1 工具链：

| 命令 | 结果 |
| --- | --- |
| `cargo fmt -p joyword-ui-gpui -- --check` | 通过 |
| `cargo clippy --locked -p joyword-ui-gpui --lib -- -D warnings` | 通过 |
| `cargo test --locked -p joyword-ui-gpui` | 24 通过、0 失败 |
| `cargo xtask build joyword` | 桌面和 CLI 构建通过 |
| `python3 scripts/check_docs.py` | 通过 |

新增 GPUI 回归覆盖编辑页与文件页弹框居中、背景菜单隔离、Tab 焦点约束、Esc 取消、点击不保存、保存失败保留修改及原文件不变、弹框内确认替换并成功保存后关闭、DOCX 保存读回。既有关闭取消／丢弃和小栈保存回归一同通过。

## 本机实屏

使用独立配置与 `assets/samples/letter.docx` 的临时副本，不操作用户文档。通过原生键鼠完成：

| 场景 | 观察结果 |
| --- | --- |
| 文件页关闭未保存文档 | 居中显示三按钮弹框；点击取消后文档仍有修改。[截图](close-dialog.png) |
| 保存后关闭 | 弹框内可确认替换原文件，保存成功后回到无活动文档的文件页。[确认截图](original-replacement.png) |
| 新建前放弃修改 | 编辑页出现相同弹框；点击不保存后进入空白新文档。[截图](new-document-dialog.png) |
| 独立 XML 读回 | 临时 DOCX 中包含 `Save dialog validation`，不包含随后丢弃的 `Discard validation`。 |

2026-11-17 UOS Linux 已复验取消、保存并确认替换及放弃后新建，见[三产品记录](../20261117-uos-desktop/README.md)。Windows、实际读屏、系统退出实屏、原生另存为取消和平台全量回归仍未覆盖；JoyWord 原生另存异常单独保留，不据此关闭这些范围的验收任务。
