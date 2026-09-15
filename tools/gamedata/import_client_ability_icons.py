"""クライアント展開データのアイテムアイコンを、装着アビリティの id 名で同梱する。

`crates/gamedata/src/equipment_catalog/abilities.rs` の `"<id>", "<name>"`(隣り合う 2 つの文字列
リテラル)を読み、`<tw_assets>/item_icons/<ItemId>_<名前>(Lv.N).png` を **名前で** 引いて
`apps/desktop/src/assets/icons/equipment/<id>.png` へ複製する。装着アビリティはゲーム内では
アイテム(月石・研磨・耐性…)で、名前がそのままアイテム名になっている。id の解決は
`ui/Icon.svelte` が機械的に行うので、マッピング表は作らない(assets/icons/README.md)。

既に画像がある id(夜星・喪失系はインベントリのスクショ由来)は上書きしない。
クライアントに同名アイテムが無いもの(旧武器アビリティ「(下)尖った刃」など)は `?` のまま。

使い方:
    python tools/gamedata/import_client_ability_icons.py [--assets PATH] [--dry-run]
`--assets` 省略時は環境変数 `TW_ASSETS`、それも無ければ `C:\\github\\private\\tw_assets`。
"""

from __future__ import annotations

import argparse
import os
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ABILITIES_RS = ROOT / "crates/gamedata/src/equipment_catalog/abilities.rs"
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/equipment"

# 名前は日本語(非 ASCII)を含む。id 同士が隣り合う引数(排他グループなど)を名前と誤読しない
PAIR_RE = re.compile(r'"([a-z0-9-]+)",\s*"([^"]*[^\x00-\x7f][^"]*)"')
ITEM_FILE_RE = re.compile(r"^(\d+)_(.+?)(?:\(Lv\.\d+\))?\.png$")


def default_assets_dir() -> Path:
    env = os.environ.get("TW_ASSETS")
    return Path(env) if env else Path(r"C:\github\private\tw_assets")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--assets", type=Path, default=default_assets_dir())
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    icon_dir = args.assets / "item_icons"
    if not icon_dir.is_dir():
        sys.exit(f"item_icons が見つからない: {icon_dir}")
    # 同名が複数(Lv 違い)あるときは ItemId の小さいほう(最初に出たもの)。絵は同じ
    by_name: dict[str, Path] = {}
    for path in sorted(icon_dir.glob("*_*.png")):
        m = ITEM_FILE_RE.match(path.name)
        if m:
            by_name.setdefault(m.group(2), path)

    pairs = dict(PAIR_RE.findall(ABILITIES_RS.read_text(encoding="utf-8")))
    copied: list[str] = []
    kept: list[str] = []
    missing: list[str] = []
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    for ability_id, name in pairs.items():
        dest = OUTPUT_DIR / f"{ability_id}.png"
        if dest.exists():
            kept.append(ability_id)
            continue
        src = by_name.get(name)
        if src is None:
            missing.append(f"{ability_id} ({name})")
            continue
        if not args.dry_run:
            shutil.copyfile(src, dest)
        copied.append(ability_id)

    print(f"アビリティ {len(pairs)} 件: 複製 {len(copied)} / 既存のまま {len(kept)} / クライアントに無い {len(missing)}")
    for line in missing:
        print(f"  ? {line}")


if __name__ == "__main__":
    main()
