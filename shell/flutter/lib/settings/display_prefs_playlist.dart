/// DisplayPrefs 的**轮播列表与字节算术**顶层声明（display_prefs.dart 的 part）。

///

/// 2026-10-06 按 docs/plans/DECISION-display-prefs-2026-10-06.md 的 B2 方案

/// 从 display_prefs.dart **逐字搬出**（同库 part：所有顶层声明仍在同一 library 里，

/// import display_prefs.dart 的调用点一字不改）。

/// 这里只放**顶层**常量 / 函数 / 小类；类方法在 display_prefs_{codec,derived,limits,copy}.dart。

part of 'display_prefs.dart';

/// 背景库最多几项。
///
/// 8 是内存与观感的折中：每一项的字节都常驻内存（base64 字符串 + 轮播时
/// 解码出的位图），8 张 4 MB 的图约 64 MB，再多就开始影响启动与切图。
const int kBackgroundMaxCount = 8;

/// **单张**背景图的字节上限（24 MB，按**解码后**的字节算，不是 base64 字符）。
///
/// # 为什么还有上限，而 2026-09-27 之前是 390 KB
///
/// 之前那个数（`kBackgroundImageMaxChars = 400000`）不是安全边际，
/// 是**事故**：它让「选一张 1080p 照片」永远失败——实测 3 张图一张都存不进去。
///
/// 现在的 24 MB 不是配额保护（配额在 IndexedDB 那里，量大得多），
/// 而是**防手抖**：有人把一个 200 MB 的 TIFF 拖进来，base64 之后
/// 字符串本身就能把标签页的内存吃穿，那会拖垮整个应用而不只是背景。
/// 24 MB 已经远大于任何真实壁纸。
const int kBackgroundImageMaxBytes = 24 * 1024 * 1024;

/// 背景库字节占用（**解码后**的字节；图案零成本）。
int backgroundBytesOf(BackgroundItem item) {
  if (item is! BackgroundImage) return 0;
  final String? url = item.dataUrl;
  if (url == null) return 0;
  // base64：4 个字符 = 3 个字节。整数运算（不 import dart:convert），
  // 免得这个纯逻辑文件多一条依赖。
  final int comma = url.indexOf(',');
  final int payload = comma >= 0 ? url.length - comma - 1 : url.length;
  return (payload / 4 * 3).floor();
}

/// dataURL 大致折成多少字节（**纯算术，可 VM 单测**）。
int dataUrlBytes(String dataUrl) {
  final int comma = dataUrl.indexOf(',');
  final int payload = comma >= 0 ? dataUrl.length - comma - 1 : dataUrl.length;
  return (payload / 4 * 3).floor();
}


/// 舞台背景图 dataURL 的**长度上限**（字符数，不是字节）。
///
/// 取值理由：localStorage 的常见配额是 **5 MB / 源**，而 dataURL 是 base64
/// （比原始字节大 33%），且它会被塞进偏好记录一起写。留 ~1.5 M 字符
/// （约 1.1 MB 原始数据）给背景图，剩下的余量给其余偏好与将来新增字段——
/// 一旦超配额，**整份偏好都写不进去**（不只是背景图），那会连带丢掉主题、
/// 音量、口型设置，代价远大于「一张图没记住」。
///
/// 超限不是错误：**本次会话照常生效**，只是不写盘（UI 会如实说明）。
/// 下游裁剪（canvas 缩放）没有做——它需要浏览器 canvas，属于不可测代码，
/// 这一轮刻意不引入（见 `docs/verification/flutter-shell-manifest-checklist.md`）。
const int kStageImageMaxChars = 1500000;

/// 壳背景图 dataURL 的长度上限。
///
/// **与 [kStageImageMaxChars] 同一个预算**：壳背景与舞台背景写在**同一条**
/// localStorage 记录里，给两份独立上限只会让「整份偏好都写不进去」更容易发生。
/// 所以这里只是给壳背景一个可读的别名，**不是第二套配额**。
const int kShellImageMaxChars = kStageImageMaxChars;

/// 舞台背景轮播列表（用户手动维护）的**项数上限**。
///
/// 16 张的理由见 [kStagePlaylistMaxChars]——真正的上限是**总字符预算**，
/// 这一条只挡住「项数无界」。
const int kStagePlaylistMaxItems = 16;

/// 轮播列表的**总长上限**（所有项字符数之和）。
///
/// # 为什么必须单独有一条（2026-09-14，Wave 2 硬约束）
///
/// localStorage 是**一条记录**存整份 `DisplayPrefs`（`jsonEncode(prefs.toJson())`）。
/// rc.5 的教训是：**一旦超配额，整份偏好都写不进去**——不只是列表，连主题、
/// 音量、口型设置一起丢。所以列表既要有单张上限（每项按
/// [kStageImageMaxChars] 校验），也要有**总长上限**：超出时保留前面的、
/// 丢掉放不下的，并在界面上如实说明「已达上限」，而不是悄悄把整份设置写坏。
const int kStagePlaylistMaxChars = kStageImageMaxChars;

/// 把档位夹到 [DisplayPrefs.tiers] 里的**最近合法值**。
///
/// 不是简单把区间夹住：`10000` 落在 8192 与 16384 之间，夹成区间边界会得到
/// 一个渲染面根本不认识的档位。取最近合法档位才是「宽容但正确」。
int clampTier(int value) {
  int best = DisplayPrefs.tiers.first;
  int bestDistance = (value - best).abs();
  for (final int tier in DisplayPrefs.tiers) {
    final int distance = (value - tier).abs();
    if (distance < bestDistance) {
      best = tier;
      bestDistance = distance;
    }
  }
  return best;
}

/// 两个列表是否逐项相等（不引 Flutter 的 foundation 库，
/// 保持本文件「纯 Dart、VM 可测」的定位）。
bool _sameList(List<String> a, List<String> b) {
  if (identical(a, b)) return true;
  if (a.length != b.length) return false;
  for (int i = 0; i < a.length; i++) {
    if (a[i] != b[i]) return false;
  }
  return true;
}

/// 「加入轮播」的结果：**新列表 + 是否加进去了 + 没加的原因**。
///
/// 原因用稳定字符串（不是给人看的完整句子）：UI 只负责把原因翻译成一句
/// 可读文案，纯逻辑测试直接断言原因——文案会改，原因不该跟着改。
class StagePlaylistAppendResult {
  const StagePlaylistAppendResult({
    required this.playlist,
    required this.added,
    required this.reason,
  });

  /// 返回的列表（成功时是 `current + [dataUrl]`；失败时**原样返回** `current`——
  /// 绝不「悄悄膨胀」）。
  final List<String> playlist;

  /// 是否真的加进去了。
  final bool added;

  /// `added` / `empty` / `item_too_large` / `limit_reached` / `budget_exceeded`。
  final String reason;
}

/// 把一张图追加进轮播列表（纯函数；三条预算逐条把关）。
///
/// 超限时**不修改**列表并给出原因——调用方据此给用户一句可读反馈，
/// 而不是让列表无界膨胀到把整份偏好写坏（见 [kStagePlaylistMaxChars]）。
StagePlaylistAppendResult appendToStagePlaylist(
  List<String> current,
  String? dataUrl,
) {
  if (dataUrl == null || dataUrl.isEmpty) {
    return StagePlaylistAppendResult(
      playlist: current,
      added: false,
      reason: 'empty',
    );
  }
  if (dataUrl.length > kStageImageMaxChars) {
    return StagePlaylistAppendResult(
      playlist: current,
      added: false,
      reason: 'item_too_large',
    );
  }
  if (current.length >= kStagePlaylistMaxItems) {
    return StagePlaylistAppendResult(
      playlist: current,
      added: false,
      reason: 'limit_reached',
    );
  }
  int total = 0;
  for (final String item in current) {
    total += item.length;
  }
  if (total + dataUrl.length > kStagePlaylistMaxChars) {
    return StagePlaylistAppendResult(
      playlist: current,
      added: false,
      reason: 'budget_exceeded',
    );
  }
  return StagePlaylistAppendResult(
    playlist: <String>[...current, dataUrl],
    added: true,
    reason: 'added',
  );
}

/// 第 [index] 张在列表里的下标；找不到（含 [stageImage] 为 null）→ `-1`。
///
/// UI 用它把「当前舞台那张」标出来。判据是**数据相等**（同一份 dataURL）：
/// 列表可以被删除 / 重排，用数据比对才不会在编辑列表后指错人。
int stagePlaylistIndexOf(List<String> playlist, String? stageImage) {
  if (stageImage == null) return -1;
  for (int i = 0; i < playlist.length; i++) {
    if (playlist[i] == stageImage) return i;
  }
  return -1;
}

/// 删除第 [index] 张（Wave 3）。
///
/// - 越界（负数 / `>= length`）→ **原样返回**入参（不抛、不猜）；
/// - 成功 → 返回**新列表**（不改入参；删除只会让总长变小，三条预算不会被破坏）；
/// - 不动 [DisplayPrefs.stageImage]：删除当前张不会清空舞台——用户看到的那张
///   仍然在屏幕上，是否还在列表里由调用方决定。
List<String> removeStagePlaylistAt(List<String> current, int index) {
  if (index < 0 || index >= current.length) return current;
  return <String>[...current]..removeAt(index);
}

/// 把第 [from] 张移到 [to]（都是 0-based 下标，闭区间）。
///
/// - 越界（`from` / `to` 不在 `0..length`）/ `from == to` → **原样返回**入参；
/// - 成功 → 返回**重排后的新列表**（项集合与项数都不变，所以三条预算不受影响）。
///
/// 「上移」= `moveStagePlaylist(list, i, i - 1)`，「下移」= `(list, i, i + 1)`。
List<String> moveStagePlaylist(List<String> current, int from, int to) {
  if (from < 0 || from >= current.length) return current;
  if (to < 0 || to >= current.length) return current;
  if (from == to) return current;
  final List<String> out = <String>[...current];
  final String item = out.removeAt(from);
  out.insert(to, item);
  return out;
}
