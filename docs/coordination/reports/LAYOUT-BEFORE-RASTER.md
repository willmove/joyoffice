# 排版就绪与光栅就绪分离交接报告

- 任务：来自 IME 候选窗位置会话的转派，即 [IME-CANDIDATE-POSITION](IME-CANDIDATE-POSITION.md)“接口或依赖提案”中的建议。用户已在本会话确认该改动涉及 C06／C07 画布流水线，并经协调者批准。本任务无任务卡，不占用 Wxx／Cxx 编号。
- 执行者：Claude Code（Claude 桌面应用 Code 会话），模型 Opus 5.5。
- 主机：Apple M4 / arm64 / macOS 27.0.1（26A434）/ Rust 1.98.1（48a229cea 2026-09-01）；内置显示器 1920×1080 点，缩放系数 1。
- 代码基线：`claude/ime-candidate-position` @ `6723b9394da8b94fe70a3b45b75148e37e2f6663`，含 ImeAnchor 与 gpui-pre 0.3.9，尚未合入 main。启动时 main 为 `0dcc0f0e4f0efa5d7845b697ed7d60bcc2136ff2`。
- 分支：`claude/layout-before-raster`（独立 worktree `.claude/worktrees/elastic-shtern-26234a`）。
- 状态：有限交付。代码、无窗口测试、变异检查和 lint 均已完成；GUI 验证未完成，原因见“GUI 验证”。
- interface_ready：true。只改 JoyWord 应用内部的画布状态、几何入口和绘制，不改 engine、layout、render、shell 公共接口，也不改依赖与锁文件。
- implementation_verified：仅无窗口部分为 true。应用测试 21 项（原 15 项）、workspace clippy 和 7 项变异检查通过；两项 GUI 场景未实测。
- acceptance_complete：false。两项约定的 GUI 场景未完成，Retina、多显示器、Windows、Linux 也未覆盖。

## 背景与实测时延

原 `render_view` 在一个后台任务里依次完成排版和全部页面光栅；`ready_frame()` 要等两者都结束才为 Some。在此之前，命中测试、`character_index_for_point`、光标滚入视图和覆盖层都不可用，IME 也只能依靠起点锚点或返回 None。

用调试测试构建，以种子文档（Songti.ttc face 6，1 页）各测 3 次。测时用的临时测试未提交。

| 阶段 | 80% | 100% |
| --- | --- | --- |
| `layout_document` | 2.37–2.43 ms | 2.41–2.42 ms |
| 全部页面光栅＋转成 `RenderImage` | 29.1–30.2 ms | 40.5–40.7 ms |

排版完成后几何即可使用，每次按键的“几何不精确”空窗从约 30–40 ms（另加主线程跳转与下一帧）缩短到约 2.4 ms 加一次主线程跳转。

## 实际变更与支持范围

| 路径 | 变更 |
| --- | --- |
| `apps/joyword/src/document_canvas.rs` | 删除 `CanvasRequestKey`／`CanvasCancellation`／`CanvasFrame`／`render_view`。新增无窗口状态机 `CanvasPipeline`，以及 `LayoutRequestKey`、`RasterKey`、`LayoutFrame`、`RasterFrame`、`LayoutJob`、`RasterJob`、`Landed`。后台阶段拆为 `layout_view` 与 `raster_layout`，光栅预算循环抽成 `raster_budgeted`，测试辅助 `raster_pages` 也走同一循环 |
| `apps/joyword/src/main.rs` | `JoyWord` 用 `canvas: CanvasPipeline` 取代 `frame`／`gate`／`canvas_cancel`，增加 `layout_task`。`request_canvas` 分别发起排版和光栅任务，回调为 `layout_landed`／`raster_landed`；`ready_frame` 改为 `ready_layout`。页面框取自已安装排版，图像取自最近一次光栅 |
| `apps/joyword/src/input_ui.rs` | 几何入口全部改用 `ready_layout`；命中测试抽成纯函数 `hit_at`；新增 `capture_ime_anchor`；`ImeGeometry` 的键改为排版键加缩放；打开其他文档时改调 `clear_canvas` |

### 两段流水线

- **两个键、两道闸门**（均用现有的 `RasterRequestGate<K>`）：
  - 排版键 `LayoutRequestKey { document, revision, font, profile }` 不含投影，缩放与设备像素比只影响像素。
  - 光栅键 `RasterKey { layout, projection }` 中，`layout` 是已安装排版的“代号”，每次安装新排版加一；复用排版的修订沿用同一代号。
- **排版安装规则**：只有最新票据、且键等于当前键的结果才能安装。排版落地后立即请求光栅，不再等下一帧。
- **光栅安装规则**：只有最新光栅票据、且键等于“当前已安装排版的代号＋当前投影”的结果才能安装。新排版尚在等待时，已安装排版的光栅仍可落地，但它只提供像素，不映射位置。
- **取消**：
  - 新排版请求取消旧排版任务，但不取消已安装排版正在进行的光栅，因为那批像素仍可显示。
  - 新光栅请求取消旧光栅任务。
  - 打开其他文档（`clear`）与视图销毁（`Drop`）同时取消两者。
- **几何（`JoyWord::ready_layout`）**：要求已安装排版的键等于当前键，且没有画布错误。以下功能在排版落地、光栅之前就可用：
  - 命中测试与 `character_index_for_point`
  - `bounds_for_range`
  - 光标滚入视图
  - 选择高亮、组合下划线与光标绘制
  - 上下行／行首行尾命令的可用性
  - 缺字提示
- **绘制**：页面框与滚动范围取自已安装排版（无论是否当前），所以与几何一致。图像按页序取自最近一次光栅；新排版多出的页在新光栅到达前显示为白页。
- **缩放**：只重新光栅，几何与覆盖层立即按新缩放精确绘制。此前要等整页重排加光栅，期间覆盖层消失。旧图像在新光栅到达前仍拉伸到新页面框，这一点与此前相同。
- **IME 起点锚点**：除绘制时的捕获外，排版落地和 `apply_input` 复用排版时也立即捕获。因为 AppKit 可能在下一帧绘制之前就查询下一组组合。
- **文档号**：每个 `TextInput` 的输入修订都从 0 开始。旧实现的键只有 `(revision, font, profile, projection)`，代码分析显示两个问题，均未在 GUI 复现：
  - 未编辑种子文档就打开文件时，新文档修订 0 的键与上一次请求相同，闸门会去重，不发起渲染。画布停在“加载中”，直到下一次输入改变修订。
  - 如果种子任务尚未完成就打开文件，旧结果还可能被当成新文档安装。

  现在 `clear` 时文档号加一，旧请求既不会被误认为新请求，也不会阻塞新请求。

### 保持的约束

| 约束 | 实现 | 覆盖测试 |
| --- | --- | --- |
| 过期几何不映射位置 | `ready_layout` 要求键（文档号、修订、字体、配置）等于当前；锚点规则不变 | `a_layout_without_pixels_answers_ime_hit_testing_and_reveal_exactly`、`geometry_installs_after_layout_while_the_last_pixels_stay_painted` |
| A→B→A 结果被 `RasterRequestGate` 拒绝 | 排版、光栅各一道闸门，按序号票据判断 | `stale_layouts_lose_to_newer_requests_including_a_to_b_to_a`、`stale_rasters_cannot_install_over_another_layout_or_projection` |
| 取消仍然有效 | 按上节规则取消；被取消的任务返回错误，且票据已过期，不会安装 | 同上两项，以及 `another_document_cancels_jobs_and_never_matches_old_revisions` |
| 单次光栅 16,777,216 px 预算 | `raster_budgeted` 在全部页面间共享剩余预算 | `one_raster_stays_within_the_ui_cache_budget`：2× 屏 150% 每页约 8.0 M px，第 3 页报 `LimitExceeded(Pixels)` |
| 提交与预览相同时复用像素 | `CanvasPipeline::reuse`：`ensure_current` 通过即沿用排版与代号，像素仍就绪，不发起任何任务 | `commit_equal_to_its_preview_reuses_layout_and_pixels_without_jobs` |

内存峰值与此前相同：一份已安装的光栅，加一个进行中的光栅。

## 验证

| 完整命令或人工步骤 | 实际结果 | 环境/输入/日志 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | passed | — |
| `cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings` | passed | 仅 `block v0.1.6` future-incompat 提示 |
| `cargo test -p joyword --features dev-runtime-shaders --locked` | passed，21 passed / 0 failed | 无窗口；仓库固定 OFL 小字体 `tests/fixtures/fonts/joyoffice-test-serif.ttf` face 0 |
| 变异检查：7 种单点破坏，每次只改 `document_canvas.rs` 一处 | 每种都有至少 1 项测试失败；之后恢复文件并用 `cmp` 确认与原文件一致 | 结果见下方列表；未提交 |
| `python3 scripts/check_docs.py` | passed | — |
| `git diff --check` | passed | — |
| 排版／光栅时延测量 | 见“背景与实测时延” | 调试测试构建，临时测试未提交 |
| GUI：快速提交后输入、光标滚出视口 | **未完成** | 见“GUI 验证” |

变异检查的 7 种破坏与失败的测试：

- `ready_layout` 不比较键：6 项失败。
- 光栅安装不比较排版代号：`stale_rasters…` 失败。
- 排版安装不看票据、只比较键：`stale_layouts…` 失败。
- 键里去掉文档号：`another_document…` 失败。
- 新排版请求同时取消光栅：`stale_rasters…` 失败。
- `reuse` 不更新修订：`commit_equal…`、`stale_rasters…` 失败。
- 页面间不共享预算：`one_raster…` 失败。

新增或改写的测试：

- `document_canvas.rs`：
  - `geometry_installs_after_layout_while_the_last_pixels_stay_painted`：预编辑排版落地、光栅之前，位置已能映射（光标与命中测试互逆）。旧像素按 `Arc` 身份保持显示，新光栅落地时作为“被替换的图像”交还调用方。缩放只发起光栅任务。
  - `stale_layouts_lose_to_newer_requests_including_a_to_b_to_a`：A、B 两个预编辑的文档版本相同，只有内容散列不同。旧 A、旧 B 都不能安装。A→B→A 后，即使内容与旧 A 相同，旧 A 票据仍被拒绝。被取消的令牌使排版返回错误。
  - `stale_rasters_cannot_install_over_another_layout_or_projection`：
    - 新排版等待期间，已安装排版的光栅可以落地，但几何仍不可用。
    - 新排版安装后，旧排版的光栅即使没有更新的光栅请求也被拒绝。
    - 其他投影的光栅、以及被新票据取代的光栅也被拒绝。
  - `commit_equal_to_its_preview_reuses_layout_and_pixels_without_jobs`：由原 `composition_layout_pixels_and_commit_share_source_without_mutating_history` 改写。选择变化、以及与预览相同的提交，都沿用同一排版与同一批图像，不发起排版或光栅任务。重新从头渲染得到相同的排版键与像素；撤销后像素回到原样。
  - `another_document_cancels_jobs_and_never_matches_old_revisions`：打开其他文档时两类任务都被取消。新文档修订 0、字体和配置都与旧文档相同，键仍然不同，可以正常发起排版。
  - `one_raster_stays_within_the_ui_cache_budget`：同上表。
  - `missing_font_glyph_keeps_typed_unicode_and_emits_diagnostics`：改用 `layout_view`。
- `input_ui.rs`：
  - `a_layout_without_pixels_answers_ime_hit_testing_and_reveal_exactly`：多页文档第 2 页段首组合 “AV”，只让排版落地、完全没有光栅。组合内 4 种范围的 `ime_bounds` 与直接排版的结果一致；组合末尾在起点右侧。`hit_at` 把起点与末尾框的中心映射回各自的 UTF-16 偏移，视口外的点返回 None。`reveal_target` 直接给出活动端，不再“欠精确滚动”。
  - `commit_then_type_answers_from_the_committed_layout_before_its_pixels`：组合 “fi”，提交为不同文本 “AV\n”（模拟拼音变汉字），只让提交后的排版落地。上一组的锚点不再回答；由提交后排版捕获的锚点把下一组组合的候选位置放在新起点（下一行），与新预编辑排版落地后的精确起点完全相同。

## GUI 验证

时间与方式：2026-10-16 17:36，经用户同意占用桌面。

1. `cargo build -p joyword --features dev-runtime-shaders --locked`，然后用 `python3 scripts/package_macos_dev.py` 包装提交 `70c13e2`，再以 `open -n target/dev/JoyWord.app` 启动。窗口 1100×883，位于屏幕 (410,88)，缩放 80%。启动截图中 JoyWord 在最前。
2. 经 System Events 把 JoyWord 进程置前，按 Ctrl+Space 切到简体拼音（已确认 `com.apple.inputmethod.SCIM.ITABC`），输入 `nihao`；随后同一脚本内按空格并立即输入 `shijie`。
3. 结果：按键没有到达 JoyWord。截图显示最前应用已变为 Xcode，它正在编辑主 checkout 的 `AGENTS.md`；拼音文本被插入 `/Users/willm/Coding/joyoffice/AGENTS.md` 第 3 行，并被 Xcode 自动保存。
   - JoyWord 进程仍在运行，没有崩溃报告。
   - 焦点为何转移未查明。
4. 处理：
   - 把被改动的文件和 diff 保存到本组会话的共享笔记目录（仓库外）。
   - 用 `git restore -- AGENTS.md` 恢复主 checkout。主 checkout 的 diff 只有这一行，且文件修改时间与按键时刻一致；恢复后 `git status` 干净，数秒后复查仍干净。
5. 为避免再次误输入，本会话的自动权限检查拒绝了后续发送按键的命令，GUI 验证就此停止。
   - 两个约定场景均**未验证**：快速提交后输入时候选条落在新组合起点；光标滚出视口后输入。
   - 输入源停留在简体拼音；JoyWord 开发实例仍在运行，没有文本输入。

截图未提交，与此前报告一致。

## 已知边界

- **提交后在排版时间内立即输入。** 如果下一组的首个按键早于提交后的排版落地（调试构建约 2.4 ms，另加一次主线程跳转），该次查询仍得到 None，系统拼音沿用上一组的位置。从第二个按键起，由新预编辑排版捕获的锚点即可回答。人工输入的按键间隔远大于此窗口，脚本以毫秒级间隔连发时仍可能落入。
- **像素晚于几何约一个光栅时间。** 光栅等待期间（调试构建约 30–40 ms），光标、选择、组合下划线已画在新位置，底下仍是旧像素，会短暂看到“光标先于字形”。新排版增加的页面在新光栅到达前是白页。
- **连续快速输入。** 按键间隔短于光栅时间时，每次新排版的光栅请求都会取消上一个光栅，像素在停顿后才更新。像素方面与此前相同，但几何每次按键都会跟上。
- **光栅失败。** 例如超出预算时，仍与此前一样显示画布错误，同时隐藏几何与覆盖层。

## 接口或依赖提案

无。未改公共接口、依赖或锁文件。只用到 `joyword_render::requests::RasterRequestGate` 已有的泛型键。

## 阻塞与下一步

- **GUI 验证需要重新约定时段。** 下次开始前：
  - 关闭或隐藏打开着仓库文件的编辑器（本次是 Xcode）。
  - 每批按键前在脚本里确认最前进程是 JoyWord，不是就中止；或由用户手动输入。
- **两个待验场景**：
  - 系统拼音输入 `nihao`，空格提交后立即输入 `shijie`：候选条应落在新组合起点，即“你好”之后。
  - 滚到页面底部、光标滚出视口后输入：页面应先滚回，候选条紧贴组合文本下方。
- **合入顺序**：先 `claude/gpui-pre-migration`，再 `claude/ime-candidate-position`，最后本分支。
- **未覆盖**：Retina、外接／多显示器、Windows、Linux 输入法、发布构建（预编译 Metal）下的时序。
