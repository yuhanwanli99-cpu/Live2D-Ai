# BATCH-0295 · 🔻🔻 **我 B0294 的「两个消费点同构」是错的** —— 真正的缺口只在 LLM 侧

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— F-0294-01 的更强说法（F-0294 留的核验项）

## 跑的命令（全部只读）
```
grep -rn "\.note\b" shell/flutter/lib/ --include=*.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0294-01 被更正**（范围收窄 + 根因改写）
```
lib/app/app_shortcuts.dart:28          this.note,                        ← **另一个类型**的 note
lib/app/shortcut_help_dialog.dart:45   if (e.note != null) … e.note!,    ← 同上（快捷键说明）
lib/main.dart:1073   ? 'ok · ${o.latencyMs ?? '?'} ms**${o.note == null ? '' : ' · ${o.note}'}**
lib/api/settings_models.dart:767       this.note,
```
⇒ ⇒ **TTS 自检（`:1073`）把 `note` 显示出来了** ⇒ ⇒ **更强说法（「全局无任何展示」）被推翻**
⇒ ⇒ ⭐⭐ **真正的缺口只在 LLM 自检（`:1046-1050`）** ⇒ **本条范围收窄到一处**

### ⭐⭐⭐ 而「两个同族功能实现不一致」**本身就是缺陷的一部分**
- 同一条规则（「**成功但有话要说**」）在**两个同族功能**上**写法不同**
- ⇒ **收敛的代价 = 零**（两行字符串拼接）⇒ ⇒ **没有任何理由不一致**
- ⇒ ⇒ 与 **B0249**（`chat_session` 的重复**可接受**，因为收敛会引入**层间倒挂**）**正相反**
  ⇒ ⇒ **判据补齐**：「有理由的重复」要问**收敛的代价**；**代价为零时，不一致本身就是缺陷**
- ⇒ ⇒ ⭐ **最佳修法就在同一个文件的三十行之下**：把 `:1046-1050` 按 `:1073` 的写法改
  ⇒ ⇒ 按 B0284 的家族「**谁负责解释应当只有一处**」⇒ 此处就是「**把两个同族写法对齐**」

### ⚠ 顺带一条**为后人记的坑**：**同名不同型**
`app_shortcuts.dart` / `shortcut_help_dialog.dart` 的 `note` 是**快捷键说明**的字段，
**与 `TestOutcome.note` 无关** ⇒ ⇒ **grep `\.note` 会同时命中两者** ⇒ 记在案上以免误判

### ⇒ 而本批的元收获：**我 B0294 的那句「同构」是没核就写的**
我在 B0294 写「`testTts` 的消费点 `:1065` 附近**同构**」⇒ **那一句我没有核**（我核的是 LLM 侧，
并**推测** TTS 侧相同）⇒ ⇒ **推测恰好是错的** ⇒ ⇒ **又一次「凭印象补全」**
⇒ ⇒ ⚠ 而它**差点**让这条发现被写成「两处都缺」，从而**低估**了它 ⇒ ⇒
**B0274 的规则（别把推测当观察）在这里的代价是「结论错一半」**

## 未核实项
1. `settings_models.dart` 的 `fromJson` 体未读 · `api_client.dart:217-226` 的 `_testEndpoint` 未读全
2. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
3. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
4. `main.rs` 余约 930 行未读
5. 真实动作计划的 token 长度（F-0020-01 永久敞口）
6. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
7. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
8. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
9. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
10. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
11. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
12. Mod crates 42 未读；`mod-system` 余 8 文件未读
13. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
14. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
15. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
