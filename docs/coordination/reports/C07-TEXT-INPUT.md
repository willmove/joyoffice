# C07：会话输入、选择与 IME 适配（2026-10-15）

- 执行者：当前 Codex 协调者，单人执行；任务卡：[C07](../tasks/C07-TEXT-INPUT.md)。
- 分支：codex/coordination-text-input。
- 基线：cb219b31c1d2ed4c8ba56b0df80be4815bc3ce09（C06，含 Windows EBDT 修正）。
- 独立目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice。
- 环境：Apple M4／24 GiB、macOS 27.0.1 arm64、Rust 1.98.1、edition 2024。
- interface_ready：true；纯文本输入内核、DOCX 适配与 GPUI EntityInputHandler 已接通。
- implementation_verified：true；以下无窗口测试与编译通过，真实平台输入行为分开记录。
- acceptance_complete：false；按用户“先继续后台开发”，本轮未占用桌面或启动 GUI；T102 保持 in_progress。

## 实现与边界

shell::input 以 engine 为唯一提交入口。TextIndex 按正文段落建立 UTF-16／模型标量投影，段间 LF 和实际段落 ID 严格对应；反向选择保持锚点／活动端。半 surrogate、未知位置、反向或越界请求明确失败。多行替换以已知起点反向插入／分裂，单个事务完成，不猜测新段落 ID。跨段替换、空段落和段落标记删除可撤销。

组合态只在临时 Session 的不可变快照中排版：连续、局部、周边范围更新折叠到原提交区间，真实会话与历史在确认前不变。确认或 AppKit unmark 提交一次事务；空预编辑、Escape、失焦与窗口失活丢弃预编辑。unmark 保留标记文本，不等同于显式取消。输入会话版本竞争、显示修订溢出和所有预算在实际提交前检查；取消后的后台场景不能安装回来。平台回调本身没有组合 token，实际平台取消／迟到回调顺序仍须人工验证，不能把后台任务门当作 OS 输入事件排序证明。

使用已有精确 unicode-segmentation 1.13.3 的 Unicode 17.0 默认扩展字素边界，左右移动与删除不拆 emoji ZWJ、旗帜、肤色或组合序列；不做规范化。词选择采用默认词边界，未提供中文词典分词。多行 Enter、撤销／重做、全选、正反扩选、上下行移动、行首／行尾、鼠标命中／拖选／双击词／三击段通过同一注册表接入。注册 19 个编辑命令，中英文标签齐全；输入失败提示本地化。

engine 保留既有 apply 行为，仅新增 apply_with_mapped_selection，把撤销前选区与需要映射的最终目标分开；只读 preview_undo／preview_redo 用于在恢复前检验输入预算，真实历史仍由 engine 管理。DocxSession 只转发新入口，保持原包 backing、诊断与 readonly／locked 限制；没有公开可变底层会话以绕过策略。保守导出仍拒绝结构／格式等不安全变更。

正式窗口通过后台布局与 CPU 光栅显示输入快照。请求键含显示修订、实际字体字节／face、profile 和投影，另有序号防止 A→B→A 的旧结果。不同预编辑即使共享 engine 版本也有不同修订；只有完整 ensure_current 验证通过，才可在确认相同预编辑时复用图像。选择、光标、标记下划线、候选范围与点击命中来自当前布局；过期画布不参与位置映射。后台等待期间保留上一帧页面以稳定滚动容器，并继续拒绝旧几何；新布局／滚动／选择改变后按几何身份刷新 IME 坐标，不制造空闲持续帧。滚动偏移实时参与坐标变换，覆盖层有视口裁剪，活动光标按需滚入视图。缺字保留原文并提示，不宣称已完成字体回退。

窗口关闭与注册的 Quit 命令共用未保存提示：取消为首项，只有明确不保存才丢弃；提示绑定版本，回答后出现新修改须重新确认。预编辑本身不变脏；撤销到初始文字仍以单调版本保守标为未保存。应用明确提示会话内暂存，没有保存假按钮。GPUI 的系统终止回调不能否决 OS 终止，本轮不承诺系统注销、强制结束或崩溃恢复；文件打开、保存和本地恢复仍为后续任务。

输入硬上限：正文 16 MiB、10,000 段、每段 65,536 UTF-8 字节、100,000 run、10,000 元数据项；单次插入 65,536 字节、组合 8,192 字节、2,048 命令。可降低预算，不能请求放宽硬上限。不支持正文对象或额外 story 时拒绝，不静默扁平化。CPU 页面总缓存仍限 16,777,216 px；全量投影／重排不构成 100 页输入延迟达标证明。

## 自动验证

所有 Cargo 命令使用本独立目录与 CARGO_TARGET_DIR=/Users/willm/Coding/joyoffice/target。字体测试显式设置 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:6'、JOYWORD_REQUIRE_TEST_FONT=1；系统字体不提交。应用无窗口测试使用仓库固定 OFL 小字体；不运行事件循环或平台自动化。

| 命令 | 结果 |
| --- | --- |
| cargo test --locked | 245 passed，0 failed／ignored |
| cargo test --release -p joyword-engine -p joyword-shell -p joyword-layout -p joyword-render --locked | 155 passed，0 failed／ignored |
| cargo test -p joyword --features dev-runtime-shaders --locked | 11 passed；无窗口 |
| cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings | passed |
| cargo fmt --all -- --check | passed |
| cargo build -p joyword --features dev-runtime-shaders --locked | passed；非发行 Metal 验收 |
| cargo check --tests --locked --target x86_64-pc-windows-msvc | passed；仅默认无界面成员 |
| 同上，x86_64-unknown-linux-gnu／aarch64-unknown-linux-gnu／loongarch64-unknown-linux-gnu | passed；交叉检查不等于目标运行 |
| python3 scripts/check_docs.py | passed：82 需求、58 任务、9 用例，文案／引用检查通过 |
| PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests | 13 passed |
| git diff --check | passed |

新增测试：独立 String 模型的 120 次确定性 Unicode 替换、周期撤销／重做；多次预编辑／局部及周边更新／确认／取消／unmark；UTF-16 代理对和跨段范围；字素移动与内簇删除；readonly engine／受保护 DOCX／原始导出往返；每类输入预算及失败原子性；历史恢复超预算前拒绝；过期 backend 与修订溢出；engine 选择与只读历史预览；真实输入→布局→CPU 像素、确认像素不变和撤销像素还原；相同预编辑版本的不同内容／取消后返回／取消工作拒绝；缺字不丢原文；多页滚动与 50／80／100／150% 投影回转、光标滚动限位及关闭提示版本策略。

测试发现并修正：指令预算原先按上界多计空插入，改为实际指令数且保留硬限；词／段选择在组合状态或非法偏移时不得先取消预编辑；范围空光标沿用实际头端 Bias，避免在软换行处错误跳到另一行；覆盖层改为明确左上定位并使用内容裁剪，不能用 paint_layer 的绘制分层替代裁剪。浮点投影回转误差验证不超过 4 EMU，命中验证取实际行盒内部，不把边界舍入差当作行内点。

## 交付与未覆盖项

本地提交和推送后交付真实 SHA；远程 CI 结果另行绑定本次源码，不沿用 C06 的成功运行。C06 修正 cb219b31c1d2ed4c8ba56b0df80be4815bc3ce09 已独立核验 [38559611373](https://github.com/willmove/joyoffice/actions/runs/38559611373) 8／8 成功，其最初 Windows 失败仍保留在 C06 报告。

未完成：系统拼音／Fcitx 与候选框实际坐标、真实焦点切换／菜单／鼠标拖选／DPI、辅助技术、拖放／剪贴板、格式操作、增量重排与性能实测；Word/WPS 往返、文件打开／保存 UI、原件覆盖／锁文件／元数据、日志恢复及完整中文排版。接口就绪和自动通过均不改变这些人工／产品门槛。下一优先项为文件打开与受限导出副本的真实窗口流程，并继续保留 DOCX 安全边界。来源见 [PROVENANCE](../../joyword/PROVENANCE.md)。
