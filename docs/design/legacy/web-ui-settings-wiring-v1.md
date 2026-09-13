# 设置面板接线规格 v1（web-ui-settings-wiring-v1）

> 状态：实现中。主 Agent 设计定稿，实现照做；后端契约 + bridge 协议 + 前端接线分开定义。
> 目标：让 v2.5 迁移新增的视觉控件（角色卡 / 互动开关 / 背景图）从「纯视觉」变成「真实生效」。

## 1. 角色与人设（A1）：persona 增加 `name` / `description`

### 1.1 后端 `PersonaSettings`（live2d-ai-runtime/src/settings.rs）
新增两个 `#[serde(default)]` 字段：

```rust
/// 角色名称（酒馆卡 name；空 = 未设置）。
#[serde(default)]
pub name: String,
/// 角色描述（identity · background；空 = 未设置）。
#[serde(default)]
pub description: String,
```

- 序列化/反序列化自动跟上（struct 已有 derive）。
- `live2d-ai.toml.example` 的 `[persona]` 段补充两行示例注释。

### 1.2 `PersonaView`（settings/view.rs）

```rust
pub struct PersonaView {
    pub system_prompt: String,
    pub max_history_pairs: usize,
    pub name: String,
    pub description: String,
}
```
`From<&PersonaSettings>` 同步补 `name`/`description`。

### 1.3 `PersonaPatch`（settings/patch.rs）

```rust
pub struct PersonaPatch {
    pub system_prompt: Option<Option<String>>,
    pub max_history_pairs: Option<usize>,
    pub name: Option<Option<String>>,
    pub description: Option<Option<String>>,
}
```
apply 分支照 system_prompt 写法：`Some(fields.name)` → 非空才覆盖（清空语义同现有字段）。

### 1.4 语义约定（空串 = 清除）

- UI 输入空字符串 → 前端 patch `null`（清字段，与 api_key 清除语义一致，见 buildPatch persona 段）。
- 后端同样只把**非空**字段写入，覆盖旧值。

### 1.5 LLM 上下文注入（live2d-ai-runtime/src/settings.rs `resolve()`）

`ConversationConfig.system_prompt` 现在 = `persona.system_prompt` 原样。

新规则：`name`/`description` 非空时**前置拼接到 system_prompt**（不改原字段，拼接产物只进会话）：

```
[角色卡] 名称：{name}\n描述：{description}\n\n{system_prompt}
```

空字段跳过对应行；两者皆空则原样使用 system_prompt。拼装函数独立（纯函数方便单测）：

```rust
pub fn build_effective_system_prompt(name:&str, description:&str, base:&str) -> String
```

单测：空全部 / 只有 name / 只有 desc / 全有 + base / 全空仅 base，五分支。

## 2. 外观与舞台·互动开关（B1）：bridge `stage-config` 扩展

### 2.1 wasm listener（l2d-wasm-demo/src/main.rs `"stage-config"` 分支）

新增两个可选 bool 字段：

```rust
if let Some(v) = j.get("idleEnabled").and_then(|x| x.as_bool()) {
    st.bridge.idle_enabled = v;
}
if let Some(v) = j.get("clickEnabled").and_then(|x| x.as_bool()) {
    st.bridge.click_enabled = v;
}
```

### 2.2 `BridgeState`（main.rs）新增 `idle_enabled: bool` / `click_enabled: bool`

默认 **true**（与 HTML checkbox `checked` 初始一致）。

### 2.3 消费点（surface.rs）

- **idle**：`IdleState::update` 驱动呼吸/眨眼的位置加守卫 `if self.idle_enabled`；false 时跳过 idle 动作（呼吸/眨眼/微动），模型静止。实现放 surface：`BridgeState` 已在 surface 有镜像（查 `surface::AppState` / `IdleState` 引用 bridge 的方式，跟随现状）。
- **click**：pointerdown 触发互动（若有）时守卫 `click_enabled`；false 则点击不触发互动动作（缩放/位移仍由 zoom 控制条管，互不冲突）。

### 2.4 前端（app.js）

- `f-idle` / `f-click` change → `postToStage({type:"stage-config", idleEnabled:bool, clickEnabled:bool})`。
- 初始状态（load 时）读 checkbox checked 发一次同步（保证后端/bridge 与 UI 一致）。
- 本地行为，不持久化到 toml（B1 方案）。

## 3. 背景图（C2）：本地 dataURL → wasm 渲染

### 3.1 前端（index.html + app.js）

- `f-bgimg`：按钮旁放**隐藏 `<input type=file accept="image/jpeg,image/png">`**，点按钮触发文件选择。
- 选择后：`FileReader.readAsDataURL` → `localStorage.setItem("dsh.stage.bg", dataUrl)` → `postToStage({type:"stage-bg", dataUrl, width:v[0], height:v[1]})`。
- 加载时：localStorage 有 bg → 恢复 stage-config 后补发 `stage-bg`。
- 提供清空：按钮组「选择文件…」「清除背景」（清除=删 localStorage + postToStage dataUrl=null）。
- **dataURL 走 postMessage 本地通道，不进后端/日志/WS**（防泄漏）。

### 3.2 wasm（surface.rs）

- `stage-bg` listener：`dataUrl` 为 null/空 → 清背景；否则 `set_image_bg(data_url)`（新建 `Image`，onload 后 draw 到 canvas 底层）。
- 背景绘制：先铺背景图（cover 缩放），再叠加模型层（现状 model draw 逻辑不变，背景在 model 之下。png 半透明背景图时 model 仍可见）。
- 与 dark/light 底色共存：**背景图设置后覆盖底色**（不透明区域）；清除后恢复底色。

### 3.3 约束

- 图片解码失败 → `console.warn` + toast「背景图解码失败」，不 crash。
- 一律本地；**不新增后端端点**。

## 4. 门禁

- `cargo test --workspace --all-targets`（含新单测：build_effective_system_prompt 5 分支、patch name/desc、view 往返、wasm 逻辑编译）。
- `node --check` app.js。
- 实起服务 curl：p-name/p-desc 读回、idle/click change、bg 按钮存在。
- 提交 + tag `p3-settings-wiring-v1`。

## 5. 明确不做（本期范围外）

- 角色卡不注入 LLM 之外的任何地方（不建表、不做后台索引）。
- 背景图不做后端上传/持久化。
- 互动开关不做后端持久化（刷新恢复默认 true；本地 checkbox 状态也不存）。