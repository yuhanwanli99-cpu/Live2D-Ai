# PLAN-0.2.0-rc.1 — Mod 纪元第一基线（修订 2026-09-14）

> 上游：`v0.1.0-rc.5` = `0883c1d3`（origin 已齐）。
> **Agent 预算：~1h**（DeepSeek 4.1）。
> 叙事：钩子要经典、整件要能关、晋升有门槛；注册开放 / 官方 Mod=AGPL 兼容。
> 用户 backlog 对齐：角色卡打磨；**废除 local-llm**；壁纸/语音/记忆/导演/桌宠见后续 rc。

---

## 0. 一句话

Mod 纪元开门：现有 **persona** 可日常演示 + 加新勾选表实走过一次 + 社区许可证一页说清 + **local-llm 退出默认注册（废除启动）**。

---

## 1. 里程碑

| ID | 交付 | 验收 | 估时 |
|---|---|---|---|
| **D0** | `docs/architecture/mod-community-license.md`：注册面开放；官方分发 AGPL 兼容；闭源走商业许可/私用；链进 mod-product-chain / docs 索引 | 无「闭源可进默认包」承诺 | ~12m |
| **D1** | 加新 Mod 勾选表实走（template 复制最小 crate；可不进默认 FACTORIES；留下记录） | release 或 plans/notes | ~15m |
| **P0** | **persona** 路径打磨：配置→enable→system_prompt→disable 还原；坏输入老实报错；测试保住 | API/测试 | ~15m |
| **X0** | **废除 local-llm 启动**：移出 `AVAILABLE_MOD_FACTORIES`（或等价默认注册表）；文档标 deprecated/历史；相关断言 `mod_count` 等同步；crate 可暂留仓库不删（避免大爆炸，标注勿新用） | 启动后 mods 列表无 local-llm；cargo 绿 | ~12m |
| **R** | 版本 `0.2.0-rc.1` + `docs/releases/v0.2.0-rc.1.md`（Mod 现状表；声明主链皮肤冻结、local-llm 废除） | 三处版本同步 | ~10m |

### 不做（本 rc）

动态壁纸、语音输入、记忆/KB、导演实现、桌宠小 UI、拆子仓、动态加载、复活 Action。

---

## 2. 门禁

cargo test/clippy 全绿；Flutter 有改则 analyze+test；mod 数量断言与文档一致。

---

## 3. 审查清单

- [ ] 无主链人设/背景回潮
- [ ] local-llm 不在默认 FACTORIES / 启动列表
- [ ] persona 启停还原仍成立
- [ ] 社区许可证与 AGPL+静态链接自洽
- [ ] release 写清下一刀（rc.2：语音/外输入 + 壁纸策略 v0）

