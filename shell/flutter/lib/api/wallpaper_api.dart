/// 壁纸 Mod 接线 API + 两个**纯判据**（Wave 2 B 轨，2026-09-14）。
///
/// # 它接的是哪条线
///
/// 壁纸 Mod（`crates/live2d-ai-mod-wallpaper`）只产**决策**，不画图、不持图：
/// 它把决策投影成 `prefs_patch`，经基座 Wave 2 的
/// `GET /api/v1/mods/wallpaper/state` 交给前端；前端用**既有**
/// `applyWallpaperPatch`（`settings/display_prefs.dart`）落到 `DisplayPrefs`，
/// 再走**既有** `Live2DStage.sendStageBg`。写回列表长度用**既有**
/// `POST /api/v1/mods/wallpaper/config`。**没有**新增 ws 帧 / wasm 路径 / 端点。
///
/// # 为什么两个判据是独立的纯函数
///
/// 「要不要轮询」与「要不要写回」一旦埋在 `Timer` / `setState` 里就没法回归，
/// 而它们各自都有真实事故面：
///
/// - 停用（或 `mode = off`）却还在轮询 = 白打请求，也违背「停用即停」；
/// - 列表长度没变却天天 `POST config` = 每次都重启 Mod，把策略游标冲掉。
library;

import 'dart:convert';

import 'package:http/http.dart' as http;

import 'api_client.dart';

/// 轮询间隔（秒）。契约要求 **≥5 s**：这是「看一眼状态」而不是实时流，
/// 且 Mod 侧的节拍就是这次调用的增量毫秒——轮询本身就是时钟。
const Duration kWallpaperPollInterval = Duration(seconds: 5);

/// 壁纸 Mod 的 id（**唯一真源**，路由与配置写回都读它）。
const String kWallpaperModId = 'wallpaper';

/// `mode` 的哪个取值意味着「本 Mod 会动作」。
///
/// 与 Rust 侧 `WallpaperMode::is_active` **同一个判据**（`off` 之外都算动作）。
/// 未知 / 非字符串 / 缺失一律按 `off` 处理（宽容，fail-safe 到「不接管」）。
bool wallpaperModeIsActive(Object? mode) =>
    mode == 'follow_stage' || mode == 'interval';

/// 是否应该轮询壁纸状态（纯判据）。
///
/// 三条同时成立才轮询：**在注册表里**、**已启用**、**mode != off**。
/// 任一条不成立 → 不轮询（停用 / 关掉模式后调用方应立即停掉 `Timer`）。
bool shouldPollWallpaperState({
  required bool registered,
  required bool enabled,
  required Object? mode,
}) {
  if (!registered || !enabled) return false;
  return wallpaperModeIsActive(mode);
}

/// 是否要把本地列表长度写回服务端（纯判据）。
///
/// - 服务端已有一个不同的数字 → 要写；
/// - 服务端没有该键（老配置）→ **只有本地非 0 时才写**（0 是缺省值，写它没意义）。
bool needsPlaylistLenWriteBack({
  required int localLen,
  required Object? remoteLen,
}) {
  if (remoteLen is! num) return localLen != 0;
  return remoteLen.toInt() != localLen;
}

/// 把列表长度合进一份既有 config。
///
/// `POST /api/v1/mods/{id}/config` 是**整份替换**（不是 patch），
/// 所以必须从服务端现有 config 出发、只覆盖 `playlist_len`——否则一次写回
/// 会把用户设的 `mode` / `interval_secs` 一起抹掉。
Map<String, Object?> configWithPlaylistLen(
  Map<String, Object?> current,
  int len,
) => <String, Object?>{...current, 'playlist_len': len};

/// `GET /api/v1/mods/{id}/state` 的读取面（只读，无副作用）。
class WallpaperApi {
  WallpaperApi({String? base, http.Client? client})
    : _base = _normalize(base ?? const String.fromEnvironment('API_BASE')),
      _client = client ?? http.Client();

  final String _base;
  final http.Client _client;

  Uri _uri(String path) => Uri.parse('$_base$path');

  /// 取一次壁纸 Mod 运行态；**读不到返回 `null`**（不抛）。
  ///
  /// 契约（`mods_routes.rs::handle_mod_state_get`）：
  /// - `200 {"id","enabled","state":{…}}` → 返回 `state` 对象；
  /// - `503 state_unavailable`（未启用 / 未实现 `state_json` / worker 正持锁）
  ///   → `null`。**这不是错误**：轮询遇到它只需等下一次，写进错误横幅反而
  ///   会在「Mod 刚被停用」时闪一条假故障；
  /// - `404`（不在注册表）→ `null`（同理）；
  /// - 其它状态码 / 网络错误 → 抛 [ApiException]（调用方决定要不要提示）。
  Future<Map<String, Object?>?> state() async {
    final http.Response response;
    try {
      response = await _client.get(
        _uri('/api/v1/mods/$kWallpaperModId/state'),
      );
    } catch (error) {
      throw ApiException('network_error', '无法连接后端：$error');
    }
    if (response.statusCode == 404 || response.statusCode == 503) return null;
    if (response.statusCode != 200) {
      throw ApiException(
        'http_${response.statusCode}',
        response.body.isEmpty ? '壁纸状态读取失败' : response.body,
        status: response.statusCode,
      );
    }
    final Object? decoded = _decode(response.body)['state'];
    if (decoded is! Map) return null;
    final Map<String, Object?> out = <String, Object?>{};
    decoded.forEach((Object? k, Object? v) {
      if (k is String) out[k] = v;
    });
    return out;
  }

  void dispose() => _client.close();
}

String _normalize(String raw) {
  var value = raw.trim();
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  return value.isEmpty ? Uri.base.origin : value;
}

Map<String, Object?> _decode(String body) {
  try {
    final Object? decoded = jsonDecode(body);
    if (decoded is Map) {
      final Map<String, Object?> out = <String, Object?>{};
      decoded.forEach((Object? k, Object? v) {
        if (k is String) out[k] = v;
      });
      return out;
    }
  } catch (_) {
    // 坏 JSON → 空对象。
  }
  return const <String, Object?>{};
}
