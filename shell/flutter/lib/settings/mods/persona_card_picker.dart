/// 角色卡文件选择的**平台分派**（Web 走浏览器文件对话框，其它目标返回不可用）。
///
/// # 为什么要条件导入
///
/// 浏览器文件选择需要 `package:web`（`app/browser_io.dart`）。而
/// `settings/mods/*_panel.dart` 会被 `sections/dev_tools_section.dart` 拉进
/// `flutter test`（Dart VM）的编译面——**直接** import `package:web` 会让所有碰
/// Mod 管理区的测试在编译期变红（persona 轨实测）。
///
/// 条件导入把「Web 实现」与「非 Web 兜底」分开：VM 测试拿到 stub（零 `package:web`），
/// Web 构建拿到浏览器实现。面板本身只认 [PersonaCardFilePicker] 这个函数类型。
///
/// 与 `app/browser_io.dart` 的定位一致：平台适配集中在一处，别处只拿结果。
library;

export 'persona_card_picker_stub.dart'
    if (dart.library.js_interop) 'persona_card_picker_web.dart';
