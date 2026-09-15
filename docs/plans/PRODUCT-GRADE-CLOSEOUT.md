# 产品级加强波次 收束报告（`mod/product-grade`，2026-09-14）

> **这不是 release 文件**。本波**不发布、不 bump 版本、不打 tag、默认不 push**。
> 版本保持 **`0.2.0-rc.3`**（`Cargo.toml` / `pubspec.yaml` / README 一处未动）。
>
> - **真源**：`/home/skystar/Live2D-Ai-stabilize` @ `71348c93`（stabilize tip）。
> - **集成分支 / worktree**：`mod/product-grade` @ `/home/skystar/Live2D-Ai-product`。
> - **口径**：主链皮肤（LLM/TTS/口型/Live2D、壳/舞台背景）与 `crates/l2d-wasm-demo/`
>   （framebuffer / `stage_bg`）**一行未改**；封存 wallpaper / pet-desktop；
>   其余五个 Mod 推到产品级。
> - **用户裁决（逐条遵守）**：wallpaper **删除并封存**（不做本地下载库/自由切换）；
>   pet-desktop **封存、暂不推**（不做真窗/应用级桌宠）；其余五个推到产品级。

---

## 1. 一句话

把 Mod 层从「能编进 binary 的七个骨架」收成**五个能用、看得见、失败可读的产品能力**；
同时按用户裁决**封存 wallpaper + pet-desktop**（注册面 7 → 5），并在真点火里抓到、
修掉两个只有全量门禁 / 真链路才暴露的缺陷（`local-llm` 方法名撞名、memory 配置路径锚点）。

## 2. 封存结果（FACTORIES 最终列表与数字）

`crates/live2d-ai-desktop/src/main.rs::AVAILABLE_MOD_FACTORIES` **恰好 5 个**：

| # | id | 缺省 | 说明 |
| --- | --- | --- | --- |
| 1 | `external-input` | **on** | 外部事件接入（本波加强：计数可见 + `reset_counters` + `token_set`） |
| 2 | `persona` | off | 角色卡（本波加强：卡导入 + `state` 200 + 面板） |
| 3 | `voice-input` | off | 语音输入（本波加强：说人话面板 + `selftest` + 失败码文档） |
| 4 | `memory` | off | 会话记忆（本波加强：`records` + 清空 + 面板） |
| 5 | `director` | off | 导演（本波加强：一等决策面板 + `latest` + `clear`；**零投递不变**） |

数字护栏：`mod_count_is_seven` → **`mod_count_is_five`**（id 期望集同步）。

### 2.1 封存动作（与 `local-llm` 的 DEPRECATED 同口径）

| 项 | wallpaper | pet-desktop |
| --- | --- | --- |
| 移出 `AVAILABLE_MOD_FACTORIES` | ✅（7 → 5） | ✅（7 → 5） |
| 移出 `live2d-ai-desktop` 依赖 | ✅ | ✅ |
| crate 暂留 workspace（可编译/可测） | ✅ | ✅ |
| crate 头注 ARCHIVED / 禁止挂回 | ✅ | ✅ |
| 缺省 manifest 未引用 | ✅（本就没引用） | ✅（本就没引用） |
| 架构文加封存横幅 | ✅ `wallpaper-mod-v0.md` | ✅ `pet-desktop-mod-v0.md` |

**新台账**：[`docs/architecture/ARCHIVED-mods.md`](../architecture/ARCHIVED-mods.md)
——逐条写清「为什么封存 / 明确没有连坐删掉什么 / 恢复条件」。

### 2.2 明确**没有**被删掉的东西（用户能力）

- `DisplayPrefs` 的 `stageImage` / `shellImage` / `stagePlaylist` / `syncShellStageBg`
  与「外观与互动」里的**手动选图 / 清图 / 列表增删排序**：**全部保留**；
- Flutter 侧只拆掉 wallpaper **Mod** 的接线（`wallpaper_api.dart` / `shell_wallpaper.dart` /
  `applyWallpaperPatch` / 轮询 / `_syncWallpaperPlaylistLen`），`build/` 缓存之外**零源码残留**；
- `l2d-wasm-demo` 的背景预通道与 framebuffer：一行未改。

### 2.3 封存后的门禁同步

- `scripts/ignition-precheck.sh`：注册 id 集合 7 → **5**、`--fsm` 矩阵 5 个、
  external state 计数键改为**必含子集**检查（容忍新增 `token_set`）；
- 现行文档（`mod-product-chain.md` §5 / `mod-community-license.md` §4 / `AGENTS.md` /
  `README(.zh-CN)` / `docs/README` / `directory` / `core-chain-baseline` / `director-rfc` /
  `external-input` / `tts-is-core` / `director-mod-v0`）全部同步；历史文档（releases /
  plans / research / legacy）数字**保持不动**。

## 3. 五个 Mod 的产品级达成

口径：**用户主要靠「Mod 管理 + 壳 UI + 一条文档命令」走通主路径；enable 后有可感知效果；失败态可读。**

### 3.1 `external-input`（直播弹幕注入）

| 诉求 | 落点 |
| --- | --- |
| Mod 管理可见计数 | `state_json` = `accepts/rejects/busy/v2_ignored/ready/token_set`；面板给中文标签 + 一行「已接受 N · 拒绝 M · 忙碌丢弃 B · 礼物 v2 兜底 V」 |
| 可点动作 | 新命令 `reset_counters`（`counters::reset()`，`swap(0)` 不丢并发账）→ 面板「重置计数」二次确认，回执带清零前快照 |
| token 可见不回显 | `state.token_set: bool`（config token 或 env），**永不回显明文**（Rust + handler 双测断言） |
| 模板/前缀 UI 可完成 | spec 三字段（token/text_template/prefix）+ 面板「模板怎么写」+ 本地示例渲染（零请求，与 Rust `render_injected_text` 同语义） |
| sidecar 照 README 能播、节流一致 | `--min-interval-ms` 缺省 1000 / `0`=关 与 README 逐条一致；修掉唯一漂移（`--dry-run` 仍连 B 站收弹幕、只是不发 HTTP）；`--selftest` 16 项 OK |

### 3.2 `voice-input`（语音输入）

| 诉求 | 落点 |
| --- | --- |
| backend/locale 说人话 | 面板与 `docs/voice-input.md` §1.3 **逐字同源**：mock = 没有 ASR；sidecar = 外部进程**推**文本（Rust 从不开 socket）；locale 只影响归一化。面板显示当前生效值并区分「缺省 / 已设置 / 配错回落」 |
| sidecar 最小路径（Windows）可复制 | `voice-sidecar/README.md` §3.3 PowerShell + cmd 两段整段可复制（装依赖 → 设 URL/TOKEN → `--selftest` → `--dry-run` → 真发送） |
| 失败码 → 用户可读 | §4.5 + README §5 同源表（退出码 `0/2/3/4/5` 以脚本 `EXIT_*` 为准 + 一句话 + 触发 + 处置）；面板「出错怎么办」5 条（403/busy/sidecar 没起/backend 配错/401） |
| 与 external-input 职责一页说清 | `docs/voice-input.md` §8 对照表（入口/来源/清洗/token/locale/场景/失败码/启停/成功响应） |
| 加分 | 新命令 `selftest`（配置自检，token 只回 bool）；面板「检查配置」 |

### 3.3 `persona`（酒馆角色卡）

| 诉求 | 落点 |
| --- | --- |
| 导入卡 → enable → 人设变 → disable 还原（UI 内） | 面板：**粘贴卡 JSON**（主路径）+「选择卡文件」（`.json` / 带 `chara` 的 `.png`，条件导入注入，浏览器可用）；Mod 命令 `import_card`（`card_json` / `data_base64`，兼容 dataURL）与 `clear_import`；导入卡落 `persona-mod-card.json`（V2 规范化、`tmp+rename`），**优先级高于 config**；写主链失败**回滚** |
| Failed 可读 | 面板在 `failed`/`error` 给「角色卡没被接受，修好再打开开关」+ 日志 `mod: persona`；`mods_api.statusLabel` 补认 `failed`（原来只认 `error`，会露裸英文） |
| 与 memory 共存提示 | 面板 + 文档同一句：两者都写 `system_prompt`，**last-writer-wins（后写覆盖，不合并、不仲裁）** + 可执行建议 |
| 运行态 | 新增 `state_json`（`ready/card_source/card_name/card_format/applied_chars/has_base_snapshot/…`，零 IO 脱敏）→ `GET /mods/persona/state` 由恒 **503 升为 200** |
| 文档 | 新增 [`persona-mod-v0.md`](../architecture/persona-mod-v0.md)（来源优先级 / 逐步 curl / 坏卡路径 / 命令契约） |

### 3.4 `memory`（会话记忆）

| 诉求 | 落点 |
| --- | --- |
| 可见条数 / hits / 清空 | `state_json` 新增 `records`（JSONL 有效条数；不可解析 → `null` 并计 `errors`）；面板显示 条数 / 累计命中 / 注入 / 已淘汰 / 上轮命中 +「清空记忆库」 |
| 清空实现 | `command("clear")` → `JsonlStore::clear()` = 同目录 `.tmp` + `rename` **原子重写为空**（不是 `remove_file`），回 `{cleared,records,removed,residue}`；累计 `writes/hits/injects/evicted` **不重置** |
| 注入可关 | 沿用 spec 的 `enabled_injection`（面板说明它 **≠ Mod 启停**） |
| 可重复验收步骤 | 文档 §12 **七步可复制**（含「注入只对下一轮生效」的时序口径，避免把正常时序当失败）+ 带码排查表；面板指向该节 |
| 与 persona 策略钉死 | 文档 §5.1 + 面板：last-writer-wins，**不引入合并/仲裁** |
| 关键取舍 | `clear` **只清 JSONL、不动 `system_prompt` 残留**；残留交给既有 `strip_residue` 生命周期，并用 `residue` 字段如实提示（有单测锁定） |

### 3.5 `director`（导演）

| 诉求 | 落点 |
| --- | --- |
| enable 后 state 持续有决策 | `state_json` 新增 `latest`（最近一条决策，含 turn/emotion/intent/suggested_tts/closed/delivered）；`recent_decisions` 形状不变（向后兼容有回归） |
| 做实其一：建议 TTS **一等面板** | `DirectorPanel`：本轮情绪/意图/建议语速/建议音高/是否结项 六行（中文 + 稳定码双写）+ 最近决策列表（带 turn id）；空态给「先启用并聊一轮」；另加 `command("clear")` 清账本 |
| **零投递不变** | `delivered:false`、`channel:"none"`；面板顶部固定文案「仅建议，不驱动动作、不影响 TTS 输出」；间谍断言加强：命令通道也不得触碰 `action_tx` / `apply_settings`（`action_calls==0 && apply_calls==0`） |
| 为什么不做 apply-to-TTS | `tts.rs::SpeechRequestBody` 只有 input/model/voice/response_format（**无 speed/pitch**），且 `apply_settings` 写的是持久配置会泄漏到后续轮；要做须先进 core 开 per-request 通道（主链皮肤冻结，本波禁止）——写入 `director-mod-v0.md` §8 |

### 3.6 共享基座（主 agent，先于五轨落盘）

- **一次性命令通道**：`ModRuntime::command(command, args)`（缺省 `ModError::UnsupportedCommand`）→
  `ModRegistry::command` → `ModCommandError{Unavailable|Unsupported|Failed}` →
  **`POST /api/v1/mods/{id}/command`**（body `{command,args}`）：
  `200 ok` / `400` / `404 not_found` / `409 unsupported_command|command_failed` /
  `503 command_unavailable`（未启用或 worker 忙，可重试）；Flutter `ModsApi.command()`。
- **每 Mod 产品面板扩展点**：`settings/mods/mod_panel.dart` + `mod_panels.dart` 注册表 +
  五个 Mod 各一个面板文件（`*_panel.dart`）。**并行五轨各改各的文件，共享文件零冲突**。

## 4. 门禁数字（本轮实跑，全部在 `mod/product-grade` tip 上）

| 检查 | 命令 | 结果 |
| --- | --- | --- |
| Rust 测试 | `cargo test --workspace --all-targets` | **1132 passed / 0 failed** |
| Rust doc 测试 | `cargo test --doc --workspace` | **3 passed / 0 failed** |
| 格式 | `cargo fmt --all -- --check` | **clean**（exit 0） |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning** |
| Rust 占比 | `cargo run -p xtask -- rust-ratio` | **96.4194% PASS**（门槛 95%；69851 / 72445） |
| wasm 渲染面 | `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | **ok** |
| 前端静态检查 | `cd shell/flutter && flutter analyze` | **No issues found** |
| 前端测试 | `cd shell/flutter && flutter test` | **902 passed / 0 failed**（基线 842，+60：director 9 / memory 13 / external 13 / voice 7 / persona 18） |
| 仓库根历史资产 | `python3 -m pytest tests/ -q` | **22 passed, 1 skipped** |
| 语音 sidecar 自检 | `voice_sidecar.py --selftest` | **70 项全过** |
| B 站 sidecar 自检 | `bilibili_sidecar.py --selftest` | **OK（16 项断言）** |

## 5. 真点火实测（本波新增证据，非脚本级）

在本机 WSL2 用**用户自己的** `live2d-ai.toml`（DeepSeek LLM）+ CosyVoice TTS，
以 `--web --http-port 18099` 起本波构建的 binary 实测（不动用户已占用的 18080）：

| 项 | 实测 |
| --- | --- |
| `GET /api/v1/mods` | **5 个 id**：`director / external-input / memory / persona / voice-input`（无 wallpaper / pet） |
| `external-input` state | `{accepts, busy, ready, rejects, token_set, v2_ignored}` |
| 注入一条弹幕 | 200 `ok:true` → `accepts` **1**；`reset_counters` → 回执 `before.accepts=1`，之后 **0** |
| `voice-input` transcript | 200，带全角空格/零宽的文本 → **`把窗户关小一点`**（清洗/归一化） |
| `voice-input` selftest 命令 | 200，回 `backend=mock / locale=zh-CN / opens_network=false / problems=[]` |
| `persona` 导入卡 | 200 `{card_name:星梦, card_format:v1, applied_chars:131}`；`GET /settings` 的 `persona.system_prompt` **非空** |
| `persona` disable → 还原 | `system_prompt_len` **223 → 0**（回到主链基线）；再 enable → 人设重新生效 |
| `persona` 坏卡 | 409 `command_failed` + 可读原因（**不静默**） |
| **两轮真对话** | turn1 后 `memory.records=1/writes=1`；turn2 后 `records=2/hits=1/injects=1`，`memory.jsonl` 真落盘 2 行 |
| `director` 两轮 | `decisions=2`、`turns_ended=2`，`latest={emotion, intent, suggested_tts, closed:true, delivered:false}` |
| `memory` 清空 | 200 `{cleared:true, records:0, removed:2, residue:false}`；文件被**原子重写为空**（0 字节）；累计计数不重置 |
| `command` 失败态 | 未知 id → **404**；未知命令（已启用）→ **409 unsupported_command**；未启用 → **503 command_unavailable**；坏 body → **400**；无 Origin → **403** |

### 5.1 真点火抓到并修掉的两个缺陷（脚本/聚焦测试都没有覆盖）

1. **`local-llm` 方法名撞名**（`cargo test --workspace` 才暴露）：基座新增的
   `ModRuntime::command(&mut self, &str, &Value)` 与已封存 crate 的固有
   `LocalLlmRuntime::command(&self)` 同名 → 编译失败（E0061）。修法：调用点显式
   限定固有方法。**教训**：聚焦测试只编译各自 crate，全量门禁不能省。
2. **memory 配置路径锚点**（真链路才暴露）：`live2d-ai.toml` 在 cwd 时
   `config_path_for_web()` 返回**裸文件名**，而它是所有同目录派生物的锚点；
   `Path::parent()` 为空 → `resolve_store_path` 过滤空目录 → `None` →
   **memory 显示「运行中」却一条都记不住**（`records=null`、`errors` 增长）。
   修法：配置路径**统一绝对化**（`anchor_config_path`）并加回归。
   **教训**：`state_json` 的字段与实际写盘要对着真链路看一遍——「运行中」不等于「在干活」。

## 6. 给用户的最短体验路径（不含壁纸 Mod / 桌宠）

```bash
# 0) 只在 WSL2 里跑（唯一进程宿主）；Windows 只开浏览器
cd /home/skystar/Live2D-Ai-product

# 1) 起服务（首次用 --build：会构建 wasm 渲染面 + Flutter Web）
./scripts/ignite.sh --build        # 默认 18080

# 2) 另一个 WSL2 终端：机器预检（期望 FAIL 0）
./scripts/ignition-precheck.sh --fsm

# 3) Windows 浏览器打开 http://127.0.0.1:18080/app/
#    设置 → Mod：
#      external-input（已开）→ 展开看计数；curl 注入一条看已接受 +1；点「重置计数」
#      voice-input   → 打开 →「检查配置」；curl POST /voice/transcript 看清洗结果
#      persona       → 打开 → 粘贴卡 JSON →「导入并生效」→ 聊一轮 → 关开关看人设还原
#      memory        → 打开 → 说「记住：我叫星梦，喜欢薄荷」→ 下一轮看条数/命中 →「清空记忆库」
#      director      → 打开 → 聊一轮 → 看一等决策面板（仅建议，不驱动动作/TTS）
```

完整逐步清单（操作 / 期望 / 失败先看哪）：
[`IGNITION-CHECKLIST-product-grade.md`](IGNITION-CHECKLIST-product-grade.md)。

## 7. 未决 / 已知债（原样登记，不假装已解决）

1. **未 push、未打 tag、未 bump、未写 release 说明**——按用户裁决。
2. **人眼/Win 验收仍需用户本人**：本轮 agent 证据到「HTTP 级真链路 + 真 LLM/TTS 两轮对话」，
   `/app/` 的浏览器观感（舞台、口型、面板点击）**尚未由用户过目**。
3. **`/app/` 与 `/render` 产物在本 worktree 未重建**（gitignore）：点火实测只打到 API；
   用户跑 `./scripts/ignite.sh --build` 会自动补。
4. **文件行数约定**：Rust 源码 ≤500 行（豁免 ≤1000 需头注理由）在本仓**早已大面积超线**
   （23 个 `src/*.rs` > 500，其中 `mod_registry.rs` 1145、`external_routes.rs` 1003 > 1000）。
   本波**已如实登记、未做拆分**——它不是门禁项，且拆分会同时动到刚合并的五轨。
5. **memory `clear` 不剥离 `system_prompt` 残留**（刻意，见 §3.4）；有 `residue` 字段兜底提示。
6. **director 无 apply-to-TTS**（无 per-request TTS 参数通道，见 §3.5）。
7. **persona 导入卡优先级高于 config**：手改 `card_json` 后若曾导入过卡，需先「清除导入卡」。
8. **external 计数是进程级**（不持久化，重启归零）；**voice-input 无 `state_json`**（面板读 config，刻意）。
9. **共享 `CARGO_TARGET_DIR` 的 stale rlib 陷阱**：多 worktree 共用 target 时，
   聚焦测试可能链到别的 worktree 的旧 rlib（外/内两轨都碰到过）。收束时以
   `cargo clean -p <crate>` + 全量门禁为准；建议后续给并行轨各配独立 target。

## 8. 归档点 / 回滚

- **集成 tip**：`mod/product-grade`（本文档所在提交之后不再追加功能）。
- **基座提交**：`5a3aa77b`（封存）+ `72d1af17`（命令通道/面板基座）+ `c26f4803`（清单/预检）。
- **五轨 merge**：`4d7fba5d`（director）/ `4784376b`（memory）/ `85b379bb`（external）/
  `98adda7f`（voice）/ `0a2a72aa`（persona）；收束修复：`c7c27d0a`（撞名）/ `803129c0`（配置锚点）。
- **各轨分支仍在**：`mod/pg-{external,voice,persona,memory,director}`（worktree 保留在
  `/home/skystar/Live2D-Ai-pg-*`），可整体丢弃或单独回滚。
- **未 push**：以上全部只存在本地。

## 9. 相关文件

- 封存台账：[`../architecture/ARCHIVED-mods.md`](../architecture/ARCHIVED-mods.md)
- 点火清单：[`IGNITION-CHECKLIST-product-grade.md`](IGNITION-CHECKLIST-product-grade.md)
- 契约：[`../external-input.md`](../external-input.md)、[`../voice-input.md`](../voice-input.md)、
  [`../architecture/memory-mod-v0.md`](../architecture/memory-mod-v0.md)、
  [`../architecture/persona-mod-v0.md`](../architecture/persona-mod-v0.md)、
  [`../architecture/director-mod-v0.md`](../architecture/director-mod-v0.md)
- Mod 产品链路 / 许可：[`../architecture/mod-product-chain.md`](../architecture/mod-product-chain.md)、
  [`../architecture/mod-community-license.md`](../architecture/mod-community-license.md)
