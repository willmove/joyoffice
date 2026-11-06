# JD-305 清除字符格式时保留文本行为

2026-11-06，Linux x86_64 无桌面验证，基于主干 `7b37ad4c884706087983d352d4a11148040de02a`。独立修复分支，不包含此前剪贴板或按键优先级分支。

## 问题与处理

Ribbon「清除字符格式」原先把整份 `RunProps` 替换为默认值，除外观外也删除了点击超链接、悬停动作、语言和免校对设置。JD-305 要求保持源内容与可撤销格式编辑；本次把该命令限定为清除视觉覆盖，保留 `link`、`hover`、`lang`、`no_proof`。专门的链接编辑／移除入口继续负责改变导航行为。

新增引擎 `formatting::clear_character_format`，现有 Ribbon 通过 `format_runs` 调用它，沿用文字选区、插入点输入格式、整对象／表格目标和原有事务路径。字体、字号、字重、颜色、下划线等显式视觉属性恢复默认继承；不改变正文、字段种类和段落格式。没有新增 UI 或调整布局。

## 回归范围

先将原有全量重置操作提取到 helper，新增回归实际失败，显示链接、悬停、语言和免校对全部变为 `None`。随后改为保留这些语义属性。

- 引擎回归：中文、emoji、普通文字和幻灯片编号字段；确认视觉覆盖清除、两类动作和校对信息保持、字段种类保持；通过实际 `SetTexts` 会话验证单次修订、撤销和重做。
- Shell 回归：生成带点击网页链接、悬停页内动作及语言／校对属性的 PPTX，导入后清除格式，走 `export_preserving` 保存；并保存原生格式。两种格式重开，确认文本和语义属性一致，粗体及 32pt 显式覆盖已清除。原始 PPTX 仍可读回相同基线。

真实桌面点击按钮、插入点后继续输入、混合选区／多对象／表格操作仍待验收；本次不宣称完成 JD-305，整份页脚及其余格式命令继续开放。

验证结果：`cargo test -p joydeck-engine -p joydeck-shell` 共 **87 项通过**，0 失败、0 忽略（含两项新增回归）。`RUST_FONTCONFIG_DLOPEN=1 cargo clippy -p joydeck-engine -p joydeck-shell -p joydeck-ui-gpui --all-targets -- -D warnings`、`cargo fmt --all --check` 和 `python3 scripts/check_docs.py` 均通过。使用 Rust 1.98.1，测试／检查关闭 debug 信息和增量编译；Fontconfig 动态加载配置用于当前环境缺少开发文件的 Linux 编译检查。依赖及锁文件未改变。
