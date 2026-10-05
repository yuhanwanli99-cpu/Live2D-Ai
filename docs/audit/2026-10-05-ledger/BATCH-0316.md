# BATCH-0316 · ✅ `_testEndpoint` 结清；而它**顺带收窄了我一个挂了很久的未核实项**

Phase 1 · **未核实项清账（第 4 批）**

## 跑的命令（全部只读）
```
grep -rn "class ActionCue" -A 10 shell/flutter/lib/live2d/render_events.dart     # ⇒ 零命中
sed -n '215,228p' shell/flutter/lib/api/api_client.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；✅ **一条结清、一条仍开着**
| 未核实项 | 结果 |
|---|---|
| `api_client.dart:217-226` 的 `_testEndpoint` | ✅ 见 ① |
| `ActionCue` 定义 | ⚠ **不在 `render_events.dart`**（零命中）⇒ **本批未取**，不据此推断 |

### ① `_testEndpoint` = 「服务不可达」是**答案**、不是**异常**（第四层）
```dart
/// 「**连不上你的端点**」当作**一次成功的自检结果**，所以 `ok` 为假时**不抛** ——
/// 由调用方读 [SettingsTestOutcome.ok] 与错误详情展示。**只有 HTTP 层失败才抛**。   // :215-216
if (response.statusCode != 200) throw _errorFrom(response);            // :224
return SettingsTestOutcome.fromJson(_decodeObject(response.body));    // :225
```
四个可核点：
1. ⭐⭐ **「连不上」被当成一次成功的自检** ⇒ ⇒ **传输成功、而「答案」是否**
   ⇒ ⇒ **与 B0302（存档失败 vs 本次会话）、B0288（写成功但读状态失败）同族
   ⇒ 「服务不可达」不是异常，是答案** —— **第四层**
2. ⭐ **`ok=false` 不抛** ⇒ ⇒ **由调用方读** ⇒ ⇒ **而调用方（`main.dart:1046`）确实这么读**（B0294 已核）
   ⇒ ⇒ **契约两端都兑现**
3. ⭐ **只有 HTTP 层失败才抛** ⇒ ⇒ **两条失败通道被分开点名** ⇒ ⇒ B0284 家族
4. ⭐⭐ **`body: {'timeout_ms': ?timeoutMs}` —— 超时是传给服务端的**

### ② ⭐ 而第 4 点**顺带收窄了我一个挂了很久的未核实项**
我在 **B0229** 记过：`api_client._guard()` 只有 5 行、**无超时/重试/去重/串行**，
并把它留成长期未核实项。
**本批查到：这条路径上的超时是**委托给服务端的**（客户端只转发「请用多少预算去探」）**
⇒ ⇒ 与 AGENTS「**自检只测连通**……本机合成一次 2.4s 而自检默认只等 3s」那条裁决**完全吻合**
⇒ ⇒ ⭐ **所以「`_guard()` 没有客户端超时」在自检这条路上不是缺陷，是刻意的委托**
⇒ ⇒ 而 AGENTS 那条裁决的**另一半**（自检会与产品链路**抢资源**，故「自检说谎」比没有更坏）
**仍然是本仓认可的取舍**（B0000 段已记）⇒ ⇒ **我这条未核实项可以从「疑点」降级为「已定的分工」**

## 未核实项（本批后仍开着 9 条）
1. `ActionCue` 定义（**不在 `render_events.dart`**，本批零命中 ⇒ 按 B0274 规则**不据此推断**）
2. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ApiClient.setActiveSession` 的说明
3. `live2d_bridge.dart` 余面（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+`
4. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
5. `main.rs` 余约 930 行
6. `requestAnimationFrame` 的**暂停语义**
7. 面板余面（`dev_tools_section` ~1845 / `live2d_stage` ~600 / `memory_panel` ~545 /
   `persona_panel` ~520 / `message_bubble` ~460 / `chat_panel` ~465）
8. **`reqwest` `.timeout()` 是否覆盖 body 读取** —— **永久不可核**（禁止运行）
9. **动作计划的 token 长度** —— **仓库里拿不到**；
   `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」是否有别处 ·
   GitHub 侧 secret scanning（**仓库设置，不在代码里**）
