import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/settings/persona_card.dart';
import 'package:live2d_ai_shell/settings/persona_import.dart';

/// **真实 PNG 字节**（747 bytes）的 base64。
///
/// 不是手搓的假数据：它由 Python 的 `zlib` + `struct` 生成，**块长度与 CRC
/// 都按 PNG 规范算过**，像素数据是真实的 zlib 压缩 IDAT。
/// 里面嵌了一个 `tEXt` 块，keyword 为 `chara`，负载是
/// base64(SillyTavern **V2** 角色卡 JSON)——与酒馆导出的形态一致。
///
/// 为什么不放进 `test/fixtures/`：一个 747 字节的二进制文件塞进仓库后，
/// 没人能一眼看出它是什么；base64 至少能读、能 diff、能看出改了什么。
const String kRealCharaPngB64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAACmnRFWHRjaGFyYQBleUp6Y0dWaklq'
  'b2dJbU5vWVhKaFgyTmhjbVJmZGpJaUxDQWljM0JsWTE5MlpYSnphVzl1SWpvZ0lqSXVNQ0lzSUNK'
  'a1lYUmhJam9nZXlKdVlXMWxJam9nSWs1bGEyOGlMQ0FpWkdWelkzSnBjSFJwYjI0aU9pQWk1TGlB'
  'NVkrcTVaMlE1WnlvNXFHTTZaMmk1TGlLNTVxRTU0eXI1YWlZNzd5TTZLKzA2SytkNkwydjZMMnY1'
  'NXFFNDRDQ0lpd2dJbkJsY25OdmJtRnNhWFI1SWpvZ0l1YUZ0ZWFIa3VPQWdlZUlzZWFTa3VXb2gr'
  'T0FnZVdCdHVXd2xPYXZrdWlJakNJc0lDSnpZMlZ1WVhKcGJ5STZJQ0xubEtqbWlMZmxuS2pubExY'
  'b2hKSGxpWTNsdDZYa3ZaenZ2SXpscGJub3RyVGxuS2ptb1l6b3A1THBtYXJubllEamdJSWlMQ0Fp'
  'Wm1seWMzUmZiV1Z6SWpvZ0l1V1d0ZUtBcHVLQXB1UzlvT2U3aU9TNmp1V2JudWFkcGVXVnB1T0Fn'
  'aUlzSUNKdFpYTmZaWGhoYlhCc1pTSTZJQ0k4VTFSQlVsUStYRzU3ZTNWelpYSjlmVG9nNVp5bzVa'
  'Q1hYRzU3ZTJOb1lYSjlmVG9nNVp5bzU1cUU1WmExNzcyZUlpd2dJbk41YzNSbGJWOXdjbTl0Y0hR'
  'aU9pQWlJaXdnSW1OeVpXRjBiM0pmYm05MFpYTWlPaUFpNXJXTDZLK1Y1WTJoSWl3Z0luUmhaM01p'
  'T2lCYkl1ZU1xK1dvbUNJc0lDTG1vWXpwbmFMbHJxRG5pYWtpWFN3Z0ltTm9ZWEpoWTNSbGNsOTJa'
  'WEp6YVc5dUlqb2dJakV1TUNKOWZRPT3M3ljxAAAADElEQVR42mP4z8AAAAMBAQD3A0FDAAAAAElF'
  'TkSuQmCC';

Uint8List realCharaPng() => base64.decode(kRealCharaPngB64);

/// V1 扁平卡（酒馆 2023 前的老格式）。
const String kV1Json = '''
{"name":"Inu","description":"一只柴犬","personality":"热情","scenario":"公园",
"first_mes":"汪！","mes_example":"x","unknown_field_xyz":"keep me visible"}
''';

void main() {
  group('PNG 内嵌卡（真实字节）', () {
    test('从真实 PNG 里解出 V2 卡，五项人设都对', () {
      final PersonaImportResult r = importPersonaBytes(realCharaPng());
      expect(r.ok, isTrue, reason: r.error);
      expect(r.source, 'png');
      final PersonaCard c = r.card!;
      expect(c.format, 'v2');
      expect(c.name, 'Neko');
      expect(c.description, '一只坐在桌面上的猫娘，说话软软的。');
      expect(c.personality, '慵懒、爱撒娇、偶尔毒舌');
      expect(c.scenario, '用户在电脑前工作，她趴在桌角陪着。');
      expect(c.first, '喵……你终于回来啦。');
      expect(c.filledCount, 5);
      expect(c.isEmpty, isFalse);
    });

    test('未映射的字段如实列出来（不悄悄丢掉）', () {
      final PersonaCard c = importPersonaBytes(realCharaPng()).card!;
      // `mes_example`（示例对话）本项目没有对应字段。
      expect(c.unmapped, contains('mes_example'));
      expect(c.unmapped, contains('character_version'));
      expect(c.unmapped, isNot(contains('first_mes')), reason: '已映射的不该出现');
      expect(c.unmapped, isNot(contains('name')));
      // 排序过：diff 稳定。
      final List<String> sorted = List<String>.of(c.unmapped)..sort();
      expect(c.unmapped, sorted);
    });

    test('标签读出来（只展示，不写进设置）', () {
      final PersonaCard c = importPersonaBytes(realCharaPng()).card!;
      expect(c.tags, <String>['猫娘', '桌面宠物']);
    });

    test('不是 PNG / 没有 chara 块 → 明确报错，不返回半个卡', () {
      // 真 PNG 但没有 tEXt chara：把 chara 块的数据换成别的关键字。
      final Uint8List png = realCharaPng();
      final String raw = latin1.decode(png);
      expect(raw.contains('chara'), isTrue);
      final Uint8List other = Uint8List.fromList(png);
      // `chara` 出现在 tEXt 数据区：改成同长度的 `xxxxx`。
      final int at = raw.indexOf('chara');
      for (int i = 0; i < 5; i++) {
        other[at + i] = 'x'.codeUnitAt(0);
      }
      final PersonaImportResult r = importPersonaBytes(other);
      expect(r.ok, isFalse);
      expect(r.error, contains('没有角色卡'));
    });

    test('截断的 PNG → 报「数据不完整」而不是崩', () {
      final Uint8List png = realCharaPng();
      final Uint8List cut = Uint8List.sublistView(png, 0, 30);
      final PersonaImportResult r = importPersonaBytes(cut);
      expect(r.ok, isFalse);
      expect(r.error, isNotNull);
    });

    test('base64 损坏 → 报错而不是抛', () {
      final Uint8List png = realCharaPng();
      final String raw = latin1.decode(png);
      final int at = raw.indexOf('chara') + 6; // 数据区开头
      final Uint8List broken = Uint8List.fromList(png);
      for (int i = 0; i < 8; i++) {
        broken[at + i] = 0x21; // '!'
      }
      final PersonaImportResult r = importPersonaBytes(broken);
      expect(r.ok, isFalse);
      expect(r.error, isNotNull);
    });
  });

  group('JSON 卡', () {
    test('V1 扁平卡', () {
      final PersonaImportResult r = importPersonaFromJsonText(kV1Json);
      expect(r.ok, isTrue, reason: r.error);
      expect(r.source, 'json');
      expect(r.card!.format, 'v1');
      expect(r.card!.name, 'Inu');
      expect(r.card!.first, '汪！');
      expect(r.card!.unmapped, contains('unknown_field_xyz'));
    });

    test('V2 卡（spec 字段齐全）', () {
      const String v2 = '''
{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"Neko",
"description":"猫","personality":"懒","scenario":"桌面","first_mes":"喵",
"system_prompt":"你是猫","tags":["a"]}}
''';
      final PersonaCard c = importPersonaFromJsonText(v2).card!;
      expect(c.format, 'v2');
      expect(c.name, 'Neko');
      expect(c.systemPrompt, '你是猫');
      expect(c.unmapped, isNot(contains('spec')), reason: 'spec 是包装层，不在 data 里');
    });

    test('V2 但 spec 字段缺失/写错 → 靠 data 里有人设字段认出来', () {
      const String weird = '''
{"spec":"chara_card_v2_typo","data":{"name":"X","first_mes":"嗨"}}
''';
      final PersonaCard c = importPersonaFromJsonText(weird).card!;
      expect(c.format, 'v2', reason: 'data 里有 first_mes 就该按 V2 解析');
      expect(c.name, 'X');
      expect(c.first, '嗨');
    });

    test('缺 data 但也没有人设字段 → 当 V1 空卡（返回 null）', () {
      expect(parsePersonaCardJson('{"foo":1}'), isNull);
      expect(parsePersonaCardJson('{}'), isNull);
    });

    test('坏 JSON / 非对象 → null，不抛', () {
      expect(parsePersonaCardJson('{not json'), isNull);
      expect(parsePersonaCardJson('[1,2,3]'), isNull);
      expect(parsePersonaCardJson(''), isNull);
      expect(parsePersonaCardJson('"a string"'), isNull);
    });

    test('字段类型不对（数字/对象）→ 当空串，不崩', () {
      const String weird = '''
{"name":123,"description":{"a":1},"first_mes":null,"personality":"正常"}
''';
      final PersonaCard c = parsePersonaCardJson(weird)!;
      expect(c.name, '');
      expect(c.description, '');
      expect(c.first, '');
      expect(c.personality, '正常');
    });
  });

  group('自动判别：用户不用先想清楚「这是哪种卡」', () {
    test('PNG 走 PNG 路径', () {
      final PersonaImportResult r = importPersonaBytes(realCharaPng());
      expect(r.source, 'png');
      expect(r.ok, isTrue);
    });

    test('JSON 文本走 JSON 路径', () {
      final PersonaImportResult r = importPersonaBytes(
        Uint8List.fromList(utf8.encode(kV1Json)),
      );
      expect(r.source, 'json');
      expect(r.ok, isTrue);
    });

    test('二进制垃圾 → 既不是 PNG 也不是 UTF-8，明确报错', () {
      final PersonaImportResult r = importPersonaBytes(
        Uint8List.fromList(<int>[0xFF, 0xFE, 0x00, 0x01]),
      );
      expect(r.ok, isFalse);
      expect(r.error, contains('既不是 PNG 也不是'));
    });

    test('空文件 → 报「不是可识别的角色卡」', () {
      final PersonaImportResult r = importPersonaBytes(Uint8List(0));
      expect(r.ok, isFalse);
      expect(r.error, isNotNull);
    });
  });
}
