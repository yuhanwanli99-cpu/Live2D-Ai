# BATCH-0286 · ⭐ **本审计读到过最强的密钥处理实现** —— 且其中一条**红线文档都没提**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `lib/settings/sections/env_key_field.dart`（B0285 定位到）

## 跑的命令（全部只读）
```
grep -rn "envApi|setEnv|/api/v1/env" lib/ --include=*.dart
grep -rn "obscureText" lib/ --include=*.dart
sed -n '1,20p' lib/settings/sections/env_key_field.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **六条各堵一个具体洞，其中一条超出红线文档**
```dart
/// rc.2 之前密钥**只能在**后端进程环境里：界面上能改模型名，却**改不了 key**。
/// 用户改完配置还是 401，且没有任何地方能改 —— **这是「改不动」的那一类缺陷**。
///                                             // :4-6
/// - 值**只往上走**：`PUT /api/v1/env`，响应只回 `{key, set}`；        // :10
/// - **没有回显**：本控件**不持有、不请求、不显示当前值**，只显示「已设置/未设置」； // :11
/// - 输入框是 `obscureText`，并且**每次保存后立即清空**
///   （**不留明文在内存里等 GC**）                                    // :12-13
/// 键名**不是用户随手填的**：它来自 `GET /api/v1/env`（即配置里 `api_key_env` 指向的名字）
/// ⇒ **不存在「界面上写了一个后端不读的变量名」这种静默失配**。        // :15-17
```
六个可核点：
1. ⭐ **「不持有」** ⇒ 控件**从不持有**当前值 ⇒ **无法被误渲染出来**
2. ⭐ **「不请求」** ⇒ **从不索要** ⇒ 与 B0120 的后端口径一致（`GET /api/v1/env` 只回**键名**）
3. ⭐ **「不显示」** ⇒ 只有「已设置/未设置」⇒ **正是 B0123 核的 `has_api_key` 形状**
4. ⭐⭐⭐ **「每次保存后立即清空（不留明文在内存里等 GC）」**
   ⇒ ⇒ ⭐ **这一条我在整个审计里没见过别处** ⇒ 它堵的是**残余窗口**：
   **明文留在 widget 里直到被 GC** 是真实（虽小）暴露，而**他们把它点名了**
5. ⭐ **「值只往上走」** ⇒ **只有一个方向** ⇒ **没有任何代码路径能把密钥带回来**
6. ⭐ **键名不可随手填**（来自 `GET /api/v1/env`）⇒ ⇒ **堵掉一个正确性洞**：
   不会出现「界面上写了一个后端不读的变量名」这种**静默失配**

### ⇒ ⭐ 而「它为什么存在」那段也是范本
写的**不是**抽象要求，而是**用户可见的症状**：「用户改完配置**还是 401**，且**没有任何地方能改**」
⇒ ⇒ **先说用户会撞到什么，再说为什么这么设计**

## 未核实项
1. `env_key_field.dart` 余约 140 行未读（保存后的文案、`{key, set}` 的消费、错误分支）
2. `shell_admin.dart:95-107`（`saveEnv` 的**立即刷新**与**通知宿主**）未读全
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
