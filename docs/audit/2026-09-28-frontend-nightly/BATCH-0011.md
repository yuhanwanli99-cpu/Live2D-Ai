# BATCH-0011 — Phase 1 · web/ + pubspec.yaml + tool/（红线 K 收口）

> 账本：`AUDIT-B/`。行号基于 HEAD `5ef879f4`。

## 本批读过
| 文件 | 行 | 一句话 |
|---|---|---|
| web/index.html | 23 | 全读：`<base href="$FLUTTER_BASE_HREF">` + `flutter_bootstrap.js` + 本地图标/manifest；**零外部源**（无 gstatic / 无 CDN / 无在线字体）✓ |
| web/manifest.json | 24 | 全读：`start_url: "."`、四个本地图标、`prefer_related_applications: false`；无外部 URL ✓ |
| pubspec.yaml | 44 | 全读：运行时依赖只有 `flutter + http + web` 三个（D 维度「入口包体」无冗余）；字体自托管两条 woff2 + OFL 随产物 ✓；**头注的版本号与实际值不符（F-0011-1）** |
| tool/visual_review_test.dart | 289 | 读头注 40 行：12 张「三断点 × 四主题」PNG 的视觉回归工具；**诚实写出局限**（VM 上画不出 iframe 平台视图，舞台那格是占位） |
| test/wiring_test.dart | — | 读 1-120：逐个点名的「接线守卫」，每个符号都写清「没接上会怎样」；用 `referencedOutside(symbol, ownFile)` 的 `contains` 扫描（不剥注释——与 F-0005-6 同族的已知弱点，但每个符号都点名且今天都命中真实调用位） |
| 红线 K 三重防线（只读） | — | ① `scripts/ignite.sh:176` 把 `--no-web-resources-cdn` 写死进构建 flag；② `:185-191` 启动时 grep 产物 `main.dart.js` 里的 `gstatic.com/flutter-canvaskit`（挡住「产物已存在所以不重建」）；③ `--check` 再对**已服务的** `/app/index.html` 与 `main.dart.js` 各 grep 一次 |
| 产物取证（只读，不审产物） | — | `build/web/` 存在；`flutter_bootstrap.js` 里 `_(n,e)` 的分支是 `config.canvasKitBaseUrl ?? (engineRevision && !useLocalCanvasKit ? CDN : "canvaskit")`；`build/web/canvaskit/` 在位 ⇒ 当前产物是**带 flag 构建**的；`main.dart.js` 内**无** gstatic（与 ignite.sh 的判定一致） |
| lib/ 外部 URL 全扫 | — | 5 处 `https://`，**全部在注释里**（MDN / pub.dev / github 说明性链接 + theme.dart:28 讲 fonts.gstatic.com 风险的那句）⇒ 运行期无外部取 |

## 发现
- **F-0011-1（P3）** `pubspec.yaml` 头注写「与后端 `Cargo.toml` 的 version 对齐（0.2.0-rc.4）」，实际 `version: 0.2.0-rc.5+10` —— 注释与值不同步（与 §8 已登记的「AGENTS.md 当前版本仍是 rc.4」同族，但**是另一个文件**，且这处会误导下一个人以为版本线停在 rc.4）。

## 本批核对过、不成发现的（正面记录）
- **红线 K 在源码层完全成立**：index.html 零外部引用、manifest 零外部引用、lib/ 零运行期外部取、构建 flag 写死、产物 grep + 服务态 grep 双重兜底。这是全仓**防御最深的一条红线**（对比 F-0005-5 的字体回退缺口——那条在**运行时由数据触发**，产物 grep 抓不到，属另一类）。
- pubspec 的依赖面极小（3 个运行时依赖），没有为「也许以后用得上」引入的包——D 维度的「入口包体」这条在本项目上没有债务。
- `tool/visual_review_test.dart` 主动写明「它不能替代真机点击」「平台视图那一层只能真机验收」——**工具自己声明自己的盲区**，这正是 CONSOLIDATION 里说的「注释是最好的资产」的正例。
- 产物新鲜度：当前 `build/web/` 与 `--no-web-resources-cdn` 的形状一致（`canvaskit/` 在位 + `main.dart.js` 无 gstatic），**没有**「源码已改而产物未重建」的迹象 → 记 STATE 环境栏即可，不算缺陷。

## 本批未核实
- 产物是否比源码旧（时间戳未比：审计禁写，产物本身按 §0 不审）——只核了 CDN 形状一致，没核功能新鲜度。
- `tool/visual_review_test.dart` 的 249 行实现体（12 个 case 怎么拼、字体怎么加载）未读；它是工具不是门禁，优先级低。
