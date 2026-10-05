# BATCH-0189 · ⭐ **F-0189-01（P2）**：`load()` 无 staleness 守卫，而**声称守它的那组测试是顺序执行的**

Phase 1 · 域覆盖 · `shell/flutter/lib/settings/settings_controller.dart`（**实现侧** —— B0188 留的点）

## 跑的命令（全部只读）
```
grep -n "epoch|_gen|generation|seq|stale|乱序|inflight|inFlight" lib/settings/settings_controller.dart
grep -rn "\.load()" lib/
grep -rn "GET / PATCH 乱序" -A 40 test/settings_controller_test.dart
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0189-01（P2）** —— 19 批以来的第一条新发现
### ① 三个**可核**的事实
```
$ grep -n "epoch|_gen|generation|seq|stale|inflight" settings_controller.dart
（**零命中**）
$ grep -rn "\.load()" lib/
shell_settings.dart:18:  _settings.load(),
shell_settings.dart:108: await _settings.load(),
shell_settings.dart:224: onPressed: () => unawaited(_settings.load()),     ← ⭐ 按钮 + 不 await
```
⇒ **实现侧无任何 staleness 守卫**（`:266` `_remote = await api.fetchSettings();` 无条件覆盖）
⇒ **触发路径存在**：`:224` 是**按钮**且用 **`unawaited`** ⇒ **连按两次**即产生重叠
⇒ 而**声称守它的那组测试是纯顺序的**（`:579` `await c.load()` → `:583` `await c.save()`
   → `:586` `await c.load()`）⇒ **全程没有重叠** ⇒ `:587` 的断言在顺序执行下**恒真**
⇒ ⇒ **一个被写进测试名的并发不变式，既无实现守卫、也未被测试** —— 与模式 H 假绿同形，
**但方向相反：不是断言写错，是断言太弱。**

### ② 而它与本仓一次**真实事故**同域
AGENTS.md 记「手改 `live2d-ai.toml` 后 `GET /settings` 不跟随磁盘，下一次界面「保存」会把手改内容
**覆盖**掉」⇒ 那次修的是**磁盘跟随**那一半（`StatusContext::refresh_from_disk`，B0104 核过）
与**草稿丢弃**那一半（`:588` 断言钉住）⇒ **「两个 GET 乱序」这一半，两样都没有。**

### ③ 建议（三条，按成本排序）
① `load()` 入口加自增序号 `final int my = ++_loadSeq;` ⇒ `await` 后**先判 `if (my != _loadSeq) return;`**
   ⇒ 与 B0040 的 `epoch` 闸**同形**（本仓有先例），**不需要取消请求**（一行级）
② 补一条**真正制造重叠**的测试：第一次 GET **延迟**、第二次立即返回 ⇒ 有了 ① 就会红 ⇒ **测试才有牙齿**
③ `:224` 的按钮在 `_loading` 期间**禁用**（或入口早退），免得「连按两次」成为常态

### ④ ⚠ 诚实说明**为何是 P2 而非 P1**（并写明升级条件）
- **已核**：实现无守卫 · 触发路径存在 · 测试不制造重叠（三处 quote 齐）
- **未核**（故阻止升 P1）：**`save()` 的 patch 究竟由 `_draft` 还是 `_remote` 派生**
  ⇒ **若由 `_remote` 派生，则「旧响应 → 保存 → 写回旧值」成立，严重度升 P1**（**B0190 第一件事**）
- **反证 (a)**：`load()` 只**写** `_loading`（:262/:274），**入口没有「已在加载就早退」的判据**
- **反证 (b)**：`ApiClient` 内部**可能**有请求串行化（`fetchSettings()` 本体未读）
  ⇒ (a)(b) 任一成立都**应写进本条**，因为那会把修法从「加守卫」缩小到「已在别处解决、只需补测试」

### ⚠ 模式 Q 第四次拦截
本条写入时**又**只落在 `## BATCH-0189` 段内 ⇒ 由**收尾检查**抓出并补规范头。
⇒ **四次复发**（B0128 / B0144 / B0184 / 本批）⇒ 复发率约 **1/60 批**，**每次都由检查当场拦住**。

## 未核实项
1. ⭐ **`save()` 的 patch 由 `_draft` 还是 `_remote` 派生**（决定本条是否升 P1）—— **B0190 第一件事**
2. `ApiClient.fetchSettings()` 本体未读（是否内部串行化）
3. `settings_controller.dart:120-222` / `:279-409` 其余未读
4. 新露出的 208 条自我设防名只看了 11 条；`dev_tools_section.dart`(1874) / `tokens.dart`(928) 未读
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
