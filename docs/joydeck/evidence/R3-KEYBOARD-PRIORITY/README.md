# JD-308 字符输入与画布快捷键优先级

2026-11-06，Linux x86_64 无桌面验证，基于主干 `7b37ad4c884706087983d352d4a11148040de02a`。本批独立于剪贴板分支，无 UI 布局或新增控件变更。

## 发现与实现

源码核对发现 `Viewer::inline_key` 只检查画布焦点，然后把 `control || platform` 当作命令修饰键，未检查 `KeyDownEvent::prefer_character_input`。例如一个焦点在画布、`key = v`、Ctrl+Alt 且 `prefer_character_input = true` 的事件，会进入粘贴分支并停止传播；`x`、`b`、`i`、`u` 等也可能触发剪切或字符格式命令。此项是源码路径确认，没有宣称在真实键盘上复现。

核对仓库固定依赖 `gpui-pre 0.3.10` 的 `src/interactive.rs`：该字段用于含 AltGr 等修饰键的字符输入。其 `src/window.rs` 中 `input_preference` 会跳过绑定动作，但 `finish_dispatch_key_event` 仍调用 `dispatch_key_down_up_event`，因此产品的原始按键回调也必须尊重该标记。实现独立编写，未复制依赖的编辑器代码。

新增纯按键分流函数，并在 `inline_key` 的任何保存提示、字号、剪贴板、对象编辑或文本导航之前调用：

- 平台声明字符优先时直接返回，不调用编辑命令，也不停止事件传播。
- Ctrl/Command 与 Alt 同时按下时不执行画布快捷键，覆盖没有字符优先提示的组合；现有画布快捷键没有定义这些组合。
- 普通 Ctrl/Command 快捷键、无修饰导航及单独 Alt 的既有路径保留。原焦点检查和组合输入保护保留。

## 验证

- 直接包含生产 `inline_text/key_route.rs` 的临时 Rust 测试宿主运行 3 项测试，全部通过：字符优先覆盖所有修饰组合、Alt 与命令修饰键组合放行、普通快捷键／导航保持原分流。
- `RUST_FONTCONFIG_DLOPEN=1 cargo clippy -p joydeck-ui-gpui --all-targets -- -D warnings` 通过，包含完整 Linux 桌面 crate 和测试目标的编译检查。使用 Rust 1.98.1、关闭 debug 信息与增量编译；动态加载设置沿用当前环境缺少 `fontconfig.pc` 的处理。
- `cargo fmt --all --check`、`python3 scripts/check_docs.py` 通过。

未运行真实键盘 AltGr／Option、原位文字／备注／表格焦点切换、快捷键事件注入或原生 IME 验收；Windows/macOS 构建也未在本环境执行。上述测试验证分流策略和 Linux 编译，不替代完整事件链与真实桌面验收。JD-308 仍为已实现待验收，JD-305 整份页脚与其余格式命令保持开放。
