# 桌面共享组件验证

## 保存更改提示框

同一份 Rust 源码在 Apple M4 macOS 27.2.1 与 UOS 25 x86_64 X11／DDE 构建，工具链为 Rust 1.98.1。Word 和 Sheet 的正常入口均使用公共面板。分别以独立配置及临时文档实际启动桌面，并保存程序指纹。Word 发送系统键盘事件，Sheet 用生产窗口的 GPUI 指针事件点击实测坐标处的按钮。

| 平台 | Word | Sheet | Deck | 公共 UI 与三个产品 UI 专项 |
| --- | --- | --- | --- | --- |
| macOS | 3,655 通过，2 忽略 | 1,440 通过，1 忽略 | 984 通过 | 75 通过 |
| UOS | 2,999 通过，5 失败，2 忽略 | 1,440 通过，1 忽略 | 989 通过 | 79 通过 |

Word 核对 Tab、反向 Tab、Esc 保留修改和原件，随后在弹框确认保存和替换。独立解析 DOCX，并在新进程重开。Sheet 在另一个工作簿窗口仍打开时核对取消、失败保留草稿、重试、只关闭目标、XLSX 重开和放弃后的原文件摘要。UOS 全量 Word 的五个失败测试名称逐项与已登记的排版基线对应。[本次机器记录](dialog.json) · [UOS 排版基线](../20261116-gpui-kit-091/baseline-layout.log)

复跑时使用未被占用的输出路径，在已解锁的图形桌面执行。macOS 需允许系统键盘事件，UOS 使用已登录的 X11 会话。

```sh
cargo build --locked -p joyword -p joysheet --features joysheet/automation
python3 scripts/verify_save_changes.py word target/debug/joyword target/save-dialog-word
python3 scripts/verify_save_changes.py sheet target/debug/joysheet target/save-dialog-sheet
cargo xtask check joyword
cargo xtask check joysheet
cargo xtask check joydeck
```

## WMF／EMF 有界读取

从保存提示框提交构建基线，再从共享读取实现构建候选。两台机器上分别以同一字体、工具和语料运行四个文档的产品 CLI。每份文档提取一页场景 PNG、导出 PDF，并用 Poppler 将 PDF 解码为 RGB。脚本先确认每张图包含指定颜色的不对称三角形，再对同机基线与候选比较全部 RGB 字节。两个平台各 8 项一致。此处比较的是应用页面场景与 PDF 光栅。[像素、测试计数和失败项](meta.json)

```sh
python3 scripts/verify_shared_metafile.py /path/to/baseline/debug /tmp/joyoffice-old-images
python3 scripts/verify_shared_metafile.py /path/to/candidate/debug /tmp/joyoffice-new-images --baseline /tmp/joyoffice-old-images/pixels.json
cargo test --locked -p joyoffice-metafile -p joyword-metafile -p joydeck-metafile
```

两份私有实现已改为导入公共读取器，输入预检、裁剪和 EMF+ 的调用路径与原件相同。新库覆盖负坐标、未对齐字段、记录截断和长度溢出。Word／Deck 的 GUI 与 CLI 普通依赖均可达该库，三个 CLI 的闭包均无 GPUI。可在单一产品恢复改动前的 `bytes.rs` 并用同一语料重放。
