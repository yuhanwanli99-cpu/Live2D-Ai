# 桌宠窗口 Mod v1（`live2d-ai-mod-pet-desktop`）

> 范围真源：[`PARALLEL-WAVE3-2026-09-14.md`](../plans/parallel-mods/PARALLEL-WAVE3-2026-09-14.md) §3 轨 D
>（Wave 2 起点：[`PARALLEL-WAVE2-2026-09-14.md`](../plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md) §3E）。
> 交付清单：[`REGISTER-pet-desktop-v1.md`](../plans/parallel-mods/REGISTER-pet-desktop-v1.md)。
> 代码：`crates/live2d-ai-mod-pet-desktop/src/lib.rs` + `tests/pet_desktop_state.rs`；
> Flutter 消费面：`shell/flutter/lib/api/mods_api.dart`（`ModsApi.state`）+
> `shell/flutter/lib/settings/sections/dev_tools_section.dart`（`_ModConfigTile`）+
> `shell/flutter/test/pet_desktop_state_test.dart`。
> 基线：`mod/w3-pet` @ `118bd435`（`0.2.0-rc.3` + Wave 3 基座）。

## 1. 定位

本 Mod 管的是**桌宠窗口**这件事：总在最前 / 点击穿透 / 不透明度，以及
「语音是否活跃」这个口型联动信号。

Wave 2 的交付**不是**窗口本身，而是把这批配置与事件态**暴露到可测面**：
`GET /api/v1/mods/pet-desktop/state`（基座 Wave 2 新增）能读到一份稳定的 JSON，
`POST /api/v1/mods/pet-desktop/config` 改配置后它**立刻**变。在 Wave 2 之前，
这个 crate 只在 `start` 里注册 schema 并翻转一个内存 bool——界面上什么都看不见。

**Wave 3（2026-09-14，轨 D）**：把状态面接到 Flutter —— 产品标签**「仅状态面」**，
闭环形态是**软闭环**：`/app/` → 设置 → **Mod** → 展开「桌宠窗口」→「运行态（只读）」
块（`ModsApi.state(id)` → `GET /api/v1/mods/pet-desktop/state`）；改配置 → 保存 →
**自动重取**，字段跟着变。**仍然不设窗口**：硬开窗口要唤醒休眠原生壳，
违反 Wave 3 裁决——所以闭环的终点是**数据**，不是像素。

**选项钉死（不得自行升级）**：本轮**不真开窗口**。开窗口属于「休眠原生壳」，
唤醒它要先论证「谁来维护第二个 UI 壳」（§2）。

## 2. 范围声明：窗口未开（**这是刻意的**）

`state_json()` **恒定**返回：

```json
"window": { "opened": false, "reason": "native_shell_dormant" }
```

为什么不是 `true`：桌宠悬浮窗（`always_on_top` / `click_through` / `opacity` 的落点）
是**原生能力**，实现在 `live2d-ai-desktop` 的 **egui 原生壳**
（`src/app/` + `src/tray.rs` + `src/platform.rs`）。而那份原生壳自 rc.3 起被裁决为
**休眠保留：非主线、不验收**——唯一产品链路是 **`--web` + Flutter Web `/app/`**
（`./scripts/ignite.sh` 点火）。

真源（**不允许第三种含糊表述**）：

- [`core-chain-baseline.md`](core-chain-baseline.md) **§3.6「原生第二壳（egui / `--chat`）：非主线」**；
- `AGENTS.md`「原生第二壳的归属（休眠台账，2026-09-13 rc.3 定）」；
- 唤醒条件：**单独立项 + 先论证「谁来维护第二个 UI 壳」**。

边界表：

| 对象 | 状态 | 与本 Mod 的关系 |
| --- | --- | --- |
| `--web` + Flutter `/app/` | **主线** | 桌宠窗口 UI 在里面**不存在**；本 Mod 只经 `GET /api/v1/mods/pet-desktop/state` 出数据 |
| egui 原生壳（窗口 / 托盘 / 桌宠穿透） | 休眠保留、不验收 | **不接线、不驱动**；`window.opened` 因此恒 `false` |
| `--chat` 终端壳 | 休眠保留、不验收 | 无关 |
| `winit` / `egui` / `wgpu` | **禁止引入**（Wave 2 红线） | 本 crate 只依赖 `live2d-ai-mod-system` + `serde_json` |

因此本 Mod 的验收面是**数据**（配置 + 事件态），**不是像素**：
没有任何一条验收要求「屏幕上出现一个桌宠」。

## 3. 配置契约（`settings_spec` v1）

`PetDesktopFactory::settings_spec()` 是**静态**的（未启用也拿得到，rc.4 M2 语义），
`start` 注册的是**同一个函数**的返回值——单测
`static_settings_spec_equals_start_registered_spec` 逐字段钉住「两处不分叉」。
（旧骨架只在 `start` 里注册，于是「开关还关着」时前端拿不到表单。）

| key | kind | 语义 | 缺省 | schema 边界 |
| --- | --- | --- | --- | --- |
| `always_on_top` | bool | 窗口总在最前 | `true` | — |
| `click_through` | bool | 鼠标点击穿透 | `false` | — |
| `opacity` | number | 窗口不透明度 | `0.95` | `min = 0.1`，`max = 1.0` |

schema 里**没有**第二个 `enabled`：启停唯一真源是 Mod manifest 的 `enabled`
（与 external-input / voice-input / wallpaper 同口径，见
[`mod-product-chain.md`](mod-product-chain.md) §4）。pet-desktop **缺省停用**：
`web_api::cli_entry::default_mods_manifest()` 只启用 `external-input`，所以要看状态面
得先 `enable`（§5 步骤 2）。

**宽容口径**（配置写错不打死行为，逐个字段独立回落）：

- `always_on_top` / `click_through` 缺失 / 非 bool → 各自缺省；
- `opacity` 缺失 / 非数字（含字符串、null）→ `0.95`；
- `opacity` 是数字但越界 → 钳进 `0.1..=1.0`；非有限值（NaN / ±∞）→ 回落 `0.95`；
- **永不**返回 `0`（0 会让窗口不可见且看起来像「Mod 坏了」）。

## 4. 状态面契约（`ModRuntime::state_json`）

形状（`GET /api/v1/mods/pet-desktop/state` 的 `state` 字段，逐字段）：

```json
{
  "always_on_top": true,
  "click_through": false,
  "opacity": 0.95,
  "voice_active": false,
  "window": { "opened": false, "reason": "native_shell_dormant" }
}
```

| 字段 | 来源 | 变化时机 |
| --- | --- | --- |
| `always_on_top` / `click_through` / `opacity` | **namespaced Mod config JSON**（`create(services, config)` 注入，与 `POST …/config` 写的是同一份） | 改配置 → host `reload_config` + `restart` → 下一个实例立刻带新值 |
| `voice_active` | `VoiceStarted` = `true` / `VoiceEnded` = `false`；`shutdown` 复位 | 主链每次开口/收声 |
| `window.opened` | **常量 `false`**（§2） | 不由任何输入改变 |
| `window.reason` | 常量 `"native_shell_dormant"`（`WINDOW_REASON_NATIVE_SHELL_DORMANT`） | 同上 |

契约（与 `live2d-ai-mod-system` 的 `ModRuntime::state_json` 头注一致）：

- **只读**：实现只读内存字段，连续两次调用结果相同，不改 config / 事件态
  （回归 `state_json_is_a_read_only_snapshot`）；
- **不写盘**：本 crate 在 `state_json` 里没有任何 IO（写盘只发生在
  `POST …/config` 的 host 侧）；
- **不阻塞**：无锁等待、无网络、无 sleep。host 用 `try_lock` 取 runtime 锁，
  取不到直接回 `503 state_unavailable`——**绝不让 HTTP 线程等 Mod worker**；
- **脱敏**：本 Mod 没有 secret 字段，快照天然可外发；
- **`window.reason` 是稳定字符串**：前端据此区分「窗口关着」与「这个 Mod 没实现
  状态面」（后者 host 回 503，不是这个字段）。

**字段集是契约**（Wave 3 起有守卫测试 `state_json_key_set_is_pinned`）：顶层恰好
`always_on_top` / `click_through` / `opacity` / `voice_active` / `window` 五个 key，
`window` 恰好 `{opened, reason}`。前端按 key 渲染，增删字段必须同步本节。

**消费方（Wave 3）**：Flutter `settings/sections/dev_tools_section.dart` 的
`_ModConfigTile` —— 展开卡片的「运行态（只读）」块；API 封装在
`api/mods_api.dart` 的 `ModsApi.state(id)`。`window` 休眠时界面**逐字**显示
「窗口未开（原生壳休眠），此面仅状态」（Dart 常量 `kWindowReasonNativeShellDormant`，
与 Rust 的 `WINDOW_REASON_NATIVE_SHELL_DORMANT` 逐字一致——两侧各有一条守卫测试：
`window_reason_string_is_stable_and_ascii` / 「reason 常量与 Rust 侧逐字一致」）。

host 侧读取面语义（基座已有回归，本轨不改）：

| 情况 | 响应 |
| --- | --- |
| 在册 + 启用 + 快照可取 | `200 {"enabled":true,"id":"pet-desktop","state":{…}}` |
| 在册但停用 / worker 正持锁 / 未实现 | `503 {"error":{"code":"state_unavailable",…}}` |
| id 不在注册表 | `404 {"error":{"code":"not_found",…}}` |

## 5. 闭环操作步骤（可开关：真实服务 + curl）

前置：`./scripts/ignite.sh`（默认 `18080`），另开终端。**curl 带 loopback Origin**
（mutating 路由的既有安全口径）；`GET /state` 只读，不校验 Origin。

```bash
BASE=http://127.0.0.1:18080
ORIGIN="Origin: $BASE"
JSON="Content-Type: application/json"
```

**步骤 1 — 确认在册**（`pet-desktop` 出现在列表里，`enabled=false`）：

```bash
curl -s "$BASE/api/v1/mods" | python3 -m json.tool | grep -A 3 '"pet-desktop"'
```

**步骤 2 — 启用并同时下发起点配置**（`enable` 的 body 可带 `config`，
省掉「启用后再 POST 一次」）：

```bash
curl -s -X POST "$BASE/api/v1/mods/pet-desktop/enable" \
  -H "$JSON" -H "$ORIGIN" \
  -d '{"config":{"always_on_top":true,"click_through":false,"opacity":0.95}}'
# 期望：{"ok":true}
```

**步骤 3 — 读状态**（`enabled=true` + 全字段；`window` 恒休眠）：

```bash
curl -s "$BASE/api/v1/mods/pet-desktop/state"
```

期望（`serde_json` 无 `preserve_order`，键按字母序输出）：

```json
{"enabled":true,"id":"pet-desktop","state":{"always_on_top":true,"click_through":false,"opacity":0.95,"voice_active":false,"window":{"opened":false,"reason":"native_shell_dormant"}}}
```

**步骤 4 — 改配置**（这就是「改配置」这一步）：

```bash
curl -s -X POST "$BASE/api/v1/mods/pet-desktop/config" \
  -H "$JSON" -H "$ORIGIN" \
  -d '{"config":{"always_on_top":false,"click_through":true,"opacity":0.3}}'
# 期望：{"ok":true,"restarted":true}
```

**步骤 5 — 立刻再读**（`restarted:true` 表示新实例已带新 config）：

```bash
curl -s "$BASE/api/v1/mods/pet-desktop/state"
```

期望（三个字段全部跟随，`voice_active` / `window` 不变）：

```json
{"enabled":true,"id":"pet-desktop","state":{"always_on_top":false,"click_through":true,"opacity":0.3,"voice_active":false,"window":{"opened":false,"reason":"native_shell_dormant"}}}
```

**步骤 6 — 钳位是活的**（越界配置不改 schema，只在快照里钳）：

```bash
curl -s -X POST "$BASE/api/v1/mods/pet-desktop/config" \
  -H "$JSON" -H "$ORIGIN" -d '{"config":{"opacity":5}}'
curl -s "$BASE/api/v1/mods/pet-desktop/state"   # opacity = 1.0
```

> `POST …/config` 是**整份替换**该 Mod 的 namespaced config（`registry.reload_config`
> 直接换 `entries[id].config`，**不是**深合并）——只发 `opacity` 时，
> `always_on_top` / `click_through` 会跟着回落缺省。前端保存表单时要发**整份**配置。

**步骤 7 — 停用 → 状态面关闭**（503 而不是 404，两者刻意分开）：

```bash
curl -s -X POST "$BASE/api/v1/mods/pet-desktop/disable" -H "$JSON" -H "$ORIGIN"
curl -i -s "$BASE/api/v1/mods/pet-desktop/state"    # 503 state_unavailable
curl -i -s "$BASE/api/v1/mods/nope/state"           # 404 not_found
```

**步骤 8 — 口型信号**（可选，需主链真的开口）：在 Flutter `/app/` 里发一句话，
开口期间步骤 3 的 `voice_active` 变 `true`，收声后回 `false`。
该字段来自既有的 `VoiceStarted` / `VoiceEnded` 订阅，Wave 2 未改语义。

**界面版（Wave 3 的软闭环入口，不需要另写 curl）**：`./scripts/ignite.sh` 后开
`http://127.0.0.1:18080/app/` → 设置 → **Mod** → 展开「桌宠窗口」：

1. 运行态块显示 `总在最前：开` / `点击穿透：关` / `不透明度：0.95` /
   `语音活跃：关` / `窗口：窗口未开（原生壳休眠），此面仅状态`；
2. 改「总在最前」→ **保存** → 保存成功后卡片自动重取 state，`总在最前` 变 `关`；
   块头的「刷新运行态」按钮可手动重取；
3. Mod **未启用**时该块显示「运行态暂时读不到（…503 state_unavailable）」——
   这是 503，不是「这个 Mod 不存在」（后者 host 回 404，界面文案也不同）。

`_buildConfig()` 保存时发的是**整份配置**（从服务端已有 config 出发、只覆盖 schema
声明的字段），与上面步骤 6 的「整份替换」语义一致。

**进程内版（不需要活服务）**：`cargo test -p live2d-ai-mod-pet-desktop` 里的
`reconfigure_is_reflected_immediately` 用同一份数据形状断言了「换 config →
下一次 `state_json` 立刻是新值」，`factory_create_carries_config_into_state` 则覆盖
host `start_one` 的 `create(services, config)` 路径。

## 6. 门禁与测试

- `cargo test -p live2d-ai-mod-pet-desktop` → **17 passed / 0 failed**（Wave 2 的 15 条
  + Wave 3 的 2 条守卫；集成测试 `tests/pet_desktop_state.rs`，lib 无内联测试）；
- `cargo fmt --all -- --check` clean；
- `cargo clippy -p live2d-ai-mod-pet-desktop --all-targets -- -D warnings` → 0 warning；
- 无 doc-test（纯数据 crate，无示例代码块）；
- Flutter（消费面）：`cd shell/flutter && flutter analyze` → No issues found；
  `flutter test` → 全绿，含新增 `test/pet_desktop_state_test.dart` **9 条**
  （端点/解析、503/404 分流、休眠文案、字段顺序、展开显示、**保存→重取→字段跟着变**、
  503 上屏）。

覆盖：descriptor / `api_version`、**静态 schema 与 start 注册相等**、schema 字段与
边界且无 `enabled`、只订阅 `VoiceStarted`+`VoiceEnded`、缺省回落 `true/false/0.95`、
config 三字段优先、`opacity` 钳位（含 NaN / ±∞ / 边界原样）、逐字段坏类型回落、
`window` 恒休眠、事件态翻转与其它主题无副作用、`shutdown` 复位、**reconfigure 立刻
反映**、`create` 携带 config、快照只读稳定、纯函数 `build_state_json` 与 runtime 同形、
**字段集被钉住**（`state_json_key_set_is_pinned`）、**reason 字面量稳定**
（`window_reason_string_is_stable_and_ascii`）。

涉及文件：

- `crates/live2d-ai-mod-pet-desktop/src/lib.rs`（实现；源码 ≤500 行）
- `crates/live2d-ai-mod-pet-desktop/tests/pet_desktop_state.rs`（17 条回归）
- `shell/flutter/lib/api/mods_api.dart`（`ModStateResult` + `ModsApi.state`）
- `shell/flutter/lib/settings/sections/dev_tools_section.dart`（运行态块 + 纯函数）
- `shell/flutter/test/pet_desktop_state_test.dart`（9 条 Flutter 回归）

## 7. 已知缺口 / 下一步

1. **窗口没开**（§2）：`window.opened` 恒 `false`。要变成 `true`，先按 §2 立项论证
   「谁来维护第二个 UI 壳」，再把落点接回 egui 原生壳——**不**在本 Mod 里起窗口。
2. **Flutter 已消费本状态面（Wave 3 已闭环）**：`/app/` → 设置 → Mod → 展开
   「桌宠窗口」即可读运行态，改配置保存后字段跟着变（§5「界面版」）。
   **仍未做**的是「桌宠窗口 UI」本身——那段闭环终点是**数据**，不是像素（§2）。
   宿主若希望显式接线（而不是卡片自建同源 `ModsApi()`），见
   `REGISTER-pet-desktop-v1.md` §1.4：本轨刻意不碰 `app/shell_settings.dart`。
3. **无持久化**：`voice_active` 是进程内事件态，重启即 `false`（符合语义，不是缺陷）。
4. **配置只在 restart 后生效**：`POST …/config` 走 host 的 `reload_config + restart`，
   这是既有产品链路口径（不是本 Mod 的选择）。Mod 未启用时 `POST …/config` 只写
   manifest 不重启（响应 `{"ok":true,"enabled":false}`），此时状态面本来就是 503。

## 8. 非目标

- **不**创建窗口 / 托盘图标 / 置顶 / 点击穿透的任何原生实现（休眠壳归属，§2）；
- **不**引入 `winit` / `egui` / `wgpu` 或任何第二套渲染依赖；
- **不**重写 `live2d-ai-desktop` 壳（含 `src/app/`、`web_api/`）；
- **不**新增端点：只消费基座 Wave 2 的 `GET /api/v1/mods/{id}/state`；
- **不**改 `AVAILABLE_MOD_FACTORIES` / `mod_count_*` / `default_mods_manifest` /
  版本号（pet-desktop 已在表里，缺省停用不变）；
- **不**在 `state_json` 里做 IO / 阻塞 / 加锁等待（契约见 `factory.rs` 头注）；
- **不**把 `voice_active` 接进 TTS / 口型主链逻辑（它只是信号镜面，主链口型由既有
  路径驱动）；
- **不**改 `shell/flutter/lib/app/shell_settings.dart`（宿主接线文件，Wave 3 轨 D
  刻意不碰，避免与 B 轨冲突）：`_ModConfigTileState._loader` 在未注入时自建**同源**
  `ModsApi()`（base 解析与 `main.dart` 里那个一致），真机可用、测试注入 fake；
- **不**给休眠原生壳接线（`window.opened` 恒 `false`，§2）。
