# JoyDeck JD-209 编辑器开发包验收

2026-11-05，Apple M4、macOS ARM64。把保存发布与编辑命令修复合入同一验收分支，以 Release 验证包在仓库外中文空格路径运行。测试使用独立的临时状态目录，系统键鼠操作真实窗口。附加控制端仅读取实际文稿、选择、修订和脏状态。

1. 用原生菜单新建文稿，Ribbon 新建第二页并插入矩形。实际拖动保留对象 ID 和宽高，水平与垂直位移约 78 pt、54 pt。单次撤销恢复原位置，重做恢复同一变换。[插入](01-inserted-rectangle.png) · [拖动](02-dragged-rectangle.png)
2. 原生另存为写出 v2 文稿，正常关闭应用并以该文件重开。窗口与包内 CLI 独立读回相同的两页、对象 ID 和几何，重开后的编辑历史从空栈开始。[已保存](03-saved-clean.png) · [重开](04-reopened.png)
3. 重开后插入椭圆，撤销回到磁盘上已保存的干净内容，重做恢复相同新对象 ID。等待修订快照写入后强制终止测试进程。包内 CLI 确认该记录从占用中变为可恢复，并独立恢复出相同的两对象模型。[崩溃前](05-dirty-before-crash.png)
4. 重启桌面应用，从恢复列表选择同一份快照，核对两页、两个对象及其稳定 ID。另存为新文件后，再由包内 CLI 读回；原保存文件的 SHA-256 与强制终止前一致。[恢复列表](06-recovery-choice.png) · [恢复后的对象](07-recovered-objects.png) · [恢复文件](08-recovered-save.png)

`cargo xtask check joydeck` 通过 904 项测试，失败和忽略均为 0。使用 `cargo xtask package joydeck --release --validation --name joydeck-0.8.0-r2-editor-qualification` 生成开发包，解压核对 1,805 项文件散列。同包 CLI 通过 R1 28 项、R2 20 项、R3 20 项及 329 条命令注册核对。[包、模型与截图数据](verification.json)

本次桌面验收在 macOS 1× 执行。JD-202 的 2× 实屏和其他平台窗口继续按各自任务跟踪。
