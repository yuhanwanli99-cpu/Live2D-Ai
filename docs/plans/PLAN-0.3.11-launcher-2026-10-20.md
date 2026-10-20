# 执行单：0.3.11 启动器（2026-10-20）

> 状态：**已落地**（2026-10-20，本地 tag `v0.3.11-rc.1` = `a728d4a`（点火）、
> `v0.3.11-rc.2` = `c7e5913`（启动器），rc.3 只清理（tag `v0.3.11-rc.3`），**未 push**）。
> 交付事实与门禁见发布说明 `docs/releases/v0.3.11-rc.3.md`。
>
> 交给执行 agent。这一块就是 **`0.3.11`**。不要并进 `0.4.0` / `0.4.1`。
> 写法 `0.3.11-rc.z`：`z` 按代码自拆；每步一次本地提交加一个本地附注 tag，不 push。
> 最后一个 `rc.(z+1)` 只清理测试、文档和棘轮，不加功能。`Cargo.toml` 仍是 `0.3.1`。
> 不改 `.env`、`mods.json`、`live2d-ai.toml`。不提交 `official_live2d_samples.7z`。
> 不把 `/home/skystar/live2d-bili-sidecar/` 加进任何仓库。
> 文件名曾叫 `PLAN-ignite-mod-prestart-2026-10-20.md`。那一版「监听前自动预启动」已撤，不要再做。

维护者 2026-10-20 的点火日志是输入，不要再空等 10 分钟。当时
`./scripts/ignite.sh --build` 已经 `GET /app/ = 200`，进程听在 `127.0.0.1:18080`，
随后因为 `[tts].mode=local` 空等 `GET http://127.0.0.1:8091/v1/models` 600 秒后退出码 1，服务还在。
注册 Mod 数是 0。`mods.json` 里的旧 id 原样保留。验收用 `--port` 打空端口，不要占用户的 18080。

## 0. 做完是什么样

1. 桌面进程先听 HTTP。`ignite.sh` **不**拉起任何 Mod，**不**跑 `melo/start.sh`。
2. `GET /` 与 `GET /index.html` 返回启动器这一页（200）。`GET /app/` 仍是现有 Flutter。点火在 `GET /` = 200 后打开 `http://127.0.0.1:${PORT}/`，不再为 TTS 空等。
3. 启动器上能改的只有：已编进本次二进制的 Mod 勾选，以及「预启动」开关（缺省关）。三个数字由勾选算出来，不能手填一个装进新 Mod 的数。
4. 点「进入」之后播加载动画：按勾选调用现有启停接口；预启动开着才拉起会生进程的那几个；同时对**当前这一份** `[llm].base_url` 和**当前这一份** `[tts].base_url` 各探测一次，只报告可达性，不改档。然后进入 `/app/`。失败也进入，不等 `GET /models = 200`。

缺省工厂仍是 0。三个数字都是 0 时，「进入」照样进主界面。云端档不要求 8091 上有 Melo。

## 1. 现在的事实

单跑主仓没有 Mod。`AVAILABLE_MOD_FACTORIES` 为空，`default_mods_manifest` 是 `{"mods":{}}`。`GET /api/v1/mods` 是空列表。这是拆仓之后的缺省。

添加仍是 [`two-repo-mods.md`](../architecture/two-repo-mods.md) §4 的五步重新编译。没有动态加载器，没有商店，没有 zip 运行时。用户能拉的是 [`CURRENT.md`](../architecture/CURRENT.md) §8 的七个公开源码仓库。`mods.json` 只能开关已经编进去的 id。

`local-llm` 已删除，禁止挂回，也不要新建本地 LLM Mod。本地对话端点仍是用户自己的 OpenAI 兼容服务，地址在 `[llm]` 的 local 档。`local-engine` 是 spawn-only，不写 `[llm]` / `[tts]`，不探活，不是对话端点。

今天 `GET /` 与 `GET /index.html` 是 302 到 `/app/`（`flutter_app::root_redirect`）。`ignite.sh` 在启动桌面进程前会 `pkill` 同级 `live2d-ai-mod-local-tts` 的 `melo/serve.py` 与 `engine/`。本单撤掉这次清理：缺省二进制不拥有这些进程，不要杀掉用户自己拉起的 Melo。替换本服务的 `pkill -f "live2d-ai-desktop --web"` 保留。

Melo 权重不在公开仓库里。本单不复制、不下载 `checkpoint.pth`。

## 2. 禁止

- 不把七个 Mod 编回缺省二进制，工厂数保持 0。不建 `.so` / 下载市场 / 热插加载器。
- 不恢复厂商目录、`vendors.*`，也不为「另一档」新增第二份 `base_url`。探测只读当前段里那一份已经写下的地址。
- 不复活 `local-llm`、`pet-desktop`、`wallpaper`、CosyVoice `local-tts`。不把 `local-engine` 当成 LLM 或 TTS。
- 不把 `action_tx` 接回去。不改导演规则表。不新增 WS 帧。
- 探测不打对话、不合成语音。切换不写 `live2d-ai.toml`，不改 `.env`。
- 启动器不是第二套设置，也不是第二个 Flutter 应用。页面上不出现地址、密钥、厂商、模型名编辑。
- 不复制 Melo 权重。不提高 `DOCS_BUDGET_LINES`（45000）。不放宽 `src >500`（22）和 desktop deps（18）。
- 不改 `AGENTS.md` 历史段、`CHANGELOG.md`、已发布的 `docs/releases/**`。活文档里描述「现在入口是 302」的句子，在行为落地的同一次提交里改。

## 3. 启动器（一页，然后交给 `/app/`）

新模块 `crates/live2d-ai-desktop/src/web_api/launcher.rs`，HTML/CSS 用 `include_str!` 放在 `crates/live2d-ai-desktop/assets/launcher.html`。`flutter_app.rs` 里 `GET /` 与 `GET /index.html` 改为返回这一页（`text/html`、`Cache-Control: no-store`），不再 302。`/app/` 与 `/app/*` 的托管不动。直接打开 `/app/` 不要跳回启动器。`web_api/mod.rs` 保持 ≤500 行，路由表注释改一句即可。

页面只做这些：

- 三个数字，全部由列表算出，**没有数字输入框**。
  - **已注册**：`GET /api/v1/mods` 的条数。工厂 0 显示 0。只读。
  - **本次确认**：勾选条数。只能勾接口返回的 id，不能超过已注册。
  - **本次拉起**：预启动关着是 0；开着则等于勾选里 id 为 `local-tts-melo` 或 `local-engine` 的条数。其余 id 只算确认，不算出进程。
- **预启动**开关，缺省关。拨动它只改页面上的「本次拉起」，不发请求。这个开关不写 toml，也不写 `mods.json`。
- 勾选在点「进入」之前不调用 enable / disable，不 spawn。
- 按钮「进入」。工厂 0 时列表为空，按钮仍可点。

「进入」之后，同一页播 CSS 加载动画（一条进度、一行状态），再 `location.assign('/app/')`：

- 预启动关：对已注册的 `local-tts-melo` / `local-engine` 调用现有 `POST /api/v1/mods/{id}/disable`，避免 `launch_mode=with_app` 在启用时把进程拉起来。其它 id 按勾选调用现有 enable / disable。不跑 `start.sh`。
- 预启动开：勾选的拉起类 id 走现有 enable（沿用该 Mod 已有的 spawn）；没勾的拉起类 id disable。不等 `GET /models`。进程马上退出就在状态行写失败原因，然后继续。
- 工厂 0：零次 Mod POST。
- 动画期间做第 5 节那一次探测。探测完，无论成败，都进入 `/app/`。整段远小于 600 秒：沿用现有自检的短超时，不要新增长轮询。

字体走仓库里已有的 `shell/flutter/assets/fonts/NotoSansSC-AiSubset-Regular.woff2`，由本服务同源发出。禁止 `fonts.googleapis.com` / `fonts.gstatic.com`。页面里的汉字必须落在 `shell/flutter/assets/fonts/*.ranges.txt`。`font_subset_test.dart` 只扫 Dart，所以在 `launcher` 的 Rust 测试里对 HTML 做这道检查。

单测用临时 manifest，不要读写仓库根那份 gitignore 的 `mods.json`。

## 4. 点火

改 `scripts/ignite.sh`：

- 删掉 `[tts].mode=local` 对 `GET {tts}/models` 的 600 秒循环，以及超时后的 `exit 1`。`mode=cloud` 继续不等 8091。
- `GET /` = 200（沿用 90 秒上限；在启动器落地前的中间 rc，若 `/` 仍是 302，则等 `GET /app/ = 200`）之后打印地址并打开浏览器。启动器落地后打开的是 `http://127.0.0.1:${PORT}/`。服务仍前台 `wait`，Ctrl+C 仍停服务。
- 删掉启动前对同级 `melo/serve.py` 与 `engine/` 的 `pkill`。不要后台执行 `start.sh`。
- `curl -w "%{http_code}"` 后面再 `|| echo 000` 会把失败码拼成 `000000`。还留着的这种写法改成只保留三位。
- 文件头注释与「等待本机 TTS 就绪」文案改成与本节一致。
- `tests/test_shell_scripts_syntax.py`：local 不再要求「等待本机 TTS 就绪（缺省不自动拉起）」；cloud 仍必须出现「云端端点，不等本机进程」。判据仍是只认 `[tts].mode`，不要改成按主机名猜测。

`scripts/ignite.sh --check` 与 `scripts/ignition-precheck.sh` 里「`GET /` → 302 `/app/`」在启动器落地的同一次提交改成：`GET /` = 200 且正文含启动器标记，`GET /app/` 仍是 200，产物仍不得含 `gstatic.com/flutter-canvaskit`。启动器正文同样不得含这个串。

`flutter_app::root_paths_redirect_to_the_flutter_app` 改成断言 `/` 与 `/index.html` 为 200 HTML，而不是 302 到 `/app/`。`/app/` 不受影响。根路径的非 GET 仍交回 dispatch。

## 5. 探测当前这一份地址，不改档

0.3.9 之后 `[llm]` / `[tts]` 各只有一份 `base_url` 加一个 `mode` 标签，`vendors.*` 已删。LLM 切模式只改标签，地址不动（`patch.rs`：切模式不复制快照；`mode_is_not_part_of_transport_and_never_falls_back`：链路没有第二个端点）。TTS 切到 `local` 会调用 `apply_local_tts_defaults`，把地址盖成 `http://127.0.0.1:8091/v1` 并清掉密钥绑定，云端地址不保留。所以没有「另一档」可探。本版**不切换**。CURRENT §7「local 连不上不会偷偷切 cloud」继续有效，不要改成自动换档。

- 时机：用户点了「进入」、加载动画已经在播。`[llm]` 一次，`[tts]` 一次。不放进每一轮对话，也不放在 `ignite.sh` 里。
- 对象：只打该段**当前** `base_url`。空地址不发请求，状态行写未配置。不要用出厂缺省、主机名或厂商名编第二个地址。
- 分类沿用现有自检：`GET {base_url}/models`，404 算可达，401/403 算节点在但鉴权失败，其余按现有码。不真跑对话，不合成。
- 结果只显示：启动器状态行写「本次对话」「本次语音」是可达、不可达、未配置还是鉴权失败。能放进现有 `GET /api/v1/app/status` 的只读字段就不要新路由。
- 不改 `mode`，不改 `base_url`，不改本次进程的出站端点，不写 toml，不改 `.env`。现有那条「不自动回退」测试保持通过。
- 密钥仍只经 `secrets::lookup`。探测日志不打印密钥值。

## 6. 文档（行为落地的同一提交）

- `CURRENT.md` §7：点火不再空等；`/` 是启动器；探测按第 5 节，只报当前地址，不改档。「不自动回退」这句保留。缺省不挂 Melo 这句保留。
- `CURRENT.md` §2 里「0.3.11 已派、未执行」改成已落地，并写上最后一个 rc 的提交哈希。未做完不要改成已落地。
- `two-repo-mods.md` §4：启动器不能代替那五步。
- `README.zh-CN.md`、`README.md`、`CONTRIBUTING.md` 里「`/` 302 到 `/app/`」和「等 models 才给地址」改成第 4 节。QQ 仍只出现在 `README.md`、`COMMERCIAL-LICENSE.md`、`LICENSE-SCOPE.md`、`CURRENT.md`；不要写进 `README.zh-CN.md`。
- `AGENTS.md` 里仍在描述现行入口的那几句（`GET /` 302、点火体检三条）同步改。历史段一行不动。
- 本文件做完后把文首标成已落地，并写上提交哈希。未做完不要标。

## 7. 验收

- 工厂 0、`mode=local`、8091 无进程：点火在页面就绪后打开 `http://127.0.0.1:<port>/`，输出里没有 600 秒等待，退出码不是因为 TTS 超时。
- 启动器显示已注册 0、本次确认 0、本次拉起 0。预启动拨到开，拉起仍是 0。点进入后进入 `/app/`，且没有 spawn `serve.py` / `start.sh`。
- 单测夹具里注册了 `local-tts-melo`：预启动关着点进入，该 id 被 disable，不 spawn；预启动开着且勾选，走现有 enable。临时 manifest，不碰仓库根 `mods.json`。
- 探测单测：只打当前 `base_url`。不可达、401、空地址都不改 `mode` / `base_url`，不写 toml。空地址零次请求。`mode_is_not_part_of_transport_and_never_falls_back` 仍通过。
- `bash -n scripts/ignite.sh` 与 `tests/test_shell_scripts_syntax.py` 通过。`GET /app/` 仍 200。启动器与 Flutter 产物都不引用 gstatic。
- 未改 `.env`、仓库根 `mods.json`、`live2d-ai.toml`。未把权重加进 git。工厂数仍为 0。

## 8. 交付时写明

主仓每个 rc 的提交哈希与本地 tag。点火是否还含 600 秒循环。`GET /` 的状态码。探测是否只打当前地址且没有改档。工厂数仍为 0。`0.4.0` 仍未开始。
