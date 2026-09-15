/// Web 目标的角色卡选择：浏览器文件对话框（`.json` / 带 `chara` 的 `.png`）。
///
/// 见 `persona_card_picker.dart` 头注（条件导入的理由）。
library;

import '../../app/browser_io.dart';

/// 打开文件对话框并把选中的卡读成 dataURL（取消 → 两项皆 null）。
Future<({String? dataUrl, String? error})> pickPersonaCardFile() =>
    pickFileDataUrl(
      accept: '.json,.png,application/json,image/png',
      unreadableMessage: '这个文件读不出内容（角色卡应是 .json，或带 chara 块的 .png）',
      failedMessage: '读取角色卡失败（文件可能已被移动、删除或没有读取权限）',
    );
