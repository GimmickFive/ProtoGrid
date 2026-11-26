#!/usr/bin/env python3
"""Regenerate docs/third-party-rust.md from Cargo metadata.

Usage: python3 scripts/third-party.py
"""

import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "third-party-rust.md"


def main() -> int:
    raw = subprocess.check_output(
        ["cargo", "metadata", "--format-version", "1", "--locked"], cwd=ROOT
    )
    packages = json.loads(raw)["packages"]
    deps = [p for p in packages if p.get("source") and p.get("license")]
    unknown = sorted(
        {f"{p['name']} {p['version']}" for p in packages if p.get("source") and not p.get("license")}
    )

    lines = [
        "# Rust dependencies",
        "",
        "Сгенерировано из `Cargo.lock` скриптом `scripts/third-party.py`.",
        "Не редактировать вручную.",
        "",
        f"Всего пакетов: {len(deps)}.",
        "",
        "| Пакет | Версия | Лицензия |",
        "|---|---|---|",
    ]
    for p in sorted(deps, key=lambda p: p["name"].lower()):
        lines.append(f"| `{p['name']}` | {p['version']} | {p['license']} |")

    if unknown:
        lines += ["", "## Без указанной лицензии", ""]
        lines += [f"- {u}" for u in unknown]

    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT} ({len(deps)} packages)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
