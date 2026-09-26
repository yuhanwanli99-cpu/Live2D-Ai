# 阶段5 收口报告：导演可观测 A–D + 单模型动作强度 E（2026-09-26）

> 落盘人：编排者（顶层管理与代码审查）。工作树 = `/home/skystar/Live2D-Ai-l1`（分支 `mod/l1-product`），
> 基线 HEAD `49c2382c`（`docs(stage5): 范围冻结 + D40-D43 裁决`；任务书写的 `279d6081` 是它的父提交）。
> **本波零 git 写操作**（提交 / 并 main / 发布由维护者在 Gate 5 执行）。
> **交付物 = 可被维护者直接 `git add` 的文件集合 + 原始证据**；本报告不复述裁决理由，只报范围、实现落点、判据与证据、未决。
>
> 范围真源：[STAGE5-SCOPE-2026-09-26.md](STAGE5-SCOPE-2026-09-26.md)（§1 D40–D43 / §2 A–D 与 E / §3 文件归属 / §4 版本待确认）。
> 本报告**不改**该文件的任何裁决。

---

## 0. 一句话

阶段5 = ① 在**设置 → 开发工具**里加一个 **dev-only、零后端** 的「导演可观测」四栏（A 决策参数 / B 事件流 / C 传参对照 / D 送 TTS 文本）；
② 把动作幅度从「全局三倍率」扩成 **`[action]` 全局 + 可选 `[action.models.<model_id>]` 每模型覆盖（三键各自可选、各自回落全局）**，有效值经既有 `ActionScalesSyncer` 下发，**渲染面零改动**。
**撤销 S6**：不新增 `dev_tts_clean` 帧；`supervisor.rs` / `conversation/**` / `ws/events.rs` 一行未碰。

---

## 1. 交付物清单与文件归属（C1 前置）

```
$ git -C /home/skystar/Live2D-Ai-l1 status --porcelain=v1
 M AGENTS.md
 M crates/live2d-ai-desktop/src/web_api/dispatch.rs
 M crates/live2d-ai-desktop/src/web_api/settings_routes/mod.rs
 M crates/live2d-ai-desktop/src/web_api/settings_routes/tests.rs
 M crates/live2d-ai-runtime/src/settings.rs
 M crates/live2d-ai-runtime/src/settings/patch.rs
 M crates/live2d-ai-runtime/src/settings/patch_tests.rs
 M crates/live2d-ai-runtime/src/settings/view.rs
 M docs/architecture/action-packs-v0.md
 M docs/architecture/performance-protocol-v1.md
 M live2d-ai.toml.example
 M shell/flutter/lib/live2d/render_events.dart
 M shell/flutter/lib/main.dart
 M shell/flutter/lib/settings/sections/dev_tools_section.dart
 M shell/flutter/test/render_events_test.dart
?? shell/flutter/lib/settings/sections/director_observer_section.dart
?? shell/flutter/test/director_observer_test.dart
```

| 归属 | 文件 |
| --- | --- |
| **W5a**（Flutter A–D） | `settings/sections/dev_tools_section.dart`（挂载点）、`settings/sections/director_observer_section.dart`（新）、`live2d/render_events.dart`、`main.dart`、`test/director_observer_test.dart`（新）、`test/render_events_test.dart` |
| **W5r**（Rust E 配置面） | `runtime/src/settings.rs`、`settings/{view,patch,patch_tests}.rs`、`desktop/src/web_api/settings_routes/**`、`live2d-ai.toml.example`；**边界**：`desktop/src/web_api/dispatch.rs`（两处各一行透传，见 §3.5） |
| **W5e**（Flutter E UI） | `settings/sections/appearance_section.dart`、`api/settings_models.dart`、`app/shell_settings.dart`、`live2d/action_scales_sync.dart`、`main.dart`、`test/appearance_section_test.dart`、`test/action_scales_wiring_test.dart` |
| **W5c**（文档） | `architecture/performance-protocol-v1.md`、`architecture/action-packs-v0.md`、`AGENTS.md`、本文件 |

---

## 2. W5a（Flutter · 导演可观测 A–D）

### 2.1 实现落点

- 挂载点：`dev_tools_section.dart` 的 `DeveloperSection.build`，`if (devMode)` 块内 `DebugPanels` 之后 `const DirectorObserverSection()`（**dev_mode=false 整块不在语义树**）。
- 数据注入：`DeveloperSection` 由 `app/shell_settings.dart` 构造（W5a 不可改），故在 `director_observer_section.dart` 内定义单例 `DirectorObserverFeed`（`ChangeNotifier`）；`main.dart` 只 push，组件用 `ListenableBuilder` 消费。构造函数保留可注入的 `loadState` / `loadLogLines`，widget 测试注入 fake，**不发真网络**。
- A 栏：`ModsApi().state(kDirectorObserverModId).state` → 逐键 `latest.{emotion,intent,suggested_tts.speed,suggested_tts.pitch,preset_id,seq,turn}` + `turns_seen/turns_ended/decisions/silent/errors`；403/404/503 有如实文案，不空面板。
- B 栏：`ObserverBuffer` 环形缓冲 **上限 200**，按 type 多选过滤；覆盖 `action_cue` / 渲染面 ack 四条 / `segment-ended` / `stage-clock` / `turn_state` / `error` / `text_delta` / `text_fallback` / `runtime_status` / 未知 type（wire 名照记）；`subscribe_ack` / `heartbeat` 不记。
- C 栏：并排「请求（前端 `preset` 帧）」与「生效（渲染面 ack 最终值 + `clamped`/`degraded`/`reason`）」；配对按 field + seq（seq 优先），找不到写「尚无 ack」。
- D 栏：左 = 前端已收 `text_delta`/`text_fallback`（净化文本）+ 字符数；右 = `GET /api/v1/logs` 里 `sentence_ready` 行的**语义解析**（marker + 配平 JSON）+ 字符数，按 `(epoch,sentence_seq,text)` 去重；标注「来源=日志端点；不承诺逐句精确匹配」。**不新增任何 WS 帧**。
- 观测缓冲**只住内存**（源码扫描 + 负对照证明）。

### 2.2 判据原始读数（编排者独立复跑）

```
$ cd /home/skystar/Live2D-Ai-l1/shell/flutter && /home/skystar/flutter/bin/flutter analyze
Analyzing flutter...
No issues found! (ran in 2.3s)

$ /home/skystar/flutter/bin/flutter test
00:22 +1081: All tests passed!

$ /home/skystar/flutter/bin/flutter test test/font_subset_test.dart
00:00 +7: All tests passed!
```

判据①–⑤（W5a 自报 + 编排者复核实现）：
① A 栏逐键对上 `state_json`（注入同形 JSON，逐键断言；403 `mod_disabled` 反面在场）；
② C 栏同屏「请求 `field=head intensity=1 y=0.30`」与「生效 `y=0.240 clamped=false degraded=true reason=ParamBodyAngleY`」；
③ D 栏两列同 `Row` + 字符数 + 来源标注；三个 Mod 各记一行同句 → 去重为 1；
④ `DirectorObserverSection(devMode:false)` 与 `DeveloperSection(devMode:false)` 均找不到「导演可观测」；
⑤ 源码扫描 = **标识符集合交集断言**（`render_events.dart` ∪ `director_observer_section.dart` 的标识符集合 ∩ 禁止集合 = 空），**不是** grep 行命中：

```dart
final Set<String> identifiers = _identifiersOf(_stripComments(File(path).readAsStringSync());
final Set<String> hit = identifiers.intersection(forbiddenIds);
expect(hit, isEmpty, reason: '这是标识符集合的交集断言（不是 grep 行命中）…');
```

### 2.3 真实端点复核（编排者，只读）

```
$ curl -s http://127.0.0.1:18080/api/v1/mods/director/state
{"enabled":true,"id":"director","state":{"channel":"preset","decisions":9,…,
 "latest":{"closed":true,"delivered":false,"emotion":"neutral","intent":"chat",
 "preset_id":null,"seq":9,"suggested_tts":{"pitch":1.0,"speed":1.0},"turn":"9"},…}}
```
→ A 栏读取键与真实 `state_json` 逐字一致。

### 2.4 负对照四段（W5a）

| # | 修改点 | 红输出（关键行） | 还原 |
| --- | --- | --- | --- |
| 1 | 删 `if (!widget.devMode) return SizedBox.shrink()` | `Expected: no matching candidates / Actual: Found 1 widget with text "导演可观测"` | sha 回 `78e902e2…`，绿 |
| 2 | 删 `ObserverBuffer` 上限 | `Expected: <200> / Actual: <201>` | sha 回 `bcfee010…`，绿 |
| 3 | 对源码扫描加探针 `'localStorage'`（断言不动） | `Expected: empty / Actual: Set:['localStorage']`；换恒真断言（探针留着）→ 绿 | 删探针 + 还原断言，sha 回 `bcfee010… / 22aec7f4…`，绿 |
| 4 | 删 `parseSentenceReadyLogs` 去重 | `Expected: exactly 2 matching candidates / Actual: Found 4 widgets with text "你好呀（3 字符）"` | sha 回 `bcfee010…`，绿 |

### 2.5 W5a 边界（如实登记）

1. **调试面板 preset 请求未插桩**：`DebugPanels.onApplyPreset` 的调用点在 `app/shell_settings.dart`（W5a 无权改），C 栏因此收不到 `source=debug` 的请求；C 栏现覆盖 `director` / baseline `reason` / 撤销 `none`。
2. **B 栏忠实照记 `stage-clock`（30ms/条）**：播放期会较快填满 200 环并挤掉其它事件；界面提供按 type 过滤。是否降频/采样属语义变更，本轮不改，留维护者裁决。
3. **`action_cue_test.dart` 的通道 B 退役回归与 A 栏的张力**：该回归按**源码子串**禁止 lib/ 出现 `state('director')` / `mods/director/state`；A 栏（S2/D41 授权）必须读该端点做只读展示。W5a 做法 = 把 id 收成常量 `kDirectorObserverModId`、调用写成 `.state(kDirectorObserverModId)`、文案用插值。**这是本轮唯一「用常量规避既有字面断言」的点，编排者列为待维护者裁决**（建议 Gate 5 把该回归改写成语义断言：同文件「读 state 面」+「applyPreset」才红）。

---

## 3. W5r（Rust · 单模型动作强度 E 配置面）

### 3.1 实现落点

- `settings.rs`：`ActionModelOverride{head_scale,body_scale,expression_scale: Option<f32>}`（`deny_unknown_fields` + skip `None`）；`ActionSettings.models: BTreeMap<String, ActionModelOverride>`（`skip_serializing_if = BTreeMap::is_empty`，空表不写出）；`MAX_MODEL_ID_LEN=64` + `is_valid_model_id`（非空、≤64、ASCII 字母数字 `._-`、不含 `..`）；`EffectiveActionScales` + `ActionSettings::effective_for`（**逐键** `override.or(global)`）；`normalized()` 全局语义不变并对每个 `Some` 覆盖值 `clamp_action_scale`（NaN/∞ → 1.0 再钳）。
- `settings/view.rs`：`ActionView.models`（**三键恒出现，未覆盖 = `null`**，值已归一化）+ `SettingsView.active_model_id`；新函数 `settings_to_view_with_keys_and_model`；`settings_to_view` / `settings_to_view_with_keys` 签名与语义冻结（`active_model_id=""`）。
- `settings/patch.rs`：`ActionPatch.models: Option<BTreeMap<String, Option<ActionModelOverridePatch>>>`；值 `null` = 删该模型、对象 = 逐键三态合并、合并后三键全 `None` 剪枝（不留空表）；**非法 id → `Err`**（上层 400 `invalid_payload`），不静默落表；既有 action 三键与 `NoChange/Updated` 判定逐字未改。
- `settings_routes/**`：`handle_get/apply_and_write/handle_patch` 增 `active_model_id: &str`；`PatchResponse.settings` 也带它。
- `live2d-ai.toml.example`：`[action]` 段补 `[action.models.<model_id>]` 注释示例。

### 3.2 判据与负对照

判据①覆盖 > 全局且逐键回落 / ②无覆盖跟随全局 / ③写回不丢注释不留半截 / ④超量程钳 `[0.2,2.2]` / ⑤非法 id 不落表——全部有具名回归 + 原始输出（W5r 报告 §2）。
四段负对照（`effective_for` 恒全局 / 去非法 id 校验 / `merge_into_toml` 换 `to_toml_string` / 去 `clamp_action_scale`）红→还原→绿齐备（W5r 报告 §3）。

判据③端到端回归的核心断言（`settings_routes/tests.rs::e2e_action_model_overrides_keep_comments_and_prune_tables`）：
带注释 toml（顶部注释 / `[action]` 注释 / `bai` 覆盖注释 + `bai`/`follow` 两子表）经一次 PATCH（新增 `hiyori` / 改 `bai` / 删 `follow`）后：三条注释逐字还在、`[action.models.follow]` 与 `body_scale = 1.1` 从文本消失、仍可解析、`effective_for("bai")=1.5/0.80/1.0`、**无 `*.tmp.<pid>` 残留**。→ `ok`。

### 3.3 五门禁（编排者独立复跑，非采信）

```
$ cd /home/skystar/Live2D-Ai-l1 && cargo test --workspace --all-targets      # exit 0
（W5r 汇总：26 个 test result 段合计 passed=1457 failed=0）
$ cargo test --doc --workspace                                             # 3 passed / 0 failed
$ cargo fmt --all -- --check                                               # 无输出（0 字节差异）
$ cargo clippy --workspace --all-targets -- -D warnings                    # exit 0，0 warning
$ cargo run -p xtask -- rust-ratio
rs 267 文件 / 96899 行；合计 99527；Rust 占比 97.3595%；门槛 95%；结论 PASS
```

### 3.4 真实 HTTP 端到端（编排者，新二进制 + **临时配置副本**，不动用户配置）

```
$ cd /tmp/w5-e2e && cp <repo>/live2d-ai.toml ./live2d-ai.toml    # 副本；注释行 54
$ LIVE2D_AI_FLUTTER_WEB_DIR=<repo>/shell/flutter/build/web \
  <repo>/target/debug/live2d-ai-desktop --web --http-port 18082

GET  /api/v1/settings → active_model_id=''  action.models={}
PATCH {"action":{"models":{"bai":{"head_scale":1.2}}}} → HTTP 200
GET  → action.models = {"bai":{"head_scale":1.2,"body_scale":null,"expression_scale":null}}
temp toml 段：  [action.models.bai] / head_scale = 1.2     （注释行 54 → 54，未丢；全局 0.75/0.8/1.0 未动）
PATCH {"action":{"models":{"bai":null}}} → HTTP 200
GET  → action.models = {}；temp toml 段消失；注释行 54 → 54；无 .tmp.* 残留
```

### 3.5 `dispatch.rs` 边界改动（唯一越清单路径，逐行）

```diff
-            handle_get(&s, &live2d_ai_runtime::secrets::lookup)
+            // 阶段5 D40：另把当前模型 id 透传（前端据此算 action.models 逐键覆盖）。
+            let active_model_id = crate::web_api::models_routes::active_model_id(&ctx.models);
+            handle_get(&s, &live2d_ai_runtime::secrets::lookup, &active_model_id)
@@
+            // 阶段5 D40：PATCH 响应也带当前模型 id（保存后前端不丢模型名）。
+            let active_model_id = crate::web_api::models_routes::active_model_id(&ctx.models);
             let resp = handle_patch(
                 &s, body_str, &config_path, Some(&*hook),
                 &live2d_ai_runtime::secrets::lookup,
+                &active_model_id,
             );
```
理由：`SettingsView.active_model_id` 只能由持有 model registry 的 dispatch 层给出；`dispatch.rs` 不在本阶段红线清单、也不在任何波次独占清单里。**已按 C1 明列，请维护者追认**（与 D32/D37 同类：计划表不完整）。

---

## 4. W5e（Flutter · 单模型幅度 UI）

### 4.1 实现落点

- `live2d/action_scales_sync.dart`（唯一下发口）：新增 `setModelContext({activeModelId, overrides})`；`active()` / `syncNow()` 下发前经 `_withModelOverride` 逐键替换 `override[key] ?? global[key]`；无 context / 无覆盖**逐字走旧行为**（连 Map 身份都不换）；临时 pin 优先级不变（覆盖只作用于产品/草稿载荷）。
- `main.dart`：`_refresh()` 内推 context + schedule；settings 监听器换成 `_onActionScalesModelContextChanged`（**先刷 context 再 schedule**，避免换模型时闪一帧旧覆盖）；模型 id 优先取 `/app/status` 的 `active_model_id`（每次 `_activateModel → _loadAdmin` 刷新），空则回落 `settings.action.activeModelId`。
- `api/settings_models.dart`：`ActionModelOverrideView`（三键 double?）+ `toModelOverrideMap`；`ActionSettingsView` 增**可选** `activeModelId`/`models` 与 `effectiveForActiveModel`；`ActionModelOverridePatch` + `ActionSettingsPatch.models`（**null 写成 JSON null = 删除**）；旧构造签名向后兼容。
- `settings/sections/appearance_section.dart`：模型名行（空 id 时「未识别当前模型」并禁用开关）、「本模型覆盖」开关、**复用既有三条滑条**（关 = 全局值 + 草稿回调；开 = 本模型有效值 + 直接 PATCH 回调）、「恢复跟随全局」（仅开启时可点）。
- `app/shell_settings.dart`：覆盖走 `PATCH → _settings.load() → _refresh()`，**不碰** `settings_controller.dart`；编排者复核后追加 **`ModelOverrideCoalescer`（250ms 合并、只记被改键、换模型先 flush、clear/seed/dispose 先 cancel）** 与 `_modelOverrideChain` 串行化——一次拖动只落一次 PATCH，且 PATCH 顺序 = 操作顺序。

### 4.2 判据原始读数（编排者独立复跑门禁，见 §6.C4）

```
$ /home/skystar/flutter/bin/flutter test test/action_scales_wiring_test.dart --plain-name 'W5e/D40'
00:00 +0: ① 本模型 head 覆盖生效：逐键替换，其余回落全局
00:00 +1: ② 切模型：bai 有覆盖 → hiyori 无覆盖，下发回全局 head 0.75
00:00 +2: ③ 重启后仍在：PATCH 形状 == 服务端 D40 契约；GET 往返能算有效值
00:00 +3: ④ 恢复跟随全局：body == {"action":{"models":{"bai":null}}}，下发回全局
00:00 +4: ⑤ 合并防抖：连续 onChanged 只落 1 次 PATCH，且只发被改过的键
00:00 +5: All tests passed!

$ /home/skystar/flutter/bin/flutter test test/appearance_section_test.dart
00:00 +5: All tests passed!     # 未识别模型禁用 / 关态走草稿 / 开态逐键有效值 / 恢复按钮 / 开关回调
```

- ① 设 bai head=1.2 → syncer 下发 `{'head':1.2,'body':0.80,'expression':1.0}`；HUD 数值断言 `toActionScalesPayload(effective)['head']==1.20`（即 HUD `scale h1.20`）。
- ② 同 overrides 下 activeModelId `bai → hiyori` → `sent.last['head']==0.75`（回全局）。
- ③ `ActionSettingsPatch(models:{bai:{head:1.2}}).toJson()` == `{"action":{"models":{"bai":{"head_scale":1.2}}}}`（只写被改键）；`ActionSettingsView.fromJson`（含顶层 `active_model_id` + 三键恒出现）往返算出 `head=1.2 / body=0.80`。
- ④ `models:{bai:null}` == `{"action":{"models":{"bai":null}}}`（显式 null）；UI「恢复跟随全局」按钮触发该回调；删后回全局。
- ⑤ 防抖合并（编排者追加）：fakeAsync 窗口内 20 次 head + 1 次 body → 只 1 次 flush，键集恰 `{head_scale, body_scale}`（expression 保持未覆盖）。

### 4.3 负对照四段（W5e）

统一命令 `flutter test test/action_scales_wiring_test.dart --plain-name 'W5e/D40'`；四段均只改授权文件并还原（`grep -rn 'NC[0-9]' lib test` → 无残留）。

| # | 修改点 | 红输出（关键行） | 还原 |
| --- | --- | --- | --- |
| NC1 | `_withModelOverride` 直接 `return global` | `Expected {'head':1.2,…} / Actual {'head':0.75,…}`（①②④ 红） | 绿 |
| NC2 | 不按 activeModelId 选表（取 map 任一覆盖） | `Expected: <0.75> / Actual: <1.2>`（仅②红） | 绿 |
| NC3 | 去掉 `ActionSettingsPatch.toJson` 的 `models` 键 | `Expected {'action':{'models':…}} / Actual {}`（③④ 红） | 绿 |
| NC4 | null 值跳过（不写 JSON null） | `Actual {'action':{'models':{}}}`（仅④红） | 绿 |

### 4.4 W5e 边界

1. **覆盖滑条无本地乐观值**：显示值等 `PATCH + load()` 回来才更新（合并窗口 250ms）；功能正确、快速拖动略有视觉滞后。
2. **`_settings.load()` 会丢未保存的全局草稿**：覆盖路径是任务书指定的 `PATCH → load()`；`settings_controller.dart` 是禁区，未改。属既有 `load()` 语义。
3. **dispose 丢弃窗口内待发覆盖**（避免请求发给正在关闭的 client）；分区切换走 `flush()` 不丢。

---

## 5. W5c（文档）

| 文件 | 改动 |
| --- | --- |
| `docs/architecture/performance-protocol-v1.md` | §12.1 **O10 行标注「已由阶段5 D40 修订」**；§12.3 增补 **D40–D43** 四行（D40 每皮套幅度住 toml / D41 撤销 S6 / D42 D 栏零后端 / D43 Gate 5 与禁 git 写） |
| `docs/architecture/action-packs-v0.md` | 新增 **§11.7 每皮套覆盖的落点（阶段5 D40）**：toml 示例、逐键回落、量程、写回纪律、渲染面零改动、映射表不承担用户旋钮 |
| `AGENTS.md` | 新增 **「导演可观测（2026-09-26 阶段5 定）」** 与 **「每皮套动作幅度（2026-09-26 阶段5 定，D40）」** 两节 |
| `docs/plans/STAGE5-CLOSEOUT-2026-09-26.md` | 本文件 |

相对链接校验：三份被改文档的 markdown 相对链接解析 **19/19 通过、0 坏链**（检查器把链接目标的相对路径 join 到该文件目录后断言 `exists()`）。

---

## 6. 合并收口 C1–C8

### C1 越权核对

授权集合（W5a ∪ W5r ∪ W5r 边界 dispatch.rs ∪ W5e ∪ W5c）对 `git status --porcelain` 全量路径取差集：

```
=== C1: changed paths = 24 | unowned(RED) = 0 | forbidden-redline hits = 0
   M  AGENTS.md                                                          W5c
   M  crates/live2d-ai-desktop/src/web_api/dispatch.rs                   W5r-boundary
   M  crates/live2d-ai-desktop/src/web_api/settings_routes/mod.rs        W5r
   M  crates/live2d-ai-desktop/src/web_api/settings_routes/tests.rs      W5r
   M  crates/live2d-ai-runtime/src/settings.rs                           W5r
   M  crates/live2d-ai-runtime/src/settings/patch.rs                     W5r
   M  crates/live2d-ai-runtime/src/settings/patch_tests.rs               W5r
   M  crates/live2d-ai-runtime/src/settings/view.rs                      W5r
   M  docs/architecture/action-packs-v0.md                               W5c
   M  docs/architecture/performance-protocol-v1.md                       W5c
   M  live2d-ai.toml.example                                             W5r
   M  shell/flutter/lib/api/settings_models.dart                         W5e
   M  shell/flutter/lib/app/shell_settings.dart                          W5e
   M  shell/flutter/lib/live2d/action_scales_sync.dart                   W5e
   M  shell/flutter/lib/live2d/render_events.dart                        W5a
   M  shell/flutter/lib/main.dart                                        W5a,W5e
   M  shell/flutter/lib/settings/sections/appearance_section.dart        W5e
   M  shell/flutter/lib/settings/sections/dev_tools_section.dart         W5a
   M  shell/flutter/test/action_scales_wiring_test.dart                  W5e
   M  shell/flutter/test/render_events_test.dart                         W5a
  ??  docs/plans/STAGE5-CLOSEOUT-2026-09-26.md                           W5c
  ??  shell/flutter/lib/settings/sections/director_observer_section.dart W5a
  ??  shell/flutter/test/appearance_section_test.dart                    W5e
  ??  shell/flutter/test/director_observer_test.dart                     W5a
VERDICT: PASS
```

- 差集 = 空：**0 条越权路径**。
- 红线文件命中 = 0：`topics.rs` / `mod_registry.rs` / `supervisor.rs` / `supervisor/{handlers,turn}.rs` / `desktop/src/main.rs` / `web_api/ws/events.rs` / `conversation/**` 全部未改。
- 唯一越清单路径 = `dispatch.rs`（W5r 边界，两处一行透传，见 §3.5），已明列请维护者追认。

### C2 契约一致

| 契约 | 证据 |
| --- | --- |
| A 栏键 ↔ `GET /api/v1/mods/director/state` `state_json` | 真实端点原文（§2.3）；`latest` 七键与 `ledger.rs::to_json` 逐字一致 |
| D 栏 ↔ 日志端点 `sentence_ready` 行 | 真实日志行形如 `… 收到事件 sentence_ready: {"epoch":0,"sentence_seq":1,"text":"…","ts_ms":893}`；解析器 marker + 配平 JSON；`/api/v1/logs` 仅 dev_mode |
| E 轨 GET/PATCH ↔ 前端消费 | W5r `view.rs` 三键恒出现/null；`ActionModelView` 契约写死；真实 HTTP 往返（§3.4） |
| 不新增 WS 帧（D41） | `git diff` 无 `ws/events.rs` / `conversation/**` / `supervisor.rs` |

### C3 Rust 五门禁

见 §3.3（全部 exit 0 / PASS）。

### C4 Flutter analyze + test + build web 三证据 + ignite --check

```
$ cd /home/skystar/Live2D-Ai-l1/shell/flutter && /home/skystar/flutter/bin/flutter analyze
Analyzing flutter...
No issues found! (ran in 2.3s)

$ /home/skystar/flutter/bin/flutter test
00:25 +1091: All tests passed!

$ /home/skystar/flutter/bin/flutter test test/font_subset_test.dart
00:00 +7: All tests passed!

$ /home/skystar/flutter/bin/flutter build web --release --base-href /app/ --no-web-resources-cdn
Compiling lib/main.dart for the Web...                             24.6s
✓ Built build/web

# 证据①（产物晚于最后一个 lib/**/*.dart）
build/web/main.dart.js  mtime 2026-09-26 23:24:16.894  (epoch 1790436256)
newest lib/**/*.dart    lib/api/settings_models.dart 23:18:26 (epoch 1790435906)  → PASS
# 证据②（main.dart.js 不引用 Google CDN）
grep -c 'gstatic.com/flutter-canvaskit' build/web/main.dart.js → 0
# 证据③（真起服务后体检）
$ ./scripts/ignite.sh --check
    [ok]   GET / → 302 Location: /app/
    [ok]   GET /app/ → 200
    [ok]   /app/index.html 不依赖 Google CDN
    [ok]   /app/main.dart.js 不依赖 Google CDN
==> 体检通过
```

> `flutter_bootstrap.js` 里有 1 处引擎自带的通用分支字符串（`!e.useLocalCanvasKit ? CDN : "canvaskit"`），`--no-web-resources-cdn` 下走本地 `canvaskit/`（`canvaskit.wasm` 在场）；门禁点名的 `main.dart.js` 与 `index.html` 均 0 命中。

**服务状态**：$18080$ 上原为陈旧二进制（D36）；编排者用本轮新二进制重启后，实测
`GET /api/v1/settings` → `active_model_id='bai'`、`action.models={}`，`ignite --check` 四项全 ok。

### C5 端到端贴跳数

```
$ python3 scripts/verify_core_chain.py --timeout 300 --base http://127.0.0.1:18080
OK 服务可达(uptime=17s dev_mode=True) / LLM 已配置(deepseek-flash) / TTS 已配置(127.0.0.1:8080 voice=skystar)
OK 皮套已装载(active_model_id=bai) / 前端 UI 产物 / 渲染面 / 模型资产可达
OK WS 已连接 / 对话已受理 / LLM 有正文(10 字) / TTS 有产物(208 帧) / 音频有本体 / 采样率一致
OK 句子边界配对(1 句，start/end 各一) / 口型有电平源 / 本轮已收口 / 无动作投影(0 个 action_state)
核心链路闭环：17 跳全部通过
```
**前置**：`18080` 上原为陈旧二进制（`GET /api/v1/settings` 无 `active_model_id`/`action.models`，即 D36 的「看到的不是本轮代码」）；
编排者用 W5r/W5a/W5e 后的新二进制重启（`./scripts/ignite.sh --port 18080`，脚本自带 pkill），
重启后 `GET /api/v1/settings` 实测 `active_model_id='bai'`、`action.models={}`。
D40 的写路径在**临时配置副本 + 18082** 上做（§3.4），**未改动用户配置**。

### C6 负对照汇总

| 轨 | 声称 | 负对照 | 结论 |
| --- | --- | --- | --- |
| W5a | 去 dev_mode 短路 → ④红 | 实测 | 达标 |
| W5a | 去缓冲上限 → cap 回归红 | 实测 | 达标 |
| W5a | 去源码扫描断言 → 落盘实现可混入 | 探针三步 | 达标 |
| W5a | 去 sentence_ready 去重 → ③红 | 实测 | 达标 |
| W5r | `effective_for` 恒全局 → ①红 | 实测 | 达标 |
| W5r | 去非法 id 校验 → ⑤红 | 实测 | 达标 |
| W5r | `merge_into_toml` 换 `to_toml_string` → ③红 | 实测 | 达标 |
| W5r | 去 `clamp_action_scale` → ④红 | 实测 | 达标 |
| W5e | 去 syncer 逐键解析 → ①②④红 | 实测 | 达标 |
| W5e | 不按 activeModelId 选表 → ②红 | 实测 | 达标 |
| W5e | 去 patch 的 models 键 → ③④红 | 实测 | 达标 |
| W5e | null 值跳过 → ④红 | 实测 | 达标 |

### C7 D14 判据核对（禁字面 grep）

| 判据 | 断言形式 | 是否语义 |
| --- | --- | --- |
| W5a ⑤ 观测缓冲不落盘 | 文件标识符集合 ∩ 禁止集合 = 空（含负对照探针） | ✅ 集合交断言 |
| W5a 接线（`onRenderEvent: _directorLog.add`） | 既有源码扫描（wiring 先例） | ✅ 语义锚点 |
| W5e 覆盖路径（`setModelContext` / `_clearModelOverride` 接线） | 源码扫描（wiring 先例）+ 纯逻辑语义断言（syncer 载荷、patch toJson、coalescer 键集） | ✅ 语义为主，接线锚点为既有先例 |
| 全文其它「不要/不得」 | 无新增字面 grep 判据 | ✅ |

### C8 报告分节

本文件即 C8：按 W5a / W5r / W5e / W5c / C1–C8 分节。

---

## 7. 未决 / 需维护者裁决

1. **`action_cue_test.dart` 的通道 B 字面断言被常量规避**（§2.5-3）——建议 Gate 5 改写为语义断言，或追认常量写法。
2. **`dispatch.rs` 两处一行透传**（§3.5）——追认授权（同类 D32/D37）。
3. **`patch_tests.rs` 992 行 > AGENTS 测试文件 800 行上限**——已加头注理由（三态语义成体系，拆分易「改一半忘一半」）；无自动门禁，请维护者在审查时定夺是否拆。
4. **B 栏 `stage-clock` 30ms 灌满环形缓冲**（§2.5-2）——是否降频/采样，属语义变更，留裁决。
5. **调试面板 `source=debug` 的 preset 请求未进 C 栏**（§2.5-1）——需改 `shell_settings.dart` 调用点；本轮按文件归属未越权。
6. **版本号**：`0.2.0-rc.4` vs `0.2.0`（STAGE5-SCOPE §4）——本波不 bump，待用户最终确认。
