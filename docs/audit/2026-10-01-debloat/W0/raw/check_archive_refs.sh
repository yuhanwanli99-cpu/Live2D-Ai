#!/usr/bin/env bash
# 独立「归档名残留」核验器（verifier 自写，非实施者产物）。
# 用法: bash check_archive_refs.sh [candidates.txt]
#
# 对候选清单里每个归档名，在 docs/ AGENTS.md README.md CONTRIBUTING.md 里搜三种形态：
#   A. `docs/plans/<name>`      —— W-S0 验收判据，归档后必须 **0 命中**
#   B. 裸文件名 <name>（排除 docs/legacy/**、候选清单自身、本审计目录）—— 信息性
#   C. 裸文件名 <name> 且落在 docs/legacy/**（归档件互引，历史层，非违规）—— 信息性
#
# 排除项：候选清单自身、本审计目录（否则脚本自己的输出会自命中）。
set -uo pipefail
CAND="${1:-docs/legacy/plans-archive-candidates-2026-10-01.txt}"
TARGETS=(docs AGENTS.md README.md CONTRIBUTING.md)
SELF_EXCL='^(docs/legacy/plans-archive-candidates-2026-10-01\.txt|docs/audit/2026-10-01-debloat/)'
A=0; B=0; C=0; checked=0
while IFS= read -r name; do
  [[ -z "$name" || "$name" == \#* ]] && continue
  checked=$((checked+1))
  # A: docs/plans/<name>
  a=$(grep -rn --include='*.md' --include='*.txt' -F "docs/plans/$name" "${TARGETS[@]}" 2>/dev/null | grep -Ev "$SELF_EXCL")
  if [[ -n "$a" ]]; then A=$((A+1)); echo "== [A·docs/plans/ 形态命中] $name"; echo "$a" | sed 's/^/     /' | head -12; fi
  # 全部裸名命中
  all=$(grep -rn --include='*.md' --include='*.txt' -F "$name" "${TARGETS[@]}" 2>/dev/null \
        | grep -Ev "$SELF_EXCL" | grep -v "docs/legacy/plans/$name")
  b=$(echo "$all" | grep -v '^docs/legacy/' | grep -v '^$')
  c=$(echo "$all" | grep '^docs/legacy/' | grep -v '^$')
  if [[ -n "$b" ]]; then B=$((B+1)); echo "== [B·裸名命中（活层）] $name"; echo "$b" | sed 's/^/     /' | head -12; fi
  if [[ -n "$c" ]]; then C=$((C+1)); echo "== [C·裸名命中（docs/legacy 历史层，信息性）] $name"; echo "$c" | sed 's/^/     /' | head -4; fi
done < "$CAND"
echo "---"
echo "checked_names=$checked"
echo "A_names_with_docsplans_ref=$A   <- 验收判据（须 0）"
echo "B_names_with_bare_ref_live=$B   <- 信息性"
echo "C_names_with_bare_ref_legacy=$C <- 信息性（历史层互引）"
echo "PASS_A=$([[ $A -eq 0 ]] && echo yes || echo no)"
