"""クライアント展開データのアイテムアイコンを装備カタログの id 名で同梱する。

`crates/gamedata/src/equipment_catalog/client.rs` の各行(`source.page` に `ItemId <n>` を持つ)を読み、
`<tw_assets>/item_icons/<ItemId>_<名前>.png` を `apps/desktop/src/assets/icons/equipment/<id>.png` へ複製する。
id の解決は `ui/Icon.svelte` が機械的に行うので、マッピング表は作らない(assets/icons/README.md)。

†改・セイクリッド系は Rust 側(`assign_icon_ids`)が通常版の id へ寄せるので、ここでは行ごとに複製するだけでよい。
既存の wiki 由来の画像は同名で上書きする(絵はどちらもゲーム内のもの。出典を 1 つにする)。

使い方:
    python tools/gamedata/import_client_icons.py [--assets PATH]
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
CLIENT_RS = ROOT / "crates/gamedata/src/equipment_catalog/client.rs"
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/equipment"

ENTRY_RE = re.compile(
    r'id:\s*"([^"]+)".*?page:\s*"client DB [^"]*ItemId (\d+)"',
    re.DOTALL,
)


def default_assets_dir() -> Path:
    env = os.environ.get("TW_ASSETS")
    return Path(env) if env else Path(r"C:\github\private\tw_assets")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--assets", type=Path, default=default_assets_dir())
    args = parser.parse_args()

    icon_dir = args.assets / "item_icons"
    if not icon_dir.is_dir():
        sys.exit(f"item_icons が見つからない: {icon_dir}")
    by_item_id: dict[str, Path] = {}
    for path in icon_dir.glob("*_*.png"):
        item_id = path.name.split("_", 1)[0]
        if item_id.isdigit():
            by_item_id.setdefault(item_id, path)

    text = CLIENT_RS.read_text(encoding="utf-8")
    # 1 行分ずつ切って id と ItemId を対にする(構造体の先頭 `id:` から `source.page` まで)
    blocks = text.split("WikiEquipmentItem {")[1:]
    copied = 0
    missing: list[tuple[str, str]] = []
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    for block in blocks:
        match = ENTRY_RE.search(block)
        if not match:
            continue
        catalog_id, item_id = match.groups()
        source = by_item_id.get(item_id)
        if source is None:
            missing.append((catalog_id, item_id))
            continue
        shutil.copyfile(source, OUTPUT_DIR / f"{catalog_id}.png")
        copied += 1

    print(f"複製 {copied} 件 / アイコン無し {len(missing)} 件")
    for catalog_id, item_id in missing:
        print(f"  no icon: {catalog_id} (ItemId {item_id})")


if __name__ == "__main__":
    main()
