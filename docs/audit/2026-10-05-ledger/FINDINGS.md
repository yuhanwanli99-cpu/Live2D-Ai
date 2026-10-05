# FINDINGS.md — 去重后的全部发现
> 格式见任务书 §5 键值块。编号 `F-<批次四位>-<序号>`。
> 已登记不重复：前端 45 条（docs/plans/TRIAGE-0.2.0-audit-45-2026-09-28.md）、
> Mod 侧 S1–S6 / M1–M12 / L1–L16、P-1…P-15（见 §9 登记清单）。

（暂无）

---

## BATCH-0001（2026-09-28）

### F-0001-01 · P1
> 🧪 **B0269（Phase 4 证伪）攻它：机制存活，但我的**措辞过宽**，须收窄**
> 攻法：AGENTS 说「请求级日志在 `dispatch.rs`」⇒ 若那条日志**足够早**，本条就该被推翻
> ```
> dispatch.rs:62  let route = match_route(method, path);      // ← **先路由**
> dispatch.rs:65  if is_mutating_route(route, method) { … }
> dispatch.rs:76    tracing::warn!(…)
> dispatch.rs:88  let resp = match route { … }
> ```
> ⇒ **路由先于日志**；而 B0198 已核那条日志是**按状态阈值**发的
> （**≥500 error / ≥400 warn / mutating 成功 info / 其余 debug**）
> ⇒ ⇒ **它必然发生在「响应已知」之后** ⇒ ⇒ **纯前驱失败（连响应都没形成）才真的没有记录**
> ⇒ ⇒ ⭐ **我原来的措辞「pre-dispatch endpoints 产生零条日志」**过宽**：
> **这四个路由只要返回了响应（哪怕是 4xx/5xx），`dispatch.rs` 仍会记它**
> ⇒ ⇒ **收窄后的准确表述**：
> 「**这四个路由自身不带任何 `tracing::` 调用** ⇒ **凡是没能形成响应的失败**
> （panic / 静默提前返回）**就只出现在 dispatch 层的空缺里**」
> ⇒ ⇒ **级别维持 P1**（那正是「排障时一片空白」的情形，AGENTS 明令禁止），
> 但**范围从「这些端点」收窄为「这些端点的**无响应**失败」**
file: crates/live2d-ai-desktop/src/web_api/mod.rs:516-557
摘录: `if let Some(resp) = crate::web_api::external_routes::handle_external_chat(...) { let _ = request.respond(resp); continue; }`（mod.rs:516-527）；`mods_routes::handle_mods_route` 同款（mod.rs:547-557）。唯一的请求日志在 dispatch 末尾：`log_request_outcome(method, path, route, &resp);`（dispatch.rs:217），前置分支全部 `continue` 掉了它。
调用链: mod.rs:445 run_request_loop → mod.rs:502 handle_chat_session / :516 handle_external_chat / :532 handle_voice_transcript / :547 handle_mods_route → mod.rs:510,525,540,555 `respond` + `continue`（绕过 dispatch.rs:217）→ 落盘日志零记录
影响: 可维护性/排障（用户可见故障不可观测）；`POST /api/v1/external/chat` 是 rc.1 起的直播刚需入口
建议: 前置分支复用同一套分级日志（≥400 warn / mutating 成功 info），或把 respond 收口到一个 `respond_and_log` 包装，四处前置路由统一走它
验证: `grep -c "tracing::" crates/live2d-ai-desktop/src/web_api/{chat_routes,external_routes,voice_routes,mods_routes}.rs` → 0 0 0 0；`grep -c "println!\|eprintln!"` 同四文件 → 0 0 0 0；`grep -n broadcast external_routes.rs mods_routes.rs` 仅命中两处 `#[cfg(test)] mod tests` 内的 `dummy_ctx`（mods_routes.rs:520、external_routes.rs:1042），生产路径不广播 error 帧
置信: 高
反证: (a) 找模块内部自记日志——tracing/println/eprintln 三种方式全部 0 命中；(b) 找 mod.rs 在 respond 前后补日志——`run_request_loop`（mod.rs:445-568）全文无任何日志语句，`log_request_outcome` 是 dispatch.rs 的私有 fn，mod.rs 根本调不到；(c) 找 WS error 帧兜底——external/mods 生产路径不构造 Broadcaster，故失败既不进日志也不进 WS。这正是 AGENTS.md 明令「禁止新增只 println! 的错误路径（不进文件 sink = 排障时一片空白）」的同族缺口，且这四族端点是 2026-08-30 之后新增的、晚于 2026-09-11 那次「请求级日志」补齐。

### F-0001-02 · P3
> 🔻🔻 **B0375 重大更正：本条从 P1 降为 P3**（**我记错了主体**）
> `model_root.rs:106-120` 逐字：
> ```rust
> #[test]
> fn model_root_walks_up_to_the_repo_root() {
>   let root = model_root();
>   assert!(
>     root.join("bai/runtime/bai.model3.json").is_file() || !root.join("bai").is_dir(),
>     "向上找到的根应含内置模型的 model3.json：{}", root.display());
> ```
> ⇒⇒⭐⭐⭐ **它在 `#[test]` 里** ⇒ ⇒ **它是一条「测试断言」，不是「生产校验」**
> ⇒⇒ 而 **B0374 刚核过**：生产路径在 `models_routes::handle_import` 用 `find_model3_json`
> 做了**正确的四段判定**（`invalid_model3_json` + 文案）⇒⇒ **生产侧根本没有这个判据**
> ⇒⇒⇒ **我记的「整个判据形同虚设、导入可能静默通过」** —— **生产侧不存在这条路径**
> ⇒⇒ **实际影响**：**这条测试失去了它要证明的能力**（`model_root()` 是否找对了根），
> **而不是**「用户会导入失败」⇒⇒ **降 P3**
> ⇒⇒ **修法也随之变简单**：去掉那个 `||`（让 `bai` 不存在时**测试失败并暴露真相**），
> 或该测试随 `bai` 被移除而一并删除 ⇒⇒ **一行 / 一删**
> ⇒⇒ ⭐⭐⭐ **可提炼（本条最大的价值）**：
> > **在测试里发现的空洞，默认要问「它在守什么」。**
> > 若它守的判据**在别处已被正确实现**，那是**测试的缺陷**，不是**系统的缺陷**
> ⇒⇒ **级别要按「谁受害」定，不是按「代码多丑」定。**

> ➕ **B0374 补「对的那一处」**：同一判据，**路由层做对了、断言层做漏了**
> `models_routes/handlers.rs:138-166` `handle_import` 是**五道关卡 / 四个码**：
> `invalid_payload`（JSON 解析）· `invalid_resource_path`（id 非法 · 路径解析失败 · 目录不存在）·
> **`invalid_model3_json`（`find_model3_json` 返回 None）** + 文案「目录 X 下找不到 `*.model3.json`」
> ⇒ ⇒ **用户会看到**。而 `model_root.rs:111` 的 assert 在 `bai` 不存在时**空洞通过** ⇒ **守不住任何东西**
> ⇒ ⇒⭐⭐ **这让本条的修法收窄到一行级**：**路由层已经对，只需把 `model_root.rs:111` 那条 assert
> 换成同样的判定、或直接删掉**（它的职责已被路由层承担）—— **不是设计改动，是一行改动**
> ⇒ ⇒ ⇒ 我此前只记了「错的那处」；**本批补上了「对的那处」**，而结论本身不变、**证据变强**

> 🧪 **B0271（Phase 4 证伪）攻它：前提**对着当前工作树重新核过**（不是凭记忆）**
> 攻法：若 `assets/models/bai` **现在存在** ⇒ 那个 `||` 的后半段就变成承重的 ⇒ 本条该推翻
> ```
> test -e assets/models/bai   ⇒ **不存在**
> ls assets/models            ⇒ 只有 README.md
> git ls-files assets/models  ⇒ 只有 .gitkeep 与 README.md
> ```
> ⇒ ⇒ **273 批之后**（B0001 记的「不存在」）**仍然成立** ⇒ **攻击失败**
> ⇒ ⇒ **该 `||` 的后半段在当前树上依然是空的** ⇒ **`assert` 实际从未校验过那张 model3.json**
> ⇒ ⇒ ⭐ **顺带一条独立正面**：`git ls-files assets/models` **只有 README 与 .gitkeep**
> ⇒ ⇒ 与 AGENTS「模型由用户合法导入、`assets/models/` **不捆绑二进制**」**完全一致**
> ⇒ ⇒ 这是「不捆绑二进制」这条红线的**独立正面证据**（**不是**因为本条而来的）
> ⚠ **如实标注**：我**差点直接引用**「B0001 记过不存在」⇒ **那正是 CONSOLIDATION-18 立的规矩
> 禁止的事**（**关于世界的���实会过期**）⇒ ⇒ 本批的**收益本身就是那次教训的兑现**
file: crates/live2d-ai-desktop/src/web_api/model_root.rs:111-113（及 :93-104）
摘录: `assert!(root.join("bai/runtime/bai.model3.json").is_file() || !root.join("bai").is_dir(), "向上找到的根应含内置模型的 model3.json：{}", root.display());`
调用链: `model_root()` 定义 model_root.rs:46 → `registry_path()` :64 / `builtin_model3_path()` :79 / `wasm_assets.rs:216 assets_models_dir()` → `handle_assets` :181 `/models/*` 静态读盘 → `mod.rs:480 run_request_loop`
影响: 可维护性（假绿灯：rc.2「唯一模型根」裁决没有真回归保护）
建议: 断言改为「等于向上找到的仓库根」而不是「以 assets/models 结尾」；或注入根目录参数做纯函数测试
验证: `ls -la assets/models/` → 只有 `.gitkeep` + `README.md`，**无 `bai/`** ⇒ 第二个析取项 `!root.join("bai").is_dir()` 恒真，第一条断言与根指向哪里无关
置信: 高
反证: 试过构造让它变红的输入——把 `model_root()` 改成返回任意 `<x>/assets/models`（例如 `crates/live2d-ai-desktop/assets/models`，即 direct 回退分支）：断言一 `root.ends_with("assets/models")` 恒真；断言二 `registry_path().starts_with(&root)` 恒真（registry 恒由 root 拼出）；`model_root_walks_up_to_the_repo_root` 的 `root.parent().ends_with("assets")` 仍恒真，bai 仍不存在故第一析取项走 fallback。**两条测试在任何根下都全绿**，结构上不可能失败。

### F-0001-03 · P2
file: crates/live2d-ai-desktop/src/web_api/wasm_assets.rs:234
摘录: `PathBuf::from("/home/skystar/deepseekharness/Live2D-Ai/crates/l2d-wasm-demo/dist"),`
调用链: `wasm_dist_dir()` :229 → `handle_assets` :150（`/render` 扫 hash）与 :166（`/render/<file>` 读盘）→ `mod.rs:480`
影响: 正确性/可维护性（cwd 不在仓库根时静默加载另一棵 checkout 的过期 wasm 产物）
建议: 删掉该绝对路径候选；改用与 `model_root()` 同口径的 `ancestors()` 向上查找，找不到就返回 `None` 并让 `/render` 回 503 `wasm_unavailable`
验证: `ls -d /home/skystar/deepseekharness` → EXISTS（本机确实存在这棵树，故第三个候选会 `is_dir()` 命中）
置信: 高
反证: 试过论证「前两个候选恒命中」——candidates 前两项是相对 cwd 的 `crates/l2d-wasm-demo/dist` 与 `../crates/l2d-wasm-demo/dist`；AGENTS.md 只保证 `scripts/ignite.sh` 把 cwd 固定在仓库根，而 `cargo run -p live2d-ai-desktop` / 从 `crates/` 下启动时两者皆空 → 落到第三个绝对路径。`--web` 主路径不受影响（ignite 固定 cwd），故不是 P0。

### F-0001-04 · P2
file: crates/live2d-ai-desktop/src/web_api/mod.rs:585
摘录: `String::from_utf8(buf).unwrap_or_default()`
调用链: `read_body_from_request()` mod.rs:579-586（`take(MAX_BYTES)` 1 MiB 上限）→ mod.rs:498 `body_str` → dispatch.rs:66 `let body_present = !body_str.is_empty();`（CT 校验随之跳过）→ settings_routes/mod.rs:265 `PatchBody::parse("")` → :269-270 `tracing::warn!(code="invalid_payload", ...)` + 400 `JSON 解析失败: EOF while parsing a value`
影响: 用户可见（>1 MiB 或非 UTF-8 的设置补丁报「JSON 解析失败」，真实原因「超过 1 MiB 上限」在响应与日志里都不可见）
建议: `from_utf8_lossy` 或先判 `buf.len() == MAX_BYTES` 时显式回 413/400 `payload_too_large` 并带 code
验证: 需门禁验证——需起真实服务发一个 >1 MiB 的 `PATCH /api/v1/settings`（本任务禁跑 cargo/起服务，故未实测）
置信: 高（代码行为）/ 中（用户可见后果的可达性）

### F-0001-05 · P2
file: crates/live2d-ai-desktop/src/web_api/security.rs:94-95；crates/live2d-ai-desktop/src/web_api/mod.rs:350-351
摘录: `#[allow(dead_code)] // 由 ws::handle_ws_request 调用；当前未在 mod.rs 装配。`（security.rs:94）；`#[allow(dead_code)] // 由 cli_entry::run_web_mode 装配时调用。`（mod.rs:350）
调用链: security.rs:95 `check_ws_origin` → 真实唯一调用点 `ws.rs:95`（`check_request_origin` 内）；mod.rs:351 `with_security` → 真实调用点只有 tests_ws.rs:281,892 / tests_security_e2e.rs / chat_routes.rs:718（测试），生产由 mod.rs:428-433 `start_server` 直接覆盖 `SecurityContext::new(port, allow_no_origin)`
影响: 可维护性（安全接缝的「装配状态」注释与事实相反；`#[allow(dead_code)]` 会让「安全函数失联」不再触发编译告警——删掉 ws.rs:95 的调用也不会报 unused）
建议: 删掉两处 `#[allow(dead_code)]` 与错误注释；确属未接线时用注释说明真实装配点
验证: `grep -rn "check_ws_origin|with_security" crates --include=*.rs`（见 BATCH-0001 命令清单）
置信: 高
反证: 试过找第二个 `check_ws_origin` 调用点（生产装配在别处）——grep 全 crates 只有 ws.rs:95 一处；试过找 `with_security` 的生产调用（cli_entry 真的用它装配）——grep 全 crates 无 cli_entry 命中，cli_entry 走 `start_server` 内部覆盖。

### F-0001-06 · P3
file: crates/live2d-ai-desktop/src/web_api/dispatch.rs:160 与 :165
摘录: `let parsed: TestBody = serde_json::from_str(body_str).unwrap_or_default();`
调用链: dispatch.rs:160/165 → settings_routes/test_endpoints.rs:47 `handle_test_llm` / :59 `handle_test_tts` → `run_llm_test(current, body.timeout_ms)`
影响: 可维护性/文案一致性（坏 JSON 静默降级为默认 3000ms 静默超时，客户端请求的 8000ms 被丢弃 → 上游偏慢时自检报 `timeout`，即「自检说谎」同族）
建议: 解析失败回 400 `invalid_payload`（与 `handle_patch` 的口径一致，settings_routes/mod.rs:269 已有先例）
验证: 读 test_endpoints.rs:38-42 确认 `TestBody` 只有 `timeout_ms: Option<u32>` 一个字段，其余字段无副作用
置信: 中（可达性低：正常客户端不会发坏 JSON）

### F-0001-07 · P3
file: crates/live2d-ai-desktop/src/web_api/wasm_assets.rs:140-147
摘录: `if path.starts_with("/models/") || path.starts_with("/actions/") || path.starts_with("/render") { return Some(method_not_allowed(path)); }`（非 GET 分支）——而 GET 分支下 `/renderx` 不匹配 `== "/render"` 也不匹配 `starts_with("/render/")`，落到 :209 `None`
调用链: mod.rs:480 `handle_assets` → 非 GET `/renderx` 回 405；GET `/renderx` 落到 dispatch → mod.rs:251 `RouteId::NotFound` → 404
影响: 可维护性（同一路径族 GET/非 GET 落两个不同状态码；仓库自己的回归 tests_mod.rs:397 恰恰断言 `handle_assets(&Method::Get, "/renderx").is_none()`）
建议: 非 GET 判定改成 `== "/render" || starts_with("/render/")`，与 GET 分支同款前缀
验证: 读 wasm_assets.rs:138-148 与 :209；GET 侧断言见 wasm_assets.rs:397
置信: 高

---

## BATCH-0002（2026-09-28）

### F-0002-01 · P1
> 🧪 **B0266（Phase 4 证伪）攻它：机制存活，但**触发条件与影响面被收窄并钉死**
> 攻法：找 `supervisor_opt` 的产出，以及是否存在**第二个** watcher 创建点。
> ```
> cli_entry.rs:119  let supervisor_opt = if std::path::Path::new(&config_path).is_file() { … };
> ```
> ⇒ **`supervisor_opt` 为 `None` ⟺ `live2d-ai.toml` 尚不存在**（第一次运行）
> ⇒ 而 `grep -rn "file_watcher|FileWatcher" crates/ --include=*.rs | grep -v test` ⇒
> **唯一的构造点是 `cli_entry.rs:337`，且就在那个 `if let Some(sup)` 块内**
> ⇒ ⇒ **没有第二个 watcher**（与 B0120 问的「别处有没有做 toml 那件事」同答案：**没有**）
> ⇒ ⇒ **本条的机制成立**，但**影响面是「窗口」而非「常态」**：
> ① 只在**第一次运行**（toml 尚未存在）② 该次运行里**只影响 `.env`**（toml 本身无内容可重载）
> ③ **自愈**：一旦 toml 落盘、下次启动建出 supervisor ⇒ 监视即生效
> ⇒ ⚠ **据此收窄我的措辞**：原文写「首跑无热重载」**过宽** ⇒
> **准确表述是「第一次运行（`live2d-ai.toml` 尚不存在）时，`.env` 的改动要等到下次启动才生效」**
> ⇒ **级别仍维持 P1**（用户可见：改完 key 不生效且无任何提示，直到重启），
> 但**从「无热重载」降为「首跑窗口内无热重载」** —— ⭐ **这正是 Phase 4 想要的结果：机制更硬、措辞更准**。
> **反证（试过并排除）**：(a) 「也许别处会补一次 `refresh_from_disk`」——**未核**
> （PATCH 路径是 UI 保存时的**显式调用**，**不是**文件监视）⇒ 记为敞口；
> (b) 「首跑用户不会改 `.env`」—— **不成立**（首次配置流程**正是**「建 toml + 写 key」这一步，
> **顺序上极可能正好撞上这个窗口**）⇒ **危害不是假设，是流程默认路径** ⇒ **反而加重了它**。
file: crates/live2d-ai-desktop/src/web_api/cli_entry.rs:334-362
摘录: `let _file_watcher = if let Some(sup) = &supervisor_opt { ... } else { None };`（cli_entry.rs:334 / 360-362）；而 `supervisor_opt` 的两个 None 分支是 `Err(e) => { eprintln!("[warn] web supervisor 装配失败，chat 端点将回 503: {e}"); ...; None }`（cli_entry.rs:155-161）与 `} else { println!("web: 无配置文件，跳过 supervisor 装配（纯控制平面）"); None }`（cli_entry.rs:163-166）
调用链: 启动 cli_entry.rs:119 `if std::path::Path::new(&config_path).is_file()` → 两条 None 分支 → cli_entry.rs:334 `_file_watcher = None`；此后用户走 UI 首次配置：`PATCH /api/v1/settings` → dispatch.rs:130 `ctx.ensure_supervisor_after_patch()` → supervisor_slot.rs:126 `build_web_supervisor` 成功 → :142 `self.set(handle)`（**supervisor 活了**）——但 file_watcher 不会补装，全 crates 仅 cli_entry.rs:337 一处 `watch_config` 调用点
影响: 用户可见/可维护性（外部手改 `live2d-ai.toml` 或 `.env` 永不热重载：改了没反应、「改完 key 还是 401」——正是 rc.2 立这条红线要防的回归）
建议: 让 file_watcher 的装配条件与 supervisor 的**运行时事实**对齐（首次 `ensure_after_patch` 成功时补装），或改为常驻监听目录、reload 时才去槽位取当前 supervisor
验证: `grep -rn "FileWatcher|watch_config" crates --include=*.rs` → 定义 4 处 + **调用仅 cli_entry.rs:337 一处**；`grep -n "supervisor_opt" cli_entry.rs` → 119/174/312/334/368/373
置信: 高
反证: (a) 试过找「稍后补装 watcher」的路径——全 crates 无第二处 `watch_config`；(b) 试过论证 supervisor 也不可能活——`supervisor_slot.rs:126-143` 明确在 PATCH 后动态创建并 `set` 进槽位，返回 `ApplyStatus::Applied`，前端据此显示「已热重载」；(c) 试过论证 UI 写 `.env` 也不受影响——确实不受影响（`PUT /api/v1/env` → `secrets::write_key` → `refresh_from(path)` → `ensure_supervisor_after_patch`，secrets.rs:219），所以**坏的只是「外部手改」这一条**，而那正是 file_watcher 存在的唯一理由与 AGENTS.md 明写的红线。

### F-0002-02 · P1
file: crates/live2d-ai-desktop/src/web_api/supervisor_slot.rs:172-204
摘录: `#[test] fn slot_lifecycle_set_get_take() { let slot = SupervisorSlot::new(); assert!(slot.try_get().is_none(), "新槽位应空"); assert!(slot.take().is_none(), "空槽位 take = None"); }`（:176-178）——测试名承诺「空 → set → try_get → take」，函数体里**没有 set、没有 try_get**。第二条测试自己写明了失效原因：`// SupervisorSlot::new() 走自己的 RwLock // 与测试构造的 inner 无关——本测试仅确认 API 自身对毒化容错。`（:193-194）
调用链: 被测契约 = `SupervisorSlot::set`（mod.rs:343 `with_supervisor` 用）→ `try_get`（dispatch.rs:177 chat 路径用）→ `take`（mod.rs:398 `take_supervisor_for_reclaim` 用）；这三条在 :172-204 全部**未被验证**——`slot_poisoned_does_not_panic` 虽调了 `slot.set(test_handle())`，但注释明说「此处**不**断言 set/take 的副作用」
影响: 可维护性（假绿灯：槽位生命周期与锁毒化降级两条契约都没有真回归，`slot_lifecycle_*` 只有构造器语义）
建议: `slot_lifecycle_set_get_take` 补 `set → try_get().is_some() → take().is_some() → take().is_none()`；毒化测试改成对 `SupervisorSlot` 自己的 RwLock 注入毒化（或给 `SupervisorSlot` 加 `#[cfg(test)] fn from_inner`），否则改名自述它只测构造器
验证: 读 supervisor_slot.rs:167-233 全文；对照同批 flutter_app.rs:349-357 `repo_root_skips_the_crate_manifest` 是真测试（有独立输入 + 真实断言），差距明显
置信: 高
反证: 试过论证「毒化测试其实有效」——毒化的 `RwLock<Option<Arc<()>>>` 与 `SupervisorSlot.inner` 类型不同且不是同一个对象，`SupervisorSlot::new()` 造的是全新未毒化锁，故 `try_get()` 返回 None 与毒化与否无关；把 `try_get` 实现改成 `unlocked()` 本测试同样绿。

### F-0002-03 · P2
file: crates/live2d-ai-desktop/src/web_api/app_routes.rs:292-295
摘录: `has_api_key: p.api_key_env.as_deref().is_some_and(|n| !n.trim().is_empty()),` ——而同一次响应里 llm/tts 走的是 `dto::llm_status_from(settings, env_lookup)`（app_routes.rs:273-274），其口径为 `let has_api_key = settings.llm.api_key_env.as_deref().map(|n| !n.is_empty()).unwrap_or(false) && env_lookup(settings.llm.api_key_env.as_deref().unwrap_or("")).map(|v| !v.is_empty()).unwrap_or(false);`（dto.rs:244-252）
调用链: 定义 app_routes.rs:284 `performance_status` → app_routes.rs:277 `performance: performance_status(settings, ctx)` → app_routes.rs:262 `dto::AppStatus` → `handle_status` :226 → `GET /api/v1/app/status`
影响: 用户可见/契约一致性（声明了 `api_key_env` 但 `.env` 里没有该键时，llm/tts 报 `has_api_key=false` 而 performance 报 `true`——同一屏三个密钥状态两套口径，AGENTS.md/dispatch.rs:115-117 定的口径是「声明了键名**且**值真的读得到」）
建议: performance 段同样接 `env_lookup`（`build_status` 已有该形参，直接传下去）
验证: 读 dto.rs:240-258 与 app_routes.rs:284-314 对照
置信: 高
反证: 试过论证「performance 段本来就不需要值校验」——`build_status` 已把 `env_lookup` 作为形参传入（app_routes.rs:247），performance 分支是**没接**而不是不能接；且 `llm_status_from`/`tts_status_from` 都接了。

### F-0002-04 · P2
file: crates/live2d-ai-desktop/src/web_api/app_routes.rs:305-308
摘录: `last_fallback: stats.as_ref().map_or(|| "performance_ok".to_string(), |s| s.last_fallback().to_string()),`
调用链: `performance_status` app_routes.rs:284 → `dto::PerformanceStatus` :287 → `AppStatus.performance` → `GET /api/v1/app/status`
影响: 用户可见/文案与实现不一致（**BATCH-0009 补到 DTO 侧反证**：`dto.rs:102` 对该字段的契约注释是「最近一次回退原因码（`performance_ok` = 没回退过）」——即这个字面量的约定含义是「跑过且没回退」；而生产者在**没有 supervisor** 时 `stats` 为 `None`，也返回同一个 `performance_ok`，是第三种含义。**没有 supervisor** 时 status 仍报「上次回退原因 = performance_ok」——从未跑过的东西被报成「上次是成功」，与旁边 `plans/fallbacks/noops` 全 0 并列出现，自相矛盾）
建议: 缺省改成 `"never_ran"`（或把 `last_fallback` 做成 `Option<String>` 省略）
验证: 读 app_routes.rs:300-312（`stats` 为 `None` 时 300-304 的计数全部 `map_or(0,...)`，而 305 走 `map_or("performance_ok")`）
置信: 高（代码）/ 中（前端是否真的渲染该字段未读 Dart 侧，标未核实）

### F-0002-05 · P2
file: crates/live2d-ai-desktop/src/web_api/flutter_app.rs:253-255
摘录: `.with_header(Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..]).expect("Cache-Control 头"),` ——`bytes_response` 是 `/app/*` 全部静态产物的唯一响应构造（:176 index.html、:183 SPA 回落、以及所有 wasm/js/css/font）
调用链: `handle_flutter_app` :175/:181 → `bytes_response` :243 → 每字节产物；实测 `du -sh shell/flutter/build/web` = **47M**，其中 `main.dart.js` 单文件 3,283,073 字节
影响: 性能（`no-cache` 语义是「缓存但每次必须回源校验」，而本 server **不发 ETag / Last-Modified**——tiny_http 侧全文无该头，故浏览器无校验器可用，每次刷新 `/app/` 都要重传整套 47MB 产物）
建议: 对内容稳定的产物发 `ETag`（按 mtime+size 算）或至少给 `immutable` 资产发长 max-age；`index.html` / `flutter_bootstrap.js` 保持 no-cache
验证: `du -sh shell/flutter/build/web` → 47M；`grep -n "ETag\|Last-Modified" crates/live2d-ai-desktop/src/web_api/*.rs` 无命中（本批已确认本 4 文件内无）；全 web_api 目录的 ETag 存在性留 Phase 2 维度 D 横扫确认
置信: 高（缺校验器）/ 中（实际重传量需浏览器或门禁验证）

### F-0002-06 · P3
file: crates/live2d-ai-desktop/src/web_api/flutter_app.rs:163
摘录: `let Some(root) = resolve_build_dir() else {` ——`resolve_build_dir()` → `build_dir_candidates()` :53 每次都调 `std::env::current_exe()`、`repo_root(dir)`、`repo_root(&cwd)`，而 `repo_root`（:81-86）逐级 `ancestors()` 做 `is_file()` + `is_dir()` 两次 stat
调用链: `handle_flutter_app` :163（**每个 `/app/*` 请求一次**）→ `resolve_build_dir` :89 → `build_dir_candidates` :53；一次页面加载要取 index.html / flutter_bootstrap.js / main.dart.js / flutter.js / canvaskit.wasm / AssetManifest / 字体等数十个文件
影响: 性能（每次请求重复做 O(祖先层级) 的 stat，产物目录不变时结果恒定）
建议: `OnceLock<Option<PathBuf>>` 缓存解析结果（产物目录由环境变量/cwd 决定，进程内不变）
验证: 读 flutter_app.rs:53-91；无缓存结构（`static`/`OnceLock`/`lazy_static` 在本文件 0 命中）
置信: 高

### F-0002-07 · P3
file: crates/live2d-ai-desktop/src/web_api/file_watcher.rs:145
摘录: `tracing::info!("配置文件外部修改 detected，触发 supervisor reload");`
调用链: file_watcher.rs:143-146（`pending` 分支）→ 每次外部修改一条
影响: 可维护性（日志文案中英混排「修改 detected」；这条是外部热重载唯一的成功信号，排障时会照着搜）
建议: 改为「检测到配置文件外部修改，触发 supervisor reload」
验证: 读 file_watcher.rs:145 原文
置信: 高

---

## BATCH-0003（2026-09-28）

### F-0003-01 · P2
file: crates/live2d-ai-desktop/src/web_api/ws.rs:98-102；crates/live2d-ai-desktop/src/web_api/ws/broadcaster.rs:142-145
摘录: ws.rs:98 `tracing::debug!( origin = ?origin, port, "WS 升级被拒：Origin 校验失败（403）" )`（函数随即 `return Err(ws_origin_denied_response(...))` → mod.rs:457-462 直接 `request.respond(resp)` 给浏览器）；broadcaster.rs:142 `tracing::debug!( conn_id = removed.conn_id, "broadcaster: 慢订阅者 try_send 失败 → 移除槽位" )`
调用链: WS Origin 403：`run_request_loop` mod.rs:457 `ws::check_request_origin` → ws.rs:86 → ws.rs:98 debug! → ws.rs:103 403 响应（浏览器侧表现为 WS 连接失败）。慢订阅者：`Broadcaster::broadcast` broadcaster.rs:132 → :136 `try_send` 失败 → :142 debug! → :137 `swap_remove` 丢弃唯一 `SyncSender` → actor `recv_timeout` 得 `Disconnected`（connection.rs:157）→ close + `unsubscriber`（connection.rs:166），**该连接被静默掐断**
影响: 可维护性/排障（同仓库已有相反且更严的口径：`dispatch.rs:73-84` 把同类「会到用户脸上的 403」从 `debug!` 提到 `warn!`，理由写明「前端把 403 当普通报错显示，日志若也静默，就没人能解释那个 403 是怎么来的」。WS 侧这次拒绝、以及一次静默掐断的连接，在默认 `info` 档位下同样一个字都不留）
建议: 两处提到 `warn!`（WS 拒绝带 `code=`；慢订阅者带 `conn_id` 与队列容量），与 dispatch 的分级口径统一
验证: 读 ws.rs:86-105、broadcaster.rs:126-150；对照 dispatch.rs:76-84
置信: 高
反证: 试过论证「WS 侧 debug! 是刻意的，因为握手失败很正常」——`is_ws_path` 只在本地 Flutter 前端与测试里命中，正常轮次不会触发；但**触发它的正是排障场景**（端口不一致、`LIVE2D_AI_ALLOW_NO_ORIGIN` 误配、多标签页指向别的端口），与 dispatch 当初提级的场景完全同类。

### F-0003-02 · P3
file: crates/live2d-ai-desktop/src/web_api/ws/connection.rs:225-232
摘录: `fn build_subscribe_ack(state: &mut ConnectionState, topics: &[String]) -> String { ... let topics_json = serde_json::to_string(topics).unwrap_or_else(|| "[]".to_string()); format!(r#"{{"type":"subscribe_ack","seq":{seq},"ts":"{ts}","data":{{"topics":{topics_json}}}}}"#) }`
调用链: 唯一调用点 connection.rs:135 `build_subscribe_ack(&mut state, &[])`（恒空切片）
影响: 可维护性/安全（形参 `topics` 是死参数，永远为空；且它被**未经 JSON 字符串转义**地拼进 format! 模板——今天不可达，将来谁把 topic 名接上（多订阅/分主题）就会得到坏 JSON 或注入）
建议: 保留形参时改用 `serde_json::json!` 整体构造（与 `WsFrame` 同款），别手拼模板
验证: `grep -n "build_subscribe_ack" crates --include=*.rs` → 定义 1 处 + 调用 1 处（connection.rs:135）+ 测试 2 处（均传 `&[]`）
置信: 高

### F-0003-03 · P3
file: crates/live2d-ai-desktop/src/web_api/ws.rs:227-235
摘录: `/// WS 协议不兼容（升级后 tungstenite::accept 失败）→ 426 Upgrade Required。` ——而唯一调用点 ws.rs:131-137 是**升级前**取 `Sec-WebSocket-Key` 缺失：`Err(e) => { tracing::debug!("WS 升级失败：缺 Sec-WebSocket-Key"); return Err(ws_protocol_error_response()); }`；全文无 `tungstenite::accept` 调用（本仓走 `from_partially_read`，ws.rs:159-164）
调用链: ws.rs:131-137 → ws.rs:228 `ws_protocol_error_response()` → 426 `ws_upgrade_failed`
影响: 可维护性（头注描述的成因与实际不符，且写的是本仓**已经不用**的 `tungstenite::accept` 路径；排障时按头注去找 accept 失败会走空）
建议: 头注改成实际成因（缺 `Sec-WebSocket-Key` 的握手不合规），并注明 `from_partially_read` 取代了 accept
验证: 读 ws.rs:126-137 与 :158-164 原文；`grep -n "tungstenite::accept" ws.rs` 无命中
置信: 高

### F-0003-04 · P3
file: crates/live2d-ai-desktop/src/web_api/ws/broadcaster.rs:132-150
摘录: `if guard[idx].tx.try_send(frame.to_string()).is_err() { let removed = guard.swap_remove(idx); ... tracing::debug!(conn_id = removed.conn_id, "broadcaster: 慢订阅者 try_send 失败 → 移除槽位"); }` ——通道是 `mpsc::sync_channel::<String>(256)`（ws.rs:167），`try_send` 在**队列满**与**对端已死**两种情况下返回同一个 `Err`
调用链: `broadcast` broadcaster.rs:132 → :136 try_send 满/断 → :137 swap_remove 丢 tx → actor `Disconnected`（connection.rs:157）→ close + unsubscribe
影响: 用户可见/可维护性（**背压**被当成**死亡**：一个瞬时变慢的客户端攒满 256 帧就被硬掐断，而不是被限速。帧是 20ms 音频切片（ws/audio.rs:3），256 帧≈5 秒缓冲，故触发门槛是「客户端 5 秒不读」）
建议: 区分两种 Err（`TrySendError::Full` vs `Disconnected`）：Full 时保留槽位并跳过该帧或改为丢最旧帧；只有 Disconnected 才回收槽位
验证: 读 broadcaster.rs:126-150 + ws.rs:167 + connection.rs:157-166
置信: 高（机制）/ 中（可达性：需客户端 5 秒不读；浏览器后台标签仍会读 WS，故真实触发场景窄。前端是否有自动重连未读，标未核实）

> **⚠ BATCH-0011 回填的反证（发现被削弱，如实改写）**：本批读 `tests_ws.rs:776-796` 找到
> 直接反证——`broadcast_does_not_artificially_drop_active_subscribers` 的测试头注明写
> 「仅在 unsubscribe 显式调用 / **慢订阅者 try_send 失败时**回收」，且该测试**真的**驱动了
> `drop(rx)` 后的 `try_send` 失败路径。也就是说「背压即回收」是**被测试固化的既定策略**，
> 不是疏漏。原判「建议区分 Full / Disconnected」应降级为「若要改行为需同时改这条测试与
> broadcaster.rs:136-145 的策略声明」。**本条的实质价值降为 P3 备忘**：
> 留下的是「策略本身有代价（瞬时变慢的客户端被硬掐断）」这一事实，
> 而非「实现与意图不符」。按 §5 的反证纪律，此处显式记录发现被后续批次证伪的部分。

---

## BATCH-0004（2026-09-28）

### F-0004-01 · P2
file: crates/live2d-ai-desktop/src/web_api/chat_routes.rs:171-191
摘录: 顺序是「先改状态、后判忙」：
```rust
handle.set_active_session(session.as_deref());                                   // :171
let revocation = return_to_session_baseline(handle, broadcaster, "new-message"); // :172 —— 内部 :257-259 bc.broadcast(raw) 已把帧发出去
if handle.say_scoped(text, session) { ... json_response(StatusCode(200), body) } // :175-188
else { busy_response() }                                                        // :190 —— 响应体只有 error，无 session_id / 无 baseline
```
影响: 用户可见/契约一致性（say 通道满（429 busy）时，请求被**拒绝**，但两处状态已经变了：宿主活动会话已切到本条消息的会话，且已向 WS 广播一条 `{"type":"action_cue","cues":[],"baseline":true,"covers_upto_seq":0}` 的**整表替换**帧（按本文件 :228-235 的定义，前端 `ActionCueEvent` 消费它＝清掉当前 cue 计划）。而 429 的响应体只有 `busy_response()`（:487-497），**不含** `session_id` / `baseline`——与 200 分支的 `ChatAccepted`（:82-97，带这两项）不对称。于是：用户连发两条时，第二条明明被拒，界面却收到一条「清计划」帧，正在说话那一轮的舞台编排被清空，而 HTTP 侧一个字都没说）
建议: 把 `set_active_session` + `return_to_session_baseline` 移到 `say_scoped` 成功之后；或让 `busy_response` 也带上 `session_id` / `baseline`（哪怕语义是「已切换但未受理」），使两条分支对前端对称
验证: 读 chat_routes.rs:167-191 调用顺序、:236-267 `return_to_session_baseline` 的广播点、:487-497 `busy_response` 的响应体
置信: 高（代码行为与响应体不对称）/ 中（用户可见性：前端收到 429 后的 UI 行为未读 Dart，标未核实）
反证: (a) 试过论证「这是刻意设计」——测试 `new_message_cancels_orchestration_without_backfill`（:961-962）确有注释「第二次 say 可能撞『忙碌』（容量 1）——帧**无论 say 收不收**都要发：回基准是取消语义，不是『发送成功才回』」。**但那条注释只覆盖『帧』，一个字都没提 `set_active_session`**，而该测试的 `broadcaster` 是 Some、两次请求都带显式 `session_id`（"A" / "Z"），因此活动会话的切换在该测试里是「预期内」的，无法暴露它；(b) 试过论证「拒绝时不该取消上一轮」是否违反产品意图——头注 :156-166 的论证是「新用户消息 = 取消上一轮编排」，前提是这条消息**被受理**；429 分支恰恰说明它没有被受理，此时「取消」与「拒绝」自相矛盾；(c) 试过找覆盖该路径的测试——`chat_with_supervisor_returns_200`（:657-671）确实制造了第二次 say 撞忙，但只断言 `429` 与 `body.contains("busy")`，且 `broadcaster` 传 `None`（帧根本没发），会话副作用零断言。**测试存在但抓不到这个缺陷。**

### F-0004-02 · P2
file: crates/live2d-ai-desktop/src/web_api/chat_routes.rs:50-58
摘录: `//! # 行数豁免说明（用户硬约束 ≤500；本文件允许 ≤1000 头注豁免）` / `//! 本文件当前约 560 行：对话入口 + stop + **L1 会话 id 路由**三个 handler，`
影响: 可维护性/治理（实测 1086 行，**已越过 1000 行的豁免上限本身**，不只是越过 500；而豁免理由仍在按 560 行陈述，读者会据此以为它合规。任务书 §9 的已知结构债清单列了 `mod_registry.rs 721` / `mods_routes.rs 958` / `persona/lib.rs 998`，**未列**本文件 ⇒ 属未登记的新增超限）
建议: 按本文件自己在 :53-58 给出的理由拆测试——同目录的 `voice_routes_tests*.rs`、`models_routes/tests_models_*.rs` 都是现成范式；拆完主体约 560 行，落在豁免内且头注自动恢复真实
验证: `git ls-files ... | xargs wc -l | sort -rn` → `chat_routes.rs 1086`；`#[cfg(test)] mod tests` 起于 :525，测试体约 561 行
置信: 高

### F-0004-03 · P3
file: crates/live2d-ai-desktop/src/web_api/chat_routes.rs:340-356
摘录: 会话路由先查 CT、后查 Origin：`if !crate::web_api::security::is_json_content_type(content_type) { return Some(unsupported_media_type()); }`（:341-345，**415**）`if !crate::web_api::security::is_allowed_origin(origin, &ctx.security) { ... mutating_check_error_response(&err) }`（:346-356，**403**）
对照: `security::check_mutating_request`（security.rs:129 先 Origin、:137 后 CT）对同一类输入统一回 403（`MutatingCheckError::as_status()` :229-231 恒 403）
影响: 可维护性/契约一致性（同一仓两套 mutating 拒绝口径：dispatch 系端点 403、前置路由系端点 415；且判定顺序相反 ⇒ 「Origin 错 + CT 错」的请求在两条路径上得到不同状态码）
建议: 统一走 `security::check_mutating_request`，或至少把顺序与状态码对齐并在 security.rs 头注登记两套约定
验证: 读 chat_routes.rs:340-356 与 security.rs:122-146；测试 `chat_session_post_enforces_mutating_contract`（:747-773）把 415/403 各自钉死——即两套口径都被测试固化了，改动需同批改测试
置信: 高（不一致事实）/ 低（用户可见性：前端恒发 application/json，正常不会触发）

---

## BATCH-0005（2026-09-28）

### F-0005-01 · P2
file: crates/live2d-ai-desktop/src/web_api/external_routes.rs:113-128（同一形状见 voice_routes.rs:151-167）
摘录:
```rust
fn external_input_gate(ctx: &ServerContext) -> ModGate {
    ...
    match reg.config(MOD_ID) {
        Some(config) => ModGate { enabled: reg.is_enabled(MOD_ID), config: config.clone() },
        // 未注册 = 无门禁（见头注「启停门禁」第三点）。
        None => ModGate { enabled: true, config: serde_json::json!({}) },   // :124-127
    }
}
```
而 token 随后从这份 config 取：`let config_token = live2d_ai_mod_external_input::token_from_config(&gate.config); let effective = env_token.as_deref().or(config_token.as_deref());`（:263-264），`effective == None` 时 `check_token` **恒返回 true**（:371-388 的 `match env_token { None => true, ... }`）
调用链: `run_request_loop` mod.rs:516 → `handle_external_chat` :167 → `handle_external_chat_with` :192 → `external_input_gate` :113 → `None` 分支 :124（`enabled: true` + 空 config）→ :263 token 解析得 `None` → :265 `check_token` 恒 true → :292 `inject` → `supervisor.say`。voice 侧同构：voice_routes.rs:165 → :260 `effective_token` → :261 `check_token`（:434-435 `None => true`）→ :352 `say`
影响: 安全/可观测性（`external-input` / `voice-input` 只要**不在注册表里**，两条注入端点会同时丢掉 ①启停门禁 ②token 鉴权，变成 loopback 上任何进程都能往主链路注入任意文本（→ LLM → TTS → 出声），且**没有任何日志、没有任何响应差异**。今天生产装配恒经 `AVAILABLE_MOD_FACTORIES`（头注 :41），故不可达；但本仓**已经发生过一次工厂移除**——`local-llm` 在 0.2.0-rc.1 被移出 `AVAILABLE_MOD_FACTORIES`（见 AGENTS.md 变更历史），即这不是假想演进，而是有先例的改动类型）
建议: 把「Mod 缺席」与「Mod 停用」区分开处理——生产装配下 Mod 缺席应是**启动期错误**（`cli_entry` 装配时就断言 external-input / voice-input 在册），或至少在 `external_input_gate` 命中 `None` 分支时 `tracing::warn!` 一次（现在这两个模块 `tracing::` 命中数为 0，见 F-0001-01，降级完全不可见）
验证: 读 external_routes.rs:113-128 / :257-272 / :371-388；`grep -n "pub fn config\b" -A 14 mod_registry.rs` → `self.entries.get(id).map(|e| &e.config)`（在册恒 `Some`）；`grep -c "tracing::" external_routes.rs voice_routes.rs` → 0 0
置信: 高（代码行为）/ 中（生产可达性依赖「生产恒经 AVAILABLE_MOD_FACTORIES 装配」这条文档声明，**未核实**，留 mod_registry 批次确认）
反证: (a) 试过论证「缺席分支只给测试用」——头注 :39-41 确实这么写，但同一段代码在生产装配下**同样可达**，而代码里没有任何 `cfg(test)` 或 debug_assert 把两种情形分开；(b) 试过论证「缺席时至少 token 还在」——`None` 分支给的是 `config: json!({})`，`token_from_config` 必然取不到 token，env 档也常为空，故 `effective` 恒 `None`；(c) 试过找别处的兜底——`start_server` 只按 loopback 绑定（mod.rs:417），不区分「谁能连」；全仓无对该端点的额外鉴权。

### F-0005-02 · P3
file: crates/live2d-ai-desktop/src/web_api/external_routes.rs:104-129 与 crates/live2d-ai-desktop/src/web_api/voice_routes.rs:142-167
摘录: 两条端点各自持有一份**同名同形状**的 `struct ModGate { enabled: bool, config: serde_json::Value }` 与各自的 `*_input_gate(ctx)` 读取函数（external :105-129 / voice :143-167），连注释都对应（`/// 未注册 = 无门禁（见头注「启停门禁」第三点）。`）；差异只在「空值处理」一处就已经分叉：external 内联 `.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())`（:260-262），voice 抽成可测纯函数 `effective_token` 并附了危害说明（voice :447-457）
调用链: external :241 `external_input_gate(ctx)` / voice :240 `voice_input_gate(ctx)`；external :260-264 / voice :258-260
影响: 可维护性（同一份「注入端点骨架」——门禁 / token 优先级 / CT+Origin / 503 / busy 200——有两份平行实现；已经出现「一边内联一边抽函数」的不对称，下一个端点（若再加）会是第三份）
建议: 抽一个共享的 `injection_gate(ctx, mod_id, env_var) -> ModGate` + 共用的 token 归一化纯函数，两个 handler 只留各自的业务闸（模板 vs 唤醒词）
验证: 并读 external_routes.rs:104-129/257-272 与 voice_routes.rs:142-167/255-267
置信: 高

---

## BATCH-0006（2026-09-28）

### F-0006-01 · P2
file: crates/live2d-ai-desktop/src/web_api/mods_routes.rs:482-499
摘录:
```rust
/// 脱敏：按 schema 剥掉 `secret=true` 的 String 字段值（密钥不进 GET）。
fn redacted_config(config: &Value, spec: Option<&ModSettingsSpec>) -> Value {
    let mut obj = config.as_object().cloned().unwrap_or_default();   // :487 —— 只取顶层 object
    if let Some(spec) = spec {                                       // :488 —— spec 为 None 则整段跳过
        for field in &spec.fields {
            if let ModSettingField::String { key, secret: true, .. } = field {
                obj.remove(key);                                      // :494 —— 只删同名顶层键
            }
        }
    }
    Value::Object(obj)
}
```
而模块头注写的是无条件断言：`- GET /api/v1/mods 与 GET …/config：只读，Origin 校验不强制（CSRF 风险为 0）；**secret=true 的字段值永不出现在响应里**（`redacted_config`）。`（mods_routes.rs:14-15）
调用链: `GET /api/v1/mods` → `handle_mods_list` :343 → :364 `"config": redacted_config(&config, spec)`；`GET /api/v1/mods/{id}/config` → `handle_mod_config_get` :374 → :382 同函数。`spec` 来自 `registry.settings_spec(desc.id)`（:352 / :382），其 trait 默认实现是 `fn settings_spec(&self) -> Option<ModSettingsSpec> { None }`（`crates/live2d-ai-mod-system/src/factory.rs:31-32`）
影响: 安全/契约一致性（脱敏是 **fail-open** 的：① `spec = None` ⇒ **一个字段都不脱敏**，整份 config 原样回给浏览器；② 只按 schema 的**同名顶层键**删，config 若把密钥存在嵌套对象/数组或另一个键名下（`{"auth":{"token":"…"}}` 而 spec 声明的是 `token`）则原样返回。头注那句「永不出现在响应里」因此是**错的**。另：`GET …/state` 路径**完全没有脱敏**，契约上靠各 Mod 自行脱敏（:403「返回体里的 state 由各 Mod 自行脱敏」），无任何强制）
建议: ① 把脱敏做成**默认拒绝**——`spec = None` 时按「整份 config 不可回显」处理（或至少对 key 名含 token/key/secret/password 的键无条件剔除）；② 递归下降到嵌套 object/array 再按叶子键删；③ 把 :15 的头注改成与实现一致的话
验证: 读 mods_routes.rs:482-499 / :338-387 / :12-25；`grep -rn "fn settings_spec" -A 4 crates/live2d-ai-mod-*/src/*.rs` → 7 个出厂 Mod 全部 `Some(..)`，**当前无现网泄露**；`grep -n "external_input_settings_spec" -A 45 crates/live2d-ai-mod-external-input/src/lib.rs` → `key:"token", secret:true`（:89-93），注入 token 确实被剥
置信: 高（结构缺口与头注不符）/ 低（现网可达性：出厂 Mod 全覆盖；`live2d-ai-mod-template` 与任何未覆写 `settings_spec` 的新 Mod 会命中 ①）
反证: (a) 试过论证「所有 Mod 都有 spec 所以没事」——7 个出厂 Mod 确实都有，所以**这不是现网泄露**；(b) 但 trait 默认实现返回 `None`（factory.rs:31-32），而 `live2d-ai-mod-template` 是官方给新 Mod 的起步模板，漏覆写 `settings_spec` 就会静默变成「零脱敏」，且没有任何告警或测试拦；(c) 试过找兜底——`handle_mod_state_get` 连 `redacted_config` 都不调。

### F-0006-02 · P2
file: crates/live2d-ai-desktop/src/web_api/mods_routes.rs:162
摘录: `let id_static: &'static str = Box::leak(id.to_string().into_boxed_str());` ——`Box::leak` 的语义就是**故意泄漏**这块 `Box`（换取 `&'static str`），永不回收
调用链: `handle_mods_route` :162（每次 `POST /api/v1/mods/{id}/{action}` 都执行，**在早退分支之前**）→ `registry.enable/disable/restart/reload_config(id_static, …)`（:258 / :268 / :307-312）
影响: 性能/可维护性（无界内存泄漏：每个 mutating Mod API 请求永久泄漏一份 `id` 字符串。进程设计为常驻桌宠（跑整日/整夜），泄漏随请求数线性增长且永不回收。量级很小（十几次字节/请求），故不升级为 P1，但它是**按构造**的无界增长，不是概率问题）
建议: 让 registry 侧提供 `fn enable_by_str(&self, id: &str)` 之类——它已有 `contains(&str)` / `is_enabled(&str)` / `config(&str)` 三个 `&str` 版本（mod_registry.rs:474-489），说明内部就是按 `&str` 查表；把 `enable/disable/restart/reload_config` 也改成 `&str` 并在内部 `self.entries.get_key_value(id)` 拿回 `&'static` 的 descriptor id，即可去掉 `Box::leak`
验证: 读 mods_routes.rs:157-166 原文；`grep -n "pub fn is_enabled|pub fn config|pub fn contains" -A 8 mod_registry.rs` → 三者签名均取 `&str`，与 `&'static str` 的 mutator 口径不一致
置信: 高

### F-0006-03 · P2
file: crates/live2d-ai-desktop/src/web_api/mods_routes.rs:163-166（另见 :347 / :378 / :408）
摘录: `let mut registry = ctx.mod_registry.lock().expect("mod_registry mutex poisoned");` ——而同仓对**同一把锁**的另两处处理完全相反：`let reg = match ctx.mod_registry.lock() { Ok(g) => g, Err(poisoned) => poisoned.into_inner(), // 锁中毒不 panic：门禁读快照即可。 };`（external_routes.rs:114-117，voice_routes.rs:152-155 同款）
调用链: `handle_mods_route` :163 取锁（持锁期间在 :302-321 调 `registry.enable/disable/restart`——**Mod 工厂代码在锁内执行**）→ 若该调用 panic，锁被毒化 → 下一次任意 Mod API 请求在 :166 `.expect(...)` 上 panic → 该 panic 发生在 `run_request_loop`（mod.rs:445-568，裸 `for` 循环，**无 `catch_unwind`**）的 accept 线程上 → 整个 HTTP 服务停摆
影响: 可维护性/用户可见（一次 Mod 侧 panic 会**永久**打死 HTTP 面，而不是退化成 503；且 external/voice 两条端点在同一把锁上刻意选了不 panic，说明「不 panic」才是本仓的既定口径）
建议: 与 external/voice 统一成 `poisoned.into_inner()`；或给 `run_request_loop` 的每请求处理包一层 `catch_unwind`（顺带兜住任何 handler 的 panic）
验证: 读 mods_routes.rs:157-166 / :302-322、external_routes.rs:113-118、mod.rs:445-568
置信: 高（两处锁处理不一致 + accept 循环无 catch_unwind 均可直接读出）/ 中（触发前提：需某个 Mod 的 enable/restart 真正 panic——`mod_registry.rs` 里的实现本批**未读**，标未核实。登记在案的 M/S 侧「Mod 启动失败无日志与 last_error」说明该路径确有失败面）
反证: 试过论证「panic 会毒化但无害，因为没有第二次请求」——服务是常驻的，Mod 管理是用户会点的功能，第二次请求必然存在；试过论证「`std::sync::Mutex` 在 Rust 里 panic 后仍可用」——可用，但 `lock()` 返回 `Err(PoisonError)`，`.expect()` 正是把 `Err` 变 panic 的那个调用。

### F-0006-04 · P3
file: crates/live2d-ai-desktop/src/web_api/ws/broadcaster.rs:174；crates/live2d-ai-desktop/src/web_api/ws/audio.rs:13
摘录: `#[allow(dead_code)]` `pub fn broadcast_audio_frames(` （broadcaster.rs:174-175）——但它有生产调用点 `bc.broadcast_audio_frames(`（cli_entry.rs:502）；`#[allow(dead_code)]` `pub const AUDIO_SLICE_MS: u32 = 20;` （audio.rs:13-14）——但它在 audio.rs:96 与 :131 两处被写进帧
影响: 可维护性（**模式 D 第 5、6 例**：注释/`allow` 标注描述的装配状态与事实不符。`allow(dead_code)` 在这里是**反向**的——它让「这个函数没人调」看起来像既定事实，而实际上它在主链路上；将来真有人删掉 cli_entry.rs:502 的调用，编译器也不会报）
建议: 两处 `#[allow(dead_code)]` 直接删（删完能过 clippy 才说明确实都还接着）
验证: `grep -rn "broadcast_audio_frames" crates/ --include=*.rs` → 定义 1 + 生产调用 1（cli_entry.rs:502）+ 测试 5；读 audio.rs:96 / :131 确认 `AUDIO_SLICE_MS` 在用
置信: 高

---

## BATCH-0007（2026-09-28）

### F-0007-01 · P2
file: crates/live2d-ai-desktop/src/web_api/models_routes/registry.rs:278-299（另见 :153 / :177 / :207 / :244）
摘录: 六个持久化结构**全部**标了 `#[serde(deny_unknown_fields)]`：
`#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)] #[serde(deny_unknown_fields)] pub struct ModelRegistry { pub schema_version: u32, pub active_id: Option<String>, pub entries: BTreeMap<String, ModelEntry>, }`（:293-299）
而本文件自己写明了后果：`新字段名无 `default`，反序列化失败走 P1-5 的损坏降级路径——`model_registry.json.corrupt-<ts>` 备份后用空 registry 重新持久化。registry 是运行时数据，可重建（用户重跑 import 即可）。`（registry.rs:273-277）；损坏降级 = 「stderr 警告 + 空 registry」（mod.rs:306-308 头注）
调用链: `ServerContext::new`（mod.rs:334）→ `models_routes::default_store` → `ModelRegistry::load_from_path` registry.rs:303-311（`serde_json::from_slice` 遇未知字段**直接 Err**）→ 降级为空 registry → `GET /api/v1/models`（dispatch.rs:208-210）返回空列表、`active_id` 丢失 → 舞台回落 `BUILTIN_MODEL_URL`（model_root.rs:35）
影响: 用户可见/数据（**每一次给 registry schema 加字段的版本升级，都会把老用户的已导入模型列表整体清空**：模型文件还在 `assets/models/` 里，但 UI 列表空、`active_id` 丢失，用户必须逐个重跑 import。与红线 R 对 `mods.json` 明写的「原子写回**不得丢未知 id**」要求方向相反——那条纪律在这个同族文件上没有落地。另：`deny_unknown_fields` 也让**降级读**（新二进制读旧文件，或用户手加了自定义字段）一律变成「损坏」而非「忽略未知项」）
建议: 持久化结构去掉 `deny_unknown_fields`（改为 `#[serde(default)]` + 可选字段），只保留 `ModelSource` 那种枚举的 `#[serde(other)]` 兼容面；若确实要严格校验，把校验放到**写**侧而不是**读**侧
验证: 读 registry.rs:153/177/207/244/278-299（6 处 `deny_unknown_fields`）与 :303-311（读侧无宽容）；`default_store` 的降级实现未逐行读（见 BATCH-0007 未核实项 1）
置信: 高（结构 + 降级行为均已逐行确认；BATCH-0008 回填：`default_store` → `from_disk`（models_routes/mod.rs:149-183）确实在解析失败时备份到 `.corrupt-<ts>` 并**继续用空 registry**，且注释自陈「后续 persist 仍会覆盖 registry_path」）/ 「升级即清空」是对**下一次** schema 变更的预测，不是已发生的事实——故仍不升级为 P1

### F-0007-02 · P3
file: crates/live2d-ai-desktop/src/web_api/models_routes/util.rs:81-126
摘录: `util.rs` 有一份 `now_iso8601()`（:81）与 `unix_secs_to_ymdhms()`（:92）；`app_routes.rs:319-371` 有一份 `system_time_iso8601()` 与**同算法**的 `unix_secs_to_ymhdms()`（:332）；`ws/events.rs:252-280` 又有第三份 `iso8601_now_ms()` 与 `epoch_secs_to_ymdhms()`
调用链: `app_routes.rs:263 system_time_iso8601(ctx.started_at)`（status 的 `started_at` 字段）；`util.rs:88 now_iso8601()`（registry 的 `imported_at`）；`events.rs:201 iso8601_now_ms()`（WS 帧的 `ts`）
影响: 可维护性（同一个 crate 里「unix 秒 → 日历」有**三份**实现、「ISO-8601 毫秒」有**三份**实现；其中 `app_routes` 与 `models_routes/util` 两份是同一套手写历法算法（同样的 2100 截断、同样的 1970 回退），差别仅是 `is_leap` 被抽成函数还是内联；`ws/events.rs` 那份用的是 Howard Hinnant 算法，与前两份**行为可能不一致**（负数/超大值的处理路径不同），于是同一个时间戳在不同帧里可能出现不同字面量）
建议: 抽一个 `crate::time_iso`（保留 Hinnant 版本），三处统一调用；至少先让 `app_routes` 与 `util` 合并
验证: 分别读 app_routes.rs:316-371、util.rs:80-126、ws/events.rs:251-280 逐行对照
置信: 高（三份实现存在）/ 中（跨实现字面量不一致的实际发生条件未构造——三条路径的时间输入都是 `SystemTime::now()` 的正数区间，实践中不易分歧，故不升级）

---

## BATCH-0008（2026-09-28）

### F-0008-01 · P2
file: crates/live2d-ai-desktop/src/web_api/models_routes/handlers.rs:94-116（调用点 :88 与 :130）
摘录: `fn entry_to_list_item(store: &ModelStore, entry: &ModelEntry, active_id: Option<&str>) -> ModelListItem { let abs_dir = store.assets_root.join(&entry.relative_dir); let size = compute_dir_size(&abs_dir).unwrap_or(0); ... }` ——`compute_dir_size`（models_routes/util.rs:59-78）是**显式栈的全递归遍历**，对每个目录项做一次 `symlink_metadata`
调用链: `GET /api/v1/models` → dispatch.rs:208-210 `models_routes::dispatch` → mod.rs:270 `handle_list` → handlers.rs:88 `entry_to_list_item` → :102 `compute_dir_size`（**每个 entry 一次全递归**）；`GET /api/v1/models/{id}` 同样走 :130
影响: 性能（`size_bytes` 是纯展示字段，却要**每次请求**对每个模型目录做全递归 stat。Live2D 模型包典型是 moc3 + 数十张纹理，N 个模型 ⇒ 每次列表请求 N 次全树遍历，全部在 tiny_http 的**请求处理线程内同步执行**，无缓存、无超时、无并发上限。模型库页面每次刷新都付一次）
建议: 给 `ModelEntry` 加一个 `size_bytes` 缓存字段，在 import 时算一次；或按 `(mtime, 条目数)` 做惰性缓存；退一步也可以给 `compute_dir_size` 加遍历上限并把结果标 `size_approximate`
验证: 读 handlers.rs:80-116、models_routes/util.rs:58-78、mod.rs:269-289
置信: 高（机制与调用点）/ 中（实际耗时未实测——本机 `assets/models/` 里没有真实模型包（只有 `.gitkeep` + `README.md`），无法给出量级数字，**未核实**）

### F-0008-02 · P3
file: crates/live2d-ai-desktop/src/web_api/models_routes/mod.rs:167-179
摘录: `match fs::copy(&store.registry_path, &backup) { Ok(n) => eprintln!("[warn] registry 损坏（{}），已备份为 {}（{} 字节）：{e}。使用空 registry。", ...), Err(copy_err) => eprintln!("[warn] registry 损坏（{}），且备份失败（{}）：{copy_err}。原文件保留，使用空 registry：{e}", ...), }`
影响: 可维护性（这是**唯一会清空用户已导入模型列表**的事件（见 F-0007-01），却只写 stderr，不进 tracing 文件 sink——与 AGENTS.md 明写的纪律相反：「禁止新增只 `println!` 的错误路径（不进文件 sink = 排障时一片空白）」。用户报「模型库突然空了」时，日志文件里查不到任何痕迹）
建议: 改 `tracing::warn!`（若启动期 tracing 尚未初始化，则与 `cli_entry` 里其它启动告警统一走同一个初始化顺序；至少把「已降级为空 registry」这条写进之后一定会落盘的启动日志）
验证: 读 models_routes/mod.rs:149-183 原文；对照 `dispatch.rs:76-84`（同类失败用 `tracing::warn!` 且带 `code=`）
置信: 高（事实）/ 低（是否为规避「启动期 tracing 未初始化」而刻意为之——**未核实** `init_logging` 与 `ServerContext::new` 的先后，见 STATE 未核实区）

---

## BATCH-0009（2026-09-28）

### F-0009-01 · P2
file: crates/live2d-ai-desktop/src/web_api/app_routes.rs:267-272（DTO 契约见 crates/live2d-ai-desktop/src/web_api/dto.rs:108-119）
摘录: 生产者写死：`audio: dto::AudioStatus { backend: "none", available: false, sample_rate: live2d_ai_runtime::AudioSpec::DEFAULT_SAMPLE_RATE, channels: live2d_ai_runtime::AudioSpec::DEFAULT_CHANNELS, },`（app_routes.rs:267-272，**无条件**）。而 DTO 侧的契约注释写的是条件语义：`/// 音频后端状态（v1 占位：未启动 supervisor 时 `available=false`）。` / `/// 后端名（`"alsa"` / `"pulseaudio"` / `"none"`）。`（dto.rs:108-113）——即「有 supervisor 时应当不是 none/true」，但代码里没有任何分支
调用链: `GET /api/v1/app/status` → dispatch.rs:90-95 → `handle_status` app_routes.rs:226 → `build_status` :262-278 → `dto::AppStatus.audio` → Dart 侧 `shell/flutter/lib/settings/sections/dev_tools_section.dart:1108-1109`：`'${(status['audio'] as Map<String, Object?>?)?['backend'] ?? '未知'}'` / `'（sample_rate=${(status['audio'] as Map<String, Object?>?)?['sample_rate'] ?? '?'}）'`
影响: 用户可见/文案与实现不一致（本机此刻正在通过 WS `<audio>` + Blob(WAV) 推 PCM 音频、浏览器正在出声，但 `app.status` 恒报 `backend:"none", available:false`；诊断面板那一行的存在意义就是「如实显示当前后端」，它恒显「none」）。同一份 DTO 文件里 `AppInfo` 的注释还专门立了规矩——「**能力快照不得报未实现的能力**」（dto.rs:23-25），这里是反向的「不得报未启用的能力」
建议: 从 `Broadcaster::is_muted()` / `AudioSpec` 与实际出声路径派生 `backend`/`available`；或把 DTO 注释改成「v1 未接线，恒为 none/false」并在诊断面板标注「（未接线）」
验证: 读 app_routes.rs:262-278、dto.rs:108-119；`grep -rn "'audio'|\"audio\"" shell/flutter/lib/` → 唯一消费者是 dev_tools_section.dart:1108/1109/1186
置信: 高（字段写死 + 有真实 Dart 消费者 + DTO 注释与之矛盾）

### F-0009-02 · P3
file: crates/live2d-ai-desktop/src/web_api/models_routes/dto.rs:4 与 :63-67
摘录: 文件头注宣称：`//! 字段与 D1 §1.3 / §6.3 / §8.1 / §8.2 字段对齐表一致；snake_case。` / `//! `deny_unknown_fields` 锁住未知键——前端传错键一目了然。`（:3-4）——但同文件的三个请求体里，`DisplayPatchBody`（:96）、`DisplayModelPatch`（:118）、`DisplayStagePatch`（:133）都标了 `#[serde(deny_unknown_fields)]`，**唯一没有标的是 import 那个**：`#[derive(Debug, Clone, Deserialize)] pub struct ImportRequest { pub id: String, }`（:64-67）
影响: 可维护性（本文件里用得最多的请求体（模型导入）是唯一不锁未知键的：前端把 `{"idd":"bai"}` 传过来会被**静默忽略**并因 `id` 缺失报 `invalid_payload`，而若写对成 `{"id":"bai","overwrite":true}` 会被静默接受——正是头注说要「一眼看出」的那类错）
建议: 给 `ImportRequest` 补 `#[serde(deny_unknown_fields)]`（与同文件另两个请求体一致）
验证: 读 models_routes/dto.rs:1-10 与 :63-150 逐个 `#[derive]` 块对照
置信: 高

---

## BATCH-0010（2026-09-28）

### F-0010-01 · P3
file: crates/live2d-ai-desktop/src/web_api/settings_routes/mod.rs:184-197
摘录:
```rust
let (next, outcome) = apply_patch(current, patch).map_err(|msg| {
    // apply_patch 返回的 String 形态错误码语义不细；按消息前缀判。
    if msg.contains("base_url 非法") {
        ErrorResponse::new(ErrorDetail::new("url_invalid", msg.clone()))
    } else if msg.contains("api_key_env 非法") {
        ... "invalid_env_name" ...
    } else {
        ErrorResponse::new(ErrorDetail::new("invalid_payload", msg))
    }
})?;
```
两个被匹配的子串都产生于**另一个 crate**：`"{section} base_url 非法: {raw:?}（需要绝对 http/https URL）"`（`crates/live2d-ai-runtime/src/settings/patch.rs:715`）与 `"api_key_env 非法: {name:?}（…）"`（同文件 :334）
调用链: `PATCH /api/v1/settings` → dispatch.rs:133-140 → `handle_patch` → `apply_and_write` :184 → `apply_patch`（runtime）→ 错误消息子串匹配 → HTTP `error.code`
影响: 可维护性（**稳定错误码由中文错误文案派生**；AGENTS.md 的纪律是「两处的 `code` 是同一个字符串」，而这里 code 的来源是散文。跨 crate 改一次措辞，前端按 `url_invalid` / `invalid_env_name` 的分支会退化成 `invalid_payload`。另：注释写「按消息**前缀**判」，代码用的是 `contains`（任意位置子串），措辞与实现不符）
建议: 让 `apply_patch` 返回**结构化**错误（错误码 + message）而不是 `String`；短期至少把两个子串提成 `const` 并在 runtime 侧加同名常量测试
验证: 读 settings_routes/mod.rs:184-197；`grep -rn "base_url 非法|api_key_env 非法" crates --include=*.rs` → 生产者两处在 runtime，匹配者一处在此；`grep -rn "url_invalid|invalid_env_name" crates --include=*.rs` → `settings_routes/tests.rs:434` 有 `assert_eq!(err.error.code, "url_invalid")` 的**真实**端到端断言
置信: 高
反证: **这条差点被我报成 P2。** 按 §12.1 去找覆盖测试后确认：`settings_routes/tests.rs:420-434` 的 `url_invalid_patch_returns_400` 走真实 `apply_and_write` 路径并断言 `err.error.code == "url_invalid"`——文案一改这条测试立刻变红，属于「有回归保护」而非「静默降级」，故降为 P3。`invalid_env_name` 未找到同等级的端到端断言（只有 mod.rs:305-306 在 `handle_patch` 里按 code 分支），若只改 `api_key_env 非法` 那条文案则**无测试兜住**——这一点如实记在「建议」里。

---

## BATCH-0011（2026-09-28）

### F-0011-01 · P3
file: crates/live2d-ai-desktop/src/web_api/tests_security_e2e.rs:28-31
摘录: `fn strict_ctx() -> super::ServerContext { super::ServerContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None).with_security(strict_sec()) }` ——被 `dispatch_patch_same_origin_with_json_returns_200`（:132-143）与 `dispatch_patch_localhost_with_json_returns_200`（:146-157）**真的用来跑 `PATCH /api/v1/settings`**
调用链: 测试 → `dispatch_with_security` → dispatch.rs:133-140 `handle_patch` → settings_routes/mod.rs:203-226 `to_toml_string_merging` + `plan_atomic_write` + `fs::rename` ⇒ **真的写 `/tmp/cfg.toml`**，两条测试写**同一个**固定路径，**无唯一化、无清理**
影响: 可维护性（测试不是 hermetic：两个用例共享一个可写固定路径；`to_toml_string_merging` 会把本次内容**合并进磁盘上已有的文件**（保留注释），所以一次跑完 `/tmp` 里会留下累积内容，后续别的测试若也用它会互相影响。本机当前不存在该文件——`ls -la /tmp/cfg.toml` 无命中，说明这台机器没跑过 cargo test）
建议: 照抄同仓已有的正确做法（`chat_routes.rs:606-617` 用 `process::id()` + `thread::current().id()` 的 hash 拼唯一文件名并在收尾 `remove_file`）
验证: 读 tests_security_e2e.rs:28-36 / :132-157；`grep -rn '/tmp/cfg\.toml' crates --include=*.rs` → 另有 8 处（多为 `StatusContext::new` 只读不写）；`ls -la /tmp/cfg.toml` → 不存在
置信: 高（写入路径逐层可追）/ 中（**后果**仅为可维护性：两条测试互不依赖、结果不受磁盘状态影响，故不升级）

### F-0011-02 · P3
file: crates/live2d-ai-desktop/src/web_api/tests_security_e2e.rs:241-254
摘录: `/// `is_mutating_route` 对所有 mutating 端点返回 true；GET / 只读返回 false。/// （**已在 tests_security.rs 覆盖**；此处保留端到端 sanity。）` ——但断言体（:248-253）与 `tests_security.rs:327-355` 的 `mutating_route_classification` + `mutating_route_models_by_method` **逐条相同**（同样 3 个 Settings/Chat 断言 + 同样 3 个 Models 断言 + 同样 2 个只读断言）
影响: 可维护性（这是一条**零增量覆盖**的测试：它不经过 `dispatch_with_security`——名字里的 "end_to_end" 不成立——只是把单元测试的 6 条断言又抄了一遍；数量上让「端到端安全测试」看起来比实际多一条）
建议: 删掉，或改成真正端到端的形态（例如经 `dispatch_with_security` 打 `PATCH /api/v1/models/{id}/activate` 验证它在无 Origin 下确实被 403 —— 那条路径**目前没有任何测试**：`is_mutating_route` 的 `Models + POST` 只在单元层断言过）
验证: 逐行对照 tests_security_e2e.rs:243-254 与 tests_security.rs:327-355
置信: 高

---

## BATCH-0012（2026-09-28）

### F-0006-03 · P1  ⬆ **由 P2 升级**（BATCH-0012 收口了它的未核实项）
> ✅ **B0268 结清 ③**：`grep -n "catch_unwind" mod_registry.rs` ⇒ **零命中**
> ⇒ **Mod worker 循环没有 `catch_unwind`** ⇒ **panic 同时做两件事**：**杀死 worker** +
> **毒化它当时持有的那把槽位锁** ⇒ ⇒ **两半同时发生** ⇒ HTTP 侧三处裸 `.lock().unwrap()` **无兜底**
> ⇒ ⭐ 并把 B0117 记的「hook 侧没有 catch_unwind」**扩展到整个文件**
> 🧪 **B0267（Phase 4 证伪）攻它：最强的一击（锁的类型）失败 ⇒ 但**前提被收窄**
> 攻法：若这些锁是 `parking_lot::Mutex`，则**根本没有毒化** ⇒ 本条应被推翻。
> ```
> mod_registry.rs:34  use std::sync::{Arc, Mutex};
> mod_registry.rs:85  type SharedRuntime = Arc<Mutex<Option<Box<dyn ModRuntime>>>>;
> mod_registry.rs:27  //! `Arc<Mutex<…>>` 槽位共享。**worker 线程持这些**…
> ```
> ⇒ ⇒ **是 `std::sync::Mutex`，不是 `parking_lot`** ⇒ **毒化是真的** ⇒ **最强的一击失败**
> ⇒ ⇒ **机制成立**；但 **② 收窄**「前提」：
> 触发所需的不是「进程里任何地方 panic」，而是**「某个 Mod worker 在持有该 Mod 的槽位锁时 panic」**
> （**不是** `blocking_mods` 那把锁、**也不是**别的线程的 panic）⇒ 这比原来的措辞**窄得多、也硬得多**
> ⇒ ⇒ **③ 加重（待证的那一半）**：三个站点全是裸 `.lock().unwrap()`（`:334` 写 / `:434` take / `:617` take）
> ⇒ **若 worker 侧没有 `catch_unwind`**（B0117 已核 hook 侧没有）⇒ **毒化与 worker 死亡同时发生**
> ⇒ ⚠ **这一半本批未核**（worker 循环处是否有 catch_unwind）⇒ 记为敞口，不并入本条结论
file: crates/live2d-ai-desktop/src/mod_registry.rs:434（同款另两处 :334 / :617）
摘录:
```rust
pub fn disable(&mut self, id: &'static str) -> Result<(), ModError> {          // :432
    let Some(e) = self.entries.get_mut(id) else { return Err(ModError::Other(format!("Mod {id} 不在注册表"))); };
    if let Some(mut rt) = self.runtimes.get(id).and_then(|s| s.lock().unwrap().take()) { let _ = rt.shutdown(); }  // :434
    ...
}
```
`runtimes: BTreeMap<&'static str, SharedRuntime>`（:171），而该锁由 **Mod worker 线程**持有——
本文件自己的注释就写着：`/// 3. Mod worker 正持着 runtime 锁（`try_lock` 失败）……`（:497）
调用链（**完整**，BATCH-0006 时最后一环未核实，本批闭合）:
1. Mod worker 线程 panic（持 runtime 锁）→ 锁毒化；
2. 用户点「停用」或「重启」→ `POST /api/v1/mods/{id}/disable|restart`；
3. `mods_routes.rs:163` `ctx.mod_registry.lock().expect("mod_registry mutex poisoned")`，**并持有该守卫直到 :322**；
4. → `mod_registry.rs:434` `s.lock().unwrap()` 在**已中毒**的 runtime 锁上 → **panic**；
5. 该 panic 发生在 `run_request_loop` 的裸 `for request in server.incoming_requests()`（mod.rs:446，**无 `catch_unwind`**）里 → 展开出 accept 循环 → **整个 HTTP 服务停摆**
影响: 用户可见/可维护性（一次 Mod 侧 panic + 一次用户点击 = 服务永久停摆，无自愈、无日志。触发面是「Mod 管理」这个用户会主动点的功能，而 Mod 是社区可注册代码（`docs/architecture/mod-community-license.md`），其 panic 可预期）
建议: 三处 `.lock().unwrap()` 换成「中毒即降级」——`match s.lock() { Ok(g) => g.take(), Err(p) => p.into_inner().take() }`（取出 runtime 后 `shutdown`，语义不变）；并给 `run_request_loop` 的每请求处理包一层 `catch_unwind`，让任何 handler 的 panic 退化成 500 而不是整服务停摆
验证: 读 mod_registry.rs:306-509；`grep -n "lock()\.unwrap()|lock()\.expect(" mod_registry.rs` → ~~11 命中~~ **（B0022 更正：全文件 `lock().unwrap()` 实为 16 命中；`#[cfg(test)]` 起于 :730，故生产路径是 :334 / :434 / :617 三处，其余 13 处全在测试里）**（:771/782/1080/1087/1095/1115/1120/1131/1180/1286/1304/1322/1340）
置信: 高
反证: (a) 试过论证「同文件已有正确写法所以这不算问题」——**恰恰相反**：`runtime_state`（:502-506）刻意用 `try_lock().ok()?`，注释明写「web_api 线程**绝不**为一个可选 Mod 的状态等锁——那会让『看一眼桌宠状态』把整个 HTTP 请求循环卡在 Mod worker 上」（:500-501）。**纪律是已知的、写下来的，只是没覆盖到 enable/disable/shutdown 三处**；(b) 试过论证「Worker 不会 panic 所以锁不会中毒」——worker 跑的是社区 Mod 代码，且本文件 :309-320 自己就承认 Mod 会不兼容/失败；(c) 试过找 accept 循环的保护——`run_request_loop`（mod.rs:445-568）全文无 `catch_unwind`，`start_server` 也没有 panic hook。

### F-0002-01 · P1（补记测试覆盖检查，不改定级）
file: crates/live2d-ai-desktop/src/web_api/file_watcher.rs:46
补记: §12.1 要求「去测试里找覆盖该路径的测试并读它」。本批枚举了
`web_api/tests_reload.rs`（4 条）与 `web_api/tests_p0c.rs`（10 条）的全部测试名，结论：
- `tests_reload.rs:160 refresh_from_disk_syncs_snapshot_and_dev_mode`、
  `:192 refresh_from_disk_keeps_snapshot_on_broken_toml` ——**直接调** `StatusContext::refresh_from_disk`，测的是那个**函数**；
- `tests_p0c.rs:153 first_time_full_config_patch_dynamic_assembles_supervisor`、
  `:343 e2e_first_time_no_config_patch_completes_loop` ——测的是「无 supervisor 时 PATCH 能动态装配」，也是**函数**；
- **全仓无任何测试**断言「`FileWatcher` 在什么条件下被创建」：`watch_config`（file_watcher.rs:46）
  需要真实 `Arc<SupervisorHandle>` + `notify` watcher，一处测试都没有。
影响: 该缺陷**零测试覆盖**，且它旁边有 4 条「读起来像在覆盖『配置快照必须与磁盘一致』这条纪律」的
邻居测试——这正是任务书 §7-J 点名的接缝：**只测纯函数，不测接线**。
置信: 高

---

## BATCH-0013（2026-09-28）

### F-0013-01 · P1
> 🧪 **B0270（Phase 4 证伪）攻它：最强的一击失败，且**③ 加重成立**——文档叫你手写这个文件**
> 攻法：若 `mods.json` 是**内部产物**、不该由人编辑 ⇒ 「未知 id 丢失」无害 ⇒ 该推翻
> ```
> docs/external-input.md:125  **启用**：`mods.json` 里 {"mods":{"external-input":{"enabled":true…}}
> docs/external-input.md:128  **停用**：`POST …/disable`（**或改 `mods.json`**）
> docs/external-input.md:127  **0.2.0-rc.1 起缺省启用**（`mods.json` **不存在时的内建 manifest**）
> ```
> ⇒ ⇒ **`mods.json` 明确被文档当作可手写的配置面**（两处给出 JSON 片段、
> 且「**或改 `mods.json`**」把手改列为与 API 并列的选项）⇒ ⇒ **攻击失败**
> ⇒ ⇒ 而 `:252-253` 整份从 `self.entries` 重建 ⇒ **手写的 id 不在 `entries` 里**
> ⇒ ⇒ **第一次落盘**（任何一次配置保存 / 启停）**就把它静默抹掉**
> ⇒ ⇒ ⭐ **③ 加重**：不是「可能发生」，而是「**文档正好叫你这么做**」
> ⇒ ⇒ **反证（试过并排除）**：「文件由程序原子写，用户不会去手改」——**不成立**：
> 原子写只保证**程序写的时候**不撕裂，**不禁止人先写**；而文档明确邀请人写
file: crates/live2d-ai-desktop/src/mod_registry.rs:248-285（`persist_manifest`）
摘录:
```rust
fn persist_manifest(&self) {                                    // :248
    let mut mods = serde_json::Map::new();
    for (id, e) in &self.entries {                               // :253 —— 只遍历**在册 factory** 的 id
        mods.insert((*id).to_string(), serde_json::json!({ "enabled": e.enabled, "config": e.config }));
    }
    let doc = serde_json::json!({ "mods": mods });               // :259 —— 全新文档，原文件内容不参与
    ...
    match live2d_ai_runtime::settings::patch::plan_atomic_write(path, &text) {   // :274
        Ok(tmp) => { if let Err(e) = std::fs::rename(&tmp, path) { ... } }      // :276 —— 整体覆盖
```
读侧同样不保留未知 id：`fn parse_mod_config(manifest: &Value, id: &'static str) -> (bool, Value) { let Some(obj) = manifest.get("mods").and_then(|m| m.as_object()).and_then(|m| m.get(id))... }`（mod_registry.rs:688-695）——**按单个 id 取**，从不为「不在册的 id」建条目
调用链（完整）:
1. `cli_entry.rs:613-622` `mods_manifest_for_web()` 读**整份** `mods.json` 成一个 `Value`；
2. `cli_entry.rs:182 / :296` `ModRegistry::new(AVAILABLE_MOD_FACTORIES, &mods_manifest_for_web())` → `new` 内部只对每个在册 id 调 `parse_mod_config`（mod_registry.rs:190）⇒ **未知 id 在此被丢弃，全仓无处保留**；
3. `.with_manifest_path(mods_path_for_web())`（cli_entry.rs:290 / :297）设写回目标；
4. 用户点任意 Mod 的 启用/停用/重启/保存配置 → `mods_routes.rs:307-312` → `mod_registry.rs:422 / :432 / :441 / :463`；
5. → `persist_manifest()` → `rename` 覆盖 `mods.json` ⇒ **未知 id 及其 `config` 永久消失**
影响: **数据丢失**（红线 R 明列项「`mods.json` 原子写回**不得丢未知 id**」被违反）+ 用户可见
（丢失的 config 若是 secret 类字段则无法重建）。可达性**不是假想**——cli_entry.rs:640-642 就在
本批读的同一段里写着：「**同版废除 `local-llm` 启动**：……crate 暂留仓库但不再注册、
不再编译进 binary」（AGENTS.md 变更历史同载）。⇒ rc.0 上配置过 `local-llm` 的用户
升级到 rc.1+ 后**第一次点任意 Mod 开关**就丢条目
建议: `persist_manifest` 改成「读原文件 → 合并在册 id 的 enabled/config → 保留原文件里
其余所有 key 原样写回」，即未知 id 走**透传**而不是重建；`new` 侧可把 manifest 的其余
键存进一个 `extra: serde_json::Map<String, Value>` 字段以便写回时合并
验证: 读 mod_registry.rs:190-197（`new` 的 entries 构造）、:248-285（`persist_manifest`）、
:688-695（`parse_mod_config`）、cli_entry.rs:604-643（manifest 读取与 `local-llm` 废弃说明）
置信: 高（读侧/写侧两侧都已逐行确认；无任何代码路径把未知 id 带进 `self.entries`）
反证: (a) 试过找「原文件被读进内存后再合并」的痕迹——`mods_manifest_for_web` 确实返回整份
`Value`，但它在 `ModRegistry::new` 里只被 `parse_mod_config` 逐 id 消费，没有任何字段保存
「其余键」；(b) 试过论证「写回时用的是原 Value 而不是重建」——`persist_manifest` 的第一行
就是 `let mut mods = serde_json::Map::new();`，`self` 上不存在任何「原始 manifest」字段
（`grep manifest_path` 只有 `Option<PathBuf>` 一个字段，:174）；(c) 试过找测试——见 F-0013-02，
唯一那条往返测试的起点是空 manifest。

### F-0013-02 · P3
file: crates/live2d-ai-desktop/src/mod_registry.rs:942-963
摘录: 测试头注承诺得很宽：`/// M1：reload_config（POST …/config 内核）也写回 config，且**不丢其它 Mod**。`（:942）`——但它的起点是 `let mut reg = ModRegistry::new(FACTORIES, &serde_json::json!({})).with_manifest_path(path.clone());`（:950-951），即**空 manifest**；断言只有两条：`v["mods"]["test"]["config"]["port"] == 1234`（:956-959）与 `v["mods"]["test"]["enabled"] == false`（:960-963，「未启用者也要保留在 manifest 里」）
影响: 可维护性（**模式 B 的第三种写法**：round-trip 测试的起点是空文档，于是「不丢」这条承诺
只可能覆盖**在册** Mod；红线 R 真正要守的「未知 id 保留」在结构上**无法**被这条测试捕获，
于是一条写明「不丢」的测试给这条纪律盖了一层假安全感。另有两条 manifest 测试
（:829 / :844）同样从已知 id 出发，无一使用含未知 id 的起点）
建议: 给这条测试加一个「起点含未知 id」的变体：先写 `{"mods":{"test":{...},"ghost-mod":{"enabled":true,"config":{"k":1}}}}`，
`new` 之后 `reload_config("test", …)`，再断言 `v["mods"]["ghost-mod"]["config"]["k"] == 1`——
**这条断言在今天的实现下会红**，正是它该抓的东西
验证: 读 mod_registry.rs:938-966；`grep -rn "未知 id|unknown_id|保留|preserve" mod_registry.rs` → 仅 :839/:962 两条，都指「在册但未启用」，不是「不在册」
置信: 高

---

## BATCH-0014（2026-09-28）

### F-0014-01 · P3
file: crates/live2d-ai-desktop/src/web_api/tests_mod.rs:8-9 与 :153-167
摘录: 文件头声称的覆盖面是「`route_dispatch_table_is_stable`：**10 条**稳定 method+path → RouteId 映射」/「`route_id_as_str_is_stable`：**10 个**路由 ID 字符串契约」（:8-9）——而 `route_id_as_str_is_stable` 的函数体（:153-167）实际断言了 **13 个** `as_str()`，且**独独漏掉 `RouteId::Env`**；`route_dispatch_table_is_stable`（:75-149）也没有任何 `/api/v1/env` 的用例
影响: 可维护性/契约一致性（`RouteId` 共 14 个变体；`Env` 是 rc.2 新增的**唯一一个**此后加入的路由，而它正是**密钥端点**（`GET`/`PUT /api/v1/env`）。`as_str()` 的字符串会出现在错误体的 `endpoint` 字段与 `dispatch.rs:250-256` 的请求日志里——改了它没有任何测试会红。附带：头注的「10 条 / 10 个」是过期计数（模式 D 第 7 例））
建议: 补两条断言——`assert_eq!(match_route(&Method::Get, "/api/v1/env"), RouteId::Env);` 与 `assert_eq!(RouteId::Env.as_str(), "env");`；把头注计数改成实际值或删掉数字
验证: `grep -rn "RouteId::Env|/api/v1/env" crates/live2d-ai-desktop/src/web_api/tests_mod.rs crates/live2d-ai-desktop/src/web_api/tests_security.rs` → **零命中**（`RouteId::Env` 的唯一测试覆盖在 `env_routes.rs:375-378`，只测 `is_mutating_route`，不测路由表与 `as_str()`）
置信: 高

---

## BATCH-0015（2026-09-28）· live2d-ai-runtime 首批

### F-0015-01 · P2
file: crates/live2d-ai-runtime/src/conversation/engine.rs:534
摘录: `let worker_exit = worker.await.unwrap_or(WorkerExit::Failed);` ——上一行注释已经把行为写明了：「worker panic 同样按致命错误处理」（:532-533）。但 `JoinHandle::await` 的 `Err(JoinError)` **携带的正是 panic 消息**，`.unwrap_or` 把它整个丢弃。
调用链: TTS worker 线程 panic（HTTP 解码 / PCM 解码 / 切片越界等）→ `engine.rs:534` `worker.await` 得 `Err(JoinError)` → 静默变成 `WorkerExit::Failed` → `TurnStatus::Failed`（:542）→ `EngineEvent::Terminal{status: Failed}`（:571-578）→ 前端只看到「本轮失败」
影响: 可维护性/排障（用户可见的症状是「说了半句就没了」或「这轮失败」，而**日志里一个字都没有**：worker.rs 与 engine.rs 的 `tracing::` 命中数**均为 0**，`grep -c "tracing::"` → 0 / 0。panic 的消息与回溯被 `unwrap_or` 吞掉，WorkerExit 只有一个 3 值的枚举，携带不了任何诊断信息。这是 CONSOLIDATION-02 记的模式 E「失败不记账」的第四处）
建议: 改成 `match worker.await { Ok(e) => e, Err(je) => { tracing::error!(target: "conversation", panic = %je, "TTS worker 线程 panic，按致命错误收尾"); WorkerExit::Failed } }`；顺带给 `WorkerExit::Failed` 附一个 `reason: String` 或 `ErrorKind`，让终态能带出原因
验证: 读 engine.rs:523-585；`grep -c "tracing::" conversation/worker.rs conversation/engine.rs` → 0 / 0
置信: 高
反证: (a) 试过论证「panic 会被别处记录」——worker 的 `synthesize_sentence` 与 `tts_worker` 全文无 `tracing::`，`send_event` 也不记日志（`grep -c` 为 0 可证）；(b) 试过论证「panic 不可达」——worker 跑的是真实 HTTP 流 + `PcmS16LeDecoder` 增量解码 + `Vec` 切片，AGENTS.md 自己记录过 rc.3 那次「引擎收尾时同步发出的那批事件被静默丢掉」的时序缺陷，说明这块确实出过事；(c) 试过找 supervisor 侧的兜底——`crates/live2d-ai-desktop/src/supervisor` 侧对 `TurnStatus::Failed` 只投影成 WS `error` 帧，panic 消息早已不在信号里。

### F-0015-02 · P3
file: crates/live2d-ai-runtime/src/dialogue/sentence.rs:3-5 与 :13-14
摘录: 头注写的是绝对口径：「**契约（2026-09-10 用户裁决）：一句一单元——延迟可接受，断句不可接受。** 一个句子必须完整地交给 TTS 合成、再连续播放。因此本模块的正常路径**只按真实句读边界**切分，**绝不**按字符位置把一句话劈开。」（:3-5）——而 :13-14 的同一条注释紧接着承认：「缓冲字符数有上界（安全阀）：仅在「超过 `max_chars` 且一个真实句读都没有」时触发，触发时**优先在弱标点（逗号/顿号等）处断开**，实在没有才**按位置硬切**。」实现见 `reduce` 的 :123-136（`out.push(self.buf.drain(..cut).collect())`）
影响: 可维护性/契约一致性（AGENTS.md 把这条写成无限定红线：「分句只按真实句读边界切分；**不允许**按字符位置硬切」。实现是**有条件**的：>200 字（`DEFAULT_MAX_CHARS = 200`，:66）且无句读/或句本身超长时（:114 的 `<= self.max_chars` 判据不成立）就会走位置切。取舍本身是合理的且有史（注释记着旧值 48 会把长句劈开，TTS 出现逐段空档），但**头注的「绝不」与 AGENTS.md 的「不允许」都被写成了无条件断言**，读者会据此以为这条纪律被完全满足）
建议: 头注改成「正常路径只按句读切；**唯一例外**是超过 200 字仍无句读的安全阀，优先在弱标点断开」；并把 `DEFAULT_MAX_CHARS` 的这个性质同步进 AGENTS.md 的红线措辞
验证: 读 sentence.rs:1-30 与 :111-143 原文
置信: 高

---

## BATCH-0015 —— 三条红线的**已验证**结论（记入账本，防止下批重复怀疑）

| 红线 | 判定 | 证据 |
|---|---|---|
| **思考不进句子装配器、不进 TTS** | **✔ 双重保险 + 真测试** | ① `orchestrator.rs:59` `LlmEvent::ReasoningDelta(_) => Vec::new()`，注释写明「它一旦进句子装配器就会被合成语音」；② `engine.rs:292-307` 在**进 assembler 之前**先拦一道并只转发 UI；③ 回归 `llm.rs:503-529` `reasoning_never_reaches_the_sentence_assembler` **能失败**（驱动真实 `DialogueAssembler`，断言无 `SentenceReady`，并带 `TextDelta` 正向对照） |
| **一句一单元**（不按字符硬切） | **✔ 有条件满足**，例外见 F-0015-02 | `sentence.rs` 全量读；200 字安全阀优先弱标点（:130-135），且 `last_soft_break_at_or_before` 带 `.filter(\|end\| *end > 0)`（:230）保证不会切出空句 |
| **空句不进 TTS** | **✔ 真实执行点** | `worker.rs:105` `if !ctx.client.tts().is_configured() \|\| job.text.trim().is_empty()` → 不发 HTTP，只发空 `final_chunk` + `SentenceVoiced`；注释记录了 2026-09-11 的实测缺陷（`"你好！\n"` 切出纯空白句 → 上游 400 → TTS 错误 fatal → 整轮判失败） |
| **`clean_for_tts` 不得被旁路**（红线 P） | **✔ 两个构造点都干净** | `engine.rs:357-360` 与 `:488-491` 两处 `TtsJob` 的 `text` 均为 `cleaned`；且有 e2e 回归 `tests/conversation_engine_performance.rs:429-455`「D23：每段上屏文本 == 送 TTS 文本 == clean_for_tts(段)」 |
| **`sentence_seq` 在两条路径上一致** | **✔** | 流式路径 :334 与表演层路径 :468 **都无条件** `sentence_seq += 1`（含清洗后为空的段），:465-466 明写「D24（空白段不跳号）」——异步导演按 seq 对齐的前提成立 |

## 本批主动推翻的假设（防下批重走）
1. 「表演层开启时 `sentence_seq` 会与流式路径错位（空段跳号）」——**证伪**：两条路径都无条件自增，
   且 `covers_upto_seq = sentences.len()`（:445）与起始 seq=0 自洽。
2. 「`clean_for_tts` 被应用两次（performance fallback 先洗、engine 再洗）会破坏内容」——**证伪**：
   `clean.rs:322-326` 有幂等回归（`clean_for_tts(clean_for_tts(x)) == clean_for_tts(x)`，对多个样本循环），
   且逐条推演了 `**`/单 `*`/全半角括号/反引号/行首标记/CRLF 的二次作用，均为不动点。
3. 「`remove_paired_spans` 的首个匹配配对会吞掉后半句话（违背「宁可不剥也不吞后文」）」——**证伪**：
   开括号多于闭括号时每个开括号单独落到 `None` 分支被原样保留（:84-88），实测 `(a（b）` → `（a`。

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 1 ｜ 另：**5 条红线全部完成验证**（3 条 ✔完全满足、1 条有条件满足、1 条 P 侧 ✔）

---

## BATCH-0017（2026-09-28）· 错误码契约面（第 6 条红线的实现地）

### F-0017-01 · P2  ⭐ **候选池 C-5 在本批结案**（自 BATCH-0003 挂起至今）
file: crates/live2d-ai-runtime/src/error.rs:110-119（body 截断）→ `AppErrorEvent.message` → 两处消费
摘录（源头）:
```rust
pub(crate) fn status(status: StatusCode, body: &str) -> Self {          // error.rs:110
    let mut snippet: String = body.chars().take(MAX_SNIPPET_CHARS).collect();
    ...
    Self::Status { status, body: snippet }                              // :115-118 —— 上游响应体片段进 Error
}
```
摘录（消费 1：WS 帧，**无任何脱敏**）: `AppEvent::Error(err) => { frame.set_type("error"); frame.set_data(serde_json::json!({ "code": err.code, "stage": err.stage, "message": err.message, "hint": err.hint, "epoch": err.epoch, "fatal": err.fatal, })); }`（`web_api/ws/events.rs:189-199`）
摘录（消费 2：日志，**只有 key=value 模式脱敏**）: `tracing::error!( code = %app_err.code, ..., "{app_err_message}", app_err_message = app_err.message, )`（`supervisor/handlers.rs:147-155`）；而脱敏器只挂在**文件层**（`logging.rs:18` `file layer：…with_writer(SanitizingWriter::new(...))`），其匹配规则是**键名驱动**：
```rust
let sensitive = |name: &str| { let lower = name.to_ascii_lowercase();
    matches!(lower.as_str(), "api_key" | "apikey" | "password" | "token" | "auth") };   // logging.rs:164-170
```
调用链: 上游非 2xx → `llm.rs:310-312` `Error::status(status, &body)` → `error.rs:110` 截断存 `body` →
`ErrorKind::Llm(e)` → `AppErrorEvent::from_kind`（`supervisor/handlers.rs:146`）→
① `tracing::error!`（:147）经 `SanitizingWriter` 的 `redact`；
② `emit(AppEvent::Error(app_err))`（:157）→ `ws/events.rs:191-197` → **浏览器**。
影响: 安全/契约一致性（AGENTS.md 红线「密钥不进 GET/日志/WS/导出」在此路径上**没有兜底**：
`redact` 只认「敏感键名 + `=`/`:`」的**结构化**形态，认不出**散文里**的密钥；
而 OpenAI 兼容上游的 401 响应体典型形态是
`{"error":{"message":"Incorrect API key provided: sk-proj-abc***XYZ"}}`——key 叫 `message`，
`redact` 不会命中。WS 帧那一路**连 `redact` 都不经过**。）
建议: ① `redact` 增加一条「疑似密钥形状」规则（如 `sk-` / `sk_` 前缀的长 token、
`Bearer <token>`）作为兜底，与键名规则并存；② `AppErrorEvent` 在构造 WS 帧前对
`message` 走一次 `logging::redact`（该函数已是 `pub`，见 logging.rs:157）。
验证: 读 error.rs:78-120、logging.rs:157-190（`redact` 的键名白名单与扫描逻辑）、
ws/events.rs:189-199、supervisor/handlers.rs:136-165
置信: 高（机制逐跳有据：body 确实进 `message`，`message` 确实同时进 WS 帧与日志，
`redact` 的匹配规则确实只看键名）/ 中（**是否真出现可用密钥取决于上游**：
主流 OpenAI 兼容服务的 401 只回**打码后的**片段；但本地网关/代理类上游可能回显全量，
且即便只是打码片段，密钥片段进浏览器与日志也违反红线的字面要求）
反证: (a) 试过论证「`SanitizingWriter` 也包住了 WS 帧」——不可能，它只实现 `Write`
并只被 file layer 使用（logging.rs:18 / :280-293），WS 帧走的是 serde_json 序列化路径；
(b) 试过论证「上游 401 体一定不含密钥」——OpenAI 官方体是
`Incorrect API key provided: sk-abc***`（**含打码片段**，非空），故「一定不含」不成立；
(c) 试过找别的兜底——`MAX_SNIPPET_CHARS` 只截断长度不做脱敏；`hint_for` 产出的
提示文案是写死的常量，不含上游内容。

---

## BATCH-0017 —— 第 6 条红线的验证结论（记入账本）

**红线「链路错误必须同时进后端 tracing 与前端 WS error 帧，两处 code 是同一个字符串」—— ✔ 满足，且是强形式**

- **唯一构造点**：`let app_err = AppErrorEvent::from_kind(kind, *epoch);`（`supervisor/handlers.rs:146`）
  之后**三个消费者全部用同一个值**：
  ① `tracing::error!(code = %app_err.code, stage = %app_err.stage, epoch = …, fatal = …)`（:147-155）；
  ② `println!("\n{}", app_err.log_line())`（:156）；
  ③ `emit(AppEvent::Error(app_err))`（:157）→ `ws/events.rs:191-197` 投影出 `error` 帧的
  `code`/`stage`/`message`/`hint`/`epoch`/`fatal`。
  ⇒ **不是「两处各算一遍然后对齐」，而是「算一次、发三处」**——这是该契约能成立的最强形式。
- `code()` 的来源唯一：`ErrorKind::code()`（`error_code.rs:44-51`）= `format!("{stage}_{}", e.code_suffix())`；
  `code_suffix()`（`error.rs:85-96`）把上游状态码直接编进码（`upstream_401`），这正是
  「401 缺密钥」与「429 限流」能一眼分开的原因（模块头注 :83-85 记了这段动机）。
- 回归锁定：`error_code.rs:172-189` `code_always_starts_with_stage`（断言 `code()` 以 `stage()` 开头，
  注释写明「回归测试锁定」）。
- **前端不猜文案**（同条红线的后半句）✔：`ws_frame.dart:513` `code: _str(data['code']) ?? 'error'`
  从帧里取码；`ui/error_actions.dart:33` `if (code.startsWith('llm_') || code == 'no_supervisor')`
  **按码前缀分支**。全仓未发现「`contains('未配置')` / `startsWith('llm')` 之类从文案猜类型」的写法
  （grep `contains('未配置')|contains("key")|startsWith('llm|startsWith("llm` 仅命中上面那处按 `code` 的判定）。
- `hint()` 的可执行性 ✔：`error_code.rs:112-157` 的 `hint_for` **全变体覆盖、无 `_` 兜底**
  （:110-111 注释明写「新增 `Error` 变体时编译器会在这里点名」），
  401/403 的提示直指 `api_key_env` 指向的变量在「后端进程」环境里（:117-119），
  并追加一句反驳「是不是改提示词改坏的」（:87 的 `non_auth_note`）。

---

## BATCH-0018（2026-09-28）· TTS 侧：解码 / WAV 分支 / 音频模块结构

### F-0018-01 · P2
file: crates/live2d-ai-runtime/src/audio/rms.rs（242 行）、crates/live2d-ai-runtime/src/audio/queue.rs（174 行）、crates/live2d-ai-runtime/src/lib.rs:54-65
摘录（crate 根 doctext 把这条流水线呈现为**标准做法**）:
```rust
//! let mut pcm = PcmS16LeDecoder::new(client.tts().spec);
//! let mut queue = SampleQueue::new(24_000)?; // 1 秒 @24 kHz 的有界缓冲      // lib.rs:56
//! let mut meter = RmsMeter::new(client.tts().spec, 50.0, 0.03, 0.10)?;      // lib.rs:57
//! while let Some(chunk) = speech.next().await {
//!     let samples = pcm.decode(&chunk?);
//!     if queue.push_slice(&samples) < samples.len() { … }
//!     // 播放回调从 queue 取走样本送声卡后，把**实际消费**的样本喂给 meter。   // lib.rs:63
//! }
```
而**生产路径不经过 `SampleQueue` 与 `RmsMeter`**：
`conversation/worker.rs:176-224` 解码后直接把样本塞进 `EngineEvent::AudioChunk`
（经 `tokio::sync::mpsc`），由 supervisor 转到 `web_api/ws/audio.rs`，
那里的音量由**另一个实现**算出：`let volume = rms_s16le_mono(slice);`（`web_api/ws/audio.rs:118`，
函数体在 :149）；cpal 播放侧也**不用** `SampleQueue`——
`crates/live2d-ai-desktop/src/audio/` 自带 `ring.rs` / `prepared.rs` / `output.rs`。
调用链（生产）: `tts.rs:48 synthesize_speech` → `worker.rs:158 PcmS16LeDecoder::new` →
:176 `decode` → :182 `EngineEvent::AudioChunk` → supervisor → `ws/audio.rs:118 rms_s16le_mono` → WS 帧
影响: 可维护性（**416 行**导出型音频代码在生产路径上零调用者，却被 crate 根 doctext 演示成
「解码 → 有界队列 → RMS 表」的标准流水线。读者（含后续维护者与本审计的后来批次）据 crate 文档
理解音频链路会得到**与生产相反**的图景；而 `cargo test` 全绿（doctest 会跑），
进一步强化了「这些是活的」错觉。同一职责（RMS 音量）在两处各有一份实现，行为不保证一致）
建议: ① 若 `SampleQueue` / `RmsMeter` 确已无使用方，删除并从 `lib.rs:93` 与 doctext 中撤下；
② 若要保留（例如给未来的 cpal 路径用），把 doctext 换成**生产实际链路**，
或在 `RmsMeter` 头注写明「当前未被生产使用，线上音量走 `web_api::ws::audio::rms_s16le_mono`」；
③ 更根本的一条：`ws/audio.rs` 的 `rms_s16le_mono` 应下沉到 `audio::rms` 复用，避免两份算法
验证: `grep -rn "rms::|RmsWindow|RmsMeter|SampleQueue" crates/ --include=*.rs` → 除自身定义、
`audio/mod.rs:47/49` 的 re-export、`lib.rs:26/56/93` 的 doctext 与 re-export 外，**无任何调用点**；
`grep -rln cpal crates/` 确认 cpal 路径在 `desktop/src/audio/{output,ring,prepared}.rs`，用自有 ring 缓冲
置信: 高（调用点已穷举；生产链路的每一跳都有 file:line）
反证: (a) 试过论证「cpal 侧可能经 `SampleQueue`」——`desktop/src/audio/` 全目录 grep 无该符号，
其缓冲是 `ring.rs` 的自实现；(b) 试过论证「`RmsMeter` 被 desktop 用了」——desktop 侧的 RMS 是
`web_api/ws/audio.rs:149` 的独立 18 行实现，两者无调用关系；
(c) 试过把它们降级为「预留 API」——那也是缺陷：预留接口应当有头注声明，而 `rms.rs`/`queue.rs`
的头注（rms.rs 1-26、queue.rs 1-15）读起来都像在描述**当前**职责。

---

## BATCH-0018 —— 音频链路不变量验证（记入账本）

| 不变量 | 判定 | 证据 |
|---|---|---|
| 每句**恰好一个** `final_chunk` | ✔ | 裸 PCM 路径 :206-220（`final_chunk: true` 只此一处）与 WAV 路径 :316-330 同款；两条路径的循环条件不同（PCM `>= chunk_samples` / WAV `> chunk_samples`）导致「整句恰好整数块」时末块形态不同，但**两侧都恰好一个 final** |
| `first_chunk` 的**唯一**来源是「本句是否已发过块」 | ✔ | 两路径都用 `first_chunk: !sent_any`（:188 / :216 / :304 / :326），无第二处推导 |
| 「先完整合成、再上屏文字」三处收口都发 `SentenceVoiced` | ✔ | `worker.rs:122`（TTS 未配置/空句）、`:222`（裸 PCM 收尾）、`:332`（WAV 解析后）——`emit_sentence_voiced` 单列正是因为「有三处收口」（:232-234 注释写明） |
| PCM 解码器任意字节切割安全 | ✔ | `decoder.rs:48-64` 奇偶统一（`carry: Option<u8>`），`Vec::with_capacity(total/2)` 容量精确；`finish()`（:71-76）对悬挂字节回 `TruncatedPcm`（**不补零、不静默丢弃**，与「绝不臆造」一致） |
| 格式门禁 | ✔ | `worker.rs:131-135` `ensure_supported_format` 失败 → `ErrorKind::Decode`（fatal），不发 HTTP |
| WAV 上限防打满内存 | ✔ | `worker.rs:275-279` 超 `MAX_WAV_BYTES`(64MiB) 回 Decode 错误而非静默截断 |

---

## BATCH-0019（2026-09-28）· 密钥真源 + 表演层 + WAV 解析

### F-0019-01 · P2
file: crates/live2d-ai-runtime/src/performance/mod.rs:352-362（与 :370）
摘录:
```rust
Err(e) => {
    self.stats.fallbacks.fetch_add(1, Ordering::Relaxed);      // :353 ← 第一次 +1
    self.stats.last_reason.store(3, Ordering::Relaxed);        // :354
    tracing::warn!(target: "performance", code = e.code(), "{}（静默回退规则层）", e.message());
    self.fallback(assistant, FallbackReason::InvalidPlan)      // :361 → 进入 fallback()
}
...
fn fallback(&self, assistant: &str, reason: FallbackReason) -> Resolution {   // :367
    // 口径写死：**每一次没用上表演层 JSON 的轮次都计入 fallbacks**（含关闸）。
    self.stats.fallbacks.fetch_add(1, Ordering::Relaxed);      // :370 ← 第二次 +1
```
调用链: `PerformanceRuntime::resolve`（:302 `parse_plan` 失败）→ :353 `fallbacks += 1` →
:361 `self.fallback(..., InvalidPlan)` → :370 `fallbacks += 1` ⇒ **一轮失败计 2 次**
→ `PerformanceStats::fallbacks()`（:141）→ `state_json`（:177 `"fallbacks": self.fallbacks()`）
→ `web_api/app_routes.rs:301` `fallbacks: stats…fallbacks()` → `GET /api/v1/app/status` 的
`PerformanceStatus.fallbacks`（DTO 定义见 `web_api/dto.rs:94`）
影响: 用户可见/文案与实现不一致（这个计数**就是**该面板的立身之本——`dto.rs:71` 写明
「这是『表演层到底有没有在工作』的**可观察面**：…`fallbacks`（回退几轮）…」。
而**唯一最常见的真实失败**恰恰是 `InvalidPlan`（模型输出的 JSON 过不了校验器，
`FallbackReason::InvalidPlan` 的文档 :78 就是这么定义的），也就是说这个指标**恰好在
最需要它的时候系统性偏大**。更糟的是它与同一块面板里的 `last_fallback` 自相矛盾：
一轮失败会显示 `fallbacks: 2, last_fallback: performance_plan_invalid`——用户无法对账，
只会连带怀疑整块面板）
建议: 删掉 :353 那一行（`fallback()` 内部已统一计数，且**另外三条**回退路径
（:285 `Disabled` / :288 `EmptyAssistant` / :297 `RequestFailed`）都**不**预先 +1，
只有 `InvalidPlan` 多加了一次——这一处是不对称，不是口径）。若要保留对称，则应把
计数从 `fallback()` 里移出、由 5 个调用点各自负责，但那更易漏
验证: 读 performance/mod.rs:280-390 全段；`grep -n "enum FallbackReason" -A 40`
确认 `InvalidPlan.as_u8() == 3`（故 :354 的硬编码 3 与 :372 的再写一致，`last_reason` 正确，
**只有计数器错**）；`grep -n "self.fallback(" ` → 4 个调用点，只有 :361 这条前面有多余的 +1
置信: 高
反证: (a) §12.1 要求「去测试里找覆盖该路径的测试」——**找到了**：
`performance/tests.rs:689-690` 与 `:760-761` 确实驱动了 `InvalidPlan` 路径并断言
`out.reason == FallbackReason::InvalidPlan` 与 `rt.stats().last_fallback() == "performance_plan_invalid"`。
**但两条都不断言 `fallbacks()`**；全仓唯一的 `fallbacks()` 断言在 `tests.rs:648`，
且断言的是 `0`（「没发生回退」那条）。⇒ **测试走过了这条路径却看不见这个字段**，
既不是假绿灯也不是漏测路径，而是「断言选错了字段」——比两者都隐蔽。
(b) 试过论证「双计是有意的（InvalidPlan 更严重）」——若是，:368 的口径注释
「每一次没用上表演层 JSON 的轮次都计入 fallbacks」应当按**轮次**计数，而一轮就是 1；
且另外三条路径都是 1，不对称无法用口径解释。
(c) 试过论证「计数只用于内部判断、不进 UI」——`app_routes.rs:301` 把它直接放进
`GET /api/v1/app/status`，而 BATCH-0009 已确认该 DTO 的 `performance` 段是给
「表演层有没有在工作」用的可观察面。

---

## BATCH-0019 —— 红线验证台账更新

| 红线 | 判定 | 证据 |
|---|---|---|
| **密钥读取只能走 `secrets::lookup`**（红线 R 核心） | **✔ 全仓满足** | `grep -rn "std::env::var\|env::var("` 全仓非测试命中 **15 处**，逐一归类：`LIVE2D_AI_*` 控制开关（ALLOW_NO_ORIGIN / FLUTTER_WEB_DIR / WEB_AUDIO / UNMUTE_AUDIO / MUTE_AUDIO / WS_AUDIO / ENV_FILE）、显示与桌面变量（XDG_SESSION_TYPE / WAYLAND_DISPLAY / DISPLAY）、路径变量（XDG_DATA_HOME / HOME），以及 `secrets.rs:154`（**就在 `lookup` 内部**，是契约写的第二优先级）。**没有任何一处直接读 API key**。 |
| **日志不记密钥 / 性能层不记正文** | ✔ | `performance/mod.rs:331-337` `tracing::info!(… "performance plan ok：{}", plan.summary())`，注释明写「**不打全文**、绝无密钥」；:322-330 逐条 warning 带 `code=` 结构化字段、只有 id 没有正文 |
| **WAV 解析不许假装支持** | ✔ | `audio/wav.rs:80-88` 非 PCM(1) / 非 16-bit 一律 `UnsupportedFormat`；:52-53 声明长度超实际时**取交集**（容忍流式 0xFFFFFFFF）；:73 RIFF 奇数填充位对齐；:107-109 截断 PCM 报错不补零；:62-66 `WAVE_FORMAT_EXTENSIBLE` 子格式 GUID 处理 |

---

## BATCH-0020（2026-09-28）· 表演层校验器本体（红线 P 最后一环）

### F-0020-01 · P1 【**已撤回（B0273 · Phase 4 证伪）** —— 我的**前提就是错的**：那不是主链的客户端】
> 🔻 **撤回理由（机械可核）**：
> ```
> performance/client.rs:1  「**表演层** HTTP 客户端：**非流式** `POST {base_url}/chat/completions`」
> performance/client.rs:4-6  「**独立配置**：base_url / model / timeout_ms / api_key_env ——
>                        **独立于 `[llm]`（主模型）**；**二路断了不影响主链**」
> ```
> ⇒ ⇒ **它不是主链的客户端**，而是**表演层自己的**、**显式声明独立**的那一路
> ⇒ ⇒ 而主链的默认在 `settings.rs:187 pub const DEFAULT_MAX_TOKENS: u32 = 4096;`
> **（那是主链的，另有其人）**
> ⇒ ⇒ ⇒ **我原来的框架「`1024` vs 主链 `4096` ⇒ 机制对、**常量错**」前提不成立**

> ⭐ **而「每个调用点各有自己的预算」正是这一层的既有设计**（同族三处）：
> `director/staging_http.rs:48 MAX_TOKENS = 512` · `memory/summary.rs:54 MAX_TOKENS = 512`
> · `performance/client.rs:139 MAX_TOKENS = 1_024` ⇒ ⇒ **一场调用一个预算**
> ⇒ 而这一路返回的是**一份 JSON 动作计划**（`:1`「表演层要的是一份 JSON，不是逐字流」），
> **比对话小是合理的** ⇒ ⇒ 我**没有任何证据**说 1024 不合适
> ⇒ ⇒ **按我自己的规矩**（「拿不出证据的就不进 FINDINGS」）⇒ **撤回**

> ⚠ **若日后要重启这条**，需要的**不是**与主链比较，而是**「一份动作计划 JSON 到底需要多少输出 token」
> **这一条证据**（例如真实动作计划的 token 长度）—— **在仓库里拿不到**，故不列为 P1/P2/P3。
> **原文保留**在下方 B0020 段内供追溯。


file: crates/live2d-ai-runtime/src/performance/client.rs:138-139 与 crates/live2d-ai-runtime/src/performance/plan.rs:65
摘录（生成侧输出预算）:
```rust
/// 输出 token 上限（一份 plan 很短；显式给硬上限，不靠提示词求模型守规矩）。
pub const MAX_TOKENS: u32 = 1_024;        // client.rs:138-139
```
摘录（校验侧接受的体量上限）:
```rust
pub const MAX_SEGMENTS: usize = 64;       // plan.rs:61
pub const MAX_SEGMENT_CHARS: usize = 4_000;   // plan.rs:65
```
而两者之间有一道**硬约束**（不是建议）:
```rust
// V1 核心不变量：逐码点拼接恒等；空原文必须是空切分方案。
if segments.concat() != source || (source.is_empty() && !segments.is_empty()) {
    return Err(PlanError::SegmentsNotPartition);      // plan.rs:610-613
}
```
⇒ v1 的 plan **必须逐字复现整条回复**（`source` = 主链的 `assistant_text`，
经 `performance/mod.rs:302` `parse_plan(&raw, &self.allow, assistant)` 传入）。
调用链: `client.rs:260` `"max_tokens": MAX_TOKENS` → 上游在 1024 token 处**截断** →
`strip_code_fence` → `parse_plan` → `serde_json::from_str` 失败 → `PlanError` →
`performance/mod.rs:352-361` `Err(e)` 分支 → `FallbackReason::InvalidPlan`
→ 每轮回落到 v0 口径（`clean_for_tts(原文)` + 规则 cue，`mod.rs:367-390`）
影响: 用户可见/资产链路（**红线 P 的表演层在长回复上静默失效**）。量级：校验侧允许
4 000 字，生成侧只有 1024 token；中文按 0.7–1.4 token/字计，
1024 token ≈ **730–1 460 中文字**，即校验器接受体量的 **1/5 ~ 1/4**。
任何超过约 1 000 字的中文回复都会**必然**截断 → JSON 残缺 → 必然回退。
用户侧看到的是「这段话没有表情/动作」，而界面**没有任何提示**说明表演层为何不生效；
`fallbacks` 计数器会涨（但见 **F-0019-01**：这条路径的计数还是双计的）
建议: 两处取齐——① `MAX_TOKENS` 按「最坏情况 = 4 000 字正文 + 64 段结构 + 16 条 cue」定
（例如 8 192）；② 或反过来收 `MAX_SEGMENT_CHARS` 到与 1024 token 相称的体量，
并在 `parse_v1` 触到上限时回**明确**错误码（让用户知道是配置太小而不是模型不听话）。
**注释的前提「一份 plan 很短」已被 v1 的 segments 设计推翻**（v0 的 `speak` 是一整段，
v1 的 `segments` 逐字复现全文）——这一句是本条最直接的证据
验证: 读 client.rs:138-139 / :251-273（请求体含 `max_tokens`）、plan.rs:61-79（全部数值常量）、
plan.rs:610-613（拼接恒等硬约束）、performance/mod.rs:300-363（回退分支）
置信: 高（两个常量 + 一条硬约束，同一特性内可静态比较，不需要跑门禁）
反证: (a) 试过论证「`max_tokens` 可以约束回复长度，所以 plan 本来就短」——恰恰相反：
plan 要复现的 `source` 是主链**已经生成完**的整条回复，表演层无权缩短它；
缩短就是 `SegmentsNotPartition` 硬失败。(b) 试过论证「truncated JSON 会被宽松降级而不是报错」——
`parse_plan` 走 `serde_json::from_str`，截断的 JSON 必然解析失败 → `PlanError` → 回退；
不存在「宽松接受」路径。(c) 试过论证「1024 对典型回复够用」——典型聊天回复 300–800 字够用，
但**详细型回复**（本项目是「人设陪伴」型对话，展开讲解时）经常 1 000–2 000 字，
落在截断区。**分词器差异**让我无法给出精确字符阈值，故按区间表述，结论不受影响。

### F-0020-02 · P3
file: crates/live2d-ai-runtime/src/performance/plan.rs:735-739（标签写入点 :737）
摘录:
```rust
CueAnchor::AfterPrev => match previous {
    Some((prev_seq, false)) => (prev_seq, "after_prev".to_string()),
    Some((prev_seq, true))  => (prev_seq, "now".to_string()),   // :737 —— 标签写 "now"，seq 却是 prev_seq
    None => (1, "now".to_string()),
},
```
调用链: `plan.rs:737` `at_resolved = "now"` → :742 `cue.at_resolved = …` → :745 `to_wire_cue`
→ `plan.rs:184` `at: Some(self.at_resolved.clone())` → `PerformanceCue::to_json` → WS `action_cue`
→ `ws_frame.dart:189` `at: _str(j['at'])` → `main.dart:826 / :843` 原样透传给 `stage.applyPreset(...)`
影响: 可维护性/文案与实现不一致（**已核实不影响定位**）：前端按 `sentence_seq` 持有 cue 并在
该句音频 `first_chunk` 时应用（`ws_frame.dart:198-199` 的文档明写「前端按 sentence_seq 持有 cue…」），
而 `sentence_seq` 在这条退化路径上**没有**被改写（仍是 `prev_seq`），所以位置是对的。
问题只在于线上载荷里 `at` 写着 `"now"`（= 第 1 句）而 `sentence_seq` 是 5 之类——
**同一个 cue 的两个键自相矛盾**，任何按 `at` 判断的第三方消费者会得到错误结论。
本仓自己的渲染面是否用 `at` 做判断**未核实**（`stage.applyPreset` 的实现未读）
建议: 退化时 `at_resolved` 写成 `format!("now@seg:{prev_seq}")` 之类能同时表达两者的形式，
或直接沿用 `seg:{prev_seq}`（语义更贴合实际生效段）；并在 `PerformanceCue` 的字段注里
写明「`at` 是**诊断标签**，定位一律以 `sentence_seq` 为准」
置信: 高（标签与 seq 的矛盾可直接读出）/ 中（第三方或渲染面按 `at` 判断的可达性未核实）

---

## BATCH-0020 —— 红线 P 的验证结论

**红线 P「0.2.0 表演资产：导演层接线的 Live2D 表演 + TTS 过滤」—— 部分满足，且有一处高影响缺口**

| 判据 | 判定 | 证据 |
|---|---|---|
| 表演层**不得改写**用户听到的正文 | **✔ 硬约束** | `plan.rs:610-613` `segments.concat() != source` 直接 `Err(SegmentsNotPartition)`——是**错误**不是警告。表演 LLM 只能选择「在哪切」，不能改一个字 |
| cue 的字段级校验 + 逐条降级 | ✔ | `plan.rs:640-699`：字段类型错 → `CueField` 整条失败；轴键不允许 → 丢键 + `AxisNotAllowed` warning（:670-679）；expression id 不在能力集 → 丢 cue + `ExpressionUnknownId`（:689-694）。**降级逐条带 code**（`PlanWarning::code()`） |
| 锚点 `seg:N` 有界 | ✔ | `parse_anchor`（plan.rs:773-782）1-based 且 `n <= seg_count`，越界 → `BadAnchor` |
| 强度/TTL/轴值全部钳位 | ✔ | intensity `clamp(1,3)`（:718）、ttl `clamp(MIN,MAX)`（:709）、轴 `clamp(-1.0,1.0)`（:766） |
| 请求体不含密钥 | ✔ | `client.rs:251-273` 只发 model/messages/stream/temperature/max_tokens；key 只进 `Authorization` 头（:289-291）。`temperature: 0` 保证 plan 可复现 |
| 结构化输出与校验器同源 | ✔ | `json_schema_strict`（plan.rs:787）由本文件常量拼出，注释写明「边界就是校验器认的那一份（单一真源）」 |
| **生成侧输出预算 ≥ 校验侧体量上限** | **✘ 违反** | **F-0020-01（P1）** |

---

## BATCH-0021（2026-09-28）· 设置的真源与三态补丁（runtime 收尾）· 维度 E 主战场

### F-0021-01 · P2
file: crates/live2d-ai-runtime/src/settings/view.rs:45-48、:178-189；crates/live2d-ai-desktop/src/web_api/dto.rs:67-72
摘录（服务端**确实**算了两处表演层可观察面）:
```rust
// view.rs:45-48 —— SettingsView 里的 performance 段
/// 表演层段（`[performance]`，2026-09-22）。**只读展示**：启停与端点写在
/// `live2d-ai.toml`（本波不提供 PATCH 入口），**面板据此显示现状** +
/// 「主模型不负责表演；表演层每轮 JSON」。
pub performance: PerformanceView,        // 7 字段：enabled/wired/base_url/model/has_api_key/timeout_ms/structured
```
```rust
// dto.rs:67-72 —— AppStatus 里的 performance 段
/// 表演层摘要（2026-09-22）：`[performance]` 段配置 + 运行计数。
/// 这是「表演层到底有没有在工作」的**可观察面**：`enabled`、`wired`、`plans`、
/// `fallbacks`、`last_fallback`。
pub performance: PerformanceStatus,      // 12 字段
```
而**前端一个键都不读**（三重验证）:
- `grep -rni "performance" shell/flutter/lib/` → 14 处命中**全部**是 `performance.now()`（JS 计时器，
  4 处）或中文注释/常量文案，**无一处读 JSON 的 `performance` 键**；
- 诊断面板读的是 `app/status` 的另外 7 个键：`status['llm']` / `['tts']` / `['active_model_id']`
  / `['current_epoch']` / `['audio']` / `['uptime_s']` / `['dev_mode']`
  （`dev_tools_section.dart:1075-1189`）——**`['performance']` 不在其中**；
- 设置面板改用**一句硬编码文案**指路：`const String kLlmPerformanceDescription = …'配置只在
  live2d-ai.toml 的 [performance] 段'`（`llm_section.dart:34-42`，渲染于 :149-150）
影响: 用户可见/契约一致性（服务端**按契约算好并下发**的两块表演层可观察面，**在产品里没有任何
消费者**；设置面板不读配置，而是**硬编码一句「去改 toml」**。这与项目「配置真源 + 前端可编辑 +
热重载」的既定方向相反。与两条已记发现构成一条完整链：
**F-0020-01**（长回复必然截断→静默回退）→ **F-0019-01**（唯一能说明这件事的 `fallbacks` 计数还是双计）
→ **本条**（这块计数没有任何界面会显示）⇒ **表演层从触发失败到用户察觉，全程不可观测**）
建议: ① 诊断面板的 status 区补 `['performance']` 一行（`plans`/`fallbacks`/`cue_turns`/`last_fallback`/
  `wired` 直接可打，与现有 `audio` 同一行式样）；② 设置面板的「外观与互动」或 `llm_section`
  用 `SettingsView.performance` 的 7 个字段替掉硬编码文案（`enabled`/`wired` 已是**生效值**，
  `base_url`/`model`/`timeout_ms`/`structured` 也都回的是生效值，见 view.rs:176-189）；
③ 若确实决定不做前端展示，则把两处文档注释改成事实陈述，别再写「面板据此显示现状」
验证: 读 view.rs:1-58 / :150-200 / :225-294；读 `web_api/dto.rs:67-106`（B0009 已读）；
`grep -rni "performance" shell/flutter/lib/`、`grep -n "status\[" dev_tools_section.dart`、
`sed -n '30,45p;145,155p' llm_section.dart`
置信: 高（服务端字段存在且正确 + 前端零引用的三重 grep 证据都直接可复跑）
反证: (a) 试过论证「前端从别的端点读表演层」——`GET /api/v1/mods` 的 `settings_spec` 是 **Mod** 的
表单（`mods_routes.rs:427-480`），主链 `[performance]` 段**不在任何 Mod 的 spec 里**
（`external_input_settings_spec` 只有 listen_port/token/text_template/prefix）；
`GET /api/v1/app/status` 的 `performance` 块也无人读（已验）。(b) 试过论证「读了但用了别名」——
`grep -rni performance` 覆盖了任意大小写与任意位置，14 处命中逐一分类后无一读该键。
(c) 试过论证「`performance` 会经 `llm_section` 的某个 model 类间接读到」——`llm_section.dart`
只用到两个 `const String` 文案常量（:34/:37），不接触 settings JSON。

### 回填 F-0002-03（补上同仓的正确参照实现，使其从「两套口径」精确为「一处漏改」）
本批在 `settings/view.rs` 读到了**正确**的实现：`fn key_is_ready(env_name, lookup) -> bool`
（:246-251，判据 = 「声明了非空名」**且** `lookup(name)` 非空），且
`settings_to_view_with_keys`（:266-275）**三段都用它**：
```rust
view.llm.has_api_key          = key_is_ready(s.llm.api_key_env.as_deref(), lookup);          // :271
view.tts.has_api_key          = key_is_ready(s.tts.api_key_env.as_deref(), lookup);          // :272
view.performance.has_api_key  = key_is_ready(s.performance.api_key_env.as_deref(), lookup);  // :273
```
而 `From<&PerformanceSettings> for PerformanceView`（:190-200）与 `From<&LlmSettings> for LlmView`
（:150-159）**都只做「声明了名字」的弱判定**——那是**刻意的弱口径**（view.rs:6-9 说明纯展示/egui 用它）。
⇒ 结论精确化：`SettingsView` 的三段口径**一致且正确**；**唯一的例外**是
`web_api/app_routes.rs:292-295` 手搓的 `dto::PerformanceStatus.has_api_key`
（`p.api_key_env.as_deref().is_some_and(|n| !n.trim().is_empty())`）——它**没有**走 `key_is_ready`，
而 `build_status` 手上有 `env_lookup` 形参却只传给了 `llm_status_from` / `tts_status_from`。
修法因此是现成的：把那一行换成 `key_is_ready` 同款判据即可（该函数目前是私有，
需 `pub(crate)` 或在 app_routes 侧实现同款）。

---

## CONSOLIDATION-03 附：数字更正记录（如实，不静默改写）
- **F-0006-03** 的「验证」段原写「11 命中，其中 3 处在生产路径」。BATCH-0022 用更精确口径复核：
  `#[cfg(test)] mod tests` 起于 `mod_registry.rs:730`；`lock().unwrap()` **全文件 16 命中**，
  生产路径为 **:334 / :434 / :617**（三条**未变**）。正确分解是 **16 总 / 3 生产**。
  结论不受影响，数字已更正。

---

## BATCH-0023（2026-09-28）· **前端首批** —— 平台视图指针（红线 M）+ 高频通道（红线 O）

### F-0023-01 · P2
file: shell/flutter/lib/live2d/stage_pointer_interceptor.dart:72-82（不变量声明）
摘录（这条不变量是什么）:
```dart
/// # [enabled]：垫层自己也要能被关掉（2026-09-11，P1-1 加的）
///
/// 垫层的代价是「这块区域的指针回到父页」——**包括它没收起来的时候**。
/// 一旦某个控件改成「折叠而不是卸载」（见 `CollapsiblePanel`：
/// 折起来时子树仍在树里），它外面那层垫层也会一直留着：收起后宽度为 0
/// 看着没事，但只要它的盒子还有面积，舞台那一块就**再也拖不动模型**了。
///
/// 所以垫层必须能跟着收起：`enabled: false` 时把 DOM 元素的
/// `pointer-events` 设成 `none`，指针重新落到 iframe（模型照常可拖）。
```
调用链（两处常驻点，**实现是正确的**）:
`app_shell.dart:722-723` `StagePointerInterceptor(enabled: widget.wsStatus != WsStatus.connected, child: CollapsiblePanel(expanded: …))`
（断线横幅：连上时 `CollapsiblePanel` **折叠但不卸载** ⇒ `enabled: false` 归还指针 ✔）；
`app_shell.dart:843-847` `StagePointerInterceptor(enabled: settingsOpen, child: _compactSettingsPage())`
（compact 整页设置在 `PageCrossFade` 里**两页常驻** ⇒ 收起时归还 ✔，注释自陈
「否则 compact 下**永远拖不动模型**」）。
影响: 可维护性/关键路径测试缺口（**当前实现是对的，但这条不变量在任何层级都没有测试**）：
- 专门的回归文件 `test/stage_pointer_interceptor_test.dart`（172 行、8 个用例）里
  `grep -n "enabled"` → **零命中**：它只验「有没有垫层」，从不验「该关时有没有关」；
- 另外 4 个引用 `StagePointerInterceptor` 的测试（`compact_settings_page_test.dart` /
  `confirm_discard_test.dart` / `session_sheet_test.dart` / `app_shell_background_test.dart`）
  也没有针对 `enabled` 的断言；
- 行为级测试**在本仓不可能有**（`flutter test` 跑 VM，垫层走透明 stub——该文件头注 :17-19
  自己写明了「这里验的是**接线**，不是 DOM 行为」），但**结构级断言是可以写的**：
  以 `wsStatus: connected` 起壳，断言横幅那层的 `StagePointerInterceptor.enabled == false`。
  这样将来有人再加一个「折叠不卸载」的常驻浮层而忘了传 `enabled`，测试会红。
**为什么这条值得记**：红线 M 的失败模式是 AGENTS.md 明写的「**漏套不报错，只是看得见、
点不着、也滑不动**」——而 `enabled` 漏传的后果更隐蔽：界面完全正常，只是**舞台拖不动模型**，
且只在「折叠后仍占面积」时才发作。102 个 Dart 测试文件里对这个最难的变体零覆盖。
建议: 在 `stage_pointer_interceptor_test.dart` 补两条结构级断言：① 断线横幅在
`wsStatus. connected` 下 `enabled == false`（且 `CollapsiblePanel.expanded == false`）；
② compact 设置在 `settingsOpen == false` 时 `enabled == false`。两条都不需要 DOM，
VM 上可跑
验证: 读 stage_pointer_interceptor.dart:41-99 全文件；读 test/stage_pointer_interceptor_test.dart
全文件（172 行）；`grep -rn "StagePointerInterceptor" shell/flutter/lib/` → 9 处使用点
（app_shell 4 / nav_host 1 / session_sheet 1 / stage_host 2 / confirm_discard_dialog 1），
逐一读了两处常驻点（app_shell:718-731 / :839-848）与三处条件挂载点（:528-537 / :749-757）；
`grep -rln "StagePointerInterceptor" shell/flutter/test/` → 5 个文件；`grep -n "enabled"
stage_pointer_interceptor_test.dart` → 无输出
置信: 高（实现正确性与测试缺口都已逐处读出）
反证: (a) 试过论证「`enabled` 由别的测试间接覆盖」——`grep -rn "enabled"` 在该文件零命中，
另 4 个文件是各自测别的（compact 页 / 确认弹窗 / 会话面板 / 背景），均无 `enabled` 断言；
(b) 试过论证「VM 上测不了所以不算缺口」——**行为**确实测不了，但**结构**（构造时传没传
`enabled: X`）完全可以断言，而这是唯一能在 CI 里拦住回归的形式；(c) 试过论证「9 处使用点里
可能有更多常驻点未接 `enabled`」——已逐一读过：其余 5 处分别是 modal 浮层（挂载即显示）、
`if (stageCorner != null)` 条件挂载、nav_host / session_sheet / confirm_discard 的模态内容，
**均无条件挂载即消失**，不构成该不变量要求的场景。

---

## BATCH-0023 —— 红线验证台账（前端侧）

| 红线 | 判定 | 证据 |
|---|---|---|
| **M（平台视图指针）** | **✔ 接线正确 + 有真回归** | `test/stage_pointer_interceptor_test.dart` 的 `expectIntercepted` 用 `find.ancestor(of: inner, matching: find.byType(StagePointerInterceptor))` + `findsWidgets`——验的是**祖先关系**而非计数，故拆掉任一处的垫层**会红**；8 个用例覆盖断线横幅 / 舞台角标 / loading / error（并 `tester.tap` 断言回调真的触发）/ expanded / medium / compact 三种宿主。外加一条 VM 透明性用例。**唯一缺口**是 `enabled` 变体 → F-0023-01 |
| **O（30Hz 口型不进 Widget 树）** | **✔ 架构上成立** | `audio_player.dart:95` `static const Duration _levelTick = Duration(milliseconds: 30)`（按 `currentTime` 在句内包络取值，不是逐片事件）；`live2d_bridge.dart:90-91` `mouthMinInterval = Duration(milliseconds: 33)`（消费端二次节流）；`stage_host.dart:6` 明写「走 `GlobalKey → Bridge → postMessage`，**不进 Widget 树**」；`live2d_stage.dart:179` 对外只暴露 `GlobalKey<Live2DStageState>` 的 `setMouth`/`sync`。（未逐行读三处实现，只核了常量 + 链路面 + 公开接口） |

---

## BATCH-0024（2026-09-28）· 舞台保活（红线 N）+ 断点布局测试面

### F-0024-01 · P2
file: shell/flutter/lib/app/app_shell.dart:702-711（不变量声明与实现）
摘录（不变量**就写在实现旁边**）:
```dart
// ── 关键：三种断点共用同一个 Flex，只换 direction 与子项 flex。
//    子树形状不变 ⇒ 舞台 Element 不被反激活 ⇒ iframe 不重建。 ──
final Widget body = Flex(
  direction: compact ? Axis.vertical : Axis.horizontal,
  children: <Widget>[
    Flexible(
      flex: compact ? 3 : 1,
      child: Stack(children: <Widget>[
        Positioned.fill(child: stage),      // stage 带 key: stageKey（GlobalKey）
```
影响: 关键路径测试缺口（**实现正确，但不变量本身没有任何测试**）。核实过程：
- `test/app_shell_layout_test.dart` 有约 20 个用例，主题是「断点 → 导航形态」「五档宽度渲染出正确
  的导航形态」「设置宿主分流」——**没有一个**断言舞台 Element 在断点切换后**没被重建**；
- 全仓 `grep -rn "stageKey|currentState|重建|保活" shell/flutter/test/` 里，与「重建」相关的两条
  都是**别的**性质：`compact_settings_page_test.dart:184` 测的是**设置页**关掉再打开的保活
  （断言滚动位置，:234「再打开时回到顶部 —— 设置页被重建了」），
  `stage_bg_resend_dedupe_test.dart:105` 测的是**万一 iframe 真被重建**时的自愈
  （「资产契约 A6：**重建 iframe**（新桥）必须重发同一张图」）；
- 而**让上面两条成立的那个前提**——「三断点共用同一 Flex ⇒ 舞台 Element 不被反激活」——**无测试**。
建议: 照抄同仓已有的手法即可（`settings_panel_keepalive_test.dart:37-55` 的 `_CountingPane` +
`initState` 计数）���在 `app_shell_layout_test.dart` 加一条：以 500px 起壳记下
`find.byKey(stageKey)` 的 State 实例（或用一个会 `initState` 计数的假舞台替身），改成 1400px、
再改回 500px，断言计数**始终为 1**。约 15 行，VM 上可跑，不需要 iframe
为什么值得记: 红线 N 的失败模式是**静默且昂贵**的——一旦有人为了「可读性更好」把
compact/medium 拆成两个独立分支（这是布局代码最常见的重构方向），舞台会被重新挂载，
后果是**模型重载 + 口型归零**（AGENTS.md 对红线 N 的原话）。而这条不变量恰好是
**最难被肉眼发现**的那类：布局看起来完全正常，只是模型悄悄重载了一次
置信: 高
反证: (a) 试过找间接覆盖——`stage_bg_resend_dedupe_test.dart` 证明「重建了也能自愈」，
但**自愈不等于没重建**（模型仍然重载过、口型仍然归零过），故它不能替代本条；
(b) 试过论证「GlobalKey 已经保住了」——`stageKey` 是 GlobalKey，能在**同一父链**下保住 State，
但若断点分支的**祖先链**变了（Compact 分支 vs Medium 分支各自是独立子树），GlobalKey 也救不了；
它防的是「同位置换 key」不是「换位置」。(c) 试过找 issue/文档里的登记——本仓已有
`stage_bg_resend_dedupe_test.dart` 与 `settings_panel_keepalive_test.dart` 两块相邻拼图，
唯独缺这一块；**未找到**任何测试或注释声明它已被别处覆盖。

---

## BATCH-0024 —— 模式 H（新）：同一族红线的「易形态被钉住、难形态没被钉住」
本批与 BATCH-0023 合起来看，出现一个值得单独立档的模式：

| 红线 | 易形态（有真回归） | 难形态（无测试） |
|---|---|---|
| **M** 平台视图指针 | 「有没有垫层」——8 个用例，`find.ancestor` 验祖先关系，**能失败** | 「**该关时有没有关**」（`enabled`）——F-0023-01 |
| **N** 舞台保活 | 设置页关掉再打开的保活（`initState` 计数）；iframe 重建后的自愈（bg 补发） | 「**跨断点舞台 Element 没被重建**」——F-0024-01 |

**判据**：一个红线的**存在性**很容易被测（"这个控件在不在正确的容器里"），
而它的**条件性/时序性**很难被测（"该让位时有没有让位"、"该保活时有没有保活"）。
本仓在「存在性」上做到了 8 个用例 + 祖先断言的强度，但在「条件性」上两条都留了空。
**后果相似**：都是静默失效——M 是「舞台拖不动模型」，N 是「模型悄悄重载 + 口型归零」，
都不会报错、不会崩、只在特定交互下才发作。
⇒ **给后续前端批次的执行建议**：审 Dart 测试时，不要只问「这条红线有测试吗」，
要问「这条红线的**难形态**有测试吗」；两者常常一个有一个没有。

---

## BATCH-0025（2026-09-28）· Dart 假绿灯首轮 + 红线 K/L

### F-0025-01 · P2 【对 §9 已登记破口的「补证 + 升级」，非全新发现】
file: shell/flutter/test/font_subset_test.dart:300-309（门禁的扫描面）
摘录（门禁**只**扫源码里的字符串字面量）:
```dart
for (final File file in sources) {                       // sources = lib/**/*.dart
  for (final String literal in stringLiterals(file.readAsStringSync())) {
    for (final int cp in literal.runes) {
      if (cp < 0x80) continue;                          // :304 ASCII 必然覆盖
      if (subsets.every((SubsetRanges r) => r.covers(cp))) continue;
      offenders.putIfAbsent(cp, () => <String>{}).add(file.path);
```
门禁自述的后果（:322-327）：`这些字符不在自托管字体子集里。CanvasKit 会去 fonts.gstatic.com\n拉回退字体 —— **有网时看不出来，断网就是豆腐块**。`
影响: 离线优先（红线 K/L）。**§9 已登记「运行时文本不在源码扫描门禁内」**；本条不重复报告那个
登记本身，只补三条**新证据**并据此建议升级：
1. **前端代码明确预期聊天文本里会出现 emoji**：`chat_session.dart:410-414` 的注释写着
   「按**字符**（rune）截而不是按 UTF-16 code unit：**中文与 emoji 都在**…」，
   随后用 `text.runes` / `String.fromCharCodes` 实现——**emoji 是被当作常规输入处理的**；
2. **全仓无任何运行态字符过滤**：`grep -rn "subset|codepoint|rune|toRange|replaceAll(RegExp"`
   在 `lib/chat/` 与 `lib/data/` 下只命中 `pruneEmpty`（会话清理）与 rune 截断，
   **没有任何一处把模型输出里的字符按子集过滤或替换**；
3. 因此暴露面不是「理论上的门禁盲区」，而是「**LLM 常规输出（emoji）→ 断网时豆腐块 + 一次
   fonts.gstatic.com 请求**」，且**有网时完全不可见**——与门禁自己写的后果描述一致。
建议: ① 在**渲染前**对运行态文本过一道子集过滤（把越界码点替换为 `□` 或直接剔除）——
   这一处同时补上「模型输出」这条最大来源；② 把「运行时文本」这条已知破口写进
   `docs/architecture/` 的显式登记，附上「已评估过、暂不修」的理由（任务书 §9 要求
   「已裁决不做」的条目要留痕）。
验证: 读 font_subset_test.dart:1-90 / 150-240 / 300-341；`grep -rn "subset|codepoint|rune|
toRange|replaceAll(RegExp" shell/flutter/lib/chat/ shell/flutter/lib/data/`；
读 `chat_session.dart:405-415`
置信: 高（门禁扫描面、无运行态过滤、代码预期 emoji 三点都可复跑）/ **中**（子集是否含 emoji
**未核**：`*.ranges.txt` 在任务的排除清单内「不审」，故本条按「若子集不含 emoji」表述；
若子集恰好含 emoji，则暴露面缩小到其它越界码点，结论的方向不变——**缺口本身与 emoji 无关**）
反证: (a) 试过论证「门禁其实也扫运行态」——`sources` 来自 `Directory('lib')`（:138），
扫的是**编译期**文件，运行时文本结构上不可能进入；(b) 试过找别处的兜底（TextStyle 的
fontFamilyFallback）——AGENTS.md 明写「`fontFamilyFallback` 也不会命中系统字体，
缺字时引擎只会去 fonts.gstatic.com 下载」，所以没有兜底；(c) 试过论证「这是已登记项不该记」
——按任务书 §9「发现比登记更严重时可标升级」，本条**明确标注为补证+升级**而非新发现。

---

## BATCH-0025 —— 模式 B 在 Dart 侧的首轮复查结论：**未命中，且门禁质量高于 Rust 侧**

审了两个「源码扫描式守卫」样本，它们恰好是任务书 §7-J 点名的假绿灯高危形态，结论都是**能失败**：

### 样本一：`test/design_tokens_lint_test.dart`（206 行，5 条规则）
**它自己写着怎么防假通过**（:145-156）：
```dart
test('扫描确实覆盖到了源码（防 Directory 路径写错导致「零命中=通过」）', () {
  // 这是最危险的假通过：路径写错 → 一个文件都没扫 → 全部规则空转通过。
  expect(sources, isNotEmpty, reason: 'lib/ 下没扫到任何 .dart');
  expect(sources.map((File f) => f.path).where((String p) => p.endsWith('main.dart')), isNotEmpty,
      reason: '连 main.dart 都没扫到，说明扫描根路径不对');
  expect(sources.length, greaterThanOrEqualTo(10));
});
```
**豁免面本身被审计**（:182-204）：断言豁免恰好 2 个文件且**文件真实存在**；
断言本文件**不重复实现**别处的规则（:190-195「同一条红线的两处实现迟早会漂移」）；
断言协议时长豁免**没有**过度覆盖（拿 `lib/ui/state_pill.dart` 当反例，并记录前一个反例
文件已随 P0-1 删除）。豁免表的历史也被记账：:35-37 写明原先还有 `lib/actions/action_dispatch.dart`
那条，随手动触发入口移出成品而**整条删除**——「豁免表只能变小」。

### 样本二：`test/font_subset_test.dart`（341 行）
- **数据层防伪**：文件头记录字体字节数 + FNV-1a(32)，测试**重新计算比对**
  （:41-52 实现与 `scripts/font_subset_ranges.py` 逐字节等价）——
  头注写明理由：「若有人换了字体却没重新生成覆盖表，测试就会拿旧表放行——**比没有门禁更危险**」；
- **词法器自测**（group ①，:224-240+）：注释里的字符不算、三引号/原始字符串/相邻字面量都能抽到
  ——**门禁的抽取器本身有钉子**；
- **校准位断言**（:331-339）：断言两个历史违规字符 `▍`/`⌘` **仍然不在子集里**，
  「若哪天它们进了子集，本断言会提醒我们可以简化实现」。

⇒ 结论：**模式 B 的四种写法在 Dart 的两个门禁样本里一个都没有**。结合 Rust 侧 4 处
（`model_root.rs` / `supervisor_slot.rs` / `mod_registry.rs` / `tests_mod.rs`），
模式 B 至今**共 4 处、跨 2 种语言、全部集中在「配置/装配面」而非「门禁面」**。
这进一步收窄了 CONSOLIDATION-02 的结论：**本仓的门禁文化是好的；假绿灯出现在业务逻辑的
「自己写的单元测试」里，而不是「守门禁的门禁」里。**

---

## BATCH-0026（2026-09-28）· 红线 K 正题 + 门禁样本三/四 + **跨审计交叉引用**

### 红线 K（离线优先）—— **✔ 验证通过（源码级）**
全量扫描 `shell/flutter/lib/` 的 `https?://` 共 **12 处**，逐条归类后**无一是运行时依赖的外部源**：
- `http://127.0.0.1`（本地服务）
- 4 条 MDN 文档链接 + 1 条 GitHub 归属 + 1 条 pub.dev 引用 —— **全在注释里**，不是被加载的资源
- `fonts.gstatic.com/` —— **只出现在注释中**，且每一条注释都在说明「**为什么不能依赖它**」：
  `app_shortcuts.dart:40`「自托管的中文子集里，一旦上屏就会让 CanvasKit 去 `fonts.gstatic.com` 拉」、
  `typography.dart:98`「CanvasKit 找不到中文字形、又回去 `fonts.gstatic.com` 下载」、
  `theme.dart:28`「遇到未打包的字形，引擎只会去 `https://fonts.gstatic.com/` 下载 Noto」、
  `message_bubble.dart:232`「实测到该请求，HTTP 200」
- `shell/flutter/pubspec.yaml:33` 的注释：「自托管中文子集，**替代** Flutter 的 `fonts.gstatic.com`
  缺字回落」
- 专查 `gstatic|jsdelivr|unpkg|cdnjs|cdn\.` → 命中全部在上述注释与 pubspec 注释里
- `shell/flutter/web/index.html` → **零外链**
⇒ **源码里没有任何硬编码外链**；gstatic 只以「被规避的对象」的形式出现在注释中。这是本仓
红线纪律执行得最漂亮的一处：**连提及都只为了说明它不能被依赖**。

### 门禁样本三、四：同样自带反假通过钉子（模式 B 在 Dart 仍未命中）
- `test/no_backdrop_filter_test.dart:41-49` 逐字复刻了同一条钉子：
  `test('扫描确实覆盖到了源码（防路径写错导致「零命中=通过」）')` + `expect(sources, isNotEmpty)`
  + `expect(sources.length, greaterThanOrEqualTo(30))` + 「连 main.dart 都没扫到…」——
  **这是房级范式**，不是某个文件的偶然。
- `test/visual_language_test.dart:349` 用了更强的形式：
  `expect(findBareBold(bad), isNotEmpty, reason: '扫描器连合成违规都抓不到')`——
  喂一个**合成违规**给扫描器，断言它抓得到。即**扫描器自身的判别力**被测试了。

### ★ 跨审计交叉引用（本批最有价值的产出）
本仓**自己跑过一轮审计**，并已在 rc.7 修复了 4 条假绿灯：
- 登记处：`docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md:106`
  「| B 假绿灯 | F-0005-1 / F-0005-3 / F-0005-6 / F-0005-7 | P1/P2 | 4 条『删掉实现测试仍绿』；
  **按测试修复处理，不按业务 bug**；给所有源码扫描式守卫加『**判别力自证**』 |」
- 落地提交：`d0e22f12 2026-09-28 22:32:20 +0800 release: v0.2.0-rc.7 —— 正确性与诚实性
  （审计主发现 F-0005-2 + 4 条假绿灯 + 仓库卫生）`
- **它改动的测试文件全部是 Dart**：`chat_bubble_labels_test.dart` / `chat_notice_test.dart` /
  `no_backdrop_filter_test.dart` / `semantics_test.dart`（+ 新增 `rebuild_scope_test.dart` 326 行）
⇒ **两件事同时成立**：
1. **模式 B 是本仓已知的活跃问题**，方法论已确立（判别力自证 = 删掉被测实现看测试是否变红），
   与我在 BATCH-0001/0002 用的是同一条纪律；
2. **那一轮清扫只覆盖了前端，`crates/` 未被扫到**。我的两条 Rust 侧假绿灯
   （F-0001-02 `model_root.rs` / F-0002-02 `supervisor_slot.rs`）因此不是「团队漏掉的同一处」，
   而是**同一类问题落在未被清扫的树里**——这既解释了它们为何存活，也意味着
   **修法在本仓已有现成范式**（照 F-0005 的「判别力自证」做一次红绿双向演示即可）。
**这不是一条新缺陷，而是对两条既有 P1 的定级背景补充**；记在此处以便 Phase 4 对抗日引用。
顺带记一个模式 B 的**第五种写法**（本仓自己写下并已修复的）：
`expect(source.contains('RepaintBoundary'))` —— 被测文件里唯一的命中是**一句说「不要直接铺
RepaintBoundary」的注释** ⇒ 断言恒真，删掉真正的保护也照样绿
（记录于 `no_backdrop_filter_test.dart:248-256`，该处已改成锚定 `build` 的 return +
先剥注释与字符串）。该注释顺带提到「`test/semantics_test.dart` 还抄了一份一模一样的空转断言」——
**已核实 `semantics_test.dart` 现已无 `RepaintBoundary` 字样**，故重复副本已清理，
但**那句话现在已过期**（模式 D 第 9 例）。

---

## BATCH-0027（2026-09-28）· **跨审计去重**（对照 `ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md`）

> 本节**不新增缺陷**，只做去重与交叉验证。核对方式：把该文档登记表里的 **45 个 F 编号**
> （F-0001-1 … F-0021-1）与我账本里的 **50 条**逐条比对，并抽查三条关键项的**当前代码**。

### ⚠ 编号命名空间独立，勿混淆
他们的编号是 **`F-0001-1`（一段）**，我的是 **`F-0001-01`（两段）**。两套编号**偶然相似但语义无关**：
他们的 `F-0017-1` 与我的 `F-0017-01` 是完全不同的两条发现。
（本节以下的「重合」判定一律按**语义**而非编号。）

### 结论一：重叠只有一条，且它是**跨边界**发现
| 他们 | 我 | 判定 |
|---|---|---|
| `F-0017-1`「后端 `performance` 段前端零解析，P2」 | **F-0021-01** | **同一条，标为重复**。他们从**前端**侧发现（搜不到 `performance` 键），我从**后端**侧发现（`SettingsView.performance` 无人消费）——**同一事实的两个方向**，互为独立佐证。这是模式 G 最强的一次验证：跨端契约问题**两侧单看都自洽**，只有合起来才看得见 |
| `F-0021-1`「行内码无 `fontFamilyFallback`，红线 L 破口」 | `F-0025-01` | **相邻但不同**。他们的破口是**行内代码**（`clean.rs` 剥反引号后仍可能落到等宽字体）；我的是**运行态文本**（模型输出）。同属红线 L，但注入面不同，两条都成立 |
| 其余 43 条 | — | 落点文件**全部是 Dart**（`main.dart` / `app_shell.dart` / `shell_slideshow.dart` / `background_hydration.dart` / `appearance_section.dart` / `shell_prefs.dart` / `shell_backdrop.dart` / `memory_panel` / `director_panel` / `llm_section` …），**无一落在 `crates/`** |

### 结论二：他们的审计**范围是纯前端**，这解释了重叠为何只有一条
原文自述（:6 / :76 / :182）：
- 「审查证据：`AUDIT-B/`（**21 批**，为准）+ `AUDIT/`（4 批，短跑快照）」
- 「**P0 = 0**（审计 7 批 **58 源文件 + 34 测试文件**，未见数据丢失/权限绕过/离线红线被破/注入面）」
- 门禁清单注脚：「**只有碰过 Rust 才跑（本路线预期不碰 `crates/**`）**」
⇒ 45 条落点 100% 在 Dart。**`crates/` 从未被任何一轮审计覆盖**——与我 CONSOLIDATION-03
的判断（「前端 0/217 是最大空白」互为镜像：**Rust 侧同样从未被系统审过**，只是恰好由我这一轮补上）。
**我的 50 条里，除 F-0021-01 外全部落在 `crates/`**，因此不是「重复劳动」，而是**互补的两半**。

### 结论三：独立交叉验证 —— **两个互不知情的审计都得出 P0 = 0**
他们的范围：58 源文件 + 34 测试文件（前端）→ P0 0。
我的范围：~70 个 Rust 文件、web_api 49/49 结项 → P0 0。
两轮独立、树不同、方法不同，**结论一致**。这条比任何单条发现都更能说明该仓库的真实风险分布。

### 结论四：抽查三条已登记项在 **HEAD 的当前状态**（rc.7 已发布于本树 HEAD 前 13 分钟）
| 他们的 ID | 等级 | HEAD 现状（我核实于当前代码） |
|---|---|---|
| `F-0005-2`「每个 `text_delta` 重建整棵 AppShell 子树」 | **P1（主发现 / 放大器）** | **已修复**。`app_shell.dart:277-286` 新增 `final int settingsRevision` 参数，文档写明「（F-0005-2，审计 45 条 · rc.7 A 组）…现在分区内容按代际放行（`AppShellState._pane`），所以这个值**绝不能**跟着聊天增量前进」——机制在位 |
| `F-0012-1`「自检成败扫 `contains('ok'/'ms')`」 | **P1** | **仍存在，未修复**。`llm_section.dart:179-183`：`resultIsError: testResult != null && !testResult!.toLowerCase().contains('ok') && !testResult!.contains('毫秒') && !testResult!.contains('ms')`；`tts_section.dart:177-179` 同款。**`testResult` 是展示用 `String?`**，而服务端 `TestOutcome` 明明序列化了可靠的 `ok: bool`（BATCH-0011 已验证 `test_outcome_serializes_with_ok_tag` 断言 `j["ok"] == true`）——**可靠的布尔在手却被丢掉，改用扫展示文案判断成败**。这直接违反红线「前端不得从显示文案猜错误类型（文案会改，码是契约）」。可达后果：失败文案里含 'ok'/'ms'/'毫秒' → **假成功**；成功文案不含这三者 → **假失败**。而该条被排进 rc.7 的 R7-c1 波次（:152），rc.7 收口判据写的是「§4 的 P1/P2/P3 **逐条关闭或写明不做 + 理由**」——**HEAD 上既没关闭也没见「不做」的理由** |
| `F-0001-2` / `F-0013-1` 等 rc.6 背景域 | P2/P1 | **未抽查**（落点在 `main.dart` / `background_hydration.dart` / `shell_slideshow.dart`，属前端背景域，留 BATCH-后续） |

### 对我自己账本的影响（诚实记录）
1. **F-0021-01 降为「与团队账本重复」**——不是我发现的，是他们先发现的（`F-0017-1`）。
   按任务书 §9「不确定是否已登记 → 搜过再决定」，本条**已在 BATCH-0021 独立报出**，
   现补记重复关系并保留（因为他们的登记里**没有**「两处可观察面」中的 `AppStatus.performance`
   那一半，且没有给出「零消费者」这一判据本身）。
2. **F-0025-01 与他们的 `F-0021-1` 相邻但独立**，两条都保留，但应合并成一条「红线 L 的两处破口」的
   统一条目（留 CONSOLIDATION-04 处理）。
3. **他们的 45 条对我下一步的排期有直接影响**：前端背景域 / 轮播 / 记忆面板 / 自检渲染
   这些**已登记且部分已修**的区域，我应**降级优先级**，把预算让给 `crates/` 与未审前端。

---

## BATCH-0028（2026-09-28）· rc.6 背景域 —— 核验团队已关闭项的**残留**

### F-0028-01 · P2  【团队 F-0006-1 / F-0016-1 已于 rc.6 关闭，本条是它的残留】
file: shell/flutter/lib/data/background_decode_cache.dart:39-45（容量口径）
摘录:
```dart
/// 缺省容量：**3 条**。
/// 为什么要上限而不是无限 Map：用户可能导入过几十张图，全留着就是几十 MB
/// 常驻内存（这正是「性能修复」变成「内存泄漏」的经典形态）。
/// 3 条覆盖壳背景 + 背景库里最近点开的两张，实测使用足够。
const int kBackgroundDecodeCacheCapacity = 3;
```
而**两条路径共用这一个缓存**：
- 壳根壁纸：`shell_backdrop.dart:257` `final Uint8List? bytes = decodeDataUrlBytes(dataUrl);`（**每次 `AppShell.build` 都调**）
- 背景库缩略图：`appearance_background.dart:1472` `final Uint8List? bytes = decodeDataUrlBytes(dataUrl);`
  （由 :930 的 `ReorderableListView.builder` 构建，可见项 + `cacheExtent` ⇒ **约 8–11 个**）
调用链（失效场景）:
1. 设置面板打开（`AppShell` 的 `PageCrossFade`/内联侧板常驻）⇒ 一次 build 内先构建壳壁纸（插入 A），
   再构建约 8–11 个缩略图 ⇒ **A 被逐出 3 槽 LRU**；
2. 下一个流式 delta → `AppShell.build`（`ChatController.notifyListeners` 驱动，
   这是他们 F-0005-2 之后**仍然**每次跑的路径——F-0005-2 收窄的是**设置面板**的重建，
   壳根照旧每 delta 重建）→ `ShellBackdrop._imageLayer` 调 `decodeDataUrlBytes(A)` → **miss**
   → 整张壁纸重新 base64Decode（协议预算 ≤1.5 M 字符）+ 重解码 + 重上传纹理；
3. 如此**每个 delta 一次**，只要设置面板开着且可见缩略图 ≥ 4 个。
影响: 性能/用户可见（这正是他们 F-0006-1 描述的病态——「设了壁纸的用户，回复期间的每一帧
都重解一张图」——在「设置面板打开」这个配置下**原样复现**）。容量注释的推理前提
「背景库里最近点开的两张」与实现不符：`ReorderableListView.builder` 会构建**全部可见项 + cacheExtent**，
不是 2 个。
建议: ① 两条路径**分缓存**——壳壁纸用 1 条常驻（它就是当前选中项，永远需要命中），
   缩略图另给一个有界 LRU；② 或把容量提到 ≥ 可见缩略图数 + 1，并在注释里写明这个算式；
   ③ 根本解法：`_imageLayer` 把 bytes 存进 `State`（按 dataUrl 记忆），而不是每次 build 去问缓存
——无论哪条，**容量注释必须按真实工作集重写**，否则下一个人还会按「3 条足够」来调
验证: 读 background_decode_cache.dart:1-80、shell_backdrop.dart:250-262、appearance_background.dart:1460-1478
与 :930；`grep -rn "decodeDataUrlBytes|DataUrlBytesCache" shell/flutter/lib/` → **两个调用点共用同一函数**
置信: 高（容量、两个调用点、列表构建方式、build 触发源全部直接读出）
反证: (a) 试过论证「设置面板打开时缩略图不重建，所以 A 不会被逐出」——A 被逐出发生在
**第一次**构建面板时（可见项一次就 ≥4 个），之后每次 delta 的 A 查询都是 miss，与面板是否重建无关；
(b) 试过论证「F-0005-2 之后设置面板不随 delta 重建，所以整体影响很小」——**成立且已计入**：
面板本身不再每 delta 重建，但**壳壁纸**仍每 delta 重建，而它要的是 A 那一条命中，被面板挤掉的后果不变。
故定 P2 而非 P1。(c) 试过找他们的回归测试是否覆盖这个交互——`F-0006-1` 的回归在 rc.6 已加
（他们的收口判据要求「两条 P1 必须有可失败的回归」），但那是**单路径**判据（同一个串两次调用拿到同一实例），
**不覆盖「壳 + 面板同时存在时 LRU 被挤」**这一交互。

---

## BATCH-0028 —— 团队已登记项的 HEAD 现状核验（本批抽查 4 条）

| 他们的 ID | 等级 | HEAD 现状（我核实于当前代码） |
|---|---|---|
| `F-0006-1` 壳背景每 delta 重解码 | **P1** | **机制已修**：`background_decode_cache.dart` 的 LRU 保证「同一 dataUrl → 同一 `Uint8List` 实例」（`decode` 命中路径直接 `return hit`），理由引用了 SDK 的 `MemoryImage.==` 比身份这一事实。**但有残留 → F-0028-01** |
| `F-0016-1` 缩略图每次 build 重解码 | P2 | **已修**：缩略图与壳壁纸共用同一缓存函数（`appearance_background.dart:1472` → `decodeDataUrlBytes` → 缓存）。同样受 F-0028-01 的容量问题影响 |
| `F-0003-1` 启动水合竞态 | P1 | **未核**（落点 `main.dart::_hydrateBackgrounds`，本批未读 `main.dart`） |
| `F-0002-1` 偏好一变全量重发 stage-bg | **P1** | **未核**（落点 `shell_prefs.dart` / `live2d_bridge.dart` / `audio_bar.dart`） |

⇒ 抽查命中率：4 条中 2 条已修、2 条未核；**已修的 2 条里有 1 条带残留**。

---

## BATCH-0029（2026-09-28）· `main.dart` 水合路径 —— 核验团队 rc.6 的两条 P1

> 本批**未发现新缺陷**。产出是**对 5 条已登记项的逐条核验**，其中 4 条确认已修且**修法质量高**，
> 1 条部分核实。按 §9「已登记不重复报为新发现」，此处只记核验结果与**修法本身的可借鉴点**。

### 核验一：`F-0013-1` / `F-0001-3`（**P1** 启动水合竞态）—— **已修，且修法是教科书级的**
三处改动共同闭合了「窗口期改动被回滚」：
1. **合并基座换了**（`main.dart:201-202`）：`setState(() { _prefs = applyBackgroundHydration(_prefs, hydrated); })`
   ——第二个参数是**此刻**的 `_prefs`，不是水合开始时刻的快照。头注明写旧病灶
   （:199-200）：「旧实现是 `_prefs = hydrated.prefs`——把水合**开始时刻的快照**整体盖回去，
   窗口期的改动全部回滚」；
2. **剪枝保留集读实时值**（`main.dart:220` `liveKeptIds: () => _imageIdsIn(_prefs)`）：
   水合剪枝是**真删字节**，若只按快照算，刚导入的那张会被当孤儿删掉（`background_hydration.dart:145-148`）；
3. **`source` / `prefs` 派生视图被显式封为「线上勿用」**（`background_hydration.dart:88-96`）：
   `/// ⚠️ 线上路径**不要**用它——窗口期用户的改动不在 [source] 里，拿它去覆盖就是 F-0013-1 的病根。`
   ⇒ **误用点在类型层面被堵死**，而不只是靠调用方自觉。
另有 `applyBackgroundHydration`（:113-141）只动背景域、其余字段「一律不碰」（:107）、
补字节必须走 `copyWith`（:134-136「直接 `new BackgroundImage(id: …)` 会把用户给这张图设的
opacity / fit / align 一起清空」）、剪枝前先 `store.probe()` 确认存储健康
（:157 + :162「空清单可能只是『这一次读不到』，不是『用户清空了库』」）。

### 核验二：`F-0002-2`（**P2** 早期加图写进占位内存库）—— **结构性修好**
`main.dart:154` `final BackgroundStore _store = createBackgroundStore();` —— **final 单实例、
同步构造**（web 那份的 `open` 是惰性的），水合**不再换库**。头注（:147-153）写明旧病灶：
「先 MemoryBackgroundStore 占位 → 水合完成才换真库，于是窗口里用户导入的字节进的是
**即将被丢弃的内存库**，界面却说『已加进背景库』——下次启动那张图被判孤儿摘除。」
另有 `_backgroundHydrating`（:175）对界面暴露读回窗口（:169-174「读回窗口里的写操作与读回结果
**会打架**，而界面当时一句话都不说」）——用**禁用写操作**正面解决，而不是靠运气。

### 核验三：`F-0002-3`（**P3** `_openStore` 假兜底）—— **已修**
`main.dart:162-166` 头注点明旧实现「那层 timeout **永远不触发**（构造是同步的、也不会抛），
注释承诺的退路在线上**不存在**」，现在只保留「只管这次读」的 3 s 上限（`_hydrateTimeout`，:167），
真实兜底下沉到 `_IdbBackgroundStore` 内部（open 失败被记下、之后每个操作**诚实地失败**）。

### 核验四：`F-0002-1`（**P1** 偏好一变就全量落盘；滑杆每像素一次）—— **「每像素」半边已修**
`ui/audio_bar.dart:100` `void _onDrag(double value) => setState(() => _dragVolume = value);`
（拖动中只改本地态）vs `:106-109` `void _commit(double value) { …; widget.onVolumeChanged(value); }`
（**只在 `onChangeEnd` 提交**）。且头注（:102-105）逐条枚举了 `Slider` 的三条输入路径
（`_handleDragEnd` / `_endInteraction` / 键盘 `increaseAction`·`decreaseAction` 里的 `onChangeEnd`）
——**键盘调音量不会被漏掉**，这是「修了但漏了一条路」的经典坑，此处没踩。
**「全量落盘 / 全量重发 stage-bg」半边未核完**：`main.dart:237-241` 的 `_update` 仍是
`setState(...); return saveDisplayPrefs(next);`（有「值没变直接返回」的早退，:238），
真正的去重/去抖若存在应在 `shell_prefs.dart` / `live2d_bridge.dart` 一侧——**留 BATCH 后续核实**。

### 本批结论
**这批的产出是「修法质量」而不是「缺陷」。** 与 BATCH-0028 的解码缓存形成对照：
同一批 rc.6 修复里，`main.dart`/水合这一组是**结构性修好**（换基座、封误用点、按键分流），
而 `background_decode_cache.dart` 那一处是**机制正确但容量口径没按真实工作集重算**（F-0028-01）。
**判据由此更清楚**：一个修复是否真的闭合，看它**有没有把「误用点」本身堵死**——
水合那组把 `source` 标成「线上勿用」，解码缓存那组却留了「3 条足够」的注释而实现与它矛盾。

---

## BATCH-0030（2026-09-28）· `main.dart` 轮播 + `shell_prefs` 下发漏斗 —— 核验 F-0001-2 / F-0002-1

### F-0030-01 · P2 【**已于 B0069 撤回** —— 原判「三条通道里漏了第三条」不成立】
> ⚠ **本条已撤回，不计入有效发现**。B0069 回源码核出 `action_scales_sync.dart:53/136`
> 有 **150ms 防抖**、`:186` 有以它命名的回归、且 B0090 复核后文档已与用途一致；
> 原文三条主张**全部不成立**。**原文保留**在下方 B0030 段内以便追溯，**勿据其行动**。
file: shell/flutter/lib/app/shell_prefs.dart:132（`force: true` 的调用点）
摘录（同一个函数里的**两条**去重都在，`_applyPrefs` 自己也写明了漏斗性质）:
```dart
// # 同值不重发（F-0002-1，2026-09-28）
// 这条漏斗**每次偏好变更都要过**（音量滑杆过去每帧一次），原来无条件重发一整个
// `sync` + 一整串 `stage-bg`（后者是 ≤1.5 M 字符的整张图）。
if (!identical(bridge, _lastSyncBridge) || !listEquals(syncKey, _lastSyncKey)) { … }   // :106-107  ✔ 去重
if (stage != null && _lastSentStageBg != prefs.stageImage) { … }                     // :122      ✔ 去重
// 首帧 / 重建 iframe 后补发幅度……
_syncActionScalesNow(force: true);                                                       // :132  ✘ 绕过去重
```
而 `force` 的**文档用途**只是重挂补发，不是「任意偏好变更都要发」：
```dart
/// `force` = 绕过「值没变就不发」——iframe 重建后旧帧随旧桥销毁，**必须重发**
/// （与 `stage-bg` / `stageColor` 同一条纪律）。
void syncNow(Map<String, double>? payload, {bool force = false}) {   // action_scales_sync.dart:150
  final String key = _key(effective);
  if (!force && key == _sent) return;                                // :161 —— force 直接跳过这道
  _sent = key; send(effective);                                      // :162-163
```
调用链（每像素仍发一帧）:
音量滑杆每像素 → `_updatePrefs`（shell_prefs.dart:145）→ `_applyPrefs`（:158）→
`_syncActionScalesNow(force: true)`（:132）→ `syncNow(force: true)` →
`!force && key == _sent` 短路**失效** → `send(effective)` →
`main.dart:437-438` `_stageKey.currentState?.applyActionScales(scales)` → postMessage 给 iframe 渲染面。
影响: 性能/可维护性（F-0002-1 的三个下发通道里，`sync`（7 字段指纹）与 `stage-bg`（≤1.5M 字符串）
**都真去重了**，唯独第三条 `actionScales` 被 `force: true` 绕过 ⇒ **「滑杆每像素一次」这条
症状仍在**，只是载荷从「整帧 sync + 整张图」缩到了「三个 double」。载荷小所以不是带宽悬崖，
但仍是**每像素一次跨桥 postMessage + 渲染面侧处理**，且发生在用户拖滑杆这种高频手势里。
**根因不是「忘了加去重」，而是 `force` 的适用范围被放大**——它的头注只为「iframe 重建」这一种
场景而写，却被用在了**每次偏好变更**上；同一函数上面两行刚做完去重，第三行立刻绕过。
建议: `_applyPrefs` 只在**桥身份变化**时传 `force: true`——而它上面第 106 行**已经算出了**
这个条件（`!identical(bridge, _lastSyncBridge)`）；把它记下来复用即可：

```dart
final bool bridgeChanged = !identical(bridge, _lastSyncBridge);
if (bridgeChanged || !listEquals(syncKey, _lastSyncKey)) { … _lastSyncBridge = bridge; … }
_syncActionScalesNow(force: bridgeChanged);   // 而不是无条件的 force: true
```
这样「重挂必须补发」成立（桥换了就 force），而「拖音量不改幅度」不再每像素发帧
验证: 读 shell_prefs.dart:70-154、action_scales_sync.dart:133-172、main.dart:436-439
置信: 高（三处调用点 + `force` 的短路条件 + 下游 postMessage 全部直接读出）
反证: (a) 试过论证「音量滑杆根本不过 `_applyPrefs`」——`main.dart:571-573` 说明接线片段统一改调
`_refresh()`/`_applyPrefs`，且 `_updatePrefs`（shell_prefs.dart:145-158）就是偏好变更的落点，
音量属于 `DisplayPrefs` 字段 ⇒ 必过；(b) 试过论证「每像素一帧三个 double 代价可忽略」——载荷确实小，
所以不升级为 P1；但**它让 F-0002-1 的一半症状留存**，且与同一函数上方两行的去重自相矛盾，
属于「修了一半」的形态，与 BATCH-0028 的解码缓存残留同型；(c) 试过找测试钉住「音量拖动不发帧」——
本批未在 `shell/flutter/test/` 里找到断言 `applyActionScales` 调用次数的测试（**未核实**，
留后续 Dart 测试批次）。

---

## BATCH-0030 —— 核验结果

| 他们的 ID | 等级 | HEAD 现状 |
|---|---|---|
| `F-0001-2` 轮播双索引漂移 | P2 | **已修（两形态都修）**：`_jumpBackground`（main.dart:532-540）同时推 `_slideshow.jumpTo` 与宿主索引（否则下一次 `onAdvance` 从旧 index 走、把刚预览的切走）；`syncSlideshow`（:514-525）在 `setLibrary` 后把 `_slideshow.index` 的夹持值回写宿主（旧实现只让渲染层取值时 clamp，库从 5 清成 1 后宿主索引还是 4） |
| `F-0002-1` 偏好一变全量重发/全量落盘；滑杆每像素 | **P1** | **主体已修**：`_applyPrefs` 里 `sync` 按「桥身份 + 7 字段指纹」去重（shell_prefs.dart:106-107，指纹覆盖 `sync` 实际下发的全部 7 个字段，无遗漏）、`stage-bg` 按值去重（:122）。`_updatePrefs`（:145-153）还接住了落盘失败并如实告知（「本次会话有效，刷新会丢」）。**残留** → F-0030-01（第三条通道 `actionScales` 被 `force: true` 绕过） |

---

## BATCH-0031（2026-09-28）· 核验 F-0005-2（团队 P1「每个 text_delta 重建整棵 AppShell 子树」）

> **0 条新发现。** 本批做的是对「放大器」那条 P1 的**对抗性核验**：它的修复方式
> （加一个代际计数器）天然带一个风险——**计数器漏加就会让面板变陈旧**。必须证明它既不过度失效、
> 也不过度**不**失效。这一批的结论是：它两头都对。

### 核验一：音频路径**不会**每帧重建外壳（过度失效已被挡住）
`_ShellRootState` 自己唯一的 WS 侧 `setState` 是：
```dart
// main.dart:724-731
_eventSubscription = _ws.events.listen((WsEvent event) {
  _ui.consume(event);                                     // 委托给 tracker，只动它自己的监听者
  if (event is AudioEvent && event.muted != wasMuted) {   // ← 关键：带「真的变了」的守卫
    setState(() => _serverMuted = event.muted);
  }
```
音频事件按 20 ms 一片（BATCH-0006 已核 `AUDIO_SLICE_MS = 20`）⇒ 约 **50 次/秒**；
若这里是无条件 `setState`，`_settingsRevision` 就会每 50 帧 +1，面板缓存全废。
**它带守卫，只在静音态真变时 setState** ⇒ 音频路径不重建外壳。
30 ms 的 `stageClock` 路径（:709-720）也**不**走 `setState`，直接 `_stageKey.currentState?.sendStageClock(...)`。

### 核验二：面板内容**不会**变陈旧（欠失效也被挡住）——这是本批最关键的发现
缓存键是**两个独立代际 + 分区**：
```dart
// app_shell.dart:591-597
/// 设置数据的代际：自己订阅、自己驱动重建（浮层那条路根本没有外壳重建）。
child: ValueListenableBuilder<int>(
  valueListenable: _settingsTick,
  builder: (BuildContext _, int tick, Widget? _) => _pane(context, section, tick),
),
// app_shell.dart:628
(int revision, int tick, SettingsSection section)? _paneKey;
```
- `revision` = 宿主侧「外壳看不见的状态」（模型库 / Mod / 诊断 / 自检结果 / 本模型覆盖）
- **`tick` = 设置数据代际，由面板自己订阅 `_settingsTick`**——这一步是关键：
  **medium 的底部浮层那条路「根本没有外壳重建」**（注释 :591-592 明说）。
  若只靠 `settingsRevision`，浮层宿主的面板就永远停在旧数据；靠 `tick` 自订阅才补上这条路。
⇒ **两个计数器分别覆盖两类变化，且其中一类绕开了外壳 build**。这不是「加个计数器」，是**按失效来源分流**。

### 核验三：连「InheritedWidget 作用域陷阱」都预判并钉住了
`_pane` 必须拿到 `BackgroundRuntimeScope` **之上**的 `context`（外观区靠一个 `Builder` 读 scope）。
头注显式警告（:638-640）：
`/// ⚠️ `context` 必须是 `BackgroundRuntimeScope` **之上**那一层（`SettingsScaffold.child` 的实参位置）：
/// 外观区靠一个 `Builder` 去读 scope，约定与回归见 `test/app_shell_background_test.dart`；别改成 tick 那一层的。`
⇒ 这是把 `_pane` 挪进 `ValueListenableBuilder` 时最容易被顺手改坏的一处（`ValueListenableBuilder`
的 builder 上下文**在**该 widget 之下），他们**预先写明了约束并留了回归**。

### 一处如实记录的观察（**不记为发现**）
`main.dart:1170` 的 `_settingsRevision++` 写在 `build()` **内部**，属于「build 里的状态突变」。
它的语义是「**一次外壳重建发生过**」这个**代理量**，而注释（:1167-1169）把它表述成
「宿主这一次重建 =『外壳看不见的那批状态』可能变了」。二者不等价：任何原因的
`_ShellRootState.setState`（幻灯片推进、偏好变更、分区切换、水合完成）都会 +1。
**方向是保守的**（只多失效、不少失效），且 `AppShell` 读的是构造时传入的值，单帧内自洽，
故**不构成缺陷**；但它是 Flutter 反模式，且注释把「代理」说成了「事实」——
与模式 D 同族，记此备忘而不入 FINDINGS。

### 核验结论
**F-0005-2（P1，放大器）已修，且修法质量高于本仓平均**：两个代际分流、浮层那条「无外壳重建」
的路用自订阅补上、InheritedWidget 陷阱预先警告 + 回归。**这是本次交叉核验里修复得最干净的一条。**

---

## BATCH-0032（2026-09-28）· 核验「约定与回归」+ **为团队 F-0012-1（P1）补上复合机制**

### 一、`app_shell.dart:638-640` 声称的那条回归**真实存在且能失败**（关闭 B0031 的未核实项）
`test/app_shell_background_test.dart:93-139` 的探针**刻意用嵌套 `Builder`**，
并在头注里把不变量写了出来：
```dart
/// 探针必须是 `Builder`：`sectionBuilder` 拿到的 context 在 scope **之上**，
/// 只有它**返回的** widget 才在 scope 之下（生产路径同理）。
sectionBuilder: (BuildContext context, SettingsSection s) => Builder(
  builder: (BuildContext inner) {
    final BackgroundRuntimeScope? scope = BackgroundRuntimeScope.maybeOf(inner);
    return Text('probe:${scope == null ? 'none' : '${scope.index}/${scope.hydrating}'}');
  },
),
```
断言（:176-189）驱动 `_pumpProbe(index: 2, hydrating: true)` 后要求屏幕上是 `probe:2/true`。
⇒ 若有人把 `BackgroundRuntimeScope` 挪进面板内部（或把面板挪到它之外），`maybeOf(inner)` 回 `null`
⇒ 屏幕变成 `probe:none` ⇒ **测试变红**。**模式 B 第 6 次复查：未命中。** 这条不是恒真断言，
它把「scope 必须在被缓存子树之上」这件事变成了可观察的文本。

### 二、读屏节流**真在用**（AGENTS.md 那条「`text_delta` 毫秒级不节流会淹没读屏」已被落实）
`main.dart:555` `final LiveRegionThrottle _live = LiveRegionThrottle();` →
`:1172 _syncLiveRegion()`（每次 build）→ `:1321 announcement: _live.hasAnnouncement ? … : null`；
`state/live_region.dart:34` `static const Duration kDefaultInterval = Duration(milliseconds: 1500);`
+ 可注入 `now`（便于测试）+ `_lastAnnounce` 状态 + `:1019 _live.dispose()`。✔

### 三、⭐ 为团队 F-0012-1（P1，仍未修）补上一个他们可能没连起来的**复合机制**
他们那条登记是「自检成败扫 `contains('ok'/'ms')`；**成功结果根本不渲染**」——两半。
本批把两半的**交互**读出来了，而它比两半各自的严重性都高：

```dart
// ui/field_row.dart:609-618（FieldActionRow.build）
/// 内联结果行（连通性自检的 `ok`/`latency_ms`/`error`）。
final String? result;
final bool resultIsError;
...
  _FieldShell(
    ...
    error: resultIsError ? result : null,      // :618 —— result 只在「判为错」时才出现
    child: … OutlinedButton.icon(…)            // :619-632 —— 按钮本身不含任何结果文本
  ),
```
`result` 在整个 `build` 里**只出现在这一处**。于是：
- `resultIsError == true` → 文案以 error 形态显示（用户看到红色文字）；
- `resultIsError == false` → `error: null` ⇒ **成功文案被整条丢弃**。
而 `resultIsError` 是由 `llm_section.dart:179-183` / `tts_section.dart:177-179` 的
`!contains('ok') && !contains('毫秒') && !contains('ms')` **子串扫描**算出来的。
**复合后果**：一次**假成功**（失败文案里恰好含 `ok`/`ms`/`毫秒`，例如上游回
「timeout after 3000 ms」）⇒ 扫描判成功 ⇒ **既不显示错误、也不显示任何结果**——
用户点「测试连接」看到按钮从「测试中…」变回「测试连接」，与「测试通过了」和「根本没跑」
**在界面上完全不可区分**。
⇒ 这解释了两件此前分开看会低估的事：
   (1) 为什么 AGENTS.md 把「自检说谎」看得比「没有自检」更重——这里正是那句话的字面实现；
   (2) 为什么他们那条是 **P1** 而不是 P2/P3。
**这不是新发现**（= 他们的 F-0012-1，已登记、已排进 rc.7 的 R7-c1），此处只补机制与精确落点。
**验证**: 读 field_row.dart:590-634、llm_section.dart:169-188、tts_section.dart（结构相同）；
`grep -n "result" ui/field_row.dart` → `result` 仅出现在 :597/:598/:610/:611/:618 五处，
其中**唯一的消费点**是 :618 的三元式。
**置信**: 高（`result` 的全部出现点已穷举）

---

## BATCH-0033（2026-09-28）· 前端持久化真源（`browser_io.dart` + `display_prefs.fromJson`）

> **0 条新发现。** 本批审的是**前端唯一的落盘真源**（localStorage 一条记录 + 逐字段反序列化），
> 连续三处「可能静默丢用户数据」的地方全部证伪。

### 核验一：`saveDisplayPrefs` / `saveChatSessions` 的 `bool` / `void` 不对称是**刻意的**
两个函数都在同一文件、同一个键族上，一个返回 `bool`、一个返回 `void`，容易读成疏漏。实际两侧都写了理由：
- `browser_io.dart:48-52`（返回 `bool`）：「以前返回 `void` 且静默吞掉，于是用户
  『选了背景图 → 界面显示已应用 → **刷新就没了**』而无任何解释（rc.3 §9.1 候选原因 1）。
  现在如实返回，由调用方给一句可执行的文案」——调用方确实接住了（`main.dart:151`
  「偏好没能写入本机存储（无痕模式 / 存储被禁 / 配额满）——本次会话有效，刷新会丢」）；
- `browser_io.dart:79-83`（`void` + 静默）：「**配额满不是假设**：localStorage 常见上限 5 MB，
  而 `ChatSessionStore` 用 `maxSessions` × `kMaxMessagesPerSession` 双重夹持把体积压在配额内。
  即便如此，写失败也只丢『这次之后的新记录』，不该影响正在进行的对话。」
⇒ 判据是**「丢了用户会不会心疼」**：设置是用户明确表达的意图（必须报告），
  会话是追加日志（丢了可接受）。两侧的取舍与后果描述都自洽。

### 核验二：反序列化**逐字段**设防，坏值只丢自己
`display_prefs.dart:570-632` 的 `fromJson` 对每个字段都走「类型化读取 + 钳位」，
且**非显然的默认值都记了理由**，例如：
- `:573`「旧存档没有这个字段 → 用默认（黑），**不是『假装用户选过白』**」；
- `:592`「旧档没有这个键 → `true`（迁移默认：老用户界面不变）」；
- `:626`「旧存档没有这两个字段 → 用新默认值（不是 false/0）」；
- `:608-609`「**端点夹持**（0 仍＝关），不是『越界回落默认』——回落 0 会把用户开着的轮播静默关掉」。
`browser_io.dart:30-35` 也说明了粒度：背景清单的三条预算在 `readStagePlaylist` 里**逐项**把关，
「一条坏数据只丢自己，不会把主题 / 音量一起判死」。

### 核验三（**假设被证伪**）：`slideInterval` 的「关」哨兵不会被钳成「开」
`defaultSlideInterval = 0` 表示关闭，而 `minSlideIntervalSeconds = 5`。
若 `clampSlideInterval` 无脑夹进 [5, 300]，**用户关掉的轮播会在重启后自己活过来**——这是个具体的
可查的用户可见缺陷。实读（`display_prefs.dart:865-869`）：
```dart
static int clampSlideInterval(int value) {
  if (value == 0) return 0;          // ← 哨兵先行返回，不进区间夹持
  return _clampIntToRange(value, minSlideIntervalSeconds, …);
}
```
⇒ **证伪**。且头注 :608-609 正是为了提醒后来者「0 有特殊含义」而写的。

### 核验四：选图三态（取消 / 成功 / 读失败）已按 AGENTS.md 的记录修好
`browser_io.dart:104-110`：「返回 **取消 / 成功 / 读失败** 三态：取消（`dataUrl==null && error==null`）
不该弹东西；读失败（`error!=null`）必须说实话——以前读失败与取消不可区分，用户看到的就是
『点了选图没反应』」。并且 :100-102 诚实记录了**没有**做的事与其代价：「**不做缩放**（刻意的）：
裁剪要走 canvas，而 canvas 只能在浏览器里跑、本仓库的 `flutter test` 覆盖不到——本轮不引入
无法回归的代码。代价是『大图不写盘』，由调用方按 [kStageImageMaxChars] 如实告知用户。」

---

## BATCH-0034（2026-09-28）· 序列化方向 + 判等面（前端落盘真源的最后一半）

> 净产出：4 个假设被证伪 + **1 条 P3**（一个「装好枪但没扣扳机」的潜伏陷阱，
> 而且**枪和免责声明写在同一个文件里**）。

### 核验一：序列化方向**与字段集完全对齐**（24 / 24）
构造函数 [display_prefs.dart:96-121](</home/skystar/Live2D-Ai-fe/shell/flutter/lib/settings/display_prefs.dart:96>) 恰好 24 个字段；
`toJson`（:537-564）恰好写这 24 个键，一个不多一个不少。
⇒ 任务书 §7-E 点名的「设置字段漏一个 ⇒ 静默失效」在这条链上**不成立**。

### 核验二：`operator ==` 覆盖 24 / 24（22 个单字段 + `backgrounds` 逐项 + `stagePlaylist`）
这一点很关键，因为 `main.dart:237-241` 的落盘门是 `if (next == _prefs) return true;`
——**`==` 少一个字段 ⇒ 那个字段改了不落盘也不生效**。实读 :942-973，24 项齐。

### 核验三（**证伪**）：`slideInterval` 的「关」哨兵不会被钳成「开」
见 BATCH-0033 记录（`clampSlideInterval:866` `if (value == 0) return 0;`）。

### 核验四（**证伪**）：逐图样式控件在回调缺失时**整个按钮不渲染**，不是「点了没反应」
我一度以为找到一条 P4 静默失效：`DisplayPrefs.==` 走 `sameAs`（只看 `id`），
而 `background_item.dart:368-375` 自己写着「**要**看样式…漏掉它会让『改了逐图铺法』被判成
『什么都没变』⇒ 不落盘（静默失效）」—— 也就是说 `==` 选了「不看样式」的那一个。
再查 `onItemChanged`：**全仓除 `appearance_background.dart` 外零命中**，即**没人提供它**
（`appearance_section.dart:212-227` 提供了另外 9 个回调，独缺这一个）。
若控件仍渲染 ⇒ 死 UI（P4 静默失效）。实读 `_ImageTile`：
```dart
if (image != null && styleChanged != null)          // appearance_background.dart:1264
  TextButton(onPressed: onToggleStyle, child: Text(styleExpanded ? '收起样式' : '样式')),
```
⇒ 回调为 null 时**「样式」按钮根本不出现**。降级是优雅的：功能没接 ⇒ 入口不露。
**假设证伪。**

### F-0034-01 · P2 【B0087 已由 P3「潜在」升级为「在线」：逐图样式变更被 `DisplayPrefs.==` 静默丢弃】
> ⚠ **定级以本行为准**。B0034 原文（`## BATCH-0034` 段内）写的是「潜在陷阱」，
> B0087 顺着接线核到生产路径后**证伪了「潜在」**：回调**在生产里接上了**，
> 缺陷在**下游判等**（`display_prefs.dart:970` 调 id-only `sameAs`）⇒ **在线**，P2。
> **旧措辞保留**在下方 B0034 段内，不删。
file: shell/flutter/lib/settings/display_prefs.dart:970
摘录:
```dart
if (backgrounds.length != other.backgrounds.length) return false;
for (int i = 0; i < backgrounds.length; i++) {
  if (!backgrounds[i].sameAs(other.backgrounds[i])) return false;   // :970 —— sameAs 只比 id
}
```
而同族另一个方法（[design/background_item.dart:368-382](</home/skystar/Live2D-Ai-fe/shell/flutter/lib/design/background_item.dart:368>)）把两者的差别写得很清楚：
```dart
/// 为什么不看 [dataUrl]：同一张图在「字节读回来之前」与「之后」必须判为「同一项」…
/// 为什么**要**看样式：样式是用户改出来的「这一项怎么画」。漏掉它会让
/// 「改了逐图铺法」被判成「什么都没变」⇒ 不落盘（静默失效）。
@override
bool operator ==(Object other) => other is BackgroundImage && other.id == id &&
    other.opacity == opacity && other.fit == fit && other.align == align;   // :377-382
```
影响: 可维护性/正确性（**当前不可达，但距可达只差一条接线**）：
`DisplayPrefs.==` 刻意用 id-only 的 `sameAs`，这在今天无害，因为逐图样式这条路
`onItemChanged` 没人提供 ⇒ 入口按钮不渲染。**但一旦有人把 `onItemChanged` 接上**
（这正是 `appearance_background.dart` 已经建好的 API——`_fitControls` / `_AlignPad` /
`_moreOptions` 都在，:39 的表格里列着），
「改这一张图的铺法」就会撞上 `main.dart:238` 的 `if (next == _prefs) return true;`
⇒ **被判成「什么都没变」⇒ 既不 `setState` 也不 `saveDisplayPrefs`** ⇒
用户拖滑杆 / 换铺法，界面不动、刷新还原，且**没有任何错误**。
⇒ 这是本审计至今最典型的「**难形态未被钉住**」形态：易形态（选中/排序/删除/加图）全都有回调，
唯独最难的那条（逐图样式）既无回调也无断言。
建议: ① 在 `DisplayPrefs.==` 的 `backgrounds` 比较处改用 `==`（含样式）而不是 `sameAs`——
   `sameAs` 的语义（「字节读回前后算同一项」）可以留给**专门判字节就绪**的调用点；
   ② 若确认 `onItemChanged` 短期不接，则在 `onItemChanged` 的声明处写明
   「**接上之前 `DisplayPrefs.==` 会吞掉样式改动**」，让下一个接线的人先看见这颗雷
验证: 读 display_prefs.dart:96-121 / :537-573 / :942-973、background_item.dart:360-385、
appearance_background.dart:946-960 / :1262-1268、appearance_section.dart:212-227、
main.dart:237-241；`grep -rn "onItemChanged" shell/flutter/lib/` → **仅 1 个文件命中**
置信: 高（当前不可达已证：`onItemChanged` 零提供方 + 按钮条件渲染；一旦可达的机制也已逐环读出）
反证: (a) 试过论证「现在就会丢样式」——不会，`onItemChanged` 全仓零命中，入口按钮不渲染；
(b) 试过论证「`sameAs` 与 `==` 的差别只在 dataUrl」——**不成立**：`==` 还看 opacity/fit/align，
而 `sameAs` 只看 `id`（background_item.dart:365-366 vs :377-382），差别正是样式；
(c) 试过找测试钉住「样式改动会被吞」或「样式控件在无回调时不出现」——**未核实**
（Dart 测试尚未系统查，留后续测试批次）。若已有测试覆盖 (c)，本条应降级为纯备忘。

---

## BATCH-0036（2026-09-28）· 主链路 Dart 侧：`chat/turn_liveness.dart` —— **跨语言契约核验**

### F-0036-01 · P3 【一个空的 match 分支承载着契约，而契约只写在 Dart 侧】
file: crates/live2d-ai-desktop/src/supervisor/handlers.rs:324
摘录（Rust 侧，全部相关分支）:
```rust
RootEffect::TurnAborted { .. } => {}            // :324 —— 空分支
RootEffect::GoIdle => {}                         // :325
RootEffect::TurnCompleted { outcome, .. } => {    // :326
    emit(AppEvent::RootAudit(RootFact::TurnCompleted { epoch, … }));   // :328-330 → WS turn_state 帧
```
而这条依赖写在**另一个语言**的文件里（`shell/flutter/lib/chat/turn_liveness.dart:149-157`）：
```dart
/// 「上一轮被新代次作废」导致收口时的码。
/// 触发点是 WS 的 `runtime_status{event:"new_epoch"}`——它的语义就是「上一轮作废」
/// （见 `audio/epoch_gate.dart` 的 `mustInterruptAudio`）。
/// **必须在这里也收口**：停止路径在后端走的是 `RootEffect::TurnAborted`，
/// 而它**不投影任何 WS 帧**（`supervisor/handlers.rs`），也就是停止**不会**产生 `turn_state`。
/// 于是「别人（或另一个客户端）按了停止」在本客户端看来就是「这一轮永远不结束」——输入框锁死。
const String kTurnPreemptedCode = 'turn_preempted';
```
影响: 可维护性/正确性（**我逐字核了这条跨语言断言——它是对的**：`TurnAborted => {}` 确实是空分支，
所以「停止不产生 `turn_state`」为真，Dart 的 `new_epoch` 收口是**承重**的而非冗余兜底。
但这条依赖**只存在于 Dart 侧**：Rust 侧那个空分支没有任何注释说明「客户端依赖我为空」，
读 Rust 的人看到 `=> {}` 只会得出「这里没什么可做的」——而这在服务端视角下恰好是对的。
两个方向都会静默出事：① 有人「好心」给 `TurnAborted` 补一个 `turn_state` 帧（让停止对称），
跨语言耦合被无声改变；② 有人按 Rust 侧注释理解前端行为，得出错误结论。）
建议: 在 `handlers.rs:324` 的空分支上写一行注释，指明「**空是契约**：客户端靠
`new_epoch` 收口（见 `chat/turn_liveness.dart` 的 `kTurnPreemptedCode`）；
若将来在这里投影 `turn_state`，必须同步检查客户端是否会双重收口」
验证: 读 supervisor/handlers.rs:320-340（Rust 侧原文）、turn_liveness.dart:88-200（客户端侧原文）；
B0017 已独立核过 `RootFact::TurnCompleted` → `ws/events.rs:62-71` 的 `turn_state` 投影
⇒ 两侧一致：**只有 TurnCompleted 出 turn_state**
置信: 高（两侧原文逐行对照 + 投影链已在早前批次独立验证）
反证: (a) 试过找第三条收口路径能替代它——`mustSettleTurnOnError`（:169）只认 **fatal**，
而停止不产生 error 帧；`mustReleaseTurnOnWsLoss`（:100）只认通道 `isProblem`，
而「别人按停止」时通道正常 ⇒ 两条都不覆盖，**`new_epoch` 是唯一出路**；
(b) 试过论证「双重收口也无害」——即使将来 Rust 补了帧，`frameBelongsToCurrentTurn`（:178）
会把同 epoch 的帧都收一次，liveness 状态机大概率幂等，所以 (a) 的风险主要是**理解偏差**
而非立即故障；故定 P3 而非 P2。(c) 试过找测试钉住「停止 ⇒ 收口」——**未核实**（Dart 测试面
本批未系统查），若已有覆盖，本条可降为纯备忘。

---

## BATCH-0036 —— `turn_liveness.dart` 的其余判据核验（全部与 Rust 侧对得上）

| Dart 判据 | Rust 侧对应（我在 B0015/B0017 已核） | 判定 |
|---|---|---|
| `mustSettleTurnOnError`：**只**在 `fatal` 时立刻收口（:169-170） | `ErrorKind::is_fatal()` = `Tts \| Decode \| Backpressure`，**`Llm` 不致命**（error_code.rs:70-75）；且 `engine.rs:542` 的 `TurnStatus::Failed` 含 `llm_failed` | ✔ 两侧一致。Dart 的理由「非致命 LLM 失败后已生成的语音还要播完」正是 Rust 的 D8 裁决 |
| `frameBelongsToCurrentTurn`：两侧任一为 null ⇒ 判当前轮；都有且不等 ⇒ 丢弃（:178-184） | `turn_state` / `text_delta` / `error` / `runtime_status` 四个帧型**都带 `epoch`**（ws/events.rs 全量投影，B0003） | ✔ 契约对齐 |
| `mustReleaseTurnOnWsLoss`：用 `isProblem`（断开/已关）而**不是** `!isUsable`（:96-103） | —（纯客户端） | ✔ 理由写明「`connecting` 是正在建连，发送瞬间可能还在连，那时收掉是错的」——**这是个容易写错的区分** |
| `kReasoningOnlyCaption`：「本轮只有思考、没有正文（正文可能被『输出 token 上限』截断）」（:121-123） | 正是 AGENTS.md 记录的 512→4096 那次事故的现象 | ✔ 提示指向用户可执行的下一步 |
| `kWordlessTurnNotice` = 「本轮模型没有返回文字」（:114） | 历史文案曾写「通常是模型只调用了动作工具」——动作系统已整体删除（rc.2） | ✔ 已随裁决更新，且 :111-113 记了为什么删 |

---

## BATCH-0037（2026-09-28）· `chat_controller.dart` + 双消费路径核验

> **0 条新发现。** 本批核的是一个我在 STATE 里挂了 6 批的疑问：
> `UiStateTracker` 与 `ChatController` 是两条 WS 订阅路径，**各持一份「本轮是否在进行」与「当前错误码」**——
> 任务书 §7-A 明列的「两份快照」。结论：**不是缺陷，是刻意的分工，且失效半径只有显示层。**

### 核验一：两条路径的**收口规则不同，但互补而不矛盾**
| | `ChatController`（聊天链路） | `UiStateTracker`（状态胶囊/相位） |
|---|---|---|
| 收口信号 | `settleTurn(...)` 全部四条（keep / keepReasoningOnly / failed / wordless / stopped），走 `turn_liveness.dart` 的**纯函数** | **只**认 `turn_state`（completed 与 failed 都收）与 `text_delta{completed}` |
| 兜底 | **fatal 错误立刻收口**（`mustSettleTurnOnError`），防 `turn_state` 丢帧导致「永久转圈」 | **不**因裸 `error` 帧收口 |
| `new_epoch`（被别人停止） | 收口（`kTurnPreemptedCode`） | 收口（`_enterInterrupted()`） |

`ui_state_tracker.dart:181-182` 把这个分工写明了：
`// 正文兜底只往气泡里补文字，**不改相位**：收口仍由紧随其后的 turn_state（或 text_delta{completed}）负责。`
而 `turn_liveness.dart:162-168` 从另一侧说明同一个不变式：「致命错误后端会立即丢弃剩余排队句子并结束本轮——
若那帧 `turn_state` 因任何原因丢了（旧服务端 / 帧被截断），界面就会**永久转圈**…所以这里兜底收口。」

**我核了这条不变式在 Rust 侧确实成立**（B0013 + B0035 已读原文）：
致命错误 → `WorkerExit::Failed` → `TurnStatus::Failed`（engine.rs:542）→
`RootEffect::TurnCompleted{outcome}`（handlers.rs:326）→ `AppEvent::RootAudit(RootFact::TurnCompleted)`
→ `ws/events.rs:66-70` 投影成 `turn_state{status:"failed"}` ⇒ **胶囊一定会被收口**。

⇒ **输入框不会被锁死**：它的锁是 `chat_controller._streaming`（:487 收口时置 false），
而聊天链路走的是**带兜底**的那一套。胶囊的「说话中」只由权威帧驱动。
⇒ 若 `turn_state` 真的丢了，失效半径是**「胶囊仍显示说话中、聊天已收口」**——显示层不一致，
**下一轮 `voice_started` 即自愈**，且用户仍能正常发消息。**故不记发现**（记此备忘）。

### 核验二：错误文案两条路径**共用同一个格式化函数**（跨路径一致性有保障）
`ui_state_tracker.dart:173-178`：
```dart
case WsErrorEvent(:final message, :final code, :final hint):
  _errorActive = true;
  // 与 `ChatController` 用**同一个**格式化函数：两条订阅路径上屏文案
  // 一致，不会出现「聊天面板有码、顶部横幅没码」这种分裂。
  _errorMessage = formatWsError(code: code, message: message, hint: hint);
```
且两边的「拿不到详情就兜底码」也是**同一个字面量**：
`turn_liveness.dart:190` `kTurnFailedWithoutDetailCode = 'turn_failed_no_detail'`
↔ `ui_state_tracker.dart:169` `_errorCode ??= 'turn_failed_no_detail';`
⇒ 同一个码常量在两处使用（`turn_liveness` 导出 + tracker 硬编码字面量，**不是同一个引用**——
严格说是「同值不同源」，但两处都有注释指明同源，属可接受；见下方备忘）。

### 核验三：会话切换的**时序陷阱被显式堵住**（`chat_controller.dart:507-510 / 515 / 526 / 553`）
`newSession` / `selectSession` / `deleteSession` **每一条**都先 `if (…) _abandonTurn();`。
头注把理由写死：「否则一条迟到的 delta 会落进新会话（`_assistant` 还指着旧气泡…）
这里显式收口是为了让『切走时那一轮就算结束』这件事**在代码里看得见**，而不是依赖一个巧合。」
⇒ 这正是任务书 §7-C「精确路径匹配 / 引用语义」类陷阱，**已主动防御**。

### 核验四：落盘节流的口径与实现一致
`chat_controller.dart:497-500`：`// **一轮结束才落盘**：text_delta 是毫秒级的，
逐条写 localStorage 会把主线程拖垮（序列化整份会话 × 每秒几十次）。` → `sessions.touchActive(); _persist();`
会话增删改查（:518/528/546/554）也各自 `_persist()`，体积由 `browser_io.dart:81-83` 的
`maxSessions × kMaxMessagesPerSession` 双重夹持封住。✔

### 一处备忘（不记为发现）
`turn_failed_no_detail` 这个码在 `turn_liveness.dart:190` 定义、在 `ui_state_tracker.dart:169`
**以字面量**再写一次。两处都有注释指明同源，但「同值不同源」意味着将来改 `turn_liveness`
那一处不会传播到 tracker。属极低风险（值是稳定的协议码），**仅记备忘**。

---

## BATCH-0038（2026-09-28）· `chat_session` 体积夹持 + **`audio/epoch_gate.dart`（红线「停止必须立刻静音」）**

> **0 条新发现。** 本批把上一批的跨语言线索（`turn_liveness` 注释引用的 `epoch_gate.dart`）读完，
> 得到一次**跨三语言、两侧都有测试**的红线验证 —— 这是本审计至今证据最完整的一条。

### 核验一：「停止键 = 立刻静音」这条红线**端到端成立**，且两侧都有钉子
| 环节 | 落点 | 证据 |
|---|---|---|
| 决策判据（抽成纯函数） | `flutter/lib/audio/epoch_gate.dart:26-28` `mustInterruptAudio` | 只有 `runtime_status{event:"new_epoch"}` 触发；:32-34 `audioEpochChanged` 兜跨轮残留片 |
| 为什么要抽出来 | `epoch_gate.dart:3-8` | 「原实现埋在 `chat_controller.dart` 里——而那个文件必须 `import 'package:web'`，**在 `flutter test` 里加载不了**。于是『停止按钮等于失灵』这类回归永远抓不住。抽成纯函数后可单测」 |
| 判据的**实测依据** | `epoch_gate.dart:11-17` | 「TTS 分片是 **1 ms 级突发**到达的（一句 2 s 音频在眨眼间到齐），播放时间轴最多排到 **3 s 之后**（**实测调度提前量峰值 3.03 s**）」 |
| 顺序陷阱被写明 | `epoch_gate.dart:44-46` | 「**先判 epoch 变化，再更新当前 epoch**——反过来的话 `audioEpochChanged` 永远为假，跨轮残留的片就漏掉了」 |
| 服务端确实在 stop 时发 | `desktop/src/supervisor/turn.rs:674-677` | 「正常完成**不**发 NewEpoch：epoch 未变…再发 new_epoch 会让前端误开一个空 bubble。**NewEpoch 只由 stop 路径（`do_stop!`）在真正推进 epoch 时发送**」；发点 `turn.rs:149` |
| 服务端有回归钉子 | `supervisor/tests_stall.rs:232-239` | 「**stop 后根必须推进到 epoch=1**」→ 断言 collector 里出现 `ConversationUiEvent::NewEpoch { epoch: 1 }` |
| 帧投影 | `web_api/ws/events.rs:169-175` | `ConversationUiEvent::NewEpoch` → `runtime_status{event:"new_epoch", epoch}`（B0017 已核） |

⇒ **本条同时闭合了 F-0036-01 的一半疑问**：停止信号到达客户端走的是 `NewEpoch`
（`do_stop!` 直接发），**不是** `TurnAborted`（空分支，B0035 已核）。两条路径并存且互不矛盾：
`TurnAborted` 是 root reducer 的 effect（不投影），`do_stop!` 是 supervisor 轮模块的显式发帧。
⇒ F-0036-01 的结论（空分支承载「停止不产生 `turn_state`」这一契约）**依然成立**，
而 stop 的实际信号另有出处 —— **两者不冲突，我的记录是准确的**。

### 核验二：`chat_session` 的体积夹持**不会删掉用户正在看的会话**
`_trimSessions`（`chat_session.dart:346-363`）丢最久未用的，且：
```dart
// 这里刻意**不用** `firstWhere(..., orElse:)`：`orElse` 会在「找不到
// 非当前会话」时回落到第一个（也就是当前那个），于是兜底反而把它删了。
// 找不到就**不裁**——宁可暂时超一个上限，也不能删掉用户正在看的会话。
```
⇒ 明确避开了一个真实的反模式，并给出取舍方向（宁可超限不删用户在看的东西）。
`_trimMessages`（:365-369）从最旧端 `removeAt(0)`。
**为什么「切一半的问答对」不构成缺陷**：`ChatBody`（`web_api/chat_routes.rs:70-80`）只有
`text` + `session_id`，**不带历史**；LLM 上下文由 Rust 侧 `self.history` 提供，
受 `max_history_pairs` 约束（`conversation/mod.rs:155` 缺省 **0 = 永不保留**），
与 Dart 存储**完全解耦** ⇒ Dart 侧 500 条上限**只影响显示**，切一半不产生语义问题。

### 核验三：`relativeTime` 显式注入 `now`（可测性纪律）
`chat_session.dart:374-379`：「**纯函数、显式传 `now`**：既能单测，又**不依赖系统时钟**
（依赖 `DateTime.now()` 的断言会随机红）。放在纯逻辑层而不是 `ui/session_sheet.dart`——
那个文件 import Flutter，于是这段算术在 VM 上就测不了了」⇒ 与 `breakpoints.dart` /
`display_prefs.dart` 同一条纪律，**跨三个文件一致执行**。

---

## BATCH-0039（2026-09-28）· `live2d-ai-core` —— 休眠台账与 epoch 闸门

> **0 条新发现。** 本批审的是**红线 R 休眠台账**的落地处（AGENTS.md：「每一处残留都要能回答
> 『谁休眠、为什么、谁能唤醒』」）与该 crate 自称的**唯一权威闸门**。

### 核验一：休眠声明**逐项可答**（`lib.rs:33-56`）
- **谁休眠**：`action` 与 `performance` 两个模块（`#[doc(hidden)]` 标注，:58-61 / :71-79）；
- **为什么**：「桌面侧把 `RootEvent::Action` 送进 reducer 的**唯一**通道已整体删除
  （`SupervisorHandle::trigger_action` + supervisor 的 `action_rx` 分支）；
  动作序列的唯一驱动方 `live2d-ai-mod-director` 已删除」；
- **还剩什么**：「`ModServices.action_tx` 仍在（Mod API 契约），但 host 注入的是**固定休眠
  sender**：请求只留一行 debug 日志并返回 `false`」（B0005 已从 desktop 侧核过该 sender 的实现）；
- **谁能唤醒**：「`docs/architecture/core-chain-baseline.md` §3.2 与 AGENTS.md 休眠台账」；
- **为什么保留而不删**：「保留原样（**不变量 4 与 6 仍以它们为前提**）」——这是关键理由：
  删除会让 `State::action` 与 `ParameterFrame` 的类型定义跟着消失，比留着更危险；
- **一条防误删的显式警告**（:53-54）：「**待机生命体征**：`IdleState` 在渲染面
  （`l2d-wasm-demo`），与 performance 的动作曲线是两套机制，**删动作时绝不要连带删它**」。

### 核验二：不变量 1「epoch 单一 owner / 唯一权威闸门」**在休眠路径上同样成立**
这是本 crate 最强的一条声明，也是**最可能有旁路**的一条（动作子系统是被保留的代码）。
逐行核完，**无旁路**：
```rust
// reducer.rs:24-34 —— 闸门在任何规则之前
if let Some(event_epoch) = event.epoch() && event_epoch != state.epoch {
    return vec![Effect::Dropped { reason: DropReason::StaleEpoch { event_epoch, current_epoch: state.epoch } }];
}
```
```rust
// events.rs:91-102 —— 除 StopRequested 外，**每一个**变体都带 epoch
Event::UserSubmitted{..} | SentenceAdded{..} | GenerationFinished{..} | PlaybackStarted{..}
| PlaybackDrained{..} | PlaybackCleared{..}
| **Event::Action{..}**            // :99  ← 休眠子系统的事件**也在闸内**
| **Event::ActionPlaybackFinished{..}**  // :100
    => Some(*epoch),
Event::StopRequested => None,      // :101 —— 唯一豁免（它就是推进 epoch 的那个）
```
⇒ `lib.rs` 的原话「包括动作子系统（[`Event::Action`]）」**为真**，不是愿望。
`DropReason::StaleEpoch` 的字段注（events.rs:109-110）还写明「**先于**一切 id 判定」，
与代码顺序一致（闸门在 :25，各规则的 id 判定在 :37 之后）。

### 核验三：休眠由**三层**独立挡住（不是「靠没人调」）
| 层 | 落点 | 性质 |
|---|---|---|
| 1. 驱动通道已删 | `trigger_action` + `action_rx` 分支（rc.2 整体删除） | 结构删除 |
| 2. Mod API 返回 false | `mod_registry.rs:385-393`（B0005 已核）`ModActionSender::new(|req| { tracing::debug!(…「动作子系统休眠中（rc.2 起无驱动方）：ActionRequest 被丢弃」); false })` | 运行时拒绝 |
| 3. 闸门兜底 | 上面的 `events.rs:99-100` + `reducer.rs:25` | 状态机拒绝 |
⇒ 即使前两层被绕过，第三层仍会把它丢成 `Dropped(StaleEpoch)`；而**第三层本身不依赖休眠**——
它是常驻代码。**这是本审计见到的对「休眠资产」最稳的处置**：不靠承诺，靠闸门。
（AGENTS.md 还记了第四层：`main.rs::mod_count_is_three` 的工厂数断言，本批未核。）

---

## BATCH-0040（2026-09-28）· core 不变量 3/5 + **休眠台账的两个锚点已失效**

### 核验：双闩锁（不变量 3）与 stop 原子性（不变量 5）**按声明实现**
- **「生成结束 ≠ turn 完成」**：`maybe_finish_turn`（reducer.rs:236-253）先要 `turn.generation`
  （:240-242）**且** `turn.playback.is_terminal()`（:243-245）才收口；Completed + Playing
  ⇒ 直接 return，turn 保持存活。
- **「恰一次」的实现方式值得记**：它**不靠一个 `completed: bool` 标志**，而是靠
  `state.active_turn = None`（:247）——turn 不存在了，第二个 `GenerationFinished` /
  `PlaybackDrained` 自然落进 `UnknownTurn` 被丢。**单一真源，无第二份状态。**
- **stop 原子性**（`stop`，reducer.rs:257-272）：`epoch.next()`（:258）→ `active_turn.take()`（:259）
  → `phase = Idle`（:260）→ 无条件发 `StopPlayback` + `GoIdle`（:261，「对空 ring 幂等」），
  再按需补 `Action(End)`（:262-264）与 `TurnAborted`（:265-270）。**三件事在同一个函数里、
  无早退** ⇒ 「一次 stop 同时清空 turn 计划、动作，并推进 epoch 恰好一次」为真。
- **不变量 4**（单 active 动作）：`action_playback_finished`（:185-201）只在
  `state.action.current == action` 时收尾并给**恰好一个** `End`，否则
  `Dropped(StaleActionCompletion)`——「绝不误伤后来者」为真。
⇒ 四条不变量（1/3/4/5）**全部逐行核实通过**；`core_tests/` 的测试名与它们**一一对应**
（`completed_generation_before_drain_keeps_turn_alive` / `duplicate_or_out_of_order_facts_are_rejected`
/ `stop_clears_everything_and_advances_epoch_once` / `stale_events_of_every_kind_are_dropped_after_stop`
/ `reused_ids_across_epochs_are_isolated_by_epoch_gate` / `action_playback_finished_is_identity_checked`）。

### F-0040-01 · P3 【**已撤回（B0211 撤回 · B0218 修正撤回理由）** —— 原指控从一开始就不成立】
> 🔻 **原指控**：「AGENTS.md 内部不一致：`:105`/`:608` 说 `mod_count_is_three`」。
> ✅ **真实的撤回理由（B0218 核出，比 B0211 给的更根本）**：
> 那两行**从来没有断言「护栏是三个」**，它们**描述的是一次变更**：
> `:105` 逐字「`mod_count_is_three` **→** `mod_count_is_five`；**缺省仍只启用 `external-input`**」
> `:608` 逐字「（追加 `voice-input` / `wallpaper`），`mod_count_is_three` **→** `mod_count_is_five`」
> ⇒ **两处都是「从三变到五」的正确历史记录**，而**不是**「现在护栏是三个」的过时刻画
> ⇒ ⇒ **原指控从一开始就误读了这两行的语义** ⇒ **永久撤回**。
>
> ⚠ **B0211 的撤回理由本身是错的，已作废**：我当时写「**全文已无 `mod_count_is_three`**」，
> 而 `grep -c 'mod_count_is_three' AGENTS.md` = **2** ⇒ **那句话不成立**。
> **错因**：我用了 `grep -n "mod_count_is" AGENTS.md | head -3`
> ⇒ **只看了前 3 行**（`:45 :46 :73`，恰好都是**已修正**的说法）⇒ **截断被「可见前缀恰好支持我的结论」掩盖了**。
>
> ⭐ **由此得到一条比「别用 `head`」更精确的规则**：
> > **`head` 最危险的时候，是「被截断掉的那部分**恰好不含反证**」的时候** ——
> > 此时截断**不留痕迹**，而你会带着一个「已核实」的错误结论继续走。
> > **对策**：凡是要据此下「不存在 / 全部 / 没有」这类**全称判断**的 grep，
> > **必须让命令自己报计数**（`grep -c`）或**不加 `head`** —— **计数不会骗人，前缀会**。
>
> ⇒ 这是本审计**第 5 次 grep 模式失效**、**第 2 次 `head` specifically**（第 1 次是 B0130）。
> **原文保留**在下方 B0040 段内供追溯。


> 🔻 **撤回理由（机械可核）**：`grep -n "mod_count_is" AGENTS.md` ⇒ **只命中 `:45` `:46` `:73`**，
> 且三处**都写 `mod_count_is_five` / `mod_count_is_seven`** ⇒ **全文已无 `mod_count_is_three`**。
> AGENTS.md `:45-46` 逐字：「两者移出 `AVAILABLE_MOD_FACTORIES`，**7 → 5**，`mod_count_is_seven` →
> **`mod_count_is_five`**；crate 暂留 workspace（可编译可测）并标 **ARCHIVED**」
> ⇒ **与代码一致**（`main.rs:471` `mod_count_is_five`，B0170 已核）⇒ **本条所指的文档缺陷已被修复**。
> ⇒ ⚠ **连带撤回我在 B0170 写的「影响升级」**：那句「**文档指向了一个不存在的符号**」**已不成立**。
> ⇒ **教训**：**一条发现的「对象」可能被修好** ⇒ 复核时必须**先核对象是否还存在**，
> 再谈它是否仍成立（**这与复核代码缺陷同构**：代码改了，结论也要重算）。
> **原文保留**在下方 B0040 段内供追溯。


> ⚠ **B0170 升级影响（定级仍 P3）**：AGENTS.md 的休眠台账把护栏写成
> `main.rs::mod_count_is_three`，而**实际是 `mod_count_is_five`**（`main.rs:471`，:59 自称「防回归断言」）
> ⇒ 这不是「文档内部不一致」，而是「**文档指向了一个不存在的符号**」——
> **照着 AGENTS.md 去核对护栏的人，会去找一个没有的断言**；且台账的「三个」比实际**落后两位**。
> **同时**：director 的「已删除」指的是**旧的动作序列 driver**（归档 `archive/action-layer-p6`），
> 而**名字被复用**给了一个**刻意 no-op 的新骨架**（在 members 里 · `mod_registry.rs:670`
> 「行为**一字不变**」）⇒ **同名两物**；已核 `arbiter.rs` 拥有规则 cue 机制 ⇒
> `performance/mod.rs:65` 那句「单一真源 = director 的纯函数」**指的是新骨架、没错**。
file: AGENTS.md「动作与表演的归属（休眠台账）」+ crates/live2d-ai-core/src/lib.rs:38-42
摘录（台账点名的两条护栏之一，在当前代码里**不存在**）:
> AGENTS.md：「护栏是两条断言：`main.rs::mod_count_is_three`（工厂数不得因动作 Mod 增加）与
> `mod_registry::tests::action_request_is_dormant_not_delivered`（动作请求必须不被接受）。」
而实际（`crates/live2d-ai-desktop/src/main.rs:470-471`）:
```rust
#[test]
fn mod_count_is_five() {          // ← 真名是 mod_count_is_five，全仓无 mod_count_is_three
    assert_eq!(super::AVAILABLE_MOD_FACTORIES.len(), 5, "…exactly 5 Mod factories (external-input, persona, voice-input, memory, director)");
}
```
（`grep -rn "mod_count_is_three|mod_count_is_four" crates/` → **零命中**。
另一条 `action_request_is_dormant_not_delivered` **确实存在**，`mod_registry.rs:1244`。）

摘录（台账说「已删除」的那个 Mod，**当前仍在静态注册表里**）:
`live2d-ai-core/src/lib.rs:40`：「动作序列的唯一驱动方 `live2d-ai-mod-director` **已删除**」；
AGENTS.md 同款表述。而 `main.rs:93-95` 实际注册着它：
```rust
// Wave 3（2026-09-14）：导演最小骨架（Wave 3 从 RFC 推进一格）。缺省停用；
// **零投递**——只订阅 TurnPrompt/TurnEnded、只产决策日志与 state_json。
&live2d_ai_mod_director::FACTORY,
```
影响: 可维护性/可验证性（**两处症状，同一根因：Mod 列表在 rc.1→rc.4→Wave1/2/3→封存波次之间反复增减，
而休眠台账的措辞没跟着走**）：
1. 按 AGENTS.md 去核「工厂数护栏」的人，`grep mod_count_is_three` **什么都找不到**，
   无法确认这条护栏存在——**恰恰是红线 R 最需要可核的那个锚点**（AGENTS.md 变更历史里它
   还经历过 `mod_count_is_four` → 现在的 `mod_count_is_five`，名字一路没被台账跟上）；
2. 按台账去核「动作驱动方是否真的没了」的人，`grep live2d-ai-mod-director` 会看到它**在册**，
   而唯一能澄清「这是另一个**零投递**骨架、与 rc.2 删掉的那个不同」的那句话，
   藏在 `main.rs:466` 的**变更日志注释**里——不在读台账的人会看的地方。
   （该测试自身的头注 :458-469 其实写得很完整，包括每一次数字变化与两者的区别。）
建议: 台账与 core 头注各加一句指向当前锚点，例如：
「护栏现为 `main.rs::mod_count_is_five`（恰好 5 个工厂；`grep mod_count_is_three` 已失效——
名字随工厂数增减多次变更，**以 main.rs 里的实际测试名为准**）」；
以及「rc.2 删除的是**动作驱动版** director；现注册的 `live2d-ai-mod-director` 是 Wave 3 的
**零投递骨架**（`main.rs:72-77`），**不驱动动作序列**」
验证: 读 main.rs:60-96（工厂表 + 变更日志）、:456-477（真名与断言）、
live2d-ai-core/src/lib.rs:33-56（台账原文）；`grep -rn "mod_count_is_three|mod_count_is_four" crates/` → 零命中；
`grep -rn "action_request_is_dormant" crates/` → `mod_registry.rs:1244` 命中
置信: 高（三个 grep + 三处原文均可复跑）
反证: (a) 试过论证「AGENTS.md 那句是历史记录、不必更新」——它是**当前的**休眠台账正文
（「护栏是两条断言」是现在时），且它是红线 R 的唯一书面锚点；(b) 试过论证「director 在册说明动作
被接回来了」——不成立，`main.rs:94` 明写「**零投递**」，且 `mod_registry.rs:385-393` 的
休眠 sender 与 `action_request_is_dormant_not_delivered`（:1244）都在，**功能上确实仍休眠**；
本条的指控是**措辞与可核性**，不是「动作被误唤醒」。(c) 试过找第三处澄清——`docs/architecture/`
下的两份 Mod 文档我未读（不在本批文件清单内），**可能**已有澄清；若已澄清，本条应降为
「AGENTS.md 未交叉引用」并把修法改成加链接。

---

## BATCH-0041（2026-09-28）· `core_tests/` 断言体 —— 「难形态能否失败」第 4 次复查

> **0 条新发现。** 按模式 H 的执行纪律（不问「有没有测试」，问「**难形态**有没有测试」），
> 本批去读 `live2d-ai-core` 里最难的那条断言的**断言体**。

### `reused_ids_across_epochs_are_isolated_by_epoch_gate`（core_tests/mod.rs:196-275）
它构造的正是**唯一能让 id 判定失效**的场景，并把两侧都钉住：

**(1) 制造 ID 复用**
```rust
submit(&mut state, 7, 1);  started(&mut state, 7);        // 代次 0：turn#7 句#1
apply(&mut state, Event::StopRequested);                   // epoch → 1，全部清空
submit(&mut state, 7, 1) == vec![Effect::StartThinking{..}] // 代次 1：**重用相同数值 id** —— 合法
```

**(2) 让 id 判定**完全命中**、只有 epoch 闸门能拒**（:214-215 注释原话：「turn id 完全命中当前 turn，
但 epoch=0 → 必须丢弃」）：
`PlaybackStarted{epoch:E0, turn_id:7}` / `PlaybackDrained{epoch:E0, turn_id:7}` /
`GenerationFinished{epoch:E0, turn_id:7}` 三条**全部**断言 `stale(0, 1)`。

**(3) 断言「丢弃不留痕」**（状态不变性，:245-248）—— 这一条最关键，恒真断言通常就漏在这里：
```rust
assert_eq!(state.phase, Phase::Thinking);
assert_eq!(turn.playback, PlaybackState::NeverStarted);   // 不得被旧代次落闩
assert_eq!(turn.generation, None);                       // 同上
```

**(4) 断言闸门**没有过度拦截**（:253-256）：新代次的同 id 事实照常消费
（`started(&mut state, 7)` 无效果且 `phase → Speaking`）—— 若闸门写成「一律丢弃」，
这条会红。

**(5) 把双闩锁最难的那句直接断言出来**（:264-266）：
```rust
// GenerationFinished{Completed} 但仍在 Playing → turn 必须存活
assert!(state.active_turn.is_some());
```
随后真正的排空 → **恰好一个** `TurnCompleted`（:270+）。

⇒ **能失败**：删掉闸门 / 把闸门挪到 id 判定之后 / 让丢弃留下副作用 / 把闸门写成全拦，
这四类改法**每一种**都会让本测试变红。它是本审计读到的**最硬的一条测试**。

### 本次复查的累计结论（模式 B / H 的第 4 次）
| 复查点 | 形态 | 结果 |
|---|---|---|
| `stage_pointer_interceptor_test.dart`（B0023） | 祖先断言 | 能失败 |
| `font_subset_test.dart`（B0025） | 数据层哈希 + 词法器自测 | 能失败 |
| `no_backdrop_filter_test.dart`（B0026） | 源码扫描 + 判别力钉子 | 能失败 |
| `app_shell_background_test.dart`（B0032） | scope 身份探针 | 能失败 |
| **`reused_ids_across_epochs…`（本批）** | **ID 复用 + 状态不变性 + 不过度拦截** | **能失败** |
⇒ **连续 4 次复查「难形态」零命中**。与此对照，模式 B 的 4 处命中集中在
`model_root.rs` / `supervisor_slot.rs`（BATCH-0001/0002 早批）与
`mod_registry.rs` 的 round-trip 起点为空（BATCH-0013）两处形态。
**校准后的结论**：本仓的假绿灯**不是系统性文化问题**，而是集中在少数几个文件的少数几种写法；
门禁面与状态机核心的测试**质量很高**（含本批这条 ID 复用隔离测试）。
⇒ 后续不应再按「系统性假绿灯」去找，只需盯**新写的**测试与**未审的**目录。

---

## BATCH-0042（2026-09-28）· 休眠代码本体：`action/` —— **一个「活着但名字过期」的优先级档**

### F-0042-01 · P3 【`ActionSource::LlmTool` 是活的值，却用着一个已被拆除的子系统的名字】
file: crates/live2d-ai-core/src/action/mod.rs:112-118
摘录:
```rust
pub enum ActionSource {
    /// 用户直接命令（如手势/按钮/语音指令）：优先级 100，最高。
    UserCommand,
    /// 主 LLM 的 tool action 输出：优先级 90。        // ← 「主 LLM 的 tool action 输出」
    LlmTool,
    /// 确定性规则 fallback：优先级 50，最低。
    RuleFallback,
}
```
但它在**生产里是活的**，且含义已经不是它写的那样 —— 唯一的生产引用是这句注释：
```
// crates/live2d-ai-desktop/src/mod_registry.rs:288
/// Host 固定映射 Mod action → SemanticAction::LlmTool 优先级档。
```
影响: 可维护性/契约一致性（AGENTS.md 变更历史明写：「**LLM 工具层与动作系统已整体拆除**——
`llm.rs` 请求体不再有 `tools`/`tool_choice`/`functions`」），
所以「主 LLM 的 tool action 输出」这条**产出路径已不存在**；而这一档现在承担的是
**Mod 发来的动作请求**（host 映射进来的中间优先级带，UserCommand 100 > 该带 90 > RuleFallback 50）。
三重后果：
1. 读 `ActionSource` 的人会以为**工具层还在**，而它已被拆除 —— 这是一条**休眠台账没有登记**的残留；
2. 若将来有人决定恢复工具层，会发现「变体已经叫这个名了」，误以为只剩接线工作；
3. 反过来，删它会破坏 Mod API 契约（`ModServices.action_tx` 的请求类型带这个 source），
   所以**它不该被删，只该被改名或被注明**。
建议: 低风险修法——在 `LlmTool` 的字段注上补一句历史与现状：「原为『主 LLM 的 tool action 输出』；
rc.2 起 **LLM 工具层已整体拆除**，该产出路径不再存在；本档现由 host 承接 **Mod 发来的动作请求**
（见 `desktop/src/mod_registry.rs:288`），保留原名以免破坏 Mod API 契约」。
彻底修法（下一个大版本）：改名 `HostChannel` 或 `ModRequest`，同一处 `mod_registry.rs:288` 注释同步。
验证: 读 action/mod.rs:108-130 与 :295-320（优先级仲裁）、mod_registry.rs:288；
`grep -rn "LlmTool" crates/ --include=*.rs` → 22 处命中中，**生产代码只有 mod_registry.rs:288 的一句注释**，
其余 21 处全在 `#[cfg(test)]`（core_tests / action/tests / root_flow / supervisor tests_*）里
置信: 高（命中点已穷举；生产引用已定位到唯一一处）
反证: (a) 试过论证「这只是测试里的构造helper，无所谓」——它同时是**公开 API 的一部分**
（`live2d_ai_core::ActionSource` 经 lib.rs:72-75 再导出），且 host 的注释明确把它当作在用的档位；
(b) 试过论证「名字是 RFC D9 定的协议名不能改」——RFC 定的可能是**动作名**（`ActionId::name()`，
见 :52-61），而 `ActionSource` 是**调度优先级**的实现侧概念，没有协议稳定性义务；
(c) 试过论证「AGENTS.md 已说明工具层拆除，所以不算不一致」——AGENTS.md 说明的是**哪条路被拆**，
没有说明**这一档被谁接手**；接手信息只存在于 `mod_registry.rs:288` 的一句注释里，
不在任何台账上 ⇒ 这正是 F-0040-01 的同一根因（**台账与代码演进不同步**）的第三处症状。

### 核验：休眠代码的**仲裁语义**与模块头注一致
`apply_action`（action/mod.rs:295-320）：低优先级来者被 `ActionEffect::Dropped{LowerPriority}`
显式拒（:302-309）；同优先级「后来者胜」/更高优先级「抢占」→ **单一显式** `Transition` 效果。
与头注 :9-11「抢占是单一显式效果 `ActionEffect::Transition { from, to }`（shell 停旧起新，
替代『停旧+回中+起新』三连）」逐字相符。`ActionId` 六项的 `name()`（:52-61）与
`bit()`（:64-67，`repr(u8)` 判别式 0..=5 ⇒ `as u32` 安全）也都自洽。✔

---

## BATCH-0043 —— 休眠代码的**契约仍然有效**（与休眠台账并列核验）

`live2d-ai-core/src/performance/mod.rs` 虽然休眠，但它的三条契约与产品红线**同向**：
1. **口型所有权**（:15-16）：「本层**永不驱动嘴开合**（`MouthOpenY` 归 TTS RMS 通道），
   至多写 `mouth_form` 微调嘴形」⇒ 与红线 O（30Hz 口型走 TTS RMS 通道）**不冲突**；
   即便将来唤醒动作层，它也不会去抢那条通道。
2. **参数所有权用「释放」而非「写零」**（:95-101）：`ParameterMask` 置位表示「本帧由动作主动驱动」，
   未置位**一律不写**——注释写明理由：「『中性』意味着**释放所有权**，而不是持续写零压住
   idle/物理层」。这是比「写中性值」更正确的模型（写零会与待机/物理层打架）。
3. **除零风险证伪**：`envelope(p, attack, release_from)`（:89-91）含 `p / attack` 与
   `/(1.0 - release_from)` 两处除法；6 个调用点（:267/:274/:285/:294/:308/:324）全部传
   **字面量常量**且严格落在 (0,1) 内（0.30/0.65、0.15/0.85、0.30/0.70、0.12/0.88、0.25/0.75、0.15/0.40），
   且 `envelope` 是**私有 fn**（:89 无 `pub`）⇒ 常量无法从外部注入，
   **不变量的保护来自结构而非「目前恰好成立」**。
⇒ **模式 J（新增候选）**：休眠代码与休眠台账是**两件独立的事**——
本次连续三批（B0039 台账 / B0040 不变量 / B0042+B0043 本体）核的是前者，
而台账**只覆盖了「谁不调用」，没有覆盖「被留下的那些代码自身是否仍然正确」**。
B0042 的 `LlmTool` 与 B0043 的这三条都是台账视角看不到的东西。

---

## BATCH-0045（2026-09-28）· 渲染面：红线 O（30Hz 口型）的**接收端** —— 端到端验证通过

> **0 条新发现。** 前面 B0023 核的是 Dart 侧**发送端**（30ms 轮询 + 33ms 二次节流 +
> `GlobalKey → Bridge → postMessage`，不进 Widget 树）。本批核**接收端**，两侧合起来红线 O 才成立。

### 核验一：高频通道的**架构分工正确** —— 消息只改状态，帧循环才采样
接收端消息处理（`l2d-wasm-demo/src/main.rs:491-501`）只做一件事：
```rust
"audio-volume" => {
    // v1 口型：`mouth.payload.level`；旧协议 `audio-volume.value` 兼容保留。
    let value = payload.get("level").or_else(|| payload.get("value")).and_then(|x| x.as_f64());
    if let Some(vol) = value { st.bridge.volume = (vol as f32).clamp(0.0, 1.0); }   // ← 纯字段写
    applied = true;
}
```
**零渲染工作、零分配、零纹理操作**。真正落到参数上的是每帧调用的
`apply_bridge_effects`（`web/surface/input.rs:151-196`，由 `tick` 在 `core.update` 之后、渲染之前调）：
`let _ = st.core.set_parameter("ParamMouthOpenY", mouth);`（:196）—— **参数写，不是纹理重建**。
⇒ 30Hz 通道在 wasm 侧同样是「消息入状态 / 帧循环出参数」，
与 Dart 侧「30ms 轮询 / 帧外上传」同构。**两侧都没有把高频数据塞进重建路径。**

### 核验二：口型衰减**与帧率无关**（且把推导写下来了）
`input.rs:164-174`：`volume_display = volume.max(volume_display * exp(-dt_ms / TAU_MS))`，
注释给出两档实测推导：「60fps: exp(-16.67/90) ≈ 0.831 / 240fps: exp(-4.17/90) ≈ 0.955 /
二者单帧衰减不同，但*速率*相同——**这是正确的物理行为**」。
`dt_millis` 由 `tick` 传入真实帧间隔（:154-155 的理由：「避免高帧率（如 240fps）下口型/动作
『动得飞快』」）。
⇒ 高刷屏与 60Hz 屏上口型节奏一致。这是个容易写错、而他们写对了并留下推导的地方。

### 核验三：dB 映射替换了旧门限，且**记了被替换的实测原因**
`input.rs:187-192`：
```rust
/// **2026-09-10 修复**：旧实现把原始 RMS 直接写进参数（且只在 >0.01 时写），
/// 实测真实语音 RMS 中位数仅 ~0.03 → 嘴几乎不动。dB 映射见 [mouth_open_from_level]；
/// 静音（低于 -36 dBFS）自然归零，故不再需要旧的门限过滤（那个门限还会让轻声段落整段闭嘴）。
```
⇒ 一次**由实测驱动的**修正，且把「为什么旧方案错」与「为什么旧补丁不再需要」都留在原地。
这条与 B0019 记录的 Rust 侧「空末块也要发边界帧」是同一类工程记录密度。

### 核验四：`lipSync` 与「静音」是**两条独立的正交通路**（红线「与口型正交」）
- 渲染面：`st.bridge.lip_sync.then(|| st.bridge.volume_display)`（input.rs:176）⇒
  用户关掉口型 ⇒ 真的写 0（不是「忽略消息」）。
- 静音：B0006 已核 `ws/audio.rs:119-124` 静音时 PCM 置零但 **`volume` 仍按原始样本算**
  ⇒ **静音时嘴仍在动**。
⇒ 「静音/音量落在媒体元素上，**与口型正交**」这条 AGENTS.md 红线，
在**服务端（B0006）+ 渲染面（本批）** 两侧都成立；再叠加用户自己的 `lipSync` 开关，三者互不干扰。

### 核验五：接收端的协议面**向后兼容且不整条拒收**
`main.rs:391-405`：v1 信封 `{version,type,payload}`，**无 `payload` 对象时回退历史扁平消息**；
类型映射 `sync`/`stage` → `stage-config`、`mouth` → `audio-volume`（旧类型原样透传）。
逐字段**钳位而非拒整条**（:426-474），理由写在注释里：「非法值同样回落（`normalize_stage_color`
返回 None），**不拒整条消息**——其余字段照常生效」；`mouthSensitivity` 还额外挡了非有限值
（`s.is_finite()`，:444）防 NaN 渗进参数。
⇒ 符合红线 Q「契约只增不改」：新字段加在可选位置，旧消息原样工作。

---

## BATCH-0046（2026-09-28）· ⭐ **本审计至今最重的一条：postMessage 的 origin 校验只做了一半**

### F-0046-01 · P1 【**已撤回（B0275）** —— 我引用的代码**已不在树上**，而幸存的那一侧用的是 `location.origin`】
> ✅ **B0368 结清（2026-09-29）**：**接收端在 `shell/flutter/lib/live2d/live2d_host_web.dart`**
> （Flutter 宿主，**不是 wasm crate、也不是 `l2d/src`** —— 后两者 B0367/本批均零命中）。
> **而它把本条想要的东西全做了**：
> ```dart
> /// 监听 window message 并**校验 `source == iframe.contentWindow` 且 `origin == location.origin`**。
> if (event.origin != web.window.location.origin) return;           // ① origin
> if (source == null || source != _iframe.contentWindow) return;   // ② source
> ```
> ⇒ ⇒⇒ **「任何页面都能往这个 iframe 发消息」在当前树上不成立**；且它是**双重身份校验**
> （**同源 + 同一窗口**）⇒ **比本条建议的还多一项**。⇒ ⇒ **本条不重开**
> （B0275 的撤回**结论正确**，**理由需补这一句**）。
> ⇒ 而 B0367/B0368 两批的实际产出：**把一个我一度想「重开」的 P1，用排除清单定位到了它真正的家，
> 并确认它已按最严的方式实现。**

> 🔻 **撤回理由（三次 grep 追到源头，机械可复跑）**：
> ```
> $ grep -rn "postMessage" crates/l2d-wasm-demo/src/ --include=*.rs
> web/surface/input.rs:43   /// 任何能 postMessage 到它的东西都能塞值进来…   ← **仅注释**
> web/surface/input.rs:61   /// iframe bridge 接收状态（P0-2a）：父页 postMessage… ← **仅注释**
> ⇒ **wasm 源码树里 `postMessage` 真实调用 = 0**
>
> $ git ls-files crates/l2d-wasm-demo/dist    ⇒（空）⇒ **`dist/` 未跟踪，是过期构建产物**
>   （`dist/*.js` 里的 `__wbg_postMessage` 属**产物**，按我的排除清单本就不审）
>
> ⭐ **幸存的那一侧用的是具体 origin，不是 `"*"`**：
> `shell/flutter/lib/live2d/live2d_host_web.dart:78`
> ```dart
> target.postMessage(json.toJS, **web.window.location.origin.toJS**);
> ```
> ⇒ ⇒ **我引用的 `main.rs:380-386`（无 origin 检查）与 `main.rs:705-707`
> （`parent.postMessage(&ack,"*")`）在当前树上都不存在**
> ⇒ **文件被拆过**（AGENTS rc.3 ②「`surface.rs` 1167→**24**」），ack 的发送已不在 wasm 侧
> ⇒ ⭐ **而我那条修法 ③ 建议调用的 `normalize_resource_ref`，在 wasm 树里也已是 0 命中**
> ⇒ ⇒ **我的发现与我的修法都锚在一个已经不存在的树版本上** ⇒ **永久撤回**
>
> ⚠ **不夸大**：**本条被撤回，是因为它的对象不存在了，不是因为问题被证明不存在。**
> 若将来 iframe 通道**重建**且再次用通配 targetOrigin，**同类风险会重新出现**
> ⇒ ⇒ ⚠🔻 **B0277 更正我上面这句话（我说重了）**
> 「**这一版没有这条通道**」**是错的** —— **通道在，只是形态变了**（B0274 规则第三次生效：
> **别让一个否定结果硬化成肯定断言**）：
> - **发送侧**（Flutter→iframe）：**仍用 `postMessage`**，但 `live2d_host_web.dart:78`
>   带的是 **`web.window.location.origin.toJS`**，**不是 `"*"`** ⇒ 我引用的通配已消失
> - **接收侧**（iframe→Rust）：是**版本化 JSON 消息的字段**——`input.rs:100-102`
>   「**`stage-bg.dataUrl`（自定义背景图 dataURL；None = 无背景图）**」⇒⇒ **通道存在**
> - **回程**（Rust→父页）：是 **`emit_event`**（`main.rs:151/618/675/682`），
>   **不再由 Rust 调 `postMessage`**
> ⇒ ⇒ **准确表述**：**撤掉的是「我引用的那段代码」与「Rust 侧直接调 postMessage」**，
> **不是「这条通道」**。
> **原文保留**在下方 B0046 段内供追溯。


file: crates/l2d-wasm-demo/src/main.rs:380-386（**接收侧无 origin 校验**）与 :705-707（**发送侧 `"*"`**）
摘录（接收侧：监听器直接吃 `ev.data()`，全程不碰 `ev.origin()` / `ev.source()`）:
```rust
let closure = Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
    // data() → JsValue → String → serde_json::Value（父页 R2 后 stringify）
    let Some(text) = ev.data().as_string() else { return; };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { return; };
    let Some(raw_kind) = v.get("type").and_then(|t| t.as_str()) else { return; };
```
摘录（发送侧 ACK 用通配 origin）:
```rust
if let Ok(Some(parent)) = win.parent() {
    let _ = parent.postMessage(&JsValue::from_str(&ack), "*");   // :706 —— 应为 location.origin
}
```
**对照：同一协议的另一侧做对了**（`shell/flutter/lib/live2d/live2d_host_web.dart`）:
```dart
/// iframe 传输：`postMessage(jsonString, location.origin)` 发送，      // :41
/// …                                                                  // :42
/// `origin == location.origin`。
if (event.origin != web.window.location.origin) return;             // :48 —— 接收侧校验 origin
…
target.postMessage(json.toJS, web.window.location.origin.toJS);      // :78 —— 发送侧钉死 origin
```
影响: **安全 / 跨源注入**。三条事实叠加才构成它，我逐一验过：
1. **无 origin 校验**：`grep -rn "origin" crates/l2d-wasm-demo --include=*.rs` 在整个 crate 里
   唯一命中是 `background.rs:212` 的 `wgpu::Origin3d::ZERO`（图形概念，无关）⇒ 接收侧**零校验**；
2. **无防嵌套头**：全仓 `grep -rn "X-Frame-Options|frame-ancestors|Content-Security-Policy"`
   **零命中**；资产响应只设 `Content-Type` + `Cache-Control`（`wasm_assets.rs:309-314`）
   ⇒ 任何页面都能把 `http://127.0.0.1:18080/render` 塞进 iframe；
3. **处理函数能触发任意 URL 抓取**：`sync.payload.model`（:407-415 解析）→
   `load_model_from_url(&win, &url)`（:602 / :665），**先抓取、后校验**。
⇒ **可利用路径**：任意网页 → `<iframe src="http://127.0.0.1:18080/render">` →
   `iframe.contentWindow.postMessage(JSON.stringify({type:"sync",payload:{model:"http://attacker/…"}}),"*")`
   → 渲染面用**受害者浏览器**去抓那个地址（盲抓：服务端不回 CORS 头，攻击者读不到响应体）。
   同一条通道还能 `stage-config` 改缩放/暗色/口型开关、`preset` 改动作、`destroy` 复位舞台。
**为什么不判 P0**：抓取是**盲**的（`mod.rs:11` 明确「不回 `Access-Control-Allow-Origin`」），
拿不到响应体；也没有把密钥或 `.env` 交给跨源调用方。**定 P1**（盲抓 + 舞台劫持 + loopback 服务的跨源注入面）。
建议: ① 接收侧在 `main.rs:381` 之后立刻加
   `if ev.origin() != web_sys::window().map(|w| w.location().origin()).unwrap_or_default() { return; }`
   （与 `live2d_host_web.dart:48` **逐字对称**）；② 发送侧把 `:706` 的 `"*"` 换成
   `web_sys::window().unwrap().location().origin()`；③ 资产层给 `/render`（以及 `/app/*`）
   加 `X-Frame-Options: SAMEORIGIN` 或 CSP `frame-ancestors 'self'` —— **这一条是纵深防御**，
   即使 ①② 漏了也能挡住整条路径。
验证: 读 l2d-wasm-demo/src/main.rs:378-390 / :600-620 / :689-709；
读 live2d_host_web.dart:41-48 / :78；
`grep -rn "X-Frame-Options|frame-ancestors|Content-Security-Policy" crates/` → 零命中；
`grep -rn "origin" crates/l2d-wasm-demo --include=*.rs` → 唯一命中是 wgpu 的 `Origin3d`
置信: 高（三条事实各有可复跑的 grep；协议两侧的对照是本仓自己的代码）
反证: (a) 试过论证「loopback + 非浏览器客户端所以攻击者够不着」——**够得着**：任何网页都能
   `<iframe>` 一个 loopback URL 并用 `contentWindow.postMessage` 往里发消息，这与「谁能连到端口」无关；
   浏览器的 Private Network Access 若强制部署会缓解，但 PNA 对 iframe 的落地被推迟，**不可作为依据**。
(b) 试过论证「iframe 里的页面拿不到 `contentWindow`」——**能拿到**：它自己创建的 iframe。
(c) 试过找测试兜住（§12.1）——`grep -rln "origin|postMessage" crates/l2d-wasm-demo/src/preset/tests/ crates/l2d-wasm-demo/tests/`
   **零命中**；全 crate 无任何 origin 相关断言 ⇒ **零覆盖**，删掉/加上这个校验都不会让任何测试变红。
(d) 试过论证「这是被豁免的设计」——`live2d_host_web.dart:41-43` 把「发送钉 origin + 接收校验 origin」
   写成了**该协议的契约**；wasm 侧只是**没实现属于它的那一半**，不构成豁免。

---

## BATCH-0047（2026-09-28）· F-0046-01 的可利用性自查 → **发现自身描述不完整，已改写（升级）**

> **本批 0 条新缺陷，但把 F-0046-01 改写了。** 按 §5 的反证纪律，可利用性自查的结果
> 不只是「定级维持」，而是「**我原来的影响段低估了可达性**」。原文保留，下方标出更正。

### 自查三问的答案
1. **`postMessage` 是否只有 `install_stage_bridge` 一个入口？** —— `main.rs:378-716` 是唯一的
   `add_event_listener_with_callback("message", …)` 注册点（`grep` 全 crate 只有一个
   `install_stage_bridge`）。所以 origin 缺口**集中在这一个入口**，无别处兜底。
2. **origin 校验是否在别处兜住？** —— 否。全 crate 唯一的 `origin` 命中是
   `background.rs:212` 的 `wgpu::Origin3d::ZERO`。
3. ⭐ **`load_model_from_url` 是否有 URL 白名单？** —— **没有**。
   `net::fetch_bytes`（`web/net.rs:20-32`）第一行就是
   ```rust
   pub(crate) async fn fetch_bytes(window: &Window, url: &str) -> Result<Vec<u8>, String> {
       let promise = window.fetch_with_str(url);      // :21 —— **裸 fetch，零校验**
   ```
   **无 scheme 检查、无 host 白名单、无同源判定。** 且 `load_model_from_url`
   （`main.rs:163-181`）在抓到 manifest 后还会 `net::resolve_relative(model_url, &rel)`
   **逐个再抓**（:171-175）⇒ 一次输入可触发 **1 + N 次** 任意 URL 抓取。

### ⭐ 更正：可达路径比我原文写的**更宽**——存在一条**根本不需要 postMessage** 的入口
`net.rs:38-42` 从**查询串**取模型地址，`main.rs:267` 在页面初始化时直接用它：
```rust
let model_url = net::model_url_from_query(&window);      // main.rs:267
```
⇒ **V1（最低门槛）**：`<iframe src="http://127.0.0.1:18080/render?model=http://attacker.internal:8080/x.model3.json">`
—— **攻击者不需要任何 JS、不需要 postMessage、不需要 origin 缺口**，一个 iframe 就让
受害者的浏览器去抓那个地址（并按其内容再抓 N 个相对资源）。
**V2（F-0046-01 原文那条）**：postMessage 无 origin 校验 ⇒ 任意页面可发任意消息类型
（`stage-config` 改缩放/暗色/口型开关、`preset` 改动作、`destroy` 复位舞台、`sync.model` 换模型）。
**共同使能条件**：全仓无 `X-Frame-Options` / `frame-ancestors` / CSP（B0046 已 grep 零命中）
⇒ 任何页面都能把 `/render` 塞进 iframe。

### 改写后的 F-0046-01（影响段）
**影响（更正）**：`/render` 有**两条**互相独立的跨源入口，都缺 origin/URL 校验：
- **V1 `?model=` 查询串**（main.rs:267 → net.rs:39 → net.rs:21 裸 fetch）——**零 JS 门槛**；
- **V2 postMessage 无 origin 校验**（main.rs:380-386）——给出完整消息控制权。
两者都以「无防嵌套响应头」为使能条件，共同后果是
**受害者浏览器被当作抓取代理（浏览器介导的 SSRF / beacon）**，外加舞台状态可被任意网页操纵。
**定级：P1 维持**，理由不变（抓取是盲的——服务端不回 CORS，攻击者读不到响应体；
无密钥外泄、无权限绕过）。**但影响面比原文宽**：原文只写了 V2，且把它当成唯一入口。
**反证补记**：我试过论证「V1 会被 `extract_refs`/`from_memory_map` 的解析失败挡住」——
**不成立**：抓取发生在 :163，解析在 :166/:182，**先抓后校验**，
所以「解析失败」只是攻击者没拿到模型，**请求已经发出去了**。

---

## BATCH-0048（2026-09-28）· CI 门禁审计 —— 「红线 K 没有任何构建期自动门禁」

### F-0048-01 · P2
file: .github/workflows/{pr-checks,nightly,flutter-checks,build-upload,release-build}.yml（全 5 个文件的 `run:` 步骤已穷举）
摘录（`flutter-checks.yml` 的**全部**执行步骤）:
```yaml
- name: Resolve dependencies
  run: flutter pub get
- name: Analyze
  run: flutter analyze
- name: Test
  run: flutter test
```
摘录（AGENTS.md 自己为 wasm 那半边写的缺口声明）:
> 「wasm 那条暂时只有本地门禁——**已知缺口**，写在这里而不是假装它被 CI 守着。」
影响: 红线 K（离线优先）**没有构建期门禁**。我把 5 个 workflow 的每一个 `run:`（含
`run: |` 多行块，`build-upload.yml:52/68/76/98` 逐块读过）列了一遍，结果：
- ✅ Rust 六道门禁齐全（`--all-targets` test / doc / fmt / clippy -D warnings / rust-ratio / msrv check）
- ✅ Dart 两道（`flutter analyze` + `flutter test`）
- ✅ 根 `pytest tests/`（且 `requirements-test.txt` **在树里**，该 job 可用——已核）
- ✅ E2E 探针**严格 opt-in**：`if: ${{ vars.LIVE2D_AI_VERIFY_BASE_URL != '' }}`（nightly.yml:74），
  未配置时 job 状态是 **skipped**（不是 passed）⇒ 「缺配置明确跳过，绝不伪造绿灯」这条
  **实现正确**，且 :62-70 写明了理由（硬跑会天天红，「天天红比没有这条更坏」），
  :88 还记了一次既往误判修正（`--timeout 300`，默认 90 会误判超时）
- ❌ **没有任何一步 `flutter build web`** ⇒ `--no-web-resources-cdn` 这个**唯一**决定
  CanvasKit 是否指向 gstatic 的开关，**在 CI 里从未被执行过**
- ❌ **没有任何一步 `trunk build`**（wasm 半边；AGENTS.md **已声明**为已知缺口）
- ❌ **没有任何一步跑 `./scripts/ignite.sh --check`** —— 而那正是唯一会去 grep **构建产物**
  （`index.html` / `main.dart.js`）里有没有 `gstatic.com` 的地方（AGENTS.md「点火体检」四条）
⇒ 唯一与红线 K 相关的自动化门禁是 `shell/flutter/test/font_subset_test.dart`（B0025 已审：
字体子集 + FNV-1a 哈希 + 词法器自测，真能失败），但它扫的是**源码里的字符串字面量**，
**看不见构建开关、也看不见产出的 HTML**。
**为什么定 P2 而不是 P1**：这不是「门禁被绕过」，是「门禁不存在」；而且项目对**同一类缺口的
另一半**（wasm）**已经明确声明**了，本条是**未声明的另一半**——不是隐瞒，是漏记。
但它确实是 AGENTS.md 纪律的直接落空：「缺了要写在这里而不是假装它被 CI 守着」。
建议: 二选一，**别让第三种（默默没有）存在**：
① **补门禁**（推荐，成本一条 step）：在 `flutter-checks.yml` 或 `nightly.yml` 加
   `flutter build web --release --base-href /app/ --no-web-resources-cdn` + 对产物 grep
   `gstatic.com`（或直接调 `./scripts/ignite.sh --check`）；
② **补声明**：在 AGENTS.md「本地必跑 vs CI 必跑」表下方加一行，说明「前端**产物级**离线体检
   （ignite.sh --check）**只有本地门禁**；CI 只跑 analyze+test，不构建产物」，
   与 wasm 那行并列。
验证: 逐个 workflow 列出全部 `run:` 步骤（`grep -n "run:" -A 2 .github/workflows/*.yml`）
并逐块读 `build-upload.yml:52/68/76/98` 与 `nightly.yml:128`；
`grep -rn "ignite|gstatic|flutter build|trunk build" .github/workflows/` → **零命中**；
`git ls-files | grep requirements` → `requirements-test.txt` **存在**（该 job 可运行）
置信: 高（穷举而非抽样；每条断言都有可复跑的 grep）
反证: (a) 试过论证「`build-upload.yml` 可能有前端构建」——逐块读过 4 个多行块，只有
系统依赖 / `cargo build --release` / 二进制元数据 / 产物上传 / 摘要，**无前端构建**；
(b) 试过论证「`font_subset_test.dart` 算红线 K 的门禁」——它管的是「源码文案不得用子集外字符」，
**与构建开关和产出 HTML 无关**，两者是不同的失效面（漏 `--no-web-resources-cdn` 时该测试照样全绿）；
(c) 试过论证「本地 ignite.sh --check 覆盖了」——**这正是本条**：本地覆盖 ≠ 门禁覆盖，
AGENTS.md 的口径是「**CI 必跑**」那一列，而那一列是空的。

---

## BATCH-0049（2026-09-28）· 本地门禁执行点 —— ⭐ **红线 R 的密钥扫描门禁整体缺失**

### F-0049-01 · P1
> 🧪 **B0272（Phase 4 证伪）攻它：对着当前树重核 ⇒ 两半都仍是事实；且**修法收窄到一句话**
> 攻法：若 CI 已经接上了、或那句自称已被删 ⇒ 本条该推翻
> ```
> scripts/check_public_secrets.py:5  「**GitHub CI 额外会跑 Gitleaks 覆盖完整历史**。」
> $ grep -rn "check_public_secrets|gitleaks|Gitleaks" .github/
> （**零命中**）
> ```
> ⇒ ⇒ **274 批之后**：「自称」仍在、**CI 里仍零命中**（既没调它，**也没有 Gitleaks**）
> ⇒ ⇒ **攻击失败**，两半都还是今天的��实
> ⇒ ⭐ **② 收窄（修法更紧）**：该脚本头注**自陈扫描范围**是
> 「Rust workspace（`crates/`, `xtask/`）+ 顶层配置/脚本/测试」并**跳过二进制**
> ⇒ ⇒ 所以「`INCLUDED_PREFIXES` 没有 `shell/`」**不是列表的 bug，是它的范围选择**
> ⇒ ⇒ 而**覆盖 `shell/` 的正是那句 Gitleaks 承诺**，而那句话**不成立**
> ⇒ ⇒ **所以真正的缺陷就是一句话** ⇒ ⇒ **修法只剩两条路**：
> ① **加一个真的 Gitleaks 步骤**（兑现承诺）② **或删掉那句话**（并说明 `shell/` 不在范围内）
> ⇒ ⇒ 而**不是**「去补 `INCLUDED_PREFIXES`」—— 那会**悄悄改变脚本的自陈范围**，比原状更糟
file: scripts/check_public_secrets.py:5（**不成立的自我声明**）与 :15-31（**扫描范围**）
摘录:
```python
"""Conservative secret-pattern scan for the current repository's tracked tree.

Rust 重构后只扫描 Rust workspace（crates/, xtask/）+ 顶层配置/脚本/测试，跳过
二进制、压缩包、锁文件。**GitHub CI 额外会跑 Gitleaks 覆盖完整历史。**     ← :5
```
```python
INCLUDED_PREFIXES = ("crates/", "xtask/", "shared/", "tests/", "scripts/")   # :15-21
INCLUDED_TOP_FILES = {"Cargo.toml","Cargo.lock","rustfmt.toml",".gitignore",
                      "README.md","AGENTS.md","ANDROID_ARCHIVE_POINTER.md"}  # :23-31
```
影响: **红线 R（密钥不进仓库）在这套 CI 里既没有主扫描器，也没有它自己声称的补偿扫描器。**
三条事实叠加，每条都可复跑：
1. **该脚本在 CI 里从未被调用**：`grep -rn "check_public_secrets" .github/ scripts/ xtask/ Cargo.toml`
   → **零命中**（5 个 workflow 的全部 `run:` 步骤已在 B0048 穷举过，无一步调它；
   AGENTS.md 的「本地必跑 vs CI 必跑」表里**也没有列它** ⇒ 它连「本地必跑」都不是，只是存在）；
2. **它声称的补偿控制不存在**：`grep -rni "gitleaks" .github/ scripts/` 的**唯一**命中
   就是 `check_public_secrets.py:5` 这句话本身 ⇒ **CI 里没有任何 Gitleaks 配置**；
3. **它的扫描范围不含前端与文档**：`INCLUDED_PREFIXES` 无 `shell/`（程序化确认 `False`）
   ⇒ 217 个 Dart 文件与全部 `docs/**`、`.github/**` 都不在扫描面内。
⇒ **净效果**：一个 `sk-…` 被误粘进 `.dart` 文件或某篇文档，可以直接进仓库而**没有任何自动化会响**。
这与本项目的密钥架构直接冲突：`.env` 是唯一真源、密钥不入 GET/日志/WS/导出（B0007/B0017/B0019 已核）
——整套纪律的最后一环「**别让它进版本库**」恰恰是唯一没有门禁的一环。
**为什么判 P1**：不只是「缺门禁」，而是**门禁的自我说明与事实相反**——它声称有一个 CI 兜底，
而那个兜底不存在。这会让审计者/维护者**据此以为这条红线有网**（我在 B0048 之前正是这么相信的）。
**次生问题**（同类形态，附带记录）：即便脚本被跑起来，它的**行级豁免**也偏宽——
`_is_test_line`（:64-65）用 `marker.lower() in line.lower()`，标记含 `"INJECTED"`（**大小写不敏感**），
而本仓测试里确实在用 `sk-…-INJECTED` 这类种子（见 `web_api/dto.rs` 的 `env_with_keys`）。
⇒ 任何与 "injected" 同行的真密钥会被跳过。与 F-0006-01（`logging::redact` 只认键名）同形。
建议: ① **先修声明**：:5 删掉「GitHub CI 额外会跑 Gitleaks」（它不存在），
   改成「**本脚本当前不由任何门禁调用，是手工工具**」——不留一句不成立的话；
   ② **接进门禁**：在 `pr-checks.yml` 的 `root-py-tests` 之后加一步
   `python3 scripts/check_public_secrets.py`（或纳入 `tests/` 一起跑）；
   ③ **补扫描面**：把 `shell/flutter/lib/**`（或整个 `shell/flutter/**`，排除 `build/`）
   与 `.github/` 加进 `INCLUDED_PREFIXES`；若担心误报，用行级「排除测试标记」而非目录级排除；
   ④ 真要 Gitleaks：加一个 `.github/workflows/secret-scan.yml`（`gitleaks/gitleaks-action`），
   然后 :5 那句话才成立；⑤ 收窄行级豁免：把 `INJECTED` 改成**整串**匹配
   （如 `sk-.*-INJECTED`）而不是「行内含 injected 就跳过」。
验证: 读 scripts/check_public_secrets.py 全文件（108 行）；
`grep -rni "gitleaks" .github/ scripts/` → 唯一命中是 docstring 自身；
`grep -rn "check_public_secrets" .github/ scripts/ xtask/ Cargo.toml` → 零命中；
程序化确认 `INCLUDED_PREFIXES` 不含 `shell/`
置信: 高（四条断言均为可复跑的 grep / 程序化检查）
反证: (a) 试过论证「AGENTS.md 声称它在 CI 里」——**没有**；AGENTS.md 的对照表里**没有这一行**，
所以不构成「文档骗人」，骗人的是**脚本自己的 docstring**。我据此把措辞限定在脚本上。
(b) 试过论证「GitHub 可能有仓库级 secret scanning（不是 workflow）」——那是**平台功能**，
默认只扫 push 事件、不扫历史、且不阻止提交；它不能替代「提交前拦截」，
也不能让 docstring 那句「额外会跑 Gitleaks 覆盖完整历史」变真。**未核实**：该仓库是否在
GitHub 侧开了 secret scanning / push protection（属仓库设置，不在代码里）——
如实标注，这不影响「代码内无门禁」这一结论。
(c) 试过论证「`sk-` 模式太窄，实际泄露形态更多」——对，本条只覆盖这 4 类模式；
这让「无门禁」的后果更重（连这 4 类都没有网），但不影响结论。

---

## BATCH-0050（2026-09-28）· ⭐ **`ignite.sh --check` 的探针文件选错了 —— 红线 K 连本地网都没有**

### F-0050-01 · P2
file: scripts/ignite.sh:89（探针文件清单）+ :94（探针模式）
摘录:
```bash
# CDN 依赖只看**服务实际吐出的字节**（与浏览器拿到的一致），不看本地文件。
for f in index.html main.dart.js; do                                   # :89 ← 只探这两个
  fcode=$(curl -s -o /dev/null -w "%{http_code}" --max-time 15 "$base/app/$f" …)
  if [ "$fcode" != "200" ]; then … fail=1
  elif curl -s --max-time 15 "$base/app/$f" … | grep -q 'gstatic\.com/flutter-canvaskit'; then   # :94
```
**实测（对本仓库现有构建产物，用门禁那条**一模一样的**模式）**:
```
index.html                   命中 0   ← 门禁探
main.dart.js                 命中 0   ← 门禁探
flutter_bootstrap.js         命中 1   ← 门禁**不**探
flutter.js                   命中 1   ← 门禁**不**探
flutter_service_worker.js    命中 0
```
而**活的 CDN 配置就在未探的那个文件里**（`flutter_bootstrap.js`）：
```js
canvasKitBaseUrl ? n.canvasKitBaseUrl
                 : e.engineRevision && !e.useLocalCanvasKit
                   ? W("https://www.gstatic.com/flutter-canvaskit", e.engineRe…
```
影响: 红线 K（离线优先）**既没有 CI 网、也没有可靠的本地图**。`ignite.sh --check` 的
**探针模式是对的**（`gstatic\.com/flutter-canvaskit` 精确匹配真实失效形态）、
**探针层次也是对的**（`curl` 服务实吐字节，先查 HTTP 200 再 grep —— 这避开了
「404 空 body 被当成无外链」的经典假绿），但**探针文件清单选错了**：
真正承载 `canvasKitBaseUrl` / `useLocalCanvasKit` 决策的是 `flutter_bootstrap.js`，
而它不在清单里。
**当前构建是安全的**（`--no-web-resources-cdn` 置 `useLocalCanvasKit` ⇒ 那处 gstatic 是
**死分支**，见上面那个三元式）——所以**本条不是「产品现在是坏的」**，
而是「**这条门禁抓不住它被写出来要抓的那个故障**」：
有人漏掉 `--no-web-resources-cdn` 重新构建后，被探的两个文件很可能仍是 0 命中 ⇒ 体检报
「不依赖 Google CDN」⇒ **断网白屏**。
**为什么定 P2 而不是 P1**：无法在只读约束下构造一个「坏构建」来实证那次假绿
（本任务禁跑 `flutter build`）；我能确证的是**探针集与字符串所在文件不重叠**这一结构事实，
以及「活的配置分支在被漏掉的文件里」。这个结构足以判定门禁不可靠，但不足以断言
「下一次漏标志时它一定绿」。**如实标注这条边界。**
建议: ① 把 `flutter_bootstrap.js` 加进 :89 的清单（`flutter.js` 建议一并加，它是同一份 loader）；
② 更稳的做法：**在构建期断言**而不是在启动期 grep —— 即让 `--no-web-resources-cdn`
的构建把 `useLocalCanvasKit` 写成 true，然后门禁 grep 的是**这个标志**
（`grep -q 'useLocalCanvasKit:!0\|useLocalCanvasKit:!1'`）而不是 CDN 字符串本身，
这样「有没有落到 CDN 分支」就是直接可判的，而不是靠字符串是否出现来间接推断。
③ 顺带：F-0048-01 的修法①（把 `ignite.sh --check` 接进 CI）**必须先做本条的①**，
否则等于把一个探错文件的门禁自动化。
验证: 读 scripts/ignite.sh:88-101；用门禁的精确模式逐文件实测真实产物
（`for f in index.html main.dart.js flutter_bootstrap.js flutter.js …; do grep -c 'gstatic\.com/flutter-canvaskit' …; done`）；
读 `flutter_bootstrap.js` 中 `canvasKitBaseUrl` 的三元式确认活配置位置
置信: 高（结构事实：探针集 ∩ 字符串所在文件 = ∅；且「活分支在被漏文件里」有源码原文）
反证: (a) 试过论证「main.dart.js 里也有 gstatic，门禁照样能抓」——`grep` 计数明确是 **0**
（该文件确实含 `gstatic` 字样，但**不含**门禁那条精确模式 `gstatic\.com/flutter-canvaskit`，
所以门禁的 `grep -q` 不会命中）；(b) 试过论证「现在的构建是坏的」——**不是**，三元式的
`!e.useLocalCanvasKit` 守卫使 CDN 分支为死代码，本条的指控是「门禁抓不住」不是「现在是坏的」；
(c) 试过论证「Flutter 版本变了、配置不再落在 bootstrap.js」——本条**正是**该风险的对冲：
无论落在哪个文件，**把候选文件列全**（`flutter_bootstrap.js` / `flutter.js` / `index.html` /
`main.dart.js`）或改用标志位断言，都不会因上游重构而失效。

---

## BATCH-0051 —— **自我更正 F-0040-01①**（AGENTS.md 内部不一致，不是「整体未更新」）

我在 B0040 写「AGENTS.md 点名 `mod_count_is_three`，全仓零命中 ⇒ 台账点的锚点已不存在」。
本批 grep 到 AGENTS.md 的**另外两处已经更新**：
```
AGENTS.md:105  `mod_count_is_three` → `mod_count_is_five`；**缺省仍只启用 `external-input`**
AGENTS.md:608 （追加 `voice-input` / `wallpaper`），`mod_count_is_three` → `mod_count_is_five`，
```
⇒ **更正**：不是「AGENTS.md 通篇没跟上」，而是 **AGENTS.md 自身不一致** ——
「变更历史 / 目录约定」两处跟上了（`_three` → `_five`），**「休眠台账」那处没跟上**。
仓内三处的实际状态：`main.rs:471` 测试名 `mod_count_is_five`（断言 5）·
`scripts/ignition-precheck.sh:156` 文案「**五个已注册**」· AGENTS.md 休眠台账仍写 `_three`。
**影响**：我原文的指控（「台账点的锚点已不存在」）**过宽**——锚点在同一文件里另两处是对的。
准确的说法是：**一份文档内部有两个版本的同一事实，而读者按台账那一节去找会扑空**。
严重性不变（P3，可维护性），但**范围收窄到「单节未更新」**。按 §5 纪律显式改写，不静默。

---

## BATCH-0051 —— F-0050-01 **升级为模式**（第二处同形）

`scripts/ignition-precheck.sh:134` 复制了**同一份错误探针清单**：
```bash
for f in index.html main.dart.js; do            # :134 —— 与 ignite.sh:89 逐字相同
  …
if curl -s --max-time 15 "$BASE/app/main.dart.js" … | grep -q 'gstatic\.com/flutter-canvaskit'; then   # :138
```
而且**比 ignite.sh 更弱**：它只 grep **一个**文件（`main.dart.js`），
而那正是用门禁模式实测命中 **0** 的两个文件之一。
⇒ 「`for f in index.html main.dart.js`」这句在仓内出现 **2 次**（`ignite.sh:89`、
`ignition-precheck.sh:134`），**两处都漏掉 `flutter_bootstrap.js`（活配置所在处）**。
**F-0050-01 由「一处探针选错」升级为「同一错误探针被复制到两处」** —— 说明它不是笔误，
而是一个**被复制的错误假设**（「CDN 引用只会出现在 index.html / main.dart.js 里」）。
建议把「候选文件清单」提取成一处共享定义（两个脚本共用），
或直接改用**标志位断言**（`useLocalCanvasKit`），使错误无法被复制第二次。
顺带记一条小的：`:138-141` 两个分支的 `record` **标签文字**都是
「main.dart.js 不依赖 Google CDN」（FAIL 分支也这么写），逻辑正确但标签与语义相反，
排障时容易看反——**仅文字问题**，记此备忘不入发现。

---

## BATCH-0050 —— 对 F-0048-01 的**影响升级**（显式改写，不静默）
F-0048-01 原写：「红线 K 没有构建期门禁；唯一能 grep 构建产物的是 `ignite.sh --check`」。
本批证明**那唯一的一层也是探错文件的** ⇒ 修正为：
**红线 K 目前没有任何可用的自动化防护**（CI 无 + 本地门禁不可靠）。
F-0048-01 的定级 P2 不变，但其「影响」段须与 F-0050-01 合并阅读。

---

## BATCH-0053（2026-09-28）· E2E 探针 —— 「绝不伪造绿灯」在脚本侧**成立**（门禁轴最后一块拼图）

> **0 条新发现。** 任务书 §2 点名要核「E2E 探针是否伪造绿灯」——核完的结论是**它没有**，
> 而且它是本审计读到的**最严谨的一段探针代码**。

### 核验一：退出码由「有没有失败跳」决定，**没有**任何一条路径能吞掉失败
```python
rep.dump()
return 0 if not rep.failed else 1        # :443
```
`rep.failed` 汇总**全部** `rep.ok` / `rep.fail`（:69-77），任一跳失败 ⇒ 非零 ⇒ CI 红。**没有**「部分失败仍返回 0」的分支。

### 核验二：**超时与真失败被明确区分**，超时判**失败**而不是「跳过」
收帧循环（:317-352）有两个出口：收到 `turn_state`（:350-352 `break`）或到 deadline（:317）。
若到 deadline 仍未收到 `turn_state` ⇒ `turn_status is None` ⇒
```python
rep.fail("本轮已收口", "超时未收到 turn_state（前端输入框会锁死）")   # :434-435
```
⇒ **超时不会「反正没报错就算过」**，而是**判失败**并写明用户可见后果。

### 核验三：三条「必查项」都**显式禁止静默跳过**
- **采样率**（:383-389）：注释原话「**缺字段也要判失败，不允许静默跳过——『跳过』等于假装验过了**」；
  且与 TTS 配置不符即失败（:392-396），理由写明「非整数倍会逐片重采样 → 咔哒声」。
- **口型电平源**（:427-430）：`audio` 帧缺 `data.volume` 即失败。
- **句界配对**（:405-425）：从**线上帧**统计 `starts`/`ends`，要求
  `starts == ends and starts >= 1 and first_is_start and last_is_end`；
  注释还记了旧实现的病症（`start` 轮级 / `end` 末片，两者不对称 ⇒ 前端攒错句）。

### 核验四（意外收获）：它把「休眠台账」写成了**可执行断言**
```python
if action_frames == 0:
    rep.ok("无动作投影", "全程 0 个 action_state（LLM 纯对话）")      # :437-438
else:
    rep.fail("无动作投影", f"出现 {action_frames} 个 action_state —— 动作系统又接线了")   # :440
```
`action_state` 帧型在 rc.2 已被删除（我在 B0003 核过 `ws/events.rs:28-30`）。
该探针把期望**反转**成「必须为 0」，于是**一旦有人把动作系统重新接线，探针立刻红**。
⇒ 这是「休眠台账 → 可执行断言」的**正面样本**，与 F-0040-01 那个
「AGENTS.md 点名 `mod_count_is_three` 而真名是 `mod_count_is_five`」形成**直接对照**：
**同一件事，一个做成了可执行断言，一个只写在文档里且已经漂了。**

### 我试过推翻而没推翻的（记此以免下批重走）
- 「失败被 try/except 吞掉」——:355 的 `finally` 只做 `ws.close()`，**不**改变判定；
  `http_get` / `http_post_json` 的 `except`（:102 / :124）把异常归为「不可达」并交给上层判失败。
- 「`boundary_only_frames` 被当成通过条件」——:374-376 明写它是**诊断不是判据**，
  且**不**参与 `return 0 if not rep.failed`（它只调 `rep.ok`，不调 `rep.fail`）。
- 「`LLM 有正文` 只要有字就算过」——它要求 `joined` 非空（:359），
  且「TTS 有产物」「音频有本体」是**独立**的两跳（:364 / :369），不会因为正文有字就放行音频。

---

## BATCH-0055（2026-09-28）· ⭐ **红线 N（舞台保活）正向验证通过 —— 13 条红线全部结清**

> **0 条新发现。** 这是本审计**至今唯一一条还没做成正/反向验证的红线**，本批补上了。
> 它的风险点不是「重挂会不会发生」（错误恢复路径必然重挂），而是
> **「重挂后宿主的状态有没有被重发」** 与 **「宿主会不会在渲染面还没准备好时就下发」**。

### 核验一：渲染面侧的启动时序是**刻意排序**的，且注释写明了理由
`crates/l2d-wasm-demo/src/main.rs:338-343`：
```rust
install_stage_bridge(&state);        // :338  先装 message listener
install_stage_interaction(&state);   // :339  再装输入监听
// **协议 v1**：listener 就绪 → `ready`（宿主可开始下发）；模型已进核心 → `loaded`。
emit_event("ready",  serde_json::json!({ "protocol": true }));   // :341
emit_event("loaded", serde_json::json!({ "url": model_url }));   // :342
start_render_loop(state);           // :343  rAF 循环**最后**才开
```
**`ready` 发在 listener 装好之后** ⇒ 宿主一看到 `ready` 就可以开始下发，
**不存在「宿主第一条消息早于监听器到达」的竞态**。这正是红线 N 需要的时序保证。

### 核验二：宿主侧的重发是**无条件**的，且两侧口径对得上
- 宿主端（B0030 已核）：`live2d_stage.dart:120-123` 明写
  「iframe 是**可重建**的（首帧前入队、错误后 retry、热更新重挂）…现在 `_attach`
  **每次挂桥都重发一次**」；`app_shell.dart:1207` 把 `onReady` 接到 `_applyPrefs`。
- 渲染面端（本批）：重挂 = 一段全新的 init 流程 → `ready` → 宿主在 `onReady` 里重发。
- 两者衔接一致 ⇒ **重挂后舞台状态（含 `stage-bg`）会被重建，不是靠残留**。

### 核验三：换模型路径也发 `loaded`，不留「换完了但宿主不知道」
`main.rs:670`（`sync.payload.model` 换模成功）与 `:342`（首次加载）**走同一个 `loaded` 事件**
（:609-611 的 `load-model` 旧路径亦然）；失败则发 `error` 事件（:616-619 / :674-677），
**不**发 `loaded` ⇒ 宿主不会把失败的换模误认成成功。

⇒ **红线 N 判定：✔ 通过（正向）**。至此任务书 §6 的 13 条红线**全部完成判定**（见 STATE 台账）。

---

## BATCH-0056（2026-09-28）· 红线 P 的**渲染侧落点** —— `FieldRuntime` 核验通过

> **0 条新发现。** B0015 只核到 `accept_cue` 的**调用点**（`engine.rs:522`），本批核**它内部**，
> 以及该文件头注声明的整套协议语义。

### 核验一：`accept_cue` 的状态机有三个关键性质（field_runtime.rs:526+）
1. **跨 epoch 结束旧动画段**（D26，:530-539）：
   `if cue.has_epoch && (!self.has_epoch || cue.epoch != self.epoch) { for field in Field::ALL { self.clear_field(...) } self.epoch = …; self.prev = None; self.clock.reset_closed(); }`
   ⇒ **新代次到达时把**所有**字段的贡献清空**，与 Dart 侧 `audioEpochChanged`
   （B0045）、Rust 侧 `epoch_gate`（B0038）**三层同口径**。
2. **expression 的 `none` 撤销哨兵**（:542-549）保持 V11「语义不变」，且仍写 `prev`
   ⇒ 后续 `after_prev` 锚点**仍能解析**（不被撤销动作打断）。
3. **未知 / 缺失 expression id 不静默**（:551-565）：
   `events.push(AckEvent::dropped_from_cue(epoch, now, &cue, reason))`，
   reason 区分 `expression_id_missing` / `expression_unknown_id`，注释写明「**不得静默**」。
   ⇒ 与 desktop 侧（`main.rs:524-532`「非法 cue **不得静默**」）**两端同纪律**。

### 核验二：时钟切换时 `dt` 归零，而不是硬算出一个巨大的跳变（:788-804）
```rust
let domain = self.clock.domain();
let dt = match self.last_now_ms { Some(prev) if self.last_domain == domain => (now - prev).max(0.0), _ => 0.0 };
```
`ClockDomain` 切换（墙钟 ⇄ 音频时钟）时 `dt` 取 0 ⇒ 域切换不会把 `elapsed_ms` 推成垃圾值。
这与头注 :24-25 声明的语义一致：「`stage-clock` 到位后用**音频时间轴**当 `now_ms`；
没有 clock（段 A）回落墙钟（§6.1 / §6.2 / O6）」。

### 核验三（本批最有价值的对比）：该文件的**行数豁免声明与事实相符**，而 F-0004-02 的不相符
`field_runtime.rs:3-8` 的豁免头注：
```rust
//! **行数豁免（≤1000 头注）**：本文件当前 953 行，超过 AGENTS「源码 ≤500」的常规线。
//! 豁免理由是**语义不可拆**：…「先加后钳」（C3）要求累加与钳位在同一函数内可读…
//! 本文件保持在 1000 行以内（AGENTS 的豁免上限）。
```
实测 **953 行**、上限 1000 ⇒ **声明与事实一致**。
**对照 F-0004-02**（`chat_routes.rs` 实测 **1086 行**，已越过 1000 上限，而其头注仍写
「本文件当前**约 560 行**」）⇒ 同一份仓库里，**两种截然不同的记账卫生**。
⇒ **可提炼的判据**：凡是**在写下的那一刻核对过数字**的注释（`field_runtime.rs:3`、
`web_api/mod.rs` 的行数说明）至今都真；凡是**跨越多轮改动而没人回头核**的
（`chat_routes.rs:52`、AGENTS.md 的 `mod_count_is_three`）都已漂。
**台账腐烂的机制不是「写错」，是「没有人在改动的同一次里重新核对」。**

### 核验四：纯逻辑位置的纪律被写成了理由，且点名了两个历史受害者
`field_runtime.rs:10-14`：「本仓反复踩过同一个坑：把纯逻辑写在 wasm 门控后面 =
原生 `cargo test` 编译不到 = **没有回归**（`mouth.rs` / `stage_bg.rs`）…
wasm 侧只做『收消息 → 调这里 → 发事件』。」
⇒ 与 B0033 核到的同一条纪律（`relativeTime` 放纯逻辑层而非 import Flutter 的 UI 层）
**是同一条**，且这次给出了**踩坑记录与受害者文件名**。

---

## BATCH-0058（2026-09-28）· 帧循环实体 + 换模型的资源侧（红线 N 资源半边）

> **0 条新发现。** 补上前几批留下的两个空：帧循环 `tick` 的实体、以及**换模型时旧资源是否释放**。

### 核验一：`tick` 对「切后台回来」的巨步有**双重**防护，且失败即停循环
`web/surface/render.rs:245-273`：
```rust
let real_dt = match st.hud.last_frame_ms {
    Some(prev) if now_ms > prev => ((now_ms - prev) / 1000.0).clamp(1.0/240.0, 1.0/15.0),  // :260
    _ => FIXED_DT_60HZ as f64,
};
…
if let Err(e) = st.core.update(real_dt as f32) {          // :266
    drop(st);                                             // :267 ← 先放掉 RefCell 借用
    status(&format!("停止渲染循环：update({real_dt:.4}s) 失败：{e}"));
    return;                                               // :269 ← 不在坏状态上空转
}
```
- **dt 钳到 [1/240, 1/15] s**，注释写明用途：「防巨步（**切后台回来**）与零步」——
  这正是浏览器标签页切换回来时的真实故障模式；
- 与 B0054 核过的 `pose_stack::update` 的 `InvalidDt` 拒绝**构成双保险**（一个钳、一个拒）；
- `:267` 的 `drop(st)` 先释放 `RefCell` 借用再调 `status()`（该函数也要 borrow）——
  **顺序错了会 panic**，此处是对的。

### 核验二：帧循环记录了**一次被推翻的方案**及其实测理由
`:246-254`：「**P3.1 修正（R4 撤销）**：R4 的 60Hz accumulator 修好了『240Hz 四倍速』，
但引入了『**姿态每 4 帧跳一次**』的视觉步进…因此**直接用真实 dt 更新**」并给出依据
（`PhysicsEngine::update` 原生支持任意 dt、内部按 min_fps 拆分 ticks）。
⇒ 这是本审计见到的**第三处「记录推翻过的方案 + 实测理由」**的样本
（前两处：WASM 渲染面的 `stage_css`、字体子集的 `▍`/`⌘`）。

### 核验三：换模型时**旧姿态栈被整体替换**（即被 drop）
`crates/l2d/src/renderer/model_core.rs:84-99`：
```rust
self.renderer.load_model(loaded.handle().shared(), &texrefs)…   // :87-89 新资源交给渲染器
self.stack = PoseStack::from_loaded_model(loaded)…             // :91-92 赋值替换 ⇒ 旧的被 drop
self.transform = layout_transform(loaded);                     // :93
let initial = self.stack.finalize();                            // :96 确定性初始姿态
self.renderer.driver().apply_pose(&initial);                   // :97
```
`stack` 的**赋值替换**意味着旧 `PoseStack`（含其**有状态**的 `physics_work` 与
`Option<PhysicsEngine>`）在换模型时被释放 —— 这是正确性要求（旧物理引擎的内部状态
指向旧模型的参数）。
`l2d/src/model.rs` 的 `ModelHandle` 只是 `Arc<ParsedModel>`（:69-71）**无自定义 `Drop`**
⇒ CPU 侧纯引用计数，无需手工释放（这是对的，不是遗漏）。

**审计边界如实标注**：**纹理 / wgpu 资源**由 `self.renderer.load_model(...)` 内部处理，
落在**上游 crate `ayagami`**里，**不在本仓审计范围内**。本仓这一侧能核的（姿态栈替换 +
handle 无需手工 drop）**都核过且正确**；渲染器那一侧**不作断言**。

---

## BATCH-0060（2026-09-28）· ⭐ **换根后的第一个发现 —— Mod→settings 信任边界「失败开放」**

> 这是**从未被任何一轮审计追过的根**（8 个 Mod crate / 24,665 行）上的第一条发现，
> 也是 CONSOLIDATION-07 留下的那个待检验问题（「尚未出现某全新区域第一次被审就出 P1/P2」）的**第一个答案：有**。

### F-0060-01 · P2
file: crates/live2d-ai-mod-system/src/services.rs:158-160（**契约声明**）
摘录:
```rust
/// **边界**：patch 是 `live2d-ai.toml` 的 namespaced JSON 形态（如
/// `{"llm":{"base_url":"..."}}`）；Mod **不得**借此写主链语义之外的键
/// （**写不存在的键由 host 的 SettingsPatch 反序列化拒绝**）。
```
而**实现里没有任何拒绝未知键的机制**：
```rust
// crates/live2d-ai-desktop/src/web_api/cli_entry.rs:188-200（生产接线的唯一实现）
apply_settings: Arc::new(move |patch_json: serde_json::Value| {
    let patch: SettingsPatch = match serde_json::from_value(patch_json) {
        Ok(p) => p,                                   // ← 未知键在这一步被 serde **静默丢弃**
        Err(e) => { tracing::error!(…); return false; }   // 只有「类型错」才拒绝
    };
    …
```
`grep -rn "deny_unknown_fields" crates/live2d-ai-runtime/src/settings/patch.rs
crates/live2d-ai-desktop/src/web_api/settings_routes/mod.rs` → **零命中**
⇒ `SettingsPatch` / `LlmPatch` / `TtsPatch` **一律按 serde 默认行为忽略未知键**。
调用链（Mod → 宿主配置，完整）:
`ModServices.apply_settings.apply(patch)`（services.rs:175）→
`mod_registry.rs:367-375` 的 `ModSettingsApplier` 闭包（顺手 `tracing::info!` 打印**整个 patch**）→
`HostChannels.apply_settings`（mod_registry.rs:66 的 `Arc<dyn Fn(Value) -> bool>`）→
`cli_entry.rs:188` 生产实现 → `cli_entry.rs:193` `serde_json::from_value::<SettingsPatch>` → :209 `apply_patch` → :223 合并写回 → **返回 `true`**
影响: 可维护性/契约一致性（**信任边界失败开放 + 契约声明与实现相反**）：
1. **Mod 侧静默失效**：Mod 发 `{"llm":{"base_urll":"http://typo"}}`（键名拼错）⇒ serde 忽略该键 ⇒
   `apply_patch` 无变更或只改其他字段 ⇒ **写盘成功、返回 `true`** ⇒
   Mod 侧 `apply()` 认为「已配置成功」，而宿主配置**根本没变**。
   这正是本仓明令禁止的一类（AGENTS.md / `display_prefs.dart:161-162` 引用的
   「那是本项目 P4 明令禁止的『**静默失效**』」）。
2. **边界声明是错的**：契约说「写不存在的键由 host 的 SettingsPatch 反序列化拒绝」，
   实际是「**被忽略**」。Mod 作者据此假设「写错键会收到 false」——
   而 `mod_registry.rs:1311` 的回归 `apply_settings_applier_reaches_host` 只断言
   「**合法** patch 被接受」，**没有**一条断言「非法键被拒绝」。⇒ 该边界**无测试保护**。
3. **附带风险**：`mod_registry.rs:371-372` 在 applier 里
   `tracing::info!(target:"mod", mod_id, config_path, "apply_settings {}: patch={}", …, patch)`
   —— **整个 patch 原样进日志**。一个（合理地）尝试用
   `{"llm":{"api_key":"sk-…"}}` 写密钥的 Mod（而密钥本就该走 `PUT /api/v1/env`），
   会把该值打进日志。文件层有 `SanitizingWriter`（B0017 已核其按**键名**匹配）可兜住
   `api_key`，但 **stdout sink 不经该 writer**。
建议: ① 在 `SettingsPatch` / `LlmPatch` / `TtsPatch` 上加 `#[serde(deny_unknown_fields)]`，
   使契约声明成真（并让键名拼错立刻拿到 `false`）——注意这会同时影响
   `POST/PATCH /api/v1/settings` 的前端路径，需与前端字段集一起核；
   ② 若不能加 `deny_unknown_fields`（向前兼容考虑），则改契约文案为「未知键**被忽略**」，
   并让 applier 在「patch 含任何未知键」时返回 `false`（可在 `from_value` 前做一次
   `SettingsPatch` 的 `deny_unknown_fields` 影子结构体做探测）；
   ③ applier 的日志**别打整个 patch**：打「哪些段被应用」即可（键名可打、值不打），
   与 AGENTS.md「写操作日志不记 body」同一纪律
验证: 读 services.rs:150-202（契约与两个 sender）、mod_registry.rs:60-70 + :360-405（闭包与日志）、
cli_entry.rs:180-230（生产接线）、error/patch.rs（grep deny_unknown_fields 零命中）
置信: 高（契约原文、唯一生产实现、serde 默认行为、grep 零命中四者互相印证）
反证: (a) 试过论证「`apply_patch` 内部会拒绝未知键」——它接收的是**已反序列化的 `SettingsPatch`**，
   未知键在 `from_value` 那一步就没了，`apply_patch` 根本没机会看见；
(b) 试过论证「`PatchBody`（desktop 侧）有 `deny_unknown_fields` 所以整条链是严格的」——
   Mod 走的是 `SettingsPatch`（runtime 侧），**不是** `PatchBody`；两者都无该属性；
(c) 试过找测试兜住 —— `mod_registry.rs:1311` 的 `apply_settings_applier_reaches_host`（:1336-1339）
   断言的是「合法 patch 被接受」，**没有**非法键用例；`settings_routes/tests.rs` 测的是 HTTP 路径
   而非 Mod 路径。⇒ 该边界**零覆盖**。

---

## BATCH-0061（2026-09-28）· Mod 侧「读」通道与会话隔离 —— 三项核验通过 / **0 条新发现**

> 按停止规则**保持在同一根**（Mod）。本批把 F-0060-01 的**对照面**核完：
> 同一条信任边界的**读**半边与**写**半边，姿态相反。

### 核验一：`ModSettingsReader` 的契约**为真**（读通道干净）
契约（`services.rs:183`）：「返回当前生效的 namespaced settings JSON（由 host 用**脱敏视图**
构造——**不含任何密钥明文，也不含 `api_key_env` 变量名**）」。
生产实现（`cli_entry.rs:273-287`）：
```rust
read_settings: Arc::new(move || {
    let Ok(s) = AppSettings::load_from_path(&path_for_read) else { return json!({}); };
    serde_json::to_value(settings::view::settings_to_view_with_keys(&s, &secrets::lookup)).unwrap_or_else(|_| json!({}))
})
```
⇒ 返回的正是 B0009/B0010 核过的 **`SettingsView`**：`LlmView` 只有 5 个字段
（`base_url` / `model` / `has_api_key` / `max_tokens` / `show_reasoning`，view.rs:62-76），
**没有 `api_key_env` 字段**；`has_api_key` 是 `key_is_ready()` 算出的**布尔**
（view.rs:271-273）⇒ **变量名与密钥明文都不出门**。契约为真。

### ⭐ 对照结论（同一边界的两半，姿态相反）
| 通道 | 契约声明 | 实现 | 姿态 |
|---|---|---|---|
| **读**（`ModSettingsReader`） | 「不含密钥明文，也不含 `api_key_env` 变量名」 | 返回 `SettingsView`（**确实没有**） | **严格，且与声明一致** |
| **写**（`ModSettingsApplier`，F-0060-01） | 「写不存在的键由 `SettingsPatch` 反序列化**拒绝**」 | serde **静默忽略**未知键 | **失败开放，且与声明相反** |
⇒ 同一条 Mod↔host 信任边界上，**读侧做对了、写侧做反了**。这比单说「写侧有洞」更有信息量：
**同一批代码里，两半的严格程度不一致，说明这不是「整体松」，而是写侧那一次实现漏了考虑。**

### 核验二：会话 id → 文件名，**三重归一**且无路径穿越
- 归一函数（`mod-system/src/session.rs:96-108`）：长度上限 + 字符白名单
  `is_ascii_alphanumeric || matches!(c, '-' | '_' | '.' | ':')` ⇒ **不含 `/` 与 `\`**；
- 第一次：宿主投递前归一（B0004 在 `chat_routes.rs` 核过该闸门被用在**写入**侧）；
- 第二次：`memory/src/lib.rs:711` `on_scoped_event` 再验，且**非法 id 退回全局桶 + warn**
  （:712-716「memory 收到非法会话 id，本轮退回全局桶（不拿它拼路径）」）；
- 第三次：`memory/src/strategy.rs:571` `resolve_session_store_path` **自己再验一遍**。

### 我试过推翻而没推翻的两条（记此以免后人重提）
1. **「白名单含 `.` ⇒ `sanitize_session_id("..")` 会通过 ⇒ 路径穿越」** —— **证伪**：
   id 的用法是 `format!("{id}.memory.jsonl")`（strategy.rs:573），**永远是文件名的一部分**，
   不是一个裸的路径段。`..` 经该格式化后是 `...memory.jsonl` 这个**字面文件名**，
   仍在 `sessions/` 目录内；而 `/`、`\` 已被字符白名单排除，**分段不可能发生**。
   ⇒ 路径穿越**不成立**。回归 `resolve_session_store_path_lands_under_sessions_dir`
   （`strategy_tests.rs:290-302`）对两个用例断言了精确解析结果。
2. **「Mod 读到的 settings 里可能带密钥片段（base_url 常含 token）」** —— **未证伪也不成立**：
   `base_url` 是用户自己填的端点地址，`SettingsView` 忠实回传；本仓的脱敏判据是
   「密钥（值）与 `api_key_env`（变量名）不出门」，URL 本身不在此列
   （AGENTS.md 的红线原文即如此）。**记此边界以免日后被当成漏洞**。

### 附带确认：`AssistantReplied` **不依赖 `current_session`**
`lib.rs:718-721`：「助手侧正文带**本轮**会话（host 与 TurnPrompt 投同一个 id）：
**直接用它选桶，不依赖 `current_session`**（切会话瞬间也不会错位）」——
避免了「用户切会话的那一帧，`current_session` 还是旧的 ⇒ 记忆写错桶」这个典型竞态。

---

### F-0062-01 · P2 【已登记 S3 的「升级」：保存 Mod 配置会删掉 secret 键 ⇒ 注入端点鉴权随之失效】

> ⚠ **本条为 BATCH-0062 的规范头**（该批最初只在散文里记了它，
> CONSOLIDATION-12 补上，以便机械计数与索引）。详细论证见该批段落。

## BATCH-0062（2026-09-28）· ⭐ **升级已登记的 S3：保存配置抹掉 secret —— 并追加它没说的后果**

file: crates/live2d-ai-desktop/src/web_api/mods_routes.rs:258 + crates/live2d-ai-desktop/src/mod_registry.rs:463-468
摘录（宿主侧，**整段链路上没有任何合并**）:
```rust
// mods_routes.rs:258
let reload_result = registry.reload_config(id_static, config.clone());
// mod_registry.rs:461-468
pub fn reload_config(&mut self, id: &'static str, config: serde_json::Value) -> Result<(), ModError> {
    let Some(e) = self.entries.get_mut(id) else { return Err(…) };
    e.config = config;                 // :464 ← **整份替换**，不合并、不保护 secret 键
    self.persist_manifest();           // 重写 mods.json
```
摘录（前端侧，**守卫存在但瞄错了失效形态**）:
```dart
// flutter/lib/settings/sections/dev_tools_section.dart:705-708
// secret 留空 = 「不修改」：服务端不回值，发空串会把已存的密钥清掉。
if (f.secret && _stringValue(f).isEmpty) { … }
```
调用链（把「保存一次无关字段」走到底）:
1. `GET /api/v1/mods` → `redacted_config`（B0006 已核）**剥掉** `secret:true` 的 `token`
   ⇒ **前端永远拿不到 token 值**（`mods_api.dart:20` 也这么声明：「`secret=true` 的 String
   服务端**不回值**，界面渲染为密码框，空 = 不修改」）；
2. 用户在面板改 `text_template` 或 `prefix` → 前端**省略**空的 `token` 键（:707）；
3. `POST /api/v1/mods/external-input/config` → `reload_config` → `e.config = config`
   ⇒ **`token` 键从 `mods.json` 里消失**；
4. 界面回 `'$id 配置已保存'`（`shell_admin.dart:235`，**成功文案**，无任何警告）。
影响: **安全 + 可维护性**。这是**已登记的 S3**（「保存配置抹掉 secret」），
本条把它**升级**为「**机制已定位 + 后果比登记所述更重**」：
- **登记表说的是「抹掉 secret」；实际后果是「抹掉之后鉴权直接失效」**：
  token 若只存在 Mod config（而非 `EXTERNAL_INPUT_TOKEN`），B0005 已核的
  `external_input_gate` 会得到 `config_token = None`；若 env 也未设 ⇒ `effective = None`
  ⇒ `check_token` 对**任何**请求返回 `true` ⇒ **注入端点变成无鉴权**
  （仍是 loopback-only，即 B0005 记录的「两者都空 → 不鉴权」那条向后兼容档）。
- **两侧各自都「看起来对」**：前端注释说对了失效后果（「发空串会把已存的密钥清掉」），
  却只防了「发空串」这一种形态；而宿主是**整份替换**，所以**「不发」同样等于删除**。
  ⇒ 这是**接缝缺陷**（模式 C：两侧组件各有真断言，接缝无人测），
  而**不是**哪一侧写错了。
建议: ① 宿主侧做**合并**：`reload_config` 保留既有 `secret:true` 键的旧值
   （需要 `settings_spec` 来判定哪些键是 secret —— `registry.settings_spec(id)` 已有，
   B0006 核 `redacted_config` 用的就是它）⇒ 前端「留空 = 不修改」的契约**才真正成立**；
   ② 合并后前端那条守卫的注释就名副其实了（两侧一起对）；
   ③ 加一条**接缝**回归：「配置含 secret → GET 剥掉 → 前端回传不含该键 → 落盘后该键仍在」
   —— 这条测试现在**不存在**，所以这个洞能长期存在
验证: 读 mod_registry.rs:455-470（`enable_with_config` / `reload_config`）、
mods_routes.rs:157-166 + :250-266、dev_tools_section.dart:700-712、
external-input/src/lib.rs:143-151（`token_from_config` 过滤空串）、
mod-system/src/services.rs:180-202（读侧脱敏契约）
置信: 高（写侧两个函数 + 路由调用点 + 前端守卫 + B0005 已核的鉴权链，四段都能直接读出）
反证: (a) 试过论证「宿主在某处合并了 secret 键」——`reload_config` 函数体只有 4 行
（取 entry / 赋值 / persist），路由层 :258 直接透传，**无任何合并**；
(b) 试过论证「前端会把 token 原样回传」——**不可能**：`redacted_config` 在 GET 侧剥掉了它，
前端**手里没有值**，回传也回传不出（B0006 已核）；
(c) 试过找回归兜住——`mod_registry.rs:1252/1289` 的测试用
`apply_settings: Arc::new(|_| true)` 这类桩，与 config 合并无关；**没有**一条断言
「保存非 secret 字段后 secret 键仍在」。
(d) 试过论证「这只是 P3（配置易失）」——**不是**：它是**静默的安全降级**（鉴权消失）
且界面回的是**成功**文案，用户与排查者都得不到任何信号。

---

## BATCH-0063（2026-09-28）· **F-0062-01 的扩散核：5 个在册 Mod 里有 2 个中招**

file: crates/live2d-ai-mod-voice-input/src/lib.rs:188-193 与 :237-238（**它自己写明了后果**）
摘录:
```rust
ModSettingField::String {
    key: "token".to_string(),
    label: "访问令牌（**空 = 不鉴权**）".to_string(),     // :190 ← 字段标签自己写了后果
    secret: true,
    default: None,
},
```
```rust
/// 空值语义是「**不鉴权**」而不是「用空 token 鉴权」。        // :237
pub fn token_from_config(config: &serde_json::Value) -> Option<String> {
```
扩散核的结果（`grep -n 'secret.*true' <各 Mod>/src/lib.rs`）:
| 在册 Mod | `secret:true` 字段 | 是否受 F-0062-01 影响 |
|---|---|---|
| `external-input` | `token`（lib.rs:93） | **是**（B0062 已核全链） |
| `voice-input` | `token`（lib.rs:192） | **是**（同一接缝，且其标签/文档自证「空 = 不鉴权」） |
| `persona` | 无 | 否（无 secret 键可丢） |
| `memory` | 无 | 否 |
| `director` | 无 | 否 |

影响: **F-0062-01 的范围据此确定**（不是「一个 Mod 的一个字段」，而是）：
**宿主侧从未实现 secret 保留语义，因此每一个带 `secret:true` 字段的 Mod 都会丢**，
当前是 **5 个在册 Mod 里的 2 个**（`external-input` 与 `voice-input`，两者都是
`token`，两者都把「空 = 不鉴权」写在**自己代码的注释与标签里**）。
⇒ 也就是说：**这两个 Mod 的作者已经精确写下了「token 空了会怎样」，
而那个「空」恰恰是本产品自己的 UI 保存流程会制造出来的**。
这让本条的严重性从「配置易失」稳定落到「**静默的鉴权降级**」——
用户为改一个模板/语言/端口而点保存，就把自己 Mod 的本地端点改成了无鉴权，
界面显示「配置已保存」。
建议: 同 B0062 的建议①（宿主侧按 `settings_spec` 保留 secret 键旧值）。
本条新增的**验收要求**：修复后应有一个**覆盖全部在册 Mod** 的回归，
逐个断言「保存非 secret 字段 ⇒ 该 Mod 的 secret 键仍在盘上」——
因为这是**跨 Mod 的共同契约**，逐个 Mod 各自测会漏掉「新增 Mod 忘了遵守」。

---

## BATCH-0064（2026-09-28）· 会话分桶的实现面 —— **隔离以「类型级」保证，核验通过**

> **0 条新发现。** 补上挂了 4 批的空：`ModSessionPrompts` 的生产实现
> （`session_scopes().as_mod_prompts()` → `SessionScopeStore`）。本批把它审完，
> 「按会话分桶、A 会话的人设不泄漏到 B 会话」这条契约**闭环**。

### 核验一：隔离是**类型级**的，不是运行时检查（最强的一档）
`session_scope.rs:162-166` / `:246-250` 取值都是**精确键查找**：
```rust
pub fn prompt_for(&self, session: Option<&str>) -> Option<String> {
    let id = session.and_then(sanitize_session_id)?;   // :163 闸门
    let inner = self.guard();
    compose_slots(inner.prompts.get(&id)?)             // :165 get(&id)? —— **命中不到就是 None**
}
```
`?` 在未命中时直接返回 `None`，**没有任何跨会话回落**。
而合成器（`:111-117`）的签名是：
```rust
fn compose_slots(slots: &Slots) -> Option<String> { compose_session_prompt(slots.iter().map(…)) }
```
它**只接受某个会话的那一份 `Slots` 的引用**，拿不到 `Inner`、也拿不到 id
⇒ **结构上不可能读到别的会话**。这不是「记得加判断」，是**类型上就够不着**。

### 核验二：三个写入口与两个读入口**共用同一道闸与同一个键空间**（显式防「第二套会话表」）
`:163` / `:205` / `:211` / `:247` / `:254` —— 五处**各自**调 `sanitize_session_id`；
`:202-203` 把这个约定写死：「`baseline_for` 与 `prompt_for` **逐字同构**（同一道归一化闸、
同一个键空间）」；`:168-172` 更直接：「**不要在这里另起一套归一化或容量判定——
那正是『第二套会话表』的开端**」。
`set_owned`（`:252-266`）按**每条贡献**判长度（不静默截断）、空串 = 撤销该槽；
`remove_slot`（`:120-130`）最后一条槽被删时**整条会话条目也删** ⇒ `len()`/`sessions()` 不留空壳。

### 核验三：多写者的顺序是**单一真源**
`owner_merge_rank`（`mod-system/src/session.rs:115-118`，B0061 已核）规定
「匿名槽最前 → persona → memory → 其余按字典序」，且注释写明
「**宿主实现与测试替身都必须用它**，否则『宿主里一个顺序、单测里另一个顺序』
会让组合结果变成**两份真相**」。
⇒ 宿主与 persona Mod 与 memory Mod 三方写进同一张表，合成顺序只有一份定义。

### ⭐ 与 F-0062-01 的对照（同一批代码里两种不同的工程姿态）
| 契约 | 宿主侧实现 | 形态 |
|---|---|---|
| **会话分桶不串味**（`ModSessionPrompts`） | 单一键空间 + 单一闸门 + **类型级**隔离 + 单一合成顺序 + 防「第二套表」的显式警告 | **做对了，而且防的是「多写者」这个真实风险** |
| **secret 留空 = 不修改**（`apply_settings` / config 保存） | **无任何实现**（`reload_config` 整份替换） | **没做**（F-0062-01） |
⇒ 两个契约面对的都是「**多方参与**」的现实（Mod + 宿主），
但一个按「多写者会出事」设计，另一个按「只有一条路」设计。
**这说明本项目会认真处理「多写者」的问题**（会话表有完整防���），
**却漏了「同一份数据经两条路回来」的问题**（config 往返：GET 剥 secret / POST 整份替换）。
⇒ 这可作为模式 L 的一个补充判据：**审计多写者系统时，除了「谁在写」，
还要问「同一个值会不会经两条路往返」**。

---

## BATCH-0065（2026-09-28）· `settings_spec` 契约 —— **F-0062-01 的修法有现成地基，且 spec 侧有一处硬ening**

> **0 条新发现。** 收尾 Mod 根。核 `mod-system/src/settings.rs`（155 行），
> 因为它同时是**脱敏契约的数据源**与 **F-0062-01 修法的地基**——地基若不可用，我的建议就落不了地。

### 核验一：`secret` 标记是**单一数据源**，且宿主侧现成可取
`ModSettingField::String { secret: bool }`（`settings.rs:20-33`）是「这个键是密钥类」的**唯一声明处**，
字段注写明「前端渲染为 password 输入；**内容不落日志**」。
`ModSettingsSpec::validate`（:74-80）校验字段 key **唯一**（重复 → `Err`）。
⇒ **F-0062-01 的建议①可直接落地**：`reload_config` 已在同一个结构体上，
`self.settings_spec(id)`（`redacted_config` 已在用它，B0006 核过）就是那份「哪些键是 secret」的清单，
**改动量约 5 行**（合并时按 spec 跳过 secret 键）。地基已验可用。

### 核验二：spec 侧有一处**针对「同一键两种身份」的硬ening**（值得单独记）
`validate`（:73-80）拒绝**重复的字段 key**。这不是洁癖：
若允许同一个 key 声明两次（一次 `secret:false`、一次 `secret:true`），
那么**脱敏时按哪一份判定就成了实现细节** —— 一个 Mod 可以让同一个键
「在 GET 里明文、在面板里当普通框」⇒ 密钥泄露面。
⇒ **重复 key 拒绝 = 堵死了「同一键两种身份」这条路**。这是我在本审计见到的
**第三处「结构上堵死一类攻击面」的设计**（前两处：`compose_slots` 的类型级隔离、
`sanitize_session_id` 的字符白名单）。

### 核验三：schema 自己写着「禁止 Mod 注入 HTML/JS/DOM」
`settings.rs:3`：「**禁止 Mod 注入 HTML/JS/DOM**——前端通过 `fields` 生成控件。」
⇒ 这条安全边界写在**数据契约文件的头部**（而不是散落在 UI 实现里），
所以读前端面板的人不必翻 UI 代码就能看到它。与红线 G（注入面）同向。

---

## BATCH-0066（2026-09-28）· ⭐ **自我更正 F-0062-01 的机制描述**（结论不变，机制更准）

> **0 条新发现**，但本批**修正了我自己 B0062 的机制表述**。结论（保存配置会删掉 secret、
> 并让注入端点鉴权失效）**不变且已被本批再次确证**；错的是我说的「环节」。

### 我 B0062 写错的地方
原文写：「前端**省略** secret 键 + 宿主整份替换 ⇒ 键被删除」，
并据此说「前端那侧的守卫瞄错了失效形态」。
**实际情况**：前端**没有**把 secret 从整份 config 里删掉 —— 它**做的是合并**：
```dart
// shell/flutter/lib/settings/sections/dev_tools_section.dart:697-716
/// 从服务端已有 config 出发、只覆盖本 Mod 在 spec 里声明的字段：
/// spec 之外的既有键（未来扩展）不该被一次「保存」顺手抹掉。
Map<String, Object?> _buildConfig() {
  final Map<String, Object?> next = Map<String, Object?>.of(widget.mod.config);   // :699 ← 合并基线
  for (final ModSettingField f in _spec.fields) {
    // secret 留空 = 「不修改」：服务端不回值，发空串会把已存的密钥清掉。
    if (f.kind == ModFieldKind.string && f.secret && _stringValue(f).isEmpty) { continue; }  // :707-709
    next[f.key] = switch (f.kind) { … };                                            // :710-715
  }
```
⇒ 前端**有合并**、且**确实保护了「spec 之外的既有键」**；那条注释（:698-700）写得**完全正确**。
**我错在**：把它当成了「只发自己那几个字段」。

### 真正的机制是**三段接缝**，而 secret 死在**第一段**
```rust
// ① 桌面/src/web_api/mods_routes.rs:483-499
fn redacted_config(config: &Value, spec: Option<&ModSettingsSpec>) -> Value {
    let mut obj = config.as_object().cloned().unwrap_or_default();
    for field in &spec.fields {
        if let ModSettingField::String { key, secret: true, .. } = field {
            obj.remove(key);                      // :494 ← **整键删除**（不是置空）
        }
    }
    …
}
```
三段合起来：
| 段 | 行为 | 局部评价 |
|---|---|---|
| ① `redacted_config`（GET） | 按 spec **整键删除** secret（`:494` `obj.remove`） | **正确**（红线 R 要求不回值）；**副作用**：它产出的 config **不可能**再带回原值 |
| ② `_buildConfig`（前端） | 从 ① 的结果**合并**，跳过空 secret（:699/:707） | **正确**（保护 spec 外键、不写空串）——但基线已被 ① 掏空 |
| ③ `reload_config`（宿主写） | `e.config = config` **整份替换**（mod_registry.rs:464） | **缺**：没有任何一处负责把 secret 键**带过去** |

⇒ **结论不变、且更硬**：因为 ① 已经把值取走，**② 在结构上就不可能**保留它
（它手里没有值）⇒ ③ 收到的那份 config 里**必然没有** `token` ⇒ 盘上那份也没了。
⇒ **修法只能是宿主侧**（与我 B0062 建议①一致）：`reload_config` 合并时
按 `self.settings_spec(id)` 跳过 secret 键、保留旧值。**前端无法修**（它拿不到值）。
⇒ **对模式 C 的升级**：这不是「两侧组件各有一半断言、接缝无人测」，而是
**三段各自局部正确、而「把值带过第二段」这个职责从未被指派给任何人**。
**三段都合理 ⇒ 没有一段需要改作者的意图，只有接缝缺一个责任人。**

---

## BATCH-0067（2026-09-28）· `min`/`max` 的两侧口径 + 团队 F-0012-1 放大器的 HEAD 复核

### F-0067-01 · P3
file: shell/flutter/lib/settings/sections/dev_tools_section.dart:1011-1019（**唯一的强制点**）
摘录:
```dart
case ModFieldKind.number:
  return NumberField(
    label: f.label, icon: Icons.tag, value: _intValue(f),
    // 尊重 spec 的 min/max：`NumberField` 对越界输入**不回调**。
    min: f.min?.toInt(), max: f.max?.toInt(),
    onChanged: (int v) => setState(() => _values[f.key] = v),
  );
```
影响: 契约不对称（**不是**失效）：
- **前端确实强制** `min`/`max` —— `_values` 只能经 `onChanged` 写入，而 `NumberField`
  对越界输入**不回调**，所以越界值**进不了** `_buildConfig`（我原先怀疑「未钳位」，
  **证伪**：`_intValue` 的 `toInt()/tryParse` 确实不钳，但不需要钳）；
- **宿主侧零强制**：`grep -rn "clamp" mod-system/src/settings.rs mod-external-input/src/lib.rs`
  → **零命中**；`reload_config` 是 `e.config = config` 整份替换，**不按 spec 校验**。
⇒ 所以「盘上某个 Mod 的数字落在 spec 的 min/max 内」这条不变式
**只在「所有写入都经过这个 UI」时成立**；直接 `POST /api/v1/mods/{id}/config`
塞一个越界值会被**原样存下**，且**无任何提示**（前端下次打开时 `_intValue` 原样回显，
`NumberField` 的值又落在 min/max 外——用户会看到一个「控件显示的非法值」）。
建议: ① 宿主侧在 `reload_config` 里按 `self.settings_spec(id)` 校验数值字段的
`min`/`max`（与 F-0062-01 的 secret 合并**同一个函数**里做，一次改两件事）；
② 或在 `ModSettingsSpec` 的 `min`/`max` 字段注上「**仅前端控件约束，宿主不校验**」，
让 Mod 作者知道不要把它当安全边界。
**为什么定 P3**：无安全影响、无数据丢失、越界值的后果是该 Mod 自己跑不起来
（端口越界 ⇒ 绑不上），且**没有**任何契约声明说宿主会校验（与 F-0060-01
那种「声明拒绝而实际忽略」不同）。但它是**第三条边界上「校验只在一条边上」的样本**，
值得记，以免将来有人以为这条不变式全局成立。
验证: 读 dev_tools_section.dart:1011-1019 与 :678-682（`_intValue` 不钳）；
`grep -rn "clamp" mod-system/src/settings.rs mod-external-input/src/lib.rs` → 零命中；
读 mod_registry.rs:463-468（整份替换不校验）
置信: 高
反证: (a) 试过论证「前端会把越界值发上去」——`NumberField` 不回调 ⇒ `_values` 不变 ⇒ 不发；
(b) 试过找宿主侧的第二道闸（`settings_spec` 的 `validate`）——`validate`（settings.rs:74-80）
只校验**字段 key 不重复**，**不校验值域**；(c) 试过论证「越界值有安全后果」——无：
最坏是该 Mod 的监听端口绑不上，属自伤不属攻击面。

---

## 独立复核：团队 F-0012-1（P1，放大器）**在 HEAD 仍成立**（本审计 F 命名空间外，仅记录复核）
`shell/flutter/lib/ui/field_row.dart:610-618`：
```dart
/// 内联结果行（连通性自检的 `ok`/`latency_ms`/`error`）。
final String? result;
final bool resultIsError;
…
error: resultIsError ? result : null,      // :617
```
`result` 在整个文件里**只出现在这一处**（B0032 已核）⇒ 上游若把失败误判为成功
（`llm_section.dart:179-183` / `tts_section.dart:177-179` 的 `ok`/`毫秒`/`ms` 子串扫描），
`resultIsError == false` ⇒ `error: null` ⇒ **界面上什么都不显示**，
与「测过了且通过」「从没测过」**三者不可区分**。
⇒ 这是 AGENTS.md「**自检说谎比没有自检更坏**」的教科书实例，且**放大器仍在**。

---

## BATCH-0068（2026-09-28）· ⭐ 「不回调」核实为真，**但发现它的代价是静默拒绝**

> **0 条新缺陷**（B0067 的未核实点已结），但顺带把 F-0067-01 的**代价**看清了，
> 并发现一个此前没注意到的显示/存储不一致。

### 核验一：`NumberField` 确实是「**不回调**」—— B0067 的判读成立
`shell/flutter/lib/ui/field_row.dart:427-433`（`NumberField` 内部 `TextField` 的 `onChanged`）：
```dart
final int? parsed = int.tryParse(raw.trim());
…
if (widget.min != null && parsed < widget.min!) return;   // :432 越界 → **直接 return**
if (widget.max != null && parsed > widget.max!) return;   // :433
widget.onChanged(parsed);
```
⇒ `dev_tools_section.dart:1016` 那句注释「`NumberField` 对越界输入**不回调**」**准确**。
`_values` 只能经 `onChanged` 写入 ⇒ 越界值**进不了** `_buildConfig` ⇒ 前端强制**成立**。

### F-0068-01 · P3 【越界输入被静默拒绝 ⇒ 框里显示的值不一定是将被保存的值】
file: shell/flutter/lib/ui/field_row.dart:427-433（拒绝）+ :437-440（helperText 只显示区间）
摘录:
```dart
if (widget.min != null && parsed < widget.min!) return;   // 无提示、无回弹、无错误态
…
helperText: widget.min == null && widget.max == null
    ? …
    : '${widget.min ?? '−∞'} – ${widget.max ?? '∞'}',      // :439 只显示**合法区间**
```
影响: 可用性/一致性（**显示 ≠ 存储，且零信号**）：
- 用户在框里键入越界值 ⇒ `TextField` **照常显示**（拒绝只拦回调，不拦文本），
  而 `_values[key]` **仍是旧值** ⇒ 点「保存」时提交的是**旧值**；
- 界面**没有任何**反馈说明这次输入没被接受（没有错误态、没有回弹、没有 toast），
  唯一的线索是 helperText 里那个区间——而它**本来就在那儿**，不构成「刚才被拒了」的信号；
- 与 F-0067-01 叠加出一条**可达链**：宿主侧零校验 ⇒ 直接 `POST .../config` 可把越界值写上盘
  ⇒ 面板回显一个越界值 ⇒ 用户**无法判断它是无效的** ⇒ 若他此时点「保存」，
  那个越界值会被**原样再存一遍**（因为它在 `widget.mod.config` 里，作为合并基线被带回）。
建议: ① 越界时给出**明确反馈**（`setState` 一个 error 文本，或把输入回弹到最后一个合法值
   ——回弹比提示更不容易让人误以为已保存）；② 或改成「**钳位后回调 + 显示钳位提示**」
   （让用户看见系统采用了 30 而不是 99999）；③ 两侧一起做时，在 F-0067-01 的宿主侧
   校验落地前，前端至少要有①，否则「盘上有越界值」这个状态在界面上**完全不可见**。
**为什么定 P3**：无安全影响、无数据丢失（保存的是**上一个合法值**，不是垃圾值）；
但它把「无效输入」变成了**无声丢弃**，与本仓反复强调的「不静默」纪律是同一条线，
且修复成本低。
验证: 读 field_row.dart:375-445（`NumberField` 构造 + 内部 `onChanged` + helperText）；
读 dev_tools_section.dart:1000-1033（控件构造与 `_values` 的唯一写入口）
置信: 高
反证: (a) 试过论证「TextField 是受控的、越界会回弹」——`value:` 只用于**初始显示**，
   内部 `TextController` 持有用户文本，代码里**没有**在拒绝时回写 controller；
(b) 试过论证「helperText 的区间足以让用户自查」——区间是**常驻**的，
   用户无法区分「我刚才输的 99999 被拒了」与「我还没输」；
(c) 试过找测试覆盖 —— **未核实**（`field_row` 的测试是否存在越界用例，未查）。

---

## BATCH-0069（2026-09-28）· ⚠ **撤回 F-0030-01（P2）** —— 我的链路漏了一层，结论不成立

> **本批 0 条新发现，但撤回了本审计自己的一条 P2。** 这是至今为止**最重的一次自我证伪**，
> 按 §5 纪律**不删原文**，只在其后追加撤回段并说明三条子主张各自的下场。

### 撤回理由一：链路漏了**防抖**层 ⇒ 「滑杆每像素一次」为**假**
B0030 我写的链路是：
`_updatePrefs`（音量滑杆**每像素**）→ `_applyPrefs` → `_syncActionScalesNow(force: true)`
→ `syncNow(force: true)` → `!force && key == _sent` 短路失效 → `send(effective)`。
**我漏掉了 `syncNow` 之前的一层**：
```dart
// shell/flutter/lib/live2d/action_scales_sync.dart
const Duration kActionScalesDebounce = Duration(milliseconds: 150);   // :53
…
_timer = Timer(kActionScalesDebounce, () => syncNow(payload));       // :136
```
⇒ 一次连续拖动**最多每 150ms 一帧（约 7 帧/秒）**，不是「每像素一次」。
**并且它有对应回归**：`action_scales_preview_test.dart:142`
`test('防抖窗口内连拖多次：只发最后一帧', …)`。
⇒ 我原文「『滑杆每像素一次』这条症状仍在」**不成立**。

### 撤回理由二：`force` 的语义**有以它命名的测试**
同文件 `:186`：`test('force 绕过去重：iframe 重建后必须重发', …)`
⇒ 「绕过去重」不是遗漏，是**被单独立项、被命名、被钉住**的行为。
另有一条独立测试证明**非 force 路径会去重**（`:162`
`test('值没变不重发；变了才发（与舞台无关的编辑不打扰渲染面）', …)`）。

### 撤回理由三：「文档用途与实际用途不符」在 HEAD **已不成立**
B0030 我写「`force` 的**文档用途**只是重挂补发，不是『任意偏好变更都要发』」。
HEAD 上**两处**都已写明更宽的用途并给出理由：
- `shell_prefs.dart:129-131`：「`force: true` **不会**冲掉临时覆盖：syncer 发的是它的
  `active()`（临时覆盖 > 产品值）。**改主题 / 调音量等任意偏好变更走到这里**，临时值照样在」；
- `action_scales_sync.dart:33-35`：「`[syncNow]` 发的是 `[active]` 而不是入参，所以
  `_applyPrefs` 的 `force: true` 补发（**改主题 / 调音量等任意 `DisplayPrefs` 变更都会走**）
  也冲不掉临时值」。
⇒ 「描述与实际用途不符」这一条**已被作者修正**（或我读的是修正前的版本）。

### 顺带确认：那条依赖的机制是真的
`shell_prefs.dart:129-131` 的注释声称「syncer 发的是它的 `active()`（临时覆盖 > 产品值）」——
`action_scales_sync.dart:15-17` 的头注确实这么定义：「`[active]` 是**给舞台的唯一取值口**：
有临时覆盖给临时值，否则给产品/草稿值」；且 `:18-21` 记录了
`main.dart` 里那条直发路径**已删除**（两个写者各自去重会让「同一操作序列的结果**依赖历史**」）。
⇒ 注释**不是**新的模式 D 样例。**模式 I 的第二个样本（F-0030-01）随之撤回**，
模式 I 回到 1 个样本（F-0028-01 解码缓存容量）。

### 我从中提炼的教训（写进模式 L 的执行规则）
**追链路时必须核「发送前还有没有一层」**。我在 B0030 从「偏好变更」一路追到 `send()`，
把 `syncNow` 当成了发送前的最后一环 —— **而它不是**。
同一条纪律对已成立的其他结论同样适用：**任何「每 X 一次」的性能主张，
都必须先确认 X 与发送之间没有防抖/节流/合帧**。
⇒ 另注：这条与 B0023（30ms 轮询 + 33ms 二次节流）恰好互为镜像 ——
那次我**因为**记得节流才没写错，这次我**忘了**就写错了。
**知识资产会随时间失效，必须每轮重新核，不能靠「我上轮核过」。**

---

## BATCH-0070（2026-09-28）· ⚠ **F-0034-01 的「潜在陷阱」证伪**（id 是**内容指纹**，不是任意标识）

> **0 条新发现。** 按 B0069 的教训**不靠旧结论**重核了我留的另一条未结论化线索。

### 我 B0034 写的东西
> 「`display_prefs.dart:970` 用 id-only `sameAs`（`other.id == id`）⇒ **潜在陷阱**：
> 若同 id 不同内容，`_updatePrefs` 的 `if (next == widget.prefs) return;` 会**静默跳过**这次变更。」

### HEAD 实况：id 是**内容指纹**，同 id 必然同内容
`shell/flutter/lib/design/background_item.dart:494-495`：
```dart
String backgroundIdOf(String dataUrl) =>
    'bg${backgroundFingerprint(dataUrl).toRadixString(16).padLeft(8, '0')}';
```
⇒ `id = "bg" + hex(指纹(dataUrl))`：**id 由内容派生**。
同文件 :40 的注释还说明了为什么不用 `Object.hash`：
「…的 id**——『图不见了』就是这么来的。`Object.hash` 更糟：**按平台而变**」
（即 id 必须是**跨平台稳定**的，否则同一张图在不同平台上算出不同 id ⇒ 加载时找不到 ⇒ 图消失）。

⇒ 因此 **id-only 判等是正确的，不是陷阱**：
| 我担心的 | 实况 |
|---|---|
| 同 id 不同内容 | **不可能**（id 是内容指纹） |
| `==` 只比 id 会漏掉样式变更 | 样式（`opacity`/`fit`/`align`）变了会**换 dataUrl 吗**？不会 —— 但那也不影响：`==` 比的是 **`backgrounds` 列表里每一项的 `sameAs`**，而**逐图样式**存在**独立的** `styleOverrides` 一侧（B0034 已核：`appearance_background.dart:1264` 的 `if (image != null && styleChanged != null)` 说明**样式控件在无回调时不渲染**，即样式是**独立字段**、不在 `BackgroundImage` 里）⇒ 改样式走的是**另一个字段**，`==` 会在那里判出差异 |
| 去重调用点语义不符 | `shell_prefs.dart:475` `if (prefs.backgrounds.any((b) => b.sameAs(item))) { …『已经有这个图案了』; return; }` —— **正是去重**用途，与 `sameAs` 的文档注「**用于去重**」一致 |
⇒ **F-0034-01 的「潜在陷阱」不成立**，记此证伪；**不记发现**。

### 顺带记一条**不构成发现**的观察（备忘）
两处 `hashCode` 与 `==` 不是对称配对：
- `background_item.dart:385` `hashCode = Object.hash('image', id, opacity, fit, align)`，
  而 `sameAs` 只比 `id` ⇒ `a == b` 可能为真而 `a.hashCode != b.hashCode`（Dart 允许）；
- `display_prefs.dart:975-994` 的 `hashCode` **完全不含 `backgrounds`**，而 `==`（:968-971）含。
两处都**合法**，且 `DisplayPrefs` 不作 map key 使用 ⇒ **无实际影响，仅备记**。

---

## BATCH-0071（2026-09-28）· `backgroundFingerprint` —— 残留风险量化后**归零**

> **0 条新发现。** B0070 明确留下的唯一残留风险是「若指纹只取前若干位 ⇒ 两张不同的图判等
> ⇒ F-0034-01 以另一种形式复活」。**核完：不是。**

### 核验：指纹是**完整 31 位**，无任何截断
`shell/flutter/lib/design/background_item.dart:472-484`：
```dart
/// 取模用的素数（`2^31 - 1`，梅森素数）。
///
/// 必须是奇素数：`2^31 - 1` 恰好是奇数，且 **33 与它互质 ⇒ 哈希不退化**。
const int kBackgroundHashModulus = 0x7FFFFFFF;

/// djb2 变体：`h = h * 33 + c`（mod [kBackgroundHashModulus]）。
int backgroundFingerprint(String text) {
  int h = 5381;
  for (int i = 0; i < text.length; i++) {
    h = (h * 33 + text.codeUnitAt(i)) % kBackgroundHashModulus;
  }
  return h;                                    // :483 返回**完整 31 位**
}
```
`backgroundIdOf` = `'bg' + h.toRadixString(16).padLeft(8,'0')`（:494-495）
⇒ 8 个十六进制位 = 32 位表示一个 31 位值 ⇒ **没有截断、没有取低位**。
⇒ 「两张不同的图因指纹碰撞而被判等」的概率：31 位空间 ≈ 2.1e9；
用户级背景数量（个位数到几十）下碰撞概率 ≈ n²/2^32，**n=100 时约 1.2e-6**，
且**碰撞的后果是良性的**（第二张图被当作重复项提示「已经有这个图案了」，
**不丢数据、不损坏存储、不影响渲染**）⇒ **不构成缺陷，不记发现**。

### 附带：三处微决策都**写了理由**，且理由正确
1. **模数必须是奇素数**（:473-475）：djb2 的乘子 33 与模数互质 ⇒ 哈希不退化
   （否则循环节变短、指纹分布变差）——这是 djb2 用素数模的正确理由，写对了；
2. **id 带 `bg` 前缀**（:488-489）：「这样它在日志 / IndexedDB 里**一眼能认出**
   「这是背景字节，不是别的东西」」；
3. **整个 dataURL（含 MIME 头）参与哈希**（:491-493）：「同一张图但换了 MIME 头
   （浏览器对不同扩展名给的类型不同）应当算**两张不同的记录**，行为上**更可预期**」
   ——选「更可预期」而非「更精确」，且**明确承认了这是取舍**。
⇒ 这三处与 B0070 记的「`hashCode` 与 `==` 非对称」**并不矛盾**：
djb2 是**非密码学**哈希，用途是**本地身份标识**（去重 + 存储键），**不承担安全职责**；
不需要抗碰撞攻击，只需要「够散 + 跨平台稳定」，而这两点都满足
（:40 注释已说明为何不用按平台而变的 `Object.hash`）。

---

## BATCH-0072（2026-09-28）· ⚠⚠ **再撤回一次：F-0070 对 F-0034-01 的「证伪」本身是错的 ⇒ F-0034-01 恢复并升级**

> **0 条新发现，但这是一次比 B0069 更严重的自我更正**：我在 B0070 **证伪**一条旧结论时，
> **用了一个错误的前提**，并据此写下了一张三行的「不成立」表。按 §5 纪律全文留档、显式改写。

### 我 B0070 的证伪前提是什么（原文）
> 「② 逐图样式存在**独立字段**（B0034 已核 `appearance_background.dart:1264` 无回调时样式控件
> 不渲染）⇒ 改样式走**另一个字段**，`==` 会在那里判出差异」
⇒ **「样式在独立字段」是我推的，不是我读到的。而它恰好是整个证伪的承重墙。**

### 实际（`background_hydration.dart:134-136`，刚读到）
```dart
// **必须走 copyWith**：直接 `new BackgroundImage(id: …)` 会把用户给这张图
// 设的 opacity / fit / align 一起清空（F-0013-1 的硬要求）。
next.add(item.copyWith(dataUrl: url));
```
⇒ **逐图样式（`opacity` / `fit` / `align`）就住在 `BackgroundImage` 里面**，
不是独立字段；保住它的是 `copyWith`（:283 的 `id: id ?? this.id` 那一族），
**不是**某个独立的样式存储。
⇒ 我 B0070 那句「样式走另一个字段、`==` 会在那里判出差异」**是错的**。

### 于是 F-0034-01 的机制要**重写**（比 B0034 原文更准）
链条（每一步都已核）：
1. `backgroundIdOf(dataUrl) = 'bg' + hex(指纹)`（:494-495）⇒ **同 id 必然同 dataUrl**
   ⇒ `sameAs` 只比 id，对**去重**用途是**正确**的（`shell_prefs.dart:475`「已经有这个图案了」）；
2. **但** `DisplayPrefs.==` 复用了它做**变更检测**：`display_prefs.dart:968-971`
   `if (!backgrounds[i].sameAs(other.backgrounds[i])) return false;`
   ⇒ **逐图样式变了（`opacity`/`fit`/`align`），`sameAs` 看不出差异**；
3. `shell_prefs.dart:146` `if (next == widget.prefs) return;`
   ⇒ **纯样式变更会被 `==` 判成「没变」而整条丢掉**。
⇒ **准确的说法不是「id-only 判等错了」（它没错）**，而是：
> **一个为「去重」设计的 id 比较，被复用为「整对象变更检测」的判据，
> 而这个对象里恰好含有逐项可变的子状态。**
⇒ 这又是**接缝**（模式 C）：`sameAs` 两侧各自都对，中间的复用没人管。
**为什么仍是「潜在」而非「在线」**：B0034 已核 `appearance_background.dart:1264`
`if (image != null && styleChanged != null)` ⇒ 无回调时**样式按钮根本不渲染** ⇒ 当前不可达。
但 `background_hydration.dart:134-135` 证明作者**知道**样式住在 `BackgroundImage` 里、
并且专门为它写了守卫 ⇒ 一旦有人把样式控件接上（而 B0034 记的「全部控件都由各自 section 的构造器决定」
意味着接线是常规改动），这个洞立刻变成**在线**的静默丢弃。
建议: ① `DisplayPrefs.==` 的 backgrounds 比较**不用 `sameAs`**，改用「逐项 `==`」
   （`BackgroundImage` 已实现 `==`：`Object.hash('image', id, opacity, fit, align)`，见 :385 ——
   **hash 与 == 已对称**，只是 `sameAs` 绕过了它）；② 或者给 `sameAs` 改名成
   `sameIdentity` 之类，让「去重」与「变更检测」在名字上就分家，避免第三次被复用。
**定级 P3**（潜在、当前不可达、修法两行），但**记录价值高**：它是我这轮两次自我更正里
**唯一一处「证伪本身错了」**的样本，教训是——
**做证伪时必须逐句核对自己引用的每个前提，不能因为「我上轮读过相关文件」就认为前提成立。**

---

## BATCH-0073（2026-09-28）· F-0034-01 修法①**核实通过**，且发现「正确的判据就在旁边」

> **0 条新发现**，但把 F-0034-01 从「潜在陷阱」升级成**完全指定的接缝**，并核实了它的修法。
> 本批第一件事就是核我自己建议里引用的前提（B0072 教训：**先核自己写下的前提**）。

### 核验：`BackgroundImage` **已经实现了正确的判据**，`==` 与 `hashCode` 完全对称
`shell/flutter/lib/design/background_item.dart:377-385`：
```dart
@override
bool operator ==(Object other) =>
    other is BackgroundImage &&
    other.id == id &&
    other.opacity == opacity &&      // ← **逐图样式在 == 里**
    other.fit == fit &&
    other.align == align;

@override
int get hashCode => Object.hash('image', id, opacity, fit, align);
```
⇒ `==` 与 `hashCode` 覆盖的字段**完全一致**（id + opacity + fit + align）—— 这是教科书正确的值类型。
对照 `BackgroundPattern`（:430-434）：`==` 只比 `id`，`hashCode` 是 `('pattern', id)`
⇒ **同样对称**（图案没有逐项可变样式，id-only 对它是**正确的**）。
⇒ 所以 `design/` 这一层**没有错**。

### 缺陷因此变成**完全指定**的一条（比 B0034/B0072 的表述都准）
| 层 | 判据 | 正确？ |
|---|---|---|
| `BackgroundImage.==` | id + opacity + fit + align（与 hashCode 对称） | **✔ 正确** |
| `BackgroundPattern.==` | id（与 hashCode 对称） | **✔ 正确** |
| `sameAs`（:365/:426） | id，文档注「**用于去重**」 | **✔ 对其用途正确** |
| **`DisplayPrefs.==`（display_prefs.dart:970）** | **调 `sameAs`（id-only）** | **✘ 用错了判据** |

⇒ **不是「缺少一个判据」，而是「正确的判据已经写在同一个 crate 里、调用方却用了更弱的那个」**。
`display_prefs.dart:970` 一行改成 `backgrounds[i] != other.backgrounds[i]`
（或 `!(backgrounds[i] == other.backgrounds[i])`）**即完全修复**，无新增代码、无 API 变更。
⇒ **修法①核实通过**（B0072 提出的，依据当时未读的 `==`；本批读了，成立）。

### 为什么这让「潜在」这个定级仍然成立（且不会恶化）
- 不可达性依据不变：`appearance_background.dart:1264`
  `if (image != null && styleChanged != null)` ⇒ 无回调时样式控件**不渲染**（B0034 已核）；
- 但**接线成本现在明确等于 1 行**（把 `sameAs` 换成 `==`），
  而「接上样式控件」是常规改动（B0034：全部控件由各自 section 的构造器决定）⇒
  **两个各自很小的改动叠加，就得到一个在线的静默丢弃**。这正是模式 C 的典型形状。
建议（不变，但更具体）: ① `display_prefs.dart:970` 改用逐项 `==`（**一行**）；
② 或者，若坚持用 `sameAs`（因为去重语义确实需要它），就**给它改名**（`sameIdentity`），
让两种用途在名字上分家——**第三次复用时人就会看见**。
**未核实**：`BackgroundImage.==`（含逐图样式）**是否有测试**。若有，说明测试存在但
`DisplayPrefs.==` 没被同样对待；若没有，两处都该补。

---

## BATCH-0074（2026-09-28）· ⚠ **又一处自我更正**（B0070 的备忘错了一半）+ **F-0074-01（P2）**

### 更正一：`DisplayPrefs.hashCode` **确实含** `backgrounds` —— 我 B0070 的备忘**是错的**
B0070 我读了 `display_prefs.dart:975-994`，写下一条备忘：
> 「`display_prefs.dart:975-994` 的 `hashCode` **完全不含 `backgrounds`**，而 `==`（:968-971）含。」
**错**：`:988-1004` 的第二组里写着 `...backgrounds`（**:1003**）。我当时只读到 :994 就下了关于整个
`hashCode` 的结论。**又一次「读片段 → 断言整体」。**

### 更正二：`==` 与 `hashCode` 的字段清单**当前是一致的**（我原以为已漂）
逐项对完（`==` :943-972 ／ `hashCode` :977-1005）：
`==` 比 22 个标量 + `backgrounds`（经 `sameAs`）+ `stagePlaylist`；
`hashCode` 两组合计覆盖**同样这些字段**，且 `:1003` 的 `...backgrounds`、`:986` 的
`Object.hashAll(stagePlaylist)` 都在。
⇒ **两份手维护清单目前没有漂移**，「加字段忘了改 `==`」是**未来**风险而非**现状**缺陷。
⇒ 这条**不是**发现；但它把「两份手写清单」这个风险从「已漂」降级为「靠人守」，
与 F-0074-01 的测试缺口**指向同一处**，故合并记在下面。

### F-0074-01 · P2 【全应用的变更检测判据 `DisplayPrefs.==` **零测试覆盖】，且它委托的判据也没测】
file: shell/flutter/lib/settings/display_prefs.dart:941-973（`==`，22 个标量 + 两个列表）
调用链（它是**每个**偏好变更的总闸**）:
`shell_prefs.dart:145-146`
```dart
void _updatePrefs(DisplayPrefs next) {
  if (next == widget.prefs) return;        // ← **没变就整条丢掉**
  …
  final bool saved = widget.onPrefsChanged(next);
```
⇒ 主题 / 音量 / 缩放 / 口型灵敏度 / 背景 / 轮播……**所有**本机偏好都经这道闸。
摘录（测试覆盖的实测结果，两条 grep 均**零命中**）:
```
$ grep -rn "prefs ==|prefs !=|== prefs|equals(prefs)|isNot(equals" shell/flutter/test/*.dart   → 无命中
$ grep -rn "sameAs" shell/flutter/test/*.dart                                                     → 无命中
```
唯一碰到 `BackgroundImage ==` 的用例（`background_store_test.dart:58`）：
```dart
expect(r.prefs.backgrounds.single, const BackgroundImage(id: 'bg0000001'));
```
**两侧都是默认 `opacity`/`fit`/`align`** ⇒ 这条测试**在 `==` 丢掉三个样式字段时照样绿**
⇒ 对本轴而言它**结构上无法失败**。
影响: 测试层选择（**P2**）：
1. 全应用的总闸 `==` **没有任何**相等/不等断言；
2. `==` 把逐项比较**委托**给 `sameAs`，而 `sameAs` **零测试**；
3. 唯一相邻的测试用默认值，**对本轴无法失败**；
⇒ 三层叠加 ⇒ F-0034-01 那条「纯样式变更被静默丢弃」的洞，**没有任何测试会拦住**，
而它恰好是**模式 H（难形态未被钉住）**的教科书实例。
建议: ① 补两条**聚合层**断言（不是值类型层）：
   「仅逐图 `opacity` 不同 ⇒ `prefsA != prefsB`」与「全字段相同 ⇒ `==`」；
② 补 `sameAs` 的两条：同 id 判等、**同 id 但样式不同仍判等**（把它的「去重语义」钉成有意为之，
   免得第三次被当作通用判据复用）。
**为什么定 P2 不定 P1**：当前不可达（`appearance_background.dart:1264` 样式控件无回调时不渲染），
且需要一次后续改动才会在线；但**总闸无测试**这件事本身是**当前就成立**的，
且影响面是「所有偏好变更」，不是某一个字段。

### ⭐ 本批提炼：**第三次「读片段 → 断言整体」**（B0070 证伪、B0070 备忘、本批更正二的前身）
三处都在同一区域、同一形状：
| 批次 | 我读了 | 我断言了 | 实际 |
|---|---|---|---|
| B0070 | `background_item.dart` 部分 | 「样式在独立字段」 | **在 `BackgroundImage` 内**（B0072 推翻） |
| B0070 | `display_prefs.dart:975-994` | 「hashCode 不含 backgrounds」 | **含**（`:1003`） |
| B0074 | `display_prefs.dart:943-994` | 「两份清单已漂」 | **未漂**（`:1003`/`:986` 都在） |
⇒ **规则（模式 L 执行规则 3）**：
> **对「一份字段清单/一组分支/一个函数」的完整性断言，必须读到它的结尾标记**；
> **只要结论是「没有 X」，就必须找到 X 的搜索终点**（`}` / `];` / 文件末），
> 否则「没找到」与「不存在」不可区分。
⇒ 这与 B0070 的「必须核发送前还有没有一层」是同族：都是**「没看到」≠「没有」**。

---

## BATCH-0075（2026-09-28）· `DisplayPrefs` 三处手维护清单**机械对账**：当前一致 · **0 条新发现**

> **0 条新发现。** 上一批把「加字段忘了改 `==`」记成「靠人守」的风险；
> 本批按规则 3（**没看到 ≠ 没有**）**机械**核一遍，而不是抽查。

### 核验方法（可复用的手法）
把三个清单**各自读到底**后做集合差：
```python
fields = 正则扫构造函数字段          # 24 个
eqf    = set(扫 "other.X !=" )        # == 的 22 个标量
pset   = set(扫 copyWith 形参 )       # 25 个（含 clearStageImage 合成标志）
```
⇒ `== 比、copyWith 无` = **∅**；`copyWith 有、== 不比` = `{backgrounds, stagePlaylist}`
（这两个**确实在 `==` 里** —— `backgrounds` 走 :968-971 的 `sameAs` 循环、
`stagePlaylist` 走 :972 的 `_sameList`，只是不作为 `other.x != x` 标量出现）。
⇒ **`==` / `copyWith` / `hashCode`（B0074 已核 :1003 含 `backgrounds`）三处当前完全一致。**

### 我在过程中犯了一次「解析失败当成结论」的错（当场纠正）
第一次跑我的正则用了旧 Dart 风格 `name: value == null`，而代码用现代 `Type? name,`
⇒ 解析出 **0** 个参数，差集显示「22 个字段 copyWith 全都没有」。
**按规则 3 的精神，「解析出 0」同样不是「不存在」的证据** ⇒ 改用正确模式重跑，
得到上面的 ∅。**若我没警觉，这条「copyWith 缺 22 个字段」的假发现就会进账本。**
⇒ 规则 3 补一句：**「我的工具解析出空/异常」也要按「没看到 ≠ 没有」处理。**

### 结论的准确表述（不夸大）
- **不是**发现：清单**当前一致**，无缺陷可报；
- 但与 **F-0074-01** 合起来构成一条值得记的事实：
  **「三处手维护清单今天是对的，而且没有任何测试会在它们开始漂的那天告诉你。」**
  ⇒ 这是**风险陈述**（不是缺陷陈述），修法与 F-0074-01 相同：补一条
  「构造两个只差一个字段的 prefs ⇒ `!=`」的参数化测试，
  **字段数增加时它自动覆盖新字段**（参数化枚举字段名，比手写 22 条断言更抗漂）。

---

## BATCH-0077（2026-09-28）· `readBackgrounds` / `_readBackgroundSource` 本体 —— **0 条新发现**（两个正面样本）

> 结掉 B0076 留的唯一尾巴。核完即**离开 `display_prefs.dart`**（本区域已连续 5 批零发现 + 2 次自我更正，
> 按停止规则换根）。

### 正面样本一：**一次带根因诊断的迁移修复**（`_readBackgroundSource` :644-659）
```dart
/// 写成「读旧键」而不是「改旧键的默认值」：**改默认值会让「显式设过 true」和「从没设过」分不开**，
/// 而那正是本缺陷的成因。
static int _readBackgroundSource(Map<String, Object?> json) {
  final Object? stored = json['backgroundSource'];
  if (stored is int) {
    return stored == backgroundSourceStageImage ? backgroundSourceStageImage : backgroundSourceLibrary;
  }
  final bool legacySync = _readBool(json['syncShellStageBg'], false);
  final String? stage = _readImage(json['stageImage'], kStageImageMaxChars);
  return legacySync && stage != null ? backgroundSourceStageImage : backgroundSourceLibrary;
}
```
三个可核的要点：
1. **未知值不生效**：非 `backgroundSourceStageImage` 的一律回落 `backgroundSourceLibrary`
   ⇒ 落盘垃圾值**进不了**运行态（与 B0076 核到的读时钳位同一纪律）；
2. **迁移走「读旧键」而非「改旧键默认值」**，理由写清：**改默认值会让「显式设过 true」
   与「从没设过」不可区分** —— 这是**根因层面的设计约束**，不是补丁；
3. 旧键 `stageImage` 的读取**带长度上限**（`kStageImageMaxChars`）⇒ 迁移路径也有界。

### 正面样本二：**判据单一化，并写明它防的是哪一类不一致**（`effectiveBackground` :661-675）
```dart
/// 壳当前**实际会画**的那一项（判据只有这一处）。
///
/// 渲染层不再自己判来源——**判据散到两处就会出现「设置说用背景库、画的不是背景库」这类不一致**。
BackgroundItem? get effectiveBackground { … }
```
⇒ 不只声明「单一真源」，还**点名了它防的那类具体症状**（这比「请保持一致」有用得多）。
同函数 :668-669 还记了一个**性能决策**并给出理由：舞台那张用**固定 id**
（`kStageImageItemId`）而非内容哈希，「免得**每帧**重算一张几 MB 的哈希」
⇒ 与 B0071 核的 djb2 指纹正好构成一对：**该用内容指纹的地方用它（持久标识），
不该用的地方用固定 id（每帧路径）**，两处都写了理由。

---

## BATCH-0078（2026-09-28）· 「唯一漏斗」声明**核实为真**（第三次换根后的第一个结论）—— **0 条新发现**

> 核的问题很具体：`shell_prefs.dart:133-137` 自称「`_applyPrefs` 正是这条『已生效』的
> **唯一漏斗**」—— 它**真的唯一吗**？有没有第二条路径能改到舞台/壳的状态而绕过它。

### 核验一：调用点**恰好三处**（定义 + 两条真调用），无第四条
`grep -rn "_applyPrefs" shell/flutter/lib/`：
- `shell_prefs.dart:76`（定义）
- `shell_prefs.dart:158`（`_updatePrefs` ⇒ 每次偏好变更）
- **`main.dart:1207` `onReady: _applyPrefs`**（首次挂桥 / iframe 重建后就绪）

### 核验二：声明**自己划了作用域**，而不是笼统说「唯一」
`main.dart:496`：
> 「`clickEnabled` 四个字段**只有** `_applyPrefs` 会发（`_attach` 的 sync …）」
⇒ 舞台组件**自己**确实也直写桥（`live2d_stage.dart:259/264/285/347/356/472/498/501/518`
的 `sendSync` / `sendStageBg`），所以「唯一漏斗」**不是**对全部舞台状态成立；
它成立的范围是**那四个字段**，而代码**明确列出了是哪四个**。
⇒ 这是**精确的边界声明**，不是虚假的全局唯一性主张。

### 核验三：这条链**有两个测试在记它**，且其中一个是**回归**
- `test/action_scales_wiring_test.dart:8`：「`_applyPrefs → _syncActionScalesNow(force: true)`，
  `force` 绕过去重 ⇒ …」
- `test/stage_color_motion_test.dart:6`：「过去 `_applyPrefs` 一按就下发终色」——
  **一个真实缺陷的回归**（立刻下发终色会**杀掉过渡动画**），
  修复方向是「把终色留给渲染面自己算」（与 B0045 核到的「消息只改状态、帧循环出效果」同一条纪律）。
⇒ 漏斗链**从声明到实现到回归**三处对齐。

### ⚠ 第 5 次同类失误（新的变体）：**我的 grep 假设了调用形态**
第一次跑 `grep "_applyPrefs("`（**带左括号**）只得到 2 处，**漏掉了 `main.dart:1207` 的
`onReady: _applyPrefs`** —— 那是 **Dart 的方法 tear-off（无括号传递）**。
若我据此下结论「`onReady` 并未接上 `_applyPrefs`，所以『首次靠 onReady』这句是假的」，
就会记下一条**方向完全相反**的假发现。
⇒ **模式 L 执行规则 3 再补一个变体**：
> **查「某函数被谁调用」时，模式必须覆盖「调用 / tear-off / 作为回调字段传递」三种形态**；
> 只匹配 `name(` 会**系统性地**漏掉后两种，而它们恰恰是**接线**最可能出现的地方。

---

## BATCH-0087（2026-09-28）· ⭐ **F-0034-01 从「潜在」升级为「在线」：P3 → P2**

> **本批 0 条新缺陷，但把一条既有 P3 的定级与定性都改了。** 触发点：B0034 我写「潜在」时
> 依据的是「`appearance_background.dart:1264` 无回调时样式按钮不渲染」——
> **我只核了按钮的渲染条件，没核那个回调在生产里到底有没有被接上。** 本批顺着接线**走到根**。

### 接线的完整证据（每一跳都有行号）
```dart
// ① appearance_background.dart:392-398（**生产代码，不是测试**）
// 逐图样式（§5.3 第 2 条）：只改这一项的覆盖，列表其余原样。
onItemChanged: (int index, BackgroundImage next) {
  if (index < 0 || index >= items.length) return;
  final List<BackgroundItem> list = List<BackgroundItem>.of(items);
  list[index] = next;                                   // ← 换掉**这一项**，id 不变
  onChanged(prefs.copyWith(backgrounds: list));         // ← 走 onChanged
},
```
```dart
// ② :957-961  样式回调由 onItemChanged 派生（有它才有样式编辑）
onStyleChanged: widget.onItemChanged == null ? null : (index, next) => widget.onItemChanged!(index, next),
// ③ :1263 / :1301  样式按钮的渲染条件
if (image != null && styleChanged != null) …
// ④ :1194  回调的**类型**就是产出 BackgroundImage
final ValueChanged<BackgroundImage> onStyleChanged;
```
```dart
// ⑤ shell_prefs.dart:145-146（`_updatePrefs`，所有偏好变更的总闸）
void _updatePrefs(DisplayPrefs next) {
  if (next == widget.prefs) return;                       // ← 判「没变」就在这里
```
```dart
// ⑥ display_prefs.dart:968-971  这个 == 怎么比 backgrounds
if (backgrounds.length != other.backgrounds.length) return false;
for (int i = 0; i < backgrounds.length; i++) {
  if (!backgrounds[i].sameAs(other.backgrounds[i])) return false;   // ← **id-only**
}
```
### 为什么「只改样式」一定被判成「没变」
- 新旧两项的 **id 相同**（样式是覆盖，不是新图；`backgroundIdOf` 是**内容指纹**，
  而 `dataUrl` 未变 ⇒ id 必然不变，B0070/B0071 已核）；
- `BackgroundImage.sameAs` = `other.id == id`（`background_item.dart:365-366`）
  ⇒ `==` 在 `backgrounds` 这一项上**判等**；
- 而逐图样式（`opacity` / `fit` / `align`）**只存在于 `BackgroundImage` 内部**
  （B0072 已核 `background_hydration.dart:134-135` 的原文：「直接 `new BackgroundImage(id: …)`
  会把用户给这张图设的 **opacity / fit / align** 一起清空」），
  且 `DisplayPrefs` **没有**任何独立的样式存储（本批核：`styleOverride` / `perItemStyle` /
  `itemStyles` 三个候选名**零命中**）⇒ **没有第二个字段能让 `==` 判出差异**。
⇒ **用户调逐图样式 ⇒ 界面不报错、不保存、无变化。**

### 升级后的表述
**F-0034-01 · P2**（原 P3、定性「潜在」）：**在线的静默丢弃**。
- 触发：外观设置 → 背景库 → 选一张图 → 改它的透明度/适配/对齐（§5.3 第 2 条的正式功能）
- 现象：**什么都不发生**（没有报错、没有 toast、没有视觉变化），
  而用户以为改了 —— 与 F-0036-01（口型/缩放静默不刷新）**同一族**
- 修法（不变，仍是一行）：`display_prefs.dart:970` 的
   `!backgrounds[i].sameAs(other.backgrounds[i])` ⇒ 改用 `backgrounds[i] != other.backgrounds[i]`
   （`BackgroundImage.==` 已含三字段且与 `hashCode` 对称，B0073 已核）
- 测试（与 F-0074-01 同一处）：补**聚合层**用例「仅逐图 `opacity` 不同 ⇒ `prefsA != prefsB`」

### ⭐ 本批的方法论教训（第三次，且最贵的一次）
我给这条发现打「潜在」标签时，用的是「按钮的渲染条件里回调可能为 null」。
**但「条件写了这个判断」≠「运行时它就是 null」** —— 我核了**条件**，没核**那个值的来源**。
三次同族失误的**共同形状**：
| 批次 | 我核了 | 我没核 | 后果 |
|---|---|---|---|
| B0070 | `sameAs` 只比 id | id 的来源 | 假证伪（样式在 `BackgroundImage` 内） |
| B0076 | 内联 `json['key']` | 辅助函数 `readBackgrounds()` | 差点记 P1 级假发现 |
| **B0087** | 按钮渲染**条件** | 回调**值的来源**（是否在生产接上） | **把在线缺陷记成潜在**（低估） |
⇒ **规则 3 再补一个变体**：
> **「条件依赖某值」时，必须把那个值追到它的**赋值点**并确认它在生产路径上非空；
> 只读条件表达式 = 只证明「它可能是 null」，不证明「它是 null」。**
> 方向上要特别小心：这类错误既可能**高估**（以为接线了其实没有），
> 也可能**低估**（以为没接线其实接上了）—— **本批是后者，而后者更危险，
> 因为它让一个在线缺陷以「潜在」的名义留在账本里。**

---

## BATCH-0091（2026-09-28）· 第四次换根进 `crates/l2d/` —— **F-0046-01 的机制被削尖**（同一攻击、更深的根因）

> **0 条新缺陷**，但把 F-0046-01（P1）的**修法位置**从「加 origin 校验」修正为
> **两个位置，且其中一个在更早的层**。本批读的文件：`crates/l2d/src/lib.rs`(90) + `net.rs` 两函数。

### 核验一：crate **确实有**路径加固，而且写得很严（正面）
`crates/l2d/src/lib.rs:19-20` 与 `asset/mod.rs:14-16 / 52-54 / 73`：
```rust
/// 路径安全：清单里的每个相对引用都先经 [`normalize_resource_ref`] 规范化
/// （折叠 `.` 与连续 `/`；**拒绝绝对路径、`\` 分隔符与任何 `..` 上跳段**，
/// 错误携带清单里的原始资源名）。
…
"皮套资源相对路径非法（禁止绝对路径、`\\` 与 `..` 上跳）: {name}"
```
⇒ **拒绝 `..`、绝对路径、反斜杠**三件套，且错误**携带原始资源名**（可定位）。

### 核验二：⭐ 但**这层加固在抓取之后**，因此**挡不住 F-0046-01 触发的抓取**
`crates/l2d-wasm-demo/src/web/net.rs:52-63`：
```rust
pub(crate) fn resolve_relative(model_url: &str, rel: &str) -> String {
  let dir = match model_url.rsplit_once('/') { Some((dir, _)) => dir, None => "" };
  let rel = rel.strip_prefix("./").unwrap_or(rel);
  if dir.is_empty() { rel.to_owned() } else { format!("{dir}/{rel}") }   // ← **纯文本拼接**
}
```
**无 `..` 归一、无 scheme 检查、无任何校验。**
而 `main.rs:170-175` 的顺序是：`extract_refs` 解析**刚抓回来的**字节 →
对每个 ref `resolve_relative` → **立刻 `fetch_bytes`** ⇒ **`ModelPackage::from_memory_map`
（`asset/mod.rs` 那个加固点）在 :182，排在全部抓取之后。**
⇒ 攻击者给的 model3.json 里写 `"Textures": ["../../../../api/v1/settings"]`，
**浏览器就会去抓 `http://127.0.0.1:18080/api/v1/settings`**，
而 `asset/mod.rs` 之后才把这条 ref 判为非法 —— **那时请求早已发出**。

### 对 F-0046-01 的**修正与削尖**（不改定级，仍 P1）
| | 原记录（B0046/B0047） | 本批削尖后 |
|---|---|---|
| 入口 | V1 `?model=`（零 JS）· V2 postMessage 无 origin 校验 | 不变 |
| 抓取对象 | 攻击者给的 model URL + **其内容引用的 N 个相对资源** | **明确：这 N 个相对路径也完全由攻击者控制，且 `resolve_relative` 不校验** |
| 危害 | 盲抓（读不到响应） | **盲抓 + 路径可上跳**：`..` 不能换 host（`dir` 前缀恒在），但**能在同一 host 上走到任意路径** ⇒ 攻击者页面可让受害者浏览器对本机服务发任意 GET |
| 修法 | ① wasm 侧加 `origin` 校验 ② 加 `X-Frame-Options`/CSP | **增补 ③（关键）**：**在 `net.rs` 抓取之前**校验 ref（复用 `asset::normalize_resource_ref` 的规则或等价物），否则 crate 的加固是**马后炮** |
⇒ **定级不变（P1）**：仍是盲抓、仍无外泄（响应进的是受害者自己浏览器的纹理缓冲）。
但**修法少一条就无效** —— 只加 origin 校验而不校验 ref 路径，攻击者仍可用 V1（`?model=`，**根本不走 postMessage**）驱动同源路径抓取。

### 我试过论证而**不成立**的两点（防止把危害说过头）
- **「`rel` 可以是绝对 URL 从而换 host」** —— **不成立**：`dir` 前缀恒在，
  `rel = "http://evil.com/x"` 会拼成 `http://<dir>/http://evil.com/x`（仍是原 host 的畸形路径）。
- **「`//evil.com/x` 协议相对能换 host」** —— **不成立**：同样被 `dir` 前缀吞掉。
⇒ **危害严格限定为「同 host 路径上跳」**，本条**不夸大成 SSRF 到任意主机**。
（这与 F-0046-01 原记录「盲抓」的定性一致，此处只是把「能走哪些路径」说清楚了。）

---

## BATCH-0098（2026-09-28）· `field_map` 通道白名单核验通过 + **一处文档不一致（P3，安全方向）**

### 核验一：白名单是**单一真源**，且**结构性地禁止动作写口型**
- 定义在 `preset/table.rs`，经 `preset/mod.rs:692` `pub use table::{ALLOWED_PARAMS, …}` 再导出
  ⇒ `field_map.rs:22` `use super::{ALLOWED_PARAMS, BODY_LIMIT, EXPRESSION_LIMIT, HEAD_LIMIT, …}`
  是**消费**而非**复制** ⇒ **无第二份清单**。
- 机械计数：**`ALLOWED_PARAMS` = 13 条**（合 `whitelist_stays_the_documented_thirteen` 的断言）
- **`ParamMouthOpenY` 不在其中**（机械确认 `False`）；`FACIAL_PARAMS`（`field_map.rs:27-38`）7 条
  且**不含**口型、不含任何 `ParamAngle*` / `ParamBodyAngle*`。
⇒ **红线 O 的第三层保险**：服务端（静音 ⊥ volume，B0006）→ 渲染面（口型参数只来自 RMS，B0045）
→ **协议侧（动作字段表根本不允许写口型通道）**。
⇒ 与 B0043 核的 core 侧 `ParameterMask`（「未置位一律不写」）**同构**：
一个在 core 用位掩码表达所有权，一个在协议用白名单表达 —— **两处都把「口型归 TTS」写成了结构**。

### F-0098-01 · P3 【同一个 crate 用两种说法描述同一契约：一边说「静默降级」，一边说「缺参数不得静默」】
file: crates/l2d-wasm-demo/src/preset/mod.rs:58 与 :696
摘录（说**静默**的两处）:
```rust
//! 渲染层静默降级（未知参数 `override_parameter` 返回 `false`，不报错）。      // :58
…
/// 写 `final_override`；模型缺该参数返回 `false`（静默降级）。              // :696
```
而**同一个 crate 的实现侧说的是反面**（`field_runtime.rs:28-29`，且 B0080 已核其行为）:
```rust
//! - **缺参数不得静默**：`PresetSink::override_param` 返回 `false` → 发
//!   `preset-applied` + `degraded=true`，§7.1 / §10 #9）。
```
影响: 可维护性（**方向是安全的**）：实际行为**更响**（发 `preset-dropped` 或
`preset-applied` + `degraded=true`，B0057 已枚举到回归
`missing_param_emits_preset_dropped_instead_of_silent_noop`），
而 `:58`/`:696` 把它描述成「不报错」。⇒ 三种可能后果：
① 读者以为最坏是静默 ⇒ 可能**加一层并不需要的兜底探测**；
② 读者**不信任** `preset-dropped` 信号（以为它不会来）；
③ 同一 crate 内两处说法**互相矛盾**，读者不知道信哪个。
建议: 把 `:58` 与 `:696` 的措辞改成与实现一致，例如
「模型缺该参数 → `override_param` 返回 `false`；**调用方据此发 `preset-dropped`
（或 `preset-applied` + `degraded=true`），不会静默**（见 `field_runtime.rs` 头注）」。
**为什么定 P3 而不是 P2**：它**低估**了实现的保证（安全方向），
不影响任何运行时行为；与模式 D（注释**高估**保护 ⇒ 危险）方向相反。
**同时记一条对我自己模式表的修正**：
> **模式 D 只跟踪了「注释高估」这一个方向。** 实际上注释错误有**两个方向**：
> **高估**（声称的保护不存在 ⇒ 漏检）与**低估**（声称的降级不存在 ⇒ 冗余兜底 / 不信任信号）。
> **高估危险、低估安全，但两者都会让读者对代码行为做出错误预测。**

---

## BATCH-0099（2026-09-28）· 外置覆盖表**无法突破白名单** —— 红线 O 第三层保险在**两条路径**上都成立

> **0 条新发现**。本批核的是一个能让上一批那条保险**失效**的具体可能：外置
> `assets/actions/field_map.json`（`with_override_json`，按 model id 分节）能否引入 13 条之外的通道。

### 核验一：覆盖表路径上有**两道独立的闸**，每道拒绝都**带 warning**
`field_map.rs::parse_targets`（:321+）对外部 JSON 的**每一条**目标：
```rust
if !ALLOWED_PARAMS.contains(&param) {                                  // :340
    self.warnings.push(format!("field_map: {param} 不在 13 参白名单内，已丢弃"));  // :342
    continue;
}
if !field_allows(field, param) {                                        // :345
    self.warnings.push(format!("field_map: {} 字段不允许写 {param}（**通道红线**），已丢弃", …));  // :347
    continue;
}
```
（另有「不是数组」「缺 param」「缺 amount」各自带 warning 拒绝。）
⇒ **两道闸互相独立**：第一道管「是否在 13 条内」，第二道管「这个字段能不能写这个通道」。
⇒ 覆盖层无法把 `ParamMouthOpenY` 塞进来（不在 13 条内），也无法让 `body` 写到五官通道。

### 核验二：第二道闸的实现**刻意避开了更松的写法**，并把理由写在原地
`field_allows`（:144-152）：
```rust
Field::Body => scale_class(param) == ScaleClass::Body,
Field::Head => scale_class(param) == ScaleClass::Head,
// 顺序要紧：FACIAL_PARAMS 是显式小词表——不能只判
// scale_class == Expression（**那会把 ParamMouthOpenY / ParamBreath 放进来**）。
Field::Expression => FACIAL_PARAMS.contains(&param),
```
⇒ **这是本审计见到的第 4 个「显而易见的实现会踩红线、他们选了更严的、并写下为什么」样本**：
`normalize_resource_ref` 遇 `..` **拒绝**而非归一（B0092）· `guess_format` **先查长度**再索引（B0095）·
`alpha_coverage` 空输入**显式返哨兵**而非 panic（B0094）· 本条 `FACIAL_PARAMS` 显式小词表。
⇒ **可提炼为正面模式 P3**（下条对账正式立档）：**「更松的写法在哪里」要写进注释**——
因为下一个维护者最容易写的正是那个更松的版本，而**他不会想到它踩的是哪条红线**。

### 对 F-0098-01 的**补强**（第三处「静默」措辞被证伪）
本批的 `parse_targets` 拒绝时**明确 push warning**（:342 / :347），
而 `mod.rs:58` / `:696` 说那是「静默降级」⇒ **不成立的第三处**，
且正确描述应是「**丢弃 + 记 warning**」（字段路径另有 `preset-dropped`，B0080 已核）。
⇒ F-0098-01 的建议**据此补全**：修 `mod.rs` 两处措辞时，
措辞应写成「**被丢弃并记 warning**（`field_map` 路径）/ **发 `preset-dropped` 或 `applied+degraded`**
（`field_runtime` 路径）」—— **两条路径的「不静默」形式不同**，这才是准确的描述。

---

## BATCH-0100（2026-09-28）· 覆盖表的**可达性**与缺文件降级 —— 核验通过，并**第四次证伪 F-0098-01 的「静默」措辞**

> **0 条新发现**。本批回答一个可达性问题并核一条降级路径。

### 核验一：**代码路径活着，但仓库不带数据；缺文件是默认态**
- `assets/actions/` 在树里只有 `preset_labels.json` 与 `presets.json`
  ⇒ **`field_map.json` 不在树里**（`git ls-files | grep actions/` 实测）
- 而 `with_override_json` 在**生产**被调（`l2d-wasm-demo/src/main.rs:236`）
- 头注已声明这是预期状态（`field_map.rs:9-12`：「本轮**不要求**每皮套一份，
  所以内建默认表就是 bai 皮套的完整映射」）
⇒ **结论**：覆盖路径是**给用户准备的**扩展点（用户可自备 `field_map.json`），
**出厂缺它是常态而非故障**。

### 核验二：缺文件/坏文件的降级**逐层可读**（`main.rs:228-248`）
```rust
let bytes = match net::fetch_bytes(window, FIELD_MAP_URL).await {
    Ok(b) => b,
    Err(e) => return format!("字段映射覆盖表未加载（{e}），**用内建默认表**"),        // :230
};
let text = match String::from_utf8(bytes) {
    Ok(t) => t,
    Err(e) => return format!("字段映射覆盖表不是 UTF-8（{e}），用内建默认表"),      // :234
};
match FieldMap::with_override_json(model_id, &text) {
    Ok(map) => { … if warnings.is_empty() { … } else {
        format!("字段映射覆盖表：model={model}（外置；{} 条告警：{}）",
                warnings.len(), warnings.join("；")) } }                         // :244-247
```
⇒ **三层降级，每层都带原因**；覆盖生效时**告警条数与内容**也上屏。
⇒ **边界澄清（防止与 F-0046-01 混淆）**：这里的 `fetch_bytes` 用的是
**编译期常量** `FIELD_MAP_URL`，**不是**攻击者可控的 URL ⇒ B0091 的顾虑**不适用**于这条调用。

### 对 F-0098-01 的**第四次补强**：实际降级**没有一条是静默的**
已找到四处与「静默降级」矛盾的实际行为：
1. `field_runtime.rs:28`（B0080 核）—— 字段路径发 `preset-dropped` / `applied+degraded`；
2. `field_map.rs:342/347`（B0099 核）—— 丢弃时 `push` warning；
3. `main.rs:230/234`（本批）—— 缺文件/坏文件**各带原因回退**；
4. `main.rs:244-247`（本批）—— 告警**条数与内容**上屏。
⇒ `mod.rs:58` 与 `:696` 的「静默降级」是**本 crate 内唯一**声称静默的地方，
而**四处实际行为都不静默** ⇒ 该措辞不是「过时」，是**与实现相反**（F-0098-01 的原判成立并加强）。

---

## BATCH-0103（2026-09-28）· `release-build.yml`（门禁轴收尾）—— 一条 P3 + **本审计见过最高的事故记录密度**

### 核验（正面）：两个事故记录**带 run ID、带日期、带「不要再写未证实结论」的告诫**
**① Linux apt 步（:82-99）**
> 「2026-09-11 起这步在 Linux 上以 **exit 100** 反复失败（**runs/34659978040、34660260293**）。
> **已排除「包名不存在」**……而且收窄到两个包之后**同样失败**，所以**根因至今未确定**——
> **不要再写「多余包导致失败」这类未经证实的结论**。」
> 「因此这一步把**判定依据**打进日志，并且**不用 apt 的退出码当成功判据**」
⇒ 每个诊断都写明**它区分哪两种可能**（`lsb_release` 坐实镜像；update 前后各打一次
`apt-cache policy` 区分「索引本来不全」与「update 弄坏了」；`Candidate` 硬校验避免拖到编译期报天书；
`pkg-config --modversion alsa` 回答「构建期真能找到吗」）。

**② `+crt-static` 的移除（:116-132）**：记录移除日期（2026-09-12）与「此前从未验证过」，
并诊断**两个独立问题叠加**：① `RUSTFLAGS` 全局注入会命中过程宏（`async-recursion` 报
`cannot produce proc-macro`）② `+crt-static` → `-static-pie` → 需要 `libasound.a`，
而 Ubuntu `libasound2-dev` **只提供 `libasound.so`** ⇒ 指出正解是 musl 目标并说明为何不在本 RC。
另 `defaults: run: shell: bash`（:42-44）带 **2026-09-11 windows PowerShell 失败**的诊断。
⇒ 这些注释**可被任何人按 run ID 复核** ⇒ 与我 B0056 提炼的记账卫生判据（「写时核对过的注释至今都真」）
**完全吻合**；这是该判据在**仓库里**的最强证据。

### F-0103-01 · P3 【发布构建**本身不跑任何测试**：直接推 tag 可绕过全部门禁】
file: .github/workflows/release-build.yml:16-19（触发器）+ :133-136（唯一的构建步）
摘录:
```yaml
on:
  push:
    tags: ["v*"]              # :17-19
…
- name: Build desktop binary (release)
  run: cargo build -p live2d-ai-desktop --release      # :134 —— 无 test / fmt / clippy / rust-ratio
- name: Verify binary exists
  run: ls -lh target/release/${{ matrix.bin_name }}   # :136
```
影响: 门禁完整性（**范围有限，如实标注**）：
- 正常流程下 **tag 来自已过 `pr-checks.yml` 的提交** ⇒ 用户下载的工件对应一个**过了全部门禁**的代码；
- 但触发器是 **`push: tags`**，**不需要 PR、不需要 review** ⇒ **任何人（或任一有 tag 权限的
  自动化）直接推一个 tag，就会产出一个从未跑过 `cargo test` / `clippy` / `rust-ratio`
  / `flutter test` 的发布工件**，而 workflow **不会失败**（`cargo build` 成功即成功）。
- 与 AGENTS.md「一张表，一套真相」的对照：那张表把门禁列在 `pr-checks.yml` / `nightly.yml` /
  `flutter-checks.yml` 名下，**本 workflow 不在表里** ⇒ 它**不是**门禁，是产物构建器
  ⇒ **不构成「表与实现不符」**，但**「tag 是发布入口」这一事实没有对应的门禁说明**。
建议: 在本 workflow 的头注或 AGENTS.md 的表旁补一句（如选 ② 之外的 ①）：
**①** 加一个轻量闸（例如 `cargo test -p live2d-ai-desktop --lib` 或至少 `clippy`），
   或 **②** 明确写「**tag 只能由已合并的 PR 产出**（人工纪律，CI 不强制）」——
   两条都比现在的**沉默**好，因为现在的沉默会让读者以为「发版是门禁过的」。
验证: 读 release-build.yml:1-142（头注、触发器、matrix、全部 run 步骤）；
对照 B0048 的 workflow 穷举（5 个 workflow 的全部 `run:` 步骤里**无** test/clippy/rust-ratio）
置信: 高
反证: (a) 试过论证「`nightly.yml` 的定时任务覆盖了」——**不成立**：nightly 跑的是 `main` 的
当前状态，**不是**被 tag 的那个 sha；tag 指向的提交若未经 PR，nightly 不构成保证；
(b) 试过论证「这是业界惯例、不算缺陷」——**同意它是惯例**，所以定 P3 且**范围写明**：
问题不是「不跑测试」这个选择，而是**「tag 是发布入口」这件事没有任何一处文字说明它不受门禁保护**；
(c) 试过找它是否声明了什么豁免——头注只声明了「桌面壳/Android 壳不在此矩阵」与「wasm 可选」，
**没有**关于门禁的说明 ⇒ 「沉默」这个判断成立。

---

## BATCH-0107（2026-09-28）· ⚠ **自我更正 B0104 的一处「唯一」断言**（`head` 截断的第二次代价）

> **0 条新发现**，但**我必须更正自己 B0104 写下的一个关于全仓的结论** ——
> 那次结论建立在一次**被 `head` 截断**的枚举上。

### 我 B0104 写错的地方
原文：「**唯一**带**真实 HTTP 客户端**的 Mod（director）」。
⇒ **错**。穷举（`grep -rln "reqwest" crates/live2d-ai-mod-*/src/*.rs`，**不截断**）得**四个文件**：
| 文件 | 角色 | 密钥来源 |
|---|---|---|
| `director/src/staging_http.rs` | 异步二路客户端（B0106 全读） | `api_key_env` → **`secrets::lookup`** |
| **`memory/src/summary_http.rs`** | 真摘要的 LLM 客户端 | `api_key_env` → **`secrets::lookup`**（:74-80，**与 director 逐字同构**） |
| **`memory/src/summary.rs`** | 「**HTTP 实现共用**」的 reqwest blocking 客户端**工厂**（:335 `build_http_client`） | 不持密钥（只是客户端构造） |
| `local-llm/src/lib.rs` | **已废止**、未注册（B0040 核过 `AVAILABLE_MOD_FACTORIES` 不含它） | — |

⇒ 我 B0104 那次 `grep … | head -6` 恰好只露出 director 的三行，**把 memory 的两个文件截掉了**。

### 更正后的结论（**红线 R 的判定不变，且更强**）
- ❌ 不是「一个 Mod 带 HTTP 客户端」
- ✅ 是「**两个**在册 Mod（`director` / `memory`）带 LLM HTTP 客户端，
  **两者的密钥来源纪律逐字同构**：只收 `api_key_env` **变量名** → 值经
  **`live2d_ai_runtime::secrets::lookup`** → `with_api_key`（直接注入的构造**只被测试调用**）
- ✅ 加上 `runtime/performance/client.rs`（B0020 审过），**全仓三个 HTTP 客户端同一形态**
⇒ 「正面模式 P2（私有汇合 + 策略闭包）」的消费者数从 5 增至 **6**，
且**覆盖面从 1 个 Mod 扩到 2 个** ⇒ 纪律的**一致性**比我原先写的更强。

### ⭐ 这是规则 3 变体 7（`head` 截断）的**第二次代价，且这次最贵**
第一次（B0105）代价是「差点记一条假发现」；**这次代价是「我基于不完整枚举，
对全仓下了一个关于『唯一性』的结论」**。
⇒ **补一条执行要求**：
> **凡是用来支撑「唯一 / 只有 / 没有别的」的枚举，必须用 `-l` / `-c` / 无 `head` 的形式**；
> `head` 只能用于**逐条读内容**，**不能**用于**枚举后再下结论**。
⇒ 这条也解释了本审计**为什么「唯一性」类断言最容易出错**：它们**天然依赖枚举的完整性**，
而我几乎每一批都在用 `head`。

### 附带核验：`with_api_key` 在**三个** crate 里都**只被测试直接调用**
`director`（:112 生产经 `new()`；:280/:325/:333/:344 测试）·
`memory`（:81 生产经 `new()`；:236/:279/:286/:296 测试）·
`runtime/performance`（:199 生产经 `new()`；:478/:533/:561/:578 测试）
⇒ **三个 crate、同一形态** ⇒ 「显式注入密钥」的构造器**不是生产路径**。

---

### F-0111-01 · P2 【ASR token 经 argv 传给 sidecar 子进程 ⇒ 出现在 /proc/<pid>/cmdline】
> ➕ **B0222 补「可达性的确切形状」（定级仍 P2，不变）**
> `mod-voice-input/src/lib.rs:196/214` 的 `settings_spec` 把 **`sidecar_script`** 与
> **`sidecar_python`** 都声明成 **普通 String 字段**（`secret: false`、
> `sidecar_python` 的 `default: None`、label「Python 解释器（缺省 python3）」）
> ⇒ 而面板**按 kind 渲染**（B0105 已核：Bool/String/Number/Select）⇒ **它们是两个普通文本框**
> ⇒ ⇒ **用户不经改文件、不设环境变量，就能在正常设置界面里把本 Mod 指向
> 机器上任意可执行文件与任意脚本路径**；而 ASR token 随后进入**那个进程**的 argv。
> ⇒ ⚠ **但这不构成提权**（用户自己选了解释器与脚本 ⇒ 信任本来就是他的）⇒ **P2 不变**；
> **真正的问题是两条**：① **无白名单** ⇒ 打错字或好奇即可启动任意程序；
> ② **token 落在那个进程的 argv** ⇒ 同机进程可读（B0120 核过 `.env` 侧的同类问题，
> 而那里**已经**用「先 chmod tmp 再 rename」关过一次窗口 —— **这一处没有对等处置**）。

> ⚠ **本条为 BATCH-0111 的规范头**（该批最初只在散文里记了它，
> CONSOLIDATION-12 补上，以便机械计数与索引）。详细论证见该批段落。

## BATCH-0111（2026-09-28）· ⭐ **红线 R 在「进程边界」上有一条没人考虑的通道：token 进了 argv**

file: crates/live2d-ai-mod-voice-input/src/sidecar.rs:132-135
摘录:
```rust
if let Some(t) = token.map(str::trim).filter(|t| !t.is_empty()) {
    argv.push("--token".to_string());
    argv.push(t.to_string());          // ← **明文 token 成为命令行参数**
}
```
而 `commands.rs:292-295` 正是逐参数 spawn：
```rust
let mut cmd = std::process::Command::new(&argv[0]);
cmd.args(&argv[1..]);                  // :293 ← token 随 argv 进入子进程命令行
```
**接收侧已经支持更安全的通道**（`docs/examples/voice-sidecar/voice_sidecar.py:492`）:
```python
p.add_argument("--token", default=os.environ.get("VOICE_INPUT_TOKEN", ""),
               help="可选 token（env VOICE_INPUT_TOKEN；也可放 body，脚本用 Authorization 头）")
```
（`:490` 的 `--url` 同样有 `VOICE_INPUT_URL` 兜底。）
⇒ **Rust 侧却从不设这个环境变量**，一律走 argv。

影响: **安全 / 密钥泄露**：
- **暴露面**：argv 即进程命令行 ⇒ 在 Linux 上位于 **`/proc/<pid>/cmdline`**，
  而 `/proc` 的 `hidepid` **默认为 0** ⇒ **同机任何用户/进程都能读到该 ASR token**，
  窗口 = 侧车转写时长（数秒级，`commands.rs:300-306` 的后台 wait 证明它是短生命周期进程）。
- **与本仓自身的纪律不一致**（这是本条的真正分量）：密钥**不出现在**
  `GET /api/v1/env`（B0009 核）· 文件日志 sink 按键名脱敏（B0017 核）· Mod 的 `state_json`
  （B0105 核：只回 `api_key_set` 布尔）· 进程内只经 `secrets::lookup` 解析（B0104 核）——
  **argv 是唯一一条被漏掉的通道**，且 `grep "ps |进程表|可见|泄露"` 在该 crate **零命中**
  ⇒ **没有任何注释说明它被考虑过**。
- **不是可远程利用的面**（与 B0110 的结论一致）：token 只能由本机用户经
  loopback + Origin 校验的 Mod config 路由写入 `mods.json`（B0110 已核）。
建议: ① **改走环境变量**（接收侧已支持，**改一处即可**）：
   `cmd.env("VOICE_INPUT_TOKEN", token)`（以及 `VOICE_INPUT_URL`），**不再**把 token 放进 argv；
   `--audio`（本地临时路径）可继续走 argv（敏感度低一个量级）。
   ② 若要保留 argv 兼容旧脚本，**至少**在 `build_sidecar_argv` 的头注上写明
   「token 走 argv ⇒ **会出现在进程命令行**（同机可见）」——让下一个读代码的人知道。
   ③ 顺带核对：`--url` 是否也可能带凭据（若用户把 `user:pass@host` 写进 base_url，
   同样会进 argv）。
**定 P2**（而非 P1）：暴露窗口短、需同机其他用户/进程、机器是个人桌面（项目自身的
WSL2↔Windows 分工即如此）；但**修法一行**、且与本仓其余密钥纪律**明显不一致**。
验证: 读 mod-voice-input/src/sidecar.rs:116-139（argv 构造）、src/commands.rs:262-306（解析 + spawn）、
docs/examples/voice-sidecar/voice_sidecar.py:483-493（参数与环境兜底）；
`grep -rn "ps |进程表|可见|泄露" mod-voice-input/src/*.rs` → 零命中
置信: 高（三处原文 + 一条零命中的反证）
反证: (a) 试过论证「`hidepid` 默认是 2（更安全）」——**不成立**，多数发行版默认 `0`；
即便开了 `hidepid=2`，**同用户的其他进程**仍可读 ⇒ 泄露面缩小但不为零；
(b) 试过论证「sidecar 是用户自己配的脚本，泄露无所谓」——脚本可信**不等于**命令行可信：
读 argv 的是**别的进程**（可能是本机上的其它软件），与脚本作者无关；
(c) 试过找「这是有意的」注释——**零命中** ⇒ 不能当作有意取舍。

---

## BATCH-0112（2026-09-28）· **扩写 F-0111-01（同根因合并，不新开条目）**

> **0 条新缺陷。** B0111 留的问题是「`--url` 是否可能带凭据」。答案是**会**，且它与
> F-0111-01 是**同一机制、同一修法** ⇒ 按同根因合并纪律**并入 F-0111-01**，不新开条目。

### `--url` 确实可能带凭据，且没有任何过滤
`sidecar.rs:36-39`：
```rust
/// 读 `sidecar_url`：非字符串 / 空白 → 空串（= 必须由命令参数给）。
pub fn sidecar_url_from_config(config: &Value) -> String { trimmed_string(config, "sidecar_url") }
```
⇒ **原样取自 Mod 配置**，**无 URL 解析、无 `user:pass@` 剥离、无 scheme 校验**。
⇒ 用户若把 `https://user:pass@host/v1/chat` 写进 `sidecar_url`，这串**带凭据的 URL**
就会经 `build_sidecar_argv` 的 `--url`（:128-129）落到 **`/proc/<pid>/cmdline`** ——
**与 F-0111-01 的 token 完全同一条通道、同一后果**。
⇒ **而接收侧同样已支持更安全的通道**：`voice_sidecar.py:490`
`p.add_argument("--url", default=os.environ.get("VOICE_INPUT_URL", DEFAULT_URL), …)`
⇒ **F-0111-01 的修法扩成两处 env 即可**：
`cmd.env("VOICE_INPUT_TOKEN", token)` + `cmd.env("VOICE_INPUT_URL", url)`。

### 顺带：`sidecar_python` 是**第二条**用户可控可执行路径
`sidecar.rs:52-59` `sidecar_python_from_config` 原样取 `sidecar_python`，而它就是 **`argv[0]`**
（`build_sidecar_argv:125`）⇒ 用户可让宿主执行**任意程序**（与 B0110 记录的 `sidecar_script`
是**两条**独立路径，两者都在 Mod 配置里、都只能由本机用户写入）。
⇒ 计入**能力台账**：`voice-input` 持 **两条**用户可控的执行入口（`sidecar_python` = argv[0]、
`sidecar_script` = argv[1]），**均无路径白名单**（只做 `is_file()`）。
**为何不记发现**：这是**该 Mod 的设计用途**（ASR 侧车本就要执行一个脚本），
且输入**只能由本机用户经 loopback+Origin 校验的配置路由写入**（B0110 已核）⇒ 无远程提权面。
**但**它使 F-0111-01 的后果**略微加重**：argv 里同时可能出现「可执行路径 + 带凭据 URL + 明文 token」。

---

### F-0114-01 · P2 【伪造 MEMORY_MARKER_END ⇒ 记忆块污染 system prompt 基线并逐轮累积】

## BATCH-0114（2026-09-28）· ⭐ **F-0114-01（P2）：伪造 marker 可让记忆注入块污染「基线」并持续累积**

file: crates/live2d-ai-mod-memory/src/strategy.rs:35-37（marker 常量）+ :383-390（行清洗）+ :478（拼装）+ :367-377（剥离）
摘录:
```rust
pub const MEMORY_MARKER_BEGIN: &str = "<!-- live2d-ai:memory-v0:begin -->";   // :35
pub const MEMORY_MARKER_END:   &str = "<!-- live2d-ai:memory-v0:end -->";     // :37
…
pub fn sanitize_memory_line(text: &str) -> String {                            // :383
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");            // :384 ← 折叠空白
    …
}
…
format!("{MEMORY_MARKER_BEGIN}\n{body}\n{MEMORY_MARKER_END}")                   // :478 ← 拼装
…
pub fn strip_memory_block(prompt: &str) -> String {                             // :367
    let mut out = prompt.to_string();
    while let Some(start) = out.find(MEMORY_MARKER_BEGIN) {                     // :369 第一个 BEGIN
        let end = match out[start..].find(MEMORY_MARKER_END) {                   // :370 其后**第一个** END
            Some(rel) => start + rel + MEMORY_MARKER_END.len(),
            None => out.len(),
        };
        out.replace_range(start..end, "");                                      // :374
    }
    out.trim_end().to_string()
}
```
影响: **静默的持久状态损坏**（四步全部已核，不是推断）：
1. **拼装**是 `base + BEGIN\n{body}\nEND`（:478）；
2. **记录里的 marker 不会被中和**：`sanitize_memory_line` 的 `split_whitespace().join(" ")`
   （:384）把连续空白折成单空格，而 marker **本身就只含单空格**
   ⇒ 用户消息里若含 `<!-- live2d-ai:memory-v0:end -->`，折叠后**逐字保留**；
3. **剥离会提前收口**：`strip_memory_block` 找「BEGIN 之后**第一个** END」（:370），
   伪造的 END 在 `body` 里 ⇒ 删掉的只是 `BEGIN..伪造END`，
   **真实 END 及其后的记录残片被留在 `out` 里**；随后 `find(BEGIN)` 已无命中 ⇒ 循环退出
   ⇒ 结果 = `base + 残片 + 真实END`：**记忆正文被并进「基线」**，且基线里**固化了一个 END marker**；
4. **逐轮累积**：下一轮又把 `BEGIN…END` 拼在这个被污染的基线之后 ⇒
   提示词里出现**两个 END**、**上一轮的记忆残片常驻基线** ⇒ **每轮都在变差**。
⇒ 用户侧的表现是「模型开始复读我以前说过的话，而且关掉记忆 Mod 也不掉」（`lib.rs:491/744` 的
清除路径同样按 marker 剥离 ⇒ 剥不干净）。
**为什么定 P2**：触发需要用户（或角色卡）文本里**逐字含该 marker**（概率低、但复制一段
含 marker 的提示词/诊断输出就会命中，且**无远程路径**）；后果是**持久**的状态损坏（不是一次抖动），
且与本仓「不静默失效」的纪律直接冲突。**不是 P1**：无安全影响、无数据丢失到外泄、需本机用户内容命中。
**顺带**：`:364-366` 已考虑「有 BEGIN 无 END 的**半截块**」并选择「剥到末尾」（这个选择是对的），
但**没有考虑「多一个 END」** ⇒ 与 B0109 的「不可拆的两半」同源：两半都对，接缝没人管。
建议: ① **在 `sanitize_memory_line` 里中和 marker**（一行）：
   把 `<` 换成全角 `＜`，或直接删掉等于 marker 的子串 ⇒ 记录**不可能**携带 marker，
   剥离算法随即对**任意**用户内容都正确（这是「让约束跟着组件」的同一思路，F-0092 的修法）；
   ② 若要更稳：`strip_memory_block` 改成「取**最后一个** END」或「BEGIN/END 计数配平」，
   使**多 END** 也不致提前收口；③ 补回归：一条内容含 END marker 的记录 ⇒
   `strip(strip(x)) == strip(x)` 且**基线不含任何记录文本**。
验证: 读 strategy.rs:35-37 / :357-377 / :383-390 / :478；
`grep -rn MEMORY_MARKER strategy.rs`（穷举 6 处引用，无 head）
置信: 高（拼装 / 清洗 / 剥离 / 常量 四处原文互证；`:384` 的折叠**不会**破坏 marker 这一点可逐 token 验证）
反证: (a) 试过论证「空白折叠会破坏 marker 从而挡住它」——**不成立**：marker 内的空格**本来就是单空格**，
`split_whitespace` 把 `<!--` 与 `-->` 之间的多空格折成单空格后**与原串相同**；
(b) 试过论证「`MAX_MEMORY_LINE_CHARS=200` 会截掉 marker」——marker 仅 36 字符，落在 200 之内；
(c) 试过论证「`while` 循环会清掉残留」——**不成立**：残留里已**没有 BEGIN**（真 BEGIN 已被删），
`find(BEGIN)` 返回 `None` ⇒ 循环立即退出 ⇒ 残留**永久留存**。

---

## BATCH-0121（2026-09-28）· `plan_atomic_write`：**契约未提权限**，而两个调用方里有一个必须自己补一步

file: crates/live2d-ai-runtime/src/settings/patch.rs:738-771（共享写盘函数）
摘录（它的全部保证）:
```rust
pub fn plan_atomic_write(path: &Path, content: &str) -> Result<PathBuf, String> {
    if content.is_empty() { return Err(format!("拒绝空内容写盘（{}）：写空 live2d-ai.toml 会静默丢失配置", …)); }  // :739-744
    let tmp = append_tmp_suffix(path, process::id());                       // :747-748  <path>.tmp.<pid>，不替换扩展名
    let write_result = (|| -> std::io::Result<()> {
        let mut f = File::create(&tmp)?; f.write_all(content.as_bytes())?;
        f.sync_data()?;                                                     // :757 fdatasync，崩溃语义说明在 :754-756
        Ok(())
    })();
    if let Err(e) = write_result { let _ = std::fs::remove_file(&tmp); return Err(…); }   // :761-769
    Ok(tmp)                                                                 // :770 ← **只写 tmp，不 rename**
}
```
### F-0121-01 · P3 【共享写盘函数的契约**不含权限**，于是「谁该 chmod tmp」成了调用方各自的知识】
影响: 可维护性/契约完整性（**当前无泄露**）：
- `plan_atomic_write` 用 `File::create(&tmp)` ⇒ tmp 权限是 `0666 & ~umask`（**通常 0644**），
  它的文档（:746-757）只讲**内容与崩溃语义**，**一个字都没提权限**，也**不负责 rename**（:770 只返回 tmp 路径）；
- **两个调用方的做法不同，而两者各自是对的**：
  | 调用方 | rename 前是否 chmod tmp | 是否需要 | 理由 |
  |---|---|---|---|
  | `secrets.rs:211-214`（写 `.env`） | **是** | **需要** | 内容含**密钥明文**；不收紧就有「文件已是新密钥、权限还是 0644」的亚秒窗口 |
  | `settings_routes/mod.rs:217-218`（写 `live2d-ai.toml`） | **否** | **不需要** | toml **只持 `api_key_env` 变量名**（AGENTS.md 明文），**无密钥材料** |
- ⇒ **不对称是按内容划分的、正确的**，**不记为缺陷**。
- **但契约有洞**：`plan_atomic_write` 是**三处 toml 写盘点 + `.env` 共用**的函数，
  它的契约**不含权限** ⇒ 调用方**无法从签名或文档得知自己该负责什么**。
  今天 `.env` 那个调用方**知道**（因为有人专门想过并写下注释），
  三个 toml 调用方**不需要知道** —— 这靠的是**外部知识**（toml 里没有密钥），
  而**不是**函数契约。若将来有人往 toml 里放**任何敏感内容**，
  这条路径**不会有任何提示**。
建议: 在 `plan_atomic_write` 的文档里补两句（成本一行）：
① 「**本函数不设文件权限**：tmp 由 `File::create` 创建，权限是 `0666 & ~umask`（通常 0644）；
调用方若写入**含密钥/敏感内容**，必须在 rename **之前**收紧 tmp 权限
（先 chmod tmp 再 rename；rename 之后再 chmod 就晚了）」；
② 或加一个 `mode: Option<u32>` 参数，让「这份内容要不要 0600」成为**调用点必须回答的问题**。
**为什么定 P3**：当前**没有任何泄露**（toml 无密钥；`.env` 已有正确处理），
缺的是**契约表述**，属可维护性。
验证: 读 patch.rs:738-778（`plan_atomic_write` / `append_tmp_suffix`）、
settings_routes/mod.rs:200-222（toml 写盘：merge → create_dir_all → plan_atomic_write → rename）、
secrets.rs:167-175 + 211-214（`.env` 写盘：merge_env_line → plan_atomic_write → **chmod tmp** → rename）
置信: 高（三处原文互证）
反证: (a) 试过论证「toml 里其实有敏感内容（`persona.system_prompt`）⇒ 0644 窗口是隐私泄露」——
**部分成立但非缺陷**：该文件**本来就是 0644**（首次保存同样经 `File::create`），
窗口**没有授予原本没有的访问权**；且用户若手动收紧到 0600，
**第一次保存会把它放宽回 0644** —— 这一点我**未核**（是否有代码在别处 chmod 过 toml），
**如实标注为未核实**，但即便成立也是**长期存在**的状态而非窗口问题；
(b) 试过论证「`File::create` 会用 0600」——**不成立**，Rust 的 `File::create` 就是
`OpenOptions::new().write(true).create(true).truncate(true)`，**不带 mode** ⇒ 0666 & ~umask。

---

## BATCH-0125（2026-09-28）· `secrets::parse`：**键的半截写法被挡住了，值的半截写法没有**

file: crates/live2d-ai-runtime/src/secrets.rs:62-90
摘录:
```rust
pub fn parse(text: &str) -> BTreeMap<String, String> {
    …
    let Some((key, value)) = line.split_once('=') else { continue; };
    let key = key.trim();
    if !is_valid_key(key) { continue; }          // :74-76 ← 键写错 ⇒ **整行跳过**
    let value = value.trim();
    let value = unquote(value);                 // :78
    out.insert(key.to_string(), value.to_string());
}
…
/// 去掉成对的引号（单层，不做转义展开）。
fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && (bytes[0] == b'"' || bytes[0] == b'\'') && bytes[bytes.len()-1] == bytes[0]
```
影响: 可用性/密钥正确性（**静默失效**，方向与 F-0098-01 相反 —— 这次是**实现**没说清、而**文档**已说清）：
- **键写错**被显式挡住，理由写在头注 :58-59：「键名必须是合法环境变量名，否则**整行跳过**
  （**宁可少一个变量，也不要让一行手写错的内容变成半截 key**）」⇒ **这个哲学是对的**；
- **但值写错没有同等对待**：`unquote` 只在**首尾是同一种引号**时才成对去掉（:87-89）。
  于是手写 `DEEPSEEK_API_KEY="sk-abc`（**漏了收尾引号**）会得到值 `"sk-abc`
  —— **那个引号成为密钥的一部分**。
- ⇒ 后果链：密钥**实际是错的** ⇒ 每次请求 401；而 UI 侧显示的是
  `has_api_key` **布尔**（B0105/B0123 已核：它只是「是否查到值」，不校验值可用）
  ⇒ **用户看到「已设置」却全程 401，且没有任何一处说「值里有引号」**。
⇒ 与本仓自身纪律的冲突点很精确：**同一条注释里的哲学只被应用到了键，没被应用到值。**
建议: ① 值以引号开头但**没有**配对收尾时，**按与键同样的哲学处理** ——
   要么去掉那个孤立引号，要么**跳过该行并留下可见痕迹**（`.env` 路径无 logger 时，
   可在 `parse` 的文档里注明「值的引号必须成对」并让 `is_set` 侧能提示）。
   ② 无论选哪种，都值得在 `unquote` 的注释里点明「**不配对的引号会留在值里**」，
   让排障的人第一眼能想到这一种。
**为什么定 P3**：触发是手写漏引号（概率低、无远程面）；后果是**持续 401 + 界面说「已设置」**
（可诊断性差，但不丢数据、不外泄）。**与 F-0098-01 不同**：那条是**文档说静默而实现不静默**
（安全方向），这条是**文档已说清规则、而规则本身漏了一种形态**。
**顺带核到两个正面点**：
- **CRLF 被正确处理**：`text.lines()` 对 `\r\n` 会**去掉 `\r`** ⇒ Windows 编辑器保存的 `.env`
  解析正常（否则每个密钥会带上一个尾随 `\r`，症状是**全线 401**）。这一条**头注没写**，
  但属**安全方向**的未文档化（与 F-0098-01 同一判据：安全方向不记发现）。
- 值的注释语义与头注一致：**只跳整行注释**、不做行尾 `#` 截断（`KEY=v # c` 的值是 `v # c`），
  这与主流 dotenv 实现一致，且文档明写「以 `#` 开头的行 → 跳过」。

---

### F-0127-01 · P3 【`speak` 文本被 `chars().take()` 静默截断，而同函数内的其它上限都报错】
> ➕ **B0173 补一条对照数据（加强本条的收窄判断）**：`plan.rs` 同一文件里两类上限**处置相反**——
> **受维护的 v1 `segments`**：超限**报错**，且文案写明「**不截断**」/「**只能切分，不能改写**」；
> **已弃用的 v0 `speak`**：`chars().take()` **静默截断**、无任何说明。
> ⇒ **纪律掉落在「被弃用的那条路」上**，不在维护中的路上
> ⇒ 故本条**不是「这文件的风格不一致」，而是「只有 legacy 路径才不一致」**（**加强** B0128 的收窄）。
> ⚠ **两处须同时读：**
> ① **定级 P3（不是 B0127 段内写的 P2）** —— B0128 顺着 `json_schema_strict`(plan.rs:787) 核到：
> `speak` 是 **v0 旧字段、已弃用（V11）**，schema 的 `description` 逐字写「**已弃用（V11 保留）：
> 与 segments 同现时被忽略**」（:805）⇒ **现代路径（模型按 schema 产出 `segments`，
> 而 `segments` 在 `required` 里，:793）根本走不到这个截断**；且与 `segments` 同现时是
> **`SpeakIgnored` + warn**（:307/:318「segments 与 speak 同现：按 O1 忽略 speak」），**有提示**、不静默。
> ⇒ **真实范围 = 弃用兼容路径**（`speak` 存在而 `segments` 缺席，:458 `MissingSegments` 的另一侧）
> 上的静默半句。**B0127 段内原文记述范围过宽，保留在下方供追溯。**
> ② **本条是「模式 Q 自身被违反」的样本**：B0127 写这条时**又只写在了批次散文里、没有 `### F-` 规范头**
> —— 正是 CONSOLIDATION-12 §② 刚修掉的第 1 类缺陷。**隔一批就复发**，故此处显式标注。

## BATCH-0127（2026-09-28）· ⭐ **F-0127-01（P2）：`speak` 文本被 `chars().take()` 静默截断，而同函数内的其它上限都报错**

file: crates/live2d-ai-runtime/src/performance/plan.rs:510-514（截断处）vs :521-522 / :592-594（报错处）
摘录（**同一个解析函数里，三种上限、两种处置**）:
```rust
// ① speak 文本：超 4000 字符 ⇒ **静默截断**
Some(trimmed.chars().take(MAX_SPEAK_CHARS).collect::<String>())      // :514
…
if cue_list.len() > MAX_CUES {
    return Err(PlanError::TooManyCues(cue_list.len()));             // :521-522 ← **报错**
}
…
if segment_list.len() > MAX_SEGMENTS {
    … "段数 {} 超过上限 {MAX_SEGMENTS}"                              // :592-594 ← **报错 + 消息**
```
影响: 正确性/一致性（**静默的数据丢失**）：
- LLM（或手写 plan）给出**超过 4,000 字符**的 `speak` 字段 ⇒ 被 `chars().take()` **从中间切断**，
  **无错误、无消息、无痕迹**；TTS 会把**半句话**念出来，界面气泡也只显示半句；
- 而**同文件、同函数**里的「cue 太多」「段数太多」都**返回带消息的 `PlanError`**
  ⇒ **该文件自己的约定是「超限 ⇒ 告诉调用方」，`speak` 是唯一的例外**；
- **结构上没有告警通道**：`parse_plan` 的返回类型是
  `Result<PerformancePlan, PlanError>`，**没有** memory Mod 那样的 `warnings: Vec<String>`
  （对比 B0102：`ModelError::degraded` + `FieldMap.warnings`）⇒ 这种截断**在结构上不可能**被上报。
- 与 **F-0020-01（P1）的关系**：那是 `MAX_TOKENS` 从**请求侧**截断整个 plan；
  这里是**字段级**的第二次截断，**独立存在**，且两者叠加时症状相同（回复变半句 / 一个字都不上屏）。
建议: ① 改成与邻居一致的处置：**超限即 `Err`**（新增 `PlanError::SpeakTooLong(len)`），
   或 ② 若为了鲁棒必须截断，则**同时**：
   - 至少让调用方拿得到「被截断了」这件事（给 `PerformancePlan` 加一个
     `speak_truncated: bool` 或在 `PlanError` 之外加一个 `warnings` 通道 ——
     `PerformancePlan` 已有 `summary()`(:356) 与 `code()/message()` 的错误面，加个标记成本很低）；
   - **不要从中间切**（`chars().take()`），或至少在切点补一个明确的省略标记，
     让「念到一半断掉」变成可归因的现象而不是玄学。
**为什么定 P2**：默���配置下不易触发（4,000 字符的 `speak` 需要很大的 `max_tokens`
或手写 plan），但一旦触发就是**静默的半句话**，且违反该文件自身的约定。
验证: 读 plan.rs:61-79（全部上限常量）· :489-530（`parse_plan` 头段：speak 分支 + cues 上限）·
:592-594（段数上限）；穷举 `MAX_SPEAK_CHARS` / `MAX_SEGMENTS` 的全部使用点
置信: 高（三处上限的处置在同一函数内可直接对读）
反证: (a) 试过论证「截断是刻意的鲁棒性选择」——**可能是**，但**没有注释说明**这一点，
而邻居都报错 ⇒ 即便是有意的，也**缺一句「这里为什么可以静默」**（属 F-0098-01 所说的
「文档未覆盖的形态」，但这次方向相反：**实现静默而文档没说清**）；
(b) 试过论证「4000 字符远超任何合理输出」——**不成立为免责**：`max_tokens` 是**可配的**（B0013 已核
默认 4096、上限由配置给），而本仓自己有「长思考 + 正文」的场景（`reasoning_content`，B0009/B0009 侧）；
(c) 试过找别处兜底——`parse_plan` 无告警通道（见上），**没有兜底**。

---

## BATCH-0044（2026-09-28）· ⚠ **本段为 CONSOLIDATION-13 从 INDEX/BATCH 文件回填**（见下）

> **账本缺陷说明**：本批当时**只写了** `BATCH-0044.md` / `INDEX.md` / `STATE.md`，
> **没有写 FINDINGS 段** ⇒ `F-0044-01` / `F-0044-02` 两条被 INDEX 对外引用，
> 却在 FINDINGS 里**查不到**。这是 B0129 的**机械检查**首次抓出的「**最坏形态**」
> （不是「缺规范头」，是「**正文根本没落盘**」）。此处按 INDEX 的记述**回填**。

### F-0044-02 · P3 【`chat_routes.rs` 的行数注释与实际差 926 行，且它「自己就是」豁免的反例】
file: crates/live2d-ai-desktop/src/web_api/chat_routes.rs（行数注记处）
摘录（INDEX 当时的记述，逐字保留）:
> 「`chat_routes.rs` 实测 1086 却写「约 560」」
影响: 可维护性/文档可信度：该文件**自己**就是「超过 ≤500、需头注豁免理由」这条规矩的**适用对象**，
而它的头注数字（**约 560**）与实测（**1086**）相差近一倍，且**已越过 1000 的豁免上限**。
⇒ 这与 `appearance_background.dart:22-44` 那种**「如实标注：超 1000，是既有债」**的写法
**形成直接对照** —— 同一仓库里一处主动认账、一处数字失真。
建议: 把头注改成**实测值**并按 `appearance_background.dart` 的样式**标注为既有债**
（写明规模来源 + 为何不现在拆 + 拆分表的去处），使两个「超长文件」的处置口径一致。
**为什么定 P3**：纯文档；不影响运行时行为；且**该文件无对应测试因而无「假绿灯」风险**。
验证: `wc -l crates/live2d-ai-desktop/src/web_api/chat_routes.rs` → **1086**（B0044 实测）；
对照 `crates/l2d-wasm-demo/src/settings/sections/appearance_background.dart:22-44`（B0088 读的写法）
置信: 中高（行数为实测；头注原文未在本段重引，若需引用请回 BATCH-0044.md 核对）
反证: (a) 试过论证「注释里的行数可能指某个子模块」——待核（**如实标注为未完全核实**）；
(b) 试过找测试覆盖 —— 无需：该条是文档与实测的差，不是行为。

---

## BATCH-0125 · `secrets::parse`：**键的半截写法被挡住了，值的半截写法没有**

> **账本缺陷说明**：本段原先只以「段标题」形式落盘，而**段标题里的 `F-0125-01` 前缀
> 在写入时丢失** ⇒ 该条被 INDEX/STATE 对外引用却**在 FINDINGS 里 grep 不到**。
> 同样由 B0129 的**机械检查**抓出。此处补上规范头（正文见下）。

### F-0125-01 · P3 【`parse` 对**值**的半截写法缺少与**键**同等的对待】
file: crates/live2d-ai-runtime/src/secrets.rs:62-90
摘录:
```rust
let Some((key, value)) = line.split_once('=') else { continue; };
let key = key.trim();
if !is_valid_key(key) { continue; }          // :74-76 ← 键写错 ⇒ **整行跳过**
let value = value.trim();
let value = unquote(value);                 // :78
…
/// 去掉成对的引号（单层，不做转义展开）。
fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && (bytes[0] == b'"' || bytes[0] == b'\'') && bytes[bytes.len()-1] == bytes[0]
```
影响: 可用性/密钥正确性（**静默失效**）：**键**写错被显式挡住，头注 :58-59 写明
「否则**整行跳过**（宁可少一个变量，也不要让一行手写错的内容变成**半截 key**）」⇒ 哲学正确；
**但值写错没有同等对待** —— `unquote` 只在**首尾同种引号**时成对去掉 ⇒ 手写
`DEEPSEEK_API_KEY="sk-abc`（**漏收尾引号**）得到值 `"sk-abc`，**引号成为密钥的一部分**
⇒ 密钥**实际是错的** ⇒ 全程 401；而 UI 只显示 `has_api_key` **布尔**（B0105/B0123 已核它只表示
「是否查到值」）⇒ **用户看到「已设置」却全程 401**，且无任何一处提示「值里有引号」。
**冲突点精确**：**同一条注释里的哲学只应用到键，没应用到值**。
建议: ① 值以引号开头但**无配对收尾**时，按与键同样的哲学处理（去孤立引号 / 跳过该行）；
② 在 `unquote` 注释里点明「**不配对的引号会留在值里**」，让排障的人第一眼想到这一种；
③ 顺带补两条测试（B0126 已核 `secrets` 的 10 条测试**唯独没有**这两种形态）：
不配对引号、**CRLF**（CRLF 行为正确但无回归保护 ⇒ Windows 用户存 CRLF 会全线 401）。
**为什么定 P3**：触发是手写漏引号（低概率、无远程面）；后果是持续 401 + 可诊断性差，
不丢数据不外泄。**与 F-0098-01 方向相反**：那条是「文档说静默而实现不静默」（安全方向），
这条是「**文档已说清规则、而规则本身漏了一种形态**」。
**顺带核到两个正面点**：**CRLF 被正确处理**（`str::lines()` 对 `\r\n` **去掉 `\r`** ⇒
Windows 保存的 `.env` 解析正常，否则每个密钥带尾随 `\r` ⇒ **全线 401**；此条**头注未写**，
但属**安全方向**的未文档化，不记发现）；值的注释语义与头注一致（**只跳整行注释**、
不做行尾 `#` 截断，与主流 dotenv 一致）。
验证: 读 secrets.rs:50-90（`parse` 全部规则 + `unquote`）；B0126 枚举了同文件 10 条测试名
置信: 高（实现逐行可读；测试名逐条枚举过）
反证: (a) 「截断是刻意的鲁棒性选择」——**可能**，但**没有注释说明**，而邻居都报错
⇒ 即便有意也缺一句「这里为什么可以静默」；(b) 「4000 字符远超任何合理输出」——
`max_tokens` 是**可配的**（B0013 已核），且本仓自己有长思考场景，不成立为免责。


---

### F-0184-01 · P2 【`stripCommentsAndStrings` 在 `test/` 下有 8 份副本，其中 1 份已漂移（漂移的那份更强）】
> ➕ **B0185 修正本条的**建议**（观察不变、修法要改）**：
> 查了落点 —— **`test/` 下没有任何子目录**（`git ls-files test/` 无 `NF>2`）、
> **不存在** `support/helper/util/common/shared` 之类的共享文件、
> 且 **0 处 import 共享版** ⇒ **8 份副本各自自足，而 `test/` 是刻意保持「扁」的**。
> ⇒ 我原先建议的「抽成 `test/support/source_scan.dart`」**会打破这个约定**。
> ⇒ **修正后的建议（二选一，并说明取舍）**：
> **(a) 保持扁平** ⇒ 把 **8 份收敛成同一份内容**（采用加固版 `out.write('S')`），
>   代价是**接受重复**（换来「每个测试文件自足、互不依赖」）；
> **(b) 引入 `test/support/`** ⇒ 一次抽取，代价是**打破扁平约定**、且测试文件之间**产生 import 依赖**。
> ⇒ **无论选哪条，8 份必须内容一致**（当前的漂移才是问题，重复本身不是）。
> ⚠ **未核实**：`test/` 扁平**是否**为刻意约定（无任何注释写明）—— 现有**间接证据**较强：
> **8/8 各自定义** + **0 处 import** + **无子目录** ⇒ 若属偶然，早该有人抽了。
> ⇒ 这与 B0152 的教训同源：**判据要加「理由」这一维** —— 这里的理由很可能是「测试独立性」。
> ⚠ **本条曾第三次漏掉规范头**（模式 Q 违规：B0128 / B0144 / 本批各一次），
> 由**每批收尾的模式 Q 双向检查**当场抓出并补上 ⇒ **这道检查第三次发挥了作用**。

## BATCH-0184（2026-09-29）· **F-0184-01（P2）：`stripCommentsAndStrings` 在 `test/` 下有 8 份副本，其中 1 份已漂移**

file: shell/flutter/test/{glass_rim,transient_results,no_client_side_preset_ttl_prediction,asset_guard_preset_dispatch,asset_guard_stage_attach_resend,asset_guard_director_cue,motion_wiring,design_tokens}_test.dart
摘录:
```
$ grep -rln "String stripCommentsAndStrings" test/ lib/     # 8 个文件
$ for f in …; do awk '/String stripCommentsAndStrings/,/^}$/' "$f" | md5sum; done
358277f5da69d0f7bef6d55019cc9e0d  glass_rim_test.dart
358277f5da69d0f7bef6d55019cc9e0d  transient_results_test.dart
358277f5da69d0f7bef6d55019cc9e0d  no_client_side_preset_ttl_prediction_test.dart
358277f5da69d0f7bef6d55019cc9e0d  asset_guard_preset_dispatch_test.dart
358277f5da69d0f7bef6d55019cc9e0d  asset_guard_stage_attach_resend_test.dart
358277f5da69d0f7bef6d55019cc9e0d  asset_guard_director_cue_test.dart
358277f5da69d0f7bef6d55019cc9e0d  motion_wiring_test.dart
476d1bd0231eace7da83457dd0bd192a  design_tokens_test.dart      ← **唯一不同**

$ diff …  →  21a22  >  out.write('S');      # 漂移的那份**多一行**
```
**多数版的字符串分支把字符串「删空」**（`glass_rim_test.dart` 片段，行号相对函数体）：
```dart
if (c == "'" || c == '"') { … continue; }    // 剥完 **不写任何东西** ⇒ 两侧代码会**粘连**
```
**漂移版写 `out.write('S')`** ⇒ 保留一个**分隔符** ⇒ 字符串两侧的 token **不会粘连**。

影响: ① **副本漂移已经发生一次**（8 份里 1 份不同）⇒ 「复制会漂移」不是理论风险，**是已发生的事实**；
② 多数版存在一处**理论上的假绿**：守卫形如 `src.contains('foo bar')` 时，源码 `foo'…'bar`
剥完会得到 `foo bar` ⇒ **匹配成立**。**注**：现有 7 条守卫的 needle 都是**长而具体**的
（`'_store = createBackgroundStore()'`、`'pinnedScales: _actionScalesSyncer.pinned'` 等）
⇒ 这种巧合**概率很低、我无法演示任何一条被误判** ⇒ **本条的 P2 不建立在「某条守卫已被骗过」之上**，
而建立在「**同目录内 8 份承重辅助函数、零共享、且已漂移一份**」这个**可核事实**上。
③ 反向看：`design_tokens_test.dart` 那份**是加固版** ⇒ 有人已经想到过这个问题，但**只改了一处**。

建议: ① **收敛成一份**（放进 `test/` 的共享 helper，如 `test/support/source_scan.dart`），
   **以 `design_tokens_test.dart` 那份为准**（它带 `out.write('S')` 分隔符）；
② 收敛时**顺手把函数名改得自解释**（如 `stripCommentsAndStringsPreservingSeparators`），
   让「保留分隔符」这个关键性质**出现在名字里**，而不是只存在于一行实现中
   （⇒ 正面模式 **P3「更严的写法要连更松的写法在哪里」** 与 **P19** 的应用）；
③ 收敛后这些结构型守卫才真正满足 **P19** 的全部含义（**8 份同时更强**，而不是 1 份）。
**为什么定 P2**：属「**明显会漂移的承重复制**」—— 漂移**已发生**，且这 8 份是 **P19 那套方法学的承重实现**
（结构型守卫全靠它）。**不是 P1**：我**无法演示**任何一条现存守卫被误判。
验证: `grep -rln "String stripCommentsAndStrings" test/ lib/`（8 命中）·
逐份 `awk | md5sum`（7 同 1 异）· `diff`（差 1 行 `out.write('S')`）·
`glass_rim_test.dart` 的字符串分支读至 `continue;`
置信: 高（机械可复跑；漂移的事实与方向都核过）
反证: (a) 试过论证「8 份各是独立测试、互不依赖 ⇒ 重复无害」——**不成立**：
它们是**同一个函数**的 8 份拷贝，任何一份被加固而另一份没有，就是**守卫之间强度不一致**；
(b) 试过论证「漂移那份可能更弱」——**已读源码**：漂移那份**多写一个分隔符** ⇒ **更强**，不是更弱；
(c) 试过找「共享 helper」已存在——**零命中**（`grep -rln` 只见这 8 份定义，无一处 import 共享版）。

---

## BATCH-0189（2026-09-29）· **F-0189-01（P2）：`SettingsController.load()` 没有任何 staleness 守卫，而**声称**守它的那组测试是顺序执行的**

file: shell/flutter/lib/settings/settings_controller.dart:261-277（`load` 本体）
摘录:
```dart
Future<void> load() async {                      // :261
  _loading = true; _error = null; notifyListeners();
  try {
    _remote = await api.fetchSettings();          // :266  ← **无 epoch / 无序号 / 无 in-flight 判据**
    _draft = SettingsDraft();                     // :267
    _lastOutcome = null;
  } on ApiException catch (e) { _error = e.toString(); }   // :269-270
  …
}
```
**可核的三个事实**：
```
$ grep -n "epoch|_gen|generation|seq|stale|乱序|inflight|inFlight" lib/settings/settings_controller.dart
（**零命中**）

$ grep -rn "\.load()" lib/
lib/app/shell_settings.dart:18:      _settings.load(),
lib/app/shell_settings.dart:108:     await _settings.load(),
lib/app/shell_settings.dart:224:     onPressed: () => unawaited(_settings.load()),   ← ⭐ **按钮，且不 await**
```
**而声称守它的那组测试（`test/settings_controller_test.dart:559-589`）是纯顺序的**：
```dart
group('GET / PATCH 乱序：响应回来得晚也不能覆盖新状态', () {                        // :559
  test('load → save → 新的 load，最终 remote 是最后一次 load 的结果', () async {   // :560
    await c.load();   expect(c.remote!.llm.model, 'deepseek-flash');                // :579-580
    c.edit((d) => d.ttsVoice = 'nova');  expect(await c.save(), SaveOutcome.savedApplied);  // :582-583
    await c.load();   expect(c.remote!.llm.model, 'later-model', …);               // :586-587
```
影响: **一个被写进测试名的并发不变式，实际既无实现守卫、也未被测试**：
1. **实现侧无守卫** ⇒ 两个 `load()` 重叠时，**哪个响应后到哪个生效**（`:266` 无条件覆盖 `_remote`）
2. **可达性已核**：`shell_settings.dart:224` 是**按钮**且用 **`unawaited(...)`**
   ⇒ 用户**连按两次**即产生重叠；`unawaited` 也意味着框架**不会**阻止第二次按压
   ⇒ 后果：**界面显示较旧的那次响应**（与 `:267` 顺带把草稿清空叠加）
3. **测试侧是顺序的**：`:579/:586` 两处 `await` ⇒ **全程没有重叠** ⇒
   `:587` 的断言「最后一次 load 才是真相」在顺序执行下**恒真**
   ⇒ **该测试对它自己名字承诺的不变式，是「结构上无法失败」的**
   （⇒ 与模式 H 假绿同形，但方向相反：**不是断言写错，是断言太弱**）
4. **它与本仓的一次真实事故同域**：AGENTS.md 记「手改 `live2d-ai.toml` 后 `GET /settings` 不跟随磁盘，
   下一次界面「保存」会把手改内容**覆盖**掉」⇒ 那次修的是**磁盘跟随**那一半
   （`StatusContext::refresh_from_disk`，B0104 核过）与**草稿丢弃**那一半（`:588` 的断言钉住）
   ⇒ **「两个 GET 乱序」这一半既无守卫也无有效测试。**

建议: ① 在 `load()` 里加**最小守卫**（一行级）：进函数时取一个自增序号 `final int my = ++_loadSeq;`
   ⇒ `await` 之后**先判 `if (my != _loadSeq) return;`** 再赋 `_remote` / `_draft`
   ⇒ 语义与 B0040 的 `epoch` 闸**同形**（本仓已有先例），且**不需要取消请求**；
   ② 补一条**真正制造重叠**的测试：让 `MockClient` 的**第一次** GET **延迟**、第二次立即返回
   （`Future.delayed` + `Completer`），断言**最终 `_remote` 是第二次的值**
   ⇒ 有了 ①，② 就会红 ⇒ **测试才有牙齿**；
   ③ 顺带把 `:224` 的按钮在 `_loading` 期间**禁用**（或在 `load()` 入口用 `_loading` 早退），
   免得「连按两次」本身成为常态。
**为什么定 P2**（并诚实说明升级条件）：
- 已核：实现**无守卫**（`grep` 零命中）· 触发路径**存在**（`:224` 按钮 + `unawaited`）·
  测试**不制造重叠**（三处 quote）
- 后果：**界面显示较旧响应** + 草稿被清空（后者是**设计内**的，`:588` 已钉）
- **未核**（⇒ 阻止我把它定 P1）：`save()` 的 patch 究竟由 `_draft` 还是 `_remote` 派生
  ⇒ **若由 `_remote` 派生，则「旧响应 → 保存 → 写回旧值」成立，严重度升 P1**
  （**B0190 第一件事**）。**另**：连按两次的**用户行为**是否常见，我**无法从仓库证明**。
验证: `grep -n "epoch|_gen|generation|seq|stale|inflight" settings_controller.dart`（**零命中**）·
`grep -rn "\.load()" lib/`（3 处，`:224` 为 `unawaited` 按钮）·
`settings_controller_test.dart:559-589` 全文（`await c.load()` :579 · `await c.save()` :583 · `await c.load()` :586 ⇒ **顺序**）
置信: 高（三个事实均为原文/机械可复跑）
反证: (a) 试过论证「`_loading` 标志会挡住第二次」—— **未核**：`load()` 只**写** `_loading`
（:262/:274），**入口没有「已在加载就早退」的判据**；若调用方在按钮层面禁用了它，`:224` 不会写 `onPressed`；
(b) 试过论证「`ApiClient` 内部有请求串行化」—— **未核**（`api.fetchSettings()` 本体未读）
⇒ 两者都**可能**成立，因此**不定 P1**；**但 (a)(b) 中任一成立都应在本条里写明**，
因为那会把修法从「加守卫」缩小到「已在别处解决、只需补测试」。

### F-0189-01 · P2 【`SettingsController.load()` 无 staleness 守卫，声称守它的那组测试是顺序执行的】
> ➕ **B0195：反证 (b) 彻底排除，调查收口** —— `api_client.dart:229-235` 的 `_guard()`
> **只有 5 行、只做一件事**（把传输异常翻成 `ApiException`）⇒ **无超时/无重试/无去重/无串行化**
> ⇒ **客户端层不提供任何能给两个并发 GET 定序的机制**
> ⇒ **本条全部未决点结清**：实现无守卫 · 测试顺序（对该主张结构上无法失败）· 触发已排序 ·
> (a) 推翻 · (b) **彻底排除** · (c) 无法证明（**维持 P2**）· 危害 = **两个成功的 load 乱序**
> ⇒ **丢弃未保存草稿**（主）+ 显示旧值（次）· 修复 = **一个自增序号**、**不需取消机制**
> ⭐ 顺带：`_decodeObject`(:237-245) **坏 JSON ⇒ 返回空 map 不抛** ⇒ 与表演层「失败 ⇒ 回落 + 计数」
> （B0151 的 **P9**）**同形** ⇒ **降级不留异常给 UI**
> ➕ **B0194：修正 B0193 的一处高估** —— `_ensureSettingsLoaded()` 有
> **`if (_settingsLoadedOnce) return;` 一次性守卫**（`shell_settings.dart:15-17`）
> ⇒ init `load()` **只发生一次**（首次进入设置面）⇒ **不能**作为**后续任意** `:108` 回读的搭子，
> **除非**用户的**第一个动作**就是保存覆盖且 init GET 仍在飞 ⇒ **窗口窄于我 B0193 的说法**
> ⇒ ⚠ **B0193 那句「重叠不需要连点两次、是正常操作」高估了，如实修正。**
> ⇒ **修正后最现实的触发**（按现实性）：
> **① 连续保存两次覆盖** ⇒ 每次都触发 `:108` 的整份回读 ⇒ **两个 GET、无按钮参与**（最高）
> ② init GET 在飞时立刻保存覆盖（中，窗口窄）③ `:224`「重试」连点两次（中）
> ⇒ **发现本身不变**（无守卫 / 可乱序 / 可丢草稿），**只是主路径换了**。
> ➕ **B0193：主路径现实性上调**（`:108` **不是按钮**，是「保存 ⇒ 回读」的尾巴）
> - `shell_settings.dart:108` 位于「保存本模型覆盖」之后 ⇒ **每次保存覆盖都触发一次整份回读**
> ⇒ **重叠不需要「连点两次」**：`打开设置 → init GET(:18)` **与** `改覆盖→保存→GET(:108)`
> **天然会交叉**（用户在面板里改东西是**正常操作**）⇒ 「打开设置后立刻改东西」即可触发
> - 同一处的 `if (!mounted) return;`（:99/:107/:109）说明**异步纪律存在、只是漏了 staleness**
> ⇒ **缺的不是异步意识，而是「两次异步的结果谁作数」这一个判据**
> ⇒ **修复定位更清楚**：**不需要取消机制**（其心智模型里没有），**只需一个自增序号**
> （与 B0040 的 `epoch` 闸同形、仓里已有先例）
> - ⚠ **仍定 P2**：t0 一步（init GET 的位置）是**推断**（`:18` 上下文未读）
> ⇒ **反证 (c) 仍未被证明**，但**不再依赖「连按两次」这个前提**
> ➕ **B0192：反证 (a) 推翻 + 危害再收窄一格**
> - ❌ **反证 (a) 推翻**：`shell_settings.dart:224` `onPressed: () => unawaited(_settings.load())`
>   **永远非空**（无 `loading ? null : …`）⇒ **连点两次即两个重叠 `load()`**；
>   且该分支是**「读不到服务端设置」的界面**（:215）⇒ **用户恰在「以为没反应」时连点「重试」**
> - ⚠ **危害收窄一格**：`load()` 的 `_draft = SettingsDraft()` **在 `try` 内的成功路径**（:267），
>   两个 catch（:269-272）**只设 `_error`** ⇒ **失败响应既不覆盖 `_remote` 也不重置 `_draft`**
>   ⇒ **「迟到响应丢弃草稿」只发生在两个都成功的 load 乱序时**；「显示旧值」则任何乱序都可能
> - ⚠ **反证 (c) 仍无法从仓库证明**（连按两次是否常见）⇒ **维持 P2**
> - **未核**：`:108` 那处 `await _settings.load()` 的**触发方式**（是否也是可连点按钮）
> ➕ **B0191：反证 (b) 被推翻** —— `ApiClient.fetchSettings()`（`api_client.dart:177-183`）
> **没有任何内部串行化**（`await _guard(() => _client.get(...))` 裸发，无队列/无序号/无 in-flight 判据）
> ⇒ **危害未被客户端层消解** ⇒ 本条范围**维持不变**（重叠 `load()` 仍可乱序到达并丢弃草稿）。
> ⇒ **反证 (a) 仍开着**：`load()` 只**写** `_loading`、入口无早退 ⇒ 未核调用方是否禁用按钮。
> ➕ **B0190 结清「是否升 P1」（按模式 Q 改本行）**：
> **升 P1 的条件已被排除** —— `save()` :281 `final SettingsPatch patch = _draft.toPatch();`
> ⇒ **patch 由 `_draft` 派生、不是 `_remote`** ⇒ **过期的 `_remote` 永远到不了线上**
> ⇒ **不存在「旧响应 → 保存 → 写回旧值」**，**本条维持 P2**。
> ⚠ **但危害被改写成更具体的一条（比原措辞更重）**：
> `load()` :267 **`_draft = SettingsDraft();` 是无条件执行的**
> ⇒ **一个迟到的响应会直接丢掉用户未保存的草稿**（而用户只按了一次按钮，只是第二次慢）；
> 而 `save()` 的头注（:280）写着「失败时**保留草稿**」⇒ **本项目自己在意不丢草稿**，
> ⇒ **被一个迟到响应丢掉，与他们自己的口径相冲突**（比「界面显示旧值」更接近数据丢失）。
> **修法范围随之收窄/明确**：守卫必须**同时**保护 `_remote` **与** `_draft`
> （只挡 `_remote` 的一半挡不住真正的危害）。
> 另：B0188 的第二条测试把「load 会丢弃未保存草稿」钉为**设计行为**——那是**用户主动**的顺序 load；
> 对**迟到**响应而言，这个丢弃是**副作用**、不是用户意图 ⇒ 两者必须分开。
> ➕ 本条在写入时**又**只落在 `## BATCH-0189` 段内 ⇒ **模式 Q 第四次**（B0128 / B0144 / B0184 / 本批）⇒ 下方同段的**权威正文**即本条，**请以本行为准**。

---

### F-0199-01 · P2 【Mod 把 `TextDelta` 的 payload 整块写进日志 ⇒ 助手正文逐块落盘并可在诊断面板看到】
> ➕ **B0201：范围确定为「孤例」** —— 另 6 个 Mod 共 **50 处** `logger.` 调用，逐处查参数：
> `memory` 的具名参数是 `{covers}{e}{id}{pct}{reason}{removed}{role}{session}{total}{turn}`（**全是非内容**）；
> 另 45 处是 `{}`（静态文案或已算好的值）⇒ **零处携带 `payload`/`text`/`prompt`/`content`**。
> ⚠ 两处**疑似**命中**已逐一定位、均非内容**：
> `voice-input/commands.rs:105` `logger.warn(&message)` —— 该 `message` 是**固定状态串**
> （:95 `format!("已注入主链（backend={backend}, locale={locale}）")` / :101 忙碌文案），
> **不含转写文本**，且**只在被拒时**记录（:104 `if !accepted`）⇒ 刻意。
> `persona/sessions.rs:270` `path.display()` ⇒ 是**文件路径**（本机日志的常规内容）。
> ⇒ **本条是「一个文件里的一个站点」的孤例** ⇒ 修法的**一行**性质更明确，**发现的精确度更高**。
> ➕ **B0200：修法被源码「定死」了 —— director 已经就是正确写法**（本条的修法据此收紧）
> ```
> let len = payload.chars().count();                    // mod-director/src/lib.rs:711  ← ⭐ **先算长度**
> match self.ledger.record_prompt(payload, …) {         // :712  payload 传给**分析器**、不是日志
>     PromptOutcome::Silent => self.services.logger.info(&format!(
>         "director 收到空正文（**len={len}**），本轮静默：无决策、零副作用"));   // :719-720
> ```
> ⇒ **director 不记 payload，只记 `len={len}`** ⇒ 而那**正是我建议的修法 ①**
> ⇒ ⇒ **这不是一个设计问题，是「让 `external-input` 做 director 已经做的事」**
> ⇒ **同仓、同 Mod 模式、两个 Mod、相反的选择，而正确的那个已在树里。**
> ⚠ **据此降权我的修法 ②**（在 `mod_registry.rs:402` 的 `ModLogger` 层统一收口）：
> **Mod 自己就能选对**（director 已证）⇒ 在宿主层强制会**过度设计**；
> ⇒ **正确修法就是那一行**，② 降为「可选：把这条约定写进 `ModServices` 的文档」。
> ⇒ ⭐ 顺带一条关于 API 形状的观察（**P2 私有汇合**的思路）：
> `ModLogger` 的签名**不区分「记一个标量」与「记一段内容」**，
> 而 director 的 `len=` 表明**这条约定是可学的、且已被遵守过一次**
> ⇒ ⇒ **不需要改 API，只需要让第二个 Mod 也照做**。

file: crates/live2d-ai-mod-external-input/src/lib.rs:242-248
摘录:
```rust
fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {   // :242
    // v1：**仅记录事件（日志）**，不做业务。验证事件经有界 worker 送达。                      // :243
    self.services.logger.info(&format!(
        "external-input 收到事件 {}: {}",
        topic.as_str(),
        payload                       // :247  ← ⭐ **整块 payload 原样进 Mod 日志**
    ));
```
而它订阅的正是（:234-235）：
```rust
registrar.subscribe(ModEventTopic::TurnStarted)?;
registrar.subscribe(ModEventTopic::**TextDelta**)?;
```
**四跳链路（每跳都核到行）**：
| # | 位置 | 发生了什么 |
|---|---|---|
| 1 | `external-input/src/lib.rs:247` | **`TextDelta` 的 payload（= 助手正文块）整块写进 Mod 日志** |
| 2 | `mod_registry.rs:402` | `ModLogger::new(\|_lvl, msg\| tracing::info!(target: "mod", "{}", msg))` ⇒ **透传进宿主日志文件**（B0198 已核） |
| 3 | `log_routes.rs:86` | `read_tail_lines(file_path, MAX_LINES)` ⇒ **逐字转发**给前端（B0198 已核） |
| 4 | `dev_tools_section.dart:1139-1165` | 诊断面板「**日志**」区**显示**这些行（B0197 已核） |

影响: **同一仓库内两种口径**（这是本条的核心，不是「日志里有正文」本身）：
- **宿主侧刻意不记内容**：`dispatch.rs:250/252/254` 只有 `%method, path, route, status`
  ⇒ 与 AGENTS.md 明文「**不记录请求体**（含用户提示词等内容）」**一致**（B0198 已核）
- **Mod 侧整块记内容** ⇒ 且宿主**透传**（`mod_registry.rs:402`）⇒ **Mod 绕过了一条宿主已立的规矩**
- ⇒ 后果：助手回复**逐块**进入宿主日志文件，并**逐字显示**在诊断面板；
  该面板还带「**复制诊断快照**」入口 —— ⭐ 但**快照本身只含标量**（B0197 已核 `_snapshotLines()`），
  **日志不在快照里** ⇒ **复制这条路是干净的**，**显示这条路不是**
- ⇒ **不是 P1**：内容是**助手正文**（**用户输入不经过这条路** —— 该 Mod 只订阅
  `TurnStarted`/`TextDelta`；弹幕文本走 `POST /api/v1/external/chat` 端点，不作为 Mod 事件 payload），
  且落在**本机**（同用户可见）；无远程面、无密钥、无跨用户泄漏
建议: ① 把 :247 的 `payload` 换成**长度 + 首若干字符的摘要**（如 `payload.chars().count()`，
   或前 20 字 + `…`）—— 事件**类型**与**规模**才是排障需要的信息，**正文不是**；
   ② 更根本的一层：**宿主侧** `mod_registry.rs:402` 的 `ModLogger` 应提供**只记 `topic`+`len`**
   的通道（或明确文档化「Mod 日志**原样进宿主日志**」这条代价）⇒ **取舍要被写下来**
   （B0198 已记：透传是「设计」，但**代价当时没写**）
   ③ 若确定要保留正文，则**至少**说明这是**用户可见的本机日志**、并考虑给日志文件加与 `.env`
   同样的 `0600`（`secrets.rs` 已有 `chmod tmp` 先于 `rename` 的先例，B0120）
**为什么定 P2**：可核（4 跳链路全有 quote）· 违反的是**本仓自己写的规矩**（不是外部标准）·
后果限于**本机显示**。**不是 P1**（无远程、无密钥、**无用户输入**、不跨用户）。
验证: `external-input/src/lib.rs:234-235`（订阅）· `:242-248`（记录）·
`mod_registry.rs:402`（透传）· `log_routes.rs:86`（转发）· `dev_tools_section.dart:1139-1165`（显示）·
B0198 的 69 处 `tracing::` 穷举（`dispatch.rs` 无 body）
置信: 高（四跳均核到行）
反证: (a) 试过论证「payload 是**用户弹幕文本**」—— ❌ **自己推翻**：
该 Mod 只订阅 `TurnStarted`/`TextDelta` ⇒ payload 是**助手正文块**；弹幕走端点、不作 Mod 事件
⇒ **这条我先猜错了，已在正文里改正**；
(b) 试过论证「日志会经**复制快照**出去」—— ❌ **不成立**：`_snapshotLines()` 只含标量（B0197 核），
**日志不在快照里** ⇒ **泄漏面只有「面板显示」，没有「剪贴板」**；
(c) 试过论证「这是 `external-input` 独有的」—— **未核**：其余 Mod 的日志内容未逐个查
（director 的 `format!` 参数是 `{payload}`/`{note}`/`{e}`/`{len}`/`{epoch}`/`{cleared}`/`{pitch}`/`{speed}`，
**含 `{payload}`** ⇒ **同一形态可能在 director 侧也存在**，列为未核实。


---

### F-0209-01 · P3 【AGENTS.md 仍把 `syncShellStageBg` 写成现行机制，而代码已在 2026-09-27 换掉它（新默认随之翻转）】
> ➕ **B0211：不是「单字段漂移」，是「一处机制、三处描述」**
> `grep -rn "effectiveShellImage" shell/flutter/lib/` ⇒ **零命中** ⇒ 它已被
> `display_prefs.dart:665` 的 **`get effectiveBackground`** 取代
> ⇒ 而 AGENTS.md `:123` 与 `:641` **都提到 `effectiveShellImage`「是一份真相」**
> ⇒ ⇒ **同一处机制（壳画哪一张图），AGENTS.md 用三种说法描述，其中三种都已失效**：
> ① `syncShellStageBg`（**字段已换成 `backgroundSource` 枚举**）② 默认 `true`（**实为「背景库」**）
> ③ `effectiveShellImage`（**getter 已改名 `effectiveBackground`**）
> ⇒ **发现范围应扩大**：不是「一个字段名写错了」，而是「**一个机制在文档里整体停留在替换前**」。
> 🔻 **B0210 撤回本条的核心论据「AGENTS.md 整体落后 13 天」（我自己推翻）**
> 机械核：`grep -oE "2026-09-[0-9]{2}" AGENTS.md | sort -u | tail -3` ⇒ **09-26 / 09-27 / 09-28**
> ⇒ **AGENTS.md 的正文已更新到 09-28**（代码里的日期标记也只到 **09-28** ⇒ **两者同步**）
> ⇒ **错的只是「变更历史」那一节**（最后一条 2026-09-14），**不是整个文件**
> ⇒ 而本条的**受影响陈述**：AGENTS.md **正文** `:123` 与 `:641` **仍把 `syncShellStageBg` 写成现行机制**
> ⇒ ⇒ **发现本身仍成立**（用户可见的默认值在正文里写错了），但**「无更新条目能覆盖」那句是错的**，
> 且**「落后 13 天」应改为「变更历史一节落后 14 天、正文并未落后」**。
> ⚠ **这是我在同一形态上第 4 次犯错**（B0104「唯一」· B0186 零命中 · B0196 猜名 · 本次「读一节、推广到全文」）
> ⇒ **教训升级**：**「某节落后」不能推出「全文落后」** —— 要推广一个结论，
> **必须先机械核一遍全文**（本批就是靠 `grep -oE` 核出来的）。
> ➕ 规范头（与下方段内正文同源，正文含全部 quote 与反证）。

## BATCH-0209（2026-09-29）· **F-0209-01（P3）：AGENTS.md 仍把 `syncShellStageBg` 写成现行机制，而代码已在 2026-09-27 换掉它（新默认随之翻转）**

file: shell/flutter/lib/settings/display_prefs.dart:152 / :636-659（代码）；AGENTS.md §0.1.0-rc.5 段（文档）
摘录（代码，逐字）:
```dart
/// 读「壳画哪一张」，**含旧字段迁移**。                                        // :636
/// 旧形态的 `syncShellStageBg` 是「壳跟随舞台」，**默认 `true`**。迁移规则：      // :638
/// - **确实有一张舞台图**（`stageImage != null`）→ 迁到「舞台那张」，老用户**看到的界面一个像素都不变**；   // :640-641
/// - **没有舞台图**（绝大多数用户…）→ 迁到「背景库」，也就是**新默认**。      // :642-643
/// 写成「读旧键」而不是「改旧键的默认值」：**改默认值会让「显式设过 true」和「从没设过」分不开，
/// 而那正是本缺陷的成因**。                                                    // :645-646
final bool legacySync = _readBool(json['syncShellStageBg'], **false**);        // :654
return legacySync && stage != null ? backgroundSourceStageImage : backgroundSourceLibrary;   // :656-658
```
文档侧（AGENTS.md §0.1.0-rc.5，仍作为**现行版本**呈现）:
> 「`DisplayPrefs` 增加 `shellImage` 与 **`syncShellStageBg`（默认 `true`）**……**`effectiveShellImage` 是一份真相**」

影响: **文档落后于代码 13 天，且落在一个用户可见的默认值上**：
- 代码里 `syncShellStageBg` **只作为旧键的迁移输入**（:654），**新机制是 `backgroundSource` 枚举**
  （`backgroundSourceStageImage` / `backgroundSourceLibrary`）
- **新默认是「背景库」**（:643）⇒ 而 AGENTS.md 说的现行默认是「**跟随舞台**（`true`）」
- ⇒ **照 AGENTS.md 理解的人会以为「壳默认跟随舞台」**；实际是「壳默认用背景库」
- 且 `display_prefs.dart:152` 自己写着「**为什么换掉 `syncShellStageBg`**（2026-09-27 **修一个真缺陷**）」
  ⇒ **代码里带着修复日期，文档里没有这次修复的任何痕迹**
- ⇒ 时间可核：AGENTS.md 变更历史**最后一条是 2026-09-14**（rc.1）⇒ **没有任何更新的条目能覆盖它**
  ⇒ **漂移确认，不是「另有新章节」**

**⚠ 诚实说明：这不是「实现有缺陷」** —— 恰恰相反，迁移**写得很好**：
把「改旧键默认值」与「读旧键」**两种做法的差别**都写明了（:645-646），并**点出了原缺陷的成因**
（「显式设过 true」与「从没设过」分不开）；`effectiveBackground`（:661-665）还声明
「**判据只有这一处**……判据散到两处就会出现『设置说用背景库、画的不是背景库』这类不一致」
⇒ **正面模式 P2**（私有汇合 + 点名被防的不一致）的**又一次实例**。
建议: ① **AGENTS.md 增一条变更历史**（日期 2026-09-27）：`syncShellStageBg` → `backgroundSource`，
   默认由「跟随舞台」改为「**背景库**」，并**说明迁移规则**（有舞台图 ⇒ 迁舞台；无 ⇒ 背景库）
   与**为什么用「读旧键」而非「改旧键默认值」**；
   ② 顺带把 rc.5 段里 `syncShellStageBg` 的描述标注为**已被 2026-09-27 取代**（**保留原文**，
   与本仓「记录推翻过的方案」的做法一致 —— B0039–B0042 / B0170 核过多次）；
   ③ 这次漂移**不是孤例的风险信号**：AGENTS.md 是**任何 agent（含本审计）最先读的文件**，
   而它的变更历史停在 13 天前 ⇒ **「文档比代码旧」本身值得作为一条独立观察跟进**。
**为什么定 P3**：纯文档；**实现正确且自带迁移说明**；无运行时影响。
**定级说明**：**不是 P2** —— 因为**代码里每一处都写明了**（:152 记了替换与日期、:636-658 记了迁移规则与理由），
**漂移只存在于 AGENTS.md**；而 AGENTS.md 的读者是 agent 与新维护者 ⇒ 影响是「**理解偏差**」，
不是「行为错误」。
验证: `sed -n '634,665p' display_prefs.dart` · `grep -n "为什么换掉 \`syncShellStageBg\`" display_prefs.dart` → :152 ·
AGENTS.md 变更历史最后一条为 **2026-09-14（rc.1）**
置信: 高（代码逐字可读；日期与「最后一条」可核）
反证: (a) 试过找「AGENTS.md 里是否有更晚的条目覆盖它」—— **无**：变更历史止于 2026-09-14，
而替换发生在 2026-09-27 ⇒ **漂移确认**；
(b) 试过论证「新默认其实仍是跟随舞台」—— **不成立**：:643 明写「没有舞台图 → 迁到『背景库』，**也就是新默认**」，
而「绝大多数用户」落在这一支 ⇒ **新默认是背景库**。

### F-0224-01 · P3 【`ModSettingField::String { secret }` 的「内容不落日志」是约定、无机制，且没覆盖真正泄露的那条路】
> ➕ **B0225 修正修法 (b) 的成本估计**（读 `ModLogger` 本体后）
> `services.rs:132-149`：`ModLogger` **已经是唯一日志入口** —— 三个方法都是一行转发
> ```rust
> inner: std::sync::Arc<LogFn>,                       // Fn(Level, &str)
> pub fn info(&self, msg: &str)  { (self.inner)(log::Level::Info,  msg); }
> pub fn warn(&self, msg: &str)  { (self.inner)(log::Level::Warn,  msg); }
> pub fn error(&self, msg: &str) { (self.inner)(log::Level::Error, msg); }
> ```
> ⇒ **不需要「收敛入口」（已经是了）**；修法 (b) 只是**加一个「按字段记」的入口**
> （传 `&ModSettingField` + 值，**由契约决定「secret 字段只记 key 与长度」**）
> ⇒ 而宿主那侧（`mod_registry.rs:402`，B0198 已核）也**已经是那一个闭包**，
> **改一处即可覆盖 8 个 Mod** ⇒ **成本比我在 B0224 写的小得多**。
> ⚠ **但修法的形状必须避开「启发式」陷阱**（B0164 的教训）：
> **不要**在 `inner` 那里加「按字符串模式猜密钥」的过滤器 —— 那是**猜**，
> 而 B0164 刚教过「保守启发式 + 声明误判代价」的代价有多贵；
> ⇒ **正确的形状是「类型」**（结构化字段 + 值 ⇒ 由**契约**决定记什么），**不是「文本模式」**。
> ➕ 规范头（与下方段内正文同源）。


---

## BATCH-0224（2026-09-29）· **F-0224-01（P3）**：`ModSettingField::String { secret }` 的「内容不落日志」是**约定、无机制**，而它**没覆盖真正泄露的那条路**

file: crates/live2d-ai-mod-system/src/settings.rs:20-24
摘录:
```rust
String {
    key: String,
    label: String,
    /// 是否密钥类（前端渲染为 password 输入；**内容不落日志**）。      // :23
    secret: bool,
```
**可核的三件事**：
```
$ grep -rn "secret" crates/live2d-ai-desktop/src/mod_registry.rs     ⇒ **零命中**
```
| # | 事实 | 状态 |
|---|---|---|
| ① | `secret: true` 的**前端半边**（「渲染为 password 输入」）**有机制** | ✅ B0105 已核（面板对 secret 字段**留空不提交**） |
| ② | `secret: true` 的**后端半边**（「**内容不落日志**」）**没有机制** | ⚠ `mod_registry.rs` **零命中** ⇒ **宿主从不读这个 flag** |
| ③ | 已发布的 Mod **确实都没记**（6 个 Mod、50 处 `logger.` 逐处查参数，B0201） | ✅ **无实际泄露** |

影响: **一条被声明、但无机制的保护**，且**它防的那条路恰好不是真正出问题的那条**：
- 「内容不落日志」靠的是**每个 Mod 自己的纪律**（B0201 已核：50 处日志的**参数**全是枚举 / id / 计数 / 长度 / 固定串），
  ⇒ **flag 在这里只是「前端渲染」的开关**；
- 而**真正发生过的**那条路是 **B0120 的 F-0111-01：ASR token 进 `argv[0]` 之后的子进程命令行**
  ⇒ ⇒ **`secret` 覆盖的是「日志」，而泄露发生在「进程参数」** ⇒ **它防错了面**。

建议: ① **二选一，并写清是哪一个**：
   (a) **把 flag 的语义对齐到它真正在做的事** —— 改注释为
   「**仅供前端渲染成 password 输入**；**日志纪律由各 Mod 自负**」，
   并在 `mod-system` 的活文档里把「Mod 不得记录配置值」写成**一条纪律**（像 B0217 的模板那样指名）；
   (b) **或让它成为机制** —— 在 `ModLogger`（`services.rs:134-146`）上做**唯一的日志入口**，
   由调用方传 `&ModSettingField` 而非裸串，从而「secret 字段的**值**无法被传进去」。
   ⇒ **(b) 更彻底**，且正好落在 B0223 观察到的那处：**契约层是唯一能一次性覆盖 8 个 Mod 的地方**。
**为什么定 P3**：**无实际泄露**（B0201 已逐处核）· 声明在字段注释里、无运行时后果 ·
修法成本小。**不是 P2**：不存在「已经发生」的泄露；而「无机制」本身只是**未来的风险**。
验证: `sed -n '18,32p' mod-system/src/settings.rs`（`secret` 声明与注释）·
`grep -rn "secret" mod_registry.rs`（**零命中**）· B0201 的 50 处日志参数逐处核验
置信: 高（`secret` 的宿主侧零命中是机械可复跑的）
反证: (a) 试过找「宿主是否在别处读 `secret`」—— `grep -rn "secret"` 在 desktop 侧只命中
`mods_routes`/Dart 侧（B0105 核过 `redacted_config` 会把 secret 键**从输出里删掉**），
⇒ **「不回显」这一半有机制**（B0066 已核机制在 `mods_routes.rs:494` 的 `obj.remove(key)`），
**缺的只是「不落日志」这一半** ⇒ **本条的缺口比我上面写的更窄**；
(b) 试过论证「这属于红线 R 的第七处缺口」—— **不是**：红线 R 是「密钥不出口」，
而「日志纪律」是**另一条纪律**（`dispatch.rs` 不记 body，B0198 已核）⇒ **归为本条，不并入红线 R**。


### F-0255-01 · P3 【两个**前端自造**的错误码，让「拿界面上的码去日志里搜」必然落空】
> ⚠ **本条曾第四次漏掉规范头**（模式 Q 违规：B0184 / B0189 / **本批**），由**收尾的模式 Q 双向检查**
> 当场抓出并补上 ⇒ **这道检查第四次发挥了作用**。
> ⇒ **四次复发都在同一形态**：**新发现急着写正文、忘了先写头** ⇒ 已立执行提示（**先头后正文**）。


---

## BATCH-0255（2026-09-29）· **F-0255-01（P3）**：两个**前端自造**的错误码，让「拿界面上的码去日志里搜」**必然落空**

file: shell/flutter/lib/api/{api_client,models_api,env_api,mods_api,voice_api,diagnostics_api}.dart；
     shell/flutter/lib/state/ui_state_tracker.dart:169；shell/flutter/lib/chat/turn_liveness.dart:190
摘录:
```dart
throw ApiException('network_error', '无法连接后端：$e');        // api_client:233（另有 5 处同形）
_errorCode ??= 'turn_failed_no_detail';                            // ui_state_tracker:169
const String kTurnFailedWithoutDetailCode = 'turn_failed_no_de…';  // turn_liveness:190
```
**机械对账的结果**（Dart 侧 18 个「像错误码」的 token 逐个回 Rust 查同拼写字符串）：
**12 个对得上** · 6 个查不到，其中 4 个**不是错误码**（`active_id` / `prev_active_id` 是 JSON 字段名、
`error_banner` 是文件名、`invalid_use_of_protected_member` 是 Dart 分析器诊断码）⇒ **真正对不上的是上面这两个**。

影响: **契约覆盖上的一个洞**（AGENTS.md 明文：「用户要拿界面上的码去日志里搜」）：
- **`network_error`** 由 **6 个** API 包装器在**请求根本没到服务端**时抛出
  ⇒ ⇒ **按构造，服务端日志里就不存在对应行**
- **`turn_failed_no_detail`** 是「本轮失败但**没带回任何码**」时的**占位码**
  ⇒ ⇒ 同样，**服务端没有这一行**
⇒ ⇒ 用户把这两个码抄去搜日志，**保证搜不到** ⇒ ⇒ 不是码写错了，而是**这类事件根本没到服务端**
⇒ **定 P3**：无功能影响（这两个码对 UI 自己的分支仍然有用 —— B0227 核过 `network_error` → 「重试」）；
**受影响的只有「搜日志」这一步**，而它**在排障最需要的时候白跑一次**。

建议: **二选一，但 (a) 更便宜也更诚实**：
- **(a) 把这条边界写下来**（与本仓既有做法同形：B0222 点名**不存在的 API**、
  B0244 点名「**不改 `shell_settings.dart` 的硬约束**」）：
  在 `error_banner.dart` 或 AGENTS 的错误码契约处写明
  「`network_error` 与 `turn_failed_no_detail` 是**前端自造**的码，
  **服务端日志里必然没有对应行** —— 这不是丢日志，是事件没到服务端」；
- (b) 或让**这一行日志存在**（前端本地用同名 code 记一条）⇒ 则搜索能命中。
⇒ ⭐ 顺带一条**正面**：这两个码**在六处 API 包装器里拼写一致**（P2 的第五种落点：**码的一侧收敛**，
B0254 已立）⇒ ⇒ **前端这一侧是整齐的**；缺的是**它与服务端的边界说明**。

**为什么定 P3**：无运行时缺陷 · 有现成的自洽实现 · 缺的是**一处说明**。
验证: 机械对账脚本（Dart 18 个 token → Rust `"…"` 字符串）**12/18 命中** ·
`grep -rn "network_error|turn_failed_no_detail" shell/flutter/lib/` ⇒ 8 处，**全部在前端** ·
Rust 侧 `"network_error"` 与 `"turn_failed_no_detail"` **零命中**
置信: 高（机械可复跑；两侧命中数都比过）
反证: (a) 试过论证「`network_error` 也可能由服务端发出」—— `grep -rn '"network_error"' crates/` **零命中**
⇒ **不是同码不同源**，而是**纯前端码**；(b) 试过论证「这不算问题，因为用户看到的是中文提示不是码」——
**不成立**：B0227 核过 `error_banner` 明确按 `code` 分流，且 AGENTS 的契约明写「界面上的码」。



### F-0263-01 · P3 【`presetStatus` 只有 ack 一个写者 ⇒ iframe 重建后旧快照会一直留着】
> ➕ 规范头（与下方段内正文同源）。

---

## BATCH-0263（2026-09-29）· **F-0263-01（P3）**：`presetStatus` 只有 ack 一个写者 ⇒ **iframe 重建后旧快照会一直留着**（新渲染面**没东西可 expire**）

file: shell/flutter/lib/live2d/live2d_stage.dart:365-379（唯一写者）· :380-382（`_handleHostError`）
摘录:
```dart
widget.onRenderEvent?.call(event);
final PresetStatusUpdate update = presetStatusUpdateFor(event, DateTime.now());
switch (update.action) {
  case PresetStatusAction.set:    presetStatus.value = update.status;
  case PresetStatusAction.clear:  presetStatus.value = null;
  case PresetStatusAction.ignore: break;
}
…
void _handleHostError(String message) { if (!mounted) return; setState(() => _hostError = message); }  // :380-382
```
影响: **一个「机制有边界、而边界没被写下来」的缺口**（B0251 已核的未决点，本批结清）：
1. `presetStatus` 的**唯一写者**是 `presetStatusUpdateFor`（**只吃渲染面 ack**），
   而唯一的 `clear` 来自 `replaced / expired / dropped` **ack**；
2. `_handleHostError`（错误后要 `retry` 重建 iframe，`:120` 说过）**只设 `_hostError`，不碰 `presetStatus`**；
3. ⇒ ⇒ **重建之后，新渲染面没有正在播的动画** ⇒ ⇒ 它**不会**发 `expired/dropped`
   ⇒ ⇒ **旧快照不是「留一会儿」，而是可能一直留着**（直到用户做别的操作产生新 ack）
4. **为什么不升 P2**：B0232 已核 `presetStatus` 是「**渲染面 ack 的显示快照**」——
   ⇒ **没有任何行为依赖它**（它不驱动动画、不驱动口型）⇒ **危害限于「诊断区显示了一个已不生效的状态」**
   ⇒ 而诊断区出现陈旧值，属于「看起来没事」那一族的**最小实例**

建议: **一行** —— 在**重建/重试路径**上补一次显式清空
（`_handleHostError` 或 `retry()` 入口 `presetStatus.value = null;`）
⇒ 而这之所以**便宜**，正是因为 B0251 已核：`PresetStatusAction.clear` **已是一等动作**（不必造机制）
⚠ 更一般的教训（**这才是本条的真正价值**）：
> **「清理由 ack 驱动」的设计，在「重建后**没有东西可清**」的场景下会失灵。**
> ⇒ 纯函数 + 单一写者（P27 同族）是好的，但**它有边界**；
> **而机制有边界时，边界必须被写下来** —— 否则下一个人会以为「ack 驱动 = 永不失真」。
建议顺带在 `presetStatus` 的字段文档上补一句
「**仅由 ack 更新；重建后需手动清**（见 `_handleHostError`）」—— 成本一行。
**为什么定 P3**：无行为影响（纯显示）· 触发路径窄（**必须先出错再重试**）· 修法一行。
验证: `sed -n '365,382p' live2d_stage.dart`（唯一写者 + `_handleHostError` 不碰它）·
`grep -n "PresetStatusAction.clear|clear()" live2d_stage.dart`（`:65` 的 `..clear()` 是**另一个** `ActionCue` map，
与 `presetStatus` 无关 —— **两处都核过，不混为一谈**）
置信: 高（两处写者都核到；**并核了那个同名但无关的 `:65`**，避免误报）
反证: (a) 试过论证「重建后 `ready` 会带来一次 `clear`」—— **未核**：`onReady` 的实现里**没有**任何
`presetStatus` 写入（`grep` 全文件只有 :373 一处 `clear` 分支）⇒ **我的结论不依赖这一点**
（**即便它会发，那也只是「有时会清」而非「一定清」**）；
(b) 试过论证「这是 debug 区，无所谓」—— **不能免**：`director_observer_section` 把它当**四栏之一**展示给排障者
（B0239 核过四栏且 `ObserverBuffer` 有上界）⇒ **排障者会读到它** ⇒ 值得一行修复。


### F-0278-01 · P3 【`bg_data_url` 的**字段注释**说走 canvas CSS，而**同一文件 140 行下方**写着这条路已被实测证伪】
> ➕ 规范头（与下方段内正文同源）。

---

## BATCH-0278（2026-09-29）· **F-0278-01（P3）**：`bg_data_url` 的**字段注释**说「应用为 canvas CSS background-image」，而**同一个文件 140 行下方**写着这条路已被实测证伪

file: crates/l2d-wasm-demo/src/web/surface/input.rs:100-102（**过时的说法**）vs :236-247（**正确的说法**）
摘录:
```rust
// input.rs:100-102  —— 【过时】
/// stage-bg.dataUrl（自定义背景图 dataURL；None = 无背景图）。
/// 应用为 **canvas CSS background-image**（cover 缩放），与 background-color 共存。
pub bg_data_url: Option<String>,

// input.rs:236-247  —— 【正确，且就在同一个文件里】
// 舞台底色与背景图**不在这里**画。
// 2026-09-14（rc.5）：这里**曾**把 stageColor / 背景图写成 canvas 的 CSS
// `background-color` / `background-image`，靠「canvas 像素透明」把 CSS 露出来。
// **实测证伪**：WebGPU 的 canvas 只能不透明合成（见 `background.rs` 头注），
// 透明 clear 的像素被合成为不透明黑，把 CSS 整块盖住。
// 现在两者都由 [`super::background`] 在**背景预通道**里画进 framebuffer：
// `stage_color` / `dark` 决定 clear 值，`bg_data_url` 决定要不要贴纹理。
```
**三方对照（都机械可核）**：
| 来源 | 说的是 |
|---|---|
| `web/surface.rs:8` | 「[`background`]：舞台底色与背景图**画进 frameb…**」 |
| `web/surface/background.rs:1-3` | 「把 stageColor 纯色底 + 背景图画进 **framebuffer**，**而不是依赖 canvas 的 CSS**」 |
| `background.rs:5-13` | WebGPU canvas 只能不透明合成（wgpu 29 只报 `Opaque`）⇒ 透明 clear 变**不透明黑** ⇒ **盖住 CSS** |
| `input.rs:236-247` | 「**不在这里**画」+「**实测证伪**」+「现在…**画进 framebuffer**」 |
⇒ ⇒ **四处一致说 framebuffer；只有 `input.rs:100-101` 说 CSS** ⇒ ⇒ **过时的是那一条**

影响: **不只是文案陈旧，而是「会把人引到错误的层去查」**：
`bg_data_url` 是那条**消息结构体上的字段**，而它的文档告诉读者「这个值会被写成 canvas 的 CSS」，
⇒ ⇒ 将来若背景不出图，**顺着这条注释去查 CSS / `stage_css.rs` 会一无所获**，
⇒ ⇒ 而真正的落点在 `web/surface/background.rs` 的**背景预通道**。
⚠ 附带：AGENTS rc.4 ⑦ 记的是「CSS 写入**已删除**、`stage_css.rs` 随之删除」⇒ **与本条同向、不是漂移**
⇒ ⇒ **所以这一条是「一处注释落后于自己文件里的正确说明」，不是「文档与实现分歧」**。

建议: 把 `input.rs:100-102` 的两句注释改成与 `:236-247` 同口径
（「**用于背景预通道：`stage_color` 决定 clear 值，`bg_data_url` 决定要不要贴纹理；
**不**走 canvas CSS —— 那条路已被 WebGPU 的不透明合成证伪，见 `background.rs` 头注**」），
**并加一句指向 `background.rs`** ⇒ 成本两行、零风险。
**为什么定 P3**：**只影响一个字段注释的准确性** · 无运行时后果（实现是对的）·
· **不升 P2** 的理由：它**不是「文档与实现不一致」那种会让人改错代码的偏差**，
而是**同一文件里两段注释一新一旧**，读文件的人（尤其是只搜字段名的）**会拿到旧的那段**。
验证: `sed -n '100,102p' web/surface/input.rs`（过时）· `sed -n '236,247p'` （正确）·
`sed -n '1,13p' web/surface/background.rs`（第三方一致）· `sed -n '8p' web/surface.rs`（第四方）
置信: 高（四处引用 + 两处原文，行号全部可复跑）
反证: (a) 试过论证「也许 CSS 那条在某个平台/某条路径上仍生效」—— **未证**：我未读 `apply` 的分支体；
但**即便有例外路径**，该注释**也没有说「仅在某条件下」** ⇒ **它至少是不完整的**，
⇒ ⇒ 而「不完整」已足够支撑 P3 ⇒ **不因未证例外而上调**；
(b) 试过论证「这只是 AGENTS 已经说过的事」—— **不是**：AGENTS 说的是**实现层**（CSS 写入已删），
**没说这个字段的注释还在描述那条被删的路** ⇒ ⇒ **本条的对象是那两行注释本身**。


### F-0294-01 · P3 【自检成功路径上 **`o.note` 被丢弃** ⇒ 服务端那句「有解释价值的话」在 UI 上无处落地】
> 🔻🔻 **B0295 更正本条（我 B0294 的「两个消费点同构」是错的）**
> 我在 B0294 写「`testTts` 的消费点 `:1065` 附近**同构**」⇒ **那一句错了**。
> ```
> $ grep -rn "\.note\b" shell/flutter/lib/ --include=*.dart
> lib/app/app_shortcuts.dart:28      this.note,                       ← **另一个类型**的 note
> lib/app/shortcut_help_dialog.dart:45  if (e.note != null) … e.note!,  ← 同上（快捷键说明）
> lib/main.dart:1073   ? 'ok · ${o.latencyMs ?? '?'} ms**${o.note == null ? '' : ' · ${o.note}'}**
> lib/api/settings_models.dart:767  this.note,
> ```
> ⇒ ⇒ **TTS 自检（`:1073`）把 `note` 显示出来了** ⇒ **更强说法（「全局无任何展示」）被推翻**
> ⇒ ⇒ ⭐⭐ **而真正的缺口只在 LLM 自检（`:1046-1050`）** ⇒ **本条范围收窄到一处**
> ⇒ ⇒ ⭐⭐⭐ **而「两个同族功能实现不一致」本身就是缺陷的一部分**：
> 同一条规则（「成功但有话要说」）在**两个同族功能**上写法**不同**
> ⇒ **收敛的代价 = 零**（两行字符串拼接）⇒ **没有任何理由不一致**
> ⇒ ⇒ 与 **B0249**（`chat_session` 的重复**可接受**，因为收敛会引入层间倒挂）**正相反**
> ⇒ ⇒ **最佳修法就在同一个文件的三十行之下**：把 `:1046-1050` 按 `:1073` 的写法改
> ⇒ ⇒ 而按 B0284 的家族：**「谁负责解释」应当只有一处** ⇒ 此处就是**把两个同族写法对齐**
> ⚠ **同名不同型**：`app_shortcuts.dart` / `shortcut_help_dialog.dart` 的 `note` 是**快捷键说明**的字段，
> **与 `TestOutcome.note` 无关** ⇒ **记录在此以免后人 grep 时被误导**
> ➕ 规范头（与下方段内正文同源）。

---

## BATCH-0294（2026-09-29）· **F-0294-01（P3）**：自检成功路径上 **`o.note` 被丢弃** ⇒ 服务端那句「有解释价值的话」在 UI 上无处落地

file: shell/flutter/lib/main.dart:1046-1050（消费点）· shell/flutter/lib/api/settings_models.dart:780-784（字段与规则）
摘录:
```dart
_llmTest = o.ok
    ? 'ok · ${o.latencyMs ?? '?'} ms'
          '${o.modelEcho == null || o.modelEcho!.isEmpty ? '' : ' · 模型：${o.modelEcho}'}'   // :1047-1048
    : '失败：${o.errorMessage ?? o.errorCode ?? '未知原因'}';
```
（`testTts` 的消费点 `:1065` 附近同构。）
而字段侧（`settings_models.dart:780-784`）逐字写着：
> 「**成功但有话要说**（例如『上游可达但未提供 /models』）……**只有真有解释价值时才非空**
> —— 服务端**刻意不在成功路径上默认塞一句**……」

影响: **服务端那条裁决的「+ 一句 `note` 说明」在 UI 上落地不了。**
- AGENTS 记的裁决是：404 **算通过**（上游回话即可达，`/models` 并非所有语音实现都提供）**+ 一句 `note`**；
- 服务端据此产出 `note`；Dart 侧**把它建模成字段**且**为它写了规则**；
- ⇒ 而**成功分支只读了 `latencyMs` 与 `modelEcho`** ⇒ **`note` 没有任何消费点**
- ⇒ ⇒ **用户看到的是「ok · 12 ms · 模型：X」**，**看不到「为什么这次是通过而上次是失败」**
- ⚠ **这不是「假装成功」**：`ok` 仍来自服务端的 `ok`，`errorCode`/`errorMessage` 也在失败分支完整呈现
  ⇒ **问题是「有意提供的解释被丢在半路」**，不是「把失败说成成功」

建议（两条路径，任选其一，**改的都是一处**）：
① **成功分支带上 note**（一行）：`'ok · N ms · 模型：X'${o.note == null ? '' : '（${o.note}）'}'`
   ⇒ 与字段侧「**成功但有话要说**」的语义**对齐**；
② 或**在 Dart 侧删掉 `note` 字段与其规则注释**，并在服务端那条裁决处注明
   「note 由 v1 客户端忽略」⇒ ⇒ **让「谁负责解释」只有一处**（B0284「必须精确到『不假装 X 而仍要做 Y』」）

**为什么定 P3**：**无功能影响**（`ok` 正确）· 缺失的是**解释**而不是**判定** · 改法一行或一处。
**不升 P2** 的理由：**没有任何用户会因此做出错误判断**（结果是「通过」，与裁决一致），
**被损害的是「知道为什么」的便利**。
验证: `sed -n '1040,1060p' shell/flutter/lib/main.dart`（两处消费点同构）·
`sed -n '780,784p' lib/api/settings_models.dart`（字段规则）·
`grep -rn "SettingsTestOutcome" lib/` ⇒ **消费点只有 `main.dart:1043` 与 `:1065` 两处**，
**而 `:1047` 的成功分支不含 `note`** ⇒ **`note` 确无消费点**
置信: 高（三个 grep/读全部可复跑；「无消费点」是**穷举**结论而非抽样）
反证: (a) 试过论证「note 可能在别处被显示（如 banner/toast）」—— **未核**：
`grep -rn "\.note" lib/` 我只查了 `shell_admin.dart`；**`main.dart` 之外的 banner 路径未逐一排**
⇒ **本条只声称「`main.dart:1046-1050` 这条成功分支不显示它」**（这是可证的），
**而「全局无任何展示」这一更强说法我未核** ⇒ **建议接手者先跑一次 `grep -rn "\.note" shell/flutter/lib/` 再定修法**
(b) 试过论证「404 通过那类场景很少、note 多数时候是空的」—— **可能为真**，但这**不改变字段被丢弃的事实**，
且按 B0293 的规则「**只有真有解释价值时才非空**」⇒ **恰恰是「有值的那次」被丢掉了** ⇒ **本条更成立**。

### F-0387-01 · P2
file: crates/live2d-ai-mod-voice-input/src/lib.rs
摘录:
> //! # 配置字段（settings_spec v1）                                  // :40
> //! | key | 语义 |                                                  // :42
> //! | `backend` | `mock`（缺省）＝ 由集成方/测试喂文本；`sidecar` ＝ 外部识别进… |
> //! | `locale` | 识别语言提示（缺省 `zh-CN`）→ **只**影响 text 归一化档 |
> //! | `token` | 可选访问令牌（secret）；空 = 不鉴权，且**永不**回读明文 |

而 `voice_input_settings_spec()`（`:151` 起）实际声明**九**个 key：
`wake_phrase` · `manual_enabled` · `backend` · `locale` · `token` ·
`sidecar_script` · `sidecar_url` · `sidecar_transcriber` · `sidecar_python`

影响: 头注那张表是这个 Mod 的「配置字段」索引，而它**漏了 6 个键**，
其中 **`wake_phrase` 与 `manual_enabled` 正是 `web_api/voice_routes.rs` 四个闸门读的那两个**
（`manual_enabled=false` → `403 voice_manual_off`；`wake_phrase` 显式空 → `403 voice_gate_closed`）。
⇒ 查「唤醒闸在哪配 / 能不能关」的人**先读这张表**，会得出「闸门不可配」的结论。
⇒ 而 `GET /api/v1/mods` 把它交给 Flutter 按 spec 渲表单（B0364 已核），**所以配置面板上确实有这两项** ——
**即「面板里有、文档里没有」**。

建议: 把 `lib.rs:42-46` 那张表补到九个键，至少先补 `wake_phrase` / `manual_enabled` 两行
（一行表格的成本，堵住「闸门不可配」这个误判）。
或改口径：表头写成「**常用**配置字段」并指向 `voice_input_settings_spec()` 为准。

验证: 逐字比对 `voice_input_settings_spec()` 里的 `key:` 与表里的行；两边计数应相等（当前 9 : 3）。

置信: 高（两处均已逐字读取并机械计数）
反证: 若该表另有声明「只列常用」的口径，则本条降为文档措辞问题；**但我在 `:40-50` 未见此类声明**。
> 🔎 **B0388 横扫结论：本条是 `voice-input` 独有的，不推广。**
> 四个 Mod 的机械对照：`external-input` 4:7（**表比代码多**）· `voice-input` **9:3**（本条）·
> `pet-desktop` 3:3 · `persona` 8:2（**那张表是「命令」表** `import_card`/`clear_import`，
> 而那 8 个 key 是 **SillyTavern 卡片字段**、不是配置旋钮）
> ⇒⇒ **三种不同情况都表现为「计数不等」⇒ 计数不是判据**
> ⇒⇒ **逐项确认「那张表自称为是什么」之后，本条才成立**（表自称「配置字段（settings_spec v1）」）

  若 `wake_phrase`/`manual_enabled` 另有一份文档覆盖，则本条不成立 —— **仓库内我未找到第二处**。

### F-0411-01 · P2
file: shell/flutter/lib/state/live_region.dart
摘录:
> /// 句末标点（**与 `live2d-ai-runtime` 的分句标点对齐**）。
> const String kSentenceEnders = **'。！？…；\n'**;                        // :19-22

而 `live2d-ai-runtime/src/dialogue/mod.rs:11-12` 对 `sentence::SentenceAssembler` 的记载是：
> 「中英文 **`.?!。！？…\n`** 均为句界」

**逐字符比对（两侧各自读出）**：

| 字符 | Dart `kSentenceEnders` | Rust `SentenceAssembler` |
|---|---|---|
| `。` `！` `？` `…` `\n` | ✅ | ✅ |
| **`.`（ASCII 句点）** | ❌ **缺** | ✅ |
| **`?`（ASCII 问号）** | ❌ **缺** | ✅ |
| **`!`（ASCII 感叹号）** | ❌ **缺** | ✅ |
| **`；`（全角分号）** | ✅ **多** | ❌ |

影响: ① **注释里的「对齐」断言不成立**，两个方向都不同 ——
**Dart 少三个 ASCII 标点** ⇒ **英文句子结尾不触发「立刻播报」**，退化为 1.5 s 限速兜底
（**体验降级、不丢信息**）；**Dart 多一个全角分号** ⇒ 在一个「runtime 还不会切句」的位置提前播报
（**提前、不丢内容**）。
② 顺带修正本审计自己的一条说法：我在 B0350 记「读屏播报与切句**共用**『整句到达』判据」，
**当时的依据是注释**；逐字比之后两侧集合并不相同 ⇒ 那条说法**只对「判据的形状」成立**、
**对「标点集合」不成立**。

建议: 二选一，并让注释与代码一致（**一行改动**）：
- 把 `kSentenceEnders` 改成 `'。？！…；.?!\n'` 之类**与 runtime 逐字相同**的集合，
  并把注释改成「**取 runtime 的同一集合**」；或
- 若前端**有意**只要全角（读屏按中文播报），把注释改成「**只认全角**（英文句尾走 1.5 s 兜底）」，
  并把 `；` 的意图写清。

验证: 两侧集合逐字相等（可用一条跨语言断言把 runtime 的集合导出给 Dart 测试，
> 🔎 **B0413 性质修正：本条不是「枚举不一致」，是「**缺了一档**」**
> `sentence.rs:123-136` 揭示安全阀是**三层降级**：
> | 层 | 切点 | 切出来的东西 | 用户会不会觉得坏 |
> |---|---|---|---|
> | 正常 | **真实句读** `.?!。！？…\n` | 一句一单元 | 不 |
> | 降级① | **弱标点** `，、；：` | 自然停顿 | 不 |
> | 降级② | **字符位置硬切** | 一句话被劈开 | **会** ⇒ **最后一层** |
> ⇒⇒ **⇒ 排序判据是「这一层切出来的东西，用户会不会觉得是坏的」**
> ⇒⇒⇒⭐⭐ **⇒ `；` 在 Rust 侧不是句界、它是「降级①」的切点；而 Dart 侧把降级①的点当成了正常档**
> ⇒⇒⇒⇒⇒ **⇒ 修法也随之更准**：不是把 `；` 拉进 runtime 那一档，
> **而是让 Dart 侧明确「我只有句界这一档」，或补上「停顿点」那一档、但显式写明它**不**构成「立即播报」的理由**
> ⇒⇒ **（另**：「正常路径**绝不**走到这里」「旧值 `48` 会被按字符位置硬切」两处
> 都是**先说这条路本不该命中 / 曾经命中过并造成了什么**的写法）**

避免「对齐」只是一句注释）；或在 `live_region_test.dart` 里逐个字符列出期望集合。

置信: 高（两侧均逐字读出并机械比对，差异表见上）
反证: 若 `SentenceAssembler` 实际判据另有出处、而 `mod.rs:11-12` 的这句只是过时注释，

> 🔎 **B0451 前提证成、性质收紧**：`lib/settings/mods/voice_input_panel.dart:77` 有 `'wake_phrase_set': '唤醒词已自定义'`；
> 且 `dev_tools_section.dart:706/712/715` 的 spec 驱动表单**同时处理 `string` 与 `bool`**
> ⇒⇒ **两个闸门字段各有一个输入框** ⇒⇒⭐⭐⭐ **「面板里有、文档表里没有」成立**
> ⇒⇒⭐⭐⭐⭐ **⇒⇒ 性质收紧**：**不是「功能缺失」、是「索引与实现不同步」**
> ⇒⇒ **「功能在、索引缺」比「功能缺」轻得多** ⇒⇒ **而本条的修法（补两行表格）正好对应这个性质**
> ⇒⇒ **并**：`voice_input_panel.dart:77` 是 B0426「值归服务端、文案归客户端」的**第五次出现**

> 🔎 **B0412 加强（本条**加强**、不撤回）** —— 我在上一批写的反证条款**被用上了**，结果是**反证不成立**：
> `crates/live2d-ai-runtime/src/dialogue/sentence.rs:22-24`
> ```rust
> /// 句终止符集合：中英文句读 + 换行（**协议固定，不含分号/冒号等弱标点**）
> fn is_terminator(c: char) -> bool { matches!(c, '.' | '?' | '!' | '。' | '！' | '？' | '…' | '\n') }
> ```
> ⇒⇒ **代码逐字证实了 `mod.rs` 的记载** ⇒⇒ **差异表的 Rust 侧那半完全正确**
> ⇒⇒⭐⭐⭐ **而 `；` 那一半更锋利**：`；` 被显式归入 `is_soft_break`（弱标点），
> 且 `is_terminator` 的文档**逐字写着「不含分号/冒号等弱标点」**
> ⇒⇒⇒ **⇒ Dart 把 `；` 放进 `kSentenceEnders` = 在一个「**协议明确规定不是句界**」的位置立即播报**
> ⇒⇒ **这不是「多了一个无害字符」，是「与一条显式协议约定相反」**
> ⇒⇒ **而 Rust 侧给的理由是「协议固定」⇒ 集合是契约、不是实现细节**
> ⇒⇒ **⇒ 「让 Dart 引用 runtime 的同一集合」的建议因此有了更强依据**
> ⇒⇒ **而根因也清楚了**：`is_soft_break`（停顿点）这个概念在 Dart 侧**不存在** ⇒ **缺了那一档**

则本条的 Rust 侧依据不成立 ⇒ 但**「三 个 ASCII 标点缺失」这一半仍成立**（Dart 侧逐字可读），
**故本条至少是一个 P3 级的注释不准确**，不会整体失效。

### F-0433-01 · P3
file: shell/flutter/lib/settings/sections/dev_tools_section.dart
摘录:
> // ── 导演可观测（阶段5 W5a，D40–D43）────────────
> // 四栏只读观测（A 决策参数 / B 事件流 / C 传参对照 / D 送 TTS 文本）
> // **只在 devMode 下渲染**——off 时**整块不在语义树**（本 if 块一起消失）     // :1296-1298

而全仓 `test/*.dart` 里**唯一**断言语义树的是 `test/audio_bar_test.dart:173-183`
（`tester.ensureSemantics()` + `find.bySemanticsLabel('主音量')`，属主音量滑杆的无障碍回归）。
**没有任何测试断言「devMode 关闭时导演可观测那整块不在语义树」。**

影响: 注释里的「整块不在语义树」**今天为真**（`if (devMode)` 不构建该 widget），
但它是**唯一一处把「无障碍树里没有这个节点」当成可断言事实写下来的地方，而没有对应的断言**。
⇒⇒ 按 P24「让新增/变更必然失败」的原则（B0242 的字体 `fieldCount`、B0335 的穷举表、
B0333 的「相位只被派生一次」都是同一原则）：**把 `if (devMode)` 改成
`Opacity(opacity: devMode ? 1 : 0)`（一种常见的“可见性重构”）时，没有任何东西会变红**，
而后果是**读屏用户会读到整块导演观测面板**（A/B/C/D 四栏 + 事件流 + TTS 文本）。
⇒⇒ 这是**回归防护缺口**，不是当前缺陷 ⇒ 故 P3。

建议: 一条测试即可（注释已经把该断言的形状写好了）：
```dart
testWidgets('devMode 关闭时导演可观测整块不在语义树', (tester) async {
  final SemanticsHandle h = tester.ensureSemantics();
  await tester.pumpWidget(app(devMode: false));
  expect(find.bySemanticsLabel(contains('送 TTS')), findsNothing);   // 或 find.byType(DirectorObserverSection)
  h.dispose();
});
```
⇒ 与 `audio_bar_test.dart:173-183` 同一手法（`ensureSemantics()` + `bySemanticsLabel`）。

验证: 先写测试、在当前代码上通过；再把 `if (devMode)` 换成 `Opacity(...)`、确认测试变红。
> ➕ **B0435 落点**：本条该进的地方是 `test/wiring_test.dart:51` 那个 group
> （`group('必须被接线的构造（点名，每个都写清后果）')`，146 行、已守四条接缝：
> `StageHost` / `shortcutHelp` / `LiveRegionThrottle` / `EpochGate`）。
> ⇒⇒⇒⭐ **「加一行」从建议变成「照抄本仓已有清单的一行」**
> ⇒⇒⇒⭐⭐ **并且**：B0434 那句「分层之后没人补跨层断言」经横扫**不成立为普遍问题** ——
> 这一层是**有系统覆盖**的，本条**只是清单少一项** ⇒⇒ **故本条性质是「清单缺口」而非「缺一层测试」**
> ⇒⇒⇒⭐ **而清单式测试的失败模式是「清单会过期」** ⇒⇒ **同 B0344「门禁防的是过期合规」同一形状**


置信: 中高（注释与代码的形状已逐字读出；「无对应测试」这一点由全仓 `semantics|bySemanticsLabel` 检索得出，
> 🔎 **B0434 范围收窄与反证结清（本条**不撤回**）**
> - **反证条款结清：没有等价断言。** 且 **`devMode` 门控这个模式本身是有测试的** ——
>   `test/wiring_test.dart:117` `testWidgets('历史轮数只在 devMode 出现')`（`:123 false` / `:134 true`）
>   ⇒⇒⇒ **修法因此是「照抄一条已存在的测试形状」** ⇒⇒ **成本比原先写的更低**
> - `test/director_observer_test.dart:135/171/230/276` **直接构造 `DirectorObserverSection`、绕过门控**；
>   该文件里 `devMode: false` **零命中** ⇒⇒ **这不是疏忽，是测试分层的正常结果**
> ⇒⇒⇒⭐ **真正的缺口是「分层之后没人补一条**跨层**断言」**
> ⇒⇒⇒⭐⭐⭐ **本条范围精确成一句话**：「`devMode` 门控有测试（对聊天基建那块），但**没有一条跨层断言**
> 覆盖『从面板进去时导演观测整块不在语义树』」⇒⇒ ⇒⇒ **缺口不是「忘了测」，是「分层之后没有一条跨层的断言」**

但**未逐个通读 `test/` 全部文件** ⇒ 若存在一条用 `find.byType(DirectorObserverSection)` 而不用语义树的等价断言，
本条的**影响**会下降、而**建议**依然成立）
反证: 若导演观测面板的可见性由别处保证（例如整个设置面板在非 dev 模式下不可达），
则本条的**风险显著降低** ⇒ **未核这一点** ⇒ 但即便如此，注释里那句「整块不在语义树」仍然是一个
**没有断言支撑的自述**，而按 B0418 核的「可核对的自我描述」标准，这一句恰好**不是**那种。

### F-0436-01 · P3
file: shell/flutter/test/wiring_test.dart
摘录:
> test('**`EpochGate` 的判据被 `ChatController` 用上**（钉子 13 真的生效）', () {      // :77
>   expect(referencedOutside('**mustInterruptAudio(**', 'lib/audio/epoch_gate.dart'), isTrue);   // :79
>   expect(referencedOutside('**decideAudioFrame(**',   'lib/audio/epoch_gate.dart'), isTrue);   // :82
> });

全仓检索（`grep -rn "EpochGate" --include=*.dart .`）**只有这一行测试名**命中 ——
**`EpochGate` 不是一个存在的类型名**；`lib/audio/epoch_gate.dart` 存在（被两条断言按路径引用）。

影响: **测试名承诺的强于断言给出的**：
- 名字说「**被 `ChatController` 用上**」⇒⇒ 读者会以为断言里出现了 `ChatController`
- 断言实际查的是「**`mustInterruptAudio(` / `decideAudioFrame(` 被 `epoch_gate.dart` **文件外**引用**」
  ⇒⇒ **任何一个别的文件引用它们，这两条断言都通过**
⇒⇒ 而这条测试**不是废的**：它守的是「判据不被内联回调用方」⇒⇒ **断言本身有意义**
⇒⇒ 问题是**名字与断言的落差**，不是断言无效 ⇒⇒ 故 P3、非 P1/P2。

另一层（B0435 说的「清单会过期」在**同一条清单**上的第一次实证）：
`EpochGate` 这个**类型名**在代码里不存在 ⇒⇒ 而它被写进一个承诺「**每个都写清后果**」的清单里
⇒⇒ ⇒ **清单的第一个条目用了一个仓库里没有的名字** ⇒⇒ **而这不是「漏一项」，是「多了一个不存在的项」**

建议: 二选一（**各一行**）：
- 把测试名改成它真正断言的那一句：``test('`epoch_gate.dart` 的两个判据被文件外引用（不被内联回调用方）')``；
  **或**
- 保留现在的名字、**补第三条断言**把 `ChatController` 点名（例如
  `expect(referencedBy('ChatController', 'mustInterruptAudio('), isTrue)` 一类），
  让名字与断言同强。
⇒⇒ 并**顺手把 `EpochGate` 这个不存在的类型名从清单里去掉或换成文件/函数名**。

验证: 改名或补断言后，测试仍通过；`grep -rn EpochGate --include=*.dart .` 应只剩真实存在的名字。
> ➕ **B0441 正解（对 F-0436-01 修法的重要补充）**：「排除注释」在这个仓**已经是 helper 级** —
> `test/background_hydration_window_test.dart:228-252` 用 **`stripCommentsAndStrings(src)`** 再断言，
> 并在 `reason` 里写明「**代码路径，不是注释里提一句**」。
> ⇒⇒⇒⭐⭐⭐ **⇒ 「强的那种不是没被写，是**没有成为默认**」**
> ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ 因此本条的**首选修法**是
> 「**让 `referencedOutside` 也走那个 helper**」，而不是新造一个反向对照** ⇒⇒⇒⭐⭐⭐
> ⇒⇒⇒⭐ **⇒ 同层已有正解可抄**（B0381 的合成样例 / 本条的 helper）⇒⇒ **两个先例都在同一层**

> 🔎 **B0439 修正「唯一一处」**：`test/stage_overlay_single_test.dart`（135 行）**也有自我证明**，
> 且**形态不同** ⇒⇒ **「这条纪律在本仓有两种形态、都存在」**：
> | | `visual_language_test`（B0381） | `stage_overlay_single_test`（B0439） |
> |---|---|---|
> | 手段 | 扫源码 + 合成样例 | **真 widget** + **存在/不存在成对** |
> | 自我证明 | 「扫描器能抓到违规」 | **`find.textContaining('加载中') findsNothing`** |
> | 承认脆弱性 | — | **`reason: '找不到 _StageBadge，测试需要跟着改'`** |
> ⇒⇒⇒⭐⭐⭐⭐ **⇒ `stage_overlay_single_test` 的 `reason` 是「自我证明」的另一种写法**：
> **它不说「我不会坏」，它说「我坏了会这样说」** ⇒⇒ **「测试会随重构一起坏」是结构测试的固有属性，
> 而本仓把���个固有属性写进 `reason`** ⇒⇒ **修法 F-0436-01 因此有两种同层先例可循**

> ➕ **B0438 规模依据**：「扫源码字符串」的测试在 `test/` 里有 **30 个文件**
> （`wiring_test.dart` 146 行 … `display_prefs_test.dart` 892 行）。
> **我已核注释处理强度的只有 2 处**（`visual_language_test` 排除 · `referencedOutside` 不排除），**其余 28 未核**。
> ⇒⇒⇒⭐ **⇒ 所以「两处强度不同」是一个观察，而「30 处强度相同」不是结论。**
> ⇒⇒⇒⭐⭐ **⇒ 而「是否排除注释 / 是否指名调用方」这两条差异，在任何一个文件里都没有被声明**
> ⇒⇒⇒⭐⭐ **⇒⇒ `visual_language_test` 是唯一一处把它做成「反向对照」的（B0381 已核）**
> ⇒⇒⇒⭐⭐ **⇒⇒ 而 `wiring_test:77` 本可以照抄 B0381 的做法、而没有** ⇒⇒ **本条的修法因此有同层先例可循**


置信: 高（测试名与两条断言已逐字读出；`EpochGate` 全仓只在此行出现，已用全仓检索确认）
反证: 若 `referencedOutside` 的实现比名字暗示的更强（例如它带一个「只认某个调用方」的参数），
> 🔎 **B0437 反证结清** —— `referencedOutside` 的实现（`test/wiring_test.dart:38-49`）：
> ```dart
> bool referencedOutside(String symbol, String ownFile) {
>   final sources = Directory('lib').listSync(recursive: true).whereType<File>()
>       .where((f) => f.path.endsWith('.dart'))
>       .where((f) => !f.path.endsWith(ownFile)).toList();
>   for (final file in sources) {
>     if (file.readAsStringSync().contains(symbol)) return true;
>   }
>   return false;
> }
> ```
> ⇒⇒⭐⭐⭐⭐ **① 反证结清：它只有 `(symbol, ownFile)` 两个参数、不接受「调用方」**
> ⇒⇒⇒ **⇒⇒ 「被 `ChatController` 用上」这个承诺确实没有断言支撑**
> ⇒⇒⇒⭐⭐ **⇒⇒ 「建议补第三条断言」成立**、而「删掉多余参数」那条分支**不成立**
> ⇒⇒⭐⭐⭐ **② 它是 `contains(字符串)`、不解析符号 ⇒ 注释里提到也算命中**
> ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ 同仓另一条扫源码字符串的测试（`visual_language_test`，B0381 已核）
> **刻意排除注释**（有反向对照：注释里的 `**` 不该被扫出来）⇒⇒⇒ **两者强度不同**
> ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 「同一种手法在同一个仓里被用成两种强度」，
> 而**弱的那一种恰好被写在一条承诺更强的测试名下** ⇒⇒ **一行注释就能让 `:77` 变绿**
> ⇒⇒⭐⭐ **③ `!f.path.endsWith(ownFile)` 排除了自己那个文件 ⇒ 「在别处被引用」被正确检查**
> ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ 「意图对、强度弱」—— 这就是本条的准确性质**（不是「测试无效」）
> ⇒⇒⭐ **④ 它扫 `lib/`（不含 `test/`）⇒ 与「必须被接线」的意图一致** ✅
> ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ **本条仍为 P3、不升级**（当前无缺陷、是回归防护与命名的落差）
> **但新增一层**：「被文件外引用」这条断言**可以被一行注释满足**，而它的名字说的是
> 「被 `ChatController` 用上」⇒⇒ **⇒ 两层落差** ⇒⇒ **⇒ 修法相应地是「**排除注释**」+「改名或点名调用方」两处**

则断言强度与名字一致 ⇒ **但本批只读到调用处、未读 `referencedOutside` 的实现**
⇒ **⇒ 「建议：补第三条断言」在那种情况下变成「删掉那个多余的参数」** ⇒⇒ **故本条的不受影响部分（`EpochGate` 是不存在的名字）仍然成立**

### F-0443-01 · P3
file: shell/flutter/test/（8 个文件各含一份**逐字节相同**的 helper）
摘录:
> `String stripCommentsAndStrings(String src) { … }`

**机械证据**（逐个取定义起 15 行、去空行、`md5sum`）：

| 文件 | 行 | 长度 | md5 |
|---|---|---|---|
| `asset_guard_preset_dispatch_test.dart` | 79 | 467 | `96c4bad6` |
| `transient_results_test.dart` | 33 | 467 | `96c4bad6` |
| `no_client_side_preset_ttl_prediction_test.dart` | 31 | 467 | `96c4bad6` |
| `asset_guard_stage_attach_resend_test.dart` | 36 | 467 | `96c4bad6` |
| `glass_rim_test.dart` | 26 | 467 | `96c4bad6` |
| **`design_tokens_test.dart`**（它的家） | 18 | 467 | `96c4bad6` |
| `asset_guard_director_cue_test.dart` | 40 | 467 | `96c4bad6` |
| `motion_wiring_test.dart` | 30 | 467 | `96c4bad6` |

**八份全部逐字节相同。** 而**另有 3 个文件已经从 `design_tokens_test.dart` import 它**
（`design_tokens_lint_test.dart:5` · `no_backdrop_filter_test.dart:5` · `background_hydration_window_test.dart:37`）。

影响: **8 份 × 15 行的完全重复**，分布在 8 个测试文件里。
⇒⇒ 修改「剥注释」的实现（例如要同时剥 doc 注释、或要剥掉某种字符串）**要改 8 处**；
⇒⇒ 而**其中任何一处漏改的后果是假绿/假红**，不是编译错误。
⇒⇒ ⭐ **而同一层里「共享」这条路已经被走过 3 次** ⇒⇒ **所以准确的判断是：
「它不是不能共享，是没有成为默认」** —— 这是一个**收窄成一句话**的事实。
⇒⇒ 相比之下 P25（「收敛若引入层间倒挂则重复更便宜」）在这里**不成立**：
**同层共享已被做出来 3 次，不存在「层间倒挂」这一免责理由。**

建议: 把 8 份本地定义换成从 `design_tokens_test.dart` 的 **一次 import**（**与那 3 处同款**），
并把这份 helper 归到一个与它职责相符的位置（例如 `test/support/strip_comments.dart`）
—— **顺带解决「11 个文件在用的东西住在 `design_tokens_test.dart` 里」这个落点问题**。
⇒⇒ 若担心 `test/` 之间互相 import 不可取：**本仓已有 3 个先例**（见上），故该顾虑已被本仓自己回答过。

验证: 替换后 `grep -rc "^String stripCommentsAndStrings" test/` 应为 1；`flutter test` 结果不变。

置信: 高（8 份的长度与 md5 一致，且 3 处 import 已逐字读出）
反证: 若这 8 份**已经**在某个未读的公共 helper 里被间接共享（例如本文件另有 import 链），
则「重复」的判断需重算 ⇒ **但 8 份**各自**定义了同名顶层函数、且逐字节相同**，
而同一目录下 Dart 不会因此报错（同库不同文件各自定义同名函数是允许的）⇒⇒ **故「重复」成立**

### F-0446-01 · P2
file: shell/flutter/lib/settings/sections/director_observer_section.dart
摘录:
> /// | D 送 TTS 文本 | 左 = 前端已收 text_delta；**右 = 日志端点 sentence_ready** |      // :21
> Text('**原始段（日志端点 sentence_ready）**', style: muted)                          // :634
…
> Future<List<String>> _defaultLoadLogLines() async {                                // :210
>   final List<LogLine> lines = await (_diagApi ??= DiagnosticsApi()).logs();
>   return **lines.map((LogLine l) => l.asText).toList()**;                            // :212
> }

影响: **「只含 `sentence_ready`」这个说法只存在于注释（`:21`）与那行小标题（`:634`）里；代码是全量映射、
零过滤。** ⇒⇒ 于是**右列显示的是全部日志行**（WS 帧、错误行、请求日志…），
而它的标题写的是「原始段（日志端点 `sentence_ready`）」⇒⇒ **⇒ 标题在替一个不存在的过滤背书**，
用户与后来的读代码者都会以为这一列只有 `sentence_ready`。
⇒⇒ 而**这一列是 dev_mode 下唯一的**「D 送 TTS 文本」**对照面**（B0433 核过这一块整块只在 devMode 下渲染）。

另外：**过滤只能做在客户端** —— B0321 已核过 `GET /api/v1/logs?limit=…` 的**查询参数服务端不读**
（`handle_logs()` 只按内置 `MAX_LINES` 截尾，参数被删掉而不是留着）⇒⇒
⇒⇒ **⇒ 所以「按 `sentence_ready` 过滤」这条路是存在的、只是没写** ⇒⇒⇒ **⇒⇒ ⇒⇒ 这也说明缺口是**实现**缺口、不是能力缺口**。

建议: 二选一，**都在同一处**：
- **按标题兑现**：`_defaultLoadLogLines` 里 `.where((l) => l.type == 'sentence_ready')`
  （**字段名以 `LogLine` 的实际定义为准**）；或
- **按代码改标题**：把 `:21` 与 `:634` 的「日志端点 sentence_ready」改成「服务端日志（全部行）」
⇒⇒ **且不管选哪个，建议把「这条日志列是不是全量」写进 `DiagnosticsApi.logs()` 的文档** ——
**那个方法刚被 B0321 改过（B0321 那条就是「参数不生效 ⇒ 删参数」）** ⇒⇒ ⇒ **两次改动落在同一个方法上，而它的文档没记这件事**。

验证: 改动后打开 D 栏：若选过滤，右列只出现 `sentence_ready` 行；若选改标题，标题与内容一致。
> 💡 **B0448 顺手可做（不单开条）**：`logs()` 的 `['lines'] ?? ['logs']` **第二个分支从未被走到**
> （全 `test/` **零命中**造出 `logs` 键的 payload）⇒⇒ **「服务端两种键名都认」这条承诺没有测试在守**
> ⇒⇒ **⇒ 判它是观察、不是发现的理由**：「用户可察觉的后果」**没有**（两种键名都工作时界面上无差别）；
> 「两条真相会分叉」**可能**，而分叉方向正是 `??` 要挡的；修法**一行**
> ⇒⇒⇒⭐⭐⭐ **⇒ 「一行成本」不足以把它抬到 P3** ⇒⇒ **⇒ 而它就在 F-0446-01 的同一段代码里**
> ⇒⇒⇒⭐⭐⭐ **⇒ 方法论**：**「一个分支没被测到」不是缺陷** —— 它是否缺陷取决于
> 「**走到它与走不到它，界面上有没有差别**」⇒⇒⇒⭐⭐ **「分支覆盖率」是指标、
> 而「两条路径是否等价」才是判据** ⇒⇒ **本仓的 `??` 兜底是等价的两条路、所以覆盖率低没有害**

「这条列是不是全量」应有至少一条断言（它现在是**一个只有注释主张的事实**）。

置信: 高（三处均逐字读出：表头 `:21`、小标题 `:634`、实现 `:210-213`）
反证: 若 `DiagnosticsApi.logs()` 返回的 `lines` **本身**就只含 `sentence_ready`
> ✅ **B0447 定论成立（软栗解除），而根比「没过滤」更靠下**
> `diagnostics_api.dart` `logs()`：取 `['lines'] ?? ['logs']` ⇒ **逐条 `fromJson`、零 `where`**；
> `LogLine`（`:58-70`）只有 **`level` / `target` / `message`** 三个字段 + `asText => '[$level] $target: $message'`
> ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ 「`LogLine` 没有 `type` 字段」比「`logs()` 没过滤」更根本**
> ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ 要按 `sentence_ready` 过滤，**必须先让 `LogLine` 带一个区分日志种类的字段****
> ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ 「这不是加一个 `where` 就完的事」**
> ⇒⇒ **（服务端的 `MAX_LINES` 是按行数截尾、不是按类型 ⇒ 那个方向也没有现成字段可用）**
> ⇒⇒ ⇒ **⇒ 本条的**建议改写**：
> - **首选（零协议成本）**：把 `:21` 与 `:634` 的「日志端点 `sentence_ready`」改成它实际显示的东西（**服务端日志（全部行）**）
> - 若确实要按 `sentence_ready` 过滤 ⇒ ⇒ **两步**：服务端在行里加一个种类字段 + 客户端 `LogLine` 带上它 + `where`
> ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒「一行文案」是**唯一不需要动协议**的落点**

（即过滤发生在服务端或 `logs()` 内部）⇒⇒ **则本条不成立** ⇒⇒ **而我未读 `DiagnosticsApi.logs()` 与
`LogLine` 的定义** ⇒⇒ **⇒ 这是本条唯一的软肋，且它是可一读定论的** ⇒⇒ **按 B0271：强度不足时先记待核**

### F-0469-01 · P3
file: crates/live2d-ai-mod-local-llm/Cargo.toml
摘录:
> description = "Live2D-Ai 本地大模型进程管理 Mod（节点 E E5，P1-1）：spawn            // Cargo.toml:3

而同一 crate 的 `lib.rs:3` 有：
> //! # ⚠️ 已废除启动（0.2.0-rc.1，2026-09-14）— **DEPRECATED，勿在新代码引用**

调用链: `cargo metadata` / `cargo search` / IDE 依赖树 / 任何按包名或 `description`
选型的人 **读的是 `Cargo.toml`** ⇒⇒ **「已废除」这三个字不在他们读的那一行里**。
⇒⇒ 「不许挂回」这条禁令的**判据是运行时**的（`main.rs::mod_count_is_three` 断言工厂数，
`AVAILABLE_MOD_FACTORIES` 已把它移出）—— 那条**已经守住**（B0468 核过：活代码零残留）。

影响: **两个读者看到的东西不一样**：
- 读 `lib.rs` 的人：**第一屏第二行**就是「⚠️ 已废除启动 · DEPRECATED」⇒ 看得到。
- 读 `Cargo.toml` / 工具输出的人：**只看到「本地大模型进程管理 Mod」** ⇒ **看不到它已废弃**。
⇒⇒ 而后者恰好是最可能**做错选型**的那类读者（「我想加一个本地 LLM Mod ⇒ 这个 crate 现成」）。
⇒⇒ 按 B0448 的判据（「走到它与走不到它，界面/输出上有没有差别」）**两条路径不等价** ⇒⇒ 该记一条。

判据的可迁移形状（**本条的价值主要在这句**）：
> **「一个 crate 的弃用信息放在哪」的判据是「谁会先看到它」。**
> `Cargo.toml` 的 `description` 是**工具读**的（`cargo search` / 依赖树 / IDE 补全）；
> `lib.rs:3` 的 `//!` 是**人读源码**时看到的。⇒⇒ **只放后者 ⇒ 对前者不可见。**

建议: **一行** —— 在 `Cargo.toml` 的 `description` 末尾补一句
`（0.2.0-rc.1 已废除启动 · DEPRECATED）`。
⇒⇒ 若担心重复维护，可把这一句与 `lib.rs:3` 的措辞**逐字一致**（同 B0386「一条纪律在三处被独立说出」的取舍），
因为**两处服务的读者不同**，所以**重复是对的而不是冗余**。

验证: 改动后 `cargo metadata` 输出里该 crate 的 description 含「已废除」；
`grep -c DEPRECATED crates/live2d-ai-mod-local-llm/Cargo.toml` 由 0 变 1。

置信: 高（两行都逐字读出；`Cargo.toml` 里 `deprecat` 零命中已机械确认）
反证: 若本仓有别的机制保证「按 `description` 选 Mod」这件事不会发生
（例如 `AGENTS.md` 与 `docs/architecture/mod-product-chain.md` 是唯一的选型入口且都写明「禁止挂回」）
⇒⇒ **则本条的用户可察觉后果显著降低、而「判据 = 谁会先看到它」这句仍然成立** ⇒⇒
**⇒⇒ ⇒⇒ 换句话说：即使这条不记为缺陷，它提炼出的判据也不依赖这条**

### F-0595-01 · P2
file: shared/mcp_tools.json
摘录:
> AGENTS.md：「**LLM 工具层与动作系统已整体拆除**——**不要**再以「LLM 调用工具」「动作系统」为前提写代码或文档。」
> AGENTS.md：「`docs/architecture/mod-product-chain.md`（**与 `plugin-sdk.md` 冲突时以它为准**）」

调用链: `grep -rln "mcp_tools" --include=*.rs --include=*.dart --include=*.py --include=*.yml --include=*.md .`
⇒ 命中**只有 `.md`**：`CHANGELOG.md` · `docs/architecture/Phase-0-architecture.md` ·
`docs/architecture/plugin-sdk.md`（**AGENTS 明说已被 `mod-product-chain.md` 取代**）·
`docs/architecture/observability.md` · `docs/architecture/core-contracts.md`。
**`crates/**`、`shell/flutter/**`、`tests/**`、`scripts/**`、`.github/**` 零命中。**

影响: 一个**描述已被整体拆除的工具层**的 JSON 文件留在 `shared/`，而它的**唯一引用者全是文档**、
其中一份（`plugin-sdk.md`）**已被 AGENTS 宣告失效**。
⇒⇒ 于是**读代码的人**（按 `plugin-sdk.md` 走）会找到一份**看起来是当前契约**、实际描述**不存在的东西**的 JSON；
⇒⇒ 而**读那份 JSON 的人**没有任何机制知道它已经不适用了
⇒⇒ 「**文档与实现同源**」这条纪律（AGENTS「**契约来源（读源码确认，不是猜的）**」，B0454 核的 settings_models 头注）
在这一个文件上**不成立**。

建议: 二选一，**各一处**：
- **随实现一起删**（`shared/mcp_tools.json`），并让那五处 md 里的引用失效或改指 `mod-product-chain.md`；
  或
- **留在原地但自述失效**：在 JSON 里加一个 `"_status": "DEPRECATED — 工具层已于 0.2.0-rc.2 整体拆除，见 AGENTS.md"`，
  并把 `plugin-sdk.md` 顶上加一行「**本文件已被 mod-product-chain.md 取代**」（AGENTS 已这么写、**文档里没写**）。

验证: 删/改后 `grep -rln mcp_tools --include=*.md .` 命中的每一处都指向现行文档；
`python3 -m pytest tests/ -q` 仍全绿（本条不涉及运行时）。

置信: 高（两侧证据都逐字读出：AGENTS 的两句 + 全仓检索只命中 md）
反证: 若 `plugin-sdk.md` 仍被某个**未审过的** `docs/plans/**` 当作现行契约引用
⇒⇒ 则「读者会被误导」的范围比本条写的更大（**未核**：`docs/plans/**` 我只读了两份计划）
⇒⇒ **但那不影响本条的建议**（两份文档都要处置）
### F-0606-01 · P3
file: scripts/verify_edge_alpha.py
摘录:
> """M1-A3 边缘像素断言脚本 —— **contract-R1-render.md §3 M1-A3**（联合验证批次）
> - 判据：**平均 RGB 残留 < 8/255**（**非预乘白边特征 = 残留 > 40/255**，修复后…）
> - **输入截图背景须为黑/透明**（契约 §3 M1-A3 / §4 M1-A3 前置）
> - 输出 JSON + 控制台；退出码：判据失败非 0（可被 pytest/CI/批次4 联动）"""

```
git ls-files | grep -i contract
  ⇒ docs/architecture/core-contracts.md
  ⇒ docs/plans/node-d-api-contract-2026-08-28.md      # 没有 contract-R1-render.md
find . -name "*contract*R1*" -not -path "./target/*"   ⇒ 零命中
ls -d verify                                           ⇒ No such file or directory
git ls-files verify/ | wc -l                          ⇒ 0
grep -n "^verify" .gitignore                          ⇒ 零命中（不是被忽略的）
```

调用链: `scripts/verify_edge_alpha.py`（**只读 14 行头注**）⇒ 输入 `verify/render_baseline_before.png`
与 `verify/render_after.png` ⇒ **该目录在工作区与 git 里都不存在**；
出处 `contract-R1-render.md` ⇒ **git 与工作区都没有**（脚本里出现三处：`:2` / `:131` / `:144` 的输出 JSON 字段）。
⇒ **而它不进 CI**：AGENTS 的 `root-py-tests` 跑 `python3 -m pytest tests/ -q`，本文件在 `scripts/`。

影响: **一个可执行判据的三个要素，只有一个在仓内。**
- **判据**在仓内（`< 8/255`）⇒ 读它的人知道**判据是什么**；
- **出处**不在仓内 ⇒ **读不到「为什么是这个数」**（8 与 40 从哪来、8…40 灰区算什么）⇒ 与 B0604 记录的「8…40 之间没有说明」是**同一个洞的两面**：B0604 看到的是「头注没写中间那段」，本条看到的是「**那段也许写在仓外那份文档里**」；
- **输入**不在仓内 ⇒ **无法复现**（要跑它得先造出那两张基准图）。
⇒⇒ **而它不进 CI 是根因、不是「测试没写」** ⇒⇒ 与 B0243「不是只有 2 轮就自己动手」同族：**先看它有没有被谁调用**。
⇒⇒ 用户可察觉后果：**零**（不进产品路径、不进 CI）⇒ **故 P3 而非 P2**；
⇒⇒ **与 F-0595-01 不合并**（按「同根因才合并」）：那一条是「**文件在、语义没了**」，这一条是「**脚本在、输入与出处没了**」。

建议: **一处**，二选一：
- **把 `contract-R1-render.md` 与两张基准图入库** —— 它们是**判据的证据**，而 B0590 已核过
  `verification/rust-bakeoff/` 的三个对比 PNG **确实入库了**，⇒ **本仓已有「证据入库」这个先例**；或
- **让头注自足**：把 8 与 40 的来源、8…40 灰区的处理、背景前置为什么必要，**写进脚本头注**（一行一句即可），
  并注明「**输入需另行准备（不在仓内）**」。

验证: 入库（或补头注）后，`git ls-files | grep contract-R1` 命中、或头注不再引用仓外文件；
本条不涉及运行时，`python3 -m pytest tests/ -q` 结果不变。

置信: 高（六条检索全部逐条列出并给出输出：git 两条、find 一条、ls 一条、git ls-files 一条、gitignore 一条）
反证: 若那两张图与那份文档**在别的分支或本机仓外路径**（AGENTS 记「公开历史重算、只保留维护者本地的归档仓库」），
则本条的**「无法复现」**只在**本仓这一份 clone** 上成立 ⇒⇒ **而 AGENTS 明写 `Live2D-Ai-Android` / `Live2D-Ai-pc` 只在维护者本地保留**，
⇒⇒ **若这两张图属于那两个已归档仓，则本条应改写为「脚本引用了归档仓的产物」** ⇒⇒ **按 B0271 记为待核、不改结论**
### F-0616-01 · P1
file: scripts/deploy_android.sh:5
摘录:
> # 环境:
> #   手机: 192.168.0.x:5555 (无线调试)
> #   **锁屏密码: <REDACTED-DEVICE-PIN-2026-10-06>**
> #   uiauto控制器: F:\uiauto\uiautodev-desktop.exe

调用链: `git ls-files --error-unmatch scripts/deploy_android.sh` ⇒ **入库**（在工作区与 git 里）⇒⇒
AGENTS.md：「**2026-09-11 起这两个归档的远端 ref 已删除，只在维护者本地保留**——**公开历史重新起算**
（`main` 成为单个根提交）**⇒⇒ **⇒ 该文件在「重新起算」后的公开历史里**。

**而本仓有一个专门扫公开密钥的脚本，它会看这个文件，却没有被跑：**
- `scripts/check_public_secrets.py:32-36` `SKIP_SUFFIXES` = `.png/.jpg/.jpeg/.gif/.webp/.ico/.wav/.mp3/.ogg/
  .zip/.tar/.gz/.bz2/.xz/.7z/.lock/.ttf/.otf/.woff/.woff2/.mp4/.mov/.pdf` ⇒⇒ **⇒ 不含 `.sh`** ⇒⇒
  **⇒ 扫描器结构上能看到 `deploy_android.sh`**
- `grep -rn "check_public_secrets" .github/` ⇒ **零命中** ⇒⇒ **⇒ 它不在任何 CI workflow 里**
- `grep -c "check_public_secrets" AGENTS.md` ⇒ **0** ⇒⇒ **⇒ 它也不在「本地必跑 vs CI 必跑」那张表里**

影响: 三个缺口**互相加固**，而**每一个单独看都不显眼**：
1. **凭证在公开历史里**：设备 IP（`192.168.0.x:5555`）与**锁屏 PIN**同时在文件头注里。
   它是**个人设备的锁屏密码**、不是产品密钥 ⇒⇒ **⇒ 但「公开历史重新起算」这条裁决的直接后果就是
   「仓里的一切都是公开的」** ⇒⇒ **⇒ 而这条裁决是在**同一份 AGENTS.md** 里写的**。
2. **扫描器看不到它吗？** —— **能看**（`.sh` 不在跳过表里）⇒⇒ **⇒ 所以这不是「工具的盲区」**
3. **扫描器跑它吗？** —— **不跑**（`.github/` 零命中、AGENTS 门禁表零命中）⇒⇒
   **⇒ 而这与 AGENTS 自己写的「**一张表，一套真相**：本地提交前跑的命令，必须能在 CI 清单里逐条找到」
   是同一份文档里的两条相邻规定** ⇒⇒ **⇒ 现状是「不在表里、也不在 CI 里」——不是「在表里但漏了 CI」**
⇒⇒ **⇒ 净结论：不是扫描器不行（那是 F-0049-01 的事），是**扫描器没被调用****

建议: **三处，各一行/一处**：
- **把 `check_public_secrets` 接进 `pr-checks.yml`**（AGENTS 那条「一张表，一套真相」要求的形状：
  在门禁表加一行「仓库公开密钥扫描 | `python3 scripts/check_public_secrets.py` | `pr-checks.yml` → `public-secrets`」）；
- **把 `锁屏密码: <REDACTED-DEVICE-PIN-2026-10-06>` 从头注删掉**（改写成「锁屏密码见本机 `~/.config/l2d-ai/` 下的机读文件」或
  「由 `adb shell input text` 交互输入，不落盘」）⇒⇒ 与 AGENTS「**密钥真源 = `.env`**、**密钥不进 GET/日志/WS/导出**」
  **同一条纪律的设备侧版本**；
- **把 `192.168.0.x:5555` 改成占位/参数**（脚本头注写「手机: `<serial>`（无线调试，默认按 `ANDROID_SERIAL` 读）」）。

验证: ① `grep -rn "check_public_secrets" .github/` **有命中**；② `grep -c "锁屏密码" scripts/deploy_android.sh` = 0；
③ `python3 scripts/check_public_secrets.py` 退出码 0（**本审计不运行它**：它会读 `.env`）。
注：② 之后**须重跑一次扫描器**确认无其它命中（本条只核了这一个文件）。

置信: 高（**六条检索全部逐条列出并给出输出**：`git ls-files --error-unmatch` 入库 ·
`SKIP_SUFFIXES` 32-36 · `path.suffix.lower() in SKIP_SUFFIXES` 87 · `.github/` 零命中 · `AGENTS.md` 零命中 ·
头注三行逐字读出）
反证: 若该锁屏 PIN **已在上游重算历史之前就存在、并随 `main` 成了根提交** ⇒⇒ **它就是「公开历史重新起算」本身的后果**
⇒⇒ **那本条的建议②③仍成立**（**头注里本来就不该有一个 PIN**），而建议①（接进 CI）**更加必要**
⇒⇒ **⇒ 两种情况都不推翻本条**；差别只在「是遗留」还是「是新引入」⇒⇒ **⇒「是否已轮换该 PIN」本条无法核**
（需要访问那台设备，**超出只读范围**）⇒⇒ **记为待核，不改结论**
### F-0637-01 【已撤回（B0598）】· P1
file: Cargo.toml:29 · crates/live2d-ai-desktop/Cargo.toml:69 · crates/live2d-ai-mod-director/src/lib.rs
摘录:
> AGENTS.md：「**`live2d-ai-mod-director`（动作序列的唯一驱动方）** | **已删除** | 归档在 `archive/action-layer-p6`」
> AGENTS.md：「**`director` Mod 已于 `0.1.0-rc.2` 删除**」

```
git ls-files crates/live2d-ai-mod-director/
  => crates/live2d-ai-mod-director/Cargo.toml
  => crates/live2d-ai-mod-director/src/arbiter.rs
  => crates/live2d-ai-mod-director/src/decision.rs
  => crates/live2d-ai-mod-director/src/ledger.rs
  => crates/live2d-ai-mod-director/src/lib.rs

grep -n "mod-director" Cargo.toml crates/live2d-ai-desktop/Cargo.toml
  => Cargo.toml:29:    "crates/live2d-ai-mod-director",          # workspace members
  => crates/live2d-ai-desktop/Cargo.toml:69:live2d-ai-mod-director = { path = ".." }
```

调用链: `Cargo.toml:29`（**workspace members**）=> `crates/live2d-ai-desktop/Cargo.toml:69`
（**desktop 直接依赖它**）=> `crates/live2d-ai-mod-director/src/{lib,arbiter,decision,ledger}.rs`。

影响: **AGENTS 的休眠台账与仓库实际状态不符，而且差的是最要紧的一格。**
- 那张「动作与表演的归属」表里 `live2d-ai-mod-director` 一行写「**已删除**」，
  而它在 **workspace members 里**、**被 desktop 直接依赖**、**5 个源文件都在 git 里**
  => **「能不能被唤醒」那一列因此答错了**。
- 而 AGENTS 自己给的两条护栏——`main.rs::mod_count_is_three`（工厂数不得因动作 Mod 增加）
  与 `mod_registry::tests::action_request_is_dormant_not_delivered`（动作请求必须不被接受）——
  **守的是「注册表里不许有它」，不是「工作区里不许有它」**
  => **=> 二者之间恰好留着一条缝** => **=> 这条缝就是本条**。
- 与 B0468 核的「注释指向死代码」同族，而**这次指向的是一整个 crate**。

建议: **二选一，各一处**：
- **真删**：从 `Cargo.toml:29` 的 members 去掉、从 `live2d-ai-desktop/Cargo.toml:69` 去掉依赖、
  删 `crates/live2d-ai-mod-director/`（归档到 `archive/action-layer-p6`，与 AGENTS 的说法对齐）——
  **本审计不执行任何 git 写操作，此项仅记录**；
- **或改台账**：把 AGENTS 那一行改成「**已停用、crate 仍在工作区**（`Cargo.toml:29` 是 members、
  `live2d-ai-desktop` 直接依赖；只是**未注册进 `AVAILABLE_MOD_FACTORIES`**）」，
  并在 `crates/live2d-ai-mod-director/src/lib.rs` 头注写明「**本 crate 不参与启动、禁止挂回注册表**」
  （与 `live2d-ai-mod-local-llm` 的 DEPRECATED 标注**同一手法**）。

验证: ① `grep -rn "mod-director" Cargo.toml crates/*/Cargo.toml` **无命中**（真删路线）
或 **有命中且注释与实际一致**（改台账路线）；② `python3 -m pytest tests/ -q` 不受影响；
③ `main.rs::mod_count_is_three` 仍绿（**与本条无关**——**它守的是注册表、不是工作区**，
**这正是本条说的那条缝**）。

置信: 高（**五条检索全部逐条列出并给出输出**：`git ls-files` 五行 · 两次 `grep -n` 带行号 · AGENTS 两句原文）
反证: 若 `crates/live2d-ai-mod-director/` 是**从 `members` 之外**被构建的（例如只被某个
未审的脚本直接 `cargo build -p`）=>⇒ **那它就不进 CI 编译图** ⇒⇒ **「在编译图里」这句话要收窄**
⇒⇒ **⇒ 但「AGENTS 写已删除、而目录在 git 里」这一半仍然成立** ⇒⇒ **⇒⇒⇒⇒**
⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒ Eqs.
### F-0644-01 · P1
file: crates/live2d-ai-desktop/src/mod_registry.rs:188-192 · :688-694
摘录:
> ```rust
> // :188
> for factory in factories {
>     let desc = factory.descriptor();
>     let (enabled, config) = parse_mod_config(manifest, desc.id);
>     let mut reg = RegisteredMod::new(*factory);
>     reg.enabled = enabled;
>
> // :688
> fn parse_mod_config(manifest: &serde_json::Value, id: &'static str) -> (bool, serde_json::Value) {
>     let Some(obj) = manifest.get("mods").and_then(|m| m.as_object()).and_then(|m| m.get(id)) else {
>         return (false, serde_json::Value::Object(Default::default()));
>     };
>     let enabled = obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
>     let config  = obj.get("config").cloned().unwrap_or_else(|| serde_json::Value::Object(..));
>     (enabled, config)
> }
> ```

```
grep -rn "unknown|未知|不认识的|多余" crates/live2d-ai-desktop/src/mod_registry.rs
  => 零命中
```

调用链: `cli_entry.rs:613 mods_manifest_for_web()`（先读 `mods.json`、读不到回落 `default_mods_manifest()`）
⇒ `cli_entry.rs:182 / :296 ModRegistry::new(AVAILABLE_MOD_FACTORIES, &manifest)`
⇒ `mod_registry.rs:188 for factory in factories` ⇒ **`parse_mod_config(manifest, desc.id)`**。

影响: **`mods.json` 里写一个当前不存在的 Mod id，它被静默丢掉。**
- 用户视角：改了 `mods.json`、**没有任何反应、也没有任何提示** ⇒ 而 `mods.json` 是**用户可手改**的文件
  （AGENTS：`mods.json` 启停/配置**原子写回**、`GET /api/v1/mods` 带 `settings_spec`）⇒ **⇒⇒⇒⇒**
</content>
### F-0644-01 补充（B0646 追加的另一半证据 · 使本条升为 P1）
file: crates/live2d-ai-desktop/src/mod_registry.rs:255-259
摘录:
> ```rust
> (*id).to_string(),
> serde_json::json!({ "enabled": e.enabled, "config": e.config }),
> ...
> let doc = serde_json::json!({ "mods": mods });   // mods 是**新建的 map**
> let text = match serde_json::to_string_pretty(&doc) { ... };
> ...
> match live2d_ai_runtime::settings::patch::plan_atomic_write(path, &text) { … fs::rename … }
> ```

影响: **B0646 核到的那一半把本条从 P2 升为 P1。**
- 读侧：`:188 for factory in factories` ⇒ **未知 id 没有任何去处**（B0644 已核）
- 写侧：`:255-259` 的 `doc` **从工厂重建、不读原文件** ⇒ **下一次写回时，用户手写的未知键被抹掉**
- 写回是 `plan_atomic_write` + `rename`（AGENTS 的「**原子写回**」）⇒ **这不是「没写成功」，
  是「成功地把别人的东西改掉了」**
- ⇒ **⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒ Eqs.
### F-0644-01 补充二（B0564 追加：顶层键也丢 · 证据链闭合）
file: crates/live2d-ai-desktop/src/mod_registry.rs:248-259
摘录:
> ```rust
> fn persist_manifest(&self) {
>     let Some(path) = self.manifest_path.as_deref() else { return; };
>     let mut mods = serde_json::Map::new();
>     for (id, e) in &self.entries {
>         mods.insert((*id).to_string(),
>             serde_json::json!({ "enabled": e.enabled, "config": e.config }));
>     }
>     let doc = serde_json::json!({ "mods": mods });
> ```

影响: **证据链闭合，两段都在。**
- 读侧（`:188`）只遍历工厂 ⇒ **未知 id 没有任何去处**（B0644）
- 写侧（`:252` `Map::new()` + `:259` 只写 `"mods"` 一个顶层键）
  ⇒ **未知 id 被抹掉**，**且 `mods` 以外的顶层键也一并消失**
- ⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
  ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒ Eqs.
  ⇒ **判据 = 「写回时那个键还在吗」—— 答案是不在。** 与 B0488「head-file 白名单扫描」
  **同一判断方式**：去看那条数据会不会被写回去。

建议: **二选一，各一处**：① `persist_manifest` 改成**读-改-写**（先把原文件读成
`serde_json::Value`、只替换 `mods` 里**本 registry 拥有的那几项**、其余原样保留）；
② 或**明确声明 `mods.json` 是「服务端拥有的文件」**（在 `persist_manifest` 头注与
`docs/architecture/mod-product-chain.md` 各写一句「手改的未知键不保留」）。
本审计**不执行任何写操作**，此项仅记录。

验证: 手写一个未知键 ⇒ 改任一 Mod 的 `enabled` ⇒ **①方案** 下 `grep <未知键> mods.json` 仍有；
**②方案** 下该键消失**且文档已写明**。两者都算通过。

置信: 高（`:248-259` 九行原文 + `:188` 读侧 + `mod_count_is_five` 三处交叉）
反证: 若 `mods.json` 从设计上就**由服务端独占**、用户只经 UI 改 ⇒ 则「手写未知键」
本就不是受支持的行为 ⇒ 上面「影响」要收窄为「文档未声明这一点」⇒ **本条仍成立，只是
严重度可降为 P2** ⇒⇒ **判据 = 文档有没有写「这个文件归服务端」** ⇒⇒ **未核**
（**下一次要核的就是这一句**）。
### F-0637-01 【已撤回，见文件末尾「撤回（B0598）」块】补充四（B0565 追加：第 5 处反证 —— 数据协议里仍留着 director 的字段用例）
file: crates/live2d-ai-mod-system/src/settings.rs:43-46
摘录:
>     /// **缺省值**（可空）：config 里没有这个键时表单回填它，
>     /// 保存时按它写入。用于「服务端缺省不是第一个选项」的场景
>     /// ——**导演的动作预设**正是这种（缺省是 smile，不是 none）。

影响: **AGENTS 那句「director Mod 已于 0.1.0-rc.2 删除」与五处实际状态全部不符。**
| # | 位置 | 实际状态 |
| --- | --- | --- |
| 1 | Cargo.toml:29 | 在 workspace members 里 |
| 2 | live2d-ai-desktop/Cargo.toml:69 | desktop 直接依赖 |
| 3 | crates/live2d-ai-mod-director/src/ | 5 个源文件在 git 里 |
| 4 | desktop/src/main.rs:95 | 注册表 AVAILABLE_MOD_FACTORIES 第 5 位 |
| 5 | **mod-system/src/settings.rs:45** | **数据协议的头注拿它当活例** |
第 5 处与前四处性质不同：前四处是「代码还在」，这一处是**文档层面的依赖**——
如果按 AGENTS 执行「删除 director」，`Select.default` 的头注就变成一个指向不存在东西的例子。
⇒ 反过来说：这一处证明 **director 不只是没删干净，而是被当作现存能力写进了公共数据协议**。

建议: **二选一，各一处。**
① 若决定删：删 workspace members + desktop 依赖 + src/ 五文件 + 注册表那一行 +
   mod_count_is_five 改回 mod_count_is_four，并**把 settings.rs:45 的例子换成别的**
   （例如 persona 的 V1/V2 字段），否则头注悬空。
② 若决定留：在 AGENTS 的「动作与表演的归属」表里把 director 那行的状态从
   「**已删除**」改成「**在册、零投递**」，并写明「crate + 注册项 + 数据协议用例三处都还在」。
本审计**不执行任何写操作**，此项仅记录。

验证: `grep -rn "live2d-ai-mod-director" Cargo.toml crates/` 若为空 = 方案 ① 已做完；
再 `grep -n "导演" crates/live2d-ai-mod-system/src/settings.rs` 应为 0 命中。
方案 ② 则检查 AGENTS 该行是否已改写。

置信: 高（settings.rs:43-46 逐字 + 前四处 B0637/B0640 已核）
反证: 若「导演的动作预设」其实是**另一个尚未实现的设计**（不是 director Mod 的产物），
则本条**不成立**。判据 = 该字段用例在 planner / 计划文档里是否被 director 引用。
⇒⇒ **未核**，下一批核这一句。
### F-0637-01 【已撤回】补充五（B0583 追加：第 6 处反证 —— 连「怎么把它加进去」的操作手册都在，且已全部勾完）
file: docs/plans/parallel-mods/REGISTER-director-v0.md:1-9,34-44
摘录:
> ```
> # REGISTER — `director`（Wave 3 G 轨 → 集成收束用）
>
> > 本文件**不是**注册动作本身，而是交给**主 agent** 的**待办清单 + …**
> > Wave 3 worker **被禁止**碰 `main.rs::AVAILABLE_MOD_FACTORIES` / `mod_count_*` /
> > `mod_factory_ids_match_expected` / `cli_entry::default_mods_manifest` / 全局版本…
> > / `docs/README.md` / `AGENTS.md` / `mod-product-chain.md` 的 Mod 清单表
> > —— **这些全部留在这里**。
> ```
>
> ```
> :34 - [x] crates/live2d-ai-desktop/Cargo.toml 追加 path 依赖
> :37 - [x] crates/live2d-ai-desktop/src/main.rs 的 AVAILABLE_MOD_FACTORIES 追加
> :40 - [x] 数量断言 `mod_count_is_six`（main.rs:454）→ 改名 `mod_count_is_seven`
> :42 - [x] mod_factory_ids_match_expected（main.rs:463）的 expected 追加
> :44 - [x] **不改缺省 manifest**：director **缺省停用**，不进 …
> ```

影响: **前 5 处反证是「代码还在」，第 6 处是「连操作手册都在」。**
- 仓库里有一份**逐条列出「怎么把 director 加进注册表」**的清单，
  且 5 条**全部已勾**；本批逐一对照代码现状（依赖在 / 注册表第 5 位在 /
  `mod_factory_ids_match_expected` 含 director / 缺省 manifest 不含 director）**全部相符**。
- 另有两份**现存**的上位契约：`docs/architecture/director-mod-v0.md`、
  `docs/architecture/director-rfc.md`（本批已 `ls` 确认存在）。
- ⇒ **⇒⇒⇒⇒** 这不是「删漏了」，是**有人在册、有契约、有勾选记录**；
  而 AGENTS 那一行写的是「**已于 0.1.0-rc.2 删除**」。

建议: 与补充四同（**二选一，各一处**）。此处补一条**判据**供维护者自查：
**「某 Mod 已删除」这句话，应该能对上四样东西**：① workspace members 无它、
② 无 crate 依赖、③ `AVAILABLE_MOD_FACTORIES` 无它、④ **无 `REGISTER-*` 清单 / 无上位契约文档**。
本批实测 ①②③④**全部命中**，故该行不成立。

验证: `ls docs/plans/parallel-mods/REGISTER-director-v0.md`
与 `ls docs/architecture/director-*` —— 若删除 director，这两处应一并消失；
现在它们都在，且清单已勾完。

置信: 高（清单逐行 + 代码现状逐项对照 + 两份契约文件存在性已验）

反证: 若该 REGISTER 文件属于**已废弃的并行批次产物**、且 `director-rfc.md` /
`director-mod-v0.md` 也已标注作废，则本条**降为「历史件未清理」的 P3**。
判据 = 那两份契约文档的头部有没有「已废弃 / superseded / 勿当现网」字样。
⇒ **未核，下一批核这一句。**

### F-0637-01 【撤回（B0598）】· 原指控「AGENTS 说 director 已删除、实际六处证据在册」不再成立
file: AGENTS.md:302-305
摘录:
> | `live2d-ai-mod-director`（动作序列的**唯一驱动方**） | **已删除** | 归档在 `archive/…`
> | `live2d-ai-mod-director`（2026-09-14 **重加**的**决策/按句 cue**版） | **已接线**（不再休眠）：规则层产 WS `action_cue`（**唯一驱动舞台**…
影响: **撤回理由是文档已改，不是证据不足。**
- AGENTS.md:302 那行「已删除」被**括号限定**为「动作序列的唯一驱动方」那一版；
- **紧接的 :304 就是现役那一版**，状态「**已接线**（不再休眠）」；
- ⇒ 本条原指控的六处证据（`Cargo.toml:29` / `desktop/Cargo.toml:69` / 5 个源文件 /
  `main.rs:95` / `settings.rs:45` / `REGISTER-director-v0.md` + 两份契约）
  **全部是「现役那一版」的证据，现已被文档逐条覆盖**。
- ⇒ **矛盾来自把 :302 读成了「唯一那行」**（本审计的读法错误，B0440 家族）。
建议: 无需改动代码。**若要进一步降低歧义**（P3 级，非必须）：
  给 :302 行的限定语加一个版本号（如「rc.2 那一版」），使两行**不靠括号称谓区分**。
验证: `sed -n '302,304p' AGENTS.md` —— 应看到两行并存、各带限定语。
置信: 高（整节逐行读过；B0597 先行发现前提已变、B0598 给出决定）
反证: 若 AGENTS 在别的位置仍有一句**不带限定语**的「director 已删除」，
则本撤回不成立。**⇒ **已核（B0599）**：AGENTS.md 共 27 处 `director` 命中，**全部读过**；
**唯二两处「已删除」都被限定**（`:166`「rc.2 那个『动作序列唯一驱动方』的 director」
与 `:303` 台账同格限定语），另有一处 `:171` 明写「**同名不同职责**」。
⇒ **反证不成立，撤回维持。**
