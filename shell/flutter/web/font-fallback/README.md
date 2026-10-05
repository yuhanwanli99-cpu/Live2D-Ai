# font-fallback/ —— 引擎回落字体的**同源**精选镜像

## 这是什么

Flutter Web（CanvasKit）**没有系统字体回落**：已打包字体缺某个字形时，引擎的
字体回落服务会去下载 Noto。它的 base URL 默认是
`https://fonts.gstatic.com/s/`（引擎 `configuration.dart` 的 `fontFallbackBaseUrl` 默认值）。

本项目是**本地优先**的桌宠，运行期不允许依赖 Google CDN。因此
`shell/flutter/web/flutter_bootstrap.js`（自定义 bootstrap）用**现代 API**
把 base URL 改成**同源相对路径**：

```js
_flutter.loader.load({ config: { fontFallbackBaseUrl: 'font-fallback/' }, /* …SW 设置 */ });
```

- 引擎做 `Uri.parse(baseUrl).resolve(font.url)` ⇒ base URL **必须以 `/` 结尾**；
- 相对路径相对**文档 base**（`web/index.html` 的 `<base href="/app/">`）解析 ⇒
  实际命中 `/app/font-fallback/**`，换 base-href 时无需改这里
  （写成绝对 `/app/font-fallback/` 反而会在别的 base-href 下失效）；
- 因此**目录结构必须与引擎回落表的 url 逐字一致**：
  `font-fallback/<family>/v<ver>/<file>.woff2`。

## 镜像内容（整族，21 文件 / 2 815 292 B ≈ 2.69 MiB）

| 字族 | 文件数 | 说明（覆盖的典型字形） |
| --- | --- | --- |
| `notocoloremoji` | 12 | Emoji / 麻将牌等（如 `🀄` U+1F004） |
| `notosanssymbols2` | 6 | 符号补充区（箭头、几何、装饰符号等） |
| `notosanssymbols` | 1 | 基础符号 |
| `notomusic` | 1 | 乐谱符号（如 `𝄞` U+1D11E） |
| `notosansmath` | 1 | 数学字母数字符号（如 `𝕏`） |

**只镜像这 5 族**的理由：它们是「中文 UI / LLM 输出里最可能出现的非 CJK 缺口」
（emoji 尤其高频），而全量回落表是 **724 文件 / 20.73 MiB**——对一个本地优先的
桌宠来说，往构建产物里塞 20 MiB 只为了「所有冷僻字形都不缺」不划算。
大 CJK 五族（jp/hk/sc/tc/kr，≈11.9 MiB）**刻意不镜像**：本应用已自托管
`NotoSansSC-AiSubset`（见 `shell/flutter/assets/fonts/`，覆盖 20 976 个汉字），
再镜像一份 CJK 回落是纯冗余。

## 明确接受的取舍：**离线时子集外字形 = 豆腐块**

未镜像的字族在运行期会请求同源 `font-fallback/<family>/…` → **404**。
引擎把 404 当该字体的**永久失败**（`font_fallback_service.dart`
`_isPermanentStatus`：4xx 即永久，不重试），于是那个字形显示为**豆腐块**。

这是**有意接受**的：

1. **不跨源**——失败也失败在同源，断网/离线场景不会白屏、不会静默出网；
2. **不半族**——同一字族**要么整族都在、要么整族都缺**。半族比全缺更坏：
   引擎会「部分字形能显示、部分变豆腐块」，看起来像随机 bug；
3. 需要更多字形时的正确做法是：把字族加进
   `scripts/font_fallback_mirror.sh` 的 `FONT_FAMILIES`（或用 `FONT_FAMILIES=…` 覆盖）
   重新生成，或把字形并进 `assets/fonts/` 的子集（用
   `scripts/font_subset_ranges.py` 重新生成覆盖表）。

## 重新生成 / 审计

```bash
# 按 Flutter SDK 里的引擎回落表重新下载（需要网络）
scripts/font_fallback_mirror.sh

# 逐文件 sha256/bytes 校验 + 「磁盘集合 == 引擎表全集」完整性校验
# **不访问网络**：只读本地文件与 MANIFEST.txt ⇒ 断网也能跑
scripts/font_fallback_mirror.sh --check
```

`MANIFEST.txt`（`family / url / bytes / sha256` + 头部说明）由脚本生成，**勿手改**；
`OFL.txt` 是 5 个字族的上游许可原文（OFL-1.1，再分发时必须保留）。

Flutter 升级后回落表可能变化（版本号/文件名/字族切分），`--check` 会因此报红——
那正是需要重新跑一次生成脚本的信号（`RESULT: FAIL` 会指出是哪个文件缺失/陈旧）。

## 相关

- 设计与实测证据：`docs/architecture/font-fallback-offline.md`
- 运行期接线：`shell/flutter/web/flutter_bootstrap.js`
- 打包中文字体子集：`shell/flutter/assets/fonts/`
