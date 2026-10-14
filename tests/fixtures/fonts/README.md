# 固定拉丁测试字体

joyoffice-test-serif.ttf 是 Noto Serif 的改名、静态、最小拉丁测试子集，SIL Open Font License 1.1；原许可与版权见 [OFL.txt](OFL.txt)，上游提交、输入/输出散列和生成参数见 [provenance.json](provenance.json)。不包含 macOS/Windows 系统字体。

用途仅为 W03 行拟合测试中的 fi 连字、AV 字距和空格。保留相关 GSUB/GPOS，不用于 CJK 文字、应用默认字体或 Office 保真声明。该文件只有约 4 KiB，测试无需网络或额外 Python 库。

Windows CI 原来选 Segoe UI，但该 face 不满足测试的 fi 连字前提，导致测试在进入被测逻辑前失败。现在所有平台默认使用相同 fixture，连字与字距断言保持原样。显式 JOYWORD_LINE_LATIN_FONT 覆盖仍适用于专门实验，但指定字体必须满足这些测试前提。

再生成仅需 fonttools==4.60.1：从 provenance.json 的固定上游 URL 取得原文件，执行下面命令，输出散列应与登记一致。脚本只读取源字体，不覆盖它。

```sh
python3 make_subset.py /path/to/pinned/NotoSerif.ttf
```

处理：固定 wght=400、wdth=100，保留 AVfi 与空格及所需布局 glyph，改名为 JoyOffice Test Serif，保留版权/许可名称记录，不重算构建时间。OFL.txt 保留完整上游文本。正常 Cargo 测试只读取已经提交的 TTF。
