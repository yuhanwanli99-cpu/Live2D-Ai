/// 浏览器 I/O 适配层：localStorage 偏好/会话存档 + 本地文件选择。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**——那里曾写着「这里是唯一允许
/// `import 'package:web'` 的地方之一」。搬出的理由不是行数：存储、文件选择、图片读取
/// 都属于**平台适配**，与组合根「谁持有谁、哪个回调接哪个方法」的职责无关；
/// 而它们需要 `package:web`，留在入口会把 web 依赖渗进整个组合根。
///
/// **纯搬移，零行为变化**：下面每个函数的实现与注释都与原来逐字相同
/// （包括「任何异常都回落默认 / 失败静默」这两条纪律）。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:js_interop';
import 'dart:typed_data';

import 'package:web/web.dart' as web;

import '../chat/chat_session.dart';
import '../settings/display_prefs.dart';

/// 本地显示偏好的存储键（localStorage）。
const String kDisplayPrefsKey = 'live2d-ai.display-prefs';

/// 聊天会话存档的存储键（localStorage）。
///
/// 与 `kDisplayPrefsKey` 分开：偏好是「小而常变」，会话是「大而不常变」。
/// 混在一个键里会让每次拖音量滑杆都重写整份聊天记录。
const String kChatSessionsKey = 'live2d-ai.chat-sessions';

/// 读取本地显示偏好：**任何异常都回落默认**（坏存储不该让舞台渲染不出来）。
DisplayPrefs loadDisplayPrefs() {
  try {
    final raw = web.window.localStorage.getItem(kDisplayPrefsKey);
    if (raw == null || raw.isEmpty) return const DisplayPrefs();
    final decoded = jsonDecode(raw);
    if (decoded is! Map) return const DisplayPrefs();
    return DisplayPrefs.fromJson(decoded.cast<String, Object?>());
  } catch (_) {
    return const DisplayPrefs();
  }
}

/// 写入本地显示偏好（失败静默：无痕模式等场景下 localStorage 可能抛错）。
void saveDisplayPrefs(DisplayPrefs prefs) {
  try {
    web.window.localStorage.setItem(
      kDisplayPrefsKey,
      jsonEncode(prefs.toJson()),
    );
  } catch (_) {
    // 忽略：偏好丢失不影响本次会话。
  }
}

/// 读取聊天会话存档：**任何异常都回落空 store**。
///
/// 与 `loadDisplayPrefs` 同一条纪律：一份被改坏 / 版本不符的存储不该让
/// 聊天区打不开——最坏情况是「历史没了」，而不是「页面崩了」。
ChatSessionStore loadChatSessions() {
  try {
    final String? raw = web.window.localStorage.getItem(kChatSessionsKey);
    if (raw == null || raw.isEmpty) return ChatSessionStore.empty();
    return ChatSessionStore.fromJson(jsonDecode(raw));
  } catch (_) {
    return ChatSessionStore.empty();
  }
}

/// 写入聊天会话存档（失败静默：无痕模式 / 配额满都会抛）。
///
/// **配额满不是假设**：localStorage 常见上限 5 MB，而 `ChatSessionStore`
/// 用 `maxSessions` × `kMaxMessagesPerSession` 双重夹持把体积压在配额内。
/// 即便如此，写失败也只丢「这次之后的新记录」，不该影响正在进行的对话。
void saveChatSessions(ChatSessionStore store) {
  try {
    web.window.localStorage.setItem(
      kChatSessionsKey,
      jsonEncode(store.toJson()),
    );
  } catch (_) {
    // 忽略：存储不可用不影响本次会话。
  }
}

/// 让用户挑一个本地文件并读成字节（**组合根专有**：需要 `package:web`）。
///
/// 为什么不用文件选择器插件：一次 `<input type=file>` 就够了，
/// 引一个插件只为这个属于典型的「顺手加一个」。
/// 用户取消选择时返回 `null`（**不当成错误**）。
Future<Uint8List?> pickLocalFile() async {
  final web.HTMLInputElement input =
      web.document.createElement('input') as web.HTMLInputElement;
  input.type = 'file';
  // 酒馆卡常见两种：`.json` 与内嵌人设的 `.png`。
  input.accept = '.json,.png,application/json,image/png';
  final Completer<Uint8List?> done = Completer<Uint8List?>();
  input.onchange = ((web.Event _) {
    final web.FileList? files = input.files;
    if (files == null || files.length == 0) {
      if (!done.isCompleted) done.complete(null);
      return;
    }
    final web.File file = files.item(0)!;
    final web.FileReader reader = web.FileReader();
    reader.onload = ((web.Event _) {
      final Object? result = reader.result;
      if (result == null) {
        if (!done.isCompleted) done.complete(null);
        return;
      }
      final ByteBuffer buffer =
          (result as JSArrayBuffer).toDart.asUint8List().buffer;
      if (!done.isCompleted) {
        done.complete(buffer.asUint8List());
      }
    }).toJS;
    reader.onerror = ((web.Event _) {
      if (!done.isCompleted) done.complete(null);
    }).toJS;
    reader.readAsArrayBuffer(file);
  }).toJS;
  // **取消选择也要能收口**：`oncancel` 在旧浏览器上可能不触发，
  // 所以这里不做超时兜底——用户若一直不选，这个 Future 就挂着，
  // 而它没有任何副作用（不占资源、不改状态）。
  input.oncancel = ((web.Event _) {
    if (!done.isCompleted) done.complete(null);
  }).toJS;
  input.click();
  return done.future;
}

/// 让用户挑一张本地图片，读成 **dataURL**（组合根专有：需要 `package:web`）。
///
/// 用 `FileReader.readAsDataURL` 而不是自己拼 base64：它按文件的真实 MIME
/// 产出 `data:image/png;base64,…`，浏览器直接认。
///
/// **不做缩放**（刻意的）：裁剪要走 canvas，而 canvas 只能在浏览器里跑、
/// 本仓库的 `flutter test` 覆盖不到——本轮不引入无法回归的代码。
/// 代价是「大图不写盘」，由调用方按 [kStageImageMaxChars] 如实告知用户。
Future<String?> pickImageDataUrl() async {
  final web.HTMLInputElement input =
      web.document.createElement('input') as web.HTMLInputElement;
  input.type = 'file';
  input.accept = 'image/*';
  final Completer<String?> done = Completer<String?>();
  input.onchange = ((web.Event _) {
    final web.FileList? files = input.files;
    if (files == null || files.length == 0) {
      if (!done.isCompleted) done.complete(null);
      return;
    }
    final web.FileReader reader = web.FileReader();
    reader.onload = ((web.Event _) {
      if (done.isCompleted) return;
      final Object? result = reader.result;
      done.complete(result is String && result.isNotEmpty ? result : null);
    }).toJS;
    // 读失败当作「取消」：不弹错误，用户看到的就是没变化。
    reader.onerror = ((web.Event _) {
      if (!done.isCompleted) done.complete(null);
    }).toJS;
    reader.readAsDataURL(files.item(0)!);
  }).toJS;
  input.oncancel = ((web.Event _) {
    if (!done.isCompleted) done.complete(null);
  }).toJS;
  input.click();
  return done.future;
}
