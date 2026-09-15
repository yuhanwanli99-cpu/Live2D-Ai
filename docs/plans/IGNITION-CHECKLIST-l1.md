# 点火验收清单（L1 产品级 / mod/l1-product）—— 给用户在 Windows 上照单验收

> **这是什么**：把「L1 = 壳内点开能走完主路径、聊天可感知」做成**逐步可勾选**的清单。
> 每一项写清 **操作 / 期望可见结果 / 失败时先看哪**，并标注
> **〔agent 已预检〕** 与 **〔必须人眼/Win〕**。
>
> - **本清单对应 tip**：`mod/l1-product`（worktree `/home/skystar/Live2D-Ai-l1`）。
> - **本波不发布**：不 push、不打 tag、版本仍是 **`0.2.0-rc.3`**。
> - **范围真源**：[PRODUCT-L1-GOALS-2026-09-15.md](PRODUCT-L1-GOALS-2026-09-15.md)。
> - **口径**：L1 ≠ 「测试绿 + 有面板」。**enable 之后主聊天必须可感知**；
>   需重载时有**重启提示**；disable / 清空**可预期**。
> - **全局新增**：任何 Mod 变更（启停 / 关键配置 / 导入卡 / 导入或清空记忆）
>   之后，前端给**统一的**「需重新点火 / 重启后生效」提示（聊天区顶部常驻 + 一条
>   SnackBar，两处都带 `./scripts/ignite.sh` 入口）。
> - **命名勘误**：口语里的「isonyou」是「is on you（发给你看）」的记录，**不是产品名**；
>   该能力的 id / 面板标题 / 文档标题一律是 **`external-input` / 外部事件接入**。
> - 上一版清单（产品级加强波次）：[IGNITION-CHECKLIST-product-grade.md](IGNITION-CHECKLIST-product-grade.md)
>   ——**已被本文件取代**，只留作历史。

---

## 0. 给最忙的你：五步开点火

```bash
# 0) 只在 WSL2 里跑（唯一进程宿主；Windows 只开浏览器）
cd /home/skystar/Live2D-Ai-l1

# 1) 起服务（默认 18080；首次或产物缺失时用 --build）
./scripts/ignite.sh --build

# 2) 另一个 WSL2 终端：机器预检（应 FAIL 0）
./scripts/ignition-precheck.sh --fsm

# 3) Windows 浏览器打开（只此一个入口）
#    http://127.0.0.1:18080/app/

# 4) 照 §3 逐项勾选（人眼项）
```

---

## 1. 前置（先确认这些，否则后面全是假失败）

### 1.1 WSL2 起服务

| 项 | 值 |
| --- | --- |
| 唯一进程宿主 | **WSL2**（本波 worktree `/home/skystar/Live2D-Ai-l1`） |
| 起点火命令 | `./scripts/ignite.sh`（首次/产物缺失：`./scripts/ignite.sh --build`） |
| 端口 | **18080**（`--port N` 可改；改了后面所有 URL 跟着改） |
| 入口 URL | `http://127.0.0.1:18080/app/`（`/` 会 302 到它） |
| Mod 状态面 | `http://127.0.0.1:18080/api/v1/mods` |
| 会话状态面 | `http://127.0.0.1:18080/api/v1/chat/session`（L1 新增） |

> **模型资产不入库**（`assets/models/*` 被 gitignore）：新 worktree 的
> `assets/models/` 可能是空的，表现为舞台「模型加载失败」。点火前放进去：
>
> ```bash
> cp -r /home/skystar/Live2D-Ai-product/assets/models/bai /home/skystar/Live2D-Ai-l1/assets/models/
> ```
>
> **符号链接不行**：静态服务 canonicalize 后会拒绝落在模型根之外的路径（实测仍 404）。

### 1.2 TTS 没起时的预期（别把它当 L1 失败）

见上一版清单 §1.3：TTS 未起时正文仍会上屏（带「未收尾」），状态胶囊显示出错。
**L1 判的是「点开能走完主路径」，不要求本机 TTS 一定可用。**

---

## 2. 机器预检〔agent 已预检〕

```bash
./scripts/ignition-precheck.sh --fsm
```

- [ ] **期望**：汇总行 **FAIL 0**
- [ ] 关键新断言（本波新增）：
  - [ ] `GET /api/v1/chat/session` → **200**
  - [ ] `POST /api/v1/chat/session {"session_id":"…"}` → **200**，再 GET 能读回同一个 id
  - [ ] `POST /api/v1/voice/transcript` 在**总闸未配**时 → **403 `voice_gate_closed`**
  - [ ] 配上唤醒短语后 → **200**，且返回的 `text` **已剥掉唤醒短语**
- [ ] **失败先看**：脚本每项都打印「期望 / 实得」；先看服务是否在跑（§1.1）

---

## 3. 人机验收〔必须人眼/Win〕

### 3.0 打开页面
- [ ] **操作**：Windows 浏览器打开 `http://127.0.0.1:18080/app/`
- [ ] **期望**：壳出现；左侧舞台；右侧聊天输入框
- [ ] **失败先看**：白屏 → `./scripts/ignite.sh --check`

### 3.1 舞台模型
- [ ] **操作**：看舞台
- [ ] **期望**：模型可见且会呼吸/眨眼；HUD 有 `GPU:`、`FPS`、`bg:`

### 3.2 聊天一轮（主链）
- [ ] **操作**：输入「用一句话打个招呼」→ 发送
- [ ] **期望**：用户气泡 → 助手气泡逐字流式 →（TTS 起）出声 + 口型
- [ ] **失败先看**：一个字都没有 → 日志 `llm_upstream_401`；界面错误码拿去日志搜

### 3.3 ★ 会话绑定（L1 全局基座）〔本轮新增〕
- [ ] **操作**：AppBar 的「会话」→ 新建会话 A → 发一句话；再新建会话 B → 发一句话
- [ ] **期望**：两个会话各自有独立记录；切换会话时列表内容随之切换
- [ ] **操作**：在 A 里导入一张角色卡（见 §3.6），切到 B 聊一句，再切回 A 聊一句
- [ ] **期望**：A 的人设生效、**B 不受影响**（不串卡）；切回 A 人设**仍在**
- [ ] **期望（降级语义）**：persona / memory 面板在**还没有任何会话**时明确写
      「会落到全局桶（与所有会话共享）」，不假装已经绑好
- [ ] **失败先看**：`GET /api/v1/chat/session` 的 `active_session` 是否跟着切会话变；
      浏览器 Network 里 `POST /api/v1/chat` 的 body 是否带 `session_id`

### 3.4 ★ Mod 变更 → 统一重启提示〔本轮新增〕
- [ ] **操作**：设置 → Mod → 打开任意 Mod 的开关
- [ ] **期望**：① 弹出一条 SnackBar，含「需重新点火 / 重启后生效」与
      `./scripts/ignite.sh`；② 聊天区顶部与 Mod 分区顶部各留下一条**常驻**提示（可关闭）
- [ ] **操作**：改一个 Mod 的关键配置并保存；导入一张角色卡；导入/清空记忆
- [ ] **期望**：同样出现统一提示（文案同源，不是各处各写一句）
- [ ] **失败先看**：提示出现但内容不含入口 → 属回归（见 `shell/flutter/lib/ui/restart_notice.dart`）

### 3.5 ★ `external-input`（外部事件接入）〔本轮推到 L1〕
- [ ] **操作**：设置 → Mod → 展开 `external-input`
- [ ] **期望（命名）**：卡片/面板主标题是 **`external-input` / 外部事件接入**
      （**不得**出现任何别的品牌名）
- [ ] **操作**：在面板的「测试注入」输入框写一句话 → 点「测试注入」
- [ ] **期望**：面板显示「已注入：<渲染后的文本>」；**聊天区角色接话**；
      运行态里 **已接受** 计数 +1
- [ ] **操作**：点「重置计数」→ 再注入一条
- [ ] **期望**：计数清零并重新变成 1
- [ ] **操作**：**关闭** external-input 开关 → 再点「测试注入」
- [ ] **期望**：**可读失败**——面板显示带 code 的中文（未启用 → 503
      `command_unavailable`；HTTP 端点是 **403 `mod_disabled`**，面板里有明确说明）
- [ ] **期望（真 HTTP 等价）**：面板给出 curl；照它注入仍进主链：
  ```bash
  curl -s -X POST http://127.0.0.1:18080/api/v1/external/chat \
    -H 'Origin: http://127.0.0.1:18080' -H 'Content-Type: application/json' \
    -d '{"text":"直播间的第一条弹幕！"}'
  ```
- [ ] **期望（sidecar 定位）**：面板/文档写明 B 站 sidecar 是**增强**，不是唯一主路径
- [ ] **失败先看**：415 → 少了 Content-Type；403 `origin_required` → 没带 Origin；
      401 → token 不一致；计数不动 → 先看 `GET …/external-input/state`

### 3.6 ★ `memory`（被动 + 主动面板）〔本轮推到 L1〕
- [ ] **操作**：启用 memory；说「记住：我叫星梦，喜欢薄荷」→ 等这一轮结束 →
      再问「我叫什么？喜欢什么？」
- [ ] **期望（被动，注入只对下一轮生效）**：提问轮 `GET …/memory/state` 的
      **命中/注入**增长；回复里出现 星梦/薄荷 通常要**再下一句**
- [ ] **操作**：设置 → Mod → 展开 `memory` → 点「刷新」
- [ ] **期望（主动线 = 面板，不靠聊天口令）**：能看到**记忆列表**（文本 + id）；
      每条有「编辑」「删除」；顶部有**导入一条**输入框；底部有「清空记忆库」
- [ ] **操作**：用面板「导入一条」写「我喜欢薄荷」→ 再聊一句相关的话
- [ ] **期望**：后面的对话**用得上**这条（命中/注入计数增长）
- [ ] **操作**：编辑一条、删除一条、清空
- [ ] **期望**：列表与计数按操作变化；清空后条数归零（文件被原子重写）
- [ ] **期望（会话分桶）**：面板显示当前会话桶；A/B 会话的记忆互不串
      （无会话时写明落到全局桶 `memory.jsonl`）
- [ ] **失败先看**：条数=0 → 这轮没被判为可记忆；hits=0 → 问法与写入词面不重叠

### 3.7 ★ `persona`（角色卡，会话绑定）〔本轮推到 L1〕
- [ ] **操作**：设置 → Mod → 展开 `persona` → 粘贴一张卡 JSON → 点「导入并生效」
- [ ] **期望**：面板显示「已绑定到会话 <id>」；**聊天人设变**（回复风格变化）；
      `GET …/mods/persona/state` 的 `sessions` 里出现该会话
- [ ] **操作**：切到另一个会话聊一句
- [ ] **期望**：**不串卡**（那个会话仍是全局/基线人设）
- [ ] **操作**：切回原会话 → 关闭 persona 开关 → 再聊一句
- [ ] **期望**：① 切回原会话人设**仍在**；② **停用后还原**（回到基线人设）
- [ ] **操作**：导入一张**坏卡**（非法 JSON）
- [ ] **期望**：**可读失败**（不崩、不静默），提示可处置
- [ ] **失败先看**：日志 `mod: persona`；面板是否在 `activeSessionId == null` 时
      提示「先发一条消息或用『导入为全局人设』」

### 3.8 ★ `voice-input`（唤醒闸 + 手动闸 + 硬主路径）〔本轮推到 L1〕
- [ ] **操作**：设置 → Mod → 展开 `voice-input`
- [ ] **期望（总闸）**：面板顶部显示「能力总闸」状态；未配唤醒短语时**明确写**
      「总闸未开：所有转写都会被拒绝（403 `voice_gate_closed`）」
- [ ] **操作**：不配总闸，直接点「验证闸门（手动注入一条转写）」
- [ ] **期望**：**可读拒绝**（403 `voice_gate_closed` 或 `command_unavailable`），
      不是静默成功
- [ ] **操作**：在配置区填**唤醒短语**（例如「小梦」）并保存；把「手动闸」打开
- [ ] **期望**：面板总闸状态变为「已开」
- [ ] **操作（硬主路径）**：在「用官方 sidecar 识别音频文件」里填
      `docs/examples/voice-sidecar/fixtures/fake_zh.wav` → 点按钮
      （**不需要任何额外环境变量**：官方 sidecar 会按目标 URL 自动带同源 loopback
      `Origin` 头，标准 `./scripts/ignite.sh` 点火即可）
- [ ] **期望**：① 面板显示「已拉起 sidecar（pid …）」；
      ② 稍等后**聊天区角色接话**（sidecar 把 `把窗户关小一点` POST 回主链）；
      ③ 面板 `sidecar_status` 显示 `exited` + 退出码 0
- [ ] **操作**：把「手动闸」关掉 → 再点「验证闸门」
- [ ] **期望**：**403 `voice_manual_off`**，可读
- [ ] **操作**：唤醒短语**不匹配**的文本（例如「天气不错」）→ 点「验证闸门」
- [ ] **期望**：**400 `wake_phrase_required`**，可读
- [ ] **期望（剥短语）**：匹配时进主链的正文**不含唤醒短语**（返回的 `text` 已剥离）
- [ ] **失败先看**：403 `mod_disabled` → 先 enable；`404` 脚本路径 → 配
      `sidecar_script`；`python3` 找不到 → 配 `sidecar_python`

### 3.9 `director`（本轮**不做** L1）
- [ ] **操作**：启用 director；展开面板
- [ ] **期望**：面板顶部**明写**「本轮不做 L1（用户裁决：director 下轮）」，
      并且仍写清「仅建议，不驱动动作、不影响 TTS 输出」（`delivered:false`、
      `channel:"none"`）
- [ ] **期望**：`GET …/mods/director/state` 含 `experimental: true` 与
      `l1_scope: "next-round"`
- [ ] **不要**把 director 面板当作本轮 L1 的完成证据

### 3.10 两个 sidecar 的 Windows 最小路径
- [ ] **操作**：照 `docs/examples/voice-sidecar/README.md` 的「Windows 最小成功路径」执行
- [ ] **期望**：`--selftest` → `--dry-run` → 真发送三步都走通
- [ ] **操作**：`python3 docs/examples/bilibili-sidecar/bilibili_sidecar.py --selftest`
- [ ] **期望**：`selftest OK`

### 3.11 已封存的 Mod
- [ ] **操作**：`curl -s http://127.0.0.1:18080/api/v1/mods | python3 -m json.tool | grep id`
- [ ] **期望**：**只有 5 个 id**（director / external-input / memory / persona / voice-input），
      **没有** wallpaper / pet-desktop
- [ ] **期望**：「外观与互动」里的舞台/壳背景选图、清图、列表增删**照常可用**

---

## 4. 通过标准

### 4.1 必须全绿才算「L1 通过」

| 类别 | 项 |
| --- | --- |
| 机器预检 | `ignite.sh --check` 全 ok；`ignition-precheck.sh --fsm` **FAIL 0** |
| 壳/舞台 | §3.0 打开、§3.1 模型可见且会动 |
| 主链 | §3.2 一轮对话正文上屏 |
| 全局基座 | §3.3 会话切换可用且不串；§3.4 Mod 变更后有统一重启提示（带入口） |
| external-input | §3.5 **测试注入按钮把一句话送进聊天** + 计数增长 + 停用可读失败 |
| memory | §3.6 **面板导入一条 → 后续对话用得上** + 可编辑/删除/清空 |
| persona | §3.7 **会话 A/B 不串卡** + 停用还原 |
| voice-input | §3.8 **总闸关 → 拒绝且可读**；**一条硬主路径把音频送进主链** |
| 封存 | §3.11 注册面恰为 5 个、无 wallpaper/pet 卡片 |
| director | §3.9 明确标注「本轮不做 L1」——**不作为通过项** |
| 回归 | `mod_count_is_five` 不得变红；版本仍是 `0.2.0-rc.3` |

### 4.2 可选（不计入通过）
- 真 ASR（`--transcriber cmd:"…"`）代替 fixtures 的 `fake`
- B 站真实弹幕（房间号 + SESSDATA）
- 本机 TTS 出声 + 口型

### 4.3 不算失败
- TTS 未起导致的上屏降级（见 §1.2）
- 主链忙时的 200 `ok:false busy`（退避重试即可）
- 未装 Python 时 sidecar 拉不起来（改用等价的命令行注入，见 `docs/voice-input.md`）

---

## 5. 签名栏

| 项 | 值 |
| --- | --- |
| 验收人 | |
| 日期 | |
| 服务 tip | `mod/l1-product` / `git rev-parse HEAD` = |
| 端口 | |
| 结论 | |

---

## 6. 相关文件

- 范围真源：[PRODUCT-L1-GOALS-2026-09-15.md](PRODUCT-L1-GOALS-2026-09-15.md)
- 收束报告：[L1-CLOSEOUT.md](L1-CLOSEOUT.md)
- 会话基座：[session-scope-l1.md](../architecture/session-scope-l1.md)
- 契约：`docs/external-input.md` / `docs/voice-input.md` /
  `docs/architecture/{memory-mod-v0,persona-mod-v0,director-mod-v0}.md`
- Mod 产品链路：[mod-product-chain.md](../architecture/mod-product-chain.md)
- 已封存 Mod：[ARCHIVED-mods.md](../architecture/ARCHIVED-mods.md)
