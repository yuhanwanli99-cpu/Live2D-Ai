# Legacy legal / publication notes (pointers only)

The **authoritative** license scope for the current product is the Rust baseline:

- [LICENSE-SCOPE.md](../../../LICENSE-SCOPE.md)
- [NOTICE](../../../NOTICE)
- [publication.md](../publication.md)

The following paths described older Python (`Live2D-Ai-pc/`) and Android stacks, Cubism Web Core user-download flows, Pixi-based PC renderer, and PC-only Preview gates (`uv` / npm). They are **not** the Rust `0.1.0-rc.1` baseline.

| Historical topic | Where it lived / how to recover |
|---|---|
| PC Preview publication (Python) | Former `docs/legal/pc-preview-publication.md` (replaced by `../publication.md`). Recover prior text from git history. |
| Cubism Web Core “user downloads, not redistributed” | Former `NOTICE` / `LICENSE-SCOPE.md` sections. Rust baseline uses Ayagami instead. |
| Android archive | [ANDROID_ARCHIVE_POINTER.md](../../../ANDROID_ARCHIVE_POINTER.md) |
| Python tag | git tag `py-legacy` (see root README) |

Do not treat Cubism proprietary redistribution rules from the old PC stack as requirements of the current Ayagami-based renderer — but also do **not** start shipping Cubism Core in this repo.
