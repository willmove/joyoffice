# M3 解锁后 macOS 验证与修复

日期：2026-10-30。用户确认桌面已解锁后，**此前无活动窗口阻断已解除，更新后的最终分发包 M0/M1/M2/M3 adapter 回归、系统拼音集成和 35 项测试均通过**。这仅关闭本页列明的本机条件；完整 M3 以及 M0–M2 全部退出条件仍以 [23 遗留矩阵](23-m3-implementation-report.md) 为准。

## 本轮修复

1. **画布输入层位置**：绝对定位的 canvas 原先没有 top/left 锚点，有预览图时会被布局到页面下方，导致文字/光标和指针坐标偏移。补齐锚点后，真实 OS 输入和截图确认文字与选区落在文本框内。此前基于同一错误 origin 的 adapter 驱动未发现该问题，故不能用它替代真实坐标证据。
2. **macOS 占位字体**：`.LastResort` 声称覆盖全部 Unicode，CPU fallback 会提前选择它，显示类别方框。字体数据库现在排除此占位 face；真实缺字仍保留为缺字，不能以方框假装正确字体。
3. **混排 emoji**：锁定 usvg 的 fallback 在换字体导致 glyph 数量变化时可能放弃替换。按完整 emoji grapheme 检查系统字体覆盖并预先分出字体 span，避免拆开 ZWJ/组合序列，不修改文档的源字体声明。新增 glyph 测试确认中英、emoji、组合音标无 glyph 0、无 LastResort 占位。
4. **焦点和选区**：窗口失活只取消未提交 composition，保留活动文字与选区；修正反向拖选方向和多行反向 caret 的行归属。切回应用可继续编辑。
5. **文字布局缓存**：按对象内容/样式/几何、scale 和窗口位置缓存活动文本 ShapedLine。native 测试移动光标并等待重绘，确认 shaping 计数不增长。此缓存不等于已经完成 CPU/编辑统一排版。

## 自动检查与最终分发包

- `cargo xtask check`：fmt、`clippy --workspace --all-targets --locked -- -D warnings` 与 **35 项测试通过**；[日志](evidence/legacy-logs/m3-unlocked-check.log)。
- `cargo xtask stage M3`：本机实际链接和 workspace tests 通过；[日志](evidence/legacy-logs/m3-unlocked-stage.log)。
- 最终分发二进制执行 M0/M1/M2/M3 adapter 回归全部通过；[汇总](evidence/legacy-logs/m3-verified-package-native.log)，[M0](evidence/legacy-logs/m0-native-darwin.log)、[M1](evidence/legacy-logs/m1-native-darwin.log)、[M2](evidence/legacy-logs/m2-native-darwin.log)、[M3](evidence/legacy-logs/m3-native-darwin.log)。
- M3 原生日志含 `painted_lines=2 complete=true`、`overlay_shape_cache=true`。M2 包含保存重开、恢复、dirty close 取消/保存/放弃与旧快照保存后仍 dirty。
- 分发包 31 个文件 SHA-256、ZIP CRC 和分发二进制与最后构建逐字节一致性检查通过。包未签名/公证，未验证干净机器安装。

环境：macOS 27.0 / Darwin ARM64、Rust 1.98.1、Kit 0.8.1 / GPUI pre 0.3.10，debug 构建；无 release 性能结论。

## 实际系统拼音集成

运行 `python3 scripts/smoke-macos-ime.py --binary dist/joydeck-m3-verified-darwin-arm64/joydeck`，使用独立测试窗口和系统输入源 `com.apple.inputmethod.SCIM.ITABC`。通过 OS 键盘事件输入 zhongwen、Space 提交“中文”，再组合 quxiao 并 Esc 取消；由真实 NSTextInputClient/IME 链路进入生产输入桥。不是直接调用 replace 方法，也不冒充物理键盘或 Windows/Linux/日文/韩文验收。

最终包 [测试输出](evidence/legacy-logs/m3-verified-package-pinyin.log)、[native 日志](evidence/m3/pinyin-1793315456742457000/native.log)、[结果](evidence/m3/pinyin-1793315456742457000/result.json)、[保存重开样例](evidence/m3/pinyin-1793315456742457000/roundtrip.joydeck)：

- `marked_calls=8 commit_calls=1`，预编辑不修改文档，提交只需一次撤销。
- 下一次组合取消，文档内容与 revision 不增加；撤销回到空文本，重做恢复“中文”。
- 保存重开模型一致，系统输入源在 finally 恢复为运行前的 ABC；复核已恢复。
- [真实候选窗](evidence/m3/pinyin-1793315456742457000/candidate.png) 位于画布文字旁；[提交后窗口](evidence/m3/pinyin-1793315456742457000/committed.png) 已检查。

最初仅从后台 CLI 选择输入源的尝试未产生 composition，保留失败记录；最终方案在测试前台窗口通过 OS 切换输入源，并记录实际输入源 ID。测试工具不从字面输入结果推断真实 IME 已通过。

## 图像核对

最终包的 [编辑态](evidence/m3/verified-active.png) 与 [静态 CPU 预览](evidence/m3/verified-static.png) 均已实际截图并检查：中文、彩色 emoji、组合音标及第二行可见，无先前类别方框；文字不再被画到页面下方。编辑态与静态排版仍存在度量差别，**统一文字布局仍是软件阻断项**。

## 产物与剩余条件

- [已验证 macOS ARM64 开发包（旧归档：joydeck-m3-verified-darwin-arm64.zip）](../joyoffice/JOYDECK-ARCHIVES.md)。旧 preview 包保留用于对照。
- [本轮源码快照（旧归档：joydeck-m3-verified-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md)，以及 [M3 开发前回退点（旧归档：joydeck-before-m3-source.tar.gz）](../joyoffice/JOYDECK-ARCHIVES.md)。无 Git 元数据，未伪造提交。
- QA 脚本：`scripts/smoke-native.py`、`scripts/smoke-macos-ime.py`、`scripts/macos-input-source.c`；打包脚本支持 `--name`，仍拒绝覆盖已有目录。

未完成：统一 CPU/编辑布局、自动换行、完整段落/列表/缩进/段距/混合字号和文字装饰、旋转输入、完整 bidi/视觉行导航、其他脚本与嵌入字体、其他平台/输入法、物理键盘、跨屏 DPI、读屏；M1 LARGE/资源诊断与完整故障/断电/其他平台门槛也未因此关闭。不能将本机原生通过等同于整个 M3 或此前全部收尾完成。
