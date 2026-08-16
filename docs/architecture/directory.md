# Live2Dai 目录约定

> 目标：让仓库顶层保持“一眼能看懂”，生成物/本地环境不混进源码。

## 顶层结构（目标态）

```
Live2Dai/
├── README.md                 # 项目入口：定位、快速开始、架构入口
├── AGENT.md / AGENTS.md      # Agent/开发者导航
├── HANDOVER.md               # 交接文档（当前版本定稿）
├── CHANGELOG.md              # 版本变更
├── LICENSE / NOTICE / CREDITS.md
├── shared/                   # 跨端共享配置与契约（单源真理）
│   ├── persona.yaml
│   ├── model_registry.json
│   ├── model_dict.json       # LLM 路由表
│   ├── model-adapter/
│   ├── emotion-protocol.md
│   ├── mcp_tools.json
│   └── check_cross_platform_consistency.py
├── Live2Dai-Android/         # Android 主端（Kotlin/Compose + Native）
├── Live2Dai-pc/              # Windows 桌面端（Open-LLM-VTuber）
├── docs/
│   ├── README.md             # 文档索引
│   ├── architecture/         # 架构、契约、目录、依赖、可观测性
│   ├── plans/                # 规划类文档
│   ├── research/             # 调研类文档
│   └── verification/         # 验收/测试报告
├── scripts/                  # 开发/部署/健康检查脚本
├── tests/                    # 根级 Python 测试（无 Key 优先）
```

## 目录职责

| 目录 | 放什么 | 不放什么 |
| --- | --- | --- |
| `shared/` | 跨端共享的配置、协议、校验脚本 | 任何端私有实现 |
| `Live2Dai-Android/` | Android 源码、Gradle、assets、cpp | PC 逻辑 |
| `Live2Dai-pc/` | PC 后端、前端、配置 | Android 逻辑 |
| `docs/architecture/` | 当前架构、契约、边界、依赖 | 历史规划草稿 |
| `docs/plans/` | 历史/未来规划、任务拆分 | 已废弃调研 |
| `docs/research/` | 竞品、技术调研 | 当前实现细节 |
| `docs/verification/` | 验收报告、测试证据 | 源码 |
| `scripts/` | 可复用脚本 | 一次性草稿 |
| `tests/` | 根级无 Key 测试 | Android/PC 专测（放各自目录） |

## 清理规则

- 生成物：`build/`、`app/build/`、`.gradle/`、`.cxx/`、`.pytest_cache/`、`.ruff_cache/`、`__pycache__/` 不应出现在源码树中，统一可被 `scripts/clean.sh` 清理。
- 本地环境：`.venv/`、`node_modules/`、`models/`、`live2d-models/` 属于本地运行环境，不提交、不进入源码目录。
- 根级日志：`*.log` 不入库，统一放 `logs/` 或直接删除。
- Agent 状态：`.pi/`、`.pi-glla/`、`spoq/` 属于本地/Agent 工作区，不入库，不应混入 `docs/`。
