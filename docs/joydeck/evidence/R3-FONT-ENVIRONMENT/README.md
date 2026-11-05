# R3 字体环境复验

2026-11-05，Apple M4／macOS 27.2，1× 桌面。使用仓库外解压的 ARM64 Release 验收包 `joydeck-0.8.0-r3-fonts-final.zip`，校验 1,805 项包内文件。归档 SHA-256 为 `ebbf25a3c9293c4791a9a603f8d805400d40e9ac709557074bdd82bcd5d1d17b`。样例、截图与可执行文件指纹见 [验证数据](verification.json)。

## 字体身份与刷新

- 身份由完整文件 SHA-256、集合 face 索引与有效可变轴组成。轴以标签排序，默认值可省略，超界坐标按字体声明的范围处理。回归使用两个 face 的 TTC，验证同字节不同索引、等效轴次序、不同轴实例及实际字形 advance。
- 同家族、同样式、不同身份的字体可并存。匹配优先使用随包或显式加入的 face，系统文件之间按路径稳定选取。兼容报告列出选中指纹和冲突候选，缺失家族报告实际替代 face。相同文件字节、索引和轴视为同一身份。
- 桌面后台轮询已发现的字体目录，识别 TTF、OTF、TTC、OTC 文件增删改。字体变化使已载入的系统 face、缺字查询和字形轮廓缓存换代。现有窗口刷新场景、CPU 画布、缩略图、原位几何、放映帧与字体选择器。迟到的旧环境结果在安装前拒绝，导出期间变化则要求重试。
- 找不到覆盖可见字符的 face 时保留原 Unicode 并绘制 .notdef。字体缺少轮廓时绘制占位框。空格与控制字符保留其排版语义，缺少映射时保持不可见。原生轮廓场景诊断记录缺字的对象、段落、范围与 face 指纹。
- 字体结构和坐标语义依据 [OpenType 字体文件](https://learn.microsoft.com/en-us/typography/opentype/spec/otff)、[fvar](https://learn.microsoft.com/en-us/typography/opentype/spec/fvar) 和 [hmtx](https://learn.microsoft.com/en-us/typography/opentype/spec/hmtx)。合成测试字体为原创 CC0-1.0，见 [生成脚本](../../../../crates/joydeck-fonts/tests/fixtures/make_environment_fonts.py)。

## macOS 实屏

把合成字体放入验收进程隔离的用户字体目录，打开 [测试 PPTX](live-font.pptx)。两个同名字体均为 864 字节，字形 advance 分别为 620 与 900 个设计单位。测试开始时两个文件同时存在，包内 CLI 报告窄体指纹与宽体冲突候选。运行中对同一路径进行原子替换。

| 路径 | 实测结果 |
| --- | --- |
| GPUI 画布与缩略图 | 同一窗口从一行变两行，主画布蓝色像素高度从 29 变为 81，缩略图从 4 变为 15。窄体原行仅绘制 8 个可见字形，空格保持空白。字体替换期间引擎修订与 dirty 状态保持原值。[窄体](01-gpui-narrow.png) · [宽体](02-gpui-wide.png) |
| CPU 预览 | 同一窗口切到 CPU 后从宽体两行恢复为窄体一行。[宽体](03-cpu-wide.png) · [窄体](04-cpu-narrow.png) |
| 原位编辑 | 保持原位选择时替换字体，选区和文字重排。一次键盘替换提交一个文稿修订，读回结果为单个 A。[窄体选区](05-inline-narrow.png) · [宽体选区](06-inline-wide.png) · [编辑结果](07-inline-edit.png) |
| 字体选择器 | 新增 TTC 后搜索到 1 项，移除后为 0 项。实屏与 Accessibility 树一致。[新增](08-picker-installed.png) · [移除](09-picker-removed.png) |
| 包内 CLI | 两个同名字体移除后独立读取原 PPTX，报告实际替代为 Calibri Regular，并给出 face 指纹。测试前后原 PPTX 散列一致。 |

## 回归范围

JoyDeck 产品检查 909 通过。三产品最终联合测试 5,802 通过、0 失败、3 忽略。预跑曾出现 JoyWord 两条增量布局断言失败，最终完整复跑通过。移位包通过 R1/R2/R3 的 28／20／20 项检查与 329 条命令注册表核对。Poppler 复验了混排、动态字段和链接，组合与普通文本框的 14 个搜索词边界最大差为 0 pt。

本次使用隔离目录验证文件字体增删改。Windows/Linux 实屏、2× 屏幕、Font Book 内仅切换启用状态及原生字形后端资格分别留待平台与呈现验收。
