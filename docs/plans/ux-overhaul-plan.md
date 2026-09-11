# Live2D-Ai UX 全量对标 + 重设计方案

> 产出时间：2026-08-07
> 对标项目：N.E.K.O.、Open-LLM-VTuber、AI-Vtuber(Ikaros)、Soul of Waifu
> 调研来源：docs/research/ux-benchmark.md + docs/research/benchmark-neko-vs-neurosama.md + 我们源码实测

---

## 1. 认知纠正：之前错在哪

过去 48 小时我做的事 = 逐个修 bug（表情 fade、口型、强度）——这是在**修缮一个没完成设计的房子**，不是在做产品。你要的是先把房子的**蓝图**画好。

## 2. 全局差距一览

| 模块 | N.E.K.O. | Open-LLM-VTuber | **Live2D-Ai 现在** | 差距 |
|---|---|---|---|---|
| **设置页结构** | 独立页面 + Tag 分类 | YAML 配置文件（非 GUI） | 一页长列表堆砌 | 巨大 |
| **API Key 管理** | 独立 `/api_key` 页，每个 Provider 单独卡片 | conf.yaml 环境变量 `${VAR}` | 分散输入框，无 Provider 分组 | 巨大 |
| **TTS 引擎选择** | 14+ Provider，每个独立配置卡片 | 下拉框 + 参数域 | 4 个固定 RadioButton | 大 |
| **LLM 模型选择** | 14+ 服务商可选，独立配置 | 多 Provider 下拉 | 零配置只能 GLM（无 DeepSeek 入口） | 大 |
| **Live2D 模型管理** | 5 形态 Avatar，文件导入+在线下载 | 目录式模型文件夹 | 代码改路径，无 UI | 巨大 |
| **主界面** | Live2D+气泡+底部栏（麦克风/文字/功能）+ 悬浮窗 | 纯 WebView（Electron 内） | Live2D 全屏+气泡+单输入栏 | 中 |
| **功能入口** | 顶部栏：设置/角色/插件/帮助 多入口 | 右键菜单 | 仅右上角齿轮 | 中 |

## 3. 五个核心改进（按影响排序）

### 改进 1：设置页重设计 — 对标 N.E.K.O.（最影响"粗糙感"）

```
当前：一页滚动列表，Key 分散在各处
目标：BottomSheet 或独立页 + 4 个 Tab
  ┌─────────────────────────┐
  │  [API] [语音] [形象] [更多] │  ← Tab 分类
  ├─────────────────────────┤
  │  LLM Provider     ▼     │
  │  ┌─────────────────────┐│
  │  │ DeepSeek   ● (已配)  ││  ← 每个 Provider 一张卡片
  │  │ API Key: sk-***8d1   ││
  │  │ 模型: deepseek-v4    ││
  │  └─────────────────────┘│
  │  ┌─────────────────────┐│
  │  │ 智谱 GLM  ○ (免费)    ││
  │  │ + 添加 API Key       ││
  │  └─────────────────────┘│
  │  ┌─────────────────────┐│
  │  │ + 添加自定义 Provider ││
  │  │ URL: ____  Model: __ ││
  │  └─────────────────────┘│
  └─────────────────────────┘
```

### 改进 2：统一 Key 管理（对标 N.E.K.O. 独立 /api_key 页）

- 所有 Key（LLM/TTS/第三方）在一处管理
- 每个 Provider 一张卡片：名称 + Key 输入（密码框）+ 测试连接按钮 + 余额/状态
- 支持"添加自定义 Provider"（填 endpoint + model name）
- Key 安全：密码框 + 明文切换 + 本地加密存储（当前 SharedPreferences 明文，安全不加分）

### 改进 3：TTS 引擎自定义（对标 N.E.K.O. 的 14+ Provider）

- 列表显示**所有已注册引擎**（不再只 4 个固定）
- 每张卡片：音色预览按钮、rate/pitch 滑块（可视化）、API key（可单独配置或从统一 Key 管理继承）
- "+ 添加引擎"按钮 → 表单：名称 + 类型（云端/本地/系统）+ endpoint + voice 列表

### 改进 4：Live2D 模型导入（对标 AI-Vtuber 的目录式模型管理）

- 主界面或设置页加"换形象"入口
- 内置模型列表 + "导入新模型"按钮
- 导入方式：文件选择器（.model3.json）→ 复制到 assets → 注册到 ModelRegistry
- 切换形象后 Live2D 热切换（已有 switchModel 接口）

### 改进 5：主界面功能入口（对标 N.E.K.O. 多入口）

- 顶部栏增加：麦克风按钮（已有）+ 形象切换 + 清空对话
- 长按 Live2D：弹出互动菜单（换表情/换动作/换音色）
- 消息气泡：长按复制/分享

## 4. 实施顺序

```
Wave 1: 设置页重设计（Tab + Key 管理卡片）— 解决"粗糙感"的 80%
Wave 2: 模型导入 + 形象切换 UI
Wave 3: TTS 引擎自定义 + 音色预览
Wave 4: 主界面功能入口打磨
```

## 5. 本次提交的改动

- DeepSeek key 内置进 local.properties（gitignore）✅
- LLMProviderManager 默认优先 DeepSeek ✅
- 设置页重设计：待你确认本方案后走 SPOQ

---

**确认后我走 SPOQ 流程：建 DAG → 派 Architect 出详细 UI 设计稿 → 你审阅 → Developer 实现。** 这次不做碎片修复，一次性出完整方案。
