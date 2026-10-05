# BATCH-0287 · ⭐ **「保存后立即清空」是代码里做的事**，而它的**判据比注释还讲究**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `env_key_field.dart` 的保存路径（B0286 的核验项）

## 跑的命令（全部只读）
```
grep -nE "clear\(\)|_value|await|setState" lib/settings/sections/env_key_field.dart
sed -n '62,92p' lib/settings/sections/env_key_field.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **判据与理由是同一条**（B0286 那条注释的实现侧）
```dart
final String value = _value.text.trim();                  // :66 先取值
try {
  await save(status.key, value);                          // :71 先写
  if (!mounted) return;                                   // :72
  setState(() {
    _saving = false;
    // **立刻清掉明文：它已经写进 .env，没有理由继续留在输入框里。**     // :75
    _value.clear();                                       // :76  ⭐ **写成功才清**
    _message = value.isEmpty ? '已清除 ${status.key}'
                            : '已写入 ${status.key}，**立即生效**（不用重启）';
  });
} catch (error) {
  if (!mounted) return;                                   // :83
  setState(() { _saving = false; _message = '保存失败：$error'; });  // :85  ⭐ **失败不清**
}
```
四个可核点：
1. ⭐ **清空发生在 `await save(...)` 成功之后** ⇒ **失败时输入框内容保留** ⇒ **用户不必重打**
2. ⭐⭐⭐ **清空的判据不是「保存动作完成」，而是「明文已经没有存在的理由」**
   —— 注释：「**它已经写进 `.env`，没有理由继续留在输入框里**」
   ⇒ ⇒ **理由会随事实变化**：若 `.env` **没写成功，明文确实还有理由留着**
   ⇒ ⇒ ⭐ **所以「只在成功时清」不是一条独立规则，而是那条理由的推论**
   ⇒ ⇒ ⭐ **可提炼**：**把「什么时候做」绑到「为什么做」，正确的时机就会自己掉出来** ——
   比写一条「成功后就清」更耐改（**换存储、换传输、换后端语义时都不用重推时机**）
3. ⭐ `if (!mounted) return;` 在 await 之后（`:72` 与 `:83`）⇒ **B0230 的 `mounted` 纪律第四处**
4. ⭐ **成功文案区分两态**：「已**清除** …」（空值 = 清除，**对齐 AGENTS 的「空值 = 清除」口径**）
   vs「已**写入** …，**立即生效**（不用重启）」⇒ 与 B0120 核的「写入后刷新快照 + `supervisor.reload()`」**口径一致**
   ⇒ 且**明确告诉用户不用重启** ⇒ ⇒ **把后端已兑现的收益告诉用户**（同 `error_banner` 家族）

## 未核实项
1. `shell_admin.dart:95-107`（`saveEnv` 的**立即刷新**与**通知宿主**）未读全
2. `env_key_field.dart` 余约 110 行未读（`build` 侧、文案渲染、`EnvStatus` 的消费）
3. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
4. `main.rs` 余约 930 行未读
5. 真实动作计划的 token 长度（F-0020-01 永久敞口）
6. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
7. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
8. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465
9. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
10. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
11. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
12. Mod crates 42 未读；`mod-system` 余 8 文件未读
13. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
14. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
15. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
