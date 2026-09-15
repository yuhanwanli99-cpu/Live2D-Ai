/// 非 Web 目标的角色卡选择兜底：**如实说不可用**，不摆一个按不动的入口。
///
/// 见 `persona_card_picker.dart` 头注（条件导入的理由）。
library;

/// 返回 `(dataUrl: null, error: …)`：调用方据此显示一句可处置的说明。
Future<({String? dataUrl, String? error})> pickPersonaCardFile() async => (
  dataUrl: null,
  error: '当前构建不支持文件选择（只在浏览器里可用）；请把角色卡 JSON 直接粘进输入框',
);
