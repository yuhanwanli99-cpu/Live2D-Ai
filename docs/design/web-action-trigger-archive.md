# 动作手动触发：本轮接线存档与再接入前置条件

日期：2026-09-11
状态：**已从成品移除**（实现保留在 git 历史与分支 `archive/action-trigger-p5`）

## 1. 一句话

这一轮（v0.4.13 → v0.5.0）**曾经把「手动触发动作」完整接通并测过**，
随后按用户裁决**主动缩减边界**：触发入口移出成品，代码只留历史/分支。
本文记下「为什么撤、撤掉了什么、再接入前必须先做什么、协议上踩过哪些坑」——
避免下次重新踩一遍。

## 2. 撤掉的原因（用户原话的意思）

1. **边界**：本轮目标只有核心链路（LLM → TTS → 口型 → 舞台 + UI）。
   动作/mod/导演这些系统**没有被要求**，做了也不算达标。
2. **观感**：触发区的排版僵硬、同质（`3 列等宽卡片 + 每张一个图标 + 一行字`），
   与「去 AI 味」的目标相反。
3. **顺序**：**动作本身要先做好**。动作质量决定触发入口值不值得存在——
   六个动作都还是通用关键帧的情况下，给它们配一个精致的触发面板是本末倒置。
4. **归属**：AI 主动触发/编排属于**导演系统**，应走 Mod 边界单独开一轮，
   不占核心链路。

## 3. 撤掉了什么（精确清单）

删除（`git log --diff-filter=D`）：`lib/actions/action_dispatch.dart`、
`lib/ui/action_grid.dart`、`test/action_dispatch_test.dart`、`test/action_ui_p5_test.dart`。

收窄（保留类型、删掉写入路径与 UI 调用点）：

| 位置 | 处置 |
| --- | --- |
| `api/commands_api.dart` | 删 `invoke()` 与 `CommandInvokeResult`；**保留** `fetchCommands()`（动作日志要靠它把协议名翻成中文名） |
| `app/app_shell.dart` | 删 `stageToolbar` 槽位（左下常驻工具条）；保留 `stageCorner`（缩放三键） |
| `settings/sections/actions_section.dart` | 删「动作试演」B 组；保留 A 组（舞台与口型）与 C 组（动作记录） |
| `main.dart` | 删 `_invokeAction` / `_toolbarCommands` / `_strength` / `ActionDispatch`；`cease` 缺动作名所需的游标退化为一个 `String? _performingAction` |

**保留下来的下行通道没有被削弱**：`action_state` → 渲染面 `action-state`
这条「让模型真的动」的链路（P5 之前完全缺失的那一环）仍然完整。
撤掉的只是**上行的手动触发**。

## 4. 再接入前必须做的事（按顺序）

1. **先把动作做好**：动作的观感、节奏、强度分档要经得起看。
   当前六个动作（nod / shake_no / tilt / look_around / listen / surprise）
   是渲染面 `choreography_total_ms` 的关键帧编排，尚未按动作质量评审过。
2. **重做排版**：不要再做等宽卡片网格。参考下一节的 py 做法。
3. **明确归属**：手动触发留在核心 UI，AI 主动触发/编排走导演 Mod
   （`live2d-ai-mod-director`），不要把两者混在一个面板里。
4. **协议坑**（第 6 节）直接照抄结论，不要重新试。

## 5. 排版参考：Python 时代（tag `py-legacy`）的做法

代码位置：`Live2D-Ai-pc/open-llm-vtuber/renderer/src/dev-console/action-registry.ts`

值得照搬的四点：

1. **每个动作带中文名 + 一句人话说明**，而不是只有英文 id 或只有两个字：
   `nod: { name: '点头', description: '头部上下轻点两下' }`，
   `look_around: { name: '左右张望', description: '视线先行+头部随动的环顾' }`。
   ——「点头」两个字不足以让用户预期会发生什么。
2. **列表/行式排布**（名称 + 说明 + 触发），不是等宽卡片网格。
   中文说明长短不一，网格会逼着文案去迁就格子。
3. **`allowed_sources` 与能力门控**：只允许 LLM 触发的动作不进用户面板
   （点下去会被 core 拒绝 = 「点了没反应」）；模型缺对应参数的动作
   （如缺 `Param4` 的 `pout`）由后端能力过滤后自然不出现。
4. **删除动作有硬名单**：已删除的动作即使后端目录仍下发，前端也一律拒绝注册
   （`isUiAllowedAction`）——避免「后端残留 → 前端静默多出按钮」。

## 6. 协议坑（实测结论，直接照抄）

### 6.1 `POST /api/v1/commands/{id}/invoke`

真实响应形状：

```json
{"accepted":true,"epoch":0,"command_id":"live2d_perform_action_nod",
 "kind":"single","steps":1,"action":"nod"}
```

- 不给强度时 body 里**不出现** `strength`（空 JSON `{}`），服务端用自己默认档。
- `strength` 越界是 **400 `invalid_params`**（实测 `9` → 400）。
- **重复点击也回 `accepted: true`**：服务端对「动作+强度+来源完全相同」返回
  `Dropped(AlreadyActive)` 幂等，但**响应里分辨不出来**（实测两次响应一字不差）。
  ⇒ 「同一动作正在播放」的提示**只能本地做**（当时用 2 s 窗口，
  比多数编排短，宁漏报不误报）。这条仍然成立。

### 6.2 渲染面 `action-state` 的两个反直觉点

- **`action` 从信封根读**，`state` / `strength` 从 `payload` 读
  （`l2d-wasm-demo/src/main.rs:380`）。
- **只认 `start` / `end`**，没有 `stop`。写 `stop` 会被**静默忽略**。

### 6.3 服务端目前**永远不发 `cease`**

`ActionCommand::Release` 在全仓库**没有任何调用点**（桌面包与 director Mod 里
只有 `Play`），而 `cease` 帧由 `ActionEffect::End` 驱动。
实测：invoke 之后监听 30 秒，只收到一条 `perform`。

⇒ 如果「正在演」只等 `cease`，会**永久卡住**。当时前端用本地兜底过期，
时长**抄渲染面自己的表**（不是猜的）：`l2d-wasm-demo/src/web/surface.rs`
的 `choreography_total_ms` —— nod 1320 / shake_no 1000 / tilt 1500 /
look_around 1920 / listen 2180 / surprise 1300，未知动作 1500，另加 250 ms 余量。
**真收到 `cease` 时以 `cease` 为准。**

（这条兜底表随 `action_dispatch.dart` 一起归档了；重做时按上面的出处重新取值。）

## 7. 归档在哪

| 内容 | 位置 |
| --- | --- |
| 完整实现（含 46 条测试） | 分支 `archive/action-trigger-p5` |
| 引入提交 | `48cd48de`（P5）、`d6a75607`（P2 动作来源可见）、`f17484bb`（接线修复） |
| 移除提交 | 见本文件同批次的 `refactor(shell): 边界收窄` |
| 触发区的原始设计规格 | `docs/design/web-ui-spec-v3.md`（该文件仍是权威规格，相关条目已标注作废） |

取回方式：

```bash
git show archive/action-trigger-p5:shell/flutter/lib/actions/action_dispatch.dart
git show archive/action-trigger-p5:shell/flutter/lib/ui/action_grid.dart
```
