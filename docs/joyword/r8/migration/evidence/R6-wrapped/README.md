# R6 多行输入与 R7 ARM64 开发包

本轮聚焦 Windows 和 Linux。macOS 暂缓。

## 排版路径

长文档内，一个段落完整位于一页，且增量整形后行数、行高、行顶、基线、下降度量和换行类型相同，才可沿用分页并失效对应页面的图像。跨页、插入后改变行数、域和表格等复杂情形仍完整分页。

回归分别覆盖单行编辑、已有自然折行的段落继续编辑，以及改变行数时的全量回退。增量结果与新建缓存后全量计算的页面和索引一致。[内核回归](layout-test.log)

同机 Windows Release，在同一份千页 DOCX 的第 1000 页末段逐字输入 `adghk`，旧包和新包各记录 5 次。

| 应用内耗时（ms） | 旧包 p50 / p95 | 新包 p50 / p95 |
| --- | ---: | ---: |
| 排版 | 64.3 / 70.6 | 6.4 / 7.9 |
| 输入回调至 GPUI 绘制提交 | 121.8 / 154.5 | 36.7 / 43.9 |

p95 在这组小样本中等于最大值。测量范围止于 GPUI 提交，系统合成和屏幕呈现待测。[原始样本](windows-timings.json)

## 三个 Release ZIP

归档位于 `dist/joyword/R7/`，源码指纹均以 `cb149a83b4` 开头。三个包均核验 32 项文件，Windows 和 Linux x64 直接运行搬移后的 CLI，Linux ARM64 通过 QEMU 运行。均为未签名便携包。[归档 SHA-256 和源码一致性](source-parity.json) · [JoyOffice 依赖检查](dependencies.log)

- Windows x64 从中文和空格路径启动。UI Automation 读取第二行选区，原生拼音提交、撤销、重做、另存和新进程重开通过。CLI 和独立 OOXML 读回均确认末段修改，原语料哈希一致。产品检查通过 3415 项测试，2 项忽略。[桌面读回](windows-native.json) · [重开截图](windows-reopen.jpg) · [产品检查](windows-check.log)
- Linux x64 在 Ubuntu 24.04 WSLg 中从解压目录打开千页副本，在窗口中输入字符并保存。WSLg 下无法完整观察部分文件元数据，显式确认替换验收副本后保存通过。CLI 和桌面新进程均读回修改。[原生保存读回](linux-x64-native-reopen.log) · [重开截图](linux-x64-reopen.jpg)
- Linux ARM64 ZIP 通过架构、执行权限和文件校验。解压后通过 QEMU 读取图文样本、渲染 PNG，并编辑、另存和重读千页 DOCX。ARM64 真机桌面待验。[构建与归档校验](linux-packages.log) · [千页读回](linux-arm64-roundtrip.log)

## ARM64 打包入口

Ubuntu x64 构建主机安装 Rust 1.98.1 的 `aarch64-unknown-linux-gnu` 标准库、ARM64 系统开发库及 GCC/G++ 交叉工具链。在 Linux shell 中运行以下命令。

```sh
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
export PKG_CONFIG_LIBDIR_aarch64_unknown_linux_gnu=/usr/lib/aarch64-linux-gnu/pkgconfig:/usr/share/pkgconfig
cargo xtask package joyword --release --target aarch64-unknown-linux-gnu
python3 scripts/verify_preview.py <archive.zip> --smoke --runner qemu-aarch64
```

构建主机和目标架构分别写入 `build-info.json`，打包前验证两件可执行产物的二进制架构。归档校验复用 JoyOffice 适配的架构检查，也验证 Unix 执行位。旧包仍可校验，去除执行位的测试归档会被拒绝。[打包校验回归](packaging-guards.log)
