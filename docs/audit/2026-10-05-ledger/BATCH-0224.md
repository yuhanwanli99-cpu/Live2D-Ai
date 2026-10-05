# BATCH-0224 · ⭐ **F-0224-01（P3）**：`secret` flag 的「内容不落日志」是**约定、无机制**

Phase 1 · 域覆盖 · `mod-system/src/settings.rs`（红线 R 在**契约层**的一处）

## 跑的命令（全部只读）
```
grep -n "secret|fn |脱敏|redact" mod-system/src/settings.rs
sed -n '18,32p' mod-system/src/settings.rs
grep -rn "secret" crates/live2d-ai-desktop/src/mod_registry.rs      # ⇒ 零命中
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0224-01（P3）**
```rust
String {
    key: String, label: String,
    /// 是否密钥类（前端渲染为 password 输入；**内容不落日志**）。    // :23
    secret: bool,
}
```
| # | 事实 | 状态 |
|---|---|---|
| ① | `secret` 的**前端半边**（渲染成 password）**有机制** | ✅ B0105 已核（面板对 secret 字段**留空不提交**） |
| ② | `secret` 的**后端半边**（**内容不落日志**）**没有机制** | ⚠ `mod_registry.rs` **零命中** ⇒ **宿主从不读这个 flag** |
| ③ | 已发布 Mod **确实都没记** | ✅ **无实际泄露**（B0201：6 个 Mod、50 处日志参数逐处核） |

**⇒ 而它防的那条路恰好不是真正出问题的那条**：
- 「不落日志」靠的是**各 Mod 自己的纪律**（B0201 已核）⇒ **flag 在这里只是「前端渲染」的开关**；
- 而**真正发生过的**那条路是 **F-0111-01：ASR token 进子进程的 `argv`**
  ⇒ ⇒ **`secret` 覆盖「日志」，而泄露发生在「进程参数」** ⇒ **它防错了面**。

**建议（二选一并写清是哪一个）**：
(a) **把语义对齐到它真正在做的事** —— 注释改成「**仅供前端渲染成 password 输入**；**日志纪律由各 Mod 自负**」，
并像 B0217 的模板那样把「Mod 不得记录配置值」写成**一条指名纪律**；
(b) **或让它成为机制** —— 在 `ModLogger`（`services.rs:134-146`）上做**唯一日志入口**，
由调用方传 `&ModSettingField` 而非裸串 ⇒ 「secret 字段的**值**无法被传进去」
⇒ **(b) 更彻底，且正好落在 B0223 观察到的那处：契约层是唯一能一次性覆盖 8 个 Mod 的地方。**

**⚠ 定 P3 而非 P2**：**无实际泄露**（B0201 逐处核过）· 声明在字段注释里、无运行时后果 ·
「无机制」只是**未来的风险**。

**⚠ 且我在反证里把缺口收窄了**：(a) 试过找「宿主是否在别处读 `secret`」⇒ desktop 侧只命中
`mods_routes`/Dart（B0105 核过 `redacted_config` 会把 secret 键**从输出里删掉**，机制在
`mods_routes.rs:494` 的 `obj.remove(key)`）⇒ **「不回显」这一半有机制**，
**缺的只是「不落日志」这一半**；(b) 这**不属于红线 R**（那是「密钥不出口」，而「日志纪律」是另一条）⇒ **归本条、不并入红线 R**。

## 未核实项
1. `mod-system` 其余 8 文件未读（`session.rs` 476 · `topics.rs` 164 · `settings.rs` 余段 · `factory.rs` 142 · `status.rs` · `registry.rs` · `descriptor.rs` · `error.rs`）
2. `ModLogger`（`services.rs:134-146`）本体未读 ⇒ 建议 (b) 的改动面未估
3. Mod crates 逐文件覆盖率（18/61）
4. `dev_tools_section.dart` 余面(1874) · `tokens.dart` 余 ~600 行 · `design_tokens_test.dart` 余 ~32 断言
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **前端 `.dart` 183 未读（最大面）**
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
