/// P0-3 回归：拖动排序的**结果下标语义**。
///
/// `ReorderableListView.onReorderItem` 交来的 `newIndex` 已经为「移除被拖那一项」
/// 调整过（SDK `material/reorderable_list.dart` 明文），所以
/// `shell_prefs` 侧**不能再减 1**。老实现多减一次：
/// 向下拖早一格、**向下拖一格完全没反应**。
library;

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/data/background_reorder.dart';
import 'package:live2d_ai_shell/design/background_item.dart';

const List<BackgroundItem> _abc = <BackgroundItem>[
  BackgroundImage(id: 'A'),
  BackgroundImage(id: 'B'),
  BackgroundImage(id: 'C'),
];

List<String> _ids(List<BackgroundItem> items) =>
    <String>[for (final BackgroundItem i in items) (i as BackgroundImage).id];

void main() {
  group('onReorderItem 的 newIndex 已是结果下标（不再减 1）', () {
    test('[A,B,C] 向下拖一格（0→1）→ [B,A,C]（老实现是 no-op）', () {
      expect(_ids(reorderBackgroundItems(_abc, 0, 1)), <String>['B', 'A', 'C']);
    });

    test('[A,B,C] 向上拖一格（1→0）→ [B,A,C]', () {
      expect(_ids(reorderBackgroundItems(_abc, 1, 0)), <String>['B', 'A', 'C']);
    });

    test('[A,B,C] 从首拖到末（0→2）→ [B,C,A]', () {
      expect(_ids(reorderBackgroundItems(_abc, 0, 2)), <String>['B', 'C', 'A']);
    });

    test('原地（0→0）返回**同一个实例**（调用方据此跳过写盘）', () {
      expect(identical(reorderBackgroundItems(_abc, 0, 0), _abc), isTrue);
    });

    test('越界 oldIndex 原样返回（事件下标可能来自上一次重建）', () {
      expect(identical(reorderBackgroundItems(_abc, 3, 0), _abc), isTrue);
      expect(identical(reorderBackgroundItems(_abc, -1, 0), _abc), isTrue);
    });
  });

  group('P0-3 回归点：二次调整必须从 `shell_prefs` 里消失', () {
    test('不再出现 `if (target > oldIndex) target -= 1;`，且改为调纯函数', () async {
      final String src = await File('lib/app/shell_prefs.dart').readAsString();
      expect(
        src,
        isNot(contains('if (target > oldIndex) target -= 1;')),
        reason: 'onReorderItem 已调整过 newIndex，再减一次就是双重调整',
      );
      expect(
        src,
        contains('reorderBackgroundItems('),
        reason: '排序算术必须走可单测的纯函数，避免又一次「理解错却没回归」',
      );
    });
  });
}
