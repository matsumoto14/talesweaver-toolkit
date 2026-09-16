r"""Tale Wiki の「ステータス」表に絵が無いバフのアイコンを、クライアント展開データ / 同梱済みの
スキル絵から補う(wiki に絵がある行は `import_buff_icons.py` が担当)。
本物の絵が無いバフ(クラブ効果・射手のルーン)は別の物の絵で代用せず `?` のままにする(ユーザー判断 2026-09-16)。

複製先は `apps/desktop/src/assets/icons/buffs/<buff id>.png`。UI(`ui/Icon.svelte`)は id から
機械的に解決するので、対応表はこのファイルだけが持つ。

使い方:
    python tools/gamedata/import_client_buff_icons.py [--assets PATH]
`--assets` 省略時は環境変数 `TW_ASSETS`、それも無ければ `C:\github\private\tw_assets`。
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ICONS_DIR = ROOT / "apps/desktop/src/assets/icons"
OUTPUT_DIR = ICONS_DIR / "buffs"

# buff id → クライアントの ItemId(item_icons/<ItemId>_<名前>.png)。名前は照合用のメモ
CLIENT_ITEMS: dict[str, tuple[int, str]] = {
    "plunder_bread": (1047412, "略奪パン"),
    "boiled_mimic": (1047410, "茹でミミック"),
    "ancient_relic_minigame": (1060615, "古代レリックの聖域追加侵入券"),
}

# buff id → assets/icons 内の既存画像(同じスキルの絵を 2 出典にしない)
LOCAL_ICONS: dict[str, str] = {
    # ティチエルのマスタリー「極・遊び用チンキ剤」そのもの
    "play_tincture": "masteries/tichiel_m3_3.png",
    # エアル(アナイスの召喚)のハードウエポン。wiki Skill/アナイス でも同じ ハードウェポン.png
    "hard_weapon_earl": "skills/anais_hard_weapon.png",
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

    missing: list[str] = []
    for buff_id, (item_id, name) in CLIENT_ITEMS.items():
        matches = sorted(item_dir.glob(f"{item_id}_*.png"))
        if not matches:
            missing.append(f"{buff_id}: ItemId {item_id} ({name})")
            continue
        shutil.copyfile(matches[0], OUTPUT_DIR / f"{buff_id}.png")
    for buff_id, rel in LOCAL_ICONS.items():
        src = ICONS_DIR / rel
        if not src.is_file():
            missing.append(f"{buff_id}: {rel}")
            continue
        shutil.copyfile(src, OUTPUT_DIR / f"{buff_id}.png")
    print(f"{len(CLIENT_ITEMS) + len(LOCAL_ICONS) - len(missing)} 件を複製")
    if missing:
        sys.exit("見つからない画像:\n  " + "\n  ".join(missing))


if __name__ == "__main__":
    main()
