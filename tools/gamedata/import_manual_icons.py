r"""機械的に解決できないコンテンツ / 敵の絵を、手で決めた出どころから同梱する。

通常は `import_content_images.py`(ゲーム内メダル)と `import_enemy_icons.py`
(`enemies.rs` の `client_image` → クライアントの立ち絵)で足りる。ここは残りの 2 種類だけを扱う。

- `FROM_MONSTER`: 敵データを持たないコンテンツ(= `mobs/` のフォールバックも効かない)に、
  そのコンテンツに出るモンスターの立ち絵を当てる
- `FROM_ICON`: 3D モデルで 2D 立ち絵が無い敵に、ゲーム内メダルの絵を流用する。
  `enemies.rs` 側は `client_image: None` にしてあるので `import_enemy_icons.py` とはぶつからない

使い方:
    python tools/gamedata/import_manual_icons.py [--assets PATH] [--tool PATH]
"""

from __future__ import annotations

import argparse
import csv
import glob
import os
import shutil
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
ICONS = ROOT / "apps/desktop/src/assets/icons"
FRONT_DIR_KEY = 8

# (系統ディレクトリ, id) → MonsterId。アビスは 4 段とも深淵の使徒で揃える
# (ヘル = 第2使徒 / EX = 第1使徒 は `enemies.rs` の `client_image` 側。第3使徒は 2D の絵が無い)
FROM_MONSTER: dict[tuple[str, str], str] = {
    ("contents", "abyss_normal"): "2107407",  # 深淵の第4使徒
    ("contents", "abyss_hard"): "2107408",  # 深淵の第5使徒
    ("contents", "shinchou_normal"): "2107482",  # 神鳥
    ("contents", "shinchou_hard"): "2107482",  # 神鳥
    ("contents", "orlie_defense_hell"): "2103048",  # バンダレックス・ダ・アノマラド
}

# キシニクは 3D モデルで 2D 立ち絵が無い。アフェティリア(ハード / EX)のメダルが本人の絵なので、
# キシニクが出る面(アフェティリアの 2 段 + レリックの聖域 10〜20段)すべてでこれを使う
KISINIK_ICON = ICONS / "contents/aphetiria_ex.png"
FROM_ICON: dict[tuple[str, str], Path] = {
    **{("contents", f"relic_sanctuary_{n}"): KISINIK_ICON for n in range(10, 21)},
    **{("mobs", f"relic_sanctuary_{n}"): KISINIK_ICON for n in range(10, 21)},
    ("mobs", "kisinik_h"): KISINIK_ICON,
    ("mobs", "kisinik_ex"): KISINIK_ICON,
}


# アナイスの魔法人形(ミカベア / ルシベア)の絵。人形は 3D モデルで 2D 立ち絵が無いので、
# 召喚スキル「ミカベア召喚」「ルシベア召喚」(クライアント DB 0006 の 3002602 / 3002603。アイコン指定
# {"id":95596,"w":28|29} → アニメ 95596 の 28 / 29 番 → テクスチャ 13322 のコマ 53 / 55)を使う。
# gamedata には召喚スキルが無い(攻撃スキルだけ)ので、id は「あれば付くはずの id」で置く。
# 計算タブの熊の鎖のバッジが `skills/` から引く(docs/adr/016)
FROM_SPRITE: dict[tuple[str, str], str] = {
    ("skills", "anais_mica_bear_summon"): "0133__13322_f053",
    ("skills", "anais_rucy_bear_summon"): "0133__13322_f055",
}


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
    """正面向き(FRONT_DIR_KEY)で不透明部分が最大のコマ。`import_enemy_icons.py` と同じ選び方"""
    anim_id, index = int(row["anim_id"]), int(row["anim_index"])
    pattern = str(assets / "raw" / "dt_*" / "AnimationFile" / f"{anim_id // 100:04d}" / f"{anim_id:04d}.d2a")
    d2a = glob.glob(pattern)
    if not d2a:
        sys.exit(f"{row['name']}: AnimationFile {anim_id} が無い")
    anim = parse_d2a(Path(d2a[0]).read_bytes())["anims"][index]
    for direction in sorted(anim["dirs"], key=lambda d: d["key"] != FRONT_DIR_KEY):
        candidates = []
        for track in direction["tracks"]:
            for cmd in track:
                if cmd[0] != "spr":
                    continue
                path = assets / "sprites" / f"{cmd[1] // 100:04d}__{cmd[1]:04d}_f{cmd[2]:03d}.png"
                if path.exists():
                    with Image.open(path) as img:
                        bbox = img.convert("RGBA").getbbox()
                    area = (bbox[2] - bbox[0]) * (bbox[3] - bbox[1]) if bbox else 0
                    candidates.append((area, path))
        if candidates:
            return max(candidates, key=lambda c: c[0])[1]
    sys.exit(f"{row['name']}: sprite コマが無い(3D モデルの敵)")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--assets", type=Path, default=default_assets_dir())
    parser.add_argument("--tool", type=Path, default=default_tool_dir())
    args = parser.parse_args()

    sys.path.insert(0, str(args.tool / "tools"))
    from dat_item_icons import parse_d2a  # noqa: E402

    monsters_csv = args.assets / "item_icons" / "monsters" / "monsters.csv"
    with open(monsters_csv, encoding="utf-8-sig", newline="") as f:
        monsters = {r["id"]: r for r in csv.DictReader(f) if r["kind"] == "monster"}

    written = 0
    for (kind, icon_id), frame in FROM_SPRITE.items():
        source = args.assets / "sprites" / f"{frame}.png"
        if not source.exists():
            sys.exit(f"{kind}/{icon_id}: {source.name} が sprites/ に無い")
        dest = ICONS / kind / f"{icon_id}.png"
        dest.parent.mkdir(parents=True, exist_ok=True)
        with Image.open(source) as img:
            square(img).save(dest)
        written += 1
        print(f"{kind}/{icon_id} <- {source.name}")
    for (kind, icon_id), monster_id in FROM_MONSTER.items():
        row = monsters.get(monster_id)
        if row is None:
            sys.exit(f"{kind}/{icon_id}: MonsterId {monster_id} が monsters.csv に無い")
        source = monster_frame(args.assets, parse_d2a, row)
        dest = ICONS / kind / f"{icon_id}.png"
        dest.parent.mkdir(parents=True, exist_ok=True)
        with Image.open(source) as img:
            square(img).save(dest)
        written += 1
        print(f"{kind}/{icon_id} <- {row['name']} ({source.name})")

    for (kind, icon_id), source in FROM_ICON.items():
        if not source.exists():
            sys.exit(f"{kind}/{icon_id}: 流用元 {source} が無い")
        dest = ICONS / kind / f"{icon_id}.png"
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, dest)
        written += 1
        print(f"{kind}/{icon_id} <- {source.relative_to(ICONS)}")

    print(f"written: {written}")


if __name__ == "__main__":
    main()
