r"""クライアント展開データのモンスター立ち絵を敵カタログの id 名で同梱する。

`crates/gamedata/src/enemies.rs` の各行の `client_image`(`monster_<MonsterId>` / `book_<図鑑Id>`)を読み、
透明で正方形に詰めて `apps/desktop/src/assets/icons/mobs/<id>.png` へ書く。id の解決は
`ui/Icon.svelte` が機械的に行うので、マッピング表は作らない(assets/icons/README.md)。
`client_image` が `None` の敵は `?` 表示のまま。

- `book_*` は `<tw_assets>/item_icons/monsters/book_<Id>_<名前>.png`(図鑑アイコン)をそのまま使う
- `monster_*` は `monsters.csv` の anim_id / anim_index から d2a を読み、**正面向き(方向キー 8)で
  不透明部分が最大のコマ**を `<tw_assets>/sprites/` から取る。`item_icons/monsters/monster_*.png` は最初の方向の
  最初のコマなので、後ろ向きや出現エフェクトになっていることがある。d2a の解析は
  `tw_tool_v2/tools/dat_item_icons.py` の `parse_d2a` を借りる

使い方:
    python tools/gamedata/import_enemy_icons.py [--assets PATH] [--tool PATH]
省略時は環境変数 `TW_ASSETS` / `TW_TOOL_V2`、それも無ければ
`C:\github\private\tw_assets` / `C:\github\private\tw_tool_v2`。
"""

from __future__ import annotations

import argparse
import csv
import glob
import os
import re
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
ENEMIES_RS = ROOT / "crates/gamedata/src/enemies.rs"
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/mobs"

ENTRY_RE = re.compile(r'id:\s*"([^"]+)".*?client_image:\s*Some\("([^"]+)"\)')
FRONT_DIR_KEY = 8


def default_assets_dir() -> Path:
    env = os.environ.get("TW_ASSETS")
    return Path(env) if env else Path(r"C:\github\private\tw_assets")


def default_tool_dir() -> Path:
    env = os.environ.get("TW_TOOL_V2")
    return Path(env) if env else Path(r"C:\github\private\tw_tool_v2")


def square(img: Image.Image) -> Image.Image:
    img = img.convert("RGBA")
    side = max(img.size)
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(img, ((side - img.width) // 2, (side - img.height) // 2))
    return canvas


def monster_frame(assets: Path, parse_d2a, row: dict) -> Path:
    """正面向き(FRONT_DIR_KEY)で不透明部分が最大のコマの sprite PNG。正面が無ければ最初の方向で代用"""
    anim_id, index = int(row["anim_id"]), int(row["anim_index"])
    pattern = str(assets / "raw" / "dt_*" / "AnimationFile" / f"{anim_id // 100:04d}" / f"{anim_id:04d}.d2a")
    d2a = glob.glob(pattern)
    if not d2a:
        sys.exit(f"{row['name']}: AnimationFile {anim_id} が無い")
    anim = parse_d2a(Path(d2a[0]).read_bytes())["anims"][index]
    dirs = sorted(anim["dirs"], key=lambda d: d["key"] != FRONT_DIR_KEY)
    for d in dirs:
        candidates = []
        for track in d["tracks"]:
            for c in track:
                if c[0] != "spr":
                    continue
                path = assets / "sprites" / f"{c[1] // 100:04d}__{c[1]:04d}_f{c[2]:03d}.png"
                if path.exists():
                    with Image.open(path) as img:
                        bbox = img.convert("RGBA").getbbox()
                    area = (bbox[2] - bbox[0]) * (bbox[3] - bbox[1]) if bbox else 0
                    candidates.append((area, path))
        if candidates:
            # 本体以外のトラック(飛び散る破片など)を拾わないよう、不透明部分が最大のコマを選ぶ
            return max(candidates, key=lambda c: c[0])[1]
    sys.exit(f"{row['name']}: sprite コマが無い")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--assets", type=Path, default=default_assets_dir())
    parser.add_argument("--tool", type=Path, default=default_tool_dir())
    args = parser.parse_args()
    monsters_dir = args.assets / "item_icons" / "monsters"
    if not monsters_dir.is_dir():
        sys.exit(f"モンスター立ち絵が見つからない: {monsters_dir}")
    sys.path.insert(0, str(args.tool / "tools"))
    from dat_item_icons import parse_d2a  # noqa: E402

    with open(monsters_dir / "monsters.csv", encoding="utf-8-sig", newline="") as f:
        monsters = {f"{r['kind']}_{r['id']}": r for r in csv.DictReader(f)}

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    text = ENEMIES_RS.read_text(encoding="utf-8")
    written = 0
    for enemy_id, client_image in ENTRY_RE.findall(text):
        row = monsters.get(client_image)
        if row is None:
            sys.exit(f"{enemy_id}: {client_image} が monsters.csv に無い")
        if client_image.startswith("book_"):
            source = monsters_dir / row["icon_file"]
        else:
            source = monster_frame(args.assets, parse_d2a, row)
        with Image.open(source) as img:
            square(img).save(OUTPUT_DIR / f"{enemy_id}.png")
        written += 1
        print(f"{enemy_id} <- {row['name']} ({source.name})")
    print(f"written: {written}")


if __name__ == "__main__":
    main()
