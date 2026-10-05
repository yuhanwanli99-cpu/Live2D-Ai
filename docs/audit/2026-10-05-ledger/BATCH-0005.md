# BATCH-0005 · 前置路由：external / voice 两条注入端点

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/external_routes.rs` — 1062（生产逻辑 1-425，测试 427-1062）
2. `crates/live2d-ai-desktop/src/web_api/voice_routes.rs` — 498

合计 2 文件 / 1560 行。

## 为验证调用链而读的清单外文件（只读片段，不计入本批审计结论）
- `mod_registry.rs:474-489`（`is_enabled` / `config` / `contains` 的确切语义）
- `mod_registry.rs:88-105`（`RegisteredMod` 的 `config` 初值 = 空对象）
- `mod_registry.rs:1-60` 附近（`ModFactory` / `AVAILABLE_MOD_FACTORIES` 装配面，用于验证「Mod 缺席」在生产是否可达）

## 跑过的命令（全部只读）
```
wc -l crates/live2d-ai-desktop/src/web_api/{voice_routes,mods_routes}.rs
grep -n "pub fn config\b" -A 14 crates/live2d-ai-desktop/src/mod_registry.rs
grep -n "pub fn is_enabled" -A 8 crates/live2d-ai-desktop/src/mod_registry.rs
grep -n "struct ModEntry|pub config" -B 3 -A 6 crates/live2d-ai-desktop/src/mod_registry.rs
grep -c "tracing::" ... ; grep -c "println!|eprintln!" ...
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 G（安全）：
  - 循环：token 优先级 env(`secrets::lookup`) → Mod config → 不鉴权，两条端点都**没有**直接 `std::env::var` ✔（红线 R 满足）
  - 415/403/405/401/503/400 分支齐全，每个都有稳定 code ✔
  - **`check_token` 用 `b == expected` 非恒时比较**——loopback-only + 需本地代码执行才能测响应时间，实际不可利用；**不记发现**（避免制造噪声）
  - **Mod 缺席 ⇒ 门禁与鉴权双双消失** → F-0005-01
- 维度 C（三态）：✔ 忙碌刻意 200 + `ok:false`（external :306-312 / voice :367-374），非 5xx，语义在头注明写
- 红线 R（Mod 边界）：✔ 注入统一落 `supervisor.say`（external :147 / voice :352），未绕过 core 仲裁
- 维度 E（契约）：两条端点**共用**同一套「注入端点」约定（415 先于 403、busy 回 200、503 supervisor_unavailable），
  这与 dispatch 系端点的 403 口径**不同族**——已在 F-0004-03 记录，此处确认前置族内部**自洽**（3/3 条前置路由一致）
- 维度 J（测试质量）：✔ 真实——`external_routes` 每个分支一个测试并断言状态码；
  `voice_routes` 用 `evaluate_mode` 四态覆盖。**未见恒真断言**
- 红线 K：✔ 无外链

## 已核实并主动放弃的假设（记录以免下批重走）
1. 「voice 端点没过滤空 env token，会把端点锁死」——**证伪**：`effective_token`
   （voice_routes.rs:452-457）显式 `filter(|s| non_blank(s))`，且头注把这条危害写明了
   （:450-451「空 token 若算『已配置』，任何不带 token 的请求都会 401」）。external 端点用内联
   `.map(trim).filter(!empty)`（external_routes.rs:260-262）达到同一效果。**两侧都正确，只是写法不同**。
2. 「Mod 启用但 config 键缺失 ⇒ token 被静默丢弃」——**证伪**：`RegisteredMod::new` 的 `config`
   初值是空对象（mod_registry.rs:101），且 `config(id)` 对任何在册 Mod 恒回 `Some`
   （:479-481），故 `None` 分支只在「Mod 完全不在注册表」时命中。
3. 「输入文本未限长就进模板渲染」——`read_body_from_request` 的 1 MiB 上限（mod.rs:579-586）已封顶，
   且 `MAX_TEXT_LEN` 对**渲染后**文本判定（:280），模板膨胀也拦得住。**不构成缺陷。**

## 未核实项
1. `AVAILABLE_MOD_FACTORIES` 的当前内容与 `cli_entry` 的装配面未逐行读（属 mod_registry / cli_entry 批次）；
   F-0005-01 的「生产不可达」结论依赖「生产恒经 AVAILABLE_MOD_FACTORIES 装配」这条**文档声明**
   （external_routes.rs:41），本批按文档采信，**标 未核实**，留 mod_registry 批次确认。
2. `live2d_ai_mod_external_input::token_from_config` / `render_from_config` 的实现未读（Mod crate 批次）。

## 本批新增
P0 0 · P1 0 · P2 1 · P3 1
