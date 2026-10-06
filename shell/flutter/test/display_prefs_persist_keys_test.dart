/// **E12：DisplayPrefs 落盘 24 键全集守卫**（2026-10-06，此前零覆盖）。
///
/// # 为什么要有这条门禁
///
/// 整份偏好是 localStorage 里的**一条**记录：
/// jsonEncode(prefs.toJson()) / DisplayPrefs.fromJson(...)
/// （app/browser_io.dart）。fromJson 的契约是「缺字段 / 坏值一律回落默认，
/// **永不抛**」——于是**少写一个 JSON 键的后果是静默的**：
/// 用户改过的那个设置刷新后悄悄复原，没有任何错误、没有任何日志。
///
/// 2026-10-06 实测（docs/plans/DECISION-display-prefs-2026-10-06.md §1.5）：
/// 全仓**没有一条**测试钉住 DisplayPrefs 的 24 键全集；
/// test/display_prefs_background_fit_test.dart:351 钉的是
/// BackgroundImage.toJson()（**逐图**），不是偏好本体。
/// 本文件补上这一条，并在 DisplayPrefs 拆 part 之前先落地
/// （决策纸 §4 的顺序铁律：**先加测试，再动代码**）。
///
/// # 四条判据（少一个键 / 多一个键 / 改名 / 字段与键脱节 ⇒ 红）
///
/// 1. toJson() 的键集合 == 本文件手写的 24 键全集 [kPersistedKeys]；
/// 2. const DisplayPrefs({...}) 的 this.<字段> 集合 == 同一份键集合
///    （**读库 + 它的全部 part**：readLibrarySource，拆分后扫描面不缩水）；
/// 3. 表驱动：24 个键逐个「非默认值 → toJson → fromJson」往返，
///    并断言该键确实出现在 JSON 里、且与默认值不同（防「键在但值没落盘」）；
/// 4. 未知键被忽略、旧版本键（shellImage / syncShellStageBg）仍被迁移。
///
/// # 已知边界（如实标注）
///
/// 判据 2 是**源码扫描**：它证明「构造函数声明的字段集合」与「落盘键集合」
/// 一致，不证明 fromJson 真的读了每一个键——后者由判据 3 的逐键往返覆盖。
/// 两者合起来才等于「24 键闭环」。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';

import 'support/dart_library.dart';

/// 落盘契约：**24 键全集**（toJson 的键 == 构造函数的 24 个字段）。
///
/// 加字段时**必须**同时改这里、[kPersistedKeyCount] 与 [nonDefaultPrefs]，
/// 并且把它写进 toJson / fromJson / copyWith——漏一个就红。
const Set<String> kPersistedKeys = <String>{
  'theme',
  'stageImage',
  'backgrounds',
  'backgroundSource',
  'backgroundOpacity',
  'backgroundBlur',
  'backgroundScrim',
  'backgroundEnabled',
  'imageFit',
  'imageAlign',
  'tileSize',
  'slideInterval',
  'slideRandom',
  'uiTransparency',
  'stagePlaylist',
  'scale',
  'mouthSensitivity',
  'lipSync',
  'idleEnabled',
  'muted',
  'volume',
  'allowDragZoom',
  'tier',
  'edgeStrength',
};

/// 键总数（与 [kPersistedKeys] 分开写，是为了让「改了一个忘了另一个」也红）。
const int kPersistedKeyCount = 24;

/// 一张足够小的合法 dataURL（落盘长度远低于 kStageImageMaxChars）。
const String kTinyImage = 'data:image/png;base64,iVBORw0KGgo=';

/// 每个键的「非默认值」样本：一份只把那一个字段改成非默认值的偏好。
///
/// 表驱动（而不是 24 条手写 test）的理由：**加第 25 个字段时，
/// 忘记加样本会比忘记写测试更早暴露**——下面的「表覆盖全集」断言会红。
final Map<String, DisplayPrefs> nonDefaultPrefs = <String, DisplayPrefs>{
  'theme': const DisplayPrefs(theme: AppThemeId.white),
  'stageImage': const DisplayPrefs(stageImage: kTinyImage),
  'backgrounds': const DisplayPrefs(
    backgrounds: <BackgroundItem>[
      BackgroundPattern(BackgroundPatternId.grid),
    ],
  ),
  'backgroundSource': const DisplayPrefs(
    backgroundSource: DisplayPrefs.backgroundSourceStageImage,
  ),
  'backgroundOpacity': const DisplayPrefs(backgroundOpacity: 0.42),
  'backgroundBlur': const DisplayPrefs(backgroundBlur: 3.0),
  'backgroundScrim': const DisplayPrefs(backgroundScrim: 2),
  'backgroundEnabled': const DisplayPrefs(backgroundEnabled: false),
  'imageFit': const DisplayPrefs(imageFit: 1),
  'imageAlign': const DisplayPrefs(imageAlign: 0),
  'tileSize': const DisplayPrefs(tileSize: 32.0),
  'slideInterval': const DisplayPrefs(slideInterval: 30),
  'slideRandom': const DisplayPrefs(slideRandom: true),
  'uiTransparency': const DisplayPrefs(uiTransparency: 0.2),
  'stagePlaylist': const DisplayPrefs(stagePlaylist: <String>[kTinyImage]),
  'scale': const DisplayPrefs(scale: 1.3),
  'mouthSensitivity': const DisplayPrefs(mouthSensitivity: 2.0),
  'lipSync': const DisplayPrefs(lipSync: false),
  'idleEnabled': const DisplayPrefs(idleEnabled: false),
  'muted': const DisplayPrefs(muted: true),
  'volume': const DisplayPrefs(volume: 0.3),
  'allowDragZoom': const DisplayPrefs(allowDragZoom: false),
  'tier': const DisplayPrefs(tier: 16384),
  'edgeStrength': const DisplayPrefs(edgeStrength: 1.4),
};

/// 从库源码里抽 const DisplayPrefs({ ... }) 的 this.<字段> 集合。
///
/// 为什么用源码扫描而不是 Dart 反射：本项目不用 dart:mirrors
/// （web 上也没有）；而「构造函数里声明了哪些字段」是**静态可判**的结构事实，
/// 与本仓既有的源码守卫（test/source_scan_test.dart 等）同一条纪律。
///
/// 扫描面必须走 readLibrarySource（库 + part）：DisplayPrefs 拆 part 后
/// 只读库文件会判「构造函数不见了」——那正是 E1-a 修过的漏扫形状。
Set<String> constructorFields(String source) {
  final int start = source.indexOf('const DisplayPrefs({');
  expect(
    start,
    greaterThanOrEqualTo(0),
    reason: '找不到 const DisplayPrefs({ —— 判据与源码形状脱节，这条门禁在空转',
  );
  final int end = source.indexOf('});', start);
  expect(end, greaterThan(start), reason: '构造函数没有正常收尾（找不到 });）');
  final String ctor = source.substring(start, end);
  return RegExp(
    r'this\.(\w+)',
  ).allMatches(ctor).map((RegExpMatch m) => m.group(1)!).toSet();
}

void main() {
  group('落盘 24 键全集（E12）', () {
    test('toJson 的键集合 == 手写全集（少一个 / 多一个 / 改名都红）', () {
      final Set<String> actual = const DisplayPrefs().toJson().keys.toSet();
      expect(
        actual,
        kPersistedKeys,
        reason:
            '偏好是 localStorage 的**一条**记录：少一个键 = 那个设置刷新后静默复原'
            '（fromJson 永不抛，没有任何错误）。多了/改名 = 旧存档读不回来。',
      );
      expect(actual, hasLength(kPersistedKeyCount));
    });

    test('构造函数字段集合 == 落盘键集合（字段与键脱节就红）', () {
      final Set<String> fields = constructorFields(
        readLibrarySource('lib/settings/display_prefs.dart'),
      );
      expect(
        fields.length,
        kPersistedKeyCount,
        reason:
            'const 构造声明的字段数变了：要么补 kPersistedKeys / toJson / fromJson / '
            'copyWith，要么说明这个字段**刻意不落盘**（那也要在这里显式登记）。',
      );
      expect(
        fields,
        kPersistedKeys,
        reason: '构造函数字段集合与 toJson 键集合必须 1:1：字段没有键 = 改了不落盘；'
            '键没有字段 = 读回来没处放。',
      );
    });

    test('非默认值样本表覆盖全部 24 键（加字段忘了加样本也红）', () {
      expect(nonDefaultPrefs.keys.toSet(), kPersistedKeys);
      expect(nonDefaultPrefs, hasLength(kPersistedKeyCount));
    });

    for (final String key in <String>[
      'theme',
      'stageImage',
      'backgrounds',
      'backgroundSource',
      'backgroundOpacity',
      'backgroundBlur',
      'backgroundScrim',
      'backgroundEnabled',
      'imageFit',
      'imageAlign',
      'tileSize',
      'slideInterval',
      'slideRandom',
      'uiTransparency',
      'stagePlaylist',
      'scale',
      'mouthSensitivity',
      'lipSync',
      'idleEnabled',
      'muted',
      'volume',
      'allowDragZoom',
      'tier',
      'edgeStrength',
    ]) {
      test('键 $key 落到 JSON 且能原样读回（其余 23 键不变）', () {
        final DisplayPrefs base = const DisplayPrefs();
        final DisplayPrefs changed = nonDefaultPrefs[key]!;
        final Map<String, Object?> json = changed.toJson();

        expect(
          json.containsKey(key),
          isTrue,
          reason: '键 $key 没有出现在 toJson() 里——这个字段改了不会落盘',
        );
        expect(
          json[key] == base.toJson()[key],
          isFalse,
          reason: '样本表里 $key 给的非默认值没有真的写进 JSON（键在、值没落盘）',
        );
        expect(
          DisplayPrefs.fromJson(json),
          changed,
          reason: '键 $key 往返之后整份偏好不相等：要么这一键丢了，要么串到别的字段上',
        );
        expect(
          changed,
          isNot(base),
          reason: '键 $key 没有参与 ==：改了它会被 if (next == widget.prefs) return; 静默吞掉',
        );
        expect(
          DisplayPrefs.fromJson(json).hashCode,
          changed.hashCode,
          reason: '键 $key 的 == 与 hashCode 不对称（相等对象必须同 hash）',
        );
      });
    }

    test('未知键被忽略、不抛（将来加字段再降级也不该把整份偏好读坏）', () {
      final Map<String, Object?> json = const DisplayPrefs().toJson()
        ..['futureFieldOfSomeOtherVersion'] = 42
        ..['anotherUnknown'] = <String, Object?>{'nested': true};
      expect(DisplayPrefs.fromJson(json), const DisplayPrefs());
    });

    test('旧版本键仍被迁移：shellImage / syncShellStageBg', () {
      // ① rc.5 的单张 shellImage（壳背景的旧字段）→ 迁进背景库，来源是库。
      final DisplayPrefs legacyImage = DisplayPrefs.fromJson(<String, Object?>{
        'shellImage': kTinyImage,
      });
      expect(
        legacyImage.backgrounds,
        hasLength(1),
        reason: 'shellImage 必须迁进背景库（写回时不再产生旧键，见下一条）',
      );
      expect(
        legacyImage.backgroundSource,
        DisplayPrefs.backgroundSourceLibrary,
      );

      // ② 「壳跟随舞台」且**确有一张舞台图** ⇒ 迁到「舞台那张」，老用户界面不变。
      final DisplayPrefs followStage = DisplayPrefs.fromJson(<String, Object?>{
        'syncShellStageBg': true,
        'stageImage': kTinyImage,
      });
      expect(
        followStage.backgroundSource,
        DisplayPrefs.backgroundSourceStageImage,
      );

      // ③ 本缺陷现场：syncShellStageBg 默认 true 而 stageImage 为 null ⇒ 库。
      final DisplayPrefs noStage = DisplayPrefs.fromJson(<String, Object?>{
        'syncShellStageBg': true,
      });
      expect(
        noStage.backgroundSource,
        DisplayPrefs.backgroundSourceLibrary,
        reason: '没有舞台图却迁到「舞台那张」= 用户加的图一张都不显示',
      );
    });

    test('写回**不再**产生旧键（一次性迁移，不是两套真相并存）', () {
      final Map<String, Object?> json = const DisplayPrefs(
        stageImage: kTinyImage,
      ).toJson();
      expect(json.containsKey('shellImage'), isFalse);
      expect(json.containsKey('syncShellStageBg'), isFalse);
    });
  });
}
