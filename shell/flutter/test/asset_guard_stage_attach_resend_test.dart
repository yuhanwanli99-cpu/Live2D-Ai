/// A1 资产守护网（A2/A6 接触点）：**iframe 重建后的自愈补发**。
///
/// # 被守的缺陷（rc.4 实测修复，2026-09-14；重放最易覆盖掉的就是这几行）
///
/// 「Win 侧换背景图后舞台仍纯黑」的根因之一：stage-bg 是**独立于 sync 的
/// 通道**，旧实现只在「偏好变化」时发一次 ⇒ iframe 还没就绪 / 错误后重建时
/// 那张图**永久丢掉**。修法是 _attach **每次挂桥都补发一次**
/// （lib/live2d/live2d_stage.dart:355-356）。同类纪律还有 sync 的 stageColor
/// 与 actionScales —— 重建后旧帧随旧桥销毁，必须重发（:347-354）。
///
/// # 为什么需要「新的」断言（已存在 vs 新增）
///
/// test/live2d_bridge_test.dart:92 已有**行为断言**，但它测的是
/// Live2DBridge.sendStageBg 这一条（桥有没有照发）。它**测不到**
/// 「Live2DStage._attach 有没有补发」——桥永远是被动接收者，舞台不发，
/// 桥单测照样全绿。本文件补的就是这条舞台侧的自愈接线。
///
/// # 断言类型：结构（_attach 在 VM 下不可达）
///
/// _attach 是私有方法，只由 web-only 的 buildLive2DHost(onTransport:) 回调
/// （lib/live2d/live2d_stage.dart:569-573）；VM 测试解析到
/// live2d_host_stub.dart，它从不回调 onTransport（该文件 :14-18）。
/// 见 asset_guard_preset_dispatch_test.dart 末尾的「取证」用例。
///
/// # 做法：函数体级，不是全文件 contains
///
/// [methodBody] 只取 _attach 的函数体，于是「同类里别的 sendStageBg 调用点」
/// 不会冒充 _attach 的补发。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'support/source_scan.dart';

/// 剥掉注释与字符串（共享词法器：`support/source_scan.dart`，W3-D3 起唯一定义）。
/// 取 [signature] 起那个方法的**函数体**源码（含最外层花括号）；找不到返回 null。
String? methodBody(String src, String signature) {
  final int start = src.indexOf(signature);
  if (start < 0) return null;
  final int open = src.indexOf('(', start);
  if (open < 0) return null;
  int depth = 0;
  int i = open;
  while (i < src.length) {
    final String c = src[i];
    if (c == '(') {
      depth++;
    } else if (c == ')') {
      depth--;
      if (depth == 0) {
        i++;
        break;
      }
    }
    i++;
  }
  final int brace = src.indexOf('{', i);
  if (brace < 0) return null;
  int braces = 0;
  int j = brace;
  while (j < src.length) {
    final String c = src[j];
    if (c == '{') {
      braces++;
    } else if (c == '}') {
      braces--;
      if (braces == 0) return src.substring(brace, j + 1);
    }
    j++;
  }
  return null;
}

void main() {
  final String stageSource = stripCommentsAndStrings(
    File('lib/live2d/live2d_stage.dart').readAsStringSync(),
  );

  test('结构：_attach 每次挂桥都补发 stage-bg（rc.4「Win 背景图丢失」的修复）', () {
    final String? body = methodBody(stageSource, 'void _attach(');
    expect(body, isNotNull, reason: '_attach 是挂桥的唯一入口，不许消失');
    expect(
      RegExp(r'\.sendStageBg\s*\(\s*widget\.stageImage\s*\)').hasMatch(body!),
      isTrue,
      reason:
          'stage-bg 必须每次 _attach 都补发（首帧 / retry 重挂 / 热更新重建）——'
          '只在偏好变化时发一次 = iframe 未就绪或重建时背景图永久丢失。'
          '类型 = 结构（_attach 在 VM 下不可达）。',
    );
  });

  test('结构：_attach 补发 sync 时带 stageColor 与 actionScales（与 stage-bg 同纪律）', () {
    final String body = methodBody(stageSource, 'void _attach(')!;
    expect(
      RegExp(r'stageColor\s*:\s*widget\.stageColor').hasMatch(body),
      isTrue,
      reason: '重建后舞台底色必须自愈（否则「主题切了但舞台没变」）',
    );
    expect(
      RegExp(r'actionScales\s*:\s*widget\.actionScales').hasMatch(body),
      isTrue,
      reason: '重建后动作幅度必须自愈（A2；否则换皮/重挂后幅度回出厂默认）',
    );
  });

  test('结构：值变化时 didUpdateWidget 也补发（stage-bg + actionScales）', () {
    final String? body = methodBody(stageSource, 'void didUpdateWidget(');
    expect(body, isNotNull, reason: 'didUpdateWidget 是「偏好变化」那条路');
    expect(
      RegExp(r'sendStageBg\s*\(\s*widget\.stageImage\s*\)').hasMatch(body!),
      isTrue,
      reason: '换图必须真的发出去（rc.4 的 Win 现场就是这一条断的）',
    );
    expect(
      RegExp(r'sendSync\s*\(\s*actionScales\s*:\s*widget\.actionScales\s*\)')
          .hasMatch(body),
      isTrue,
      reason: '幅度变化必须真的发出去',
    );
  });
}
