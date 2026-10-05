# BATCH-0240 · ⭐ `ObserverBuffer` **有界**，且**上界在界面上可见** ⇒ 我承诺的那项检查通过

Phase 1 · 域覆盖 · 前端 `.dart`（49/218）—— `director_observer_section.dart` 的缓冲区上界

## 跑的命令（全部只读）
```
grep -nE "class ObserverBuffer|removeRange|sublist|length > |kMax|cap|上限|有界" director_observer_section.dart
sed -n '133,142p' director_observer_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**两处超出「有界」本身的性质**
| 处 | 内容 |
|---|---|
| `:19` | 「B 事件流 ……（**环形缓冲上限 200**，按 type 过滤）」 |
| `:75` | 「B 栏的**环形缓冲**（上限 `kObserverBufferCapacity` = **200**）」 |
| `:136-139` | `void _trim<T>(List<T> list) { while (list.length > kObserverBufferCapacity) { list.removeAt(0); } }` |
| `:464` | 「共 ${buffer.length} 条 / **上限 $kObserverBufferCapacity**」 |

三个可核点：
1. ⭐ **`_trim<T>` 是泛型 helper** ⇒ **一个实现约束四个栏**（P2）
   ⇒ ⇒ **若将来加第五栏，它自动受约束** ⇒ ⇒ **P2 用在「资源上界」上**
   （⇒ 这是 **P2 的第三次新落点**：一致性（B0215）· **安全属性**（B0236）· **资源上界**（本批））
2. ⭐ **上界在界面上可见**（`:464`）⇒ ⇒ **触顶这件事对用户显形**，不是静默截断
   ⇒ 与 B0120 的 body 片段 `+…`、B0227 的「不假装成功」**同族**
3. ⇒ 而这**补齐了 B0132 那一族的两种正确处置**：
   **列表 → 虚拟化**（B0227 核过 `ListView.builder` + `itemExtent`）· **缓冲 → 上界 + 显形**（本批）

## 未核实项
1. `director_observer_section.dart` 余 ~600 行未读（四栏呈现本体 · 状态读取 · 错误呈现）
2. `message_bubble.dart` 余 ~490 · `chat_panel.dart` 余 540 · `error_banner.dart` 余 45 ·
   `persona_panel` 余 ~560 · `memory_panel` 余 ~590 · `live2d_stage` 余 ~620 未读
3. 前端 `.dart` 仍 169 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
4. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
5. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
6. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
