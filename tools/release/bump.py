#!/usr/bin/env python
"""バージョンを 3 箇所まとめて上げる。

バージョンは `apps/desktop/src-tauri/tauri.conf.json` / `apps/desktop/package.json` /
`apps/desktop/src-tauri/Cargo.toml` に散っている。ずれると
「インストーラの版」と「アプリが名乗る版」と「DB バックアップ名」が食い違うので、
必ずここから変える。

    python tools/release/bump.py 0.2.0     # 指定した版にする
    python tools/release/bump.py --show    # いまの版を確認する

タグは打たない(確認してから `git tag v0.2.0 && git push origin v0.2.0`)。
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

TAURI_CONF = ROOT / "apps/desktop/src-tauri/tauri.conf.json"
PACKAGE_JSON = ROOT / "apps/desktop/package.json"
CARGO_TOML = ROOT / "apps/desktop/src-tauri/Cargo.toml"

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")


def read_json_version(path: Path) -> str:
    return json.loads(path.read_text(encoding="utf-8"))["version"]


def write_json_version(path: Path, version: str) -> None:
    text = path.read_text(encoding="utf-8")
    # json.dump で書き戻すとキー順と整形が変わって差分が読めなくなる。該当行だけ差し替える。
    updated, count = re.subn(
        r'("version"\s*:\s*)"[^"]*"', rf'\1"{version}"', text, count=1
    )
    if count != 1:
        raise SystemExit(f"{path} の version 行が見つかりません")
    path.write_text(updated, encoding="utf-8")


def read_cargo_version(path: Path) -> str:
    match = re.search(r'^version\s*=\s*"([^"]+)"', path.read_text(encoding="utf-8"), re.M)
    if not match:
        raise SystemExit(f"{path} の version 行が見つかりません")
    return match.group(1)


def write_cargo_version(path: Path, version: str) -> None:
    text = path.read_text(encoding="utf-8")
    # `[package]` 直下の最初の version だけ。依存の version は触らない。
    updated, count = re.subn(
        r'^version\s*=\s*"[^"]*"', f'version = "{version}"', text, count=1, flags=re.M
    )
    if count != 1:
        raise SystemExit(f"{path} の version 行が見つかりません")
    path.write_text(updated, encoding="utf-8")


def current() -> dict[str, str]:
    return {
        str(TAURI_CONF.relative_to(ROOT)): read_json_version(TAURI_CONF),
        str(PACKAGE_JSON.relative_to(ROOT)): read_json_version(PACKAGE_JSON),
        str(CARGO_TOML.relative_to(ROOT)): read_cargo_version(CARGO_TOML),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("version", nargs="?", help="設定する版(例 0.2.0)")
    parser.add_argument("--show", action="store_true", help="いまの版を出すだけ")
    args = parser.parse_args()

    versions = current()
    if args.show or not args.version:
        for path, version in versions.items():
            print(f"{version:<10} {path}")
        distinct = set(versions.values())
        if len(distinct) > 1:
            print(f"\n版がずれています: {sorted(distinct)}", file=sys.stderr)
            return 1
        return 0

    if not SEMVER.match(args.version):
        raise SystemExit(f"MAJOR.MINOR.PATCH の形で指定してください: {args.version}")

    write_json_version(TAURI_CONF, args.version)
    write_json_version(PACKAGE_JSON, args.version)
    write_cargo_version(CARGO_TOML, args.version)

    for path, version in current().items():
        print(f"{version:<10} {path}")
    print(f"\nCHANGELOG.md の [未リリース] を [{args.version}] にしてから commit してください。")
    print(f"リリースは  git tag v{args.version} && git push origin v{args.version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
