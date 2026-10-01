> 历史（2026-10-01 归档，勿当现网）。

# 可复制实现提示词（P0–P6）

用法：一个 worker 一块，按波次顺序整块粘贴（每块已含公共前置）。
波次：0 = P0（用户在场）；1 = P1 / P3 / P4（可并行，文件互不重叠）；2 = P2；3 = P5；4 = P6。
同一波次不要在同一棵工作树上并发；文件归属见每块末尾。

验收底线（全部完成后逐条核）：
1. GET /api/v1/env 列出的键名 ⊆ api_key_env 实际声明，且 PUT 只接受这份名单。
2. GET /api/v1/settings 与 GET /api/v1/app/status 对同一配置给出同一个 has_api_key。
3. 密钥读取只有 secrets::lookup 一条路径（grep -rn "std::env::var" crates/ 复核，出现即报告）。

================================================================
===== 复制给 Worker P0 =====
================================================================
【工作区】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，不同分支）。
【开工前读】AGENTS.md 的「核心层（Rust）」「前端层（Flutter）」「密钥真源 = .env」三节；以及你要改的每个文件的模块头注（头注解释了为什么不那么改）。
【硬约束】源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；密钥明文不进 GET / 日志 / WS / 导出；live2d-ai.toml 只持 api_key_env（变量名）；读密钥只能走 live2d_ai_runtime::secrets::lookup，禁止 std::env::var；写回 TOML / .env 必须保注释（merge_into_toml、plan_atomic_write 原子写）；不做没被点名的重构，不确定就停下写清哪里不确定。
【门禁·Rust 改动必跑】cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
【门禁·Flutter 改动】cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test ≠ 产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
【回报】改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」代替门禁输出。

【任务 P0：LLM 切到 OpenAI 兼容官方直连，不做本地中转】
背景：live2d-ai.toml 的 [llm].base_url 现在指向 http://127.0.0.1:18181/v1（本地中转，已不在）。本任务改成直连，且配置面只保留 OpenAI 接入字段（base_url / model / api_key_env / max_tokens / show_reasoning），不引入任何网关/中转专有字段。
必做：
1. live2d-ai.toml 的 [llm] 就地改值（保留原有注释与排版）：
   base_url = "https://api.deepseek.com"
   model = "deepseek-flash"
   api_key_env = "DEEPSEEK_API_KEY"
   max_tokens = 0
   说明：base_url 不写 /v1 也可（本项目拼 <base_url>/chat/completions，与官方 curl 一致）；
   max_tokens = 0 = 省略请求体字段 = 用官方默认（非思考 8K / 思考 64K）。旧的 4096 偏小，
   推理模型的思考会吃同一份预算，可能把正文挤成半句。
2. 不新增任何中转/网关字段。
3. mods.json 里 memory 的 summary 也指着 18181，二选一：
   a) 关掉：summary_enabled = false（推荐，记忆摘要是可选能力，先别加第二条 LLM 调用）；
   b) 直连：summary_base_url = "https://api.deepseek.com"、summary_model = "deepseek-flash"、
      summary_api_key_env = "DEEPSEEK_API_KEY"。
4. .env 的 DEEPSEEK_API_KEY 换成官方 key（值不进日志/不回显；写入会自动收紧 0600）。
5. 验证（必须读 body，不能看 HTTP 状态）：POST /api/v1/settings/test/llm 无论成败都回 HTTP 200，
   结论在 body 的 ok 字段；期望 {"ok":true}。再 POST /api/v1/chat 真发一条「下午好」，
   确认后端日志没有 llm_upstream_* / 没有 connection refused，前端上屏 + 出声。
禁止：把 key 写进 live2d-ai.toml；本任务不改 Rust/Flutter 代码。
产出：配置改动 + 两次验证的原始输出（body）。

================================================================
===== 复制给 Worker P1 =====
================================================================
【工作区】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，不同分支）。
【开工前读】AGENTS.md 的「核心层（Rust）」「前端层（Flutter）」「密钥真源 = .env」三节；以及你要改的每个文件的模块头注（头注解释了为什么不那么改）。
【硬约束】源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；密钥明文不进 GET / 日志 / WS / 导出；live2d-ai.toml 只持 api_key_env（变量名）；读密钥只能走 live2d_ai_runtime::secrets::lookup，禁止 std::env::var；写回 TOML / .env 必须保注释（merge_into_toml、plan_atomic_write 原子写）；不做没被点名的重构，不确定就停下写清哪里不确定。
【门禁·Rust 改动必跑】cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
【门禁·Flutter 改动】cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test ≠ 产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
【回报】改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」代替门禁输出。

【任务 P1：/api/v1/env 的密钥键名单一真源（不再硬编码 llm+tts）】
问题：web_api/env_routes.rs 的 handle_get / handle_put 各自把 llm、tts 抄了一遍，导致
[performance].api_key_env 既列不出也写不进（PUT 回 400 unknown_key）。
必做：
1. crates/live2d-ai-runtime/src/settings.rs 新增：
   pub struct DeclaredKeyEnv { pub section: &'static str, pub name: String }
   （section 取值 "llm"/"tts"/"performance"，name 非空），并给 AppSettings 加：
   pub fn declared_key_envs(&self) -> Vec<DeclaredKeyEnv>
   顺序固定 llm → tts → performance；只收非空 api_key_env；注释写明「GET/PUT /api/v1/env 的唯一真源」。
   注释同时写明边界：Mod 级密钥（memory summary_api_key_env、director staging_api_key_env）住 mods.json、
   不在 AppSettings 里，本函数不包含。
2. web_api/env_routes.rs：handle_get 遍历 declared_key_envs()；handle_put 白名单用同一份结果
   （name 命中即可），错误文案列出全部候选键名；模块头注契约表补 performance。
3. 测试：settings_tests.rs 新增 declared_key_envs 的「顺序 / 跳过空串 / 全空返回空」；
   env_routes.rs tests 新增 performance 段（GET 列得出、PUT 接受、未声明键仍 400）。
禁止：放宽「PUT 只能写已声明键名」；把 Mod 级键塞进 AppSettings。
验收：[performance].api_key_env = "X" 时 GET /api/v1/env 能看到 X，PUT {"key":"X","value":"..."} 回 200。
【文件归属】runtime/src/settings.rs、runtime/src/settings/settings_tests.rs、web_api/env_routes.rs
（不要改 live2d-ai.toml：它的注释与 max_tokens 全部归 P0b，避免同文件并发冲突）

================================================================
===== 复制给 Worker P2 =====
================================================================
【工作区】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，不同分支）。
【开工前读】AGENTS.md 的「核心层（Rust）」「前端层（Flutter）」「密钥真源 = .env」三节；以及你要改的每个文件的模块头注（头注解释了为什么不那么改）。
【硬约束】源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；密钥明文不进 GET / 日志 / WS / 导出；live2d-ai.toml 只持 api_key_env（变量名）；读密钥只能走 live2d_ai_runtime::secrets::lookup，禁止 std::env::var；写回 TOML / .env 必须保注释（merge_into_toml、plan_atomic_write 原子写）；不做没被点名的重构，不确定就停下写清哪里不确定。
【门禁·Rust 改动必跑】cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
【门禁·Flutter 改动】cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test ≠ 产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
【回报】改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」代替门禁输出。

【任务 P2：has_api_key 收敛为单一语义（值真的读得到）】
问题：settings/view.rs 的 has_api_key = 「声明了变量名」；web_api/dto.rs 的 llm_status_from/tts_status_from
= 「声明了且值非空」。同名两义 →「已配置」与 401/503 同屏。
必做：
1. runtime/src/settings/view.rs 新增：
   pub fn settings_to_view_with_keys(s: &AppSettings, lookup: &dyn Fn(&str) -> Option<String>) -> SettingsView
   has_api_key = 名字已声明 且 lookup(name) 为非空；llm/tts/performance 三处走同一私有派生函数。
   settings_to_view 的签名与语义不动（它 =「声明了变量名」，egui/纯展示用），文档写清两者差别。
2. web_api/settings_routes/mod.rs：handle_get、apply_and_write（及透传的 handle_patch）增加参数
   lookup: &dyn Fn(&str) -> Option<String>；PATCH 响应用 settings_to_view_with_keys，与 GET 同口径。
3. web_api/dispatch.rs：调用点传 &live2d_ai_runtime::secrets::lookup。
4. Flutter 文案：llm_section.dart / tts_section.dart 的「密钥绑定」ReadonlyField 由
   「已声明密钥的环境变量名 / 未绑定密钥（无鉴权）」改成「密钥已就绪 / 未设置密钥（无鉴权）」；
   llm_section.dart 顶部「表达的是声明了键名」的注释、settings_models.dart 里 hasApiKey 的文档注释同步。
5. 测试：patch_tests.rs 的 settings_to_view_drops_key_env_name_and_emits_has_api_key 等改用
   settings_to_view_with_keys + fake lookup（有值 true / 无值 false）；settings_routes/tests.rs 按新签名补 lookup。
禁止：改 settings_to_view 的签名；把 env 变量名写进响应；测试里用 std::env::set_var（用注入的 fake lookup）。
【文件归属】runtime/src/settings/view.rs、runtime/src/settings/patch_tests.rs、web_api/settings_routes/mod.rs、web_api/dispatch.rs、上述两个 Dart section

================================================================
===== 复制给 Worker P3 =====
================================================================
【工作区】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，不同分支）。
【开工前读】AGENTS.md 的「核心层（Rust）」「前端层（Flutter）」「密钥真源 = .env」三节；以及你要改的每个文件的模块头注（头注解释了为什么不那么改）。
【硬约束】源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；密钥明文不进 GET / 日志 / WS / 导出；live2d-ai.toml 只持 api_key_env（变量名）；读密钥只能走 live2d_ai_runtime::secrets::lookup，禁止 std::env::var；写回 TOML / .env 必须保注释（merge_into_toml、plan_atomic_write 原子写）；不做没被点名的重构，不确定就停下写清哪里不确定。
【门禁·Rust 改动必跑】cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
【门禁·Flutter 改动】cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test ≠ 产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
【回报】改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」代替门禁输出。

【任务 P3：清掉前后端漂移的过期默认值】
问题：shell/flutter/lib/api/settings_models.dart 的 LlmSettingsView.defaultMaxTokens = 512，
服务端 DEFAULT_MAX_TOKENS = 4096（runtime/src/settings.rs）。该常量只在服务端漏字段时兜底，实际不可达却写错。
必做：
1. defaultMaxTokens 512 → 4096，注释写明「必须与 live2d_ai_runtime::settings::DEFAULT_MAX_TOKENS 一致」。
2. 新增 Dart 测试：defaultMaxTokens == 4096；LlmSettingsView.fromJson({})（缺 max_tokens）回落 4096。
禁止：顺手改服务端默认值（那是 P0 的配置决策，不是本任务）。
【文件归属】shell/flutter/lib/api/settings_models.dart 及其测试

================================================================
===== 复制给 Worker P4 =====
================================================================
【工作区】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，不同分支）。
【开工前读】AGENTS.md 的「核心层（Rust）」「前端层（Flutter）」「密钥真源 = .env」三节；以及你要改的每个文件的模块头注（头注解释了为什么不那么改）。
【硬约束】源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；密钥明文不进 GET / 日志 / WS / 导出；live2d-ai.toml 只持 api_key_env（变量名）；读密钥只能走 live2d_ai_runtime::secrets::lookup，禁止 std::env::var；写回 TOML / .env 必须保注释（merge_into_toml、plan_atomic_write 原子写）；不做没被点名的重构，不确定就停下写清哪里不确定。
【门禁·Rust 改动必跑】cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
【门禁·Flutter 改动】cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test ≠ 产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
【回报】改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」代替门禁输出。

【任务 P4：去掉两处重复实现】
1. crates/live2d-ai-runtime/src/lib.rs：pub(crate) fn join_endpoint → pub fn（对外可复用），
   文档加一句「base_url 拼接的唯一实现」。
2. web_api/settings_routes/test_endpoints.rs：删本地那份 join_endpoint，改用 live2d_ai_runtime::join_endpoint
   （它错误类型是 Error，调用点 .map_err(|_| HttpTestError::BadUrl)）。
3. web_api/dto.rs：删恒等函数 settings_view_as_dto（带 #[allow(dead_code)]）及其测试引用
   （dto.rs 测试模块 ~382–401 行）。
验收：cargo test 全绿；grep 后 join_endpoint 只剩 runtime 一份定义。
【文件归属】runtime/src/lib.rs、web_api/settings_routes/test_endpoints.rs、web_api/dto.rs

================================================================
===== 复制给 Worker P5 =====
================================================================
【工作区】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，不同分支）。
【开工前读】AGENTS.md 的「核心层（Rust）」「前端层（Flutter）」「密钥真源 = .env」三节；以及你要改的每个文件的模块头注（头注解释了为什么不那么改）。
【硬约束】源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；密钥明文不进 GET / 日志 / WS / 导出；live2d-ai.toml 只持 api_key_env（变量名）；读密钥只能走 live2d_ai_runtime::secrets::lookup，禁止 std::env::var；写回 TOML / .env 必须保注释（merge_into_toml、plan_atomic_write 原子写）；不做没被点名的重构，不确定就停下写清哪里不确定。
【门禁·Rust 改动必跑】cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
【门禁·Flutter 改动】cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test ≠ 产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
【回报】改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」代替门禁输出。

【任务 P5：拆掉 clear_api_key / inject 三态机制】
问题：为让「无 clear 标志的 api_key_env: null」= 保持原值，桌面层多了 PatchLlmBody/PatchTtsBody 双层包装
+ inject_clear_key_flag + apply_inject_rules(_tts)，外加几乎整份 settings_routes/tests.rs。
前端 Tri.clear() 本来就能可靠表达「清空」。
新契约（单一规则）：{"llm":{"api_key_env":null}} 就是清除；不再有 clear_api_key 字段，不再有 P0-2 改写。
必做：
1. Rust web_api/settings_routes/mod.rs：删 PatchLlmBody/PatchTtsBody，PatchBody.llm/tts 直接是
   Option<Option<runtime LlmPatch/TtsPatch>>；删 inject_clear_key_flag / apply_inject_rules / apply_inject_rules_tts；
   handle_patch 映射简化；模块头注改写成新契约。
2. runtime/src/settings/patch.rs：字段级三态 Option<Option<String>> 不动（Some(None) 本来就是清除），
   只更新文档里「P0-2 保护」的描述。
3. web_api/settings_routes/tests.rs：删 inject 矩阵；新增「{"llm":{"api_key_env":null}} → 磁盘键被移除」
   与「{"llm":{"api_key_env":"NEW"}} → 设为新值」两例。
4. Dart：settings_models.dart 的 Llm/Tts patch 删 clearApiKey 及 out['clear_api_key']=true；
   settings_controller.dart 删 clearLlmApiKey/clearTtsApiKey 字段、clear() 重置、_llmPatch/_ttsPatch 实参；
   llm_section.dart/tts_section.dart 删「清除密钥绑定」ToggleField；settings_api_test.dart 同步。
5. 文档：搜索 clear_api_key，更新现行契约文档（docs/design/web-ui-spec-v3.md 相关表行）；
   docs/legacy/plans/node-d-api-contract-2026-08-28.md 是历史计划，加一行「已被本次变更取代」即可，不重写。
禁止：改 apply_and_write 的原子写/保注释逻辑；动 Mod 级 key。
验收：Rust + Flutter 门禁全绿；curl 验证 {"llm":{"api_key_env":null}} 清空。
【文件归属】web_api/settings_routes/mod.rs + tests.rs、runtime/src/settings/patch.rs、Dart 三件套 + 测试、契约文档

================================================================
===== 复制给 Worker P6 =====
================================================================
【工作区】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product）。不要动 /home/skystar/Live2D-Ai（另一个 worktree，不同分支）。
【开工前读】AGENTS.md 的「核心层（Rust）」「前端层（Flutter）」「密钥真源 = .env」三节；以及你要改的每个文件的模块头注（头注解释了为什么不那么改）。
【硬约束】源码 ≤500 行（豁免 ≤1000 需头注写理由），测试文件 ≤800 行；密钥明文不进 GET / 日志 / WS / 导出；live2d-ai.toml 只持 api_key_env（变量名）；读密钥只能走 live2d_ai_runtime::secrets::lookup，禁止 std::env::var；写回 TOML / .env 必须保注释（merge_into_toml、plan_atomic_write 原子写）；不做没被点名的重构，不确定就停下写清哪里不确定。
【门禁·Rust 改动必跑】cargo test --workspace --all-targets；cargo test --doc --workspace；cargo fmt --all -- --check；cargo clippy --workspace --all-targets -- -D warnings；cargo run -p xtask -- rust-ratio（≥95%）
【门禁·Flutter 改动】cd shell/flutter && flutter analyze && flutter test
   ★ analyze/test ≠ 产物：只要有 .dart 改动，必须重建前端，否则界面改动在 /app/ 上不生效——
     flutter build web --release --base-href /app/ --no-web-resources-cdn
     （--no-web-resources-cdn 不可省，缺省会走 gstatic CDN，断网白屏）
【回报】改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。禁止用「应该没问题」代替门禁输出。

【任务 P6：收掉 dev 里的「密钥环境变量名」编辑（必须在 P5 合并后做）】
问题：llm_section.dart 的 dev 块让用户改 api_key_env（变量名），代码自己的注释都写着
「改它容易把自己弄成未配置」；值已能在下面写进 .env，变量名这层只有风险没有收益。
必做：
1. llm_section.dart / tts_section.dart：删 if (devMode) 块里的「密钥的环境变量名」TextFieldRow
   （清除开关已在 P5 删除）。
2. 「密钥绑定」ReadonlyField 改成直接显示 envKey?.key（来自 GET /api/v1/env）：例如「绑定到 DEEPSEEK_API_KEY」；
   envKey == null 时「未绑定密钥（无鉴权）」。保留 EnvKeyField（写值）。
3. settings_controller.dart：删 llmApiKeyEnv/ttsApiKeyEnv 字段、clear() 重置、_llmPatch/_ttsPatch 的 apiKeyEnv: 实参。
4. settings_models.dart：Llm/Tts patch 删 apiKeyEnv 字段与 putTri(out,'api_key_env',...)。
5. 服务端仍接受 api_key_env（curl/dev/toml 用），不要删。
6. 文档补一句：改密钥绑定 = 改 live2d-ai.toml 的 api_key_env（界面不再提供）。
禁止：删 GET/PUT /api/v1/env；让前端持有密钥值以外的东西。
验收：flutter analyze 0 issue + flutter test 全绿；curl PATCH api_key_env 仍生效。
【文件归属】shell/flutter/lib/settings/sections/llm_section.dart、tts_section.dart、lib/settings/settings_controller.dart、lib/api/settings_models.dart

================================================================
===== 复制给 Worker P0b（P0 收口，可与 P1/P3/P4 并行）=====
================================================================
【工作区】/home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product）。不要动 /home/skystar/Live2D-Ai。
【门禁】本任务只改 live2d-ai.toml / mods.json，不改源码 → 不触发 cargo/flutter 门禁；若你顺手改了源码，则按完整门禁跑。

【任务 P0b：给 max_tokens 封顶 + 清掉 mods.json 的死亡值】
背景：P0 把 [llm].max_tokens 设成 0（省略字段 = 官方默认 8K 非思考 / 64K 思考）以避开旧 512 的截断，
但 AGENTS.md 明确 max_tokens 是「控制回复长度的硬机制（不靠提示词求模型守规矩）」。需要一次实测收口，不要长期 0。
必做：
1. 用真实端点连发 3–5 轮「需要长思考」的提问（例如让模型分步推理），每轮记录：
   - 是否出现 finish_reason=length（被截断）；
   - 正文是否正常上屏（不允许「有思考、正文空」或半句切不出）。
2. 确认不截断后，把 [llm].max_tokens 显式设成一个封顶值（官方允许 1–393216，建议先试 16384），
   并在注释里补上官方默认值出处（非思考 8K / 思考 64K，
   https://api-docs.deepseek.com/api/create-chat-completion ）。
3. mods.json 里 memory 的三个死值**删掉**（不是改）：summary_base_url / summary_model / summary_api_key_env。
   依据：crates/live2d-ai-mod-memory/src/config.rs 的 Default 是 summary_enabled=false、base_url=""、model=""、key_env=""，
   删键 = 回落安全默认；summary_enabled=false 已在 P0 落盘，保留。（改 mods.json 走 Mods API 或直接改后重启 Mod，
   因为 mods.json 不在 file_watcher 监视集里。）
4. 核验：GET /api/v1/settings 的 max_tokens 是新封顶值；全仓 grep 18181 只剩 docs/ 下的历史记录；
   再发一条对话确认仍上屏 + 出声。
5. 顺手修 live2d-ai.toml 头部 6–8 行的密钥注释：把「运行时从进程环境读取」改成
   「密钥真源 = .env（secrets::lookup；优先级 .env 快照 > 进程环境 > 无）」，与 rc.2 口径一致。
   本任务**独占 live2d-ai.toml 的所有编辑**（P1 不碰它）。

禁止：把 max_tokens 调回会被思考挤断的量级（512/4096）；把 key 写进 toml。
产出：3–5 轮实测记录 + 新 max_tokens 值 + mods.json diff。
