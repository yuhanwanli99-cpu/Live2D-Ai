> 历史（2026-10-01 归档，勿当现网）。

# 点火验收清单（stabilize）—— 给用户在 Windows 上照单验收

> # ⛔ 已被取代（2026-09-14，产品级加强波次）
>
> 本清单是 **Wave 3 / stabilize** 那一版的点火清单，留作历史。
> **请改用** [`IGNITION-CHECKLIST-product-grade.md`](IGNITION-CHECKLIST-product-grade.md)：
> 它对应 tip `mod/product-grade`，注册面已从 **7 收到 5**
>（`wallpaper` / `pet-desktop` 封存，见
> [`../../architecture/ARCHIVED-mods.md`](../../architecture/ARCHIVED-mods.md)），
> 并加强了五个 Mod 的人眼验收项。下面所有「七个 Mod」的表述**都是这段历史的**。
>
> **另有一处行为变更**：`persona` 在本波新增了 `state_json`，`GET /api/v1/mods/persona/state`
> 由 **503 升为 200**（见 [`../../architecture/persona-mod-v0.md`](../../architecture/persona-mod-v0.md)）；
> 下文 §2.2 第 15 行与 `STABILIZE-PRECHECK-RESULT.md` 里的 503 记录都是**当时的**。

> **这是什么**：把「用户本人在这台机器的 Windows 浏览器里真实点火」做成**逐步可勾选**的清单。
> 每一项都写清 **操作 / 期望可见结果 / 失败时先看哪**，并明确标注
> **〔agent 已预检〕**（机器可跑的，本轮已跑，见
> [`STABILIZE-PRECHECK-RESULT.md`](STABILIZE-PRECHECK-RESULT.md)）与
> **〔必须人眼/Win〕**（脚本代替不了）。
>
> - **真源**：`/home/skystar/Live2D-Ai-wave3` @ `57e32ae5`
> - **本清单对应 tip**：`mod/stabilize`（稳定化代码提交 `0d33aa5b` + 本文档提交）
> - **本波不发布**：不 push、不打 tag、版本仍是 **`0.2.0-rc.3`**。
> - **口径**：主链皮肤（LLM/TTS/口型/Live2D、壳/舞台背景）与 `l2d-wasm-demo`
>   framebuffer / stage_bg 渲染算法**一行未改**；七个 Mod 全注册，缺省只启用
>   `external-input`。

---

## 0. 给最忙的你：五步开点火

```bash
# 0) 只在 WSL2 里跑（唯一进程宿主；Windows 只开浏览器）
cd /home/skystar/Live2D-Ai-stabilize

# 1) 起服务（默认 18080；首次或产物缺失时用 --build）
./scripts/ignite.sh --build

# 2) 另一个 WSL2 终端：机器预检（应全绿）
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
| 唯一进程宿主 | **WSL2**（`Live2D-Ai-stabilize` worktree） |
| 起点火命令 | `./scripts/ignite.sh`（首次/产物缺失：`./scripts/ignite.sh --build`） |
| 端口 | **18080**（`--port N` 可改；改了后面所有 URL 跟着改） |
| 入口 URL | `http://127.0.0.1:18080/app/`（`/` 会 302 到它） |
| 渲染面 | `http://127.0.0.1:18080/render`（`/app/` 的舞台 iframe 拉它） |
| Mod 状态面 | `http://127.0.0.1:18080/api/v1/mods` |

> **`/render` 与 `/app/` 是两份**gitignore** 的产物**，新 worktree / 新 clone 里
> 一开始不存在。`ignite.sh` 会自动补：`/render` 缺 → `trunk build`；
> `/app/` 缺 → **只有 `--build` 才构建**（否则打印警告）。
> 本轮实测：fresh worktree 直接起二进制时 `GET /render` = **503**，跑一次
> `trunk build` 后 200。**所以第一次点火请用 `--build`。**

### 1.2 `.env` 与 `EXTERNAL_INPUT_TOKEN`

- **密钥真源 = `.env`**（不是 `live2d-ai.toml`）。`live2d-ai.toml` 只持变量**名**
  （`api_key_env`）。本机需要 `DEEPSEEK_API_KEY`（LLM 用）。
  `ignite.sh` 会导入它；Rust 侧也会自己读 `.env`。
- 检查：`ls -la .env`（存在）+ `grep -oE '^[A-Z_]+=' .env`（只回显变量名，不回显值）。
- **`EXTERNAL_INPUT_TOKEN` 是可选的**（弹幕注入端点的令牌）：
  - **不设 = 不鉴权**（端点仍只监听 127.0.0.1）——本机自用可以留空；
  - 设了 → 请求必须带 `token` 字段或 `Authorization: Bearer`，否则 **401**。
  - 设在哪：`.env`（env 优先）或前端「Mod 管理 → external-input → token」。
- **`LIVE2D_AI_ALLOW_NO_ORIGIN=1`**：只在你想用**不带 Origin 的纯 curl** 时需要。
  浏览器永远带 Origin；本清单里给 curl 的一律**显式带 loopback Origin**，
  所以**默认不需要**开这个开关。

### 1.3 TTS 没起时的预期（**先把这条读懂，否则会误判成「模型没返回」**）

本机 TTS（CosyVoice）默认在 `127.0.0.1:8080`。**它没起时**：

1. **文本确实进了主链**：`say` → LLM 真实请求 → **模型回复正文照常产生**；
2. 轮次在 **TTS 阶段失败**，错误码 **`tts_transport`**（`stage=tts`、`fatal=true`），
   UI 状态胶囊变 **出错**，并给出「去语音合成设置」按钮；
3. 该轮回复**仍然上屏并落盘**，但带 **「未收尾」** 标记（rc.3 的
   `text_fallback` 契约），且**不出声、不驱动口型**；
4. `mod: external-input 收到事件 turn_ended` 照常发出（**失败也发**）——
   所以 director 仍会记账。

> **判定口径**：TTS 没起时，「用户气泡 + 助手正文气泡（带未收尾）」= **文本已进主链通过**；
> 只有「想听声 / 想看口型」才算失败。**不要**因为没有声音就判链路不通。
>
> 本轮 agent 已在无头浏览器里实测到这条：见
> [`STABILIZE-PRECHECK-RESULT.md`](STABILIZE-PRECHECK-RESULT.md) §3。

### 1.4 浏览器

- Windows 打开 **`http://127.0.0.1:18080/app/`**（就这一个入口；`/` 自动跳转）。
- 建议 **Chrome / Edge**（舞台渲染面走 WebGPU；太老的浏览器会回退失败）。

---

## 2. 机器预检（〔agent 已跑〕——你只需复跑，不用手点）

### 2.1 四条命令

```bash
# ① 官方点火体检（静态托管三件事 + CDN 红线）
./scripts/ignite.sh --check

# ② 本波新增：完整预检表（PASS/FAIL/SKIP）；--fsm 追加七 Mod 启停矩阵
./scripts/ignition-precheck.sh --fsm

# ③ Rust 全套门禁
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio

# ④ 两个 sidecar 离线自检（不联网、不需要 blivedm/ASR）
python3 docs/examples/voice-sidecar/voice_sidecar.py --selftest       # 期望：全部通过（70 项）
python3 docs/examples/bilibili-sidecar/bilibili_sidecar.py --selftest # 期望：OK（16 项断言）
```

### 2.2 关键 curl 的**期望状态码**表（照抄即用）

设 `B=http://127.0.0.1:18080`、`O='Origin: http://127.0.0.1:18080'`、`J='Content-Type: application/json'`。

| # | 请求 | 期望 | 说明 |
| --- | --- | --- | --- |
| 1 | `GET /` | **302** `Location: /app/` | 唯一入口 |
| 2 | `GET /app/` | **200** | 壳产物 |
| 3 | `GET /render` | **200** | wasm 渲染面；503 = dist 缺失（跑 `trunk build`） |
| 4 | `GET /api/v1/mods` | **200** | 7 个 id：director / external-input / memory / persona / pet-desktop / voice-input / wallpaper |
| 5 | `POST /api/v1/external/chat` `-H "$O" -H "$J"` `-d '{"text":"hi"}'` | **200** `ok:true` | 正常注入；忙时 **200 `ok:false` code=busy**（**不是 5xx**） |
| 6 | 同上但 `Content-Type: text/plain` | **415** | 非 JSON CT |
| 7 | 同上但 `{"text":"   "}` | **400** `invalid_payload` | 空文本 |
| 8 | 同上但 `{"text":"x"*2001}` | **400** `text_too_long` | 渲染后长度上限 2000 |
| 9 | `GET /api/v1/external/chat` | **405** | 只支持 POST |
| 10 | `POST …/mods/external-input/disable` `-H "$O"` | **200** | **无 body 无 CT 也必须 200**（本波修的 bug） |
| 11 | 停用后再打 #5 | **403** `mod_disabled` | 明确拒绝，不静默吞 |
| 12 | `POST …/mods/external-input/enable` `-H "$O"` | **200** | 复原 |
| 13 | `GET …/mods/external-input/state` | **200** | `state` 键 = accepts / busy / rejects / ready / v2_ignored |
| 14 | `GET …/mods/does-not-exist/state` | **404** `not_found` | 不在注册表 |
| 15 | `GET …/mods/persona/state` | **503** `state_unavailable` | **persona 没实现 `state_json`，503 是正常的** |
| 16 | `POST …/mods/voice-input/enable` `-H "$O"` 后 `POST /api/v1/voice/transcript` | **200** `ok:true` | 会回清洗+归一化后的 `text` |
| 17 | voice-input 停用时打 transcript | **403** `mod_disabled` | 缺省停用 |

> 带 token 时：#5 缺 token → **401 `unauthorized`**。
> 不带 Origin 且没开 `LIVE2D_AI_ALLOW_NO_ORIGIN` → **403 `origin_required`**。

### 2.3 本轮预检结论（agent 已跑，摘要）

**`PASS 34 / FAIL 0 / SKIP 1`**（SKIP = 未配置 `EXTERNAL_INPUT_TOKEN` 故 token 项跳过）；
`ignite.sh --check` **4/4 ok**。完整表见
[`STABILIZE-PRECHECK-RESULT.md`](STABILIZE-PRECHECK-RESULT.md)。

---

## 3. 人机验收〔必须人眼/Win〕

> 每一步：**操作 → 期望可见结果 → 失败时先看哪**。逐项打勾。
> 建议每步做完再进下一步；出错先别动配置，先看「先看哪」。

### 3.0 打开页面
- [ ] **操作**：Windows 浏览器打开 `http://127.0.0.1:18080/app/`
- [ ] **期望**：壳出现（标题栏「Live2D Ai」、顶部状态胶囊 **空闲**、
      「实时通道：实时通道正常」）；左侧 Live2D 舞台；右侧聊天输入框「说点什么…」
- [ ] **失败先看**：白屏 → `./scripts/ignite.sh --check`（看 `/app/` 是否 200、
      `main.dart.js` 是否含 `gstatic`）；「离线/未连接」→ 服务是否还在跑

### 3.1 舞台模型渲染
- [ ] **操作**：看左侧舞台
- [ ] **期望**：**模型可见**（不是黑屏/白屏/空白），并且**会呼吸/眨眼**（待机生命体征）
- [ ] **失败先看**：HUD（舞台左上）应显示 `GPU: webgpu/WebGPU`、`FPS` > 0、
      `bg: solid|image`；模型 404 → `assets/models/bai/runtime/bai.model3.json` 是否存在；
      `/render` 503 → dist 缺失，跑 `cd crates/l2d-wasm-demo && trunk build`

### 3.2 聊天框一轮说话
- [ ] **操作**：输入「用一句话打个招呼」→ 发送
- [ ] **期望（TTS 已起）**：用户气泡 → 助手气泡**逐字流式**出现 → **出声** →
      **嘴型随语音开合**；刷新后助手气泡仍在
- [ ] **期望（TTS 未起）**：用户气泡 → 助手正文气泡出现，带 **「未收尾」** 标记；
      状态胶囊 **出错** + 「去语音合成设置」；**这条也算「文本已进主链」通过**
- [ ] **失败先看**：气泡一个字都没有 → 看服务端日志有无 `llm_upstream_401`
      （`.env` 里的 key 没被读到）；界面报错码 → 拿码去日志里搜（两处同一个码）

### 3.3 `external-input`（直播弹幕注入）
- [ ] **操作（有 Origin 的 curl，推荐）**：
  ```bash
  curl -s -X POST http://127.0.0.1:18080/api/v1/external/chat \
    -H 'Origin: http://127.0.0.1:18080' -H 'Content-Type: application/json' \
    -d '{"text":"直播间的第一条弹幕！"}'
  ```
- [ ] **期望**：`{"ok":true,...}` **且角色接话**（浏览器里出现用户气泡 + 助手回复）
- [ ] **操作（停用门禁）**：`curl -s -X POST http://127.0.0.1:18080/api/v1/mods/external-input/disable -H 'Origin: http://127.0.0.1:18080'`
      之后再打一次上面的注入
- [ ] **期望**：注入返回 **403 `{"error":{"code":"mod_disabled",...}}`**；重新 enable 恢复
- [ ] **失败先看**：415 → 少了 `Content-Type`；403 `origin_required` → curl 没带 Origin
      且服务端没开 `LIVE2D_AI_ALLOW_NO_ORIGIN`；401 → 两边 token 不一致

### 3.4 B 站 sidecar（`docs/examples/bilibili-sidecar/`）
- [ ] **操作（离线自检，先做）**：`python3 docs/examples/bilibili-sidecar/bilibili_sidecar.py --selftest`
- [ ] **期望**：`selftest OK（16 项断言）`，**不需要** blivedm/网络
- [ ] **操作（dry-run）**：`BILI_ROOM_ID=123456 python3 docs/examples/bilibili-sidecar/bilibili_sidecar.py --dry-run`
- [ ] **期望**：打印将要 POST 的 URL + JSON，**不发请求**
- [ ] **操作（可选，正式，需真实房间号/SESSDATA）**：装依赖 + 设 `BILI_ROOM_ID`（**长号**）、
      `BILI_SESSDATA`，运行脚本
- [ ] **期望**：弹幕进来时角色接话；节流窗口内的弹幕只打印丢弃日志
- [ ] **失败先看**：一直没弹幕 → 房间号是不是短号 / SESSDATA 过期；
      礼物 v2 无反应 → 本地 blivedm 版本（旧版会记 `v2_ignored` 兜底）

### 3.5 `voice-input`（语音输入）
- [ ] **操作（离线）**：`python3 docs/examples/voice-sidecar/voice_sidecar.py --selftest`
- [ ] **期望**：`全部通过（70 项检查）`
- [ ] **操作（dry-run）**：
      `python3 docs/examples/voice-sidecar/voice_sidecar.py --audio docs/examples/voice-sidecar/fixtures/fake_zh.wav --dry-run`
- [ ] **期望**：打印 `POST …/api/v1/voice/transcript` 与 `{"text":"把窗户关小一点"}`，不发请求
- [ ] **操作（真发送）**：先启用 Mod，再发 fixture：
  ```bash
  curl -s -X POST http://127.0.0.1:18080/api/v1/mods/voice-input/enable -H 'Origin: http://127.0.0.1:18080'
  curl -s -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
    -H 'Origin: http://127.0.0.1:18080' -H 'Content-Type: application/json' \
    -d '{"text":"  把\u3000窗户\u200b关小一点  "}'
  ```
- [ ] **期望**：`{"ok":true,"text":"把窗户关小一点",...}`（**全角空格/零宽被清洗**）+ 角色接话
- [ ] **失败先看**：403 `mod_disabled` → 先 enable；200 但 `ok:false busy` → 主链正忙，退避重试即可
- [ ] **操作（可选：真麦克风）**：录一段 wav，配 `--transcriber 'cmd:"<你的 ASR CLI>"'` 再喂进去

### 3.6 `wallpaper`（壁纸）
- [ ] **操作**：前端「设置 → Mod → wallpaper」启用；把 `interval` 设成小值（如 5s）或打开 `follow_stage`
- [ ] **期望**：舞台/壳背景**可见变化**；或从 `GET /api/v1/mods/wallpaper/state` 看到
      `mode` / `decision` / `prefs_patch` 变化（`mode=off` 时是 `reason:"mode_off"`）
- [ ] **失败先看**：`GET …/mods/wallpaper/state`（200 才有运行态）；列表是不是空的（`playlist_len=0`）

### 3.7 `memory`（记忆）
- [ ] **操作**：启用 memory；聊几句让它写入（如「记住：我叫星梦」）；再问相关问题
- [ ] **期望**：相关提问时记忆被注入；`GET …/mods/memory/state` 的
      `writes / hits / injects / injected` 计数增长；停用后不再注入
- [ ] **失败先看**：`state` 里 `writes=0` → 这轮没被判为可记忆；`hits=0` → 问法与写入词面不重叠

### 3.8 `persona`（酒馆角色卡）
- [ ] **操作**：启用 persona，导入一张**好卡**（V1/V2 JSON 或带 `chara` 的 PNG）
- [ ] **期望**：enable 成功，人设生效（回复风格变化）
- [ ] **操作**：导入一张**坏卡**（非法 JSON）
- [ ] **期望**：**Failed**（不崩、不静默）；修好再 enable 成功
- [ ] **失败先看**：`GET …/mods/persona/state` **503 是正常的**（persona 无运行态面）；
      看服务端日志的 `mod: persona` 行

### 3.9 `pet-desktop`（桌宠，软闭环）
- [ ] **操作**：启用 pet-desktop；改「总在最前 / 点击穿透 / 不透明度」→ 保存
- [ ] **期望**：Mod 管理里的**运行态字段跟着变**（`always_on_top` / `click_through` / `opacity`）；
      `window` 恒为 `{"opened":false,"reason":"native_shell_dormant"}`
- [ ] **失败先看**：保存后字段没变 → 配置是**整份替换**（不是深合并），只发改动字段会把别的回落缺省
- [ ] **说明**：**不要求真的出现桌宠窗口**——原生壳休眠是既定裁决，本项只验状态面软闭环

### 3.10 `director`（导演，只观察）
- [ ] **操作**：启用 director；聊一轮；再 `GET http://127.0.0.1:18080/api/v1/mods/director/state`
- [ ] **期望**：`state.decisions` ≥ 1，`recent_decisions` 里每条含
      `{emotion,intent,suggested_tts,delivered:false}`；`channel:"none"`、`delivered:false`
- [ ] **期望（负向）**：**没有任何动作乱触发**（导演零投递：不影响输出、不驱动动作）
- [ ] **失败先看**：`decisions=0` → 这轮没走完 `turn_ended`；`delivered=true` → 违反零投递裁决，属回归

---

## 4. 通过标准

### 4.1 必须全绿才算「真实点火通过」

| 类别 | 项 |
| --- | --- |
| 机器预检 | `ignite.sh --check` 4/4；`ignition-precheck.sh --fsm` **FAIL 0** |
| 壳/舞台 | §3.0 打开、§3.1 **模型可见且会动** |
| 主链 | §3.2 **一轮对话正文上屏**（有声更好；无声但「未收尾」也算链通） |
| 注入 | §3.3 注入成功 + 停用后 **403 mod_disabled** |
| Mod 面 | §3.7~§3.10 的 **state 面**至少各看过一次且符合上表 |
| 回归 | 前端「Mod 管理」的**启用/停用能点动**（本波修的 415 不得复现） |

### 4.2 可选（不作为「通过」的必要条件）

- **真 TTS 出声 + 口型随动**：依赖本机 CosyVoice（`127.0.0.1:8080`）；
- **B 站正式跑**：需真实房间号 + `SESSDATA`；
- **真麦克风**：需自备 ASR CLI；
- **壁纸真实换图观感**、**桌宠真窗口**（后者是既定休眠，本来就不做）。

### 4.3 明确**不算**失败的情形

- TTS 没起 → 出错 + 「未收尾」，但**文本已进主链**（§1.3）；
- `persona / voice-input` 的 `state` 返回 **503**（未实现 `state_json`，契约如此）；
- 注入返回 **200 + `ok:false busy`**（主链忙，退避即可）；
- `external-input` 之外的 Mod **缺省停用**（需要你手动启用）。

---

## 5. 签名栏（点完火请填这一格）

```
日期        ：
机器        ：Windows + WSL2（内核 `uname -sr`）
tip SHA     ：（`git -C /home/skystar/Live2D-Ai-stabilize rev-parse --short HEAD`）
端口        ：
TTS 是否在跑：是 / 否（否 → §3.2 走「未收尾」口径）
结果        ：通过 / 卡在第 __ 步
卡点描述    ：
日志片段    ：（错误码 + 服务端对应行）
```

---

## 6. 相关文件

- 预检结果（本轮实跑）：[`STABILIZE-PRECHECK-RESULT.md`](STABILIZE-PRECHECK-RESULT.md)
- 本波收束：[parallel-mods/`STABILIZE-CLOSEOUT.md`](../../plans/parallel-mods/STABILIZE-CLOSEOUT.md)
- 契约：[`../../external-input.md`](../../external-input.md)、[`../../voice-input.md`](../../voice-input.md)
- 七 Mod 账本（Wave 3）：[parallel-mods/`WAVE3-CLOSEOUT-2026-09-14.md`](../../plans/parallel-mods/WAVE3-CLOSEOUT-2026-09-14.md)
