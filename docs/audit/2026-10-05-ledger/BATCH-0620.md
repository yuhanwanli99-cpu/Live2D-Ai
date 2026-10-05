# BATCH-0620 · flutter-checks.yml：paths 把自己也列进去

Phase 1 · 域覆盖 · CI

## 跑的命令（全部只读）
```
grep -nE "^name:|^on:|paths|run:|uses:" .github/workflows/flutter-checks.yml | head -8
sed -n '14,20p' .github/workflows/flutter-checks.yml
grep -rln "rust-ratio" .github/workflows/
```
未跑任何 CI 命令。

## 逐行
```
:11  name: Flutter Checks
:14  pull_request: branches: [main]
:16    paths:
:17      - "shell/flutter/**"
:18      - ".github/workflows/flutter-checks.yml"      # 把自己也列进去了
:19  workflow_dispatch:
:36  uses: subosito/flutter-action@v2
:43  run: flutter pub get
:47  run: flutter analyze
:51  run: flutter test
```

## 四个可核点
1. paths 里把自己这个 workflow 文件也列进去了
   => 改 CI 本身会触发这条 CI（否则「改门禁」这个动作会绕过门禁）。
   => 与 B0479 核的「例外被点名 + 回归被指路」同族：这里点名的是「谁该触发我」。
2. 三步门禁与 AGENTS 表逐条对应：pub get / analyze / test
   => 「flutter analyze && flutter test」在表里是一条，workflow 里是两步（pub get 是前置）。
3. `rust-ratio` 出现在**三个** workflow：pr-checks / flutter-checks / nightly
   => 而 AGENTS 表说它在 `pr-checks.yml` 与 `nightly.yml`
   => **⇒ flutter-checks 里也有一次 rust-ratio ⇒ 表与实现不一致 ⇒ 需核它那一次在干什么**
   （dart 目录在 xtask 的统计里虽不计入分母，但报告「单独打印 Dart 行数」——**假设**，**未核**）
4. `workflow_dispatch:` 存在 => 可手动触发
   => 与 B0612 核的 `-h|--help` 手工出口同族：每个自动化都有一个手工入口。

## 记为待核（第 3 点的两种可能，本批不下结论）
- 可能 A：flutter-checks 里的 rust-ratio 是**为了那份 Dart 行数报告**（AGENTS 写「xtask 报告会单独打印 Dart 行数以保证可见性」）；
- 可能 B：是**重复的门禁**（flutter 改动顺带跑一次 Rust 占比，无害但与 pr-checks 重复）。
⇒⇒ **两种都说得通 ⇒ 按 B0271 记为待核、不记发现。**
⇒⇒ 下一批第一件事：读 flutter-checks 里那次 rust-ratio 的上下文（哪一步、什么参数）。

## 未核
build-upload.yml / release-build.yml 头注与本体 · ISSUE_TEMPLATE 两份 · PULL_REQUEST_TEMPLATE.md ·
flutter-checks 里 rust-ratio 那一步 · 20-32 行（并发/超时设置）。
