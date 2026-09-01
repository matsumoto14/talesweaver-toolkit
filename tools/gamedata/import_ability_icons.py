"""ゲーム内インベントリのスクリーンショットから新装着アビリティの絵を切り出す。

新装着アビリティ(古代精霊 / 深淵 / 喪失 / 夜星)は Tale Wiki の
`Item/合成/装着アビリティシステム/新装着アビリティ` に表しか無く、添付画像が無い。
一方ゲーム内ではアイテム 1 個 = 1 マスで絵が出るので、そのスクリーンショットを
一次ソースにする(撮影 2026-09-01、ユーザー提供)。

絵は**系列ごとに 1 種類**で、同じ系列なら付く能力(生命力・鋭い刃…)が違っても
同じ絵になる。よって 1 枚を系列の全 id に配る。`Icon.svelte` は id から
機械的にファイルを引くので、マッピング表を作らずファイルを複製する。

マスにはインベントリの個数表示「1」が重なっている。これは装備の絵ではないので、
輪郭のティール色と、その輪郭に行内で挟まれた白を消して落とす。残りの枠と地は
中間色のグレーなので、外周からの塗りつぶしで透明にする。

使い方:
    python tools/gamedata/import_ability_icons.py --dry-run
    python tools/gamedata/import_ability_icons.py
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CATALOG_RS = ROOT / "crates/gamedata/src/equipment_catalog.rs"
SHOT = Path(__file__).resolve().parent / "screenshots/ability_items_134524.png"
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/equipment"

# 系列の id 接頭辞 → スクショ上のマス(左端 x, 右端 x)。1 マス 36px の等倍。
SERIES: dict[str, tuple[int, int]] = {
    "night-star": (0, 36),  # 夜星(星)
    "loss": (36, 72),  # 喪失(月)
}

# 個数表示「1」の輪郭。装備の絵は紫〜白なのでこの色域とは重ならない
DIGIT_OUTLINE = {(41, 60, 82), (74, 93, 107), (74, 89, 107), (66, 77, 107)}
# 枠と地のグレー。彩度がこの幅に収まる色を「絵ではない」とみなす
NEUTRAL_SPREAD = 20


def series_ids(prefix: str) -> list[str]:
    source = CATALOG_RS.read_text(encoding="utf-8")
    found = set(re.findall(rf'"({re.escape(prefix)}-[a-z-]+)"', source))
    return sorted(found)


def neutral(pixel) -> bool:
    r, g, b = pixel[:3]
    return max(r, g, b) - min(r, g, b) <= NEUTRAL_SPREAD


def cut(box: tuple[int, int]):
    from PIL import Image  # 切り出しのときだけ使う。アプリの依存には入れない

    image = Image.open(SHOT).convert("RGBA")
    width, height = image.size
    px = image.load()
    left, right = box
    clear = (0, 0, 0, 0)

    # 1. 個数表示を消す(輪郭 → 行内で輪郭に挟まれた白の順)
    outline = [
        (x, y)
        for y in range(height)
        for x in range(left, right)
        if px[x, y][:3] in DIGIT_OUTLINE
    ]
    for x, y in outline:
        px[x, y] = clear
    for row in {y for _, y in outline}:
        xs = sorted(x for x, y in outline if y == row)
        for x in range(xs[0], xs[-1] + 1):
            if px[x, row][:3] == (255, 255, 255):
                px[x, row] = clear

    # 2. 枠と地を外周から塗りつぶして透明にする(絵の中の白は外周とつながらない)
    stack = [(x, y) for x in range(width) for y in (0, height - 1)]
    stack += [(x, y) for y in range(height) for x in (0, width - 1)]
    seen: set[tuple[int, int]] = set()
    while stack:
        x, y = stack.pop()
        if (x, y) in seen or not (0 <= x < width and 0 <= y < height):
            continue
        seen.add((x, y))
        if px[x, y][3] == 0 or neutral(px[x, y]):
            px[x, y] = clear
            stack += [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]

    cell = image.crop((left, 0, right, height))
    return cell.crop(cell.getbbox())


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true", help="照合だけ行い画像を書き出さない")
    args = parser.parse_args()

    plan = {prefix: series_ids(prefix) for prefix in SERIES}
    for prefix, ids in plan.items():
        print(f"{prefix}: {len(ids)} 件 ({', '.join(ids)})")
        if not ids:
            print(f"equipment_catalog.rs に {prefix}-* の id が無い")
            return 1
    if args.dry_run:
        return 0

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    written = 0
    for prefix, ids in plan.items():
        icon = cut(SERIES[prefix])
        for item_id in ids:
            icon.save(OUTPUT_DIR / f"{item_id}.png", format="PNG", optimize=True)
            written += 1
        print(f"CUT {prefix} {icon.size[0]}x{icon.size[1]} -> {len(ids)} 枚")
    print(f"written={written}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
