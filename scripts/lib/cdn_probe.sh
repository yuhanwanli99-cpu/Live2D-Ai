#!/usr/bin/env bash
# 离线红线（不许依赖 Google CDN 的 CanvasKit）判据的**唯一真源**。
#
# 两个消费点：
#   - scripts/ignite.sh            （--check 走服务实吐字节 / --check-dir 走产物目录）
#   - scripts/ignition-precheck.sh （点火预检表里的「A. 托管层」）
# 两处都 source 本文件，不再各写一份清单 —— 2026-10-05 审计 F-0050-01 的教训正是
# 「同一份错误探针被复制到两处」（ignite.sh:89 与 ignition-precheck.sh:134）。
# **改本文件 = 同时改两个脚本的点火体检**，必须与两处调用点在同一个 commit 里抖动。
#
# 本文件只定义变量与函数，不做任何 IO；文件缺失时报错由调用方负责（两边都判红/退出，
# 绝不静默跳过）。
#
# ---- CDN 探针清单（唯一真源；--check 与 --check-dir 共用）-------------------
# 2026-10-05（审计 F-0050-01）：原清单只有 index.html / main.dart.js —— 实测这两个
# 文件对 gstatic\.com/flutter-canvaskit 在**当前正确构建**里命中 0，而真正的 CDN
# 决策在 flutter_bootstrap.js / flutter.js（各命中 1）。清单必须跟着**决策所在文件**
# 走，并且**分类判**——一刀切地 grep 那个串会把**正确的**构建也判红（实测：
# flutter_bootstrap.js 里恒有一段 !useLocalCanvasKit ? "https://www.gstatic.com/…"
# 的**死分支**，正确构建同样命中 1）。三类：
#   ① static：index.html / main.dart.js —— 出现 CDN 串 = 真引用 ⇒ 红；
#   ② config：flutter_bootstrap.js —— --no-web-resources-cdn 的落点是
#      "useLocalCanvasKit":true（实测：带该标志的构建有、不带则没有），按标志判；
#   ③ loader：flutter.js —— 决策由 ② 的配置喂进来，本身恒含同一段死分支；
#      这里只自保「文件存在」与「读该标志的接线仍在」：上游若改了 loader 形状，
#      ② 的标志断言会失去意义，此时必须判红让人回来重新推导，而不是继续绿。
# 附注（2026-10-05 实测）：main.dart.js 里另有 fonts.gstatic.com 的**运行期字体兜底**
# （CanvasKit 引擎行为，正确构建里同样存在，构建期消不掉）——它不是本探针的判据：
# 命中该串**不判红**；「运行期零外部请求」由真实浏览器 Network 断言负责
# （见 docs/verification/ 的浏览器验收报告）。谁也不要把它加进红模式。
# 清单非空 + 每个文件都必须拿到（HTTP 200 / 文件存在），拿不到即红——「扫不到
# 文件 = 红」是这条门禁的自保，防止清单被改空后退化成什么都不扫的假绿。
CDN_PROBE_SPEC="index.html:static main.dart.js:static flutter_bootstrap.js:config flutter.js:loader"
CDN_PATTERN='gstatic\.com/flutter-canvaskit'
CDN_REBUILD_HINT="cd shell/flutter && flutter build web --release --base-href /app/ --no-web-resources-cdn"

# 判定一段产物**内容**是否满足离线红线。$1=展示名 $2=类别 $3=内容。
# 返回 0 = ok（打印 [ok]），1 = 违规（打印 [FAIL] + 重修命令）。
# 内容经**参数**传入（调用方用 $(curl …) / $(cat …) 取），不走管道接 grep：
# set -o pipefail 下 printf 接 grep -q 会因为 grep 提前命中、上游收到
# SIGPIPE(141) 而把**命中**判成失败；here-string 没有这个坑。
cdn_judge() {
  local label="$1" kind="$2" body="$3"
  local effective
  # 配置形式的显式取值（loader 里的三元死分支不匹配这个形态）。
  effective=$(grep -Eo 'canvasKitBaseUrl"?[[:space:]]*:[[:space:]]*"[^"]*"' <<<"$body" | head -1)
  case "$kind" in
    static)
      if grep -q "$CDN_PATTERN" <<<"$body"; then
        echo "    [FAIL] $label 仍引用 Google CDN 的 CanvasKit —— 断网会白屏"
        echo "           重新构建：$CDN_REBUILD_HINT"
        return 1
      fi
      echo "    [ok]   $label 不依赖 Google CDN"
      ;;
    config)
      # 判据是**生效路径**，不是「文件里有没有那个串」：loader 的三元死分支
      # （canvasKitBaseUrl?n.canvasKitBaseUrl:…!useLocalCanvasKit?…gstatic…）
      # 在正确构建里恒在，按串判会天天红。只看**配置形式**的显式取值：
      #   canvasKitBaseUrl":"https://…  → 生效即远端 ⇒ 红
      #   canvasKitBaseUrl":"canvaskit/" → 生效即本地 ⇒ 绿
      if grep -Eq 'canvasKitBaseUrl"?[[:space:]]*:[[:space:]]*"https?://' <<<"$body"; then
        echo "    [FAIL] $label 生效的 canvasKitBaseUrl 指向远端地址 —— 断网会白屏"
        echo "           实际取值：$(grep -Eo 'canvasKitBaseUrl"?[[:space:]]*:[[:space:]]*"[^"]*"' <<<"$body" | head -1)"
        echo "           重新构建：$CDN_REBUILD_HINT"
        return 1
      fi
      if [ -n "$effective" ]; then
        echo "    [ok]   $label 生效的 $effective（本地相对路径）"
        return 0
      fi
      # 没有显式 canvasKitBaseUrl 时，生效路径由 --no-web-resources-cdn 的标志决定：
      # 该标志把 useLocalCanvasKit 写进 bootstrap 配置；缺它 ⇒ 引擎回落到上面的
      # gstatic 默认值（实测：带标志的构建有这句，不带则没有）。
      if ! grep -q '"useLocalCanvasKit":true' <<<"$body"; then
        echo "    [FAIL] $label 既无本地 canvasKitBaseUrl、也未见 \"useLocalCanvasKit\":true —— 构建很可能漏了 --no-web-resources-cdn（落 Google CDN 分支，断网会白屏）"
        echo "           重新构建：$CDN_REBUILD_HINT"
        return 1
      fi
      echo "    [ok]   $label \"useLocalCanvasKit\":true（不落 Google CDN 分支）"
      ;;
    loader)
      if ! grep -q 'useLocalCanvasKit' <<<"$body"; then
        echo "    [FAIL] $label 不再含 useLocalCanvasKit 接线 —— 上游 loader 形状变了，flutter_bootstrap.js 的标志断言已失去意义，请重新推导探针"
        return 1
      fi
      echo "    [ok]   $label 仍读 useLocalCanvasKit（决策由 flutter_bootstrap.js 的配置给出）"
      ;;
    *)
      echo "    [FAIL] 未知探针类别：$kind（自保：类别写错即红，不静默跳过）"
      return 1
      ;;
  esac
  return 0
}

# --check：只看服务实际吐出的字节（与浏览器拿到的一致），不看本地文件。
cdn_scan_http() {
  local base="$1" fail=0 scanned=0 spec f kind code body
  if [ -z "$CDN_PROBE_SPEC" ]; then
    echo "    [FAIL] CDN 探针清单为空 —— 没有任何文件可扫（自保）"
    return 1
  fi
  for spec in $CDN_PROBE_SPEC; do
    IFS=':' read -r f kind <<<"$spec"
    scanned=$((scanned + 1))
    code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 15 "$base/app/$f" 2>/dev/null || echo 000)
    if [ "$code" != "200" ]; then
      echo "    [FAIL] GET /app/$f 期望 200，实得 $code —— 探针文件扫不到即判红（不静默跳过）"
      fail=1
      continue
    fi
    body=$(curl -s --max-time 15 "$base/app/$f" 2>/dev/null || true)
    cdn_judge "/app/$f" "$kind" "$body" || fail=1
  done
  echo "    （CDN 探针共扫 $scanned 个产物文件）"
  [ "$fail" = "0" ]
}

# --check-dir：同样的判定，直接看**构建产物目录**（CI 用，不需要服务）。
cdn_scan_dir() {
  local dir="$1" fail=0 scanned=0 spec f kind body
  if [ -z "$CDN_PROBE_SPEC" ]; then
    echo "    [FAIL] CDN 探针清单为空 —— 没有任何文件可扫（自保）"
    return 1
  fi
  if [ ! -d "$dir" ]; then
    echo "    [FAIL] 产物目录不存在：$dir"
    return 1
  fi
  for spec in $CDN_PROBE_SPEC; do
    IFS=':' read -r f kind <<<"$spec"
    scanned=$((scanned + 1))
    if [ ! -f "$dir/$f" ]; then
      echo "    [FAIL] 产物缺失：$dir/$f —— 探针文件扫不到即判红（不静默跳过）"
      fail=1
      continue
    fi
    body=$(cat "$dir/$f" 2>/dev/null || true)
    cdn_judge "$dir/$f" "$kind" "$body" || fail=1
  done
  echo "    （CDN 探针共扫 $scanned 个产物文件）"
  [ "$fail" = "0" ]
}

