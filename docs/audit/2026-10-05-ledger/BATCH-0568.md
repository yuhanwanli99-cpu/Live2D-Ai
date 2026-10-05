# BATCH-0568 落盘（无装饰）· director 三个文件 + 目录全貌

## 目录全貌（wc -l，9 个文件共 4663 行）
arbiter.rs 88 · staging.rs 150 · ledger.rs 320 · plan.rs 380 · staging_http.rs 401
decision.rs 444 · presets.rs 455 · tests_staging.rs 683 · tests.rs 840
（tests_staging.rs 683 / tests.rs 840 都在测试文件 <=800 的上限内，tests.rs 840 **超了 40 行**）

## arbiter.rs:1-4（三级仲裁，纯函数）
//! 三级仲裁（P1-3，纯函数）：规则 / 异步 / 标签 按优先级合并，...
//! 优先级（见 crate::plan）：规则 10 < 异步 40 < 标签 80。同一句被多个来源命中时
//! **高优先级覆盖低优先级**；同级保留先到的（稳定）。
=> 三级优先级的数字（10/40/80）写在头注，判据在 crate::plan，两处不重复定义（与
   B0560 核的「10 个方法都是 1 句『见 trait』」同一手法）。

## decision.rs:1-4（纯函数层）
//! 导演决策的**纯函数**层（无 IO / 无网络 / 无时钟 / 无随机）。
//! 输入 = 本轮输入正文 + 情绪词表档位；输出 = Decision（情绪 / TTS **建议**参数）。
//! 同一输入恒等输出，单测不需要 mock / sleep。
=> 五个「无」逐个列出来（IO/网络/时钟/随机 实际四个 + 一句推论），
   而 B0659 核的 AGENTS 纪律是「判据在纯函数、采集在宿主」（B0543）。

## ledger.rs:1-4（账本 = state_json 唯一数据源）
//! 决策账本：最近 N 条决策 + 计数 —— **state_json 的唯一数据源**。
//! **顺序关联**：TurnPrompt 的 payload 是**正文**（不带 turn id），TurnEnded
//! 的 payload 才是 turn id。两者都由 host 在**同一个 Mod worker 上顺序**投递。
=> 这条头注解释了 B0563 核的 topics.rs：为什么 TurnPrompt 与 TurnEnded 都存在、
   以及为什么顺序投递这件事是**前提**而不是巧合。
   同时「唯一数据源」四个字把 state_json 的写入口收敛到这一个文件（与 B0649 核的
   「契约 add-only」同族：不是「随便哪个文件都能写」）。

## 三个可核点
1. 优先级 10/40/80 写在 arbiter 头注、判据在 plan —— 与 B0560「不复制说明」同族。
2. decision.rs 把「纯函数」写成四条否定（无 IO/网络/时钟/随机）并给一条推论
   （单测不需要 mock / sleep）—— 与 B0543「判据在纯函数、采集在宿主」对应。
3. ledger.rs 头注解决了 topics.rs 的疑问：两个 topic 的 payload 不同，是为了让
   同一个 worker 能**顺序关联**正文与 turn id。

## 对 F-0637-01 的意义
director 共 4663 行（含两个测试文件），其中 tests.rs 840 **超过 800 行上限**。
=> 若按 AGENTS 真删 director，需删 9 个源文件 + 两处 Cargo 依赖 + 注册表一行 +
   settings.rs 的一处用例；这是一次**跨 6 个文件**的改动，不是「一个目录」。

## 未核
tests.rs 是否真的超过 800（需看头注豁免理由）· arbiter/decision/ledger 其余本体 ·
presets.rs / plan.rs / staging*.rs 本体 · 其余 8 个 mod crate 本体 ·
mod-system 的 tests/ 四行 · shared/ 其余 8 个 json
