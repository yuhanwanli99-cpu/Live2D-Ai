# Publication policy (Rust baseline)

Applies to the current public tree and **v0.7.0-rc.1** (Rust workspace under `crates/`).

## Default exclusions and current Bai exception

- Live2D model originals under `assets/models/` are excluded by default; user-imported models stay local.
- Secrets: `.env`, provider keys, tokens, chat history, caches, crash dumps and machine-local absolute paths.
- Live2D Cubism proprietary Core / Web Core binaries are never committed or bundled.
- **Exception:** the tracked tree and public v0.7.0-rc.1 Windows zip include Bai's required runtime files and terms.
- This is the maintainer's 2026-11-20 decision, verbatim: “bai依旧分发 作者反馈了再删除”.
- Bai's terms do not explicitly grant public redistribution; this is **not** a claim of authorization or legal compliance.
- The maintainer will remove Bai after the author raises the issue; this decision is recorded, not legal advice.

## Required gates (intent)

1. Relevant tests and project CI pass on the published revision; unperformed acceptance is not claimed.
2. Secret scan is clean on the history used for publication.
3. Archive review covers every exclusion above and explicitly identifies the Bai exception.
4. README / release notes state platform scope, included Bai files, unclear redistribution terms and removal rule.

Legacy Python PC Preview and Android publication rules are historical; see [legacy/README.md](legacy/README.md).
