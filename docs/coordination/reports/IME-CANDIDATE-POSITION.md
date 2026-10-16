# IME 候选窗位置修正交接报告（macOS）

- 任务：由 gpui-pre 迁移会话转派（来源为 [GPUI-PRE-MIGRATION](GPUI-PRE-MIGRATION.md)“GUI 冒烟”中“候选窗位置”一项）；无任务卡，不占用 Wxx/Cxx 编号。
- 执行者：Claude Code（Claude 桌面应用 Code 会话），模型 Opus 5.5。
- 主机：Apple M4 / arm64 / macOS 27.0.1（26A434）/ Rust 1.98.1（48a229cea 2026-09-01）；内置显示器 1920×1080 点，缩放系数 1（非 Retina）。
- 代码基线：`claude/gpui-pre-migration` @ `616441f5194c756a10b1ad08da2e40a0c3d9cd32`（启动时该分支尚未合入 main，按任务要求以它为基线）。
- 分支：`claude/ime-candidate-position`（独立 worktree `.claude/worktrees/determined-varahamihira-19bbe1`）。
- 状态：完成本轮切片（根因定位、修正、无窗口测试、lint）；GUI 验证见“GUI 验证”一节。
- interface_ready：true。仅改 JoyWord 应用内部的 `EntityInputHandler::bounds_for_range` 与几何换算，不改 engine、shell、layout 公共接口与依赖。
- implementation_verified：true（无窗口）。新增 3 项几何测试，应用测试 14 项、workspace clippy 通过。
- acceptance_complete：false。真实系统拼音的候选窗位置以 GUI 验证一节的实测为准；Retina／多显示器／Windows／Linux 输入法未验收。

## 根因

1. **GPUI 把 None 交给 AppKit 时就是屏幕原点。** `gpui-pre-macos-0.3.9/src/window.rs` 的 `first_rect_for_character_range`（第 3393–3421 行）在 `bounds_for_range` 返回 None 时回传 `NSRect((0,0),(0,0))`。Cocoa 屏幕坐标原点在主显示器左下角，系统拼音把候选条放在该矩形下方，越界后贴着屏幕底边，即冒烟看到的约 (5,1046) 点。返回 Some 时，平台层用 `get_frame` 的窗口框架与内容高度把窗口逻辑像素（左上原点）翻转成屏幕点，换算本身正确。
2. **JoyWord 在组合态几乎总返回 None。** 原 `bounds_for_range` 必须拿到 `ready_frame()`，即画布键与当前显示修订一致。每次 `setMarkedText` 走 `TextInput::mark`，显示修订加一；新修订的排版和 CPU 光栅（`render_view`）在后台执行，完成前 `ready_frame()` 为 None。AppKit 恰在 `setMarkedText` 之后立刻查询 `firstRectForCharacterRange`，于是每次按键都落在“新排版尚未到达”的窗口里。
3. **事后刷新补不回来。** 排版到达后 `refresh_ime_geometry` 会调用 `Window::invalidate_character_coordinates`，但 gpui-pre 的实现（`gpui-pre-0.3.9/src/window.rs` 第 6653 行）只在下一帧 `selected_bounds` 返回 Some 时才通知平台；即便系统重新查询并移到正确位置，下一次按键又会回到第 2 步的空窗口。
4. **坐标系逐项核对，未发现换算错误。** 覆盖层 canvas 的 prepaint 边界就是窗口逻辑像素；滚动偏移实时相加（覆盖层不在滚动容器内）；缩放只作用于 EMU 部分（页边距、页高、行位置），`DOC_PAD`／`DOC_GAP` 不随缩放；设备像素比只进入光栅 `Projection(96 × scale_factor, zoom)`，不进入逻辑坐标，AppKit 的点与 GPUI 逻辑像素一致。选择高亮、光标与命中测试一直使用同一换算，与冒烟中“光标与选择位置正确、只有候选窗错位”的现象一致。

## 实际变更与支持范围

| 路径 | 变更 |
| --- | --- |
| `apps/joyword/src/input_ui.rs` | 几何换算抽成纯函数：`Placement`（视口、实时滚动、缩放）与缩放无关的 `SceneBox`（页序、上方页总高 EMU、框）；`project_page`、`scene_rect`、`bounds_for_range` 共用同一投影。新增 `ImeAnchor` 与 `ime_bounds`；`bounds_for_range` 改为调用 `ime_bounds` |
| `apps/joyword/src/main.rs` | `JoyWord` 增加 `ime_anchor` 字段 |

行为：

- **精确路径不变。** 有当前排版时，空范围返回光标框（沿用活动端 Bias），非空范围返回第一段选择矩形。
- **组合起点锚点。** 每次绘制且几何身份变化、并且画布为当前帧时，在 IME 起点（有组合时为标记区起点，否则为将被替换的选择起点，Bias::After）记录光标框；键为已提交文档版本（`DocEditor::version`，预编辑不改变它，撤销也单调递增）与 UTF-16 起点。新排版尚未到达时，只有“同一已提交版本、同一起点、请求范围位于当前组合或选择之内”的查询由锚点回答，投影使用实时滚动与当前缩放。排版到达后回到精确路径，并沿用原有失效通知。
- **越界钳制。** 请求超出文末时钳制到文末（AppKit 可能询问光标后一个字符）。
- **打开其他文档时清空锚点**，避免不同文档的版本号碰巧相同。
- **边界保持。** 锚点只用于候选窗定位；命中测试、`character_index_for_point`、绘制与选择仍只接受当前帧，C07“过期画布不参与位置映射”的约束不变。版本不同（例如刚提交、提交后的排版尚未到达）时仍返回 None，此时系统不显示候选窗；下一次组合开始前，提交后的排版通常已到达并重新记录锚点。
- **已知近似。** 排版未到达期间，组合内任意范围都回答起点光标，而不是预编辑末尾的精确光标；若新输入使起点所在词折到下一行，起点在排版到达前仍停在旧位置。两者都在排版到达后由精确路径纠正。

## 验证

| 完整命令或人工步骤 | 实际结果 | 环境/输入/日志 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | passed | — |
| `cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings` | passed | 仅 `block v0.1.6` future-incompat 提示 |
| `cargo test -p joyword --features dev-runtime-shaders --locked` | passed，14 passed / 0 failed | 无窗口；仓库固定 OFL 小字体 `tests/fixtures/fonts/joyoffice-test-serif.ttf` face 0 |
| 变异检查：临时让锚点永不生效，并让页间距随缩放变化 | 新增 3 项全部 failed，其余 11 项 passed；随后恢复原文件 | 证明新测试能区分旧行为和缩放公式错误；未提交 |
| `python3 scripts/check_docs.py` | passed | — |
| `git diff --check` | passed | — |
| GUI 验证 | 见下节 | — |

新增测试（`apps/joyword/src/input_ui.rs`）：

- `ime_bounds_are_window_logical_pixels_at_zoom_and_live_scroll`：100 段多页文档，光标在第 2 页首行；缩放 80%／100% × 滚动 (0,0)／(−50,−1400) 四种组合下，用 A4（595.3×841.9 pt、90 pt 侧边距）写死期望的窗口逻辑像素（如 100%、无滚动时光标 x=169、第 2 页顶 y=1246.5333），并验证该框经命中测试映射回同一 UTF-16 偏移。
- `pending_preedit_layout_answers_from_the_origin_anchor`：段首开始组合 "AV"，预编辑排版未到达时，旧逻辑（无锚点）返回 None；有锚点时组合内 4 种范围都返回与空闲时精确结果相同的框，80% 缩放和额外滚动 30 px 时随之变化；组合外 3 种范围返回 None；预编辑排版到达后精确结果与锚点一致，组合末尾光标在同一行更靠右。
- `origin_anchor_is_dropped_for_other_versions_or_origins_and_queries_clamp_to_end`：已提交版本变化、起点移动时不使用锚点；`len..len+1` 与 `len+4..len+9` 钳制为文末光标。

既有 `page_geometry_round_trips_at_zoom_with_live_scroll_on_multiple_pages` 改用新的 `Placement` 后继续通过。

## GUI 验证

待与用户约定占用桌面的时段后进行：`cargo build -p joyword --features dev-runtime-shaders --locked`，`python3 scripts/package_macos_dev.py` 包装，`open` 启动；系统简体拼音输入，截图核对候选窗贴近光标（缩放 80%／100%，以及滚动后）。另以仅用于验证、不提交的追踪补丁（`JOYWORD_IME_TRACE` 环境变量下把每次 `bounds_for_range` 的范围、是否有当前帧与返回值写到 stderr）分别构建基线与修正版本，作为根因的运行时证据。

## 接口或依赖提案

无。未改公共接口、依赖或锁文件。

## 阻塞与下一步

- GUI 验证需用户约定时段（见上节）。
- 未覆盖：Retina 与外接／多显示器（本机仅 1×）、Windows（`bounds_for_range` 同样被 Windows IME 使用，锚点逻辑平台无关但未实测）、Linux Fcitx／IBus、垂直排版与表格等非正文对象（当前输入层本就拒绝）。
- 建议协调者在合入 gpui-pre 迁移后再合入本分支，并在 C07 的未完成项中把“系统拼音候选框实际坐标”改为引用本报告的实测结果。
