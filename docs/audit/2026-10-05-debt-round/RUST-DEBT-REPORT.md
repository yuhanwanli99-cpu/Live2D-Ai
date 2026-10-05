# W1-C · Rust 债报告（task-3 / D4 依赖瘦身 + F-0062-01 Mod 密钥接缝）

- 任务：共享任务板 `task-3`；工作树 `/home/skystar/Live2D-Ai-fe`（分支 `chore/debt-round-2026-10-05`）。
- 状态：**两条任务都做完并验过**；未 commit（按硬约束），未碰 `.github/**`、`scripts/**`、`shell/**`。
- 唯一未落一半：`xtask` 的棘轮常量（不在我的写作用域）——见 §6.1，需要 Lead 收口。

---

## 1. 改动文件清单（绝对路径）

| 文件 | 改动 | 行数（改动后） |
|---|---|---:|
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/Cargo.toml` | 删 `futures-util` / `serde_with` / `tokio-util` 三个直接依赖键 | — |
| `/home/skystar/Live2D-Ai-fe/Cargo.lock` | 随删键同步（desktop 依赖列表 −3 行） | — |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/src/web_api/settings_routes/test_endpoints.rs` | `StreamExt::next()` → `reqwest::Response::chunk()` | — |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/src/web_api/settings_routes/mod.rs:61` | 新增 crate 内 `mod double_option`（同语义助手），5 处 `deserialize_with` 改指向它 | 367 |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/src/supervisor/turn.rs:43` | `use tokio_util::sync::CancellationToken` → `use live2d_ai_runtime::CancellationToken` | 682 |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-runtime/src/conversation/mod.rs:110` | 新增 `pub use tokio_util::sync::CancellationToken;`（并更新该模块 doctest 用新路径） | 556 |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-runtime/src/lib.rs` | `pub use conversation::{CancellationToken, ...}` | 217 |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/src/mod_registry.rs:471,730,754` | `reload_config` 改「按键合并 + secret 保留」；新增纯函数 `merge_mod_config` / `secret_keys_of` | 1437 |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/src/mod_registry/tests_secret.rs` | **新增**：F-0062-01 专项回归（4 条，含「全部在册 Mod」数据驱动） | 133 |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/src/web_api/external_routes.rs:700,750,790` | **新增 3 条**端到端回归：保存非 secret 后仍 401 / 显式空串清除后回到不鉴权 / 非 secret 空串照存 | 1202 |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/src/web_api/mods_routes.rs:665` | **新增**路由层回归：省略 secret 保留旧值 | 986 |
| `/home/skystar/Live2D-Ai-fe/crates/live2d-ai-desktop/src/web_api/settings_routes/tests.rs:160` | **新增**段级三态回归（替换 serde_with 后的行为锁） | 722 |

本报告：`/home/skystar/Live2D-Ai-fe/docs/audit/2026-10-05-debt-round/RUST-DEBT-REPORT.md`。

---

## 2. 任务 1 · D4 依赖瘦身：live2d-ai-desktop 25 → 22

### 2.1 计数证据（原始输出）

```
$ git show HEAD:crates/live2d-ai-desktop/Cargo.toml | awk '/^\[dependencies\]/{f=1;next} /^\[dev-dependencies\]/{f=0} f && /^[a-zA-Z]/' | wc -l
25
$ awk '/^\[dependencies\]/{f=1;next} /^\[dev-dependencies\]/{f=0} f && /^[a-zA-Z]/' crates/live2d-ai-desktop/Cargo.toml | wc -l
22
```

`cargo run -q -p xtask -- code-stats --check` 的 deps 行：当前值 **22**（棘轮上限 25，PLAN 目标 22）→ PASS。

`Cargo.lock` 的对应 diff（只删 desktop 的三条直接依赖，三个 crate 本身仍在锁里——经 runtime / reqwest 传递）：

```diff
 name = "live2d-ai-desktop"
 dependencies = [
  "cpal",
- "futures-util",
  "l2d",
 ...
  "serde",
  "serde_json",
- "serde_with",
  "tiny_http",
  "tokio",
- "tokio-util",
  "tracing",
```

### 2.2 逐条：为什么能删 / 怎么改的

1. **futures-util**（真删，用法也真改）：本 crate 唯一用法是 `test_endpoints.rs:193` 的 `use futures_util::StreamExt; let mut stream = resp.bytes_stream(); let _ = stream.next().await;`。
   等价物**存在**：reqwest 自带的 `Response::chunk()`（`~/.cargo/registry/.../reqwest-0.12.28/src/async_impl/response.rs:315`，**不带 `stream` feature 门**）就是「取下一片」，与 `StreamExt::next()` 同语义，且结果同样被忽略（该端点只判「流开得起来」）。
   改法：`let mut resp = ...; let _ = resp.chunk().await;`。行为不变，依赖真删。
2. **serde_with**（真删，用法换成 crate 内同语义助手）：唯一用法是 `settings_routes::PatchBody` 的 5 处段级三态 `deserialize_with = "::serde_with::rust::double_option::deserialize"`。
   替换为 `web_api/settings_routes/mod.rs:61` 的 14 行 `mod double_option`：`Option::<T>::deserialize(d).map(Some)`——与原 `serde_with::rust::double_option::deserialize` 逐字同语义（本 crate 只用 deserialize 面，没有 serialize 面）。
   本仓已有同款先例（`web_api/chat_routes.rs:430` 的 `deserialize_present_value` 就是手写 `deserialize_with` 助手），不是新发明。
3. **tokio-util**（删的是**直接依赖边**，用 runtime 的公开面）：本 crate 唯一用法是 `supervisor/turn.rs` 的 `CancellationToken::new()` / `child_token()` / `cancel()`，而它必须把 `CancellationToken` 交给 `ConversationEngine::run_turn`（`live2d-ai-runtime` 的公开签名，runtime 内部在 30+ 个 await 点 `select!` 它）。
   因此**无法**用 `tokio::sync` 原语在 desktop 侧替代（类型对不上），除非重写 runtime 的取消 API——那会直接动核心链路的时序铁律（AGENTS「语音输出约定 / 时序铁律」），代价与风险都不成比例，**已刻意不做**。
   采用的改法：runtime 转发该类型（`conversation/mod.rs:110`），desktop 改 `use live2d_ai_runtime::CancellationToken;`，删掉自己的 `tokio-util` 直边。
   **口径（Lead 2026-10-05 裁决：判「直边去掉即达标」）——本项的「未完成」部分在此显式记录：**

   - **未完成**：`tokio-util` **仍在依赖树里**（经 `live2d-ai-runtime` 传递），本次**没有**让该 crate 从产物/构建图中消失；删掉的只是 desktop 的**直接依赖边**（少一条耦合面与编译单元）。
   - **理由**：`ConversationEngine::run_turn` 的公开签名、以及 runtime 内部 30+ 个 await 点的 `select!` 都绑在 `CancellationToken` 上；要让传递依赖也消失必须改 runtime 的取消抽象，那是直接动核心链路时序铁律的改动，收益/风险不划算。
   - **将来前提**：只有当 runtime 的取消 API 被替换成自有类型（或该依赖被移出 conversation 模块）时，本项才可能升级为「彻底移除」。**在此之前，任何下游都不得把本项读成「tokio-util 已从产物里消失」。**

---

## 3. 任务 2 · F-0062-01：保存 Mod 配置会抹掉 secret

### 3.1 回源码核实（先证实现象，再动刀）

- 脱敏：`web_api/mods_routes.rs::redacted_config()` 把 `settings_spec` 里 `secret=true` 的 String 字段**整个删掉**（不是置空），所以 `GET /api/v1/mods` 与 `GET …/config` 里根本没有 `token`。
- 前端：`shell/flutter/lib/settings/sections/dev_tools_section.dart:891` 的 `_buildConfig()` 从 `widget.mod.config`（= **脱敏后**的配置）出发，第 897-901 行那条守卫 `if (f.secret && _stringValue(f).isEmpty) continue;` 出发点是对的，但**瞄错了形态**——secret 键压根不在起点 map 里，守卫是空转。
- 宿主：`web_api/mods_routes.rs:258` → `ModRegistry::reload_config`，旧实现第 483 行附近是 `e.config = config;`（**整份替换**）。
- 后果链：保存任意普通字段 ⇒ 配置里 `token` 被删 ⇒ `token_from_config()` 回 `None` ⇒ `POST /api/v1/external/chat` 的 `env_token.or(config_token)` 变 `None` ⇒ `check_token` 直接放行（**鉴权静默失效**），而界面回 `{"ok":true}`。
  这与账本现象逐条对上，不是推测。

### 3.2 修法选择（只做一个，并写清为什么）

**选宿主侧「按键合并 + secret 保留」**，不做前端哨兵。理由：
1. 前端拿不到 secret 值（这是脱敏的**设计要求**，不能为了保存而回传密钥）；哨兵要新增协议字段 + 改 Dart（越界），且「已设置」哨兵与「显式清除」还得再设计一套语法。
2. 宿主侧一处修复**同时覆盖两个入口**：`POST …/config` 与 `POST …/enable`（后者经 `enable_with_config → reload_config` 同一函数）。
3. 合并规则是纯函数，可独立单测，规则可以逐条钉死。

规则（`mod_registry.rs:730 pub fn merge_mod_config`，文档写在函数头注）：

| 提交里的形态 | 结果 |
|---|---|
| 键出现、值非空 | 覆盖 |
| 键**缺席** | **保留旧值**（secret 保护的地基；spec 之外的既有键也不再被顺手抹掉） |
| 键出现、值为 `""`/纯空白，且该键在 `settings_spec` 里声明了 `secret=true` | **删除该键**（= 显式清除） |
| 键出现、空串、**非** secret | 照存（不是删除语法）——与 `PUT /api/v1/env` 的「空值 = 清除整键」**刻意区分**，两者一个是 `.env` 密钥文件写入口，一个是 Mod 配置，语义不通用 |

入口：`reload_config`（`mod_registry.rs:471`）先取 `secret_keys_of(self.settings_specs.get(id))`，再 `e.config = merge_mod_config(&e.config, &config, &secret_keys)`。secret 键名来自**静态** `settings_spec`（`ModRegistry::new` 就从 factory 预填），所以**停用的 Mod** 保存配置时同样受保护。

---

## 4. 测试与门禁（原始输出）

### 4.1 新增 9 条回归

| 测试 | 位置 | 守什么 |
|---|---|---|
| `merge_mod_config_is_key_wise` | `src/mod_registry/tests_secret.rs:18` | 纯函数：未提交键保留 |
| `merge_mod_config_blank_secret_clears_only_secret_keys` | 同上 `:32` | 纯函数：只有 secret 的空串是清除 |
| `every_registered_mod_keeps_its_secret_on_config_save` | 同上 `:54` | **覆盖全部在册 Mod**：遍历 `crate::AVAILABLE_MOD_FACTORIES`，对每个带 secret 的 Mod 保存非 secret 字段后断言 secret 仍在（自动含未来新增 Mod） |
| `explicit_blank_secret_clears_stored_token` | 同上 `:103` | 显式空串真的清除（`token_from_config` 回 `None`） |
| `config_save_preserves_omitted_secret` | `src/web_api/mods_routes.rs:665` | 路由层：POST `…/config` 省略 secret 时保留 |
| `mod_config_save_without_secret_keeps_endpoint_auth` | `src/web_api/external_routes.rs:700` | **端到端**：保存后 token 仍在 **且** 不带 token 的注入请求仍 `401`（鉴权仍生效） |
| `patch_body_absent_section_is_none_and_null_is_some_none` | `src/web_api/settings_routes/tests.rs:160` | 段级三态在换掉 serde_with 后仍可区分（段缺省 / null / 对象；`dev_mode` 三态） |
| `explicit_blank_config_clears_token_and_opens_the_token_gate` | `src/web_api/external_routes.rs` | **端点级「显式清除」**：`POST …/config` 发 `{"token":""}` → 键真被删、端点回到「不鉴权」（**预期行为**） |
| `blank_non_secret_string_is_stored_not_cleared` | `src/web_api/external_routes.rs` | **语义边界**：非 secret 键的空串**照存**（≠ `PUT /api/v1/env` 的「空=清除」），且不顺手抹掉 secret |

### 4.2 targeted 测试（原始输出）

```
$ cargo test -p live2d-ai-desktop
test mod_registry::tests_secret::merge_mod_config_blank_secret_clears_only_secret_keys ... ok
test mod_registry::tests_secret::merge_mod_config_is_key_wise ... ok
test mod_registry::tests_secret::explicit_blank_secret_clears_stored_token ... ok
test mod_registry::tests_secret::every_registered_mod_keeps_its_secret_on_config_save ... ok
test web_api::external_routes::tests::blank_non_secret_string_is_stored_not_cleared ... ok
test web_api::external_routes::tests::explicit_blank_config_clears_token_and_opens_the_token_gate ... ok
test web_api::external_routes::tests::mod_config_save_without_secret_keeps_endpoint_auth ... ok
test web_api::mods_routes::tests::config_save_preserves_omitted_secret ... ok
test web_api::settings_routes::tests::patch_body_absent_section_is_none_and_null_is_some_none ... ok
test result: ok. 507 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.83s
```
```

（本轮改动前 desktop bin 为 498 条；+9 条 = **507**，**测试条数只增不减**。）

```
$ cargo test --doc -p live2d-ai-runtime
test crates/live2d-ai-runtime/src/conversation/mod.rs - conversation (line 46) - compile ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
（runtime 的 doctest 已改用新的公开路径 `live2d_ai_runtime::CancellationToken`。）

### 4.3 门禁（原始输出摘要）

```
$ cargo run -q -p xtask -- code-stats --check
| `crates/*/src` `.rs` > 500 行的文件数（上限 48） | 48 | ≤ 48 | 15 | PASS |
| Dart `lib` > 800 行的文件数（上限 7） | 7 | ≤ 7 | 2 | PASS |
| `crates/*/src` `.rs` > 1000 行的文件数（上限 4） | 4 | ≤ 4 | 0 | PASS |
| `live2d-ai-desktop` 顶层 `[dependencies]` 条数（上限 **22**——Lead 已于本轮把棘轮 25 收紧到 22） | 22 | ≤ 22 | 22 | PASS |
结论：PASS —— 判定范围内无超限；退出码 0

$ cargo fmt --all -- --check ; echo FMT_EXIT=$?
FMT_EXIT=0

$ cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -2 ; echo CLIPPY_EXIT=$?
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.82s
CLIPPY_EXIT=0
```

### 4.4 过程中撞到并修掉的一个门禁回归（如实记录）

我第一版把路由层测试直接加在 `mods_routes.rs` 里，文件 957 → 1012 行，把 `src-rs-1000` 门禁从 4 个顶到 **5 个**（limit 4）→ `code-stats --check` 退出码 1。
修法：把 F-0062-01 的 4 条注册表回归**外提**到新文件 `src/mod_registry/tests_secret.rs`（133 行，本仓既有做法：`supervisor/tests_stall.rs`），并把路由层那条压到 26 行（`mods_routes.rs` 986 行）。修后 >1000 档回到 4/4 PASS。
`mod_registry.rs` 本身 1362 → 1438（净 +76：合并函数 + 模块声明），它本就在 >1000 的棘轮档里，未新增超限文件。

补充（Lead 授权补测后）：`external_routes.rs` 1062 → **1202**（本轮共 +140，含 3 条端到端回归）。**>1000 门禁计的是「文件数」而不是行数**——该文件本来就在被容忍的 4 个里，加行不会改变计数；补测后 `code-stats --check` 仍是 **4/4 PASS**（§4.3）。若更希望这个文件不再增长，可把 3 条回归外提到 `web_api/tests_*.rs`，但代价是共享 `counter_lock`（计数器是进程级单例，跨模块各持一把锁会与既有计数回归竞态）或复制 5 个私有 helper——我没做，留作决策点。

---

## 5. 红-绿自证（先红后绿，原始输出）

### 5.1 F-0062-01：把 `reload_config` 临时改回旧的整份替换

```
$ cargo test -p live2d-ai-desktop secret   # 旧实现（e.config = config）；此轮在最终代码上跑
test mod_registry::tests_secret::merge_mod_config_is_key_wise ... ok                                  # 纯函数测试，不经过入口 → 仍绿
test mod_registry::tests_secret::merge_mod_config_blank_secret_clears_only_secret_keys ... ok         # 同上
test mod_registry::tests_secret::explicit_blank_secret_clears_stored_token ... FAILED
test mod_registry::tests_secret::every_registered_mod_keeps_its_secret_on_config_save ... FAILED
test web_api::mods_routes::tests::config_save_preserves_omitted_secret ... FAILED
test web_api::external_routes::tests::mod_config_save_without_secret_keeps_endpoint_auth ... FAILED
test result: FAILED. 3 passed; 4 failed; 0 ignored; 0 measured; 498 filtered out; finished in 0.00s

$ cargo test -p live2d-ai-desktop secret   # 还原修复后
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 498 filtered out; finished in 0.00s

# 同一条断言的失败细节（重构前那条更长的路由测试，名字/行号随 §4.4 的压缩变了，断言相同）：
thread 'web_api::mods_routes::tests::config_save_preserves_omitted_secret_and_clears_on_blank' panicked at crates/live2d-ai-desktop/src/web_api/mods_routes.rs:686:9:
assertion `left == right` failed: 省略 secret 必须保留旧值：{"path":"/y"}
  left: Null
 right: String("SECRET")
```

注意 `left: Null` = 旧实现下配置只剩 `{"path":"/y"}`，`token` 确确实实被抹掉了——**这就是账本现象的真身**。
（跑完已 `grep RED-PROOF` 确认工作树无临时代码残留。）

### 5.2 `double_option` 助手：临时换成 serde 的塌缩默认行为

```
$ cargo test -p live2d-ai-desktop patch_body   # 助手临时写成 Option::<Option<T>>::deserialize（= 缺省/null 塌缩）
test web_api::settings_routes::tests::patch_body_absent_section_is_none_and_null_is_some_none ... FAILED
test web_api::settings_routes::tests::patch_body_section_parsing ... FAILED
panicked ... tests.rs:182:5: left: None  right: Some(None)
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 503 filtered out; finished in 0.00s

$ cargo test -p live2d-ai-desktop patch_body   # 还原同语义助手后
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 503 filtered out; finished in 0.00s
```

### 5.3 D4 另外两条的结构性证据

- `Cargo.toml` 三键已删 + `Cargo.lock` 对应三条消失（§2.1 diff）；`cargo clippy --workspace --all-targets -- -D warnings` 与 507 条测试在**没有这些直接依赖**的情况下全绿——若代码里还有 `use futures_util::…` / `use tokio_util::…` / serde_with 路径，编译会直接失败。
- `cargo run -q -p xtask -- code-stats --check` 的 deps 行 = **22**（PLAN 目标值），这是「真删键」的计数侧证据（不是 feature-gate / optional 的计数把戏）。

---

### 5.4 端点级「显式清除 / 空串照存」：两次定点突变

把 `merge_mod_config` 里 `blank_secret` 的判定分别改成两个极端，看两条新回归是不是**各自**会红：

```
$ # 突变 A：secret 空串永不删除（blank_secret = false）
$ cargo test -p live2d-ai-desktop blank_
test mod_registry::tests_secret::merge_mod_config_blank_secret_clears_only_secret_keys ... FAILED
test web_api::external_routes::tests::explicit_blank_config_clears_token_and_opens_the_token_gate ... FAILED
test web_api::external_routes::tests::blank_non_secret_string_is_stored_not_cleared ... ok
test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 503 filtered out; finished in 0.01s

$ # 突变 B：任何空串都当删除（丢掉「必须是已声明的 secret」这个条件）
$ cargo test -p live2d-ai-desktop blank_
test mod_registry::tests_secret::merge_mod_config_blank_secret_clears_only_secret_keys ... FAILED
test web_api::external_routes::tests::explicit_blank_config_clears_token_and_opens_the_token_gate ... ok
test web_api::external_routes::tests::blank_non_secret_string_is_stored_not_cleared ... FAILED
test result: FAILED. 2 passed; 2 failed; 0 ignored; 0 measured; 503 filtered out; finished in 0.00s

$ # 还原后
$ cargo test -p live2d-ai-desktop blank_
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 503 filtered out; finished in 0.00s
```

两个方向各自可红：A 打掉「能清除」，B 打掉「只清 secret」——证明这两条不是同一条断言的复制品。

---

## 6. 未决问题 / 需要 Lead 决策

### 6.1 棘轮常量（**Lead 已收口**）

`xtask/src/code_stats/mod.rs:93 RATCHET_DESKTOP_DEPS` 已由 Lead 在本轮从 25 收紧到 **22**（`code-stats --check` 现在显示上限 22，实测 22 → PASS）。
这一条**不在我的写作用域**（`xtask/**`），我未改动该文件。

### 6.2 tokio-util 口径（**已裁决**：直边去掉即达标）

Lead 2026-10-05 裁决：采纳「直边去掉即达标」。「未完成 + 理由 + 将来前提」已逐字写进 §2.2 第 3 条。
**注意**：这不是「`tokio-util` 已完全移除」——它仍是 runtime 的传递依赖；本项只声称「desktop 不再有该直接依赖边」。

### 6.3 （已补）端点级「显式清除」与「非 secret 空串照存」

Lead 2026-10-05 授权补做，已在 `external_routes.rs` 的 tests 模块内加两条（见 §4.1）：

- `explicit_blank_config_clears_token_and_opens_the_token_gate`：经 `POST …/config` 发 `{"token":""}` → 配置里**键真的没了**、不带 token 的注入请求**不再 401**（200）——**预期行为**；
- `blank_non_secret_string_is_stored_not_cleared`：`{"text_template":""}` → **照存空串**（不是删键），且同一次保存**不得**抹掉 secret。

两条都做了变红自证（§5.4）。

---

## 7. 我没有做的事 & 原因

1. **没改 Dart**（`shell/flutter/**` 一行未动）：修法选宿主侧合并，前端现有守卫（留空 = 不提交）在新语义下**才真正成立**；改 Dart 属越界且无必要。
2. **没跑全套门禁**：task-3 明确「跑 targeted test 即可，全套由 Lead 跑」。我跑的是 desktop 包全量（507）+ runtime doc（3）+ workspace clippy + 全仓 fmt check + code-stats；`cargo test --workspace --all-targets` 留给 Lead。
3. **没重建发布产物 / 没点火**：二进制与 Flutter 产物由 Lead 统一重建复验。
4. **没动 wasm 目标**（`cargo check --target wasm32-unknown-unknown` 未跑）：本轮未改 wasm 相关代码。
5. **没做任何 git 写操作**（无 commit / checkout / stash / reset），工作树改动全部留在磁盘上等 Lead 复核。
6. ~~没加端点级清除断言~~ → Lead 已授权补做（§6.3 / §4.1）：两条新回归（显式清除 / 非 secret 空串照存）均已变红自证（§5.4）。
