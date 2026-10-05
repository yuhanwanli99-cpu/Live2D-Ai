# BATCH-0292 · ✅ **B0291 那条「依赖未核」的链闭合了** —— 指向处是一句**带日期的变更记录**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `tokens.dart` 的 `AppRhythms` / `AppDurations` 定义与注释本体

## 跑的命令（全部只读）
```
grep -rln "class AppRhythms|class AppDurations" lib/
grep -nE "class AppRhythms|class AppDurations" lib/design/tokens.dart
awk '/class AppDurations/,/^}/' lib/design/tokens.dart | grep -E "///|static const"
awk '/class AppRhythms/,/^}/' lib/design/tokens.dart | grep -E "///|static const"
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **B0291 的结论在「定义层」被确认**
### ① `AppDurations` 的四档**全是过渡时长** ⇒ B0291 的推理成立
| 档 | 值 | 用途（逐字） |
|---|---|---|
| `fast` | 120 ms | hover / pressed / 开关 / 徽标切换 |
| `base` | 200 ms | 内容切换、消息渐入、横幅滑入 |
| `slow` | 320 ms | 模态 / sheet 入场 |
| `reveal` | 600 ms | 「**仅此一处**长动画：启动揭示」 |
| `registry` | — | 「**只服务测试**」 |
⇒ ⇒ **四档全是「一次过渡有多快」**，**没有一档是「瞬时状态保持多久」**
⇒ ⇒ **B0291 的推理成立**：`AppRhythms.interruptedHold` 确实属于**另一个类**
⇒ ⭐ 而 `registry`（只服务测试）⇒ ⇒ **这些 token 是可机器枚举的**
   ⇒ ⇒ **B0204 `toValuesMap()` 那形状的第二个家**

### ② ⭐⭐ 而 B0291 引的那句「见该令牌的注释」**指的就是这里**，且带日期
`AppRhythms.interruptedHold`：
> 「「**瞬时状态**」的保持时长：`已打断`、`已复制` 这类**就地确认**共用一档
> 长到人眼能读完那几个字，短到不会和下一轮重叠。
> **刻意只有一档**（**2026-09-11，P2-3**）：……两个**同值**令牌只会制造「**改一处忘一处**」的[问题]」
⇒ ⇒ ⇒ **B0291 标为「依赖未核」的链在此闭合**
⇒ ⇒ 且理由是**带日期的变更记录**（P2-3 · 2026-09-11）⇒ ⇒ **B0228 那一族（带理由的日期变更）在设计 token 上的应用**
⇒ ⇒ **「刻意只有一档」+ 理由** ⇒ ⇒ 与 B0291 的「**令牌按它度量什么分类**」**互为表里**

### ③ ⭐ 而 `tokens.dart:1-5` 交代了**这个文件为什么能被信任**
> 「设计 token：**纯逻辑、零 web 依赖**，可在 VM 上单测。
> 本文件与 `typography.dart` 是**全仓库仅有的两个允许出现裸色值/裸字号的源文件**——
> 其余业务代码一律引用这里的令牌，由 **`test/design_tokens_lint_test.dart` 扫描守住**（设计规格 §2.8.3）。」
⇒ ⇒ ⭐ **「一律引用这里的令牌」是被测试守住的，不是靠自觉** ⇒ ⇒ 与 B0204 P24 **同形**（让违规**必然失败**）
### ④ 而它的**行数豁免理由**也是可核的（`:18-21`）
> 「每套 11 个颜色字段都要带「为什么是这个值」的取值理由；**拆成四个文件会让「四套必须同步改」这件事
> 从一次编辑变成四处编辑** —— 而**漏改一处正是本项目 P4「静默失效」要治的病**。」
⇒ ⇒ ⭐ **一个结构决策由「失败模式」论证，而不是由品味** ⇒ ⇒ 而它**点名了那个失败模式的编号**

## 未核实项
1. `AppRhythms` 余下档位未读（`thinkingBreath` 之外的那些）· 两者的 `registry` 内容未读
2. `message_bubble.dart` 余约 460 行未读（角色标签 · 流式光标 · `onlyReasoning` 折叠区）
3. `live2d_stage.dart` 余约 600 · `shell_admin.dart` 余面 · `env_key_field.dart` 余约 110 未读
4. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
5. `main.rs` 余约 930 行未读
6. 真实动作计划的 token 长度（F-0020-01 永久敞口）
7. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
8. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
9. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
10. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
11. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
12. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
13. Mod crates 42 未读；`mod-system` 余 8 文件未读
14. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
15. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
16. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
