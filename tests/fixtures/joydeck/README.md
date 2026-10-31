# 自建读取样例

由 `python scripts/make-fixtures.py` 确定性生成；作者为本项目，不含外部模板、字体、图片或第三方产品实现数据。

- native-20.joydeck：20 页、中文/英文、换行文字、矩形、椭圆。
- public-format-20.pptx：20 页公开格式合成文稿；非固定 presentation 路径，sldIdLst 顺序为 20→1，第二页含未知对象。测试 OPC 关系和预览子集，不作为 Office 互操作已通过的证据。
- broken.pptx：截断 ZIP；错误时保留当前文稿。

- m8-powerpoint-edited.pptx：本项目原创4×5业务表格，经PowerPoint16.114.1修改单元格130→131并另存；用于公开DrawingML读取、2×2合并、边框与native再存回归。不含外部演示文稿内容。来源与哈希见 docs/evidence/m8/gui/powerpoint-verification.json。

M11媒体样例均为本项目原创：`media/original-h264.mp4` 是Swift生成的1920×1080、30fps、5秒H.264 Main 8-bit动态色块；`original-tone.wav` 是5秒440Hz PCM单声道；`original-ten-minutes.wav` 是600秒6kHz PCM，用于实际时钟运行；`original-tone.mp3` 是原tone的96kbps MP3；`original-h264-aac.mp4` 将原创H.264与由系统afconvert编码的AAC合并。中间m4a为生成物，不在应用中开放独立m4a导入。生成源码在apps/joydeck/native，MP3仅用QA工具lameenc 1.9.0生成，不打包该编码库。无外部视频、音乐、字体或录音。
