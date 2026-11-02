# 桌面文本格式导出验证

2026-11-02，M0–M7 持续收口的第二批。基线 `95f632dee778d53bee67d9391c6c1b402861bca3`，分支 `codex/joyword-1-0-completion`。T600／T601 保持 `in_progress`；M0–M7 仍为 17 done／34 in_progress／4 planned，开放 38 项。

## 实现

- “文件 → 导出文本格式…”提供 Markdown／HTML／RTF／TXT。转换差异确认默认取消，TXT 单独明确所有格式丢失，随后选择新路径。组合输入、只读或样式近似预览禁用导出。
- 共享 `exchange::plan_document` 从已提交模型快照生成计划，先检查模型文字、段落和 run 预算，再投影已声明子集。隐藏文字、审阅、批注、额外 story 和复杂容器继续拒绝。
- CLI 与桌面共用 `exchange_output::write_new`：先确认及检查取消／扩展名，后台同目录临时写入、同步、独立读回，最后硬链接发布，拒绝覆盖已有路径。发布后清理失败会明确提示副本已经存在。
- 桌面确认前、路径选择前后核对文档代次与版本；窗口关闭请求取消。输出只生成副本，不清 dirty、不变更来源路径，也不删除撤销历史。后台写入后的新修改保持未保存，并提示生成的是较早快照。
- 原生验证发现窗口文件弹层独立写死命令列表，导致注册后的新命令不可见；改为从命令注册表生成 File 弹层并新增三平台菜单回归。
- 修复缺字诊断遮住文件操作结果的问题：完成／取消提示优先显示，下一次正常编辑后恢复版式诊断。

## 自动验证

当前代码 `cargo xtask check joyword` 通过：fmt、严格 Clippy、1314 项测试，零失败／忽略。`cargo xtask build joyword` 通过。Python 工程工具回归 146 项通过。具体命令、完整日志和当前源码散列见 [本批索引](../validation/20261102-exchange-ui/verification.json)。第一批的 Deck／Sheet 回归及 1309 项 Word 检查是历史证据，见 [第一批报告](m0-m7-completion-20261102.md)，不把其源码散列覆盖成当前版本。

新增测试覆盖未确认／取消时零写入、扩展名错误、已有目标保护、字节读回、当前模型与 DOCX CLI 路径等价、文档版本及原包不变，以及格式选择、文档切换／版本变化与窗口 File 菜单入口。

## Windows 原生验证

使用独立测试 profile，打开第一批的原创 DOCX，在正文末尾输入未保存标记 ` UI_EXPORT_1`。实际经过格式选择、损失确认及原生保存对话框，分别生成四种副本；四份产物由 CLI 再独立导入为 TXT，精确得到：

```text
中文交换 😀 斜体 加粗
Literal * and <xml> & value UI_EXPORT_1
```

Markdown／HTML 保留粗体和斜体标记，RTF 采用 Unicode 转义，TXT 明确丢弃格式；语义及字节产物见 [读回索引](../validation/20261102-exchange-ui/native-readback.json)。源 DOCX 在两次独立启动中均与输入逐字节相同，SHA-256 为 `7e31b837e5f75ceba65517173035f8dfa61d452f4bcd4a183e9bc9b9335ae843`。

格式选择与差异确认均执行 Escape 取消；路径取消另外输入 `cancel-at-path.txt` 后 Escape，目标不存在，正文及未保存标记保留。[Markdown 导出后](../validation/20261102-exchange-ui/native/04-markdown-exported-dirty.jpg)、[TXT 损失提示](../validation/20261102-exchange-ui/native/07-txt-loss-review.jpg)、[TXT 导出后](../validation/20261102-exchange-ui/native/08-txt-exported-dirty.jpg)、[路径取消后](../validation/20261102-exchange-ui/native/09-path-cancelled.jpg)为原始窗口／原生对话框捕获，未修改图像像素。

首个桌面接口在重开窗口时报告 `foreground window did not report a process id`，重新绑定一次仍失败；随后已配置的 `cua_repl` 接口成功选择同一生产窗口，完成最新 TXT 文案、导出及路径取消验证。前三种格式使用同一实现的前一构建；后续只增加 TXT 专用损失文案及状态栏结果优先级，完整自动检查重新通过。最新构建再次实际取消导出，[状态栏显示“文本导出已取消”](../validation/20261102-exchange-ui/native/10-cancel-notice-visible.jpg)，不再被缺字诊断遮挡。

## 未覆盖与后续

本批仅完成声明范围的桌面导出。桌面文本打开／导入、Strict、复杂审阅／脚注／表格交换和完整支持矩阵尚未完成，不能关闭整个 T600／T601，也不代表 1.0 发布通过。GUI 自动操作与视觉核对不等于系统 IME、实体键盘、读屏、物理断电或跨软件完整人工验收；当前画布 emoji 缺字诊断仍存在，模型和导出文字没有被替换。

用户已明确暂无 macOS／Linux 桌面环境及发行签名身份，相关当前候选验收／正式签名保留未验证；其他实现工作继续推进。信创真机按既有决策非阻塞后补。本批没有提交或推送。
