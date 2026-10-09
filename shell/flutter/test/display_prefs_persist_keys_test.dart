/// **E12：DisplayPrefs 落盘 15 键全集守卫**（2026-10-06 起，2026-10-07 收成 15 键）。
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
/// 全仓**没有一条**测试钉住 DisplayPrefs 的落盘键全集；
/// test/display_prefs_background_fit_test.dart:351 钉的是
/// BackgroundImage.toJson()（**逐图**），不是偏好本体。
/// 本文件补上这一条，并在 DisplayPrefs 拆 part 之前先落地
/// （决策纸 §4 的顺序铁律：**先加测试，再动代码**）。
///
/// # 四条判据（少一个键 / 多一个键 / 改名 / 字段与键脱节 ⇒ 红）
///
/// 1. toJson() 的键集合 == 本文件手写的 15 键全集 [kPersistedKeys]；
/// 2. const DisplayPrefs({...}) 的 this.<字段> 集合 == 同一份键集合
///    （**读库 + 它的全部 part**：readLibrarySource，拆分后扫描面不缩水）；
/// 3. 表驱动：15 个键逐个「非默认值 → toJson → fromJson」往返，
///    并断言该键确实出现在 JSON 里、且与默认值不同（防「键在但值没落盘」）；
/// 4. 未知键被忽略；旧版本键（shellImage / stageImage / 已删的样式键）仍被
///    迁移或被**忽略**——迁移只发生一次，写回不再产生旧键。
///
/// # 已知边界（如实标注）
///
/// 判据 2 是**源码扫描**：它证明「构造函数声明的字段集合」与「落盘键集合」
/// 一致，不证明 fromJson 真的读了每一个键——后者由判据 3 的逐键往返覆盖。
/// 两者合起来才等于「15 键闭环」。
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/background_item.dart';
import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/settings/display_prefs.dart';

import 'support/dart_library.dart';

/// 落盘契约：**15 键全集**（toJson 的键 == 构造函数的 15 个字段）。
///
/// 加字段时**必须**同时改这里、[kPersistedKeyCount] 与 [nonDefaultPrefs]，
/// 并且把它写进 toJson / fromJson / copyWith——漏一个就红。
const Set<String> kPersistedKeys = <String>{
  'theme',
  'backgrounds',
  'backgroundOpacity',
  'backgroundEnabled',
  'slideInterval',
  'slideRandom',
  'uiTransparency',
  'scale',
  'mouthSensitivity',
  'lipSync',
  'idleEnabled',
  'muted',
  'volume',
  'allowDragZoom',
  'tier',
};

/// 键总数（与 [kPersistedKeys] 分开写，是为了让「改了一个忘了另一个」也红）。
const int kPersistedKeyCount = 15;

/// 2026-10-07 外观卡片简化时**从落盘面删除**的键。
///
/// 它们读入时忽略、写回时不再产生；列在这里是为了让「长回来」当场判红。
const Set<String> kRemovedKeys = <String>{
  'edgeStrength',
  'backgroundSource',
  'stageImage',
  'stagePlaylist',
  'imageFit',
  'imageAlign',
  'tileSize',
  'backgroundBlur',
  'backgroundScrim',
  'shellImage',
  'syncShellStageBg',
};

/// 一张足够小的合法 dataURL（落盘长度远低于 kStageImageMaxChars）。
const String kTinyImage = 'data:image/png;base64,iVBORw0KGgo=';

/// 每个键的「非默认值」样本：一份只把那一个字段改成非默认值的偏好。
///
/// 表驱动（而不是 15 条手写 test）的理由：**加第 16 个字段时，
/// 忘记加样本会比忘记写测试更早暴露**——下面的「表覆盖全集」断言会红。
final Map<String, DisplayPrefs> nonDefaultPrefs = <String, DisplayPrefs>{
  'theme': const DisplayPrefs(theme: AppThemeId.white),
  'backgrounds': const DisplayPrefs(
    backgrounds: <BackgroundItem>[BackgroundImage(id: 'bg-1')],
  ),
  'backgroundOpacity': const DisplayPrefs(backgroundOpacity: 0.42),
  'backgroundEnabled': const DisplayPrefs(backgroundEnabled: false),
  'slideInterval': const DisplayPrefs(slideInterval: 30),
  'slideRandom': const DisplayPrefs(slideRandom: true),
  'uiTransparency': const DisplayPrefs(uiTransparency: 0.2),
  'scale': const DisplayPrefs(scale: 1.3),
  'mouthSensitivity': const DisplayPrefs(mouthSensitivity: 2.0),
  'lipSync': const DisplayPrefs(lipSync: false),
  'idleEnabled': const DisplayPrefs(idleEnabled: false),
  'muted': const DisplayPrefs(muted: true),
  'volume': const DisplayPrefs(volume: 0.3),
  'allowDragZoom': const DisplayPrefs(allowDragZoom: false),
  'tier': const DisplayPrefs(tier: 16384),
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
  group('落盘 15 键全集（E12）', () {
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
        reason:
            '构造函数字段集合与 toJson 键集合必须 1:1：字段没有键 = 改了不落盘；'
            '键没有字段 = 读回来没处放。',
      );
    });

    test('非默认值样本表覆盖全部 15 键（加字段忘了加样本也红）', () {
      expect(nonDefaultPrefs.keys.toSet(), kPersistedKeys);
      expect(nonDefaultPrefs, hasLength(kPersistedKeyCount));
    });

    for (final String key in kPersistedKeys) {
      test('键 $key 落到 JSON 且能原样读回（其余 14 键不变）', () {
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

    test('已删的旧键读入时被忽略（不再有字段可接）', () {
      final Map<String, Object?> json = <String, Object?>{
        for (final String dead in kRemovedKeys) dead: 7,
      };
      // shellImage / stageImage 有迁移语义（下面两组单独钉），这里说的是
      // 其余键**不再影响任何字段**。
      for (final String migrated in <String>['shellImage', 'stageImage']) {
        json.remove(migrated);
      }
      expect(DisplayPrefs.fromJson(json), const DisplayPrefs());
    });
  });

  group('旧存档迁移：只发生一次，写回不再产生旧键', () {
    test('没有 backgrounds 列表时，shellImage 迁成一项', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'shellImage': kTinyImage,
      });
      expect(
        p.backgrounds,
        hasLength(1),
        reason: 'rc.5 的单张壳背景图必须迁进背景库',
      );
      expect(
        p.backgrounds.single,
        BackgroundImage(id: backgroundIdOf(kTinyImage), dataUrl: kTinyImage),
      );
    });

    test('backgrounds 已是列表（含空列表）时**不读** shellImage', () {
      final DisplayPrefs emptyList = DisplayPrefs.fromJson(<String, Object?>{
        'backgrounds': <Object?>[],
        'shellImage': kTinyImage,
      });
      expect(
        emptyList.backgrounds,
        isEmpty,
        reason: '空列表 = 用户明确清空过背景库，不该被旧单张图复活',
      );

      final DisplayPrefs withItem = DisplayPrefs.fromJson(<String, Object?>{
        'backgrounds': <Object?>[
          <String, Object?>{'kind': 'image', 'id': 'bg-x'},
        ],
        'shellImage': kTinyImage,
      });
      expect(withItem.backgrounds, hasLength(1));
      expect(withItem.backgrounds.single, const BackgroundImage(id: 'bg-x'));
    });

    test('空库 + 合法 stageImage data URL → 收成库里的第一项，且舞台投影就是它', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'stageImage': kTinyImage,
      });
      expect(p.backgrounds, hasLength(1));
      expect(p.backgrounds.single, isA<BackgroundImage>());
      expect((p.backgrounds.single as BackgroundImage).dataUrl, kTinyImage);
      expect(
        DisplayPrefs.stageProjectionUrl(p, 0),
        kTinyImage,
        reason: '迁移进来的那一张必须就是投影到舞台的那一张',
      );
    });

    test('库已有一项时，JSON 里的 stageImage **不插入**', () {
      final DisplayPrefs p = DisplayPrefs.fromJson(<String, Object?>{
        'backgrounds': <Object?>[
          <String, Object?>{'kind': 'image', 'dataUrl': kTinyImage},
        ],
        'stageImage': 'data:image/png;base64,OTHER',
      });
      expect(p.backgrounds, hasLength(1), reason: 'stageImage 只在空库时收编');
      expect(
        DisplayPrefs.stageProjectionUrl(p, 0),
        kTinyImage,
        reason: '库里的那一项优先，旧 stageImage 不参与投影',
      );
    });

    test('超限 / 空串 / 非 data URL 的 stageImage 都不收（边界是闭区间）', () {
      final String okAtLimit =
          'data:image/png;base64,${'A' * (kStageImageMaxChars - 22)}';
      expect(
        okAtLimit.length,
        kStageImageMaxChars,
        reason: '测试夹具本身要落在边界上，否则下面两条不是边界断言',
      );
      expect(
        DisplayPrefs.fromJson(<String, Object?>{
          'stageImage': okAtLimit,
        }).backgrounds,
        hasLength(1),
        reason: '刚好等于 kStageImageMaxChars 是合法的（长度判据是 >）',
      );

      for (final Object? bad in <Object?>[
        '${okAtLimit}A',
        '',
        3,
        null,
        'not a data url',
        'data:image/png;base64',
      ]) {
        expect(
          DisplayPrefs.fromJson(<String, Object?>{
            'stageImage': bad,
          }).backgrounds,
          isEmpty,
          reason: '不该把 $bad 收进背景库',
        );
      }
    });

    test('写回**不再**产生旧键（一次性迁移，不是两套真相并存）', () {
      final Map<String, Object?> json = const DisplayPrefs(
        backgrounds: <BackgroundItem>[
          BackgroundImage(id: 'a', dataUrl: kTinyImage),
        ],
      ).toJson();
      for (final String dead in kRemovedKeys) {
        expect(
          json.containsKey(dead),
          isFalse,
          reason: '$dead 已从落盘面删除：写回时不许再产生它',
        );
      }
      expect(json.keys.toSet(), kPersistedKeys);
    });
  });
}
