# BATCH-0086 · `BackgroundStore` 契约与两份实现 —— **0 条新发现**（本区接缝**完全关闭**）

Phase 1 · 域覆盖 · 前端 `data/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/data/background_store.dart` — 124（**全读**：三态契约 + 抽象类 + `createBackgroundStore`）
2. `shell/flutter/lib/data/background_store_web.dart` — 240（定点 41-55 / 180 / 197-200）
3. `shell/flutter/lib/data/background_store_stub.dart` — 60（定点 24 / 33 / 50）

## 跑过的命令（全部只读）
```
sed -n '40,109p' background_store.dart ; sed -n '109,124p' background_store.dart
ls shell/flutter/lib/data/ | grep -i background_store
for f in <三个实现/契约文件>; do grep -n "probe|readChecked|retainOnly" $f; done
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；「保留集 / 剪枝」这条链**四侧全部核实**
| 侧 | 位置 | 核实结果 |
|---|---|---|
| 契约 | `background_store.dart:84-102` | `readChecked` **默认实现有损但被标注**（:82-83「能在内部区分的实现（IndexedDB 那份）**应当覆写它**」）；`probe()` **默认 `false`**（:100-101「**无法证明健康 → 不剪枝**」）⇒ **默认值即安全侧** |
| web 实现 | `background_store_web.dart:180/197-200` | **两个都覆写**；`probe` 用**专用哨兵键** `const _probeKey = '__health_probe__'`（:45）做**真实**可用性探测（`s.get(_probeKey.toJS)` :200）⇒ **剪枝在 web 上确实启用** |
| stub 实现 | `background_store_stub.dart:24/33/50` | **两个都覆写**，`probe => true`（内存库读得到即健康，合理） |
| 调用侧 | B0084 / B0085 已核 | `keep` **先于 I/O** 建集 · `healthy && !storeFailed` 双闸 · `liveKeptIds` 读**此刻**偏好 |

### 我试过推翻而没推翻的（记此以免后人重提）
**假设：「web 实现可能忘了覆写 `probe` ⇒ 默认 `false` ⇒ 剪枝被永久静默关闭 ⇒
字节库无界增长（正是 `retainOnly` 被写出来要解决的问题反向发生）」** —— **证伪**：
`background_store_web.dart:197` **确实覆写**了，且用**专用哨兵键**而不是用户键探测
（用用户键探测会有副作用；用不存在的键探测则有歧义）⇒ 剪枝**正常工作**。
⇒ 若此假设成立，症状会极其隐蔽（无报错、无日志、只是几个月后磁盘被吃），
所以这条**值得记为「已排除」**，而不是「没查」。

### 顺带记一个设计细节（备忘）
**三态契约的「不精确版」被保留而不是改签名**（`background_store.dart:70-73`）：
`read` 仍返回 `String?`（分不清两种 `null`），另加 `readChecked` 提供三态，
并写明「这个签名**分不清**……要区分请用 `readChecked`」——
⇒ **不静默改语义、另给精确变体**，比直接改 `read` 的返回类型更安全（不破坏既有调用点）。

## 未核实项（本区已无「已知未核」；以下为未读清单）
1. `background_store_web.dart` 的**其余 ~190 行**未读（`put` / `delete` 的事务边界、
   `keys()`、schema 版本、迁移策略）—— 本区**唯一**还值得读的地方
2. `settings_controller.dart:120-322` / `:372-409` 未读
3. `appearance_background.dart`(1551) 未读；`app_shell.dart` 全文未读
4. `shell_admin.dart` 余段 / `main.dart` 其余 ~1000 行未读；其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**
