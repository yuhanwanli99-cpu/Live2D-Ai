# fonts/ —— 自托管中文字体子集

## 为什么在这里

Flutter Web（CanvasKit）**没有系统字体回落**：遇到未打包的字形会去
`https://fonts.gstatic.com/` 下载 Noto。于是断网时中文变豆腐块——
而本项目是**本地优先的桌宠**，不允许依赖 Google CDN（配套修复见
`CHANGELOG.md` 的 v0.4.4：CanvasKit 已本地化）。

## 这两个文件是什么

| 文件 | 字重 | 大小 |
| --- | --- | --- |
| `NotoSansSC-AiSubset-Regular.woff2` | 400 | ≈3.15 MB |
| `NotoSansSC-AiSubset-Bold.woff2` | 700 | ≈3.24 MB |

由 **Noto Sans SC 可变字体**（`google/fonts/ofl/notosanssc`）实例化为静态字重后
用 `fonttools` 子集化而来。

**覆盖集是「整个 CJK 统一表意区」而不是「UI 里出现过的字」**：本应用显示的是
LLM 的**任意输出**，只留 UI 用字必然在用户看到人名/生僻词时变豆腐块。
实际覆盖 20 976 个汉字，外加拉丁、通用标点、CJK 标点、假名、全角形式、圈号与几何符号。

复现：
```bash
python3 -m venv /tmp/fontenv && /tmp/fontenv/bin/pip install fonttools brotli
# 取可变字体 → instantiateVariableFont(wght=400/700) → subset(--unicodes=...) → woff2
```

## 许可（**再分发时必须保留**）

上游 `OFL.txt` 随本目录一起分发（OFL-1.1 条件 2 要求）。
Copyright 2014-2021 Adobe (<http://www.adobe.com/>), with Reserved Font Name 'Source'。

- 子集化属于 OFL 定义的 **Modified Version**；OFL 条件 3 限制的是**使用保留字体名
  `'Source'` 本身**，而本字体家族名 `Noto Sans SC AiSubset` 不含该名，故合规。
- 内部字体名已由实例化脚本显式改写：可变字体默认实例的内部名是
  **`Noto Sans SC Thin`**（wght 默认 100），不改写会得到一个「名字写着 Thin、
  实际是 400」的字体。
- 全量字体不随仓库分发，只分发上述子集。
