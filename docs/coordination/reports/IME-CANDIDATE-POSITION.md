# IME 候选窗位置修正交接报告（macOS）

- 任务：由 gpui-pre 迁移会话转派（来源为 [GPUI-PRE-MIGRATION](GPUI-PRE-MIGRATION.md)“GUI 冒烟”中“候选窗位置”一项）；无任务卡，不占用 Wxx/Cxx 编号。
- 执行者：Claude Code（Claude 桌面应用 Code 会话），模型 Opus 5.5。
- 主机：Apple M4 / arm64 / macOS 27.0.1（26A434）/ Rust 1.98.1（48a229cea 2026-09-01）；内置显示器 1920×1080 点，缩放系数 1（非 Retina）；输入法为系统简体拼音（`com.apple.inputmethod.SCIM.ITABC`）。
- 代码基线：`claude/gpui-pre-migration` @ `616441f5194c756a10b1ad08da2e40a0c3d9cd32`（启动时该分支尚未合入 main，按任务要求以它为基线）。
- 分支：`claude/ime-candidate-position`（独立 worktree `.claude/worktrees/determined-varahamihira-19bbe1`）。
- 状态：完成本轮切片（根因定位、两处修正、无窗口测试、lint、macOS GUI 验证）。
- interface_ready：true。仅改 JoyWord 应用内部的 `bounds_for_range`、光标滚入视图与几何换算，不改 engine、shell、layout 公共接口与依赖。
- implementation_verified：true。新增 4 项无窗口测试，应用测试 15 项、workspace clippy 通过；macOS 系统拼音在 80%／100%、滚动后、正文中间行的候选窗位置经截图核对（见“GUI 验证”）。
- acceptance_complete：false。Retina、外接／多显示器、Windows、Linux 输入法未验收；“提交后立即输入下一组拼音”仍有偏差（见“已知边界”）。

## 根因

1. **GPUI 把 None 交给 AppKit 时就是屏幕原点。** `gpui-pre-macos-0.3.9/src/window.rs` 的 `first_rect_for_character_range`（第 3393–3421 行）在 `bounds_for_range` 返回 None 时回传 `NSRect((0,0),(0,0))`。Cocoa 屏幕坐标原点在主显示器左下角，系统拼音把候选条放在该矩形下方，越界后贴着屏幕底边，即冒烟看到的约 (5,1046) 点。返回 Some 时，平台层用 `get_frame` 的窗口框架与内容高度把窗口逻辑像素（左上原点）翻转成屏幕点，换算本身正确。
2. **JoyWord 在组合态几乎总返回 None。** 原 `bounds_for_range` 必须拿到 `ready_frame()`，即画布键与当前显示修订一致。每次 `setMarkedText` 走 `TextInput::mark`，显示修订加一；新修订的排版和整页 CPU 光栅（`render_view`）在后台执行，完成前 `ready_frame()` 为 None。系统拼音恰在 `setMarkedText` 之后立刻查询。
3. **事后刷新补不回来。** 排版到达后 `refresh_ime_geometry` 调用 `Window::invalidate_character_coordinates`；gpui-pre（`gpui-pre-0.3.9/src/window.rs` 第 6653 行）在下一帧用 GPUI 自己的 `selected_bounds` 取值并发出 `invalidateCharacterCoordinates`，但系统拼音收到后并不重新查询（见下方追踪）。下一次按键又回到第 2 步。
4. **坐标系逐项核对，未发现换算错误。** 覆盖层 canvas 的 prepaint 边界就是窗口逻辑像素；滚动偏移实时相加（覆盖层不在滚动容器内）；缩放只作用于 EMU 部分（页边距、页高、行位置），`DOC_PAD`／`DOC_GAP` 不随缩放；设备像素比只进入光栅 `Projection(96 × scale_factor, zoom)`，不进入逻辑坐标，AppKit 的点与 GPUI 逻辑像素一致。选择高亮、光标与命中测试一直使用同一换算，与冒烟中“光标与选择位置正确、只有候选窗错位”的现象一致。

**运行时证据。** 用仅用于验证、不提交的追踪补丁（`JOYWORD_IME_TRACE` 下把每次 `bounds_for_range` 的范围、标记区、是否有当前帧和返回值写到 stderr）构建基线，在文首输入 `nihao`：

- 系统拼音每次 `setMarkedText` 后只问标记文本的最后一个字符：`1..2`（标记 `0..2`）、`3..4`（`0..4`）、`4..5`（`0..5`）、`5..6`（`0..6`），全部是 `ready=false -> None`。
- 约 60 ms 后预编辑排版到达（调试构建，最后一次查询 → 首个 `ready=true` 相隔 57–64 ms），此后只有 GPUI 自己 `selected_bounds` 的空范围查询（`6..6`…`0..0`）拿到 Some；系统拼音没有再查询。
- 修正版同样操作，上述系统查询都在 `ready=false` 时返回锚点框 `(128, 205.8) 1×19.2`（80%，窗口逻辑像素），即组合起点。

## 实际变更与支持范围

| 路径 | 变更 |
| --- | --- |
| `apps/joyword/src/input_ui.rs` | 几何换算抽成纯函数：`Placement`（视口、实时滚动、缩放）与缩放无关的 `SceneBox`（页序、上方页总高 EMU、框）；`project_page`、`scene_rect`、`bounds_for_range` 共用同一投影。新增 `ImeAnchor`、`ime_bounds`、`reveal_target`、`content_size`；`bounds_for_range` 改为调用 `ime_bounds`，`reveal_caret` 改为调用 `reveal_target` |
| `apps/joyword/src/main.rs` | `JoyWord` 增加 `ime_anchor` 字段 |

行为：

- **精确路径不变。** 有当前排版时，空范围返回光标框（沿用活动端 Bias），非空范围返回第一段选择矩形。
- **组合起点锚点。** 每次绘制且几何身份变化、并且画布为当前帧时，在 IME 起点（有组合时为标记区起点，否则为将被替换的选择起点，Bias::After）记录光标框；键为已提交文档版本（`DocEditor::version`，预编辑不改变它，撤销也单调递增）与 UTF-16 起点。新排版尚未到达时，只有“同一已提交版本、同一起点、请求范围位于当前组合或选择之内”的查询由锚点回答，投影使用实时滚动与当前缩放。排版到达后回到精确路径，并沿用原有失效通知。
- **先滚动、后回答。** 输入后光标滚入视图原本也要等新排版。光标已被滚出视口时，系统拼音会拿到滚出视口的位置，随后页面才滚回（GUI 第一轮发现，见下节）。现在预编辑排版未到达时，立刻按锚点把组合起点滚入视图（以正在显示的画布页尺寸钳制），并保留“仍欠精确滚动”，排版到达后再按活动端精确滚动一次。
- **越界钳制。** 请求超出文末时钳制到文末（AppKit 可能询问光标后一个字符）。
- **打开其他文档时清空锚点**，避免不同文档的版本号碰巧相同。
- **边界保持。** 锚点只用于候选窗定位和组合起点的滚入视图；命中测试、`character_index_for_point`、绘制与选择仍只接受当前帧，C07“过期画布不参与位置映射”的约束不变。已提交版本不同（刚提交、提交后的排版尚未到达）时仍返回 None。
- **已知近似。** 排版未到达期间，组合内任意范围都回答起点光标。由于系统拼音只在按键后查询，候选条在整个组合期间停在组合起点下方，而不是跟随预编辑末尾。若新输入使起点所在词折到下一行，起点在排版到达前仍停在旧位置。

## 验证

| 完整命令或人工步骤 | 实际结果 | 环境/输入/日志 |
| --- | --- | --- |
| `cargo fmt --all -- --check` | passed | — |
| `cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings` | passed | 仅 `block v0.1.6` future-incompat 提示 |
| `cargo test -p joyword --features dev-runtime-shaders --locked` | passed，15 passed / 0 failed | 无窗口；仓库固定 OFL 小字体 `tests/fixtures/fonts/joyoffice-test-serif.ttf` face 0 |
| 变异检查：临时让锚点永不生效，并让页间距随缩放变化 | 当时新增的 3 项全部 failed，其余 11 项 passed；随后恢复原文件 | 证明测试能区分旧行为和缩放公式错误；未提交 |
| `python3 scripts/check_docs.py` | passed | — |
| `git diff --check` | passed | — |
| GUI 验证（见下节） | passed，含一项已知边界 | 2026-10-16 经用户同意占用桌面 |

新增测试（`apps/joyword/src/input_ui.rs`）：

- `ime_bounds_are_window_logical_pixels_at_zoom_and_live_scroll`：100 段多页文档，光标在第 2 页首行；缩放 80%／100% × 滚动 (0,0)／(−50,−1400) 四种组合下，用 A4（595.3×841.9 pt、90 pt 侧边距）写死期望的窗口逻辑像素（如 100%、无滚动时光标 x=169、第 2 页顶 y=1246.5333），并验证该框经命中测试映射回同一 UTF-16 偏移。
- `pending_preedit_layout_answers_from_the_origin_anchor`：段首开始组合 "AV"，预编辑排版未到达时，旧逻辑（无锚点）返回 None；有锚点时组合内 4 种范围都返回与空闲时精确结果相同的框，80% 缩放和额外滚动 30 px 时随之变化；组合外 3 种范围返回 None；预编辑排版到达后精确结果与锚点一致，组合末尾光标在同一行更靠右。
- `pending_preedit_reveals_its_origin_before_appkit_asks`：滚到顶部、组合起点在视口下方时，系统会问到的框在视口外；`reveal_target` 用锚点给出起点并标记“仍欠精确滚动”，按其滚动后同一查询落在视口内；预编辑排版到达后改为活动端的精确光标。
- `origin_anchor_is_dropped_for_other_versions_or_origins_and_queries_clamp_to_end`：已提交版本变化、起点移动时不使用锚点；`len..len+1` 与 `len+4..len+9` 钳制为文末光标。

既有 `page_geometry_round_trips_at_zoom_with_live_scroll_on_multiple_pages` 改用新的 `Placement` 后继续通过。

## GUI 验证

时间与方式：2026-10-16 16:50–16:58，经用户同意占用桌面。`cargo build -p joyword --features dev-runtime-shaders --locked` 后 `python3 scripts/package_macos_dev.py` 包装为 `target/dev/JoyWord.app`，`open -n` 启动；窗口 1100×883，位于屏幕 (410,87)。按键经 System Events（Ctrl+Space 切换到简体拼音，`keystroke "nihao"`）；滚动用会话临时目录中的 Swift 小工具发送像素滚轮事件（CGEvent，未提交）；`screencapture` 全屏截图后裁剪核对。追踪版另以同一补丁分别构建基线与修正版本，同样包装、以 `open --env JOYWORD_IME_TRACE=1 --stderr` 启动。截图未提交（与迁移报告一致）。

| 场景 | 结果 |
| --- | --- |
| 基线（616441f＋追踪），80%，文首输入 `nihao` | 候选条在屏幕左下角，复现冒烟现象；追踪全部 `ready=false -> None` |
| 修正版（80e23f1＋追踪），80%，文首 | 候选条紧贴组合文本 “ni hao” 下方；追踪见“根因”中的运行时证据 |
| 修正版（包装构建），80%，启动后首次输入 | 紧贴组合文本下方 |
| 同上，100% | 紧贴组合文本下方 |
| 100%，滚轮下移 90 px（光标仍在视口内） | 紧贴组合文本下方，随滚动位置变化 |
| 100%，滚轮下移 180 px（光标滚出视口），修正前一版 80e23f1 | **候选条出现在组合文本上方、压在提示横幅上**：系统查询时页面尚未滚回，预编辑排版到达后页面才滚回。据此补了“先滚动、后回答” |
| 补充修正后（5dcba33），80% 与 100% 各滚到页面底部（光标滚出视口）后输入 | 页面立即滚回，组合行位于视口顶部内侧，候选条紧贴其下方 |
| 100%，方向键移到第 4 行“正”字之后输入 | 候选条紧贴该行组合起点下方 |
| 空格提交“你好”后立即输入 `shijie`（同一脚本内，间隔为毫秒级） | 候选条停在上一组的起点（同一行，比新起点偏左约 47 px），见“已知边界” |
| 有未保存修改时 Cmd+Q →“不保存” | 退出；随后输入源切回 ABC |

另见现象（不计为本修正的问题）：第一轮两次重新启动修正版后，紧接着的第一次 `nihao` 没有到达应用（追踪中只有一条未组合时的 `0..0` 查询，文档无标记文本），系统拼音自己把候选条放在屏幕左下角，stderr 有系统输出 `error messaging the mach port for IMKCFRunLoopWakeUpReliable`；按 Esc 后再输入即正常。第二轮在相同条件下（上一实例退出后不切换输入源直接启动）基线与修正版的首次输入都正常进入组合态，未能稳定复现。按键未到达应用时不经过 `bounds_for_range`，与本修正无关；判断与上一进程退出时残留的输入法会话有关，未进一步定位。

## 已知边界

- **提交后立即输入下一组拼音。** 提交改变已提交版本，旧锚点失效；若下一组的首次按键早于任何新排版到达（提交的排版会被新预编辑请求取消），系统拼音收到 None，实测它沿用上一组候选条的位置。偏差限于同一行、约一个词宽。正常输入节奏下（两次按键间隔大于约 60 ms 的排版＋光栅时间）不会出现。
- **系统拼音不响应 `invalidateCharacterCoordinates` 重新查询**，所以任何在查询之后才到达的精确几何都无法再移动当前候选条，只能保证查询当下的答案正确。

## 接口或依赖提案

无。未改公共接口、依赖或锁文件。

建议（需协调者裁决，涉及 C06／C07 画布流水线）：把“排版就绪”与“光栅就绪”分开，排版完成即可供几何查询，光栅稍后替换图像。这样每次按键的空窗从“排版＋整页光栅”（调试构建约 60 ms）缩短为仅排版，也能让“提交后立即输入”在多数情况下拿到精确几何。

## 阻塞与下一步

- 无阻塞。
- 未覆盖：Retina 与外接／多显示器（本机仅 1×）、Windows（`bounds_for_range` 同样被 Windows IME 使用，锚点逻辑平台无关但未实测）、Linux Fcitx／IBus、发布构建（预编译 Metal）下的时序。
- 建议协调者在合入 gpui-pre 迁移后再合入本分支，并在 C07 的未完成项中把“系统拼音候选框实际坐标”改为引用本报告的实测结果。
