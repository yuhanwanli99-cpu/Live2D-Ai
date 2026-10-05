# BATCH-0065 · Mod 根收尾：`settings_spec` 契约

Phase 1 · 域覆盖 · Mod 根（第 6 批）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-system/src/settings.rs` — 155（**全读**：五种字段 + `validate` + 测试前半）
2. `crates/live2d-ai-mod-system/src/session.rs` — 476（B0061 已核，本批复用）

## 跑过的命令（全部只读）
```
sed -n '1,80p' mod-system/src/settings.rs ; sed -n '81,120p' mod-system/src/settings.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（三项核验通过）
1. `secret` 标记是**单一数据源** ⇒ **F-0062-01 建议①的修法有现成地基**
   （`reload_config` 已在同一结构体上，`self.settings_spec(id)` 就是 secret 键清单，改动约 5 行）
2. `validate`（:73-80）拒绝**重复字段 key** ⇒ 堵死「同一键两种身份」
   （让某键在 GET 里明文、在面板里当普通框）。**第三处「结构上堵死一类攻击面」的设计**
   （前两处：`compose_slots` 的类型级隔离、`sanitize_session_id` 的字符白名单）
3. 「禁止 Mod 注入 HTML/JS/DOM」写在**数据契约文件头部**（`settings.rs:3`），
   读前端面板的人不必翻 UI 代码就能看到

## 未核实项
1. `mod-system/src/{topics,factory,error,status,descriptor,registry,lib}.rs` 未读
2. `session_scope.rs` 的容量上界与 `:1-110` / `:266-675` 未读
3. `persona`(1004) / `director`(902) 实现本体未读；`template` crate 未读
4. `external-input` / `voice-input` 的 HTTP 门禁回归是否覆盖「token 被抹」这一态 —— 未查

## 本批新增
**0 条** ｜ **Mod 根小结**：6 批覆盖 `mod-system` 全契约面 + 5 个在册 Mod 的 spec 面 +
`session_scope` 实现面 ⇒ **产出 2 条 P2**（F-0060-01 写通道失败开放、F-0062-01 secret 被抹），
**两条都落在同一条信任边界上**；**读通道、会话隔离、路径归一、spec 硬ening 四处核验通过**。
按停止规则，Mod 根已达「连续 2 批零发现」+ 已取得实质产出 ⇒ **可以换根**。
