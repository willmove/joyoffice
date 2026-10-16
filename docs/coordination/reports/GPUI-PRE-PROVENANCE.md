# gpui-pre 来源校验交接报告：与 Zed 提交逐文件比对

- 任务：[GPUI 来源迁移报告](GPUI-PRE-MIGRATION.md)“接口或依赖提案”第 5 条——为 Cargo.lock 中全部 `gpui-pre*` 包提供可在 CI 重复运行的 Zed 源码逐文件比对（ADR-010 第三方发布的供应链审查）。由迁移会话按用户指示派出，无任务卡，不占用 Wxx/Cxx 编号。
- 执行者：Claude Code（Claude 桌面应用 Code 会话），模型 Opus 5.5。
- 主机：Apple M4 / arm64 / macOS 27.0.1（26A434）；Python 3.9.6（系统自带，无 `tomllib`，因此走脚本内置的 TOML 子集解析器）；git 2.54.0（Apple Git-157）。
- 代码基线：`claude/gpui-pre-migration` @ `937a1cbb1726b15163745ed87b0dc5aee340777b`（main `0dcc0f0` 加迁移提交）。
- 分支：`claude/gpui-pre-provenance`（独立 worktree `.claude/worktrees/nostalgic-wright-945d4b`）。
- 状态：完成本轮切片。
- interface_ready：true。命令行、退出码与偏差文件格式见下文；CI 已加入 `provenance` 作业。
- implementation_verified：true（本机）。43 项离线单测在 Python 3.9.6（内置 TOML 解析器）和 3.15.0（`tomllib`）下均通过（新增 24 项）；真实联网比对两种模式均符合预期（严格模式 exit 1，带审查偏差文件 exit 0）。Linux、Windows 由 CI 矩阵的单测覆盖。
- acceptance_complete：false。9 处偏差已由项目负责人于 2026-10-16 接受，CI 作业以及 ADR-010、PROVENANCE 的记录已加入本分支；本分支尚未合入 main。

## 结论

- 19 个 `gpui-pre*` 包（均 0.3.9）都来自 crates.io，`.crate` 的 SHA-256 与 Cargo.lock 校验和一致；全部声明 `zed-rev = 93d745668805029476e6d7fbb19652116fed51de`，该提交在 zed-industries/zed `main` 的历史中。
- **16 个包与 Zed 对应 crate 目录逐文件一致。**
- **3 个包不一致，共 9 处**：gpui-pre（改 1）、gpui-pre-apple（改 1、增 5，增加的 5 个文件与 Zed `crates/gpui/src/` 同名文件逐字节相同）、gpui-pre-macros（改 1、增 1）。改动文件首行都有 “Modified for gpui-pre” 注释，属于重新发布的路径适配和 gpui-kit 门面注入；按 JoyWord 当前用法没有行为差异（逐处分析见下）。脚本默认对其判失败。
- 共 313 个文件一致；另有每包 3 个打包生成文件（`Cargo.toml`、`Cargo.toml.orig`、`Cargo.lock`）不比对，理由见“允许的差异”。

## 裁决与接入（2026-10-16）

用户（项目负责人）在本会话中作出以下裁决，本分支第二个提交据此落实：

1. 接受 9 处偏差（按 ADR-010）。`scripts/gpui_pre_reviewed_deviations.json` 的说明改为“已接受”，各条目内容不变。
2. 按本报告的配置加入 CI。`.github/workflows/ci.yml` 新增 `provenance` 作业：ubuntu-24.04，Python 3.12（与其他作业一致），actions 沿用已按 SHA 固定的版本。
3. ADR-010 与 PROVENANCE 如何记录由执行者决定。本分支已更新：
   - ARCHITECTURE §8 ADR-010：补记选用 gpui-pre =0.3.9（Zed 93d7456）及平台特性、wgpu GL 后端仍待 T005 验证，以及供应链补偿措施和本次结果。
   - PROVENANCE §3 GPUI 行：补记版本、Zed 提交、crates.io 所有者（huacnlee，已经 crates.io API 核实）和校验方式。同节许可段删去锁文件中已不存在的 NCSA，停止维护公告改为 gpui-pre 下的 3 条，与 deny.toml 一致。§5 新增“gpui-pre 来源逐文件校验”记录。
4. 随后用户要求按迁移报告提案 2–4 更新其余文档，已在同一分支完成，相关事实均对照 gpui-pre 0.3.9 源码和 Cargo.lock 核实：
   - README 第 25 行：GPUI 固定为 gpui-pre 0.3.9（Zed 93d7456 快照，CI 核对来源）。
   - ARCHITECTURE §6.1：0.2.2 的 GlyphId 限制改记为历史情况。gpui-pre 已公开 `GlyphId(pub u32)`（text_system.rs:1255）与 `Window::paint_glyph`（window.rs:4821），但 `add_fonts` 仍返回 `Result<()>`、不返回 FontId，屏幕侧同一字形仍待 T003。
   - ARCHITECTURE §7：剪贴板条目为 String、Image、ExternalPaths，没有富文本，所以打印与富剪贴板仍需自建；Linux 渲染器为 wgpu，按 Vulkan 与 GL 枚举适配器，GL 路径待 T005 验证。
   - IMPLEMENTATION：GPUI 行改为 gpui-pre 0.3.9（含平台特性、发布者、来源校验结果、cargo-deny 由 7 条减为 3 条、去掉 NCSA）；libc 行注明锁文件中已无 xattr，原限制可在下次升级时解除（本次未升级）；“当前直接依赖”补记两个包名；验证记录新增 CI 运行 38889115731；future-incompat 说明删去已不在锁文件中的 proc-macro-error2。

## 实际变更与支持范围

| 路径 | 变更 |
| --- | --- |
| `scripts/verify_gpui_pre_provenance.py` | 新增。Python 3，仅标准库，3.9 起可用（3.11+ 用 `tomllib`，否则用内置的 Cargo TOML 子集解析器，遇到不支持的语法直接报错而不猜测） |
| `scripts/tests/test_verify_gpui_pre_provenance.py` | 新增 24 项离线测试：伪造包目录、伪造 Zed 目录与本地 git 仓库，不联网 |
| `scripts/gpui_pre_reviewed_deviations.json` | 新增。执行者对 9 处差异的审查提案，逐条按内容钉死；只有显式传入 `--reviewed-deviations` 时生效 |
| `docs/coordination/reports/GPUI-PRE-PROVENANCE.md` | 本报告 |

首次交付未修改 `.github/workflows`、根 Cargo 文件、ADR 及其他公共文档；裁决后的改动见上节。两次提交都没有改动根 Cargo 文件。

### 校验方法

1. 枚举：Cargo.lock 中名称为 `gpui-pre` 或以 `gpui-pre-` 开头的条目。来源必须是 crates.io 且带 SHA-256 校验和。
2. 取包：先找 `$CARGO_HOME/registry/cache/*/<名称>-<版本>.crate`，没有则从 `https://static.crates.io/crates/<名称>/<名称>-<版本>.crate` 下载（`--no-download` 可禁用）。SHA-256 必须等于 Cargo.lock 校验和，所以比对的正是 Cargo 实际构建的字节，而不是本地可被改动的解压目录。归档只在内存中读取、不落盘；路径越界、符号链接或硬链接等非普通文件、重复条目、解压超限一律拒绝。
3. 来源元数据：从发布后的 `Cargo.toml` 读取 `[package.metadata.gpui-pre]` 的 `zed-crate`、`zed-rev`。注意实际位置是 `gpui-pre` 子表，而不是派工说明中写的 `[package.metadata]` 顶层。`zed-rev` 必须是 40 位完整提交号；清单包名与版本须与锁文件一致；所有包须声明同一提交，可另用 `--expected-zed-rev` 钉死。
4. Zed 端：建立 git 部分克隆，`fetch --depth=1 --filter=blob:none` 只取该提交和全部树对象（约 200 KiB）。git 入库时按内容校验对象 ID，并开启 `fetch.fsckObjects`。随后用 `--filter=tree:0 --shallow-since=<提交时间前 14 天>` 只取 `main` 的近期提交对象（约 0.5 MiB），要求 `git merge-base --is-ancestor <rev> main` 成立。原因：GitHub 同一 fork 网络共享对象存储，按 SHA 从上游 URL 也可能取到只存在于某个 fork 的提交，所以“取得到”不等于“是 Zed 主干代码”。
5. 定位 crate 目录：读取 Zed 根 `Cargo.toml` 的 `[workspace] members`，选出目录名等于 `zed-crate` 的唯一成员，并核对该成员清单的 `package.name`。例如 derive_refineable 位于 `crates/refineable/derive_refineable`。Zed 树中还有 `tooling/lints/test_fixture/gpui`、`tooling/lints/test_fixture/gpui_shared_string` 这类同名目录，它们不是工作区成员；按目录名搜索全树会产生歧义，这是本次实测中发现的。
6. 逐文件比对：对包内每个文件计算 git blob ID（对 `blob <长度>\0` 加内容做 SHA-1），与 Zed 提交中同一相对路径的 blob ID 比较。提交 ID 认证它的树，树认证 blob ID，所以 blob ID 相同就是内容逐字节相同，不必下载这些 blob。这依赖 SHA-1 的第二原像抗性，与“用 SHA-1 提交号标识 Zed 代码”本身是同一个信任假设（git 使用带碰撞检测的 SHA-1）。只有根清单、成员清单和符号链接目标需要读取 blob 内容，约 20 个，一次批量请求取回，约 2 秒。
7. 内容不同、多余、缺失都判失败（exit 1）。失败行附带钉死所需的 sha256 与 Zed blob ID；如果多余文件与 Zed 其他路径内容相同，会提示来源路径。

### 允许的差异（完整列表）

| 项 | 处理 | 理由 |
| --- | --- | --- |
| `Cargo.toml` | 不比对 | `cargo publish` 规范化重写；gpui-pre 还把包和 gpui 依赖改名为 `gpui-pre-*` 并加入来源元数据 |
| `Cargo.toml.orig` | 不比对 | 发布者规范化之前的清单，Cargo 不读取 |
| 包根 `Cargo.lock` | 不比对 | `cargo package` 为单包生成；包作为依赖时被忽略 |
| `.cargo_vcs_info.json` | 不比对 | `cargo package` 写入的 VCS 说明，只是元数据（gpui-pre 发布包没有这个文件） |
| Zed 中的符号链接（每包的 `LICENSE-APACHE`） | 解析后比对目标内容 | git 中存的是符号链接，`cargo package` 打入的是目标文件内容；目标不得越出仓库，最多 8 跳 |
| 嵌套包目录（自带 `Cargo.toml` 的子目录） | 从 Zed 侧排除 | `cargo package` 不打入嵌套包。本次涉及 `refineable/derive_refineable`（它另作为 gpui-pre-derive-refineable 单独校验）和 `gpui_web/examples/hello_web` |

为防止不比对的文件被当作构建输入：清单中 `package.build` 以及 `[lib]`、`[[bin]]`、`[[example]]`、`[[test]]`、`[[bench]]` 的 `path` 必须指向参与比对的文件；指向上表中的文件或不存在的文件即失败。文件权限位（可执行位）不比对。

### 其他判定

- exit 1（校验失败）：非 crates.io 来源或无校验和；缓存和下载中都没有与锁文件一致的 `.crate`；归档异常；元数据缺失、非法或提交号缩写；提交不在 Zed `main` 历史中；多个包声明不同提交，或与 `--expected-zed-rev` 不符；Zed 工作区中找不到唯一成员或包名不符；审查偏差条目与实际差异类型不符、内容哈希不符，或没有匹配到任何差异（过期条目）。
- exit 2（无法运行）：下载或网络失败、git 不可用、提交不存在、锁文件或偏差文件不可读、偏差条目字段不合规（字段必须恰好匹配，防止拼写错误被静默忽略）。

### 未覆盖

- 脚本不比对 `Cargo.toml` 内容（依赖、features、lint 配置）。依赖的版本与来源由 Cargo.lock 校验和与 cargo-deny 覆盖；本次另做了一次性清单比对，见下文。
- 不评价 Zed 代码本身是否安全，只证明来源一致。
- 只覆盖 `gpui-pre*` 包；其他 crates.io 依赖仍依靠 crates.io 校验和与 cargo-deny。

## 真实比对结果（2026-10-16，本机联网）

命令 `python3 scripts/verify_gpui_pre_provenance.py`（严格模式，暖缓存）：exit 1，用时 27.2 s。

| 包 | Zed 目录 | 一致文件 | 结果 |
| --- | --- | --- | --- |
| gpui-pre | `crates/gpui` | 156 | 不一致：改 `src/action.rs` |
| gpui-pre-apple | `crates/gpui_apple` | 10 | 不一致：改 `build.rs`；增 `vendor/gpui/src/{color,geometry,platform,scene,window}.rs` |
| gpui-pre-collections | `crates/collections` | 4 | 一致 |
| gpui-pre-derive-refineable | `crates/refineable/derive_refineable` | 2 | 一致 |
| gpui-pre-http-client | `crates/http_client` | 5 | 一致 |
| gpui-pre-linux | `crates/gpui_linux` | 28 | 一致 |
| gpui-pre-macos | `crates/gpui_macos` | 12 | 一致 |
| gpui-pre-macros | `crates/gpui_macros` | 15 | 不一致：改 `src/gpui_macros.rs`；增 `src/gpui_pre_facade_paths.rs` |
| gpui-pre-platform | `crates/gpui_platform` | 4 | 一致 |
| gpui-pre-refineable | `crates/refineable` | 2 | 一致（排除嵌套包 `derive_refineable`） |
| gpui-pre-scheduler | `crates/scheduler` | 6 | 一致 |
| gpui-pre-shared-string | `crates/gpui_shared_string` | 2 | 一致 |
| gpui-pre-sum-tree | `crates/sum_tree` | 5 | 一致 |
| gpui-pre-util | `crates/gpui_util` | 3 | 一致 |
| gpui-pre-web | `crates/gpui_web` | 17 | 一致（排除嵌套包 `examples/hello_web`） |
| gpui-pre-wgpu | `crates/gpui_wgpu` | 11 | 一致 |
| gpui-pre-windows | `crates/gpui_windows` | 25 | 一致 |
| gpui-pre-ztracing | `crates/ztracing` | 4 | 一致 |
| gpui-pre-ztracing-macro | `crates/ztracing_macro` | 2 | 一致 |

“一致文件”包含解析符号链接后的 `LICENSE-APACHE`，不含 3 个不比对的打包文件。

### 9 处差异的审查

1. gpui-pre `src/action.rs`（改，+3/−2 行）：首行加注释；`actions!` 宏两个分支的 derive 列表中 `gpui::Action` 改为 `$crate::Action`。作用是包被改名后宏仍能找到 `Action`；JoyWord 以 `gpui` 名称重命名依赖，展开结果等价。
2. gpui-pre-apple `build.rs`（改，+2/−1 行）：加注释；`find_gpui_crate_dir` 由 `CARGO_MANIFEST_DIR/../gpui`（Zed 检出中的兄弟目录，发布包里不存在）改为 `CARGO_MANIFEST_DIR/vendor/gpui`。
3. 至 7. gpui-pre-apple `vendor/gpui/src/{scene,geometry,color,window,platform}.rs`（增）：与 Zed 同一提交 `crates/gpui/src/` 下的同名文件 blob ID 相同，即逐字节相同。只由 `build.rs` 第 76–80 行读取，用于生成 Metal 着色器头文件，不作为 crate 模块编译。
8. gpui-pre-macros `src/gpui_macros.rs`（改，只增不删）：增加 `mod gpui_pre_facade_paths;` 和注释；19 个过程宏入口都改为 `gpui_pre_facade_paths::rewrite(__gpui_pre_<名称>(…))`，原函数体移入同名私有函数，原逻辑未改。
9. gpui-pre-macros `src/gpui_pre_facade_paths.rs`（增，3481 字节）：头注释说明它由 gpui-kit 的 `script/bump-gpui.ts` 注入。它用 `proc_macro_crate::crate_name("gpui-kit")` 判断正在编译的 crate 是否依赖 gpui-kit：依赖时，把宏输出中的 `gpui::`、`gpui_platform::` 路径头以及含 `::gpui::` 的字面量改写到 gpui-kit 门面；不依赖时原样返回（只以相同定界符和 span 重建分组）。JoyWord 不依赖 gpui-kit，因此这是空操作。副作用只有编译期读取使用方清单，没有网络访问或进程调用。对应清单新增依赖 `proc-macro-crate = "3"`，已在 Cargo.lock 中并受 cargo-deny 检查。

审查意见：9 处都是重新发布所需的路径适配或门面注入，未发现对运行时 GPUI 逻辑的修改。建议接受（项目负责人已于 2026-10-16 接受），并用 `scripts/gpui_pre_reviewed_deviations.json` 钉死：每条锁定内容 sha256；“改”类同时锁定 Zed 基准 blob；“复制”类直接要求等于 Zed 指定文件。任何字节变化或新增差异仍会失败。如果不接受，可选做法是请发布者撤除改动，或改为直接依赖 Zed git 源；后者会引入 git 来源，需同时调整 deny.toml 的 sources 策略，并拉取较大的 Zed 仓库。

### 带偏差文件的 CI 模拟

命令：`python3 scripts/verify_gpui_pre_provenance.py --cargo-home <空目录> --reviewed-deviations scripts/gpui_pre_reviewed_deviations.json --expected-zed-rev 93d745668805029476e6d7fbb19652116fed51de`。空 Cargo 缓存迫使 19 个 `.crate` 全部从 static.crates.io 下载。结果：exit 0，16 个一致、3 个按审查偏差接受、0 个失败；用时 248.4 s（packages 212.4 s，Zed 35.8 s）。

### 清单补充比对（一次性，不是脚本功能）

脚本不比对 `Cargo.toml`，因此另用一次性脚本比较了 19 个发布清单与 Zed 对应清单的依赖键和 features：

- 唯一新增的依赖：gpui-pre-macros 的 `proc-macro-crate`（门面所需）。
- 删除项都是 dev-dependencies（使用方不会构建），另外 gpui-pre-http-client 删除了 `github-download` feature 及其可选依赖（async-compression、async-fs、async-tar、sha2、tempfile、util）。
- 构建脚本：恰好 gpui-pre、gpui-pre-apple、gpui-pre-windows、gpui-pre-ztracing 这 4 个含 `build.rs` 的包写有 `build = "build.rs"`，其余均为 `build = false`，与 Zed 目录中有无 `build.rs` 一致。

## 验证

| 完整命令或人工步骤 | 实际结果 | 环境/输入/日志 |
| --- | --- | --- |
| `python3 scripts/verify_gpui_pre_provenance.py` | failed（符合预期，exit 1）：16 identical / 3 failed；27.2 s（packages 0.1 s，Zed 26.9 s） | 暖缓存；逐包结果见上表 |
| `python3 scripts/verify_gpui_pre_provenance.py --cargo-home <空目录> --reviewed-deviations scripts/gpui_pre_reviewed_deviations.json --expected-zed-rev 93d745668805029476e6d7fbb19652116fed51de` | passed（exit 0）：16 identical / 3 accepted / 0 failed；248.4 s（packages 212.4 s，Zed 35.8 s） | 冷缓存，下载 19 个 `.crate`，共 6.2 MiB；本机国际链路约 30 KB/s，用时主要花在下载 |
| `python3 -m unittest discover -s scripts/tests` | passed，43 项（新增 24 项） | Python 3.9.6，走内置 TOML 解析器 |
| `python3 scripts/check_docs.py` | passed | — |
| `/opt/homebrew/bin/python3 -W error::DeprecationWarning -m unittest discover -s scripts/tests` | passed，43 项 | Python 3.15.0，走 `tomllib`；TOML 样例与 `tomllib` 结果逐项一致；弃用告警视为错误 |
| `/opt/homebrew/bin/python3 scripts/verify_gpui_pre_provenance.py --expected-zed-rev 93d745668805029476e6d7fbb19652116fed51de --reviewed-deviations scripts/gpui_pre_reviewed_deviations.json`（与 CI 作业参数相同） | passed（exit 0）：16 identical / 3 accepted / 0 failed；23.2 s | Python 3.15.0，暖缓存 |
| `/opt/homebrew/bin/python3 scripts/verify_gpui_pre_provenance.py` | failed（符合预期，exit 1）：16 identical / 3 failed；11.3 s | Python 3.15.0，严格模式 |
| `python3 scripts/check_docs.py`（改动 CI 与文档之后） | passed：9 个 workflow action 均按 SHA 固定 | 链接检查覆盖 ARCHITECTURE 与 PROVENANCE 的新链接 |
| CI `provenance` 作业（提交 `6ee61e9`，Actions 运行 38889115731，作业 116934022411） | passed：Result: OK，16 identical / 3 accepted / 0 failed；脚本用时 packages 0.3 s、Zed 0.9 s，整个作业约 4 s | ubuntu-24.04，CPython 3.12.15，冷缓存下载 19 个 `.crate`；同一运行中的 `dependencies` 与 `unified-pdf` 作业也通过 |
| Linux、Windows 上运行单测（含 git 端到端用例） | not run | 本机无对应环境；CI build 矩阵会运行；端到端用例不依赖文件系统符号链接 |

离线测试覆盖：一致；内容不同；`LICENSE` 与符号链接目标不同；多余文件；缺失文件；嵌套包文件混入发布包；缺 metadata；缩写提交号；构建目标指向不比对的文件；审查偏差的接受、拒绝、未使用和字段错误；`.crate` 越界路径与符号链接条目；锁文件筛选；TOML 子集解析与拒绝。端到端（本地 `file://` 仓库）：部分克隆全流程通过；内容被改则失败；fork 提交（不在 `main` 中）失败；校验和不符失败；提交不存在时 exit 2。

## 接口或依赖提案

第 1–3 条已于 2026-10-16 按用户裁决落实（见“裁决与接入”），原提案文本保留备查：

1. **CI 新增作业**（已放入 `ci.yml`，与 `dependencies` 作业并列）：

   ```yaml
   provenance:
     runs-on: ubuntu-24.04
     timeout-minutes: 10
     steps:
       - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
       - uses: actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7.0.0
         with:
           python-version: '3.12'
       # gpui-pre crates must equal the Zed main-branch commit recorded in ADR-010.
       - name: gpui-pre source provenance
         run: >-
           python scripts/verify_gpui_pre_provenance.py
           --expected-zed-rev 93d745668805029476e6d7fbb19652116fed51de
           --reviewed-deviations scripts/gpui_pre_reviewed_deviations.json
   ```

   - 前提：先裁决 9 处偏差。如果不接受，去掉 `--reviewed-deviations` 后作业当前必然失败，这是有意的设计。
   - 依赖：不需要 Rust 工具链、`cargo fetch` 或 GitHub token；只需 Python 3.9+（3.11+ 用 `tomllib`）和 runner 自带的 git。
   - 网络：HTTPS 访问 github.com（git smart HTTP：提交加树约 200 KiB，`main` 近期提交约 0.5 MiB，另批量取约 20 个 blob）和 static.crates.io（19 个 `.crate` 共 6.2 MiB，runner 冷缓存需要全部下载）。不调用 GitHub API，没有速率限制问题。
   - 时长：Zed 侧本机实测 27–36 s（提交与树约 10 s，`main` 历史约 15–20 s，批量 blob 约 2 s），暖缓存时几乎就是全部用时。GitHub runner 上实测脚本只用 1.2 s，整个作业约 4 s；本机冷缓存因国际带宽慢共用 248 s，其中下载 212 s。超时设 10 分钟足够。
   - 退出码：1 表示校验失败，应阻断合并；2 表示无法运行（网络或 git），可重跑。
   - 触发：对固定的 Cargo.lock 结果是确定的（`.crate` 与 Zed 提交均不可变），可以每次 push 都跑（成本低），也可以只在 Cargo.lock、该脚本或偏差文件变化时跑。
2. **ADR-010**：记录本校验是 gpui-pre 的供应链补偿措施，并记录对 9 处偏差的裁决。升级 gpui-pre 时，同步更新 ADR 中的 zed-rev 和 CI 的 `--expected-zed-rev`，按以下流程复审：先不带偏差文件运行，再审查新差异，最后依据失败行给出的 sha256 和 zed_blob 更新偏差文件。
3. **PROVENANCE**：GPUI 行补充“逐文件比对脚本 `scripts/verify_gpui_pre_provenance.py`：16/19 个包与 Zed 一致，3 个包共 9 处已审查偏差”。
4. **已知边界**：crates.io 上同一发布者还有 `gpui-pre-reqwest`（reqwest 分叉，没有 Zed 元数据）。它若将来进入 Cargo.lock，会因缺少元数据而失败，届时需要单独审查。

## 阻塞与下一步

- 无技术阻塞；CI `provenance` 作业已在 GitHub runner 上通过，同一运行（38889115731）的 9 个作业全部通过。待办：是否合入 main 由用户／协调者决定。IMPLEMENTATION 第 199–202 行“下一步”中关于 ADR-010 待选定来源的表述属于任务进度叙述，未在本次修改。
- 可选后续：把清单依赖键比对（上文一次性结果）纳入脚本；向发布者反馈，请其在发布包中保留 `.cargo_vcs_info.json`，或公开改写脚本（gpui-kit `script/bump-gpui.ts`）。
