# R3 原生 Unicode 与候选框复验

2026-11-05，Apple M4／macOS 27.2，1× 桌面。仓库外解压 ARM64 Release 验收包 `joydeck-0.8.0-r3-unicode-final.zip`，核对 1,805 项包内文件。归档 SHA-256 为 `3e99bbdfd471fa7800ccb41b8860d6ae194764f7a7728dc95473d96cff74d5f6`。[样例](unicode-input.pptx) · [验证数据](verification.json)

## 输入范围与提交

- 原生 UTF-16 范围先转成 UTF-8 标量边界。查询返回准确范围内的周围文本，包括单独的组合字符。越界或落在代理对内部的范围安全拒绝。普通编辑中的选择和替换扩到完整字素，组合内部允许输入法按标量调整。
- AppKit 更新已有 marked text 时以组合起点换算替换范围。输入法提供的选区先按原 UTF-16 换算，再映射 CRLF 规范化后的字节。回归覆盖相对替换、换行、非 BMP、分解重音、emoji ZWJ 与组合取消。[Apple 文本输入接口](https://developer.apple.com/documentation/appkit/nstextinputclient/setmarkedtext(_:selectedrange:replacementrange:)) · [Unicode UAX #29](https://www.unicode.org/reports/tr29/)
- 候选框和原生选择范围共用文字排版到窗口的变换。跨行范围使用首行几何作为候选锚点，窗口外命中返回空。[Apple 候选坐标接口](https://developer.apple.com/documentation/appkit/nstextinputclient/firstrect(forcharacterrange:actualrange:))
- 桌面控制请求只在引擎模型或选区真实变化时刷新。组合中请求返回 `busy`，原位草稿保持可继续选词。组合提交仍使用引擎 `set-text` 事务。

## macOS 实屏

| 路径 | 实测结果 |
| --- | --- |
| Unicode 删除与替换 | 退格删除一个补充平面字符，向前删除移除整个分解重音字素。选择完整 ZWJ emoji 后，通过系统剪贴板粘贴单个补充平面字符。三次操作各提交一次，读回 `A🔬Z`。[原样例选区](01-unicode-selection.png) · [替换结果](02-unicode-replacement.png) |
| 简体拼音一 | 从系统输入菜单选择简体拼音，再在画布进入原位编辑。输入 `ni`，真实候选显示“你”。高亮候选左上角相对组合前插入点底部为 `(-14.49, +9)` px。数字键选词得到 `A🔬你Z`。[插入点](03-first-caret.png) · [候选](03-first-candidate.png) · [提交](04-first-commit.png) |
| 简体拼音二 | 换到另一文字框并放大到 124%。`zhong` 的高亮候选相对插入点为 `(-14.50, +11)` px，选词得到 `中远处文本`。两次组合期间的控制查询均返回 `busy`，随后每次选词各提交一个修订。[放大插入点](05-second-zoomed-caret.png) · [候选](05-second-zoomed-candidate.png) |
| 撤销与保存 | 第二次选词用实际编辑菜单执行单次撤销和重做。原生另存后，包内 CLI 独立读回两处文本与格式，原 PPTX 散列一致。[另存结果](06-saved.png) |

JoyDeck 产品检查 914 项通过，0 失败，0 忽略。包内 CLI 的 R1/R2/R3 共 28／20／20 项复验和 329 条注册表核对通过。候选截图采集自 macOS 桌面，键鼠由系统事件驱动。JD-304 已在 macOS 复验组合生命周期和 ABC 重音。Windows/Linux 实屏继续由平台任务跟踪。[生命周期证据](../R3-IME-LIFECYCLE/README.md)
