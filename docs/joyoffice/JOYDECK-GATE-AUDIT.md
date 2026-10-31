# JoyDeck 迁移门禁审阅

2026-10-31。原始导入预扫描为407个panic-family命中，其中148个在内联单测；这些单测已迁入真实测试文件。普通GUI输入、图像帧、表格选择/调整大小、放映互斥锁、CLI及产品库错误路径已改为可恢复错误或明确停止操作。本记录不将静态命中称为运行时故障。

## 架构与扫描范围

- 单向依赖表覆盖GUI、CLI、model/engine/formats/scene/export/platform/shell和工程工具。TOML解析使用真实package名称，继承/重命名/target/build/dev声明不能绕过产品边界。
- 离线发现workspace显式成员、glob成员、内树path依赖自动成员及tools；禁止越出仓库的成员/path依赖。未知成员拒绝通过。
- 核心、CLI、公共库和工具禁止直接依赖GPUI包；公共库不能反向依赖产品；不同产品不能互相依赖。
- 验证feature不改变源码扫描范围；新工具、额外panic、纯模型中的IO都有拒绝反例。

## IO逐文件职责

以下仅列此次新增的精确源文件。没有目录级或feature级豁免；普通模型/引擎/格式/场景/输出内容crate不在IO白名单内。编解码只接收/返回内存数据；落盘、进程和原生适配留在平台/服务层。

| 文件 | 审阅职责 |
| --- | --- |
| `apps/joydeck-cli/src/main.rs` | read the explicit local control credential descriptor |
| `tools/xtask/src/main.rs` | invoke the product-scoped engineering runner, never product document commands |
| `crates/joydeck-platform/build.rs` | compile packaged Swift helpers on desktop macOS builds |
| `crates/joydeck-platform/src/files.rs` | bounded reads, private temp directories and atomic publication |
| `crates/joydeck-platform/src/playback.rs` | owned native media process and private bounded frame files |
| `crates/joydeck-shell/src/control.rs` | create/read/remove a private local-control descriptor |
| `crates/joydeck-shell/src/export.rs` | stage and publish requested output files/directories |
| `crates/joydeck-shell/src/media.rs` | bounded user-selected image import |
| `crates/joydeck-shell/src/output.rs` | invoke the packaged print helper and clean its temporary input |
| `crates/joydeck-shell/src/preferences.rs` | read/validate/atomically replace product preferences |
| `crates/joydeck-shell/src/storage.rs` | document save, recovery, locks and source fingerprint checks |
| `crates/joydeck-shell/src/video.rs` | owned encoder helper with private staging and cancellation |
| `apps/joydeck/src/authoring_smoke.rs` | validation feature: independent authoring output/reference capture |
| `apps/joydeck/src/beta_smoke.rs` | validation feature: native preference/compare/video acceptance record |
| `apps/joydeck/src/delivery_smoke.rs` | validation feature: export fixtures and output manifest |
| `apps/joydeck/src/editor_validation.rs` | validation feature: recovery/IME temporary roundtrip cleanup |
| `apps/joydeck/src/extended_smoke.rs` | validation feature: media and print cancellation evidence |
| `apps/joydeck/src/media_smoke.rs` | validation feature: clipboard/image reference capture |
| `apps/joydeck/src/render_bench.rs` | validation feature: synthetic benchmark report and raster output |
| `apps/joydeck/src/slideshow_validation.rs` | validation feature: native show acceptance report |
| `apps/joydeck/src/table_smoke.rs` | validation feature: table roundtrip and raster reference capture |
| `apps/joydeck/src/workspace_smoke.rs` | validation feature: multiwindow design and clipboard evidence |

## 显式验证程序的panic遗留清单

本轮只新增下列116个源码行级记录。一个记录可包含同一行的多个断言调用。原Word baseline保持原样。每个记录绑定完整文件路径、去除两端空白后的源码文本及重复次数；行号仅为定位。没有为普通产品路径添加panic记录。

GUI验证模块只在`validation` feature下编译，发布构建不启用该feature；两个examples只在显式运行对应QA生成器时执行。验证器使用原创固定文稿和独立输出，窗口/状态/图像/报告不变量被破坏时必须失败，不能以默认值假称通过。这里的允许不证明功能验收通过；实际结果另存迁移验证记录。

这些是逐项审阅后的明确遗留项，不是对整个产品或目录自动生成baseline。未来新增/修改源码行仍由multiset门禁拒绝。代码迁移到普通路径时必须重新审阅，不能复制这些记录。

| 源码位置 | 审阅的精确源码行 | 保留原因 |
| --- | --- | --- |
| `apps/joydeck/src/editor_validation.rs:92` | `assert!(s.gesture.as_ref().unwrap().mode == Some(Mode::Scale));` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:105` | `assert!(s.gesture.as_ref().unwrap().mode == Some(Mode::Rotate));` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:202` | `let readonly = cx.update(\|cx\| crate::viewer::open_window(cx).unwrap().0);` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:216` | `.unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:222` | `.expect("own recovery snapshot");` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:234` | `open(cx).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:235` | `cx.global::<Registry>().0.last().unwrap().clone()` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:265` | `let path = close_path.take().unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:274` | `assert_eq!(result.unwrap(), 1);` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:277` | `open(cx).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:278` | `cx.global::<Registry>().0.last().unwrap().clone()` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:292` | `let service = discarded.as_ref().unwrap().clone();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:298` | `.unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:301` | `cx.update(\|cx\| cx.global::<Registry>().0.last().unwrap().clone());` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:312` | `let service = discarded.take().unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:318` | `.unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:357` | `s.text_edit.as_mut().unwrap().cancel();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:358` | `assert_eq!(s.text_edit.as_ref().unwrap().object.text,committed.slides[0].objects[0].text);` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:365` | `let t=s.text_edit.as_mut().unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:368` | `joydeck::text::format(&mut t.object,0..6,t.typing.clone()).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:371` | `edit::save_new(&path,&s.session.deck).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:372` | `assert_eq!(edit::open_editable(&path).unwrap(),*s.session.deck);` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:373` | `std::fs::remove_file(path).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:375` | `})).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:383` | `})).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:389` | `})).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:395` | `})).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:405` | `let mut o = if table{joydeck::table::object(4,5).unwrap()}else{edit::object("text")}; o.text.clear(); if !table{o.width=480.;}` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:410` | `})).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:441` | `edit::save_new(&path,&s.session.deck).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:442` | `assert_eq!(edit::open_editable(&path).unwrap(),*s.session.deck);` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/editor_validation.rs:450` | `})).unwrap();` | 固定事件阶段中的编辑/恢复/IME验收断言；独立测试文稿与显式证据输出，失败即终止该验收，不报告成功 |
| `apps/joydeck/src/viewer_validation.rs:37` | `assert_eq!(s.deck.as_ref().unwrap().slides.len(), 20);` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/viewer_validation.rs:71` | `&& s.image.as_ref().unwrap().0.2 == 2048 =>` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/viewer_validation.rs:81` | `&& s.image.as_ref().unwrap().0.2 < 2048 =>` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/viewer_validation.rs:90` | `let old = s.deck.clone().unwrap();` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/viewer_validation.rs:93` | `assert!(Arc::ptr_eq(&old, s.deck.as_ref().unwrap()));` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/viewer_validation.rs:106` | `let d = s.deck.as_ref().unwrap();` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/viewer_validation.rs:117` | `.unwrap()` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/viewer_validation.rs:131` | `assert_eq!(s.deck.as_ref().unwrap().slides.len(), 20);` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/viewer_validation.rs:135` | `.unwrap()` | 20页原创固定样本；ready和阶段守卫后的文稿/图像/来源检查，缺失即验收失败 |
| `apps/joydeck/src/authoring_smoke.rs:14` | `if s.session.deck.slides[0].objects[1].chart.as_ref().unwrap().series[0].values[3]!=Some(131.){return Err("chart grid failed".into());}s.history(false,w,cx);if s.session.deck!=base{return Err("chart grid undo failed".into());}s.history(true,w,cx);` | 原创作者样本的图表/公开部件/参考像素及已打开窗口断言 |
| `apps/joydeck/src/authoring_smoke.rs:19` | `let mut parts=joydeck::pptx_write::generate(&s.session.deck)?;joydeck::export::save_parts(&output.join("authoring.pptx"),&parts,&AtomicBool::new(false))?;parts.get_mut("ppt/slides/slide1.xml").unwrap().extend_from_slice(b"<!-- independent public import -->");joydeck::export::save_parts(&output.join("authoring-public.pptx"),&parts,&AtomicBool::new(false))?;` | 原创作者样本的图表/公开部件/参考像素及已打开窗口断言 |
| `apps/joydeck/src/authoring_smoke.rs:24` | `if (phase==1\|\|phase==3)&&since.elapsed()>Duration::from_secs(2){let ready=handle.update(cx,\|_,w,cx\|view.update(cx,\|s,_\|->std::result::Result<bool,String>{let backend=if phase==1{"GPUI"}else{"CPU"};if (phase==1&&s.scene.is_none())\|\|(phase==3&&s.image.is_none()){return Ok(false);}let scale=s.canvas_scale(w);let width=s.session.deck.width*scale;let height=s.session.deck.height*scale;let r=preview::render(&s.session.deck,0,(width*w.scale_factor()).round()as u32,&AtomicBool::new(false))?;let mut rgba=r.bgra;for p in rgba.as_chunks_mut::<4>().0{p.swap(0,2);}image::RgbaImage::from_raw(r.width,r.height,rgba).unwrap().save(output.join(format!("M9-{backend}-reference.png"))).map_err(\|e\|e.to_string())?;eprintln!("M9_CAPTURE {backend} {}",serde_json::json!({"x":s.canvas_origin.0,"y":s.canvas_origin.1,"width":width,"height":height,"viewportHeight":f32::from(w.viewport_size().height)}));Ok(true)})).map_err(\|e\|e.to_string())??;if ready{phase+=1;}continue;}` | 原创作者样本的图表/公开部件/参考像素及已打开窗口断言 |
| `apps/joydeck/src/authoring_smoke.rs:30` | `match result{Ok(d)=>{eprintln!("M9_SMOKE grid_clipboard_undo_native_public_pdf=true complete=true");let focus=handle.update(cx,\|_,w,cx\|view.update(cx,\|s,cx\|{s.prepare_show(w,cx);s.focus.clone()})).unwrap();cx.update(\|cx\|crate::slideshow::smoke(d,output,handle,focus,cx));},Err(e)=>{eprintln!("M9_SMOKE failed={e}");cx.update(\|cx\|cx.quit());}}` | 原创作者样本的图表/公开部件/参考像素及已打开窗口断言 |
| `apps/joydeck/src/beta_smoke.rs:4` | `let out=PathBuf::from(std::env::var_os("JOYDECK_M14_OUTPUT").unwrap());` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/beta_smoke.rs:36` | `image::RgbaImage::from_raw(raster.width,raster.height,rgba).unwrap().save(out.join("M14-compare-reference.png")).map_err(\|e\|e.to_string())?;` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/beta_smoke.rs:55` | `let (report,_)=s.comparison.as_ref().unwrap();` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/beta_smoke.rs:56` | `s.comparison_pick=report.differences.iter().position(\|r\|r.kind=="object.changed").unwrap();` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/beta_smoke.rs:102` | `open_session(cx,None).unwrap();let (h,weak)=cx.global::<Registry>().0.last().unwrap().clone();` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/beta_smoke.rs:103` | `let fresh=weak.upgrade().unwrap();assert_eq!(fresh.read(cx).zoom,0.9);assert!(fresh.read(cx).show_notes);` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/beta_smoke.rs:104` | `h.update(cx,\|_,w,cx\|{let expected=fresh.read(cx).preferences.height;assert_eq!(f32::from(w.window_bounds().get_bounds().size.height).round() as u32,expected);}).unwrap();` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/beta_smoke.rs:105` | `h.update(cx,\|_,w,_\|w.remove_window()).unwrap();` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/beta_smoke.rs:123` | `std::fs::write(out.join("native-state.json"),serde_json::to_vec_pretty(&report).unwrap()).map_err(\|e\|e.to_string())?;` | 外部验证器提供输出目录；固定比较样本、窗口重开、偏好恢复和Value报告断言 |
| `apps/joydeck/src/extended_smoke.rs:6` | `let out = PathBuf::from(std::env::var_os("JOYDECK_M12_OUTPUT").unwrap());` | 外部验证器提供输出目录；独立媒体样本状态、帧尺寸、打印取消和PDF参考断言 |
| `apps/joydeck/src/extended_smoke.rs:46` | `let status = s.av_player.as_ref().ok_or("no player")?.status.lock().unwrap().clone();` | 外部验证器提供输出目录；独立媒体样本状态、帧尺寸、打印取消和PDF参考断言 |
| `apps/joydeck/src/extended_smoke.rs:51` | `image::RgbaImage::from_raw(frame.width, frame.height, rgba).unwrap()` | 外部验证器提供输出目录；独立媒体样本状态、帧尺寸、打印取消和PDF参考断言 |
| `apps/joydeck/src/extended_smoke.rs:112` | `.unwrap()` | 外部验证器提供输出目录；独立媒体样本状态、帧尺寸、打印取消和PDF参考断言 |
| `apps/joydeck/src/extended_smoke.rs:153` | `})).unwrap()).map_err(\|e\|e.to_string())` | 外部验证器提供输出目录；独立媒体样本状态、帧尺寸、打印取消和PDF参考断言 |
| `apps/joydeck/src/extended_smoke.rs:198` | `.unwrap()` | 外部验证器提供输出目录；独立媒体样本状态、帧尺寸、打印取消和PDF参考断言 |
| `apps/joydeck/src/media_smoke.rs:30` | `let image=s.session.deck.slides[0].objects[0].image.as_ref().unwrap().resource.clone();` | 已粘贴原创图片的资源引用与已验证尺寸的参考像素断言 |
| `apps/joydeck/src/media_smoke.rs:52` | `image::RgbaImage::from_raw(raster.width,raster.height,rgba).unwrap().save(output.join(format!("M5-{backend}-reference.png"))).map_err(\|e\|e.to_string())?;` | 已粘贴原创图片的资源引用与已验证尺寸的参考像素断言 |
| `apps/joydeck/src/render_bench.rs:75` | `let r = result.expect("benchmark CPU reference");` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:84` | `image::RgbaImage::from_raw(r.width, r.height, r.bgra).unwrap(),` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:103` | `.expect("benchmark CPU upload");` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:109` | `.expect("benchmark GPU draw");` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:153` | `let bounds = self.bounds.unwrap();` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:167` | `let r=scene.raster(raster_width,&AtomicBool::new(false)).unwrap();let mut rgba=r.bgra;` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:169` | `resvg::tiny_skia::Pixmap::from_vec(rgba,resvg::tiny_skia::IntSize::from_wh(r.width,r.height).unwrap()).unwrap().save_png(path).unwrap();` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:191` | `std::fs::write(s.output.join("native-benchmark.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:244` | `std::fs::create_dir_all(&output).expect("benchmark output directory");` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:250` | `let mut c = cache.lock().unwrap();` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:251` | `let edit = bench::edit(&mut c).unwrap();` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:252` | `let effect = bench::effect().unwrap();` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/render_bench.rs:294` | `.unwrap();` | 显式基准程序：固定场景生成、CPU/GPU绘制、尺寸转换、输出目录和报告失败必须中止测量 |
| `apps/joydeck/src/workspace_smoke.rs:28` | `let new=cx.update(\|cx\|->std::result::Result<_,String>{open_session(cx,Some(session)).map_err(\|e\|e.to_string())?;let (h,weak)=cx.global::<Registry>().0.last().unwrap().clone();Ok((h,weak.upgrade().ok_or("peer closed")?))})?;` | 固定双窗口/主题阶段中的peer、theme-1和参考画面断言 |
| `apps/joydeck/src/workspace_smoke.rs:33` | `let (peer_handle,peer_view)=peer.as_ref().unwrap();` | 固定双窗口/主题阶段中的peer、theme-1和参考画面断言 |
| `apps/joydeck/src/workspace_smoke.rs:35` | `let mut d=s.session.deck.design.clone();d.themes.get_mut("theme-1").unwrap().colors.insert("accent".into(),"#1265c5".into());s.command(Command::Design(Action::SetDesign(d)),cx);` | 固定双窗口/主题阶段中的peer、theme-1和参考画面断言 |
| `apps/joydeck/src/workspace_smoke.rs:45` | `let (h,p)=peer.as_ref().unwrap();h.update(cx,\|_,_,cx\|p.update(cx,\|s,cx\|->std::result::Result<(),String>{s.sync_document(cx);if s.selected!=Some(1){return Err("peer selection lost".into());}s.session=Session::new();s.renew_recovery();s.command(Command::Design(Action::PastePages(1,transfer,None)),cx);if s.session.deck.slides.len()!=2{return Err("cross document paste failed".into());}Ok(())})).map_err(\|e\|e.to_string())??;` | 固定双窗口/主题阶段中的peer、theme-1和参考画面断言 |
| `apps/joydeck/src/workspace_smoke.rs:56` | `image::RgbaImage::from_raw(raster.width,raster.height,rgba).unwrap().save(output.join(format!("M6-{backend}-reference.png"))).map_err(\|e\|e.to_string())?;` | 固定双窗口/主题阶段中的peer、theme-1和参考画面断言 |
| `apps/joydeck/src/table_smoke.rs:15` | `let id=s.table_pick.as_ref().unwrap().id.clone();let base=s.session.deck.clone();` | 固定4×5原创表格样本的焦点/预编辑/Tab/参考像素断言 |
| `apps/joydeck/src/table_smoke.rs:18` | `s.begin_cell(2,1,0,w,cx);s.replace_and_mark_text_in_range(None,"取消",None,w,cx);if s.session.deck!=base{return Err("preedit published".into());}s.text_edit.as_mut().unwrap().cancel();s.finish_text(cx);` | 固定4×5原创表格样本的焦点/预编辑/Tab/参考像素断言 |
| `apps/joydeck/src/table_smoke.rs:19` | `let event=KeyDownEvent{keystroke:Keystroke{key:"tab".into(),..Default::default()},is_held:false,prefer_character_input:false};if !s.table_key(&event,w,cx)\|\|s.table_pick.as_ref().unwrap().focus!=(1,1){return Err("Tab navigation failed".into());}s.finish_text(cx);` | 固定4×5原创表格样本的焦点/预编辑/Tab/参考像素断言 |
| `apps/joydeck/src/table_smoke.rs:20` | `let mut reverse=event;reverse.keystroke.modifiers.shift=true;s.table_key(&reverse,w,cx);if s.table_pick.as_ref().unwrap().focus!=(1,0){return Err("reverse Tab failed".into());}s.finish_text(cx);` | 固定4×5原创表格样本的焦点/预编辑/Tab/参考像素断言 |
| `apps/joydeck/src/table_smoke.rs:37` | `let scale=s.canvas_scale(w);let width=s.session.deck.width*scale;let height=s.session.deck.height*scale;let raster=preview::render(&s.session.deck,0,(width*w.scale_factor()).round() as u32,&AtomicBool::new(false))?;let mut rgba=raster.bgra;for p in rgba.as_chunks_mut::<4>().0{p.swap(0,2);}image::RgbaImage::from_raw(raster.width,raster.height,rgba).unwrap().save(output.join(format!("M8-{backend}-reference.png"))).map_err(\|e\|e.to_string())?;` | 固定4×5原创表格样本的焦点/预编辑/Tab/参考像素断言 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:12` | `.expect("output folder");` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:13` | `std::fs::create_dir_all(&out).unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:15` | `edit::save(&out.join("authoring.joydeck"), &d).unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:16` | `let parts = pptx_write::generate(&d).unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:17` | `export::save_parts(&out.join("authoring.pptx"), &parts, &AtomicBool::new(false)).unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:21` | `.unwrap()` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:28` | `.unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:29` | `let r = preview::render(&d, 0, 1600, &AtomicBool::new(false)).unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:35` | `.unwrap()` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:37` | `.unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:71` | `bench.validate().unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:72` | `let mut s = show::ShowSnapshot::new(Arc::new(bench), 0).unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:82` | `let frame = c.frame(&s).unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:83` | `preview::render(&frame, s.page(), 1024, &AtomicBool::new(false)).unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:92` | `serde_json::to_vec_pretty(&report).unwrap(),` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/authoring-fixture.rs:94` | `.unwrap();` | 只能显式运行的QA样本/基准生成器：必需输出参数、原创样本、部件、渲染和报告失败即非零退出 |
| `crates/joydeck-shell/examples/beta-qualification.rs:17` | `format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))` | 只能显式运行的QA资格化：原创语料分支构造、已生成样本hash/报告、必需输出父目录断言 |
| `crates/joydeck-shell/examples/beta-qualification.rs:20` | `std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();` | 只能显式运行的QA资格化：原创语料分支构造、已生成样本hash/报告、必需输出父目录断言 |
| `crates/joydeck-shell/examples/beta-qualification.rs:60` | `for (i, c) in o.table.as_mut().unwrap().cells.iter_mut().enumerate() {` | 只能显式运行的QA资格化：原创语料分支构造、已生成样本hash/报告、必需输出父目录断言 |
| `crates/joydeck-shell/examples/beta-qualification.rs:66` | `d.slides[0].objects[1].chart.as_mut().unwrap().series[0].values = vec![` | 只能显式运行的QA资格化：原创语料分支构造、已生成样本hash/报告、必需输出父目录断言 |
| `crates/joydeck-shell/examples/beta-qualification.rs:117` | `let t = d.presentation.slides.get_mut("page-0").unwrap();` | 只能显式运行的QA资格化：原创语料分支构造、已生成样本hash/报告、必需输出父目录断言 |
| `crates/joydeck-shell/examples/beta-qualification.rs:146` | `.unwrap()` | 只能显式运行的QA资格化：原创语料分支构造、已生成样本hash/报告、必需输出父目录断言 |
| `crates/joydeck-shell/examples/beta-qualification.rs:158` | `.unwrap()` | 只能显式运行的QA资格化：原创语料分支构造、已生成样本hash/报告、必需输出父目录断言 |
| `crates/joydeck-shell/examples/beta-qualification.rs:266` | `std::fs::create_dir_all(out.parent().unwrap()).map_err(\|e\| e.to_string())?;` | 只能显式运行的QA资格化：原创语料分支构造、已生成样本hash/报告、必需输出父目录断言 |
| `apps/joydeck/src/scene_canvas_validation.rs:25` | `assert!(s.accelerated);assert!(s.scene.as_ref().unwrap().coverage().2>0);` | 显式画布验收：固定场景、缓存不变量、同快照身份与清空场景断言 |
| `apps/joydeck/src/scene_canvas_validation.rs:26` | `before=s.gpu_cache.stats.clone();builds=s.scene_cache.lock().unwrap().stats.builds;` | 显式画布验收：固定场景、缓存不变量、同快照身份与清空场景断言 |
| `apps/joydeck/src/scene_canvas_validation.rs:40` | `assert_eq!(s.scene_cache.lock().unwrap().stats.builds,builds);` | 显式画布验收：固定场景、缓存不变量、同快照身份与清空场景断言 |
| `apps/joydeck/src/scene_canvas_validation.rs:47` | `assert_eq!(s.scene_cache.lock().unwrap().stats.builds,builds);` | 显式画布验收：固定场景、缓存不变量、同快照身份与清空场景断言 |
| `apps/joydeck/src/scene_canvas_validation.rs:54` | `assert!(Arc::ptr_eq(moved.as_ref().unwrap(),&s.session.deck));` | 显式画布验收：固定场景、缓存不变量、同快照身份与清空场景断言 |
| `apps/joydeck/src/scene_canvas_validation.rs:59` | `assert!(s.accelerated);assert!(s.gpu_cache.stats.atlas_removals>before.atlas_removals);assert!(Arc::ptr_eq(moved.as_ref().unwrap(),&s.session.deck));` | 显式画布验收：固定场景、缓存不变量、同快照身份与清空场景断言 |
| `apps/joydeck/src/scene_canvas_validation.rs:69` | `assert!(s.accelerated);assert!(s.scene.as_ref().unwrap().nodes.is_empty());` | 显式画布验收：固定场景、缓存不变量、同快照身份与清空场景断言 |

## 审阅时文件校验值

| 文件 | SHA-256 |
| --- | --- |
| `apps/joydeck/src/editor_validation.rs` | `f0f49a56d5a215e080aa30c528d99c3a463ac1227e019f10ab980210e1b6aa3d` |
| `apps/joydeck/src/viewer_validation.rs` | `158c4c4df88c7637135f971c8c01c5160945ad2605496d406c27cded0bdf11ac` |
| `apps/joydeck/src/authoring_smoke.rs` | `a4a4c25e82d1cf454eab2f0c2a8fff5b8757db9a4520dbe772e62fb8859ce6a9` |
| `apps/joydeck/src/beta_smoke.rs` | `b9f029923308e1caa46873d51e8cd34db320eca68eab1124f0e72ab1d78f78d5` |
| `apps/joydeck/src/extended_smoke.rs` | `8338c565b04e4f1042d2cde685be36341596716478f23d9b1941374082c55b28` |
| `apps/joydeck/src/media_smoke.rs` | `ff0ac729883dfe9efc8b24a55ef26649b35c7c95e305403d2938faf19ec9a569` |
| `apps/joydeck/src/render_bench.rs` | `e633e169d1edcff6518e84a8bb9224e03f67f380824c994333547b27d7430e4c` |
| `apps/joydeck/src/workspace_smoke.rs` | `43aeb7f078d28be8fc7e39b2aab79fbff4dac296aa8dfcf3912f490a884fd9f7` |
| `apps/joydeck/src/table_smoke.rs` | `fc3d7e570df331037d4678e5525a25fe441d0c3aa5b4324709df34dfa24fd8ec` |
| `crates/joydeck-shell/examples/authoring-fixture.rs` | `6df0e0c17dbc7142da0dbe55064a5b851b2319bdd4b83c725551ae98eb42a952` |
| `crates/joydeck-shell/examples/beta-qualification.rs` | `90b34b24777e538b733da0089433142789c7625342739c64f3d6d4d9b3cd6344` |
| `apps/joydeck/src/scene_canvas_validation.rs` | `51cc395df8fe2926d7a1d74eaed47444a9cd9fd37c1024485b3efd8faa2db5a4` |

## 回归要求

- 运行统一check_gates、门禁反例和Python工具测试，确保既有Word检查未放宽。
- 新增GUI安全测试覆盖无效像素长度、失效行列调整不改变原对象、合法调整只产生预览，以及poisoned放映状态拒绝继续且保留原文稿快照。
- 验证正常与validation构建，既有原生画布/输入/放映回归另行执行；CI不替代设备验收。
