#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Validate shared/model_registry.json against shared/model_registry.schema.json.

Checks:
  1. Registry JSON is valid JSON and conforms to the JSON Schema (draft 2020-12).
     Schema violations SHORT-CIRCUIT: a readable report is printed and the
     script exits 1 immediately, so malformed input never reaches the
     type-dependent code below (no bare TypeError/AttributeError crashes).
  2. Data invariants (only run on schema-valid input):
     - exactly one entry has default=true
     - the default entry is available=true
     - model ids are unique
     - each model3JsonPath directory prefix matches its model id
  3. Every available=true entry's model3.json exists on each declared platform
     root. Platform roots are opt-in via env (VALIDATE_REGISTRY_PLATFORM_ROOTS,
     JSON like {"android": "/abs/path", "pc": "/abs/path"}); when unset or any
     root is missing on disk, that platform's file check is skipped (warn).
     History: Live2D-Ai-Android / Live2D-Ai-pc have been archived; the legacy
     hard-coded platform roots are removed. Re-declare via env when the
     corresponding codebases are restored locally.

Usage:  python shared/validate_registry.py
Exit:   0 = all checks passed, 1 = validation failed, 2 = environment error
"""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY_PATH = ROOT / "shared" / "model_registry.json"
SCHEMA_PATH = ROOT / "shared" / "model_registry.schema.json"


def _load_platform_roots() -> dict[str, Path]:
    """Load opt-in platform roots from env (JSON object).

    Returns an empty dict when unset / unparsable — file-existence step then
    no-ops, keeping schema + invariant checks runnable on archived repos.
    """
    raw = os.environ.get("VALIDATE_REGISTRY_PLATFORM_ROOTS")
    if not raw:
        return {}
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        print(f"warn: VALIDATE_REGISTRY_PLATFORM_ROOTS 不是合法 JSON，已忽略: {raw!r}")
        return {}
    if not isinstance(data, dict):
        print(f"warn: VALIDATE_REGISTRY_PLATFORM_ROOTS 顶层必须是对象，已忽略: {raw!r}")
        return {}
    result: dict[str, Path] = {}
    for key, value in data.items():
        if not isinstance(key, str) or not isinstance(value, str) or not value:
            print(f"warn: 跳过非法 platform root 条目: {key!r}={value!r}")
            continue
        result[key] = Path(value)
    return result


PLATFORM_ROOTS: dict[str, Path] = _load_platform_roots()


def schema_error_path(error) -> str:
    """Render a jsonschema.ValidationError absolute path as a readable string."""
    parts = []
    for elem in error.absolute_path:
        parts.append(f"[{elem}]" if isinstance(elem, int) else f".{elem}")
    return "$" + "".join(parts) if parts else "$"


def main() -> int:
    errors: list[str] = []

    try:
        import jsonschema
    except ImportError:
        print("ERROR: 'jsonschema' package is required. Install with: python -m pip install jsonschema")
        return 2

    # 1. Parse + schema validation
    try:
        registry = json.loads(REGISTRY_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"ERROR: cannot read/parse {REGISTRY_PATH}: {exc}")
        return 1
    try:
        schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"ERROR: cannot read/parse {SCHEMA_PATH}: {exc}")
        return 1

    try:
        jsonschema.validate(instance=registry, schema=schema)
    except jsonschema.ValidationError as exc:
        errors.append(f"schema violation at {schema_error_path(exc)}: {exc.message}")
    except jsonschema.SchemaError as exc:
        print(f"ERROR: schema itself is invalid: {exc}")
        return 1

    # Schema is the first gate: any violation short-circuits here with a
    # readable report. Everything below only ever sees schema-valid input,
    # whose structure (top-level object, models as non-empty array of objects,
    # required string fields) is guaranteed by the schema — so no type guards
    # are needed and malformed input can never crash with a bare traceback.
    if errors:
        print("FAILED:")
        for err in errors:
            print(f"  - {err}")
        return 1

    print(f"OK  : {REGISTRY_PATH.name} conforms to {SCHEMA_PATH.name} (draft 2020-12)")

    models = registry.get("models", [])

    # 2. Data invariants (defense in depth; readable messages even if a
    #    schema keyword is silently ignored by an older jsonschema version)
    defaults = [m for m in models if m.get("default") is True]
    if len(defaults) != 1:
        errors.append(f"expected exactly one entry with default=true, found {len(defaults)}")
    elif not defaults[0].get("available"):
        errors.append(f"default entry '{defaults[0].get('id')}' must be available=true")

    ids = [m.get("id") for m in models]
    if len(ids) != len(set(ids)):
        dupes = sorted({i for i in ids if ids.count(i) > 1})
        errors.append(f"duplicate model ids: {dupes}")

    for m in models:
        mid = m.get("id")
        path = m.get("model3JsonPath")
        if isinstance(mid, str) and isinstance(path, str) and path.split("/", 1)[0] != mid:
            errors.append(
                f"model '{mid}': model3JsonPath directory prefix "
                f"'{path.split('/', 1)[0]}' does not match id"
            )

    # 3. File existence for available entries on both platforms
    print("--- resource file check (available entries) ---")
    for m in models:
        if m.get("available") is not True:
            print(f"skip: {m.get('id')} (unavailable/placeholder)")
            continue
        mid = m.get("id")
        rel = m.get("model3JsonPath")
        for platform, root in PLATFORM_ROOTS.items():
            target = root / rel
            if not root.is_dir():
                print(f"warn: [{platform}] root not found, skipped: {root}")
                continue
            if target.is_file():
                print(f"OK  : [{platform}] {rel}")
            else:
                errors.append(f"[{platform}] model3.json missing for '{mid}': {target}")

    if errors:
        print("\nFAILED:")
        for err in errors:
            print(f"  - {err}")
        return 1

    print(f"\nAll checks passed ({len(models)} entries in registry).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
