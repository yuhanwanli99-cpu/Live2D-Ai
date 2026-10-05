# BATCH-0191 · ⭐ **反证 (b) 被推翻**（客户端无串行化）；顺带核到一处**带实测记录的协议事实**

Phase 1 · 域覆盖 · `shell/flutter/lib/api/api_client.dart`

## 跑的命令（全部只读）
```
grep -rn "fetchSettings" -A 12 lib/api/*.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**F-0189-01 的反证 (b) 被推翻**（危害范围维持）
```dart
Future<SettingsView> fetchSettings() async {                     // :177
  final response = await _guard(() => _client.get(_uri('/api/v1/settings')));   // :178-180
  if (response.statusCode != 200) throw _errorFrom(response);
  return SettingsView.fromJson(_decodeObject(response.body));    // :182
}
```
⇒ **无队列、无序号、无 in-flight 判据** ⇒ **客户端层没有消解这个竞态**
⇒ ⇒ **F-0189-01 的范围维持**：两个重叠 `load()` 仍会**乱序到达**，后者覆盖前者并**丢弃草稿**
⇒ **反证 (a) 仍开着**：`load()` 只**写** `_loading`、入口无早退
  ⇒ **未核**调用方（`shell_settings.dart:224`）是否在 `_loading` 期间禁用该按钮。

### 顺带一处**正面样本**：协议事实**带实测记录**（同文件 :185-188）
> 「**是 PATCH 不是 PUT**：服务端 `match_route` **只认 `Method::Patch`**，
> 用 `PUT` 会 **404**（**实测** `PUT→404` / `PATCH→200`）。」

⇒ 三个要素齐备：**① 用哪个动词** · **② 为什么**（服务端只认 Patch）· **③ 实测证据**
⇒ ⭐ 这正是正面模式 **P1（理由要写「用户会看到什么」）** 的强化版：
**不只说「会 404」，还把两次观测的结果记下来** ⇒ 日后有人「顺手统一成 PUT」时，
**注释本身就是反证**。
⇒ 而它防的正是我这类审计最常遇到的**协议漂移**（前端与服务端对同一个端点的方法认知不一致）。

## 未核实项
1. ⭐ 反证 (a)：`shell_settings.dart:224` 的按钮在 `_loading` 期间**是否被禁用**（未读该处上下文）
2. `_guard()` 的实现未读（它是否已含超时/重试/去重 ⇒ 属反证 (b) 的延伸）
3. `settings_controller.dart:120-222` / `:296-409` 其余未读；`:327` 那处置 `_draft` 的上下文未读
4. 新露出的 208 条自我设防名只看了 11 条；`dev_tools_section.dart`(1874) / `tokens.dart`(928) 未读
5. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
