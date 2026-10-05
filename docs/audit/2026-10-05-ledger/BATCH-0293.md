# BATCH-0293 · ⭐ **「不假装」家族第七例，而且方向相反：不是「别少说」，是「别多说」**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 连通性自检的**响应模型**（`settings_models.dart`）

## 跑的命令（全部只读）
```
grep -rnE "auth_failed|protocol_error|non_auth_note|note|test/tts|test/llm" lib/settings/sections/dev_tools_section.dart lib/api/*.dart
sed -n '760,790p' lib/api/settings_models.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **同一族的反向形态**
```dart
/// 契约：`web_api/settings_routes/test_endpoints.rs::TestOutcome`。          // :762
/// **成功但有话要说**（例如「上游可达但未提供 /models」）。                    // :780
/// **只有真有解释价值时才非空** —— 服务端**刻意不在成功路径上默认塞一句**，
/// 否则**每次自检都多一句噪音，用户很快就不看它了**（**见服务端 `TestOutcome::note…`**）。  // :781-784
```
四个可核点：
1. ⭐ **Dart 类型是服务端 `TestOutcome` 的逐字镜像**（六字段全对：ok / latencyMs / modelEcho /
   note / errorCode / errorMessage）⇒ ⇒ **且点名了那个文件** ⇒ ⇒ **第五次「指路而非复述」**
2. ⭐ **`modelEcho` 的用途是写明的**：「服务端回显的模型/音色名 —— **用来确认『打到的确实是配置里那个』**」
   ⇒ ⇒ **回显不是装饰，是证据**
3. ⭐⭐⭐ **`note` 上有一条「不制造噪音」的规则**：
   「**只有真有解释价值时才非空**」+「服务端**刻意不在成功路径上默认塞一句**，否则
   **每次自检都多一句噪音，用户很快就不看它了**」
   ⇒ ⇒ **这是「不假装」的**反向**形态** —— 前面那些是「别假装成功 / 可用 / 能保存」，
   ⇒ ⇒ ⇒ **这一条是「别假装有话说」**
   ⇒ ⇒ ⭐ **不假装家族第七例，而且方向相反：不是「别少说」，是「别多说」**
4. ⭐ **`note` 与 `errorMessage` 是两个字段** ⇒ ⇒ 「成功但有话要说」与「失败有话说」**不共用通道**
   ⇒ ⇒ **B0284「必须精确到『不假装 X 而仍要做 Y』」的又一实例**

### ⇒ ⭐ 而第 3 点提炼出一条**新的区分**
**「真不真」的规则**（前面六例）与**「何时才有价值」的规则**（本例）**需要不同的判据**：
· 前者问「**这句话成立吗**」⇒ 用**类型/证据**判（B0232 ack 驱动 · B0247 落盘）
· 本者问「**这句话值得说吗**」⇒ 用**注意力**判（「用户很快就不看它了」）
⇒ ⇒ **同一个词（「不假装」）在两个方向上都成立，而它们不能共用一条判据**

## 未核实项
1. `settings_models.dart` 的 `fromJson` 体未读 · `dev_tools_section` 里**自检结果怎么呈现**（本批未落到 UI 侧）
2. 自检的**失败分类**在前端怎么显示（`auth_failed` / `protocol_error` / **`404 → 算通过`** 的那条裁决）
3. `message_bubble.dart` 余约 460 行未读（角色标签 · 流式光标 · `onlyReasoning` 折叠区）
4. `live2d_stage.dart` 余约 600 · `shell_admin.dart` 余面 · `env_key_field.dart` 余约 110 未读
5. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
6. `main.rs` 余约 930 行未读
7. 真实动作计划的 token 长度（F-0020-01 永久敞口）
8. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
9. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
10. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
11. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
12. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
13. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
14. Mod crates 42 未读；`mod-system` 余 8 文件未读
15. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
16. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
17. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
