# 自研替换首轮验证

基线为 `06aff812`。本页记录 Apple M4、macOS 27.2.1、Rust 1.98.1 上的结果，后续 X11／DDE 补验见 [UOS 实机记录](UOS.md)。实测使用隔离配置和测试文件。桌面断言、回退互读与截图指纹见[本机复验结果](macos-local.json)，A/B 分位数及内存见[试点测量](results.json)，逐次编辑耗时见[400 条计时样本](samples.csv)。任务归属及采用条件见[路线图](../../REFACTOR-ROADMAP.md)。

## 原生桌面

- **中文输入**。最小 GPUI 输入框验证了 marked text、选词、取消、一次撤销。三个产品的实机截图均显示插入处下方的候选面板，首个候选为“中文”。在 Word 正文、Deck 备注和 Sheet 默认 A1 中分别选词、用 Escape 取消第二次组合，再以系统快捷键撤销和重做。Deck 和 Sheet 另以 Tab 离开编辑处。三份文件均由新启动的产品进程读回。事件来自系统合成按键，未测物理键盘和完整读屏。[Word 正文](word-preedit.png) · [Deck 备注](deck-preedit.png) · [Sheet 单元格](sheet-preedit-after.png)
- **Sheet 裁切修复**。同一默认列宽下，旧版 A1 已能提交“中文”，但预编辑区只露出部分拼音。当前编辑框在有 marked text 时临时扩大，并按视口和冻结区域裁切。相同按键序列复测后，整行 `zhong wen` 可见。macOS Vision 对修复前截图的整行拼音断言失败，对修复后截图通过。另以 125% 缩放分别验证冻结首行和首列的组合态、提交和撤销，以及多行 XLSX 的编辑、换行、样式与重开。[修复前](sheet-preedit-before.png) · [修复后](sheet-preedit-after.png)
- **Word 另存与普通保存**。在 AppKit 原生面板取消，草稿仍在。随后选择 `.txt` 触发 `job-rejected`，目标未创建，草稿保留。第三次选取“重构 副本.docx”后独立解析 OOXML 读回“中文”。继续编辑副本，按 ⌘S 并在保护提示中确认，新进程读回两次修改，最初的 DOCX 哈希一致。GPUI 回归另覆盖重复请求、过时结果与写入冲突。阶段日志随[本机复验结果](macos-local.json)保存。

本机复跑需解锁桌面并启用简体拼音，以及辅助功能和屏幕录制权限。按产品逐条执行，使用空闲的输出目录。新脚本直接从项目测试语料创建副本，保存输入源后在退出时恢复。对照截图期间保持键盘焦点在测试窗口。

```sh
cargo build --locked -p joyword -p joydeck -p joysheet
python3 scripts/verify_macos_native_pilots.py word target/debug/joyword target/mac-word-01
python3 scripts/verify_macos_native_pilots.py deck target/debug/joydeck target/mac-deck-01
python3 scripts/verify_macos_native_pilots.py sheet target/debug/joysheet target/mac-sheet-01
```

### 其他平台的复跑入口

- 在固定的 gpui-pre-linux 0.3.12 中，X11 的 `X11rbClient::init(..., None).ok()` 会将初始化失败折叠成无 XIM 客户端；zed-xim 0.4.0-zed 用 `XMODIFIERS` 的 `@im=` 值寻找根窗口注册的 XIM 服务。此前 UOS 记录只证明 Fcitx5 进程活动与拼音直通，还缺应用启动环境、XIM 注册和最小控件对照。Word 和 Deck 均由 GPUI 调用原生文件面板，DDE 超时发生在哪一阶段需由同机日志判定。[此前限定结果](../20261117-uos-desktop/README.md)

在解锁的 macOS 或 UOS X11 桌面，从仓库根目录运行。macOS 需要已启用简体拼音；X11 需要 Fcitx5、xdotool、xprop 和可用的简体拼音输入法。使用未占用的输出路径。

```sh
cargo build --locked -p joyoffice-ui --example ime_probe
python3 scripts/probe_native_ime.py target/debug/examples/ime_probe target/ime-comparison-01
```

在 UOS 上先按用户日常启动环境复现，比较报告中的 `XIM_SERVERS`、`XMODIFIERS` 和活动输入法。若发现实际注册名与进程环境不符，用该服务名仅对一次测试进程设置 `XMODIFIERS` 后复测。再在相同环境逐项打开 Word 正文、Deck 备注、Sheet 单元格，选词、取消、切换焦点、撤销和保存重开。保留故障时的候选窗口截图、各阶段日志及独立 OOXML 读回结果。

DDE 脚本复制仓库 DOCX，先写入测试文字，通过 F12 在已获得焦点的 DDE 文件面板中取消、重试，再核对另存副本和源文档哈希。输出目录保留阶段日志和报告。[实机结果](UOS.md)

```sh
cargo xtask build joyword
python3 scripts/joyword/probe-dde-save.py target/debug/joyword target/dde-save-comparison-01
```

## Sheet 性能与回退决定

相同构建参数，A 为普通构建，B 为启用 `scalar-pilot` 的构建。固定每个 B 列单元格为 `=$A$1+行号`，D1 为该列的 SUM。两次预热后连续改 A1 二十次，同机按 A/B、B/A 各测一遍，P95 取合并的 40 条样本中按耗时排序的第 38 条。内核样本逐格断言含汇总共 N+1 个求值；桌面样本核对末格、汇总、公式、撤销／重做、独立 XML 和应用重开。绘制指 GPUI 编辑提交到首个匹配版本的 CPU paint。

| 场景与 P95（ms） | A | B |
| --- | ---: | ---: |
| 100 行纯重算 | 0.041 | 0.063 |
| 10k 行纯重算 | 3.334 | 2.269 |
| 100k 行纯重算 | 41.624 | 34.321 |
| 100 行桌面引擎／首次绘制 | 0.208／21.694 | 0.213／15.373 |
| 100k 行桌面引擎／首次绘制 | 71.139／82.167 | 64.097／78.688 |

100k 纯重算的两轮峰值 RSS 最大值分别为 135,217,152 和 135,233,536 字节，取自 macOS `/usr/bin/time -l` 的 maximum resident set size。另以相同二进制按 A/B、B/A 复跑，计入桌面建表、编辑、保存和重开整个子进程的最高 RSS，两种配置的最大值分别为 1,152,139,264 和 1,159,036,928 字节（+0.60%）。100 行纯重算计时在几十微秒量级，候选 P95 高于基线；已保留全部样本。桌面引擎收益不足以启用默认路径。GPU 呈现和外部客户工作簿仍待实测。

两种 Release CLI 对 `docs/joysheet/samples` 的 7 份工作簿运行 `joysheet-cli check docs/joysheet/samples --json`，输出逐字节一致。7 份全部通过 XLSX 往返对照；其中 2 份含可与缓存比对的公式，共 5 格均匹配。它们补充了合成扇出样本的文件覆盖，实际业务中的公式分布仍需另采。

**文件回退实测**。默认版与试点版各取对方桌面保存的 100k 行工作簿，用 CLI 将 A1 从 22 改为 23 后另存。独立解析 XML，逐格核对 100,000 个引用表达式、数值及 D1 的 SUM（5,002,350,000）。再交回原写入方的 CLI 核对缓存和文件往返，两次各有 100,001 个公式匹配且源文件哈希相同。[双向互读结果](macos-local.json)

### 复跑

两种配置应分别保存生成的二进制，再按上述顺序运行。下面给出单机的命令，输出目录用于一次新实验。A/B 均使用同一 target 和 Release 配置。

```sh
cargo build --release --locked -p joysheet-calc --example recalc_fanout
cp target/release/examples/recalc_fanout target/fanout-base
cargo build --release --locked -p joysheet-calc --example recalc_fanout --features scalar-pilot
cp target/release/examples/recalc_fanout target/fanout-pilot
target/fanout-base 100000
target/fanout-pilot 100000

cargo build --release --locked -p joysheet
cp target/release/joysheet target/desktop-base
cargo build --release --locked -p joysheet -p joysheet-calc --features joysheet-calc/scalar-pilot
cp target/release/joysheet target/desktop-pilot
python3 scripts/joysheet/bench-fanout.py target/desktop-base target/desktop-base-run1 --rows 100000
python3 scripts/joysheet/bench-fanout.py target/desktop-pilot target/desktop-pilot-run1 --rows 100000
```

要复现本轮 macOS 构建，可给每条 build 加 `--target aarch64-apple-darwin` 并从相应 target 子目录复制。使用默认 features 重新构建即可回退。工作簿文件格式不需要迁移。

要复跑文件回退，以相同方式分别构建并保留 `joysheet-cli` 的默认版和试点版。在上一步生成的目录中交叉读取工作簿，输出使用新路径。每次 `check` 应报告匹配 100,001 个公式、0 个往返差异。

```sh
target/cli-base run --in 'target/desktop-pilot-run1/重算 副本.xlsx' --cmd 'cell.set={"cell":"A1","input":"23"}' --out target/pilot-to-base.xlsx
target/cli-pilot check target/pilot-to-base.xlsx --json
target/cli-pilot run --in 'target/desktop-base-run1/重算 副本.xlsx' --cmd 'cell.set={"cell":"A1","input":"23"}' --out target/base-to-pilot.xlsx
target/cli-base check target/base-to-pilot.xlsx --json
```

## 工程验证

| macOS 检查 | 结果 |
| --- | --- |
| `cargo xtask check joyword` | 隔离构建目录和配置后 3,654 项通过、2 项既有忽略。涵盖重复另存、取消重试、过时对话框结果和保存冲突。 |
| `cargo xtask check joydeck` | 983 项通过。 |
| `cargo xtask check joysheet` | 默认配置 1,439 项通过、1 项既有忽略。 |
| `cargo test --locked -p joysheet-calc -p joysheet-engine --features joysheet-calc/scalar-pilot` | 696 项通过、1 项既有忽略。包括四则运算差分、错误值、空单元格、文本、缓存公式、跨表引用与范围回退。 |
| 格式、静态与文档检查 | 产品命令各自的 fmt 和严格 Clippy 通过，已另跑试点的 all-targets Clippy 和输入探针的 Clippy。全工作区 fmt、`scripts/check_gates.py`、`scripts/check_docs.py` 和三个新 Python 脚本的语法检查通过。 |

Word 首次从默认 target 运行时，`joyword-agent` 的清单契约测试未能从缓存的构建路径找到 `joyword-geom/Cargo.toml`。使用本仓库的 `CARGO_TARGET_DIR`、`CRAFT_I18N_DIR` 与临时 `JOYWORD_CONFIG_DIR` 重跑完整产品检查通过。上表按产品分别统计，共享 crate 的用例可能重复。

此次本机复验重新执行了三项产品检查，并在修复编辑框后重新执行 Sheet 全量检查和启用试点的 calc／engine 测试，通过数与上表一致。修复后的默认版 Release 还用 100k 行工作簿独立复跑 20 次桌面编辑，每次求值 100,001 格，撤销、重做及保存重开通过。上方性能表沿用同批次交替测量的 400 个样本。
