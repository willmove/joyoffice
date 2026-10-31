# Windows 界面收敛

本次交付 **0.1.0 / M8-preview2**。按 W1 复杂格式 → W2 原生窗口排列 → W3 对象/公式/焦点 → W4 边界补齐推进，每项保留构建与回归证据。按本次范围跳过 macOS/Linux 原生环境及发行签名、公证事项。

| 项目 | 实现与验证 |
| --- | --- |
| W1 | 已编译运行；复杂格式真实表单、旋转中文轮廓、线型/斜线、图案、双下划线和 XLSX 往返通过，见 [W1](evidence/W1.json)、[截图](screenshots/W1-complex-format.png) |
| W2 | 已编译运行；四种实际窗口排列、当前工作簿过滤、草稿保留及共享提交通过，见 [W2](evidence/W2.json) |
| W3 | 已编译运行；生产对象事件/公式提示/焦点/合并编辑/矢量图标回归通过，见 [W3](evidence/W3.json) |
| W4 | 已编译运行；冻结对象绘制/命中裁剪、复选框点击/空格/两次删除、多键排序表单、区域/标签键盘导航、导入形状字体与旋转轴标题回归通过，见 [W4](evidence/W4.json)；原 30 个图标名均映射到内嵌资产并校验资源 |

## 直接试用

解压 `joysheet-0.1.0-M8-preview2-windows-x86_64.zip`，双击 `Open complex formatting.cmd` 或 `Open Windows boundaries.cmd`。同名 `-setup.exe` 为当前用户安装器，包保持未签名。

1. 复杂格式样例中选中单元格，按 Ctrl+1，修改旋转/竖排、边框/斜线、图案、上下标及数字格式；保存、重开检查。正负旋转中文与图表轴文字沿用系统字体。
2. 使用“窗口”新建同工作簿视图，再试平铺/水平/垂直/层叠。仅当前工作簿选项不会移动其他文档；独立编辑草稿保留。窗口过多无法满足最小尺寸时会提示改用层叠或减少参与窗口。
3. Ctrl/Shift 点击多个图表或形状，在“对象”面板对齐/分布；拖动和 Delete 为一次撤销，方向键微调，Shift 加快。形状文字遵循原模型的整对象样式。
4. 输入嵌套公式并移动光标，检查当前参数提示和引用区域。F6/Shift+F6 在网格、公式栏、名称框、工具栏、工作表标签之间切换；标签区方向键切表，Enter 回到网格。
5. 边界样例检查复选框的点击/空格切换、Delete 清值后再次 Delete 移除样式；数据排序表单可填 `B:desc,C:asc` 指定后续关键列。冻结首行首列后滚动，图表/图片/形状不会越过冻结区域或拦截冻结单元格点击。

## 验收入口

```powershell
pwsh -NoProfile -File scripts/joysheet/dev.ps1 run --locked -p joysheet -- --sample sales
pwsh -NoProfile -File scripts/joysheet/dev.ps1 build --release --locked -p joysheet -p joysheet-cli --features joysheet/automation
pwsh -NoProfile -File scripts/joysheet/dev.ps1 xtask ci joysheet
pwsh -NoProfile -File scripts/joysheet/dev.ps1 xtask smoke joysheet --stage M8
pwsh -NoProfile -File scripts/joysheet/desktop-smoke.ps1 -BinaryDirectory dist/joysheet-0.1.0-M8-preview2-windows-x86_64 -Stage M3 -M4Task 4 -M5Task 4 -M6Task 4 -M7Task 4 -M8Task 4 -WindowsTask 4 -Performance
```

单项回归：`-Stage M1 -WindowsTask 1`（W1）、`-Stage M1 -WindowsTask 2`（W1/W2）、`-Stage M1 -WindowsTask 3 -WindowsFrom 3`（W3）、`-Stage M1 -WindowsTask 4 -WindowsFrom 4`（W4）。运行实际 Windows GPUI 进程，输入由测试接口合成派发，截图为 GPU 读回。这不替代真实 IME、系统读屏、多显示器 DPI、外部剪贴板和实际打印队列验收。

## 实现边界

复杂文字使用后台系统字体数据库和有界轮廓缓存（最多 128 块/16 MiB）；普通文字仍使用 GPUI 字形缓存。超长单元格绘制有 8,192 字符/256 行预算，形状文字有 32,767 字符/256 行预算；原始工作簿内容不截断。图案和虚线先裁剪后展开，避免远端坐标产生无界循环。缺少字体时需在目标机安装相应字体；不随包分发系统字体。

窗口排列使用安全的 Win32 包装 API，只操作当前进程登记的窗口，基于活动显示器工作区；布局负坐标有单测，真实跨显示器 DPI 仍需人工。批量对象操作复用原命令与 Session.grouped，校验工作表/修订，任一失败回滚；持久组合与独立拆分滚动不属于原模型已实现能力。单元格分段富文本未在原核心模型实现，保持上游读取语义。

## Release 大表采样

同一 Windows 基准机、Release automation 构建、1440×900 逻辑窗口，确定性 CSV 各滚动 60 次，再编辑低依赖公式 30 次。报告见 [Windows-performance.json](evidence/Windows-performance.json)。

| 数据规模 | 引擎打开 | 打开到视图 | CPU 绘制 P95 | 提交到 CPU 重绘 P95 | 进程工作集 | 可见单元格 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 10,000 × 20 | 35.62 ms | 778.85 ms | 4.15 ms | 42.06 ms | 440.0 MiB | 704 |
| 100,000 × 20 | 362.72 ms | 796.03 ms | 7.82 ms | 44.61 ms | 630.0 MiB | 704 |

打开到视图包含控制通信/轮询，工作集为采样时整个进程而非单表净内存/峰值。CPU 绘制和编辑提交数据不包含 GPUI prepaint 全部开销、GPU 呈现、系统合成器或真实设备输入延迟，不能代替 60 FPS 门槛认证。几何专项 1,004 次远端视口布局 P95 为 0.0015 ms、最多 1,100 单元格；100 万行稀疏表另有生产视图定位/点击回归。

关闭/重开时先保持引擎的窗口激活意图，待对应原生窗口确认后才接受其他 HWND 的激活通知；队列中的旧激活还需核对先前活动窗口，避免覆盖更晚的新文档选择。W4 加入六次连续关闭/重开与后续单元格读取回归，应用单测覆盖过期通知拒绝及有效用户切换。


恢复副本提示仅在应用首次启动窗口自动显示；后续新建/打开的文档默认显示完整工作区，可从设置主动进入恢复列表。回归同时检查启动发现副本时提示仍在，以及后续文档不被侧栏遮挡。

## 最终交付验证

完整 CI **1,369 项通过、0 失败、1 项上游计时测试忽略**，见 [Windows-ci](evidence/Windows-ci.json)。最终解压 Release 通过 **47 项** M3–M8/W1–W4 串联回归，见 [Windows-release](evidence/Windows-release.json)，其中含真实进程终止/重启恢复、六次关闭/重开、窗口排列及保存弹窗锁文件失败/重试。生产 GPUI 合成输入、系统 API 窗口矩形与 GPU 读回分别记录，不当作物理 IME/读屏/打印队列验收。

普通桌面构建、WASM 核心、引擎文件往返及稀疏几何基准通过。源码 ZIP 解压后锁定/离线 metadata 识别 18 个工作区成员，见 [Windows-source](evidence/Windows-source.json)。最终文档刷新打包保持生产二进制哈希与完整回归报告一致，源码检查点逐文件 SHA256 随包保留。

用户级安装器隔离安装后运行同一真实程序、执行打开保存检查、卸载并保留额外文件通过，见 [Windows-installer](evidence/Windows-installer.json)、[安装后程序](evidence/Windows-installed-runtime.json)。TEST 模式不写开始菜单/注册表，因此这两项不列为实际通过。包内附带 app-local CRT 和 508 项依赖许可文本清单；Windows PDF 预览使用内置系统渲染，系统打印对话框另需 PowerShell 7。

本轮不宣称 M8 总验收完成：macOS/Linux 环境与发行签名按范围跳过；真实 IME、读屏、多显示器 DPI、外部应用剪贴板互操作、实际打印队列及 GPU 呈现延迟仍保留人工/设备验收。

后续输入法、DPI、辅助功能和打印增量已交付 M8-preview3，自动覆盖与具体人工步骤见 [WINDOWS-QUALITY.md](WINDOWS-QUALITY.md)。
