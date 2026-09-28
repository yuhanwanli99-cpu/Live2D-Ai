# BATCH-0019 — Phase 2 · 横扫 G：安全

> 账本：`AUDIT-B/`。

## 一、注入面与危险 API 全仓扫描

| 面 | 扫描 | 结果 |
|---|---|---|
| `innerHTML` / `eval(` / `new Function(` | `grep -rn "innerHTML\|eval(\|Function(" lib/` | **零命中**（`Function(...)` 的命中全是 Dart 的 `void Function(...)` 类型标注）——Flutter Web 没有 innerHTML，代码也没自己造一条 ✓ |
| 重定向 / 新窗口 | `grep -rn "window.open\|location.href\|location.replace" lib/` | **零命中**（只有 `location.origin` 的读取，用于 postMessage 校验）✓ |
| 外部 URL 拼接 | `grep -rnoE "https?://…" lib/` | 5 处全在**注释**里（MDN/pub.dev/github 的说明链接）✓ |
| `data:` URL 解析 | `pickFileDataUrl` → `FileReader.readAsDataURL` → `decodeDataUrlBytes` | 不在任何导航/iframe 上下文使用；SVG 走 `Image.memory` 的 `errorBuilder`（Skia 不解 SVG，无脚本执行面）✓ |
| postMessage | `live2d_host_web.dart:48/78` | 收：origin + source 双校验；发：钉死 target origin ✓ |

## 二、密钥面（G 红线）

| 面 | 结论 |
|---|---|
| 前端是否持有密钥 | **否**。`EnvKey` 类里**没有 value 字段**（0008 已核）；写走 `PUT /api/v1/env`，值只出现在函数参数里（`shell_admin.dart:99` 注释：值不进 state、不进日志、不回显） |
| `GET /api/v1/env` | 后端只回键名 + `set` 布尔；前端 `EnvApi.list()` 也没有任何读值方法 ✓ |
| `GET /api/v1/app/status` | 后端**手工构建 DTO**（`dto.rs:5-6` 明写「不直接 `to_value(AppSettings)`，后者会序列化 `api_key_env` 字段名」），只带 `has_api_key` 布尔 ✓ |
| localStorage | 只有两个键：显示偏好、会话存档；两者都不含密钥（密钥只在本机 `.env`，服务端侧）✓ |
| 剪贴板 | 两个写点：① 消息正文（用户自己的文本）② 诊断快照 `_snapshotLines()`（12 行，只含 ws/版本/epoch/uptime/`has_api_key` 派生字段）——**已核** `dto.rs` 保证 `llm`/`tts` 子对象里只有布尔 ✓ |

## 发现
- **F-0019-1（P3）** `pickFileDataUrl` **不检查文件大小**：`accept` 只是文件对话框的**筛选提示**（用户仍可选「所有文件」），选完立刻 `readAsDataURL` 把整个文件读成 base64 进内存，而体积上限的检查（`kStageImageMaxChars` / `kBackgroundImageMaxChars`）发生在**读完之后**。

## 本批核对过、不成发现的（正面记录）
- 后端为「不泄露变量名」专门**手写 DTO** 而不是直接序列化配置结构（`dto.rs` 头注 P0-1）——这是把红线写进类型系统的做法，比「记得别序列化」可靠得多。
- 本项目**没有**开放重定向面、没有 `data:` 导航、没有 innerHTML：三个最常见的 Web 注入面在架构上就不存在。

## 本批未核实
- F-0019-1 的实际阈值：各上限的具体字符数（B2 核过 1.5 M 总预算），但「先崩后校验」的真实表现（OOM vs 卡顿）未在真机试。
