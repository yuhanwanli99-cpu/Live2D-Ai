# BATCH-0345 · ✅ **「逐字节等价」是真的** —— 而我第一遍**找错了目录**

Phase 1 · 域覆盖 · 门禁的**生成侧**（B0344 留：测试说「与 `scripts/font_subset_ranges.py` 逐字节等价」）

## 跑的命令（全部只读）
```
sed -n '43,58p' shell/flutter/test/font_subset_test.dart
grep -n "fnv|0x811c9dc5|…|FFFFFFFF" shell/flutter/scripts/font_subset_ranges.py   # ⇒ **零命中**
grep -n "fnv|…" scripts/font_subset_ranges.py                                      # ⇒ **命中**
sed -n '1,10p' scripts/font_subset_ranges.py
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **等价性成立，且两侧的失败模式都被写下**
**Dart 侧**（`test/font_subset_test.dart:43-50`）：
```dart
/// FNV-1a 32 位。与 `scripts/font_subset_ranges.py` 的实现**逐字节等价**。
int fnv1a32(List<int> bytes) { int h = 0x811C9DC5; for (final int b in bytes) { h ^= b;
  h = (h * 0x01000193) & 0xFFFFFFFF; } return h; }
```
**Python 侧**（`scripts/font_subset_ranges.py:46-51`）：
```python
def fnv1a32(data: bytes) -> int:  h = 0x811C9DC5 … h = (h * 0x01000193) & 0xFFFFFFFF
```
四个可核点：
1. ⭐⭐⭐ **等价性成立**：**同样的初值、同样的乘数、同样的掩码、同样的顺序**（先 `^=` 再乘）
   ⇒ ⇒ **B0344 那句「逐字节等价」经核为真**，而**核它只需要读两个函数**
2. ⭐⭐ **两侧的失败模式都被写下了**：
   Python 头注（`:4-8`）「**为什么需要它（2026-09-11，无头浏览器真机点火时抓到）**
   ……真机点火时**实测到一个外部请求**」
   Dart 头注（`:28-30`）「若有人**换了字体又忘了**重新生成覆盖表，测试就会**拿旧表放行**」
   ⇒ ⇒ **生成侧记「为什么要有这个东西」，消费侧记「它什么时候会骗人」** ⇒ ⇒ **两侧各记一半，合起来才完整**
3. ⭐⭐ **而生成器在**仓库根**的 `scripts/`，不在 `shell/flutter/scripts/`**
   ⇒ ⇒ **我第一遍找错了目录**（B0316 立的规矩：「没找到 ≠ 不存在，只等于换个人问」）
4. ⇒ ⇒ ⭐ **而这一条本身就是 B0316 那条规则的一次应用**：
   **我差点因为「零命中」而写「这个文件里没有 FNV 实现」** ⇒ ⇒ **而正确的反应是换路径**（已换，并命中）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的净效果**：**「逐字节等价」从注释变成了核过的事实**，
且**我自己在这一批又用了一次「换个人问」**（在错目录零命中 ⇒ 换路径 ⇒ 命中）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~450 / `chat_panel` ~440）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
