"""ゲーム内「Content information」のスクリーンショットからコンテンツ画像を切り出す。

ホーム「どこまでいける?」の一覧行に添える 28px の識別子なので、
**コンテンツ 1 件につき 1 枚**・**小さくても判別できる絵**が要る。
Tale Wiki の「ミニゲーム/*」ページはマップとドロップ品しか持っておらず、
コンテンツ単位の絵が無い(2026-09-01 に全ページを走査して確認)。
一方ゲーム内の「Content information → コンテンツクリア状況」タブは
**1 行 = 1 コンテンツで、行頭に専用の丸い絵**が付く。名前も行に並ぶので
取り違えようがない。よってスクリーンショットを一次ソースにする。

入力のスクリーンショットは `tools/gamedata/screenshots/content_list_*.png` に
同梱してある(撮影 2026-09-01、ユーザー提供)。差し替えるときは
同じ「コンテンツクリア状況」タブを等倍(行の高さ 64px)で撮り、
`SHOTS` の `top`(1 行目の上端 y)と `left`(行枠の左端 x)を測り直す。

使い方:
    python tools/gamedata/import_content_images.py --dry-run
    python tools/gamedata/import_content_images.py
    python tools/gamedata/import_content_images.py --contact-sheet  # 目視確認用の一覧
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTENTS_RS = ROOT / "crates/gamedata/src/contents.rs"
SHOT_DIR = Path(__file__).resolve().parent / "screenshots"
OUTPUT_DIR = ROOT / "apps/desktop/src/assets/icons/contents"
CONTENT_ID_RE = re.compile(r'Def \{ id: "([a-z0-9_]+)"')

# 行の高さ。ゲーム UI の等倍。スクショごとに変わらない
ROW_PITCH = 64
# 行頭の絵は横長の楕円メダル。1:1 のアイコンにするので中央から正方形で抜く
ICON_PX = 56
# 行枠の左端からメダル中央までの距離(星印は左に外れるので入らない)
ICON_CX = 72
# 行の上端からメダル上端までの距離
ICON_TOP = 6


class Shot:
    """スクリーンショット 1 枚の座標系。`top` は 1 行目の上端、`left` は行枠の左端。"""

    def __init__(self, name: str, top: int, left: int) -> None:
        self.name = name
        self.top = top
        self.left = left

    def box(self, row: int) -> tuple[int, int, int, int]:
        """1 始まりの行番号 → 切り出す正方形。"""
        y = self.top + ROW_PITCH * (row - 1) + ICON_TOP
        x = self.left + ICON_CX - ICON_PX // 2
        return (x, y, x + ICON_PX, y + ICON_PX)


SHOTS: dict[str, Shot] = {
    "105828": Shot("content_list_105828.png", top=80, left=261),
    "105842": Shot("content_list_105842.png", top=84, left=265),
    "105850": Shot("content_list_105850.png", top=82, left=260),
    # 110019 / 110027 は同じ画面を拡大せず切り抜いたもの(行の高さは同じ 64px)
    "110019": Shot("content_list_110019.png", top=2, left=0),
    "110027": Shot("content_list_110027.png", top=0, left=3),
}

# コンテンツ id → (スクショ, 行番号)。行番号はスクショの上から 1 始まり。
# 行に出ているゲーム内の名前をコメントに残す(contents.rs の名前と綴りが違うものがあるため)。
#
# **ゲーム内の名前とツールのコンテンツを 1 対 1 で言い切れない行は載せない**。
# 別のコンテンツの絵を当てるくらいなら、破線 + `?`(未収録)のままのほうが嘘がない。
SOURCES: dict[str, tuple[str, int]] = {
    "eclipse_subjugation": ("105828", 1),  # エクリプスボス討伐戦
    "luminous_ex": ("105828", 2),  # ルミナス(EX)
    "abyss_ex": ("105828", 3),  # アビス-深層(EX)
    "aphetiria_ex": ("105828", 4),  # キシニク(EX) = アフェティリアEX
    "colorless_land": ("105828", 5),  # 色を失った大地
    "architect_mine": ("105828", 6),  # 設計者の採掘場
    "leitia_h": ("105828", 7),  # 追従する喜び(ハード)
    "architect_h": ("105828", 8),  # 見つめる悲しみ(ハード)
    "vestige_ruins": ("105842", 1),  # アングリービックテディ = ヴェスティージの廃墟の主
    "siokan_boss_subjugation": ("105842", 3),  # ボス討伐戦(シオカンヘイムの列)
    "odin_total_war": ("105842", 4),  # オーディン
    "odin_rank": ("105842", 4),  # オーディン(ランク戦も同じボス。ゲーム内の行は 1 つ)
    "aphetiria_hard": ("105842", 7),  # キシニク(ハード)
    "moon_queen_training": ("105842", 8),  # 月の女王の軍の訓練所
    "relic_sanctuary_shinchou": ("105842", 9),  # 神鳥(古代レリックの聖域)
    "detachment_subjugation": ("105850", 4),  # 別動隊討伐
    "valley_defense": ("110019", 1),  # 異界の峡谷防衛戦
    "last_battle": ("110019", 2),  # 最後の決戦
    "clamor": ("110027", 3),  # クラモール
}


def content_ids() -> set[str]:
    return set(CONTENT_ID_RE.findall(CONTENTS_RS.read_text(encoding="utf-8")))


def crop(content_id: str):
    from PIL import Image  # 切り出しのときだけ使う。アプリの依存には入れない

    shot_key, row = SOURCES[content_id]
    shot = SHOTS[shot_key]
    image = Image.open(SHOT_DIR / shot.name).convert("RGBA")
    return image.crop(shot.box(row))


def contact_sheet(path: Path) -> None:
    """切り出し結果を 1 枚に並べる。行と名前の対応を目視で確かめるため。"""
    from PIL import Image

    ids = list(SOURCES)
    columns = 8
    rows = (len(ids) + columns - 1) // columns
    sheet = Image.new("RGBA", (columns * ICON_PX, rows * ICON_PX), (255, 0, 255, 255))
    for index, content_id in enumerate(ids):
        sheet.paste(crop(content_id), ((index % columns) * ICON_PX, (index // columns) * ICON_PX))
    sheet.save(path)
    print(f"CONTACT {path} ({' / '.join(ids)})")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true", help="照合だけ行い画像を書き出さない")
    parser.add_argument("--contact-sheet", type=Path, help="目視確認用の一覧を書き出す")
    args = parser.parse_args()

    known = content_ids()
    unknown = sorted(set(SOURCES) - known)
    if unknown:
        print(f"contents.rs に無い id: {', '.join(unknown)}")
        return 1
    print(f"contents={len(known)} mapped={len(SOURCES)} uncovered={len(known) - len(SOURCES)}")
    for content_id in sorted(known - set(SOURCES)):
        print(f"SKIP {content_id}: 対応する行を確認できたスクショが無い")

    if args.contact_sheet:
        contact_sheet(args.contact_sheet)
    if args.dry_run:
        return 0

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    for content_id in SOURCES:
        destination = OUTPUT_DIR / f"{content_id}.png"
        crop(content_id).save(destination, format="PNG", optimize=True)
        print(f"CUT {destination.name} <- {SOURCES[content_id][0]} 行 {SOURCES[content_id][1]}")
    print(f"written={len(SOURCES)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
