# 点火验收清单（产品级加强波次 / mod/product-grade）—— 给用户在 Windows 上照单验收

> **这是什么**：把「用户本人在这台机器的 Windows 浏览器里真实点火」做成**逐步可勾选**的清单。
> 每一项都写清 **操作 / 期望可见结果 / 失败时先看哪**，并明确标注
> **〔agent 已预检〕** 与 **〔必须人眼/Win〕**。
>
> - **本清单对应 tip**：`mod/product-grade`（基座 `72d1af17` + 五轨产品化）。
> - **本波不发布**：不 push、不打 tag、版本仍是 **`0.2.0-rc.3`**。
> - **口径**：主链皮肤（LLM/TTS/口型/Live2D、壳/舞台背景）与 `l2d-wasm-demo`
>   framebuffer 一行未改；**注册面 5 个 Mod**（external-input / persona / voice-input /
>   memory / director），**wallpaper 与 pet-desktop 已封存**（见
>   [`../architecture/ARCHIVED-mods.md`](../architecture/ARCHIVED-mods.md)）。
> - 上一版清单（Wave 3 / stabilize）：[`IGNITION-CHECKLIST-stabilize.md`](IGNITION-CHECKLIST-stabilize.md)
>   ——**已被本文件取代**，只留作历史。

---

## 0. 给最忙的你：五步开点火

```bash
# 0) 只在 WSL2 里跑（唯一进程宿主；Windows 只开浏览器）
cd /home/skystar/Live2D-Ai-product

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
| 唯一进程宿主 | **WSL2**（`Live2D-Ai-product` worktree） |
| 起点火命令 | `./scripts/ignite.sh`（首次/产物缺失：`./scripts/ignite.sh --build`） |
| 端口 | **18080**（`--port N` 可改；改了后面所有 URL 跟着改） |
| 入口 URL | `http://127.0.0.1:18080/app/`（`/` 会 302 到它） |
| 渲染面 | `http://127.0.0.1:18080/render`（`/app/` 的舞台 iframe 拉它） |
| Mod 状态面 | `http://127.0.0.1:18080/api/v1/mods` |

> **`/render` 与 `/app/` 是两份 gitignore 的产物**，新 worktree 一开始不存在。
> `ignite.sh` 会自动补：`/render` 缺 → `trunk build`；`/app/` 缺 → **只有 `--build` 才构建**。
> **第一次点火请用 `--build`。**

### 1.2 `.env` 与 `EXTERNAL_INPUT_TOKEN`

- **密钥真源 = `.env`**（不是 `live2d-ai.toml`）；`live2d-ai.toml` 只持变量**名**。
  本机需要 `DEEPSEEK_API_KEY`（LLM 用）；`ignite.sh` 会导入它。
- **`EXTERNAL_INPUT_TOKEN` 是可选的**：不设 = 仅本机不鉴权；设了 → 请求必须带
  `token` 字段或 `Authorization: Bearer`，否则 **401**。
  设在哪：`.env`（env 优先）或前端「Mod 管理 → external-input → token」。
  面板运行态会显示 **`令牌已设置/未设置`**（本波新增，**永不回显明文**）。
- **`LIVE2D_AI_ALLOW_NO_ORIGIN=1`**：只在你想用不带 Origin 的纯 curl 时需要；
  清单里给 curl 的一律显式带 loopback Origin，所以**默认不需要**。

### 1.3 TTS 没起时的预期（先读懂，否则会误判成「模型没返回」）

1. **文本确实进了主链**：`say` → LLM 真实请求 → 模型回复正文照常产生；
2. 轮次在 **TTS 阶段失败**，错误码 **`tts_transport`**（`stage=tts`、`fatal=true`），
   状态胶囊变 **出错**；
3. 回复**仍然上屏并落盘**，但带 **「未收尾」** 标记，且**不出声、不驱动口型**；
4. `turn_ended` 照常发出（**失败也发**）——所以 **director 仍会记账**（§3.10 可验）。

> **判定口径**：TTS 没起时「用户气泡 + 助手正文气泡（带未收尾）」= 文本已进主链通过。

---

## 2. 机器预检（〔agent 已跑〕——你只需复跑，不用手点）

```bash
# ① 官方点火体检（静态托管三件事 + CDN 红线）
./scripts/ignite.sh --check

# ② 完整预检表（PASS/FAIL/SKIP）；--fsm 追加五个注册 Mod 的启停矩阵
./scripts/ignition-precheck.sh --fsm

# ③ Rust 全套门禁
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio

# ④ 两个 sidecar 离线自检（不联网、不需要 blivedm/ASR）
python3 docs/examples/voice-sidecar/voice_sidecar.py --selftest
python3 docs/examples/bilibili-sidecar/bilibili_sidecar.py --selftest
```

### 2.1 关键 curl 的期望状态码表

设 `B=http://127.0.0.1:18080`、`O='Origin: http://127.0.0.1:18080'`、`J='Content-Type: application/json'`。

| # | 请求 | 期望 | 说明 |
| --- | --- | --- | --- |
| 1 | `GET /` | **302** `Location: /app/` | 唯一入口 |
| 2 | `GET /app/` | **200** | 壳产物 |
| 3 | `GET /render` | **200** | wasm 渲染面 |
| 4 | `GET /api/v1/mods` | **200** | **5 个 id**：director / external-input / memory / persona / voice-input |
| 5 | `POST /api/v1/external/chat` `-H "$O" -H "$J"` `-d '{"text":"hi"}'` | **200** `ok:true` | 忙时 **200 `ok:false` code=busy**（不是 5xx） |
| 6 | `GET /api/v1/mods/external-input/state` | **200** | `state` 必含 accepts/rejects/busy/ready/v2_ignored（本波还会有 token_set） |
| 7 | `POST /api/v1/mods/external-input/command` `-d '{"command":"reset_counters"}'` | **200** `ok:true` | 计数清零（本波新增命令通道） |
| 8 | 停用 external-input 后再打 #5 | **403** `mod_disabled` | 再 enable 复原 |
| 9 | `POST /api/v1/mods/memory/command` `-d '{"command":"clear"}'` | **200** `ok:true` | 清空记忆库（需先启用 memory） |
| 10 | `GET /api/v1/mods/persona/state` | **200** | 本波新增运行态面；未合入时应为 503 |
| 11 | `POST /api/v1/mods/does-not-exist/command` | **404** `not_found` | 不在注册表 |
| 12 | `POST /api/v1/mods/director/command` `-d '{"command":"nope"}'` | **409** `unsupported_command` | 不认识这条命令 |

> 带 token 时：#5 缺 token → **401 `unauthorized`**。
> 不带 Origin 且没开 `LIVE2D_AI_ALLOW_NO_ORIGIN` → **403 `origin_required`**。

---

## 3. 人机验收〔必须人眼/Win〕

> 每一步：**操作 → 期望可见结果 → 失败时先看哪**。逐项打勾。

### 3.0 打开页面
- [ ] **操作**：Windows 浏览器打开 `http://127.0.0.1:18080/app/`
- [ ] **期望**：壳出现（标题栏「Live2D Ai」、顶部状态胶囊、左侧 Live2D 舞台、右侧聊天输入框）
- [ ] **失败先看**：白屏 → `./scripts/ignite.sh --check`；「离线/未连接」→ 服务是否还在跑

### 3.1 舞台模型渲染
- [ ] **操作**：看左侧舞台
- [ ] **期望**：**模型可见**且**会呼吸/眨眼**（待机生命体征）
- [ ] **失败先看**：HUD 应显示 `GPU: webgpu/WebGPU`、`FPS` > 0、`bg: solid|image`

### 3.2 聊天框一轮说话（主链）
- [ ] **操作**：输入「用一句话打个招呼」→ 发送
- [ ] **期望（TTS 已起）**：用户气泡 → 助手气泡逐字流式 → **出声** → **嘴型随语音开合**
- [ ] **期望（TTS 未起）**：助手正文气泡出现且带 **「未收尾」**，状态胶囊 **出错**（见 §1.3）
- [ ] **失败先看**：一个字都没有 → 日志有无 `llm_upstream_401`；界面错误码拿去日志搜（同一个码）

### 3.3 ★ `external-input`（直播弹幕注入）〔本波加强〕
- [ ] **操作**：curl 注入一条：
  ```bash
  curl -s -X POST http://127.0.0.1:18080/api/v1/external/chat \
    -H 'Origin: http://127.0.0.1:18080' -H 'Content-Type: application/json' \
    -d '{"text":"直播间的第一条弹幕！"}'
  ```
- [ ] **期望**：`{"ok":true,...}` **且角色接话**
- [ ] **操作**：设置 → Mod → 展开 `external-input`
- [ ] **期望（计数可见）**：运行态里 **已接受 / 已拒绝 / 忙碌拒绝 / 礼物 v2 兜底 / 已就绪**
      都是中文可读；**令牌已设置/未设置** 如实显示（不回显明文）
- [ ] **操作**：点面板里的 **「重置计数」**
- [ ] **期望**：计数归零并刷新；再注入一条后又变成 1
- [ ] **操作**：停用 external-input（开关）→ 再注入
- [ ] **期望**：注入返回 **403 `mod_disabled`**；重新启用恢复
- [ ] **失败先看**：415 → 少了 `Content-Type`；403 `origin_required` → curl 没带 Origin；
      401 → 两边 token 不一致；计数不动 → 先看 `GET …/external-input/state`

### 3.4 `voice-input`（语音输入）〔本波加强〕
- [ ] **操作（离线自检）**：`python3 docs/examples/voice-sidecar/voice_sidecar.py --selftest`
- [ ] **期望**：全部通过（脚本会打印项数）
- [ ] **操作**：设置 → Mod → 展开 `voice-input`
- [ ] **期望（说人话）**：能读到这三句——
      `mock` = 没有 ASR，由外部喂转写文本；`sidecar` = 外部 ASR **推**文本到端点
      （Rust 侧从不开 socket）；`locale` 只影响文本归一化，不是识别语言开关。
      面板显示**当前生效值**（backend / locale）与**失败码→处置**短列表。
- [ ] **操作（真发送，先启用）**：
  ```bash
  curl -s -X POST http://127.0.0.1:18080/api/v1/mods/voice-input/enable -H 'Origin: http://127.0.0.1:18080'
  curl -s -X POST http://127.0.0.1:18080/api/v1/voice/transcript \
    -H 'Origin: http://127.0.0.1:18080' -H 'Content-Type: application/json' \
    -d '{"text":"  把\u3000窗户\u200b关小一点  "}'
  ```
- [ ] **期望**：`{"ok":true,"text":"把窗户关小一点",...}`（全角空格/零宽被清洗）+ 角色接话
- [ ] **失败先看**：403 `mod_disabled` → 先 enable；200 `ok:false busy` → 主链忙，退避重试
      失败码见 `docs/voice-input.md` 的「失败码 → 人话 → 处置」表

### 3.5 两个 sidecar 的 Windows 最小路径
- [ ] **操作**：照 `docs/examples/voice-sidecar/README.md` 的「Windows 最小成功路径」整段复制执行
- [ ] **期望**：`--selftest` → `--dry-run` → 真发送三步都照文档走通
- [ ] **操作**：`python3 docs/examples/bilibili-sidecar/bilibili_sidecar.py --selftest`
- [ ] **期望**：`selftest OK`；`--dry-run` 打印将 POST 的 URL + JSON（不发请求）
- [ ] **失败先看**：房间号是不是短号 / SESSDATA 过期；节流窗口内的弹幕只打印丢弃日志
      （`--min-interval-ms`，缺省 1000，`0` = 关——与 README 一致）

### 3.6 ★ `persona`（酒馆角色卡）〔本波加强〕
- [ ] **操作**：设置 → Mod → 展开 `persona`
- [ ] **期望**：有**导入角色卡**的入口（粘贴卡 JSON，或选文件）；界面写明
      「保存后在下面打开开关即生效；关闭开关会还原主链基线的 system_prompt」
- [ ] **操作**：导入一张**好卡** → 打开开关 → 聊一轮 → 关闭开关 → 再聊一轮
- [ ] **期望**：① 启用后人设生效（回复风格变化），
      ② `GET …/mods/persona/state` 显示卡名/来源，
      ③ **停用后还原**成基线人设（与启用前一致）
- [ ] **操作**：导入一张**坏卡**（非法 JSON）
- [ ] **期望**：**Failed**（不崩、不静默），面板给可处置提示；修好再启用成功
- [ ] **期望（与 memory 共存提示）**：面板明确写「persona 与 memory 都写 system_prompt，
      **后写覆盖（last-writer-wins）**，不合并、不仲裁」
- [ ] **失败先看**：日志 `mod: persona` 行；`state` 读不到 → 服务端是否已合入本波

### 3.7 ★ `memory`（记忆）〔本波加强〕
- [ ] **操作**：启用 memory；按 `docs/architecture/memory-mod-v0.md` 的**可重复步骤**：
      说「记住：我叫星梦，喜欢薄荷」→ 等这一轮结束 → 再问「我叫什么？喜欢什么？」
- [ ] **期望**：相关提问时记忆被提起（回复里出现 星梦/薄荷）；
      `GET …/mods/memory/state` 的 **条数 / 命中 / 注入** 计数增长
- [ ] **操作**：设置 → Mod → 展开 `memory`
- [ ] **期望（可见条数/hits/清空）**：运行态显示 **条数 / 命中次数 / 注入轮数 / 已淘汰 / 上轮命中**；
      面板有 **「清空记忆库」** 按钮
- [ ] **操作**：点「清空记忆库」→ 重新查看 state
- [ ] **期望**：**条数归零**；文件被原子重写（不是留半截）
- [ ] **操作**：把 `enabled_injection` 关掉（保存）→ 再问相关问题
- [ ] **期望**：**只记不注入**（回复不再带记忆内容）；`注入开关 ≠ Mod 启停` 在面板有说明
- [ ] **失败先看**：`条数=0` → 这轮没被判为可记忆；`hits=0` → 问法与写入词面不重叠

### 3.8 ★ `director`（导演决策面板）〔本波加强〕
- [ ] **操作**：启用 director；聊一轮；展开 `director`
- [ ] **期望（一等面板）**：面板把决策拆成**带标签的行**——本轮情绪 / 意图 / 建议语速 /
      建议音高 / 是否已结项，下面是最近若干条决策（带 turn id）；**不是一坨裸 JSON**
- [ ] **期望（持续有决策）**：每聊一轮，`GET …/mods/director/state` 的 `decisions` 增长，
      面板的「本轮决策」跟着更新
- [ ] **期望（零投递仍成立）**：面板顶部用文字写明「仅建议，不驱动动作、不影响 TTS 输出」；
      state 里 `delivered:false`、`channel:"none"`
- [ ] **期望（空态可读）**：未启用/还没聊过时给「先启用并聊一轮」的指引，不是空白
- [ ] **失败先看**：`decisions=0` → 这轮没走完 `turn_ended`（TTS 失败也会发，见 §1.3）；
      `delivered=true` → 违反零投递裁决，属**回归**

### 3.9 已封存的 Mod（`wallpaper` / `pet-desktop`）
- [ ] **操作**：`curl -s http://127.0.0.1:18080/api/v1/mods | python3 -m json.tool | grep id`
- [ ] **期望**：**只有 5 个 id**，**不出现** `wallpaper` / `pet-desktop`
- [ ] **操作**：设置 → Mod
- [ ] **期望**：Mod 管理里**没有** wallpaper / pet-desktop 卡片
- [ ] **期望（用户能力仍在）**：设置 →「外观与互动」里**舞台背景/壳背景的选图、清图、
      背景列表增删排序照常可用**（这是用户手动能力，与已封存的 wallpaper **Mod** 是两回事）
- [ ] **失败先看**：出现了这两个卡片 → 有人把工厂挂回去了（看
      `docs/architecture/ARCHIVED-mods.md` 的恢复条件）

---

## 4. 通过标准

### 4.1 必须全绿才算「真实点火通过」

| 类别 | 项 |
| --- | --- |
| 机器预检 | `ignite.sh --check` 全 ok；`ignition-precheck.sh --fsm` **FAIL 0** |
| 壳/舞台 | §3.0 打开、§3.1 **模型可见且会动** |
| 主链 | §3.2 **一轮对话正文上屏**（有声更好；无声但「未收尾」也算链通） |
| 封存 | §3.9 **注册面恰为 5 个**、无 wallpaper/pet 卡片、用户背景能力仍在 |
| 五 Mod | §3.3~§3.8 各自的**产品级可见项**至少各勾一次 |
| 回归 | 「Mod 管理」的**启用/停用能点动**；`mod_count_is_five` 不得变红 |

### 4.2 可选（不作为「通过」的必要条件）

- **真 TTS 出声 + 口型随动**：依赖本机 CosyVoice（`127.0.0.1:8080`）；
- **B 站正式跑**：需真实房间号 + `SESSDATA`；
- **真麦克风**：需自备 ASR CLI。

### 4.3 明确**不算**失败的情形

- TTS 没起 → 出错 + 「未收尾」，但**文本已进主链**（§1.3）；
- `persona / voice-input` 的 `state` 在**未合入本波**时返回 **503**（本波合入后 persona 应为 200）；
- 注入返回 **200 + `ok:false busy`**（主链忙，退避即可）；
- 除 `external-input` 外其余四个 Mod **缺省停用**（需要你手动启用）。

---

## 5. 签名栏（点完火请填这一格）

```
日期        ：
机器        ：Windows + WSL2（内核 `uname -sr`）
tip SHA     ：（`git -C /home/skystar/Live2D-Ai-product rev-parse --short HEAD`）
端口        ：
TTS 是否在跑：是 / 否
结果        ：通过 / 卡在第 __ 步
卡点描述    ：
日志片段    ：（错误码 + 服务端对应行）
```

---

## 6. 相关文件

- 封存台账：[`../architecture/ARCHIVED-mods.md`](../architecture/ARCHIVED-mods.md)
- 收束报告：[`PRODUCT-GRADE-CLOSEOUT.md`](PRODUCT-GRADE-CLOSEOUT.md)
- 契约：[`../external-input.md`](../external-input.md)、[`../voice-input.md`](../voice-input.md)、
  [`../architecture/memory-mod-v0.md`](../architecture/memory-mod-v0.md)、
  [`../architecture/persona-mod-v0.md`](../architecture/persona-mod-v0.md)、
  [`../architecture/director-mod-v0.md`](../architecture/director-mod-v0.md)
- 上一版清单（历史）：[`IGNITION-CHECKLIST-stabilize.md`](IGNITION-CHECKLIST-stabilize.md)
