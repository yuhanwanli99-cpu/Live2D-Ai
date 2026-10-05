# 字体回落的**结构性离线化**（R4-T4）

> 状态：**已落地（2026-10-06 轮）**。判据「注入子集外字符时跨源请求 = 0」已用真实浏览器实测通过。
> 相关：`shell/flutter/web/flutter_bootstrap.js`（运行期接线）、
> `shell/flutter/web/font-fallback/`（镜像 + MANIFEST + 许可）、
> `scripts/font_fallback_mirror.sh`（生成/审计）、`scripts/font_offline_check.mjs`（验证据）、
> `shell/flutter/assets/fonts/`（自托管中文子集）。

## 1. 问题

Flutter Web 的 CanvasKit **取不到设备字体**，`fontFamilyFallback` 也不会命中系统字体。
已打包字体缺某个字形时，引擎的字体回落服务会去下载 Noto，base URL 默认是：

```
https://fonts.gstatic.com/s/      # 引擎 configuration.dart 的 fontFallbackBaseUrl 默认值
```

对一个**本地优先**的桌宠，这条默认值有三个问题：

1. **断网即豆腐块**——回落根本拿不到字体；
2. **静默出网**——哪怕只显示一个 emoji 也会连 Google（隐私 + 可用性）；
3. 「有网时看不出来」——所以它能一直躺在代码里不被发现（本轮之前，
   实测形态与复现记录见 AGENTS.md 的 2026-10-05 债轮「已知缺口 #1」）。

## 2. 机制：同源相对路径 + 自定义 bootstrap

引擎把回落字体 URL 拼成 `Uri.parse(baseUrl).resolve(font.url)`
（`font_fallback_service.dart`），所以 base URL **必须以 `/` 结尾**。

本项目新增 `shell/flutter/web/flutter_bootstrap.js`（flutter_tools 支持用户自带模板：
`packages/flutter_tools/lib/src/build_system/targets/web.dart`），用**现代 API** 注入：

```js
const flutterConfig = {
  fontFallbackBaseUrl: 'font-fallback/',
};
_flutter.loader.load({ config: flutterConfig, serviceWorkerSettings: { /* 原语义保留 */ } });
```

三条约束（每条都对应一个真实的失败模式）：

| 约束 | 为什么 |
| --- | --- |
| base URL **以 `/` 结尾** | `Uri.resolve` 缺尾斜杠会吃掉最后一段 |
| 用**相对路径** `font-fallback/`，不用绝对 `/app/font-fallback/` | 相对路径相对**文档 base**（`web/index.html` 的 `<base href="/app/">`）解析；换 base-href（如部署到 `/`）时无需改这里 |
| 用 `load({config})` → `initializeEngine(config)`，**不用** `window.flutterConfiguration` | 老写法已 deprecated，且引擎会 assert 二者不可混用 |

模板里 **必须**保留 `{{flutter_js}}` 与 `{{flutter_build_config}}` 两个占位符
（flutter_tools 的构建校验依赖它们）。另有一条**踩过的坑**：flutter_tools 是全文
`replaceAll`，**占位符字面量不许出现在注释里**——写了会被注入一整份 `flutter.js`
把注释行切断。模板里占位符各出现 **恰好 1 次**，由验证据脚本前的构建模拟断言。

## 3. 精选镜像：5 个整族 / 21 文件 / 2 815 292 B（≈2.69 MiB）

镜像目录：`shell/flutter/web/font-fallback/<family>/v<ver>/<file>.woff2`
（**路径与引擎回落表的 url 逐字一致**；`web/` 下除 `index.html` 与
`flutter_bootstrap.js` 之外的文件会被 `flutter build web` 递归复制到 `build/web/`）。

| 字族 | 文件数 | 字节 | 覆盖的典型字形 |
| --- | --- | --- | --- |
| `notocoloremoji` | 12 | 2 014 496 | Emoji / 麻将牌（`🀄` U+1F004） |
| `notosanssymbols2` | 6 | 430 608 | 符号补充区（箭头、几何、装饰） |
| `notosansmath` | 1 | 226 412 | 数学字母数字符号（`𝕏`） |
| `notomusic` | 1 | 74 660 | 乐谱（`𝄞` U+1D11E） |
| `notosanssymbols` | 1 | 69 116 | 基础符号 |
| **合计** | **21** | **2 815 292** | |

**为什么不镜像全量**：引擎回落表共 **724 文件 / 20.73 MiB**；大 CJK 五族
（jp/hk/sc/tc/kr）另占 **≈11.9 MiB**——而本应用已自托管
`NotoSansSC-AiSubset`（`shell/flutter/assets/fonts/`，20 976 个汉字），
再镜像一份 CJK 回落是纯冗余。所以只买「中文 UI 与 LLM 输出里最可能出现的非 CJK 缺口」。

## 4. 失败语义与**明确接受的取舍**

- 引擎把 **4xx 当永久失败**（`font_fallback_service.dart: _isPermanentStatus`，
  404 不重试），于是那个字形显示为**豆腐块**；命令行/控制台会打一条
  `Permanent HTTP failure (status 404) …` + `… is permanently unavailable.`
- 全局 kill switch：**「已有过成功注册」就不会触发**（`_registeredFonts.isNotEmpty`
  时不再累计致命）；只有「连续 10 次永久失败且一次都没成功」才停服务
  （`_maxGlobalFailuresBeforeBroken = 10`）。
- **不许半族**：同一字族要么整族都在、要么整族都缺。半族比全缺更坏——
  一部分字形能显示、一部分变豆腐块，看起来像随机 bug。`--check` 因此同时校验
  「磁盘集合 == 引擎表里这些字族的全集」。

⇒ **离线时子集外字形 = 豆腐块**，这是**明确接受的取舍**（不是遗漏）：

1. 失败也失败在**同源**——不跨源、不静默出网、不白屏；
2. 想要更多字形有两条明路：把字族加进 `scripts/font_fallback_mirror.sh` 的
   `FONT_FAMILIES` 重新生成，或把字形并进 `assets/fonts/` 的子集
   （`scripts/font_subset_ranges.py` 重新生成覆盖表）。

## 5. 可复现 / 可审计

```bash
# 重新下载镜像（需要网络）：读 Flutter SDK 里的引擎回落表，整族拉取并重写 MANIFEST.txt
scripts/font_fallback_mirror.sh

# 审计（**不访问网络**，只读本地文件 + MANIFEST.txt）：
#   逐文件 sha256/bytes + 「磁盘集合 == 引擎表全集」完整性
scripts/font_fallback_mirror.sh --check
```

`MANIFEST.txt` 列 `<family>\t<url>\t<bytes>\t<sha256>`；`OFL.txt` 是 5 族的上游许可
原文（OFL-1.1，再分发必须保留）。**Flutter 升级后回落表可能漂移（版本号 / 文件名 /
字族切分）**，`--check` 会因此报红——那正是「该重新生成」的信号，不是脚本坏了。

### 实测（2026-10-06，本机 Flutter 3.47.3 / engine 06a2e2a1）

```
$ scripts/font_fallback_mirror.sh --check
引擎表字族（整族镜像，共 21 文件）：
  notocoloremoji     12
  notosanssymbols2   6
  notosanssymbols    1
  notomusic          1
  notosansmath       1
清单条目 21 · 磁盘字体文件 21 · 引擎表期望 21 · 合计 2815292 字节
RESULT: PASS（清单 == 磁盘 == 引擎表全集；逐文件 sha256/bytes 相符）
```

（同一命令在 `https_proxy=http://127.0.0.1:9` 的死代理下同样 PASS —— 它不联网。）

## 6. 运行期验证据：`scripts/font_offline_check.mjs`

零依赖 CDP 脚本（Node 22 自带 WebSocket；写法参照 `scripts/browser_probe.mjs`）。
它打开页面 → 往 Flutter 的文本宿主里注入 `𠮷`(U+20BB7) / `🀄`(U+1F004) /
`𝄞`(U+1D11E) → 记录 `Network.requestWillBeSent` / `responseReceived` →
**按 host 分组**输出请求表，并断言「非 loopback 请求 = 0」+「同源 `font-fallback/**`
命中 notocoloremoji 与 notomusic 200」。

```bash
PROBE_CDP=http://127.0.0.1:9222 FONT_CHECK_URL=http://127.0.0.1:18099/app/ \
  flock /tmp/l2d-browser.lock -c 'node scripts/font_offline_check.mjs'
```

**本次实测（副本自验，见 §7）原始输出**：

```
# 注入实证：输入框实测 = "𠮷🀄𝄞"（三个字符都在）

== 按 host 分组的请求数（load=页面加载期 / inject=注入期 / after=注入后 / control=阳性对照）==
host                                load  inject  after  control  total   loopback
127.0.0.1:18099                       16       3      0        0     19   yes
fonts.gstatic.com                      0       0      0        1      1   NO

== 跨源（非 loopback）请求 ==
  注入前 = 0  注入后 = 0  阳性对照（脚本故意） = 1
    (对照，不计入断言) https://fonts.gstatic.com/s/notosanssc/v1/__probe__.woff2 → 404

== 同源 font-fallback/** 请求（3 条）==
  404  inject  notosansjp      …/app/font-fallback/notosansjp/v53/….1.woff2
  200  inject  notocoloremoji  …/app/font-fallback/notocoloremoji/v32/….4.woff2
  200  inject  notomusic       …/app/font-fallback/notomusic/v20/pe0rMIiSN5pO63htf1sxItKQB9Zra1U.woff2

== 控制台/日志里的字体相关行（3 条）==
  inject/warning: Permanent HTTP failure (status 404) for Noto Sans JP 1 at font-fallback/notosansjp/v53/….1.woff2.
  inject/warning: Font Noto Sans JP 1 at font-fallback/notosansjp/v53/….1.woff2 is permanently unavailable.
  inject/warning: Could not find a set of Noto fonts to display all missing characters. …

== 断言 ==
  [PASS] 注入前：非 loopback 请求 = 0（页面加载期不出网） —— 实测 0 条
  [PASS] 注入后：非 loopback 请求 = 0（含字体回落窗口） —— 实测 0 条
  [PASS] 同源命中 notocoloremoji（🀄 U+1F004）200 —— 1 条
  [PASS] 同源命中 notomusic（𝄞 U+1D11E）200 —— 1 条
  [PASS] 注入实证：三个子集外字符真的进了 Flutter 输入宿主 —— "𠮷🀄𝄞"
  [PASS] 阳性对照：探测器确实看得见跨源请求 —— 1 条
  [PASS] 同源 font-fallback 无 5xx / 非 200|404 状态 —— []
RESULT: PASS
```

三行 warning 同时回答了最后两个问题：**404 确实走「永久失败 → 豆腐块」**，
而且 URL 是**相对**形式（`font-fallback/notosansjp/…`，由 fetch 按文档 base 解析）。

### 阳性对照为什么必须有

`external = 0` 有两种可能：真的没出网，或**探测器瞎了**。所以脚本在断言窗口**之外**
故意跨源请求一次 `fonts.gstatic.com`，并把它单列成 `control` 一栏——
它证明探测器看得见跨源请求。**对照不计入断言**。

## 7. 副本自验（不碰 Lead 独占的 `build/web`）

本轮的**构建期集成**（`flutter build web` 对自定义 bootstrap 的占位符校验、
`web/` → `build/web/` 的递归复制）由 Lead 的构建验证。运行期行为在**副本**上验：

```bash
# 1) 造副本：只读复制 build/web → <dir>/app（放在 app/ 下，让 <base href="/app/"> 与线上
#    一致 ⇒ 相对路径 font-fallback/ 的解析被真验到），复制 web/font-fallback，
#    再用**本仓模板**模拟 flutter_tools 的 token 替换生成副本的 bootstrap
#    （断言：每个占位符在模板里恰好出现 1 次、替换后无残留、node --check 通过）
node scripts/font_offline_check.mjs --prepare-copy /tmp/font-check-web

# 2) 静态站
python3 -m http.server 18099 --bind 127.0.0.1 --directory /tmp/font-check-web

# 3) 验证据（浏览器锁必须独占：本机只有一个无头 Chrome）
flock /tmp/l2d-browser.lock -c \
  'FONT_CHECK_URL=http://127.0.0.1:18099/app/ node scripts/font_offline_check.mjs'
```

`--prepare-copy` 的自证输出（副本 bootstrap 与本次实测逐字节相同）：

```json
{
  "tokenCounts": { "{{flutter_js}}": 1, "{{flutter_build_config}}": 1, "{{flutter_service_worker_version}}": 1 },
  "bootstrapBytes": 14307,
  "bootstrapSha256": "df104661f0addc6092a9d7bb75cc8b92c1a361dd090f5fed94deff09944f2015",
  "leftoverTokens": [],
  "hasFontFallbackBaseUrl": true,
  "indexBaseHref": "/app/"
}
```

实测结果：`RESULT: PASS`（上面那张表），原始事件落 `/tmp/font-offline-check-copy.json`。

**这一节的边界**：副本实验只验**运行期行为**；它**不能**证明
`flutter build web` 接受自定义 `web/flutter_bootstrap.js`（占位符校验、`web/` 递归复制、
`--no-web-resources-cdn` 组合）。那三条必须在真实构建里验——见 §8。

## 8. 未完成 / 风险

| 项 | 状态 | 谁负责 |
| --- | --- | --- |
| **构建期集成**（`flutter build web` + 自定义 bootstrap 的占位符校验 + `web/font-fallback` 递归进 `build/web`） | **未验**（build/web 由 Lead 独占，本轮不跑构建） | **Lead**：构建后 `curl /app/font-fallback/<任一文件>` 应 200，且 `build/web/flutter_bootstrap.js` 含 `font-fallback/` |
| 线上 `/app/` 的端到端复验（18080） | 未做（本轮只在 18099 副本上验；18080 仍是旧产物） | Lead 重建后跑一次 `font_offline_check.mjs`（默认 URL 就是 18080） |
| 对照跑（未改动产物的「改前」跨源数） | **跳过**：浏览器锁被其他队友长期占用，且「注入即出网 gstatic」已有 AGENTS.md 的实测记录 | 可选 |
| Flutter 升级导致回落表漂移 | `--check` 会报红（缺文件/陈旧条目） | 谁升级谁重跑 `scripts/font_fallback_mirror.sh` |
| 未镜像字族（含 CJK 回落）离线 = 豆腐块 | **明确接受**（§4） | 需要时按 §4 的两条明路加 |

## 9. 口径

- 不再有「缺字回落到 gstatic」的表述：运行期回落是**同源**的；
- 「离线时子集外字形 = 豆腐块」是**取舍**，不是缺口；
- 界面文案仍不得出现子集外字符（`shell/flutter/test/font_subset_test.dart` 守着），
  这条与运行期回落是**两件事**：前者管「我们自己写的字」，
  后者管「LLM/用户输入的任意字」。
