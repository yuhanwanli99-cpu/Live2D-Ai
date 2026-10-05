# BATCH-0294 · ⭐ **F-0294-01（P3）**：成功路径上 **`note` 被丢弃** —— 有意提供的解释**掉在半路**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 自检结果的两个消费点（`main.dart:1043` / `:1065`）

## 跑的命令（全部只读）
```
grep -rnE "\.ok|errorCode|errorMessage|note|latencyMs|modelEcho" lib/app/shell_admin.dart
grep -rln "SettingsTestOutcome" lib/ ; grep -rn "SettingsTestOutcome" lib/ | grep -v settings_models
sed -n '1040,1060p' lib/main.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0294-01（P3）**
```dart
_llmTest = o.ok
    ? 'ok · ${o.latencyMs ?? '?'} ms'
          '${o.modelEcho == null || o.modelEcho!.isEmpty ? '' : ' · 模型：${o.modelEcho}'}'   // :1047-1048
    : '失败：${o.errorMessage ?? o.errorCode ?? '未知原因'}';
```
### ① 三处**做对了**的
1. ⭐ **成功路径把 `latencyMs` 与 `modelEcho` 都显示** ⇒ ⇒ B0293 核的 `modelEcho`
   （「**确认打到的确实是配置里那个**」）**真的被呈现了** ⇒ 回显是**证据**，不是装饰
2. ⭐⭐ **失败路径三层兜底 `errorMessage ?? errorCode ?? '未知原因'`** ⇒ ⇒ **先人话、再机器码、最后兜底**
   ⇒ ⇒ **顺序本身就是设计**（同 B0261「先人话后码」家族）
3. ⭐ **`ApiException` 单独一支**（`:1053-1057`）⇒ ⇒ 「**请求根本没到服务端**」与「到了但被判失败」是两回事
   ⇒ ⇒ **B0284 的「必须精确到不同类」**；且两次 await 后都有
   **`if (!mounted || epoch != _resultEpoch) return;`** ⇒ ⇒ **`mounted` 管「控件还在不在」、
   `epoch` 管「这次结果是不是最新的」，各管一件事**（B0140 核过的 epoch 闩锁）

### ② ⭐⭐ 而缺口是：**成功分支只读了 `latencyMs` 与 `modelEcho`，`o.note` 被丢弃**
- AGENTS 记的裁决：**404 算通过**（上游回话即可达）+ **一句 `note` 说明**
- 服务端据此产出 `note`；Dart 侧**把它建模成字段**且**为它写了规则**（「**只有真有解释价值时才非空**」）
- ⇒ 而**成功分支不读它** ⇒ ⇒ **用户看到「ok · 12 ms · 模型：X」**，
  **看不到「为什么这次是通过而上次是失败」**
- ⚠ **这不是「假装成功」**（`ok` 来自服务端、失败分支完整呈现错误码）⇒
  **问题是「有意提供的解释被丢在半路」**

### ③ 建议（两条路径，改的都是一处）
① **成功分支带上 note**（一行）：`'… · 模型：X'${o.note == null ? '' : '（${o.note}）'}'` ⇒ 与字段侧语义**对齐**
② 或**在 Dart 侧删掉 `note` 字段与其规则注释**、并注明「note 由 v1 客户端忽略」
   ⇒ ⇒ **让「谁负责解释」只有一处**（B0284 家族）

### ④ ⚠ 而我把反证保留在案：**「全局无任何展示」这个更强说法我未核**
我只穷举了 `SettingsTestOutcome` 的两个消费点（`main.dart:1043`/`:1065`）**这一条路径**；
**banner/toast 等其它路径未逐一排除** ⇒ ⇒ **本条只声称「`main.dart:1046-1050` 不显示它」**（可证）
⇒ ⇒ **接手者先跑 `grep -rn "\.note" shell/flutter/lib/` 再定修法**
· 另：反证 (b)「404 通过那类场景很少」——**可能为真，但不改变事实**；
且按 B0293 的规则「**只有真有解释价值时才非空**」⇒ **恰恰是「有值的那次」被丢掉了** ⇒ **本条更成立**

## 未核实项
1. ⭐ **`note` 是否在别处（banner/toast）被展示** —— **明确未核**（本条的更强说法依赖它）
2. `settings_models.dart` 的 `fromJson` 体未读 · `api_client.dart:217-226` 的 `_testEndpoint` 未读全
3. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
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
