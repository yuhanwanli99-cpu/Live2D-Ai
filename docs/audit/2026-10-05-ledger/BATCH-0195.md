# BATCH-0195 · ⭐ **`_guard()` 只有 5 行** ⇒ 反证 (b) **彻底排除**；F-0189-01 调查**收口**

Phase 1 · 域覆盖 · `api_client.dart:229-249`（`_guard` / `_decodeObject` / `_errorFrom`）

## 跑的命令（全部只读）
```
grep -n "_guard(String|Response> _guard" -A 20 lib/api/api_client.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**F-0189-01 全部未决点结清**
```dart
Future<http.Response> _guard(Future<http.Response> Function() run) async {   // :229
  try { return await run(); }
  catch (error) { throw ApiException('network_error', '无法连接后端：$error'); }
}
```
⇒ **只做一件事：把传输异常翻译成 `ApiException`。**
⇒ **无超时、无重试、无去重、无串行化** ⇒ ⇒ **反证 (b) 不是「没找到」，是「彻底排除」**：
   客户端层**不提供任何**能给两个并发 GET **定序**的机制
⇒ ⇒ **F-0189-01 至此完全查清**（五批）：
| 维度 | 结论 |
|---|---|
| 实现 | **无守卫**（`grep epoch\|seq\|stale` 零命中） |
| 测试 | 组名声称该不变式，**体是顺序的** ⇒ 对该主张**结构上无法失败** |
| 触发 | 已枚举并排序：**① 连续保存两次覆盖**（最高）② init GET 在飞即保存 ③「重试」连点 |
| 反证 (a) | ❌ 推翻（`:224` `onPressed` 永远非空） |
| 反证 (b) | ❌ **彻底排除**（本批） |
| 反证 (c) | ⚠ 无法从仓库证明 ⇒ **维持 P2** |
| 危害 | **两个都成功**的 load 乱序 ⇒ **丢弃未保存草稿**（主）+ 显示旧值（次） |
| 修复 | **一个自增序号** + `await` 后早退，**不需取消机制**（与 B0040 `epoch` 闸同形） |

### 顺带两处形状核验
- ⭐ `_decodeObject` (:237-245)：**坏 JSON ⇒ 返回空 map，不抛** ⇒ 前端看到 `{}` 而非异常
  ⇒ 与表演层的「失败 ⇒ 回落 + 计数」（B0151 的 **P9**）**同形** —— **降级不留异常给 UI**
- `_errorFrom` (:247+) 从 `data['error']` 取详情 ⇒ 错误细节**由服务端体提供**
  ⇒ 与 B0015/B0161 核的「前端**从码/字段分流**、不猜文案」**一致**

## 未核实项
1. `_errorFrom` 全文未读（码如何映射到 `ApiException`）
2. `settings_controller.dart:120-222` / `:296-409` 其余未读；`:327` 那处置 `_draft` 未读
3. 新露出的 208 条自我设防名只看了 11 条；`dev_tools_section.dart`(1874) / `tokens.dart`(928) 未读
4. Mod crates 逐文件（约 53/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
5. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
6. `join_endpoint` 回归断言体 · `mod_count_is_five` 断言体 · `_finishTurn` 是否幂等 ·
   `secrets.rs` 断言体 · B0120「chmod toml」未核
