# legacy —— 旧 JS 前端预览，**勿当现网**

> 一句话：**这个目录里的东西不是现行产品界面。**
> 现行产品面 = Flutter `/app/`（`shell/flutter/`），规格看 `docs/design/web-ui-spec-v3.md`。

## 里面是什么

| 文件 | 是什么 |
|---|---|
| `ui-preview-v2-oldjs.html` | **旧原生 JS 前端**的静态 HTML 设计预览（v2 token 时代，`--brand: #40C5F1` 那套） |
| `preview-main-oldjs.png` | 上面那份 HTML 的主界面截图 |
| `preview-modal-oldjs.png` | 模态/浮层截图 |
| `preview-actpanel-oldjs.png` | 动作面板截图（**该能力已整条拆除**，见 `docs/architecture/core-chain-baseline.md` §3.1） |

## 为什么单独放在这里（2026-09-12，rc.2）

原生 JS 前端已于 **2026-09-11 整体删除**（`web_api/index.html` + `app.js` +
`chat.js` + `style.css` + `index_html.rs`；`/` 现在 302 到 `/app/`）。

但上面这几份预览**留在了 `docs/design/` 顶层**，和现行规格摆在同层。后果很具体：
它长得像一个「当前的界面设计稿」，维护者和本地 agent 都会顺手把它当成现网去对照——
而它描述的是**另一个产品**（另一套 token、另一套布局、还带着已删除的动作面板）。

所以处理方式是**隔离 + 改名**（不是删除）：文件名带 `-oldjs`，目录名带 `legacy`，
再挂上这句「勿当现网」。这样可逆、可追溯，也不会有人误读。

## 要恢复参考价值怎么办

别从这里抄。现行规格是 `docs/design/web-ui-spec-v3.md`；真值以 Flutter 源码为准
（`shell/flutter/lib/`）。这里只作为「当时长什么样」的历史存档。

## 追加（2026-09-13，rc.3）：过时的「网页 UI 规格」也搬进来

`docs/design/` 顶层原本 8 项并存，其中 4 份是**旧原生 JS 前端时代**的规格：
`web-ui-spec-v2.md`、`web-ui-polish-spec.md`、`web-ui-redo-spec-st1-2.md`、
`web-ui-settings-wiring-v1.md`。它们描述的是**另一套已经删掉的前端**（vanilla JS + rail + 模态），
与现行 Flutter 规格 `../web-ui-spec-v3.md` 并存只会让人读错——所以同样**隔离 + 保留**。

现行规格仍然只有一份：`docs/design/web-ui-spec-v3.md`（Flutter Web）。
`web-action-trigger-archive.md` **留在顶层**：活代码在引用它（见上文）。
