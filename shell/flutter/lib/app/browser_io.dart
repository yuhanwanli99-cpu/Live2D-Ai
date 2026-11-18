/// 浏览器 I/O 适配层：localStorage 偏好/会话存档 + 本地文件选择。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**——那里曾写着「这里是唯一允许
/// `import 'package:web'` 的地方之一」。搬出的理由不是行数：存储、文件选择、图片读取
/// 都属于**平台适配**，与组合根「谁持有谁、哪个回调接哪个方法」的职责无关；
/// 而它们需要 `package:web`，留在入口会把 web 依赖渗进整个组合根。
///
/// 各函数仍然「**任何异常都回落默认**」（坏存储不该让页面打不开），但**落盘
/// 失败不再静默**：聊天会话存档的写入会返回 bool 并在 store 上留提示（#9，
/// 2026-11-18），偏好写入也早已返回 bool——静默失败是本项目最贵的 bug 类型。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:js_interop';

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

/// 逐会话条目的键前缀（v2 格式，见 `chat/chat_session.dart` 的存档格式说明）：
/// `live2d-ai.chat-sessions.<会话 id>`。
///
/// 拆键的理由写在纯逻辑层：v1 一个键装全量，一条坏记录就吃掉整份历史（#10）。
/// 键名放在这层（而不是纯逻辑层）是因为只有这里允许 `package:web`；纯逻辑层
/// 只认「id → JSON 原文」这个抽象，于是能在 VM 上单测。
const String kChatSessionEntryPrefix = '$kChatSessionsKey.';

/// 读取本地显示偏好：**任何异常都回落默认**（坏存储不该让舞台渲染不出来）。
///
/// 整份偏好是**一条** localStorage 记录（`jsonEncode(prefs.toJson())`），
/// 所以 `stagePlaylist`（舞台背景轮播列表）也走这里，不单独开键——
/// 它的三条预算（每张 / 总长 / 项数）在 `DisplayPrefs.readStagePlaylist`
/// 里**逐项**把关：一条坏数据只丢自己，不会把主题 / 音量一起判死。
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

/// 写入本地显示偏好；返回**是否真的写进去了**。
///
/// 无痕模式 / 配额满 / 站点存储被禁都会抛异常。以前返回 `void` 且静默吞掉，
/// 于是用户「选了背景图 → 界面显示已应用 → 刷新就没了」而无任何解释
/// （rc.3 §9.1 候选原因 1）。现在如实返回，由调用方给一句可执行的文案。
bool saveDisplayPrefs(DisplayPrefs prefs) {
  try {
    web.window.localStorage.setItem(
      kDisplayPrefsKey,
      jsonEncode(prefs.toJson()),
    );
    return true;
  } catch (_) {
    return false;
  }
}

/// 读取聊天会话存档：**任何异常都回落空 store**（但会带上「读不出来」的提示）。
///
/// v2（#10）：先读索引，再**逐条**读每个会话自己的键。一条坏记录只丢它自己，
/// 不再像 v1 那样「一个坏字节 → 整份存档读不出来」。索引本身坏掉时无从知道
/// 该读哪些条目，只能返回空 store——但会标
/// [ChatSessionStore.markArchiveUnreadable]，让界面说「存档读不出来」而不是
/// 假装「从没有过会话」。
///
/// JSON 结构判据全在 `chat/chat_session.dart` 的纯函数里（VM 可测）；本函数
/// 只做 localStorage I/O。
ChatSessionStore loadChatSessions() {
  try {
    final String? raw = web.window.localStorage.getItem(kChatSessionsKey);
    final ChatSessionLoadPlan plan = decodeChatSessionIndex(raw);
    // v1：索引键里装着全量会话，decode 已在内存里迁移成 store（下次保存写 v2）。
    final ChatSessionStore? legacy = plan.legacyStore;
    if (legacy != null) return legacy;
    if (plan.indexCorrupt) {
      return ChatSessionStore.empty()..markArchiveUnreadable();
    }
    if (plan.ids.isEmpty) return ChatSessionStore.empty();
    // 逐条读：缺失的键保持 null，由纯逻辑层按「坏条目」计数跳过。
    final Map<String, String?> entries = <String, String?>{};
    for (final String id in plan.ids) {
      entries[id] = web.window.localStorage.getItem(
        kChatSessionEntryPrefix + id,
      );
    }
    return loadChatSessionEntries(plan, entries);
  } catch (_) {
    // localStorage 不可用（隐私模式 / 站点存储被禁）：不是「没有历史」，
    // 但也做不了更多——带提示回落空 store。
    return ChatSessionStore.empty()..markArchiveUnreadable();
  }
}

/// 写入聊天会话存档；返回**是否真的写进去了**（#9）。
///
/// 失败不再静默：无痕模式 / 配额满 / 站点存储被禁都会抛。以前返回 `void` 并
/// 吞掉异常，用户会以为记录还在，刷新后才发现全没了。现在：
/// ① 如实返回 `false`；② 在 store 上置 [ChatSessionStore.markSaveFailed]，
/// 由 UI 显示 `kSessionsNotPersistedNote`。
///
/// **配额满不是假设**：localStorage 常见上限 5 MB，而 `ChatSessionStore`
/// 用 `maxSessions` × `kMaxMessagesPerSession` 双重夹持把体积压在配额内——
/// 即便如此，写失败也只丢「这次之后的新记录」，不该影响正在进行的对话。
///
/// v2 写入顺序（#10）：**先逐条、后索引**。索引是提交点——顺序反过来的话，
/// 中途失败会留下「索引指向不存在的条目」，那些会话在加载时就成了幽灵。
/// 最后按前缀清扫索引里已不存在的旧条目键：删掉的会话不能只从索引里消失，
/// 否则条目键会永久占着配额。
bool saveChatSessions(ChatSessionStore store) {
  try {
    final web.Storage storage = web.window.localStorage;
    for (final ChatSession session in store.sessions) {
      storage.setItem(
        kChatSessionEntryPrefix + session.id,
        encodeChatSessionEntry(session),
      );
    }
    // 提交点：索引写成功之后，这份存档才算「完整更新」。
    storage.setItem(kChatSessionsKey, encodeChatSessionIndex(store));
    _removeStaleSessionEntries(storage, store);
    store.migratedFromLegacy = false;
    store.clearSaveFailure();
    return true;
  } catch (_) {
    store.markSaveFailed();
    return false;
  }
}

/// 清掉索引里已经没有的逐会话键（v2）。
///
/// **先收集再删**：边遍历边 `removeItem` 会让下标前移、跳过一项。
/// 必须在索引写成功之后调用：索引是「活着的 id 列表」的唯一真源。
void _removeStaleSessionEntries(web.Storage storage, ChatSessionStore store) {
  final Set<String> live = <String>{
    for (final ChatSession session in store.sessions)
      kChatSessionEntryPrefix + session.id,
  };
  final List<String> stale = <String>[];
  for (int i = 0; i < storage.length; i++) {
    final String? key = storage.key(i);
    if (key != null &&
        key.startsWith(kChatSessionEntryPrefix) &&
        !live.contains(key)) {
      stale.add(key);
    }
  }
  for (final String key in stale) {
    storage.removeItem(key);
  }
}

/// **重新加载本页**（`/app/`）。
///
/// 2026-10-09：设置保存做成一步之后，服务端已经用上新值——而页面上那几份
/// 「服务端真相」都是加载时取的快照，不刷新就会停在旧值。这里做的是**同源
/// 整页重载**（不是改 `location.href`），所以不产生一条新的历史记录。
void reloadAppPage() {
  web.window.location.reload();
}

/// **关闭本页**（`/app/`）。浏览器只允许关闭「由脚本打开」的窗口，被拒时
/// `close()` 是静默 no-op（返回 void、不抛）。所以调用方必须**先把提示上屏**
/// 再调这里：允许关 → 页面消失；不允许关 → 提示就是最终画面。
void closeAppPage() {
  web.window.close();
}

/// 让用户挑一张本地图片，读成 **dataURL**（组合根专有：需要 `package:web`）。
///
/// 用 `FileReader.readAsDataURL` 而不是自己拼 base64：它按文件的真实 MIME
/// 产出 `data:image/png;base64,…`，浏览器直接认。
///
/// **不做缩放**（刻意的）：裁剪要走 canvas，而 canvas 只能在浏览器里跑、
/// 本仓库的 `flutter test` 覆盖不到——本轮不引入无法回归的代码。
/// 代价是「大图不写盘」，由调用方按 [kStageImageMaxChars] 如实告知用户。
///
/// 返回 **取消 / 成功 / 读失败** 三态：取消（`dataUrl==null && error==null`）
/// 不该弹东西；读失败（`error!=null`）必须说实话——以前读失败与取消不可区分，
/// 用户看到的就是「点了选图没反应」。
Future<({String? dataUrl, String? error})> pickImageDataUrl() =>
    pickFileDataUrl(
      accept: 'image/*',
      unreadableMessage: '这张图读不出内容（可能不是浏览器能识别的图片格式）',
      failedMessage: '读取图片失败（文件可能已被移动、删除或没有读取权限）',
    );

/// 通用版：挑**一个**文件读成 dataURL（`accept` 用 input 的 accept 语法）。
///
/// 产品级加强波次从 [pickImageDataUrl] 抽出，供**角色卡导入**复用（`.json` /
/// 带 `chara` 的 `.png` 都要能选）。行为与原来逐字一致，只是 `accept` 与
/// 两条失败文案可配（图片那条保持原文案，避免改用户可见的话）。
Future<({String? dataUrl, String? error})> pickFileDataUrl({
  required String accept,
  required String unreadableMessage,
  required String failedMessage,
}) async {
  final web.HTMLInputElement input =
      web.document.createElement('input') as web.HTMLInputElement;
  input.type = 'file';
  input.accept = accept;
  final Completer<({String? dataUrl, String? error})> done =
      Completer<({String? dataUrl, String? error})>();
  void finish(String? dataUrl, String? error) {
    if (!done.isCompleted) done.complete((dataUrl: dataUrl, error: error));
  }

  input.onchange = ((web.Event _) {
    final web.FileList? files = input.files;
    if (files == null || files.length == 0) {
      finish(null, null); // 取消
      return;
    }
    final web.FileReader reader = web.FileReader();
    reader.onload = ((web.Event _) {
      final Object? result = reader.result;
      if (result is String && result.isNotEmpty) {
        finish(result, null);
      } else {
        finish(null, unreadableMessage);
      }
    }).toJS;
    reader.onerror = ((web.Event _) {
      finish(null, failedMessage);
    }).toJS;
    reader.readAsDataURL(files.item(0)!);
  }).toJS;
  input.oncancel = ((web.Event _) => finish(null, null)).toJS;
  input.click();
  return done.future;
}
