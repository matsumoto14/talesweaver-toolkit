"""補正源(pages/chars/sourceId.ts の SourceId)の代表アイコンをクライアント展開データから同梱する。

補正源はゲーム内の「物」ではないので id から機械的には引けない。代わりに **その補正源を象徴する
アイテム**(強化剤・スクロール等)を 1 つ選び、`<tw_assets>/item_icons/<ItemId>_<名前>.png` を
`apps/desktop/src/assets/icons/sources/<SourceId>.png` へ複製する。UI 側(`ui/Icon.svelte`)は
`kind="source"` + SourceId で機械的に解決するので、対応表はこのファイルだけが持つ。

既に別系統で同梱済みの絵(研磨・称号・ソウルリンク)は `assets/icons/` 内から複製する。

使い方:
    python tools/gamedata/import_source_icons.py [--assets PATH]
`--assets` 省略時は環境変数 `TW_ASSETS`、それも無ければ `C:\\github\\private\\tw_assets`。
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ICONS_DIR = ROOT / "apps/desktop/src/assets/icons"
OUTPUT_DIR = ICONS_DIR / "sources"

# SourceId → クライアントの ItemId(item_icons/<ItemId>_<名前>.png)。名前は照合用のメモ
CLIENT_ITEMS: dict[str, tuple[int, str]] = {
    "status": (1016470, "ステータス再分配スクロール"),
    "equipment": (1046188, "†パワーダンシングゼリッピ"),
    "pet": (1006173, "ペットＳスキルスクロール(応援)"),
    "rune": (1021058, "ルーンの種(Lv1)"),
    "crown": (1000816, "†クラウン"),
    "monsterCard": (1035945, "モンスターカード強化の札(STAB +1)"),
    "relic": (1042562, "神鳥の聖物"),
    "siena": (1060574, "還流の書"),
    "randomOption": (1034550, "オプション変化石"),
    "commonSkill": (1039618, "共通スキル(オーグメント)"),
    "criticalRate": (1020306, "クリティカルカード"),
    "thesis": (1020270, "テシスコア"),
    "avatar": (1033839, "アバター強化剤 (突き攻撃力 +1)"),
    "skills": (1042955, "スキルスクロール"),
    # SourceId には無いが、ホーム「今日の強化」のタイルが id `enchant` で引く固定名
    # (キャラタブでは装備の中の操作なので補正源のペインが無い)
    "enchant": (1033038, "エンチャント強化呪文書"),
}

# SourceId → assets/icons 内の既存画像(同じ物の絵を 2 出典にしない)
LOCAL_ICONS: dict[str, str] = {
    "polish": "buffs/equipment_polish.png",
    "title": "titles/title.png",
    "soulLink": "buffs/soul_link_explore.png",
    # 中ディレイ減少を表すアイテムは無い。共通スキル「フルスロットル」(中ディレイ減少)の絵を使う
    "actualDelay": "skills/full_throttle.png",
}


def default_assets_dir() -> Path:
    env = os.environ.get("TW_ASSETS")
    return Path(env) if env else Path(r"C:\github\private\tw_assets")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--assets", type=Path, default=default_assets_dir())
    args = parser.parse_args()
    item_dir: Path = args.assets / "item_icons"
    if not item_dir.is_dir():
        sys.exit(f"item_icons が見つかりません: {item_dir}")

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    missing: list[str] = []
    for source_id, (item_id, name) in CLIENT_ITEMS.items():
        matches = sorted(item_dir.glob(f"{item_id}_*.png"))
        if not matches:
            missing.append(f"{source_id}: ItemId {item_id} ({name})")
            continue
        shutil.copyfile(matches[0], OUTPUT_DIR / f"{source_id}.png")
        print(f"{source_id:13} <- {matches[0].name}")
    for source_id, rel in LOCAL_ICONS.items():
        src = ICONS_DIR / rel
        if not src.is_file():
            missing.append(f"{source_id}: {rel}")
            continue
        shutil.copyfile(src, OUTPUT_DIR / f"{source_id}.png")
        print(f"{source_id:13} <- {rel}")
    if missing:
        sys.exit("見つからない画像:\n  " + "\n  ".join(missing))


if __name__ == "__main__":
    main()
