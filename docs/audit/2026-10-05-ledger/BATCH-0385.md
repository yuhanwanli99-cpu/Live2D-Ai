# BATCH-0385 · ⭐⭐⭐ **同一句话写在两侧**；判据真源被点名且「**handler 不得内联等价判定**」

Phase 1 · 域覆盖 · **Rust**（服务端）—— `voice_routes` 的唤醒闸 + PTT 分支（B0384 留）

## 跑的命令（全部只读）
```
grep -rn "wake_phrase" crates/live2d-ai-mod-external-input/src/                     # ⇒ **零命中**
grep -rln "voice-input" crates/*/src/ --include=*.rs | head -4                      # ⇒ desktop/{mod_registry,main,supervisor,web_api/voice_routes}
grep -rn "wake_phrase" --include=*.rs crates/ | head -5
sed -n '52,82p' crates/live2d-ai-desktop/src/web_api/voice_routes.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0383 的问题第三次被回答，而这次带机制**（0 条新发现）
### ① 四个闸门，顺序「**钉死在 handler 里**」（`:52-56`）
| 序 | 条件 | 码 |
|---|---|---|
| 1 | `manual_enabled == false` | `403 voice_manual_off` |
| 2 | ⭐ **`wake_phrase` 显式为空** | `403 voice_gate_closed`（**总闸 = 唤醒短语**） |
| 3 | 清洗后**不以**唤醒短语开头 | `400 wake_phrase_required`（空输入也算没听见） |
| 4 | 命中 | **剥掉唤醒短语** → 归一化 → 空文本 → 长度 |

⇒ ⇒⭐⭐⭐ **而第 2 条里写着「**键缺失**不在此列 —— 走产品缺省『小可爱』」**
⇒ ⇒⇒⭐ **这直接解释了 B0383 的「不对称」**：
**服务端明确区分「显式空」（= 关）与「键缺失」（= 缺省）**，
**而客户端 `_loadWakePhrase` 把两者都收敛成「缺省那个词」**
⇒ ⇒⇒ **这是有意的**：**客户端要一个可读的词 · 服务端要一个闸门状态** ⇒⇒⇒ **两边的不对称各有各的道理**

### ② ⭐⭐⭐ **同一句话写在两侧，措辞对得上**
- **B0382 核过的客户端注释**：「**同一道预检：PTT 也走 voice-input 端点（手动闸 / 总闸不变）**」
- **本批服务端**（`:77-79`）：「`"ptt": true` 是**用户显式按键**……闸门对该请求**跳过唤醒匹配**……
  **手动闸 / 总闸（`wake_phrase` 显式空）/ 清洗 / 归一化 / 长度 / token 全不变**。若按住时仍说了唤醒词，**顺手剥掉**。」
⇒ ⇒⇒ **这不是靠工具同步的两处，是**两边各自写着同一句话**的一处** ⇒⇒ **我在本仓见到的最强的一次「一处改动、两处一致」**

### ③ ⭐⭐ 而 `wake_phrase_matched` 记着一次**谎报**（`:69-72`）
> 「`wake_phrase_matched` 是**如实**的命中标志（L1，2026-09-16）：常驻路径命中才为 true；
> **PTT 裸正文不含唤醒词时为 false** —— **旧实现恒 `true`，等于对「按住说话」谎报「听见了唤醒词」**。」
⇒ ⇒ **又一次「如实报标志」**（B0348 的 `residue` 族）⇒⇒ **它防的是「给一个恒真的标志」**

### ④ ⭐⭐ 而判据真源被点名，且**禁止内联**
> 「真源是 Mod crate 的纯函数 [`live2d_ai_mod_voice_input::gate::evaluate_mode`]（**四态**），
> handler **不得**内联一份等价判定。」                                       // :64-66
⇒ ⇒ **又一次「判据集中、禁止走私」** ⇒⇒ **B0291「注释说为什么、要看它指向的定义」的实操版**
⇒ ⇒ ⭐ **而这个 crate 叫 `live2d-ai-mod-voice-input`（不是我搜的 `live2d-ai-mod-external-input`）**
⇒ ⇒⇒ **开头那个「零命中」是路径错、不是事实** ⇒⇒ **「换个文件问」第五次**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `gate::evaluate_mode` 的**四态本体**（`wake_phrase_required` / `voice_gate_closed` / …）
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
