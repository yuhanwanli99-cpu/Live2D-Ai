# Mod 社区许可与注册边界（0.2.0-rc.1）

> 状态：**生效**。与 [Mod 产品链路](mod-product-chain.md) 配套；冲突时以本文的许可口径为准。
> 范围：**Mod 的注册面、分发面与许可证**。项目整体许可见根 [LICENSE](../../LICENSE) /
> [LICENSE-SCOPE.md](../../LICENSE-SCOPE.md) / [COMMERCIAL-LICENSE.md](../../COMMERCIAL-LICENSE.md)。

---

## 0. 一句话

**注册面开放、分发面有门槛**：任何人都能写自己的 Mod 在本机装载；但**要进官方默认包 /
正式发布清单**，实现必须与 **AGPL-3.0-only 兼容**（现行 Mod 全部 Rust，静态编译进同一
binary）。闭源或 AGPL 不兼容的 Mod 可以**私用**，但不能搭上官方分发——想随官方包分发
请走商业许可。**本文件不承诺任何「闭源 Mod 可进默认包」的路径。**

---

## 1. 为什么「注册面」和「分发面」要分开

| 面 | 谁决定 | 规则 |
|---|---|---|
| **注册面**（本机加载/启用） | 用户自己 | 开放：按 [mod-product-chain §3](mod-product-chain.md) 的勾选表加 crate 即可；官方不审核你本机装什么 |
| **分发面**（进官方 binary / 发布说明 Mod 清单） | 维护者 | 有门槛：许可兼容 + Rust/C 主体 + 通过门禁（见 §3） |

本项目是**静态编译**的 Mod 系统（`AVAILABLE_MOD_FACTORIES`，无动态加载）；
Mod 与 host 编进**同一个 binary**。这正是分发面必须看许可的根本原因（见 §2）。

---

## 2. 静态链接下的许可现实（关键）

`AGPL-3.0-only` 是**强 copyleft**。官方 binary 里静态链接的每一行
项目自有代码，其源码都必须按 AGPL 对**网络使用者**可用。

由此推出三条**硬结论**：

1. **官方分发的 Mod 必须 AGPL-3.0-only 兼容**。不兼容的许可证（专有、GPL-2-only、
   AGPL 不兼容的其它 copyleft 等）**不得**编进官方 binary，也不得写进正式发布说明的
   Mod 清单。
2. **闭源 / 不兼容 Mod 可以私用**：你可以在自己的 fork / 本机构建里加任何 Mod，
   只要你不对外分发那个 binary。一旦对外分发，AGPL 的源码提供义务就落到**整个 binary**上。
3. **想闭源且要官方分发 → 商业许可**：见 [COMMERCIAL-LICENSE.md](../../COMMERCIAL-LICENSE.md)。
   它**不会**给已按 AGPL 发布的代码撤权，也不会替第三方依赖/资产重新授权。

> 第三方依赖与资产（Live2D Cubism Core、模型文件、blivedm 等）**保持各自条款**，
> 本项目不代其授权——见 [LICENSE-SCOPE.md](../../LICENSE-SCOPE.md) §2–§3。

---

## 3. 进官方分发面的门槛（勾选表）

新 Mod 想进 `AVAILABLE_MOD_FACTORIES` / 正式发布说明，逐条满足：

- [ ] **许可**：Mod 代码以 **AGPL-3.0-only 兼容**许可提供（缺省即 AGPL-3.0-only），
      或已就兼容/商业安排与维护者书面确认。
- [ ] **主体语言**：**Rust（或 C）**为实现主体；Python/Dart/JS 等只能标**实验**，
      **不得**进正式发布说明的 Mod 清单（口径同 [mod-product-chain §6](mod-product-chain.md)）。
- [ ] **不绕过 core 仲裁**：经 `live2d-ai-mod-system` 接入，不自行接管主链路。
- [ ] **密钥**：不直接持有密钥；读取走 `live2d-ai-runtime::secrets`，
      日志/WS/导出不得出现值。
- [ ] **门禁**：Rust 全套门禁绿（见 [AGENTS.md](../../AGENTS.md) 一张表）；
      Mod 数量/ID 断言同步更新。
- [ ] **失败隔离**：`create` / `start` / `on_event` 失败
      只关该 Mod，主链继续。

---

## 4. 现行 Mod 的许可状态（0.2.0-rc.1）

全部为 **Rust + AGPL-3.0-only**，与官方分发面兼容：

| Mod | 缺省 | 许可 | 分发面 |
|---|---|---|---|
| `external-input` | **on** | AGPL-3.0-only | 官方包内 |
| `pet-desktop` | off | AGPL-3.0-only | 官方包内（骨架） |
| `persona` | off | AGPL-3.0-only | 官方包内 |
| ~~`local-llm`~~ | — | AGPL-3.0-only | **已废除启动**（移出注册面；crate 暂留仓库） |
| `live2d-ai-mod-template` | — | AGPL-3.0-only | 模板 crate，**不**注册 |

---

## 5. 社区 Mod 的常见问题

**Q：我写了个闭源 Mod，能进官方 release 吗？**
不能直接进。两条路：① 以 AGPL-3.0-only 开源后走 §3；② 走商业许可。
你现在就可以**私用**它（自建 binary，不对外分发）。

**Q：我把 Mod 写成 Python 插件，算正式 Mod 吗？**
不算。非 Rust/C 只能标注**实验**，不进正式发布说明清单（§3）。把它当成外部工具，
用 `external-input` 这类**已定义端点**与主链交互，而不是编进 workspace。

**Q：Mod 里能直接读 `DEEPSEEK_API_KEY` 吗？**
不能。密钥只经 `live2d-ai-runtime::secrets`；Mod 拿不到原始环境变量。

**Q：改官方 Mod 的许可证行不行？**
项目自有代码缺省 AGPL-3.0-only；变更必须由版权方书面同意，且不能撤销已发布版本的既有权利。

---

## 6. 相关文件

- [Mod 产品链路（契约 / 勾选表 / 非目标）](mod-product-chain.md)
- [LICENSE-SCOPE.md](../../LICENSE-SCOPE.md) — 各部分许可总表
- [COMMERCIAL-LICENSE.md](../../COMMERCIAL-LICENSE.md) — 商业许可说明
- [LICENSE](../../LICENSE) — AGPL-3.0-only 全文
