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

## 原生另存目标

Sheet 的两个保存入口在原生面板打开前捕获窗口、工作簿及修订号。单写者在写入前核验三者，面板从已有文件的目录打开。macOS 和 UOS 的真实另存面板均通过取消后重试。面板打开时修改 A1，接受的过时文件名没有产生文件，草稿仍在。再次选择中文带空格文件名后，独立解析 XLSX 并在新桌面进程读回。单写者专项另验文档已关闭、同修订号的其他文档以及切换活动窗口。Deck 的取消、失败和成功回调均绑定文稿代次和读取请求，接受时另核对修订号。两端 Deck 均取消后再次另存，并在新进程读回三页演示文稿。[结果和产品回归](save-target.json)

```sh
python3 scripts/verify_native_save_target.py sheet target/debug/joysheet /tmp/joyoffice-sheet-panel
python3 scripts/verify_native_save_target.py deck target/debug/joydeck /tmp/joyoffice-deck-panel
```


### UOS 中文输入

UOS 25 x86_64 的 DDE 用户会话、物理显示器 `:0`、系统 Fcitx5 和发行版拼音引擎。专用 XIM 名称、D-Bus 和临时用户配置使本次测试与桌面用户的搜狗服务分别运行。测试使用 X11 系统按键，生成中的拼音由 Fcitx 候选窗口显示。截图来自被测窗口范围。独立拼音使用 XIM 的 on-the-spot 设置；首测时默认搜狗服务的 selection 所有者为 0；后续按用户授权重启。Fcitx 按 XMODIFIERS 注册 XIM 名称。[Fcitx XIM 源码](https://github.com/fcitx/fcitx5/blob/5.1.12/src/frontend/xim/xim.cpp)

同一桌面、同一拼音，改动前 Word 和 Deck 通过，Sheet 提前发送 XIM_SET_IC_VALUES，连 XIM_OPEN 尚未完成，键入后单元格为 `zhongwen`。修复后最小控件收到 marked text。三个应用选词“中文”，Esc 取消第二段，切换焦点、撤销、重做后保存，独立读取文件包并用新进程核对编辑结果。每个产品记录二进制、输出文件和候选截图 SHA-256。[机器结果](uos-ime.json) · [Word 正文](uos-word-candidate.png) · [Deck 备注](uos-deck-candidate.png) · [Sheet 单元格](uos-sheet-candidate.png)

安装或从 UOS 发行版软件包解出拼音插件和词库后，在已登录桌面的环境中运行 `cargo build -p joyoffice-ui --example ime_probe` 以及三产品开发构建（Deck 启用 validation，Sheet 启用 automation），再执行 `sh scripts/verify_uos_pinyin.sh /abs/path/to/target/debug /tmp/new-ime-report`。该脚本创建独立的 Fcitx 配置与进程，退出时释放其 selection。要检查现有输入法，可分别执行 `python3 scripts/verify_uos_native_ime.py word /path/to/joyword /tmp/new-word-report --method pinyin` 等命令。回退 Linux 兼容补丁时同时恢复 Cargo.toml 的 registry 依赖和 Cargo.lock 校验值，再用相同输入法复跑。

**默认搜狗复验。** 重启 DDE 管理的用户 Fcitx5 服务后，默认 `@server=fcitx` 的所有者由 0 变为有效窗口，XOpenIM 从失败转为成功。默认配置由服务端显示预编辑，候选期间控件内容维持原值，按空格才收到“中文”。在该配置下最小控件和 Word、Deck、Sheet 均通过选择、取消与撤销；三产品另验重做、落盘和新进程读回。测试结束后恢复了原来的键盘输入源与未激活状态。复测命令加 `--method com.sogou.ime.ng.fcitx5.uos --server-preedit`。[默认服务记录](uos-sogou.json) · [Word](uos-sogou-word.png) · [Deck](uos-sogou-deck.png) · [Sheet](uos-sogou-sheet.png)

## 包读取与字体

普通 Word 与 Deck 入口将已验证的包成员移交给各自的格式 reader。成员名、长度和 CRC 不符时回到原解压路径，格式层继续执行自己的预算检查。Word 专项的重复解压从 120,016 字节降到 0，改坏一个成员缓存只重新解压该成员。两代保存和旧版读回覆盖批注／脚注、OMML、OLE、图表和 SmartArt 语料，按脚本实际列出的对象对照。[逐项结果](complex-docx.json)

同机交替执行普通 CLI 打开六轮，每轮保留峰值 RSS 和输出文本摘要。12 MiB 不透明部件样例的最高 RSS 在 macOS 上由 72.88 降至 62.70 MiB，在 UOS 上由 58.83 降至 56.84 MiB。对应打开时间中位数分别为 170.14 → 177.23 ms 和 87.81 → 83.36 ms。两端的页面场景及 PDF 各八项像素与同机基线匹配。复跑入口为 `scripts/joyword/measure-opc-open.py`、`scripts/joyword/check-complex-corpus.py` 与 `scripts/verify_shared_metafile.py`。[macOS](macos-opc.json) · [UOS](uos-opc.json) · [固定字体输出](font-pixels.json)

## Sheet 工作簿与重算

两份用户授权工作簿各自用默认与候选 CLI 验证往返，原始工作表没有公式。脚本在临时副本中按实际数值位置加入算式和汇总，生成独立期望缓存，逐格核对两种实现及往返结果。两端分别匹配 916/916 和 16/16。报告只记录摘要、计数和计时。复跑使用 `scripts/joysheet/verify-business-workbooks.py --help`。桌面扇出测量另用 `scripts/joysheet/bench-fanout.py`，每组 20 次编辑，验证撤销、重做和 XLSX 重开。[业务结果](business-workbooks.json) · [历史基线与候选](recalculation.json)
