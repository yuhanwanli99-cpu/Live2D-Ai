import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/state/ui_phase.dart';

/// 构造信号（只写关心的那一位，其余保持默认 = 全 false）。
UiSignals sig({
  bool ws = false,
  bool turn = false,
  bool voice = false,
  bool interrupted = false,
  bool error = false,
}) => UiSignals(
  wsConnected: ws,
  turnActive: turn,
  voiceActive: voice,
  interrupted: interrupted,
  errorActive: error,
);

void main() {
  group('判定顺序：顺序本身就是契约（表驱动）', () {
    // 每一行都是「这条优先级真的生效了吗」的证据。
    final List<(String, UiSignals, UiPhase)> cases = <(String, UiSignals, UiPhase)>[
      // 注意「全空」**不是** idle：没有连接就是 offline。
      // 这一条曾经被我写反过——它正是「表驱动测试」的价值。
      ('全空（未连上）→ offline', sig(), UiPhase.offline),
      ('连上了、空闲 → idle', sig(ws: true), UiPhase.idle),

      // error 必须盖住一切。
      ('error 盖住 thinking', sig(ws: true, turn: true, error: true), UiPhase.error),
      ('error 盖住 speaking', sig(ws: true, voice: true, error: true), UiPhase.error),
      ('error 盖住 interrupted', sig(ws: true, interrupted: true, error: true), UiPhase.error),
      ('error 优先于 offline', sig(error: true), UiPhase.error),

      // **offline 的唯一判据是「WS 不可用」**（2026-09-11 修）。
      // 渲染面的失败不再参与相位派生：它有自己的舞台覆盖层，
      // 而且旧信号只置位不清除，会永久钉住胶囊（详见 ui_phase.dart 头注）。
      ('未连上 → offline', sig(turn: true), UiPhase.offline),
      ('未连上、还在说话 → 仍是 offline（连不上优先）', sig(voice: true),
          UiPhase.offline),
      ('连上了 → 即使本轮还开着也是 thinking（不是 offline）',
          sig(ws: true, turn: true), UiPhase.thinking),

      // interrupted 必须先于 speaking（最容易写反的一处）。
      ('打断中、voiceActive 还残留 → interrupted（不是 speaking）',
          sig(ws: true, interrupted: true, voice: true), UiPhase.interrupted),
      ('打断中、本轮还开着 → interrupted', sig(ws: true, interrupted: true, turn: true),
          UiPhase.interrupted),

      // voice 先于 turn。
      ('说话中且本轮未收口 → speaking', sig(ws: true, voice: true, turn: true),
          UiPhase.speaking),
      ('只受理、未出声 → thinking', sig(ws: true, turn: true), UiPhase.thinking),
    ];

    for (final (String name, UiSignals signals, UiPhase expected) in cases) {
      test(name, () => expect(deriveUiPhase(signals), expected));
    }
  });

  group('每个相位都可达（没有死枚举值）', () {
    test('6 个相位都能被某组信号派生出来', () {
      final Set<UiPhase> reachable = <UiPhase>{
        deriveUiPhase(sig()),
        deriveUiPhase(sig(ws: true)),
        deriveUiPhase(sig(ws: true, turn: true)),
        deriveUiPhase(sig(ws: true, voice: true)),
        deriveUiPhase(sig(ws: true, interrupted: true)),
        deriveUiPhase(sig(error: true)),
      };
      expect(reachable, UiPhase.values.toSet());
    });

    test('没有 listening（本项目没有输入侧，规格 §6.7）', () {
      expect(
        UiPhase.values.map((UiPhase p) => p.name),
        isNot(contains('listening')),
      );
    });
  });

  group('视觉通道：每个相位至少占「色/形/字」里的两个', () {
    test('标签两两不同（字通道必须可区分）', () {
      final Set<String> labels =
          UiPhase.values.map((UiPhase p) => p.label).toSet();
      expect(labels, hasLength(UiPhase.values.length));
    });

    test('标签非空', () {
      for (final UiPhase phase in UiPhase.values) {
        expect(phase.label.trim(), isNotEmpty, reason: phase.name);
      }
    });
  });

  group('UiSignals.copyWith 与默认值', () {
    test('默认全 false（不假设任何东西是活的）', () {
      const UiSignals s = UiSignals();
      expect(s.wsConnected, isFalse);
      expect(s.turnActive, isFalse);
      expect(s.voiceActive, isFalse);
      expect(s.interrupted, isFalse);
      expect(s.errorActive, isFalse);
    });

    test('copyWith 只改给定字段', () {
      const UiSignals s = UiSignals(wsConnected: true, turnActive: true);
      final UiSignals next = s.copyWith(turnActive: false);
      expect(next.wsConnected, isTrue);
      expect(next.turnActive, isFalse);
      expect(next.voiceActive, isFalse);
    });

    test('copyWith 能把字段显式设回 false（不是「没给就不改」的陷阱）', () {
      const UiSignals s = UiSignals(errorActive: true);
      expect(s.copyWith(errorActive: false).errorActive, isFalse);
    });
  });

  group('打断窗口是常数（不是魔法数字散在各处）', () {
    test('kInterruptedHold 是个具体值', () {
      expect(kInterruptedHold, const Duration(milliseconds: 1200));
    });
  });
}
