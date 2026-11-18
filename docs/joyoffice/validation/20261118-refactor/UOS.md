# UOS 25 桌面补验

2026-11-18 在 UOS Desktop 25 Professional x86_64 的已登录 X11／DDE 桌面经 SSH 操作。使用 Rust 1.98.1，在独立源码目录构建 `06aff812` 加首轮改动。三产品使用仓库语料的副本及隔离配置。输入事件来自 X11 合成键鼠。

## 结果

| 场景 | 实测结果 |
| --- | --- |
| Word DDE 另存为 | **连续两轮通过**。F12 后取消保留草稿。选择 `.txt` 时 Word 记录 `job-rejected`，草稿仍在。再次选择“重构 副本.docx”，对应 `picker-selected → write-start → readback-verified`。独立解析 DOCX 读到本次修改，新进程重开一致，源文件哈希相同。 |
| Word 普通保存 | **通过**。从中文空格路径打开副本，经 X11 编辑、撤销重做及查找，用 Ctrl+S 并确认替换。读取 DOCX XML、重新启动产品读回和语料哈希核对均通过。 |
| Deck DDE 另存为 | **通过**。同一桌面的原生选择器接受中文及空格文件名，独立解析输出确认 3 张幻灯片。 |
| 系统中文输入 | **未通过**。最小 GPUI 输入框超时等待 marked text，Word 正文、Deck 备注、Sheet A1 都收到直通拼音。Sheet 记录 `composing=false`，提交值为 `zhongwen1`。 |
| Sheet 编辑与重算 | **通过**。125% 下分别验证冻结首行／首列的合成组合态、提交和撤销。多行编辑保留 LF、CRLF、空行及样式，存为 XLSX 后重开一致。默认构建 100k 行桌面重算运行 20 次，每次求值 100,001 格，撤销、重做及重开通过。CLI 复核输出文件的 100,001 个公式结果全部匹配，XLSX 往返差异为 0。 |

另存脚本初次复跑遇到 DDE 延后选中默认文件名的主体，保留了已有扩展名。最终脚本在文件名框稳定后用 Home／Shift+End 选中整行，逐个输入字符，并核对输出的精确文件名。截图保存了提交前的原生文件名框。[Word DDE 对话框](uos-word-dde.png) · [机器结果](uos-local.json)

## 输入法故障定位

桌面会话使用 `LANG=zh_CN.UTF-8`、`XMODIFIERS=@im=fcitx`，实际探针进程有效 LC_CTYPE 为 `C.UTF-8`。X11 根窗口列出两个 `@server=fcitx` 条目，Xlib 为该 atom 查询到的 selection owner 为 **0**，`XOpenIM` 返回空。请求 Fcitx5 打开当前 X11 连接时，守护进程回报连接已存在。最小 GPUI 探针在明确配置 XMODIFIERS、临时切换 Shift 以及移除 XMODIFIERS 的对照中均无预编辑。

| 对照 | 默认输入模式 | 按左 Shift 切换后 |
| --- | --- | --- |
| GTK Entry，经 Fcitx D-Bus | `zhongwen1` | `中文` |
| Xlib 客户端 xterm | `zhongwen1` | `zhongwen1` |

GTK 对照确认了搜狗中文引擎能提交文字。当前桌面的 XIM 连接缺少可用的 selection owner，这使走 XIM 的 GPUI 输入路径无法开始组合。UOS 系统拼音仍待 XIM 服务恢复后的同机验证，Sheet 的合成组合态单独记录。

2026-11-18 12:26（UTC+8）在同一 X11 桌面只读复查。使用 `XMODIFIERS=@im=fcitx` 初始化 Xlib 后，`XGetSelectionOwner(@server=fcitx)` 仍为 `0x0`，`XOpenIM` 仍返回空。本次复查仅查询会话状态，上表的产品输入结果来自此前的实机操作。

同一已登录的 UOS 桌面可以按以下命令复跑，用新输出目录保存结果。脚本仅在测试窗口获得焦点时发送按键。

```sh
cargo build --locked -p joyoffice-ui --example ime_probe
cargo xtask build joyword
python3 scripts/probe_native_ime.py target/debug/examples/ime_probe target/uos-ime-02
python3 scripts/joyword/probe-dde-save.py target/debug/joyword target/uos-dde-02
```

## 工程检查

| 命令 | UOS 结果 |
| --- | --- |
| `cargo xtask check joyword` | fmt、Clippy 通过。2,998 项通过、2 项忽略，在 5 项排版测试失败后停止。 |
| `cargo test --locked -p joyword-ui-gpui` | 补跑 32 项全部通过，包含另存重复请求、取消后重试和过时回调。 |
| `cargo xtask check joydeck` | fmt、Clippy 和 988 项测试通过。 |
| `cargo xtask check joysheet` | fmt、Clippy 和 1,439 项测试通过，1 项忽略。 |
| `cargo test --locked -p joysheet-calc -p joysheet-engine --features joysheet-calc/scalar-pilot` | 696 项通过，1 项忽略。 |

Word 的 5 项失败与已登记的符号项目符号行高、Word 2013 两端对齐、浮动图片重排、旧表单字段及长脚注分页一致。[既有排版基线](../20261116-gpui-kit-091/baseline-layout.log)
