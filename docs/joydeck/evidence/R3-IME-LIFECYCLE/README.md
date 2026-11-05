# R3 原位输入法组合生命周期

2026-11-05，Apple M4、macOS 27.2、1× 实屏。移位解压 `joydeck-0.8.0-r3-ime-lifecycle.zip` ARM64 Release 验收包，1,805 项载荷散列一致。归档 SHA-256 为 `cf26e4569441f41bde92bcba6c8cc3ffafad739474fface295de2ba7ad4877a9`。[两页样例](lifecycle.pptx) · [验证数据](verification.json)

旧包在拼音候选打开时点击第 2 页，会先把 `ni` 提交到第 1 页，再切页。修复后，原生输入服务结束组合时，提交先留在当前文字草稿。应用鼠标按下事件可取消同一次事件触发的提交。窗口失活会释放草稿。确认选词仍通过单个 `set-text` 事务发布。空 preedit 可以重复清除，原文、选区方向和输入样式得以保留。[AppKit 组合结束接口](https://developer.apple.com/documentation/appkit/nstextinputclient/unmarktext()) · [Windows 取消组合消息](https://learn.microsoft.com/en-us/windows/win32/intl/wm-ime-composition)

| 操作 | macOS Release 实测 |
| --- | --- |
| 重复更新、取消 | 系统简体拼音收到 `n`、`ni`、`nih` 与退格，真实候选显示“你”。连按两次 Esc 后两页原文、修订 0、干净状态及空撤销历史均保持。[候选](01-pinyin-candidates.png) |
| 切页、失焦 | 候选打开时点击第 2 页缩略图。原页和目标页文字、修订 0 和空撤销历史均保持。再次组合时，将 OS 前台切到独立 AppKit 窗口后切回，结果同样保持。[切页结果](02-slide-switch.png) |
| 确认和历史 | 连续两次按数字键确认候选“你”，两次各产生一个修订，得到“你你”。原生编辑菜单的一次撤销得到“你”，一次重做得到“你你”。两次独立复验分别用 Return 确认字母 `ni`，以及双击候选“尼”，各得到一个修订。 |
| 另一种系统组合 | ABC 布局 Option+E 显示重音标记，随后 E 提交 `é`。仅一次文字事务。原生撤销和重做精确恢复文字。[重音草稿](03-abc-dead-key.png) · [提交结果](04-abc-commit.png) |
| 另存与恢复 | 原生另存为后，包内 CLI 独立读回两页。产生一个未保存的确认选词，在第 8 个修订的恢复记录落盘后再次开始拼音。候选仍显示且查询返回 `busy` 时终止验收进程。CLI 恢复得到完整的修订 8，源 PPTX 和已另存文稿散列保持。[另存](05-saved.png) · [终止前组合](06-active-preedit-before-crash.png) |

JoyDeck 产品检查通过 915 项，0 失败、0 忽略。包内 CLI 通过 R1、R2、R3 的 28、20、20 项检查及 329 条注册表核对。JD-303 的字素删除、剪贴板、两处拼音候选及另存读回也用同一包复验。Windows 空替换取消和 Linux 删除 marked range 的适配按固定 GPUI 版本的事件实现核对。Windows/Linux 系统输入实屏仍由平台任务跟踪。
