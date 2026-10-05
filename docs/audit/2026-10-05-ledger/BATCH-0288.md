# BATCH-0288 · ✅ **密钥写入链首尾读完** —— 五步，每一步的理由都写着，且**没有一步多余**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `lib/app/shell_admin.dart:95-112`（`saveEnv`）

## 跑的命令（全部只读）
```
sed -n '93,112p' shell/flutter/lib/app/shell_admin.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **失败被归因到正确的那一半**
```dart
/// 值**只在参数里出现**：不写进 state、不进日志、不回显。保存后重读一次
/// `GET /api/v1/env` **只为**刷新「已设置/未设置」这个布尔。            // :100-101
await _envApi.write(key, value);   if (!mounted) return;                // :102-103
try { final EnvStatus env = await _envApi.list();  if (!mounted) return;
      _envStatus = env;  _refresh(); }                                 // :105-108
on ApiException { /* 写成功了但读状态失败：**不动现有状态**（`EnvKeyField` 自己的提示仍然准确）*/ }
```
五个可核点：
1. ⭐ **它明确拒绝一句多余的话**：「后端写完刷新快照 + 重建 LLM/TTS client ⇒ 所以这里
   **不需要（也不该）**提示『重启』或『重跑 ignite.sh』」（`:96-97`）
   ⇒ ⇒ **防的是「加一句以防万一」** ⇒ ⇒ **文档式的诚实**（B0227「不假装」在文案层）
2. ⭐ **「值只在参数里出现」**：不进 state、不进日志、不回显 ⇒ 与 B0286/B0287 首尾相接
3. ⭐⭐ **重读的理由被限定**：「**只为**刷新『已设置/未设置』这个布尔」
   ⇒ ⇒ **不是「顺便同步一下」** ⇒ **这一读只有一个用途，被写明了**
4. ⭐⭐⭐ 而 `on ApiException` **分清了是哪一半失败**：
   「**写成功了但读状态失败**：**不动现有状态**（`EnvKeyField` 自己的提示仍然准确）」
   ⇒ ⇒ **它知道写入是成功的** ⇒ ⇒ **不清空、不报错、不回滚 UI**
   ⇒ ⇒ **失败被归因到正确的那一半**，而 **UI 的既有提示仍然是真的**
5. ⭐ 两个 `if (!mounted) return;`（`:103`/`:106`）⇒ **B0230 纪律第五、六处**

### ⇒ 整条链（跨 4 个文件、两种语言）**首尾读完**
| 步 | 位置 | 做了什么 |
|---|---|---|
| ① | `env_key_field.dart:1-17` | 控件**不持有、不请求、不显示**；`obscureText`；**键名不可随手填** |
| ② | `env_key_field.dart:66-76` | **只往上走**；**写成功才清**（理由：「已写进 `.env`，**没有理由**继续留着」） |
| ③ | `shell_admin.dart:96-97` | **拒绝多余的话**（「不需要提示重启」） |
| ④ | `shell_admin.dart:100-101` | 值**只在参数里**；重读**只为那个布尔** |
| ⑤ | `shell_admin.dart:109-111` | 读失败 ⇒ **不动状态**（因为**写确实成功**） |
| ⑥ | 后端（`env_routes` / B0120） | 原子写 + `0600` + 就地改行 + 刷新快照 + `supervisor.reload()` |
⇒ ⇒ **0 findings**；**五步每一处都带理由，且没有一处是「以防万一」加的**

## 未核实项
1. `shell_admin.dart` 余面与 `env_key_field.dart` 余约 110 行未读
2. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
3. `main.rs` 余约 930 行未读
4. 真实动作计划的 token 长度（F-0020-01 永久敞口）
5. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
6. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
7. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465
8. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
9. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
10. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
11. Mod crates 42 未读；`mod-system` 余 8 文件未读
12. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
13. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
14. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
