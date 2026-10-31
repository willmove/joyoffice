# M6-alpha-hotfix1：保存确认弹窗

修复“保存更改”弹窗点击保存无反应，并选中或拖动底层单元格/绘图对象的问题。

原因为遮罩层只有背景绘制，没有设置 GPUI 鼠标遮挡命中区域。底层网格在捕获阶段处理鼠标按下并停止传播，按钮没有收到完整点击。命令面板、侧栏、最近文件及打印预览的浮层也补上了对应遮挡。

关闭确认现在拥有独立焦点，阻挡工作表键盘、滚动、拖选和对象操作；Esc 取消并恢复焦点，Enter/Ctrl+S 保存后关闭。保存或文件选择期间显示“保存中…”并禁止重复操作。保存失败保留弹窗和工作簿，在弹窗中显示错误，允许重试；保存使用原窗口、文档及版本校验，取消文件选择可继续操作。确认期间暂停自动保存，避免后台写入。

原生回归通过生产界面的真实按钮坐标点击，覆盖：

- 弹窗遮罩下的鼠标拖选、滚动、键盘输入及快捷键不影响工作簿。
- “取消”不写入文件；“保存”写入中文及对象修改，成功后才关闭；重开 XLSX 验证实际磁盘内容。
- 独占锁定文件，点击“保存”触发错误，保留文档并解除忙碌状态；释放文件锁后点击同一按钮成功保存，重开验证。
- “不保存”关闭窗口且文件哈希不变。

```powershell
pwsh -File scripts/joysheet/dev.ps1 xtask ci joysheet
pwsh -File scripts/joysheet/desktop-smoke.ps1 -Stage M1 -SaveDialog
pwsh -File scripts/joysheet/desktop-smoke.ps1 -M4Task 4 -M5Task 4 -M6Task 4 -SaveDialog -BinaryDirectory dist/joysheet-0.1.0-M6-alpha-hotfix1-windows-x86_64
```

修复包使用独立目录，旧版本不会被覆盖或自动更新。旧窗口若有未保存修改，可按 Esc 退出旧弹窗，再按 Ctrl+S 保存，之后运行修复版。

证据：[Debug 回归](evidence/save-dialog-debug.json)、[保存失败弹窗](screenshots/save-dialog-error.png)、[遮罩验证](screenshots/save-dialog-blocking.png)。最终 Release 回归见 [save-dialog-release.json](evidence/save-dialog-release.json)。

最终解压的 Release 修复包通过全部 30 项 M3/M4/M5/M6 与保存弹窗原生回归，程序 SHA256 为 D6EED857295E2255CE822C56C98801EC0FF2FEC2CAE6D4F395B75092FB12B0FB。全工作区 1353 项通过、1 项忽略，严格 Clippy 和分层检查通过；[CI 汇总](evidence/save-dialog-ci.json)。

