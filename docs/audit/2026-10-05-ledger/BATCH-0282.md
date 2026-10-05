# BATCH-0282 · ✅ **F-0278-01 那条链彻底闭合** —— 只剩**一处注释**，实现 40 行都写对了

Phase 1 · 域覆盖 · `web/surface/render.rs:405-413`（背景分支怎么选）

## 跑的命令（全部只读）
```
sed -n '405,420p' crates/l2d-wasm-demo/src/web/surface/render.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **链条闭合**
```rust
        });  background.draw(&mut pass);  }        // ← **背景预通道**
    gpu.queue().submit(Some(encoder.finish()));
}
// 模型通道：**Load** —— 预通道刚铺好的底色与背景图**必须留下**，
// **若仍用默认的 Clear(TRANSPARENT) 会把背景整块擦掉。**      // :411-412
match core.render_to_view_submit_with_load(&view, config.format, **wgpu::LoadOp::Load**) {
```
⇒ ⇒ **第三个参数就是 `LoadOp::Load`**，而注释**写明了「默认会做什么、为什么错」**
⇒ ⇒ ⭐ 那是 **P26-a 的加强版**（B0229 是「这不是微优化」）—— **这次直接点名「那个默认值」及其后果**
⇒ ⇒ 且**两段结构是显式的**：**背景预通道**（pass）· **模型通道**（`Load`）⇒ **顺序就是设计**

### ⇒ 而 F-0278-01 的**完整链条**（七步，每一步我都核过）
| 步 | 内容 | 核验 |
|---|---|---|
| ① | 消息字段 `bg_data_url` | B0277 |
| ② | ⚠ **该字段的注释说「走 CSS」** | **B0278（F-0278-01，唯一错的一步）** |
| ③ | 真实机制是**画进 framebuffer**（WebGPU 只能不透明合成） | B0278 |
| ④ | **同一文件** `:236-247` 已写明「实测证伪」 | B0278 |
| ⑤ | 全树**无活的 CSS 背景写**（唯一字面在 `#[test]` 黑名单） | B0280 |
| ⑥ | 禁令写在**边界**（`surface.rs:12-13`，带 C1/C6/C8 编号） | B0281 |
| ⑦ | 包装关系（`render_to_view_submit` 转调 `..._with_load`） | B0281 |
| ⑧ | **模型通道用 `Load`**，注释点名默认 `Clear` 会擦掉背景 | **本批** |

⇒ ⇒ ⭐ **结论：一个 322 行的文件里，只有 2 行注释是错的，其余 40 行实现与说明都写对了。**
⇒ ⇒ 而 F-0278-01 定 **P3**（两行注释、零运行时后果、修法两行）**站得住**

## 未核实项
1. `web/surface/gpu.rs`(168) 与 `main.rs`(962) 未读
2. `apply_bridge_effects` 的**调用顺序**未核（B0279 留）
3. `stage-bg.dataUrl` 的**发送方**在 Flutter 侧哪一处 —— 未核（B0277 留）
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
