# BATCH-0278 · ⭐ **F-0278-01（P3）**：一个**字段注释**落后于**同一文件里 140 行下方**的正确说明

Phase 1 · 域覆盖 · 回读 `l2d-wasm-demo` 余面（`web/surface/background.rs` + `input.rs`）

## 跑的命令（全部只读）
```
sed -n '1,14p' crates/l2d-wasm-demo/src/web/surface/background.rs
sed -n '236,248p' crates/l2d-wasm-demo/src/web/surface/input.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0278-01（P3）** —— B0277 那条「未核实」**结清，且答案是「有一处注释落后」
### ① **结清**：CSS vs framebuffer ⇒ **framebuffer 是对的，AGENTS 也是对的**
| 来源 | 说的是 |
|---|---|
| `web/surface.rs:8` | 「[`background`]：舞台底色与背景图**画进 frameb…**」 |
| `background.rs:1-3` | 「把 stageColor 纯色底 + 背景图**画进 framebuffer**，**而不是依赖 canvas 的 CSS**」 |
| `background.rs:5-13` | WebGPU canvas 只能不透明合成（wgpu 29 只报 `Opaque`）⇒ 透明 clear 变**不透明黑** ⇒ **盖住 CSS** |
| `input.rs:236-247` | 「**不在这里**画」+「**实测证伪**」+「现在…**画进 framebuffer**」 |
⇒ ⇒ **四处一致说 framebuffer** ⇒ ⇒ **AGENTS rc.4 ⑦ 不是漂移**

### ② ⭐ 而**唯一说 CSS 的那一行，就在这个文件里**
```rust
// input.rs:100-102  ← 【过时】
/// stage-bg.dataUrl（…）。**应用为 canvas CSS background-image**（cover 缩放），与 background-color 共存。
pub bg_data_url: Option<String>,
// input.rs:236-247  ← 【正确，就在同一个文件里】
// 舞台底色与背景图**不在这里**画 … 这里**曾**写成 canvas 的 CSS … **实测证伪** …
// 现在两者都由 [`super::background`] 在**背景预通道**里画进 framebuffer
```
⇒ ⇒ **一个消息结构体的字段注释，在描述一条已被本文件证伪的路**

### ③ ⇒ **影响不止是「文案陈旧」，而是「会把人引到错误的层去查」**
`bg_data_url` 是**那条消息结构体上的字段** ⇒ 将来若背景不出图，
**顺着这条注释去查 CSS / `stage_css.rs` 会一无所获** ⇒ **真正的落点在 `background.rs` 的背景预通道**
⇒ ⇒ **修法**：把 `:100-102` 改成与 `:236-247` 同口径并**加一句指向 `background.rs`** ⇒ **成本两行、零风险**
⇒ ⇒ **定 P3**：只影响一个字段注释的准确性、无运行时后果（**实现是对的**）

### ④ ⭐ 而这一条的**真正教训**在结构上
**一个文件里同时存在「过时的说法」与「它被证伪的记录」**，
⇒ ⇒ **只搜字段名的人**（`grep bg_data_url`）**只会拿到旧的那段**
⇒ ⇒ ⇒ **可提炼**：**把「某条做法被证伪」记在代码里是有价值的；但若那个字段的注释仍在描述它，**
**这条记录就保护不了任何人** ⇒ ⇒ **作废的说明要就地改掉，不能只在别处记「已作废」**
⇒ ⇒ 这与 B0212 的「同一文件里**同时**有旧注释与自我更正」**正好相反**：
**那次是更正写在了正文、注释留旧；这次是正文已更正、注释还是旧的** ⇒ ⇒ **两种都要治**

## 未核实项
1. ⭐ **`apply` 的分支体**未读 ⇒ 「是否在某个平台/某条路径上 CSS 仍生效」**未证**
   ⇒ 但该注释**没有说「仅在某条件下」** ⇒ **至少是不完整的** ⇒ **不因未证例外而上调级别**
2. `web/surface/{render,idle,gpu}.rs` 与 `main.rs`(962) 未读
3. `stage-bg.dataUrl` 的**发送方**在 Flutter 侧哪一处（`live2d_bridge.dart`?）未核
4. 真实动作计划的 token 长度（F-0020-01 永久敞口）· `shell/flutter` 是否真可能出现密钥
5. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
6. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
7. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~465 · `error_banner` ~15
8. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
9. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
10. **两侧措辞同步无机制**（B0260 敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
11. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
12. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
13. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
14. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
