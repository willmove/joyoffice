# JoyOffice macOS 桌面补验

2026-11-05 在解锁后的 Apple M4、macOS 27.2 上补验。使用已校验的 JoyWord 0.1.1 和 JoyDeck 0.8.0 ARM64 Release 开发包。文档操作使用测试副本和隔离的 JoyOffice 配置。

## JoyWord 千页输入

在固定 1,000 页样例上冷启动三次，每次使用键盘跳到第 1,000 页，在自然换行的末段逐字输入 `adghk`。三次首次绘制和 15 次编辑绘制均由 `JOYWORD_PERF=1` 记录，编辑绘制均发生在第 1,000 页。测试原件 SHA-256 一致。

| 阶段 | 中位数 | 最大值 |
| --- | ---: | ---: |
| 发起打开至首次 GPUI 绘制提交 | 114.7 ms | 140.7 ms |
| 末段编辑布局 | 0.31 ms | 0.45 ms |
| 输入回调至 GPUI 绘制提交 | 16.8 ms | 41.1 ms |

最高采样 RSS 为 221.0 MiB。计时止于 GPUI 提交，物理屏幕呈现、跨对象编辑和同机历史版本对照仍待验。[原始样本与二进制散列](thousand-page-gpui.json)

## Word 和 WPS 双向往返

使用 [JWREC003 补验](../20261105-macos-open/README.md) 中含表格、PNG、页眉 PAGE 域、插入／删除修订和额外 XML 部件的快照。分别用 Microsoft Word 16.114.1、WPS 12.1.29181 打开副本，在实屏键盘输入标记并保存，再用 JoyWord Release CLI 编辑并另存。每条链路两轮，最后由对应外部编辑器重新打开 JoyWord 的第二轮输出并保存一个独立标记。独立解包核对各步的正文标记、表格、原 PNG 散列、页眉域和修订，均保留。

Word 与 WPS 首次保存时都移除了无包关系的 `vendor/opaque.xml`。另做 JoyWord 单独编辑的对照，有包关系时该部件逐字节保留，无包关系时普通另存也会丢弃。WPS 两轮保存均附带了一个零字节媒体部件，JoyWord 重写只保留实际 PNG。正文与列出的结构完成往返，完整未知部件保真仍开放。[版本、散列和分步读回](word-wps-interop.json)

## 原生文件对话框

JoyWord 和 JoyDeck 分别从 Release App 打开本地副本。原生「打开」面板取消后保留当前文稿，再次调用后用鼠标选取另一份样例，实际窗口标题切换到所选文件。原生「另存为」取消时没有创建目标文件，确认后分别写入新 DOCX 和 PPTX。独立解包读回 DOCX 的千页末段与 PPTX 的三页清单，原测试输入散列一致。

JoyDeck 的重复另存测试中有一次因名称字段保留原扩展名而生成 `native-copy.pptx.joydeck`。本次已取得指定 PPTX 路径的成功读回，原生面板扩展名切换的交互一致性仍待核对。[产物与二进制散列](native-dialogs.json)
