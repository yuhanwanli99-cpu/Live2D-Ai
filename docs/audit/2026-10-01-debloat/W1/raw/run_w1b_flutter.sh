#!/usr/bin/env bash
# W-VERIFY-1b：在 4 个已提交波次的**归档副本**上独立跑前端门禁（不在工作树跑）
set -uo pipefail
REPO=/home/skystar/Live2D-Ai-fe
OUT=$REPO/docs/audit/2026-10-01-debloat/W1/raw/w1b_flutter_all.txt
export PATH="$HOME/.cargo/bin:$HOME/flutter/bin:$PATH"
: > "$OUT"
for sha in cbbedef0 1d0c6e66 77da64e5 d140604f; do
  full=$(git -C "$REPO" rev-parse "$sha")
  dir="/tmp/w1b_$sha"
  rm -rf "$dir"; mkdir -p "$dir"
  git -C "$REPO" archive "$full" | tar -x -C "$dir"
  {
    echo "=================================================================="
    echo "### commit $sha ($full)"
    git -C "$REPO" log -1 --format='# %s' "$full"
    echo "# 归档副本: $dir"
    echo "# 副本完整性（改动文件 md5 vs commit）:"
    for f in $(git -C "$REPO" diff --name-only "$full^" "$full" -- shell/flutter | sort); do
      if [ -f "$dir/$f" ]; then
        a=$(git -C "$REPO" show "$full:$f" | md5sum | cut -d' ' -f1)
        b=$(md5sum "$dir/$f" | cut -d' ' -f1)
        printf '#   %-64s %s\n' "$f" "$([ "$a" = "$b" ] && echo SAME || echo DIFF)"
      fi
    done
    echo
    echo "\$ flutter pub get"
    (cd "$dir/shell/flutter" && flutter pub get) 2>&1 | tail -3
    echo "PUBGET_EXIT=$?"
    echo
    echo "\$ flutter analyze"
    (cd "$dir/shell/flutter" && flutter analyze) 2>&1 | tail -6
    echo "ANALYZE_EXIT=$?"
    echo
    echo "\$ flutter test"
    (cd "$dir/shell/flutter" && flutter test) 2>&1 | tail -4
    echo "TEST_EXIT=$?"
    echo
  } >> "$OUT" 2>&1
done
echo "DONE" >> "$OUT"
