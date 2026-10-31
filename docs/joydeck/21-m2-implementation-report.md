# M2 基础编辑切片实施记录

2026-10-29，Windows x64，Rust 1.98.1，GPUI Kit 0.8.1/pre 0.3.10。按用户授权在保留 M1 未测项的同时推进 M2。完整 M2 尚未验收。

## 已交付

- edit.rs：独立类型化命令、Session 身份、验证后提交、不可变快照、单调 revision、100步/32 MiB历史预算、undo/redo分支。非法几何/目标/末页删除失败不发布半份文稿。
- editor.rs：默认启动原生编辑器；新建、页面增删复制、基础矩形/椭圆/直线/文本框，画布或列表单对象选择，位置/尺寸/颜色/纯文字属性应用与对象删除。PPTX和参数路径仍通过 M1 只读查看器，--viewer可打开空查看器。
- 保存：后台快照另存为 .joydeck，临时文件create_new、ZIP完成并sync后提交。首次保存用同目录硬链接创建目标，目标已存在则拒绝，避免检查到提交间的覆盖竞态；文件系统不支持硬链接则明确失败。内部恢复快照使用rename替换。旧保存只标记自己的快照，后续编辑仍dirty。
- 重开：独立后台读取；未知schemaMinor/字段/额外资源拒绝编辑，仍可只读查看。打开期间发生编辑则丢弃结果。未处理dirty时阻止替换。
- 生命周期：OS窗口关闭、编辑器关闭动作、从只读窗口发起的全局退出均检查dirty/读写状态；允许取消或明确放弃。另存后可正常关闭。
- 恢复：每次已提交变更后台写本机独立会话快照，按revision防旧任务反写；支持读取最近快照为dirty副本。保存源文稿与恢复副本路径分离；不保存属性输入草稿。

## 按步骤实际运行

A：编辑内核/v1写入build通过，真实M0原生窗口smoke通过，见 [step-a](evidence/m2/step-a.log)。

B：编辑器build成功，真实窗口启动且响应；制作三页、插入形状/文字、撤销重做、保存重开/恢复验证通过。见 [step-b](evidence/m2/step-b.log)、[截图](evidence/m2/step-b.png)。

C：加入关闭保护、跨窗口退出保护、严格编辑导入、首次保存不覆盖；check/stage M2通过。最终分发EXE实际执行属性应用、dirty关闭弹窗/取消、UI异步另存后clean、三页重开及恢复，见 [分发smoke](evidence/m2/desktop-smoke.log)、[窗口截图](evidence/m2/packaged-editor.png)。这些测试调用真实窗口的生产方法，不替代物理鼠标键盘、OS文件系统故障或异常断电验收。

检查：[fmt/Clippy/14项测试](evidence/m2/check-m2.txt)。包括错误事务回滚、文档身份、历史上限/redo分支、旧保存不清dirty、保存往返、已有目标内容保持，以及全部M1读取/像素/安全测试。[stage M2](evidence/m2/stage-m2.txt)运行时为新增最后两项测试前的12项，最终check为14项。M1实际桌面回归通过，见 [日志](evidence/m2/m1-regression.log)。

## 当前范围与待验收

这是基础编辑增量，不是完整M2交付。命令元数据/schema目录、复合事务、稳定对象ID、多选/框选/重排/隐藏、页面尺寸工具、箭头/圆角矩形、拖动/缩放/旋转及Esc取消、透明度/描边、多选混合值仍待补。只支持当前基础v1模型，无资源索引或未知字段写回能力。编辑器使用整页后台CPU预览，选框命中为对象矩形边界。

当前UI只允许新路径另存；覆盖保存的fingerprint/冲突交互、保存后关闭的一体化对话框仍待补。恢复目前为最近快照入口，没有完整列表、配额、debounce、源fingerprint/跨实例占用标记或损坏隔离；最多扫描1024个目录项。未做真实进程崩溃/断电、跨平台和长期资源验收。恢复快照保存在 LOCALAPPDATA/JoyDeck/recovery，包含本机文稿内容，正常保存不自动删除。

所有M2父任务保持未勾选，已完成范围以11的增量子项记录。下一步继续T-M2-03/04直接操作与事务、07覆盖保存/关闭流程、08恢复管理及AT-03/06/14完整演示。

## 产物

`dist/joydeck-m2-windows-x64/joydeck.exe`、同名ZIP、`dist/joydeck-m2-source.zip`。EXE SHA256：BEC890C84166E9161BC94627262229C55BBD65AC525ABA69C8700C7663139984。M0/M1包保持可回退。无Git元数据，未创建提交；无远端CI、签名或安装器发布。

运行：pwsh.exe -File scripts/dev.ps1 run；验收：pwsh.exe -File scripts/smoke-m2.ps1；只读：cargo run --locked -p joydeck -- --viewer。打包脚本现在生成M2目录，历史包不覆盖。
