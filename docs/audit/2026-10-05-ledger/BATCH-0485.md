# BATCH-0485 · ✅ **`ValueKey(_generation)` 是唯一的换节点键**

Phase 4 · 证伪（兑现 B0484 留的：读点是不是唯一）

## 跑的命令（全部只读）
```
grep -rn "ValueKey" lib/live2d/ lib/app/ --include=.dart | head -8
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**唯一性成立**（0 条新发现）
| 位置 | 是什么 |
|---|---|
| `lib/live2d/live2d_stage.dart:570` | `key: ValueKey<int>(_generation)` —— **舞台侧唯一** |
| `lib/app/page_cross_fade.dart:144` | `key: const ValueKey<String>('page-first-opacity')` —— **常量**、页面淡入 |
| `lib/app/page_cross_fade.dart:152` | `key: const ValueKey<String>('page-second-opacity')` —— **常量**、页面淡入 |

### 两个可核点
1. ⭐⭐⭐⭐⭐ **⇒ 舞台侧只有一处 `ValueKey`、而它带变量** ⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒⇒⇒ ⇒ ⇒⇒⇒ ⇒ 「换节点」这件事在舞台侧**只有一个入口**（B0484 核的「自增也只在一处」**合起来** = 入口唯一）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒ ⇒⇒⇒⇒** ⭐⭐⭐⭐⭐**
2. ⭐⭐⭐ **⇒ 而另两处是**常量** `ValueKey<String>`、与节点重建无关** ⇒⇒⭐⭐⭐
   ⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒⇒⇒ ⇒ 它们是**「区分两层」而不是「区分两次」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**
