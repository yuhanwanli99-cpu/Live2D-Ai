/// 主题标识：**纯逻辑、零依赖**（连 `package:flutter` 都不 import）。
///
/// 单独成文件的理由：`DisplayPrefs` 要持久化它，而 `DisplayPrefs` 是
/// 「可在 VM 上单测的纯逻辑」——把枚举放在 `tokens.dart` 里会让它顺带拖进
/// Material。这里只有 id / 中文名 / 存储键，没有任何颜色。
///
/// # 为什么是「主题」而不是「强调色」
///
/// 2026-09-11 用户裁决：「配色保留黑色的同时支持 3 色切换，
/// 我们做 live2d 相关，支持 黑/白/蓝/灰 4 色先」。
///
/// 四套是**整套配色**（舞台底 / 面板 / 文字 / 强调色 / 语义色一起换），
/// 不是只换一个 accent——只换 accent 会让「黑底 + 深蓝强调」这种低对比组合
/// 出现，那正是「看起来不舒服」的来源之一。具体的取值在
/// `lib/design/tokens.dart` 的 [AppPalette] 里。
library;

/// 四套主题。
///
/// **枚举顺序 = 设置里的显示顺序**，第一项是默认。
enum AppThemeId {
  /// 默认：纯黑舞台 + 冷白强调（产品原本的观感）。
  black('black', '黑', '纯黑舞台，冷白强调'),

  /// 纯白舞台 + 近黑强调。
  white('white', '白', '纯白舞台，近黑强调'),

  /// 深蓝舞台 + 天蓝强调。
  blue('blue', '蓝', '深蓝舞台，天蓝强调'),

  /// 中性灰舞台 + 灰白强调（比黑柔一档）。
  gray('gray', '灰', '中灰舞台，中性强调');

  const AppThemeId(this.wire, this.label, this.hint);

  /// 存储键（localStorage JSON 里的值）。**改名等于让所有用户回到默认**，
  /// 所以它与枚举名解耦、单独写出来。
  final String wire;

  /// 界面上的短名（一个字，放在四选一的分段控件里）。
  final String label;

  /// 一句话说明（设置里显示）。
  final String hint;

  /// 找不到 / 坏值时的回落目标。**默认黑**（用户裁决：保留黑色）。
  static const AppThemeId fallback = AppThemeId.black;

  /// 从存储值解析：**永不抛**。
  static AppThemeId fromWire(Object? raw) {
    if (raw is! String) return fallback;
    for (final AppThemeId id in values) {
      if (id.wire == raw) return id;
    }
    return fallback;
  }

  /// 下一套主题（循环）。给快捷键 / 单按钮切换用。
  AppThemeId get next => values[(index + 1) % values.length];
}
