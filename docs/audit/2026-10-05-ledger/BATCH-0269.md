# BATCH-0269 · ⭐ **Phase 4 攻 F-0001-01**：机制存活，但**我的措辞过宽** ⇒ 收窄

Phase 4 · **证伪 / 自相矛盾** —— 攻第三条 P1

## 跑的命令（全部只读）
```
grep -nE "tracing::|route|dispatch|match |=> \{" crates/live2d-ai-desktop/src/web_api/dispatch.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0001-01 的 Phase 4 结果 = ② 收窄**
### ① 攻法
AGENTS 明文「请求级日志在 `web_api/dispatch.rs`」⇒ **若那条日志足够早、或足够全，本条就该被推翻**
### ② 结构核到了
```
dispatch.rs:62  let route = match_route(method, path);      // ← **先路由**
dispatch.rs:65  if is_mutating_route(route, method) { … }
dispatch.rs:76    tracing::warn!(…)
dispatch.rs:88  let resp = match route { … }
```
⇒ ⇒ **路由先于日志**；而 **B0198 已核那条日志按状态阈值发**（**≥500 error / ≥400 warn / mutating 成功 info / 其余 debug**）
⇒ ⇒ **它必然发生在「响应已知」之后** ⇒ ⇒ **纯前驱失败（连响应都没形成）才真的没有记录**

### ③ ⭐ 而**我原来的措辞过宽**
原文「pre-dispatch endpoints 产生零条日志」⇒ **过宽**
⇒ **这四个路由只要返回了响应（哪怕 4xx/5xx），`dispatch.rs` 仍会记它**
⇒ ⇒ **收窄后的准确表述**：
> 「**这四个路由自身不带任何 `tracing::` 调用** ⇒ **凡是没能形成响应的失败**
> （panic / 静默提前返回）就只落在 dispatch 层的空缺里**」
⇒ ⇒ **级别维持 P1**（那正是 AGENTS 明令禁止的「排障时一片空白」）
⇒ 但**范围从「这些端点」收窄为「这些端点的*无响应*失败」**

### ④ ⭐ 而这条也示范了 Phase 4 的**第二种收益**
B0266 拿到的是**边界**（首跑窗口）· B0267 拿到的是**前提**（该 Mod 的槽位锁）·
**本批拿到的是「我自己的话说过宽了」** ⇒ ⇒ **三批分别把三条 P1 的**边界 / 前提 / 措辞**各修了一次**
⇒ ⇒ 而**级别一条都没降** ⇒ ⇒ ⭐ **Phase 4 的主要作用不是「找新问题」，是「把已有结论变得更准」**

## 未核实项
1. Phase 4 待攻 P1：F-0001-02 · F-0013-01 · F-0020-01 · F-0046-01 · F-0049-01
2. 那四个路由**具体有哪些**「无响应失败」的路径（**能否点名**）⇒ 未核；
   ⇒ 即便点不出名，**「自身零日志 + 兜底只在响应之后」这个结构**已成立
3. 能否点名会在持锁期间 panic 的 Mod（B0268 的敞口）· `refresh_from_disk` 的文件变更路径（B0266 敞口）
4. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
5. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
6. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
7. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
8. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
9. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
10. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
11. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
