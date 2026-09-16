r"""`equipment_catalog/items.rs` に手書きで持つ装備(クライアント DB 由来の client.rs の行ではないので
`import_client_icons.py` の対象外)のアイコンをクライアント展開データから同梱する。

- レリック(`relic_item(...)`。神鳥・ルナリアのペンダント / ブレスレット)と
  アーティファクト(`artifact_item(...)` / `defensio_artifact(...)`)は行を読んで **名前** で
  `<tw_assets>/item_icons/items.csv` を引く。照合は † と空白を落とし NFKC で全角半角を揃える
  (エーテリアルチューブは client 側に † が無い)
- 表示名がゲーム内名と違う行は `RENAMED` に id → クライアントの名前を書く

複製先は `apps/desktop/src/assets/icons/equipment/<id>.png`。

使い方:
    python tools/gamedata/import_manual_item_icons.py [--assets PATH]
`--assets` 省略時は環境変数 `TW_ASSETS`、それも無ければ `C:\github\private\tw_assets`。
"""

from __future__ import annotations

import argparse
import csv
import os
import re
import shutil
import sys
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ITEMS_RS = ROOT / "crates/gamedata/src/equipment_catalog/items.rs"
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/equipment"

ENTRY_RE = re.compile(r'(?:relic_item|artifact_item|defensio_artifact)\(\s*"([^"]+)",\s*"([^"]+)"')


def normalize(name: str) -> str:
    return unicodedata.normalize("NFKC", name).replace("†", "").replace(" ", "")

# 表示名がゲーム内名と違う手書き行。id → クライアント DB の名前
RENAMED: dict[str, str] = {
    # 盾+ の成長カフス。表示名「†ライジングホリックカフス」はユーザー指定で、ゲーム内名はこちら
    "rising-holic-cuffs": "†ライゾシンボリックカフス",
}


def default_assets_dir() -> Path:
    env = os.environ.get("TW_ASSETS")
    return Path(env) if env else Path(r"C:\github\private\tw_assets")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--assets", type=Path, default=default_assets_dir())
    args = parser.parse_args()
    item_dir: Path = args.assets / "item_icons"
    csv_path = item_dir / "items.csv"
    if not csv_path.is_file():
        sys.exit(f"items.csv が見つかりません: {csv_path}")

    by_name: dict[str, str] = {}
    with csv_path.open(encoding="utf-8-sig", newline="") as f:
        for row in csv.DictReader(f):
            if row.get("status") == "ok":
                by_name.setdefault(normalize(row["name"]), row["icon_file"])

    entries = ENTRY_RE.findall(ITEMS_RS.read_text(encoding="utf-8")) + list(RENAMED.items())
    missing: list[str] = []
    for item_id, name in entries:
        icon_file = by_name.get(normalize(name))
        if icon_file is None:
            missing.append(f"{item_id}: {name}")
            continue
        shutil.copyfile(item_dir / icon_file, OUTPUT_DIR / f"{item_id}.png")
    print(f"{len(entries) - len(missing)} / {len(entries)} 件を複製")
    if missing:
        sys.exit("client に同名の行が無い:\n  " + "\n  ".join(missing))


if __name__ == "__main__":
    main()
