# BATCH-0021 — Phase 2 · 横扫 I：样式令牌 / 主题覆盖

> 账本：`AUDIT-B/`。

## 一、字体族（红线 L）覆盖盘点

`grep -rn "fontFamily" lib/`（排除令牌声明与 theme.dart 自身的注释）**只有一处**：

```dart
// lib/ui/message_bubble.dart:445-448 —— 行内码
style: (base ?? const TextStyle()).copyWith(
  fontFamily: 'monospace',          // ← 唯一的组件层字体族覆盖
  backgroundColor: codeBackground,
),
```

- 其余全部走 `ThemeData.fontFamily`（theme.dart:101）单一入口 ✓
- `buildAppTextTheme` 刻意不设 fontFamily（typography.dart:101-103 注释）✓
- `EmphasizedText` 的两处 `const TextStyle()` 只带 `fontWeight` ✓

⇒ 但这唯一的一处正是 **F-0021-1**：它给码段换掉了字体族，而没有给出 `fontFamilyFallback`。

## 二、其余令牌绕过面（人工扫，不靠 lint）

| 绕过面 | 结果 |
|---|---|
| 裸 `TextStyle(...)` 构造 | 4 处，**全部只带 `fontWeight`**（两个 `const TextStyle()`、两处 `?? const TextStyle()` 兜底）⇒ 合法 ✓ |
| `Colors.xxx` | lint 规则 `\bColors\.(?!transparent\b)[a-z]` 覆盖全部成员名（Material 的 `Colors` 常量一律小写下划线）✓ |
| `fontSize:` | lint 规则裸匹配字面量，`.copyWith(fontSize:` / `TextStyle(fontSize:` 都在内 ✓ |
| `width:`/`height:` 当间距用 | 规则刻意不拦（注释给了理由）；人工抽查 `app_shell.dart:588-591` 等处均为**元素自身尺寸**（图标 18 / 发送键 40×40 / 数值列 44–56）⇒ 与规则意图一致 ✓ |
| `EdgeInsets` 裸数字 | lint 规则覆盖 ✓（B6 已核） |

## 发现
- **F-0021-1（P2）** 行内码强制 `fontFamily: 'monospace'` 且**没有 `fontFamilyFallback`** ⇒ 码段里出现中文时，引擎会走**网络字体回退**（`fonts.gstatic.com`）——正是红线 L 要消灭的那次外部请求（项目自己有真机证据：`font_subset_test.dart:9-13` 记录过 `GET …/notosanssc/v37/…woff2 [200]`）。

## 本批核对过、不成发现的（正面记录）
- 字体族的**架构**是对的：单一入口 + 组件层零覆盖 + 刻意不在 TextTheme 里设 family（三处注释互相呼应），本次发现的是**一处漏配 fallback**，不是架构问题。
- 四套配色 + 12 字段 `AppColors` 的五件套齐全（B6 核过），本批复核 `radiusScale` 仍是「不进令牌面但进相等性/结构枚举」的正确处理。

## 本批未核实
- F-0021-1 的**触发频率**取决于模型是否输出「反引号包中文」（例如 `` `超时` ``、`` `未配置` ``）——真实项目里很常见，但本环境无法取样统计；机制层面已由项目自己的真机记录闭环。
