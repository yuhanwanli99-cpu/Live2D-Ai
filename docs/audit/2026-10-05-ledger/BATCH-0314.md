# BATCH-0314 · ✅ **四条未核实项结清**；而两条注释各带一个**我没预期的**东西

Phase 1 · **未核实项清账（第 2 批）**

## 跑的命令（全部只读）
```
sed -n '121,124p' shell/flutter/lib/chat/turn_liveness.dart
sed -n '/void _trimSessions/,/^  }/p' shell/flutter/lib/chat/chat_session.dart
grep -rn "pub struct ErrorResponse" -A 6 crates/live2d-ai-desktop/src/web_api/dto.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；✅ **四条结清**
| 未核实项 | 结果 |
|---|---|
| `kReasoningOnlyCaption` 的值 | ✅ 见 ① |
| `_trimSessions` 裁剪体 | ✅ 见 ② |
| `ErrorResponse` 定义 | ✅ **只有一个字段** `error: ErrorDetail`（`dto.rs:150-153`）⇒ B0258/B0257/B0256 那条链**闭合** |
| `settleTurn` 分支顺序 | ⚠ **仍未取**（本批只取了尾部） |

### ① ⭐⭐ 而 `kReasoningOnlyCaption` **给出了可执行的出路**
> `'本轮只有思考、没有正文（**正文可能被「输出 token 上限」截断；`
> `'可在「设置 → 对话模型」把它调大**）'`
⇒ ⇒ **它把「为什么」说成了「你可以做什么」**，且**指名了那个设置页与那个字段**
⇒ ⇒ ⭐ 而这与 B0302 的「原因留给诊断日志」是**两种分工**：
`kWordlessTurnNotice`（**没有原因可给** ⇒ 只报事实）·
`kReasoningOnlyCaption`（**原因已知且可调** ⇒ 给出路）
⇒ ⇒ ⇒ **「只报事实」不是一刀切，而是「没有可执行的下一步时才只报事实」**
⇒ ⇒ 且**这一条恰好是 F-0020-01 那个被撤回的 P1 的正解形态**：
我当初说「表演层 `MAX_TOKENS=1024` 可能太小」，而**这里的主链侧**早就把
「思考挤掉正文」做成了**一条用户可自行纠正的提示** ⇒ ⇒ **同一现象，两侧两种处置，且都被写下来了**

### ② ⭐⭐ 而 `_trimSessions` 的注释记着**一次兜底反被咬的教训**
> 「丢最久没用过的；**绝不丢当前选中的那个**（否则用户正看着它消失）
> 这里刻意**不用** `firstWhere(..., orElse:)`：`orElse` 会在「找不到非当前会话」时回落到第一个
> （**也就是当前那个**），于是**兜底反而把要保的删了** ⇒ **找不到就不裁** ——
> **宁可暂时超一个上限，也不能删掉用户正在看的**」
⇒ ⇒ ⭐⭐ **又一个「记录走过的弯路」，而且记的是「兜底逻辑本身是缺陷」** ——
`orElse` 这种**看似稳妥**的写法，在「兜底目标恰好是要保护的对象」时**反而造成伤害**
⇒ ⇒ 与 B0310 记的两次错法**同族** ⇒ ⇒ **第三次出现「弯路记录」**
⇒ ⇒ 而**解法是纯结构性的**：显式循环 + 找不到就 `return` ⇒ **不裁** ⇒
**上限可以被突破，但保护对象不能被删** ⇒ ⇒ **取舍写在注释里**

## 未核实项（本批后仍开着 13 条）
1. `settleTurn` 的**完整分支顺序**（只见到尾部）
2. `ModServices.apply_settings` 本体 · `ActionCue` 定义 · `api_client.dart:217-226` 的 `_testEndpoint`
3. `settings_models.dart` 的 `fromJson` 体 · `ApiClient.setActiveSession` 的说明
4. `live2d_bridge.dart` 余面（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+`
5. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
6. `mustReleaseTurnOnWsLoss` / `settleTurn` **有没有测试**
7. `main.rs` 余约 930 行
8. `requestAnimationFrame` 的**暂停语义**
9. 面板余面（`dev_tools_section` ~1845 / `live2d_stage` ~600 / `memory_panel` ~545 /
   `persona_panel` ~520 / `message_bubble` ~460 / `chat_panel` ~465）
10. **`reqwest` `.timeout()` 是否覆盖 body 读取** —— **永久不可核**（禁止运行）
11. **动作计划的 token 长度** —— **仓库里拿不到**
12. `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」是否有别处
13. GitHub 侧 secret scanning / push protection（**仓库设置，不在代码里**）
