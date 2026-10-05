# BATCH-0467 · ✅ **B0466 的第 3 点错了** —— 而那份自述的判据是**「可达否」**

Phase 4 · 证伪（兑现 B0466 留的：自检那三个码有没有一份列出它们的地方）

## 跑的命令（全部只读）
```
grep -rn "protocol_error" --include=*.rs --include=*.md --include=*.dart . \
  | grep -vE "^\./target|^\./AUDIT-REPO" | head -6
sed -n '262,272p' crates/live2d-ai-desktop/src/web_api/settings_routes/test_endpoints.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0466 的第 3 点被证伪并撤回**（0 条新发现）
`test_endpoints.rs:262-270`：
> 「`GET {base}/models` 探针的结果分类（**纯函数**，可单测）。
> **口径（每条都有理由，别随手改）：**
> - `2xx` → 通；
> - `401`/`403` → `auth_failed`：**可达**，但 key 不对/缺失——这是自检最该…**
> - `404` → **算通过**，附一句说明：上游回了响应就证明「可达」，而 `/models`
>   **并非所有语音实现都提供**（只实现 `/audio/speech` 的也合法）。这里若判失败
>   就是又造一个「**自检说没通、实际能用**」的假失败——正是本次要消的
> - 其余 `4xx`/`5xx` → `protocol_error`（**可达但坏了**），带上响应片段。」

### 四个可核点
1. ⭐⭐⭐⭐⭐ **那份自述存在** —— 在两个探针函数的头注里（`:262-270` 与 `:312`）
   ⇒ B0466 写的「只有用法、没有自述」**不成立，撤回**。
2. ⭐⭐⭐⭐⭐ **而它的判据不是 `<stage>_<suffix>`、是「可达否」**：
   | 码 | 判据 | 序数 |
   |---|---|---|
   | `auth_failed` | **可达**，但 key 不对/缺失 | 2 |
   | （`404`） | **可达**，且未提供 `/models` ⇒ 算通过 | 2 |
   | `protocol_error` | **可达但坏了** | 4 |
   ⇒⇒ **三个非 2xx 的分支里有两个是「可达」** ⇒⇒ **「可达」是这套码表的**判据**，而对话链的码按**环节**分**。
3. ⭐⭐⭐⭐⭐ **而段首那句「**每条都有理由，别随手改**」把判据与禁令写在一行**
   ⇒⇒ 与 B0330（段级禁令 + 理由）、B0421（禁令自带成立条件）**同一族**。
4. ⭐⭐⭐⭐ **而 `probe_outcome` 自称「**纯函数**，可单测」** ⇒ 与 B0384「handler 调 Mod 纯函数」**同形**；
   而 B0464 核的那条回归（`:372`）就是它的单元测试。

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `setLibrary` 里那次 `clamp` 的实现 · `HttpTestError` 三个变体（`Runtime` / `BadUrl`）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
