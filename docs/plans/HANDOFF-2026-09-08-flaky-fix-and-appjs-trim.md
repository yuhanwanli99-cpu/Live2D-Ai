# 交接：flaky 测试加固 + app.js 减重（2026-09-08 续作）

> 本轮为上一会话（`HANDOFF-2026-09-08-render-tiers-doc-consistency.md`，渲染档位落地）的**续作**。
> 承接两件遗留技术债：① `live2d-ai-runtime` 的 flaky 集成测试；② app.js 减重。
> run 分支：`piagent/run/20260908-201956-lp8l`（**未 push**，共 9 笔提交，自 base `63b1e542`）。
> 回滚锚点链：`a99f171a`（HEAD）→ `280a25db` → `52280d25` → `7306c3cd`（上一会话）…… → base `63b1e542`。

---

## 1. 本轮完成（按 commit）

### `52280d25` — 修复 flaky 测试（根因定位 + 实证）✅ 已解决
- **现象**：`live2d-ai-runtime/tests/conversation_engine_lifecycle.rs::tool_call_becomes_tool_action_event`
  间歇性 FAIL（隔离跑 ~50%），与 l2d 改动无关（main 上同样抖动，已复核）。
- **失败信号**：`Error { kind: Tts(Http(reqwest::Error { source: hyper::Error(IncompleteMessage) })) }`。
  第 1 句 TTS 成功、第 2 句 TTS 请求报 `IncompleteMessage` → 引擎置 `TurnStatus::Failed`。
- **根因**：`OpenAiClient` 用默认 reqwest（带连接池）。池化复用了 mock 服务器已
  `connection: close` 的半关闭连接，下一个请求读到 hyper `IncompleteMessage`。
  **这是 mock 与 reqwest 池化的经典竞态**，不是引擎逻辑 bug。
- **修复**：`crates/live2d-ai-runtime/src/lib.rs::OpenAiClient::new` 的 reqwest 构建器加
  `.pool_max_idle_per_host(0)`（禁空闲连接复用）。
- **已排除的假设**（均实测无效）：① 片间 sleep 10ms 改 0（16/40 更差）；② 去 `connection: close` +
  全关 socket（18/40）；③ 换用 `#[serial]` 不适用（隔离跑也抖，非跨测试争用）。
- **实证**：目标测试 **50/50 通过**；runtime 全量测试无回归；fmt/clippy 干净。
- **生产影响评估**：LLM 流每轮本就单连接、TTS 按句请求，桌面端多一次 TCP/TLS 握手**可忽略**；
  streaming 响应本就不参与池化复用。已加注释说明动机。

### `280a25db` — app.js 安全减重 `943 → 921`（行为等价）🔶 部分
- 删死代码 `toggleSettings`（grep 确认 index.html / chat.js 均未引用）。
- 压缩冗余多行注释为单行（保留关键告警：`cmdId` 映射 / wasm 只收 String 需 JSON.stringify /
  `apply_status` 的 snake_case）。
- 新增模块级 `setIf(el, v)`（元素存在且未聚焦才赋值），统一 `renderAll` 的
  llm/tts key-env、systemPrompt、persona 五字段赋值——**行为完全等价**。
- 验证：`node --check` 通过；desktop `489` 测试全绿（含 `tests_html` 锁 id）。
- **注意（回退的尝试）**：曾重构 `buildPatch` 的 persona 五字段为局部 `put()` helper（省 ~13 行），
  但触犯了 `index_html.rs::index_html_settings_wiring_controls_ok` 的行为守卫锚点
  `persona.name = nv === "" ? null : nv`（防"空串=null 清除"语义被破坏）。鉴于该测试是
  刻意守卫，**已回退**该重构，只保留 renderAll 侧的安全重构。

### `a99f171a` — 交接文档更新
- `docs/plans/HANDOFF-2026-09-08-render-tiers-doc-consistency.md`：把 §3.4 flaky 改为「已修复」、
  §4.1 技术债改为「已处置 + 剩余缺口」。

---

## 2. 门禁状态（已复核，全绿）

- `cargo test --workspace --all-targets` **连续 2 次全绿**（无 FAILED；flaky 修复后稳定）。
- `cargo fmt --all --check`  干净
- `cargo clippy --workspace --all-targets -- -D warnings`  干净
- `cargo test --doc --workspace`  通过
- `cargo run -p xtask -- rust-ratio`  **95.31%**（≥95%）
- `node --check app.js`  通过
- README/AGENTS Mod 计数一致 = 5；渲染档位下拉 `#appRenderTier` 存在且 tests_html 锁定。

---

## 3. 踩过的坑 / 反馈

1. **行为守卫锚点会用「代码字面量」锁语义**：`index_html.rs` 用 `APP_JS.contains("persona.name = ...")`
   做守卫。重构即使行为等价，也会因字面量消失而 fail。改这类代码前先查 `index_html.rs` 的锚点。
2. **mock 与 reqwest 池化竞态**是关键坑：flaky 信号是 `hyper::Error(IncompleteMessage)` + 下一个请求
   失败。修法是客户端禁池化，而非改动 mock 的延迟/关闭方式（实测后两者无效）。
3. **app.js 减重是真功能瘦身**：943 行里 ~845 是真实代码行（22 空行 + 76 注释），删光注释/空行也只到
   ~845；要达 ≤760 必须删 ~85 行功能代码，会触及配置面板的 25+ 函数，风险高。

---

## 4. 未做完 / 后续工作

### 4.1 本轮未闭环（仍属待办）
- [~] **app.js 仍 >760 预算**（当前 921 行）：已安全瘦身 -22 行，但**仍差 ~161 行**。
      若要继续压，需**独立重构轮**并先建行为基线（现有 `tests_html` 锁 id 可兜底），
      且需处理 `index_html.rs` 的字面量锚点。**不建议**在本轮强行删功能代码。
- [ ] 追加 app.js 相关的**行为级测试**（不仅字符串锚点），以便安全重构：目前 `tests_html`
      只锁 DOM 模板/id，不验 JS 行为，重构易隐性破坏 UI。

### 4.2 外部验收项（承上会话，需真机/浏览器，不 fake 视为完成）
- [ ] S3 原生 16384 离屏 target 实例（需独立 GPU 实测）。
- [ ] S4 档位视觉验收（浏览器 `/render`，切换档位后清晰度/回退不 crash）。
- [ ] F6 端到端（真实 LLM/TTS → 出声 → 口型动）。

### 4.3 与既有 roadmap 衔接
- 纹理档位（ADR §3）已从「文档先行」推进到「接口已落地、真机验收仍属外部」。
- doc-实现一致性门禁已补上（锁 Mod 计数 = 5）。

---

## 5. 关键文件索引
| 域 | 路径 |
|---|---|
| flaky 修复 | `crates/live2d-ai-runtime/src/lib.rs`（`OpenAiClient::new` 加 `.pool_max_idle_per_host(0)`）|
| flaky 测试 | `crates/live2d-ai-runtime/tests/conversation_engine_lifecycle.rs`（`tool_call_becomes_tool_action_event`）|
| mock 基建 | `crates/live2d-ai-runtime/tests/common/mod.rs`（`spawn_multi_server`/`respond_pieces`/`spawn_tts_mock`）|
| app.js 减重 | `crates/live2d-ai-desktop/src/web_api/app.js`（`setIf` 辅助 / renderAll 统一赋值）|
| 行为守卫测试 | `crates/live2d-ai-desktop/src/web_api/index_html.rs`（`index_html_settings_wiring_controls_ok`）|
| 本会话交接 | `docs/plans/HANDOFF-2026-09-08-flaky-fix-and-appjs-trim.md`（本文件）|
| 上一会话交接 | `docs/plans/HANDOFF-2026-09-08-render-tiers-doc-consistency.md` |

---

## 6. 回滚
- 整体回退：`git reset --hard 63b1e542`（回到本轮 base，丢弃本轮全部提交）。
- 单步回退：按 §1 锚点链逐个 `git revert`（每 commit 均为最小、独立语义）。
- run 分支未 push；建议并入 `main` 后删除该分支及 `piagent/task/20260908-201956-lp8l/*` 临时分支。
