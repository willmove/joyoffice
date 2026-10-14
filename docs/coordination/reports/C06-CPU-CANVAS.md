# C06：CPU 页面与真实只读画布（2026-10-14）

- 执行者：当前 Codex 协调者，单人执行。
- 任务卡：[C06](../tasks/C06-CPU-CANVAS.md)。
- 基线：2b29c84d6c51a8ecfffe14418d8b9325d7bb5dcc（C05）。
- 分支：codex/coordination-cpu-canvas。
- 独立目录：/Users/willm/.codex/worktrees/joyoffice-coordination/joyoffice；启动时干净。
- 主机：Apple M4／24 GiB，macOS 27.0.1 arm64；Rust 1.98.1、edition 2024。
- interface_ready：true，单色轮廓 PageScene CPU 消费者；真实只读文档后台画布已编译接入。
- implementation_verified：true，20 项内核测试、4 项新增应用适配测试、4 项核对器反例、完整默认成员与独立图像对照通过；运行时 UI 分开记录。
- acceptance_complete：false；用户已选择后台开发，GUI／IME／Office 与真机验收未执行，T003/T005/T105 仍 in_progress。

## 实际实现与依赖裁决

规格 ARCHITECTURE §6.2 明确要求 CPU 光栅消费者，用于页面预览、无窗口测试和渲染退路。本轮新增 joyword-render，复用锁文件既有精确 Skrifa 0.42.1、tiny-skia 0.11.4；app 的内存图像适配复用 image 0.25.10（不打开额外默认特性）。只新增内部 crate 和依赖边，没有新增／升级外部包。model／engine／OPC／字体／整形／PageScene 现有 API 未改，W02 实验和 vendor 未改，GPUI 仍为原 registry 0.2.2。

CPU 渲染直接依 FontKey 查找同一字体字节／collection face，Skrifa 共用现有 read-fonts 0.39.2 读取默认轴位置的未 hint 单色轮廓。按场景中 glyph id、字号、origin、advance 与 x/y offset 光栅，字体向上 y 转为页面向下 y；不再次整形、断行、名称匹配或更改原文。PNG 为白底、不透明 RGBA，BGRA 交给 GPUI RenderImage 时显式交换通道；PDF 继续从原场景写出可检索文字，不使用光栅页面替代 PDF 正文。

预检拒绝缺字体、重复 FontKey、非法 glyph／cluster／笔位置／溢出坐标。彩色、bitmap、SVG 字体尚未实现，检测到相应表就保守拒绝，不能把单色 base outline 当成功的彩色渲染；空白字形的空轮廓合法，但缺失轮廓表是错误。只支持当前场景已经表达的单色字形，尚无图片／路径／链接等未来场景扩展的绘制。

硬上限：边长 8,192 px、总像素 16,777,216、64 字体／合计 256 MiB 字节、100,000 run、2,000,000 字形、8,192 缓存轮廓、1,000,000 轮廓命令、268,435,456 填充像素和 33,554,432 边缘工作单位。调用者只能降低。重复绘制缓存轮廓仍按其命令复杂度×可见扫描行数收费，不用 cache 命中绕开工作预算。共享单调取消在预检、字形和轮廓命令之间检查；单次通用库解析／光栅和 PNG 压缩内部不能强制抢占，未测取消尾延迟。候选在局部构造，失败不返回成功半幅图，不修改场景／字体。

正式窗口不再把正文交给 GPUI 默认文字系统；后台加载系统 CJK 字体或 JOYWORD_FONT=path:face，建立真实 model 与只读 engine 会话，读取 DocumentSnapshot，生成 LayoutSnapshot 与 CPU 页。原始字体 IO 限 128 MiB；缺失／缺字／预算失败显示错误。按窗口实际 device scale×96 dpi 和 zoom 生成页面像素，物理布局不变。页面缓存合计不超过 16,777,216 px；高 DPI／长文档超限明确失败，后续需可见页／分块策略，而不是无声降低显示精度。

后台结果同时检查 LayoutKey（版本／内容／字体／profile）、Projection 与唯一请求序号；同一键失败或空闲重绘不重新调度，语言切换不翻译正文或触发光栅。新投影取消旧工作，晚到结果不覆盖当前页，关闭使用弱 view 句柄。替换／失败时显式调用 GPUI drop_image 释放旧纹理；其运行时占用仍待 GUI 测量。后台生成 RGBA→BGRA 和 RenderImage，UI 线程只接收完成结果与绘制。菜单／快捷键／按钮继续走原命令注册表。

该路径实现的是 CPU **页面**后端。GPUI 窗口仍使用其平台 GPU 后端，不能据此把无 GPU 窗口、GL 退路、GPUI FontId 原生字形通道、Retina 真机、信创平台或 ADR-010 标为完成。产品仍是明确只读预览，未提供编辑／保存假入口；T102 输入、IME、辅助技术及完整 Word profile 继续保留原目标。

## 自动验证

所有 Cargo 命令使用 CARGO_TARGET_DIR=/Users/willm/Coding/joyoffice/target，源码来自本独立目录。字体命令设置 JOYWORD_TEST_FONT='/System/Library/Fonts/Supplemental/Songti.ttc:6'、JOYWORD_REQUIRE_TEST_FONT=1，实际执行，未跳过。

| 命令 | 实际结果 |
| --- | --- |
| cargo test --locked | 默认成员 220 passed，0 failed／ignored；含 render 新增 20 项 |
| cargo test --release --locked -p joyword-render -p joyword-layout -p joyword-pdf | 70 passed：20／45／5；0 failed／ignored |
| cargo test -p joyword --features dev-runtime-shaders --locked | 5 passed，含既有注册表测试与 4 项新增适配测试；不启动窗口 |
| cargo test -p joyword-render --example compare_rasters --locked | 4 passed，含 2 px 偏移和内部漏字的拒绝反例 |
| cargo clippy --workspace --all-targets --features dev-runtime-shaders --locked -- -D warnings | passed；既有 block／proc-macro-error2 future-incompat 提示仍在 |
| cargo fmt --all -- --check | passed |
| cargo check --locked -p joyword-render --all-targets --target x86_64-pc-windows-msvc | passed；交叉检查，不是 Windows 运行 |
| cargo check --locked -p joyword-render --all-targets --target aarch64-unknown-linux-gnu | passed；交叉检查，不是目标机运行 |
| cargo build -p joyword --features dev-runtime-shaders --locked | passed；开发构建，不是 GUI 或发行验收 |
| PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests | 13 passed |
| python3 scripts/check_docs.py | passed：82 个需求、58 项任务、9 个登记用例 |
| git diff --check | passed |

新测试包括确定性与原场景／字体不变、逐像素平移、y offset 方向、zoom 与 DPI 等价投影、白底与空白轮廓、PNG 解码、BGRA 通道、实际字节／face 身份、所有预算及硬上限不可放宽、缓存命中后的工作计费、非法 glyph／cluster／坐标／溢出／缺失轮廓、彩色表保守拒绝、共享取消。CJK 测试同时检查真实 collection 的另一 face 不能替代相同文件散列下指定的 face。

请求门测试验证重复／失败键连续 1,000 次不产生新票据、A→B→A 投影的旧 A 结果仍拒绝、DPI／字体／profile 不同均拒绝。应用无窗口测试实际构建只读 Session，验证 engine 拒绝写入、场景内容、BGRA RenderImage 字节、设备倍率、缺字体／非法 spec／缺样例字形、取消和修改会话后旧布局拒绝。没有把这些替代为真实事件循环、纹理占用或 UI 交互测试。

## 同场景 CPU／PDF 独立对照

已应用 PDF 渲染复核流程；以下生成物仅位于忽略的 target，不提交系统字体或媒体：

```sh
cargo run -p joyword-render --example cpu_scene --locked -- /System/Library/Fonts/Supplemental/Songti.ttc 6 /Users/willm/Coding/joyoffice/target/c06-cpu-scene
/Users/willm/.cache/codex-runtimes/codex-primary-runtime/dependencies/bin/override/pdftoppm -r 144 -png /Users/willm/Coding/joyoffice/target/c06-cpu-scene/reference.pdf /Users/willm/Coding/joyoffice/target/c06-cpu-scene/pdf
cargo run -p joyword-render --example compare_rasters --locked -- /Users/willm/Coding/joyoffice/target/c06-cpu-scene/cpu-1.png /Users/willm/Coding/joyoffice/target/c06-cpu-scene/pdf-1.png
cargo run -p joyword-render --example compare_rasters --locked -- /Users/willm/Coding/joyoffice/target/c06-cpu-scene/cpu-2.png /Users/willm/Coding/joyoffice/target/c06-cpu-scene/pdf-2.png
```

实际结果：20 段原创合同种子，2 页／24 行／548 字形；144 dpi，两页均 840×794 px。CPU 第一页 350 字形、第二页 198 字形，各缓存 102 轮廓／5,882 命令；填充工作量分别 118,743／68,077 像素。Poppler 独立光栅后逐行墨迹边框最大偏差均为 0 px（16 行／8 行），全部深色笔画在对方 1 px 内有实际墨迹；CPU／PDF 四幅页面均实际查看，无裁切、重叠、漏字。比较同时核对图像尺寸、逐行墨迹边框及双向笔画覆盖；不是只看整页平均相似度。

核对器明确用灰度 <150 定义深色笔画、<250 定义实际抗锯齿墨迹，位置邻域固定 1 px。最初同一灰度阈值核对发现第一页 101／第二页 79 个细笔画像素灰度跨阈值；实际检查其邻域仍有灰度 151–191 的墨迹而非白色，遂区分强笔画与抗锯齿覆盖，未放宽位置容差。2 px 偏移和内部字形缺失仍被反例测试拒绝。该比较不是场景原点 0.01 pt 或 Word 几何的替代证据，hinting／抗锯齿算法可不同。

字体 Songti.ttc face 6（与 C05 的 face 0 分开），SHA-256 dd2f0628d302f322a2e9c3cf3253d5e73f418275e1f1421c84a7c65ce5a87777，仅本机使用；固定小字体测试使用仓库 OFL fixture。CI 用 Noto CJK 的实际结果需另行绑定源码与字体记录。已加入多页 CPU→PNG／PDF→Poppler 对照、核对器测试及四平台应用无窗口测试，但本提交远程结果须推送后核验，不沿用 C05 8／8 通过结论。

## 后续

继续以 T102 的系统输入／组合态、model 位置与画布几何联动为优先项；T103 原件保存、结构／格式导出、文件锁／元数据与 T104 恢复仍未完成。GUI 按用户“先继续后台开发”要求暂缓，本轮未启动或操作桌面，没有宣称 IME、Word/WPS、DPI 真机或新画布人工验收通过。来源及通用库许可见 [PROVENANCE](../../joyword/PROVENANCE.md)。
