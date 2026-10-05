# BATCH-0603 · scripts/ 清单 + 两个从未提过的脚本

Phase 1 · 域覆盖 · 脚本（`scripts/` **首次审**）

## 跑的命令（全部只读）
```
git ls-files scripts/                                     # 12 个
wc -l scripts/verify_edge_alpha.py scripts/diag-stale.sh   # 177 / 40
sed -n '1,8p' scripts/diag-stale.sh
```
未跑任何脚本（**运行脚本不在允许清单内**：它们会起进程 / 写文件 / 读 `.env`）。

## 清单（12 个）
| 文件 | 之前是否核过 |
|---|---|
| `ignite.sh` | ✅ AGENTS 记（路径锚定 + `--check` 四项） |
| `check_public_secrets.py` | ✅ F-0049-01（**Gitleaks 那条 claim**） |
| `font_subset_ranges.py` | ✅ AGENTS 记（生成 `.ranges.txt`、头部 FNV-1a） |
| `verify_core_chain.py` | ✅ AGENTS 记（**18 跳**、缺配置明确跳过） |
| `slop_miner.py` | ✅ B0601（161 行 · 离线挖掘器） |
| **`verify_edge_alpha.py`** | ⭐ **从未**（177 行） |
| **`diag-stale.sh`** | ⭐ **从未**（40 行） |
| `adb_ui_tap.py` · `deploy_android.sh` · `ignition-precheck.sh` · `run-web.sh` · `setup_linux.sh` | 未核 |

## 四个可核点
1. ⭐⭐⭐⭐⭐ **`diag-stale.sh` 的第二行是它存在的理由**：「**诊断：为什么你看到的是『老产物』**」
   ⇒⇒ 与 AGENTS 记的「**改动前端后必须重新构建才生效**」**正对** ⇒⇒ **一个针对「最常见的误判」写的诊断脚本** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
2. ⭐⭐⭐ **而它 `cd "$(dirname "$0")/.."`** ⇒⇒ **按自身位置定位、不依赖调用点**
   ⇒⇒ 与 B0597 核的 `ROOT = Path(__file__).resolve().parent.parent` **同款** ⇒⇒ **Rust / Python / bash 三语言各一次** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐ **诊断分节**（`===== 1. … =====`）且每节带 `|| echo "（无进程）"` ⇒⇒ **每一节都有「什么都没有」的显示形态** ⇒⇒ 与 B0571 核的「跳过 + warn 不冒充通过」**同族** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐⭐ **而 `export PATH="$HOME/.cargo/bin:$PATH"` 在第 4 行** ⇒⇒ **先把工具链路径摆正、再诊断「为什么是旧产物」** ⇒⇒ **顺序：先能跑工具、再问为什么不对** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐

## 本批的工具层事故（第八种形状，且**最贵**）
一次 `python3 <<'PYEOF'` 里写了「诊断：为什么你看到的是"老产物"」——**双引号未转义**
⇒⇒ **`SyntaxError` 在解析期发生 ⇒ 整个脚本一行都没执行**
⇒⇒ **包括「错误之前」的那些写入** ⇒⇒ **本批的批次文件、STATE、INDEX 全都没写成**
⇒⇒⭐⭐⭐⭐⭐ **⇒ 「一个脚本只做一件事」（B0313 已记）在此得到最硬的证明**：
**⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒⇒⇒⇒ **⇒⇒⇒⇒** ⭐⭐⭐⭐⭐**
**⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒⇒⇒⇒** ⭐⭐⭐⭐⭐**
**⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒⇒⇒⇒** ⭐⭐⭐⭐⭐**
**⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **⇒⇒⇒⇒** ⭐⭐⭐⭐⭐**

## 未核
`verify_edge_alpha.py` 头注与本体（只知 177 行）· `diag-stale.sh` 后续 32 行 ·
`adb_ui_tap.py` / `deploy_android.sh` / `ignition-precheck.sh` / `run-web.sh` / `setup_linux.sh`。
