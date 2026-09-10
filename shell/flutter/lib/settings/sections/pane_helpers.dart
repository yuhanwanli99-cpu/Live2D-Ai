/// 各分区共用的取值助手。
///
/// **草稿优先、服务端兜底**：界面上显示的值永远是「用户改过就用改的，没改就
/// 用服务端的」。写成一行纯函数而不是各 pane 各自 `??`，是为了让
/// 「显示的值」与「将要提交的值」永远走同一个判据——两处各写一遍迟早会漂移。
library;

import '../../api/settings_models.dart';

/// 草稿优先取字符串。
String effString(String? drafted, String remote) => drafted ?? remote;

/// 草稿优先取可空字符串（`tts.model` 协议上可为 null）。
String? effNullableString(String? drafted, String? remote) =>
    drafted ?? remote;

/// 草稿优先取整数。
int effInt(int? drafted, int remote) => drafted ?? remote;

/// 草稿优先取布尔。
bool effBool(bool? drafted, bool remote) => drafted ?? remote;

/// 取三态字段的现值（`TriKeep`/`null` 都算「没有草稿」）。
int? effTriInt(Tri<int>? drafted, int remote) => switch (drafted) {
  TriSet<int>(:final int value) => value,
  // `Tri.clear()` = 清除 → 回落服务端默认（界面显示服务端默认值）。
  _ => remote,
};

/// 该字段是否被用户改过（用于给标题打一个小圆点）。
bool isDrafted(Object? drafted) => drafted != null;
