# 0.3.6 执行计划：热换 P0 + cue 重绑 + 设置纪律

> 状态：**已收口**（2026-10-18；本地 tag `v0.3.6-rc.1`…`v0.3.6-rc.4`，不 push；发布说明
> `docs/releases/v0.3.6-rc.4.md`）。立档 2026-10-18 · 上游：`PROMPT-3.0-motion-first-2026-10-17.md` 与
> `docs/architecture/CURRENT.md`。
>
> 写法：本块 tag `v0.3.6-rc.1` … `v0.3.6-rc.z`；**最后一个 `v0.3.6-rc.(z+1)` 只清这一块的
> 拆分、测试、文档和棘轮，不新增功能**。`z` 不预设，由执行 agent 按代码拆。每步换一队 agent、
> 一次本地提交、一个本地附注 tag，**不 push**。`Cargo.toml` 仍为 `0.3.1`，本地 rc 不改版本串。

## 0. 现状与问题（0.3.5 已有什么）

| 项 | 现状 | 证据 |
| --- | --- | --- |
| 换模骨架 | iframe 内热换：generation 守卫、失败回滚旧 URL、成功清旧贡献 / pending、按新 id 重载 field map | `crates/l2d-wasm-demo/src/web/swap.rs` |
| P0 缺陷 | 真实浏览器里 Bai ↔ Epsilon 双向都 panic：`WebDevice::create_buffer` → `create_buffer_init` → `ayagami_render::ModelRenderer::load_model` → wasm `unreachable`；单独打开任一模型（`/render?model=…`）正常 | 0.3.5 手测；根因未定位 |
| cue 计划 | `DirectorCuePlan` 在 Flutter 侧整表替换、按 `sentence_seq` 取用；换模成功时**不清计划**，`loaded` ack 也没有模型身份 | `shell/flutter/lib/live2d/live2d_stage.dart`、`app/shell_cue_voice_wiring.dart` |
| 设置 | 已有草稿 / 保存 / 放弃骨架（`SettingsController`、`_saveAll`、`_discardAllDrafts`）；**已知漏点**：动作幅度滑条经 `ActionScalesSyncer.schedule` 把草稿即时下发到舞台，dev_mode 的调试临时覆盖另有 `pin` | `app/shell_prefs.dart`、`live2d/action_scales_sync.dart` |

## 1. 非目标

- 不做 0.3.7 的 LLM 云/本地与动作扩充；不碰 Windows（0.4.0）。
- 不改 `Cargo.toml` 版本串；不新增运行时依赖；不 push。
- **不**用「重载 iframe」当默认修法（违反 0.3.5 的「不重载 iframe」）；只有 T1 证据判定必须时，
  才交维护者裁决。
- 不重构整个设置面板；只收敛「修改 → 副作用」的边界。

## 2. 任务

### T1（P0）热换 panic：先取证，再修

写作用域：`crates/l2d-wasm-demo/src/web/swap.rs`、`crates/l2d-wasm-demo/src/main.rs`、
`crates/l2d/src/renderer/model_core.rs`（必要时同目录）。

1. **取证**：wasm 入口用既有 `web_sys::console` 自写 panic hook（不新增依赖），把 panic 文本与栈
   打到 console；`swap.rs` 加分阶段日志：texture 解码 / `renderer.load_model` / `PoseStack` /
   rollback。先拿到**第一行 panic 文本**，不靠猜。
2. **复现矩阵**：Bai→Epsilon、Epsilon→Bai、连续 5 次、播放中换 / 空闲换；记录 panic 出现在
   **正向**还是**回滚**路径（`core.load_model` 半失败会再 load 旧 URL，可能是第二次 load 才炸）。
3. **按证据选修法**（优先级从低风险到高风险）：
   - A：换模前先卸载旧模型并让出 1–2 帧（停 rAF → unload → 下一帧 load），排除在飞资源；
   - B：在同一个 `GpuContext` 上**新建 `ModelRendererCore`**，装载成功后再原子替换旧核心，
     失败丢弃新核心、旧核心继续用；不改 iframe；
   - C：若证据指向 pinned `ayagami` 的二次 `load_model` 状态缺陷，才考虑 `[patch]` 固定 rev
     并记录补丁内容；**不静默 fork**；
   - D：iframe 重载——最后手段，必须先拿维护者裁决，并在 CURRENT 里改掉 0.3.5 的「不重载」表述。
4. **不变量**（修完逐条回归）：失败保留旧模型；generation 守卫有效；成功后 revoke 旧贡献 / pending、
   保留 idle；坏 URL 只回 error 帧；连点两次换模只装最后一次。
5. **验收**：真实浏览器（WSL 服务 + Windows Chrome/Edge）双向各 10 次，无 panic、console 无 error、
   ≥55 FPS（实测多少写多少；不达标就写「未达标」）。单测覆盖 swap 状态机（fake loader：成功 /
   失败 / 回滚 / 连点），WebGPU 真路径只能肉眼，报告标「未验」边界。

### T2 cue / field map 重绑（依赖 T1 成功路径）

写作用域：`crates/l2d-wasm-demo/src/web/{swap.rs,surface.rs}`、
`crates/l2d-wasm-demo/src/preset/{mod.rs,field_map.rs}`、
`shell/flutter/lib/live2d/{live2d_stage.dart,live2d_bridge.dart}`、
`shell/flutter/lib/app/{shell_cue_voice_wiring.dart,shell_lifecycle_wiring.dart}`。

1. `loaded` / error ack 增加 `model_id` 与渲染面自增的 `model_epoch`；Flutter 只认当前 epoch 的
   ack 与 preset 回执，旧 epoch 的迟到帧丢弃。
2. 换模成功（以匹配 URL 的 `loaded` 为准）：清 `DirectorCuePlan` 与 `PresetStatus`；对新模型
   下发一次 `preset none`（两槽放回）；按新 model id 重设 `ActionScalesSyncer.setModelContext`
   并补发有效幅度。
3. 换模失败：不换 epoch、不清计划、旧模型与旧 cue 全部不动。
4. 渲染面：`load_field_map` 失败**不得沿用旧表**；ack 带 field map 来源 / 降级原因；旧 epoch 的
   `preset` 帧直接 Ignore。
5. **验收**：换模时正在播一句 → 旧 cue 不会落到新模型；下一句 cue 用新 field map 生效；dev 面板能
   看到 model id / epoch。Dart 单测（cue plan 清理、epoch 门控）+ Rust 单测（旧 epoch Ignore、
   field map 重载）+ wasm 重建后肉眼。

### T3 设置纪律：草稿不生效，只有「保存并重载」

写作用域：`shell/flutter/lib/app/{shell_prefs.dart,shell_settings.dart}`、
`shell/flutter/lib/live2d/action_scales_sync.dart`、`shell/flutter/lib/settings/**`、
`docs/architecture/CURRENT.md`、`AGENTS.md`、相关设置计划书。

1. **先审计**：列出所有「改设置 → 副作用」路径（服务端 PATCH / 本机偏好落盘 / 舞台 sync / 音频 /
   引擎 reload / `mods.json` / `.env` 密钥），逐条标注触发条件与现有测试。
2. **规则文本**（写入 CURRENT + AGENTS）：设置面所有修改只进草稿；**切换分区、关闭设置、点取消
   都不写盘、不重建引擎、不向舞台/音频下发**；只有「保存并重载」在全部成功后才生效并只刷新一次；
   保存失败草稿保留。**例外**：模型 activate 是独立立即动作；`dev_mode` 调试临时覆盖是显式开发工具
   （若维护者要求绝对禁止，则删除该入口）。
3. **已知要改**：
   - 动作幅度滑条：从「草稿即时下发」改为「舞台只吃已保存值」；保存成功后再下发；
     `ActionScalesSyncer` 保留去重 / 重建补发职责，删除草稿路径的 `schedule`。
   - 关闭 / 放弃：确认无残留 sync、无 PATCH、无引擎 reload；`_discardPrefsDraft` 只回补已保存值。
4. **回归**（Flutter，可失败断言）：拖滑条 0 次舞台 sync；切分区 0 次 PATCH / sync；关设置放弃
   0 次写盘；保存 1 次 PATCH + 1 次 sync + 1 次刷新；保存失败草稿还在、舞台不变。
   真实浏览器按同一脚本复核。
5. 旧计划书（如 `PLAN-settings-reload-2026-10-10.md`）与本规则冲突处，标注以 CURRENT 为准，
   不逐页重写。

### T4 末步 `rc.(z+1)`：只清理本块

- 更新 CURRENT / README 版本线与发布说明（文件与末 tag 同名）；docs 预算 ≤45,000。
- `>500` 行文件不新增（当前 23）；不新增依赖；不新增端点。
- 跑全门禁：`cargo fmt/clippy/test/doc`、`flutter analyze/test`、`xtask code-stats --check`、
  `prune_web_artifacts.sh --check`。
- 一次本地提交 + 一个本地附注 tag；不 push。

## 3. 版本内顺序

T1 →（T1 后半与 T3 可并行）→ T2 → T4。实际 `z` 由执行 agent 按提交边界拆，不预先钉死。

## 4. 验收汇总

| 判据 | 证据 | 未验边界 |
| --- | --- | --- |
| 双向热换可重复 | 真实浏览器 10 次 / 方向，控制台无 panic | 无 Windows Chrome 时只能标「未验」 |
| 失败保留旧模型 | 坏 URL / 半失败注入 + 单测 | WebGPU 回滚路径肉眼看 |
| cue 不串模型 | 换模时旧 seq 不下发 + 单测 | — |
| 设置无副作用 | Flutter 计数断言 + 浏览器脚本 | 浮层 / 内联 / 整页三种宿主至少各一次 |
| 文档规则落地 | CURRENT + AGENTS 链接 | — |

## 5. 风险与回退

- panic 若在 pinned `ayagami` 内部，patch 成本与许可边界要先问维护者；不能把「升级依赖」当默认解。
- 取消即时预览会改变既有 UX（W7 的滑条手感），这是本块要求，不是回归；发布说明要写清。
- 浏览器验收依赖 Windows；当天不可用就如实写「未验」，**不得用单测冒充**。
- docs 只剩约 2,000 行余量：本计划、发布说明和规则文本都要短。

## 6. 完成定义

真实浏览器双向换模稳定；换模不把旧 cue 落到新模型；设置面离开 / 切分区无副作用，只有保存并重载
生效；规则写进文档；全门禁绿；本地 commit + tag；报告区分「已验 / 未验」。
