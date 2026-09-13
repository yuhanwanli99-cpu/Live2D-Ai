# 交接：rc.2「第二基线」（2026-09-13）—— 动作层删到底 + 模型闭环 + `.env` 密钥真源 + 推理模型思考

> **本文是 rc.2 的接手入口。** 范围真源是
> [`PLAN-rc2-second-baseline-2026-09-12.md`](PLAN-rc2-second-baseline-2026-09-12.md)（§0 冻结范围），
> 发布说明是 [`../releases/v0.1.0-rc.2.md`](../releases/v0.1.0-rc.2.md)。
> 上一轮交接：`HANDOFF-2026-09-11-core-chain-baseline.md`（rc.1）。

---

## 0. 一分钟上手

```bash
cd ~/Live2D-Ai

# 起服务（WSL 是唯一进程宿主；Windows 只开浏览器）
./scripts/ignite.sh                 # 默认端口 18080；--build 会先重建 Rust + Flutter Web
./scripts/ignite.sh --check         # 另开一个终端：体检 / → 302 → /app/、/app/ 200、产物无 Google CDN

# 端到端验收（只走公开 HTTP/WS）——**改动核心链路后必跑**
python3 scripts/verify_core_chain.py            # 正常 18 跳全过
python3 scripts/verify_core_chain.py --timeout 300   # 长思考提问需要更长窗口（TTS 合成慢）

# 前端改动后必须重建（静态文件按请求读盘，不必重启后端）
cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn
```

打开 <http://127.0.0.1:18080/app/>（`/` 会 302 到 `/app/`）。
本机 TTS 是 CosyVoice3，需先在 `127.0.0.1:8080` 起着；`.env` 里已有 `DEEPSEEK_API_KEY`。

---

## 1. 这一轮做了什么（13 个提交）

顺序即**冻结的执行顺序**（§0）：`M0 → M1 → A1 → A2 → §5.4 → §8.3.1 → 发布`，
之后又按用户实测报的三个问题打了三个补丁。

| 提交 | 内容 |
|---|---|
| `5ab3ff8e` | 计划入库（步骤 1 唯一真源） |
| `d0a3b3db` | **M0 点火纪律**：`build_dir()` 补候选 + 503 列出找过的每个路径；`ignite.sh` 锚定 `LIVE2D_AI_FLUTTER_WEB_DIR` + 新增 `--check`；版本对齐；WSL↔Win 分工写进 AGENTS/README |
| `103bdbb8` | **M1 动作层删到底**：director Mod / `SupervisorHandle::trigger_action` + `action_rx`（**唯一**能把 `RootEvent::Action` 送进 core 的路径）/ `HostChannels.trigger_action` 全删；capabilities 去 5 个字段（schema→2）；`toml.example` 里教模型调已删工具的那行删掉 |
| `056f024b` | **M1.5 渲染面编舞删除**（单独提交便于整条回退）：`action-state` 接收器 + 关键帧表 + `param_scale.rs`，-770 行（`surface.rs` 1721 → 1169） |
| `076afb20` | **A1 模型根统一**：新增 `web_api/model_root.rs`（XDG 那条删除，`directories` 依赖随之删）；`find_model3_json` 向下看一层；`active_model_id` 读真实 registry；`requires_restart` 恒 `false` |
| `471874c4` | **A2 前端闭环**：激活后 `sendSync(model:)` 并**等渲染面 `loaded` 回执**才说「已切换」；补上一直缺的**导入入口** |
| `7b21437e` | **§5.4 `.env` = 唯一密钥真源**：`secrets.rs`（快照读，不用 `set_var`）+ `GET/PUT /api/v1/env`（永不回值、原子写、`0600`、就地改行、热重载）+ `.env` 进 `file_watcher` + 前端密钥输入 |
| `0f32a1ab` | **§8.3.1 旧 JS 预览隔离**到 `docs/design/legacy/*-oldjs.*` + README「勿当现网」 |
| `8507f5e6` | **发布**：版本三处同步（`Cargo.toml` / capabilities / `pubspec.yaml`）+ 发布说明 + CHANGELOG |
| `a0a77fd2` | **补丁①：推理模型的思考**（`reasoning_content` → 新帧 `reasoning_delta` → 折叠区）+ **输出上限 512 → 4096** |
| `d1064690` | **补丁②**：气泡语义 label 带「含思考 N 字」（修可达性缺口）+ 真实抓包帧回归 |
| `41ec24c2` | 门禁数字核准 |
| `218dba12` | **补丁③：TTS 自检不再说谎**（改轻量连通探针）+ 两处自检密钥读取走 `secrets` |

### 1.1 三个补丁都来自**用户实测**，不是自查

它们的共同形状是**「名实不符」**——本计划的主轴就是消灭这一类：

| 用户原话 | 真因 | 修法 |
|---|---|---|
| 「清空历史后第一次、之后也有几次，对话没有模型返回」 | 上游是**推理模型**，思考与正文**共用 `max_tokens`**：512 时正文被挤成半句（实测 `finish_reason=length`、只剩 25 字且断在 `\sqrt{a`）；而正文上屏走「该句**语音已合成完毕**」（`SentenceVoiced`）→ 半句切不出完整句 ⇒ **一个字都不上屏** | 解析思考单列事件 + 默认上限 4096 + 「只有思考没有正文」单独收口 + 前端折叠区 |
| 「思考展开也加上」 | `reasoning_content` **整条被丢弃**（`llm.rs` 只读 `content`） | 新帧 `reasoning_delta` + 气泡折叠区（默认折叠；只有思考时默认展开并说明原因） |
| 「TTS 连通性检查没通，但是可以正常播放声音」 | 自检原来**真合成一次**（本机 2.4s）而默认只等 3s；链路正在合成时点自检**稳定报 timeout**；HTTP 循环是**单线程**的，同步探针会占住整个 API 它等待的全长 | 自检改打 `GET {tts.base_url}/models`（1–2 ms、不加载模型、不抢上游）；404 算可达 + `note`；超时文案明确「这不等于不能出声」 |

---

## 2. 门禁与数字（最后一次实跑）

| 门禁 | 结果 |
|---|---|
| `cargo test --workspace --all-targets` | **795 passed / 0 failed**（18 套件） |
| `cargo test --doc --workspace` | 3 passed |
| `cargo fmt --all -- --check` | 干净 |
| `cargo clippy --workspace --all-targets -- -D warnings` | **0 warning** |
| `cargo run -p xtask -- rust-ratio` | **96.9754%**（54154 / 55843）**PASS**（门槛 95%） |
| `cargo check --target wasm32-unknown-unknown -p l2d-wasm-demo` | 0 warning |
| `trunk build`（`crates/l2d-wasm-demo`） | ✅ |
| `cd shell/flutter && flutter analyze` | No issues found |
| `cd shell/flutter && flutter test` | **813 passed** |
| `python3 scripts/verify_core_chain.py --timeout 300` | **18 跳全过** |
| `./scripts/ignite.sh --check` | 四项全 `[ok]` |

> 删除会**拉低** `rust-ratio`（分子分母一起减）：本轮删了约 1175 行 Rust，占比从
> 96.9552% 到 96.9754%（中间加回了一些）。按 95% 门槛仍有约 2.1 万行余量。

---

## 3. 交付态实测（本机 + 无头浏览器 + 用户肉眼）

### 3.1 WSL 侧（可复跑）

- `GET /api/v1/app/capabilities` → `version=0.1.0-rc.2`、`schema_version=2`、**无** actions/upload/script
- `POST /api/v1/models/import {"id":"bai"}` → 200，`model3_json_path=bai/runtime/bai.model3.json`（深一层布局认得）
- `POST /api/v1/models/bai/activate` → `requires_restart=false`、`model_url` **可 GET 200**
- `GET /api/v1/app/status` → `active_model_id=bai`（读真实 registry）
- `PUT /api/v1/env`（幂等回写）→ 响应**不含值**、`.env` 行数与注释不变、权限仍 `0600`，日志随即出现「热重载成功」
- TTS 自检：空闲 **1–2 ms**；**链路正在合成时 2/110/3 ms 全 `ok:true`**（修前是 `timeout`）
- 长思考轮的**帧时间线**（`/ws/state` 实采，25 句）：**每句 `start`/`end` 逐一配对**、
  `turn_state=completed`、末帧 `text_delta{completed:true}` 收口。
  ⇒ 也解释了为什么 `verify_core_chain.py` 用默认 `--timeout 90` 跑长提问会红两条：
  **25 句的合成超过了 90s 窗口**，是探针超时而不是链路问题——长提问请带 `--timeout 300`

### 3.2 无头浏览器（Windows Chrome 152 + CDP 9222，七项证据法）

- `flt-glass-pane` 在、视口 1280×860、iframe `/render` canvas **939×808** 非 0
- 本次加载**外部源请求 0**（断网红线）
- **音频真在播**：`blob:true` / `paused:false` / **`duration` 是实数**（3.08s、7.8s）/ `currentTime` 推进；同刻**至多一个元素在播**（一句一单元）
- **思考到了渲染层**：语义 label 出现「助手说：**63**（含思考 **78** 字）」
- **折叠开关真能点**：DOM 量高度 **98 → 190**（展开）→ **98**（收起）
- **复现并确认修好**：把 `max_tokens` 临时压到 64（热重载）→ 正文为空、思考 145 字 →
  label「助手说：（含思考 145 字）」，且**不再出现**「本轮模型没有返回文字」

### 3.3 用户肉眼（Windows 浏览器）

对话 / 出声 / 口型 / 待机体征 / 模型库导入-激活-换皮 / 密钥写入立即生效 / 思考折叠区 /
TTS 自检 —— 全部**通过**。

---

## 4. 接手第一件事

1. 读本文件 + `PLAN-rc2-second-baseline-2026-09-12.md` §0（冻结范围）+ `releases/v0.1.0-rc.2.md`；
2. `git log --oneline 65e62115..v0.1.0-rc.2` 看这 13 个提交的实际改动；
3. 跑一遍门禁（§2 那张表，逐条）；
4. 起服务 + `--check` + Windows 打开 `/app/` 确认现在就是好的；
5. 再从 §6 里挑下一轮的第一项。

---

## 5. 已确认、**故意没做**的事（都推到 rc.3）

| 项 | 为什么没做 / 建议 |
|---|---|
| `main.dart` God Object（1180 行） | rc.2 只做**接线**；拆分要动组合根，归 rc.3（计划 §5.3） |
| `surface.rs` 仍 1169 行（>1000 豁免线） | 已删掉最大一块（编舞）；继续拆要分「渲染 / 输入 / 待机」，且**wasm 门控、原生测试零覆盖** —— 必须单独立项 + 肉眼验收 |
| egui 设置面 / `--chat` / `--pet-mode` | 仍是第二壳，未 feature-gate；会动 Cargo feature 矩阵与 `cli/tests.rs` 断言 |
| CI 与本地门禁不对齐 | `pr-checks.yml` / `nightly.yml` 仍只跑 `cargo test --workspace`（无 `--all-targets`/`--doc`/`rust-ratio`/flutter job）→ M4 |
| **正文上屏依赖 `SentenceVoiced`** | **刻意**（文字与声音同拍）。代价：TTS 故障那一轮**一个字都不上屏**。rc.2 已修掉「截断」这条主因；「TTS 故障时也把已生成的正文交出去（并标明未收尾）」要动那条同拍契约，**需要单独论证** |
| 思考折叠开关对读屏不可达 | 它在气泡的 `excludeSemantics: true` 子树里；label 已带「含思考 N 字」，但开关本身要拆语义子树才能报出来 |
| 思考不落盘 | 比正文长 5–20 倍，写进 localStorage 会膨胀一个数量级；要持久化得同时给存档加体积上限 |
| `status.audio` 在 web 下恒 `none` | 未修（不属本轮范围） |
| 自检走**同步**探针 | 仍占单线程 HTTP 循环（现在只有几毫秒，可忽略）；真要长探针应改成「202 + 轮询」，那是接口形状变化 |

---

## 6. 下一轮（rc.3）建议顺序

1. **正文兜底**：TTS 故障/未成句时，把已生成的正文交给 UI 并标明「未收尾」（先论证同拍契约怎么让位）；
2. **结构性减法**：`main.dart` ≤700（admin / settings wiring / 模型库接线挪出组合根）、`surface.rs` 拆「渲染 / 输入 / 待机」；
3. **egui 双壳裁决**：feature-gate，或文档标明非主线；`--chat` / `--pet-mode` 同办；
4. **M4 门禁对齐**：CI = 本地一套真相（含 `--all-targets`/`--doc`/`rust-ratio`/flutter job/`verify_core_chain.py`）；
5. **M5 可读性收尾**：`docs/design` 收束为「一份现行规格 + 历史存档」；根目录作废文档迁 `docs/legacy/`；
6. 顺带：删空指针分支 `origin/mainline/1-core-baseline`（== `main`）。

---

## 7. 必须知道的坑（**本轮新踩的**，前一轮的七条仍在旧交接里）

### 7.1 语义树不是像素（我犯过一次，判错了一个 feature）

气泡上有 `Semantics(excludeSemantics: true)`，整条气泡会被折成**一个**节点、label 由
`message.text` 拼。于是新加的「思考」区在语义 DOM 里**完全不可见**——我据此判定
「前端没渲染」，还建了诊断版去查，最后发现数据早就到了渲染层。

**教训**：DOM/语义树是**代理**，不是画面。要判「画没画出来」，要么截图，要么把待验事实
写进 label 让它可被自动检查（本轮就是这么做的：label 带「含思考 N 字」）。
`console.error` 同理（渲染面把正常进度也走 error）。

### 7.2 自检与产品链路抢资源 → 自检说谎

自检端点在**单线程 HTTP 循环**里同步打上游。若它做的是**重活**（真合成一次），
就会出现「链路繁忙时自检报失败、而链路明明正常」。写自检的口径：
**它只能测它自称测的那件事，且不得与产品路径抢关键资源**。

### 7.3 推理模型：思考与正文共用 `max_tokens`

实测 `deepseek-flash`：一句「你好」思考 97 字；几步推理的提问思考 **1200–1600 字**。
512 上限时正文被挤空/挤半句，而**半句切不出完整句 ⇒ 一个字都不上屏**。
默认上限因此是 **4096**；改它之前先重跑 `settings_tests` 里那段实测记录。

### 7.4 「正文上屏」的闸门是 `SentenceVoiced`，不是 LLM 增量

`EngineEvent::TextDelta` **只做控制台回显**（2026-09-10 的决定，为了文字不跑到声音前面）；
推给 UI/WS 的文字由「该句语音已合成完毕」承载。**任何让某句凑不齐的原因，都会让整轮没有文字**。

### 7.5 别用 `taskkill /IM chrome.exe` 收无头浏览器

按镜像名杀会**连用户自己的 Chrome 一起关掉**（我干过一次）。Windows 侧收尾要按**命令行
特征精确匹配**：

```bash
powershell.exe -NoProfile -Command "Get-CimInstance Win32_Process -Filter \"Name='chrome.exe'\" \
  | Where-Object { \$_.CommandLine -like '*l2d-cdp*' } | ForEach-Object { Stop-Process -Id \$_.ProcessId -Force }"
```

### 7.6 抓包帧的形状要按**真实抓包**钉，别凭印象

`reasoning_delta` 的真实帧是 `{"data":{"epoch":0,"text":"我们需要","ts_ms":630},"seq":371,"ts":"…","type":"reasoning_delta"}`
（`seq`/`ts` 在**根**上）。已并入 `test/ws_frame_test.dart` 常驻回归。

### 7.7 前端改动必须重建才生效

`build/web` 是按请求读盘，所以**不必重启后端**，但**必须** `flutter build web`。
改完 UI 先重建再让人刷新——否则你会对着旧产物调试。产物里非 ASCII 会被 dart2js 转义成
`\uXXXX`，**用明文 grep 会得到假阴性**（查 `reasoning_delta` 这类 ASCII 才可靠）。

---

## 8. 归档与恢复位置

| 归档 | 内容 |
|---|---|
| tag `v0.1.0-rc.2` | 本轮全部成果（含三个补丁） |
| 分支 `archive/action-layer-p6` | M1 之前的状态（动作层完整存在）；`git show archive/action-layer-p6:crates/live2d-ai-mod-director/src/lib.rs` 可回看被删的 director |
| `archive/action-trigger-p5` / `py-legacy` / `android-archive` | 更早的归档（远端 ref 已删，只在维护者本地） |
| `docs/design/legacy/` | 旧 JS 前端预览（改名 `-oldjs`，标注「勿当现网」） |

**M1 与 M1.5 是分开的两个提交**：万一舞台渲染出问题，单独回退 `056f024b` 即可。

---

## 9. 提交方式（本轮的口径）

- 每个里程碑一个提交，删除类改动**先有归档点**；
- 提交信息写清「**删除了什么**」「**为什么**」，以及门禁数字；
- **发布顺序**：先本地提交 + tag → Windows 肉眼点火通过 → 再
  `git push origin main && git push origin v0.1.0-rc.2`（**不许先推、后发现 `/app/` 503**）。

### 9.1 推送记录（2026-09-13，**已完成**）

```
65e62115..6ab4a074  main -> main
 * [new tag]        v0.1.0-rc.2 -> v0.1.0-rc.2
```

`git ls-remote` 与本地逐一对应：`main` = `6ab4a074`、tag 对象 = `82bd78c7`。

**两个坑（下次直接照做）**：

1. **GitHub 仓库已改名**：`yuhanwanli99-cpu/Live2Dai` → **`Live2D-Ai`**。GitHub 会重定向旧名
   （push 会成功，但回一句 `remote: This repository moved…`）；本地 `origin` 已更正为规范地址。
   仓库里若再见到旧名，按旧名处理即可（`ANDROID_ARCHIVE_POINTER.md` 那处已修）。
2. **凭据在 Windows 侧，而 WSL 不继承 Windows 用户级环境变量**：`echo $GITHUB_TOKEN` 在 bash 里
   是**空**的。要读得**直接读注册表**（与当前进程环境无关）：

   ```bash
   powershell.exe -NoProfile -Command '[Environment]::GetEnvironmentVariable("GITHUB_TOKEN","User")' \
     | tr -d '\r\n' > ~/.git-token && chmod 600 ~/.git-token
   git -c 'credential.helper=!f(){ echo username=yuhanwanli99-cpu; echo "password=$(cat "$HOME/.git-token")"; };f' \
       push origin main v0.1.0-rc.2
   shred -u ~/.git-token        # 用完即毁；token 本体留在 Windows 用户变量里
   ```

   token 是**细粒度 PAT**，只给了该仓库 **Contents: Read and write**（+ 强制的 Metadata: Read-only）——
   够推提交与 tag；**没给** Workflows（本轮不含 `.github/workflows/**` 改动；哪天要改 CI，
   push 会被拒，那时才需要加这一项）。
