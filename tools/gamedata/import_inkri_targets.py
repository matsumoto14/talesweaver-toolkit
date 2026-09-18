"""ビアヌのインクリ対象装備カタログを生成する。

`crates/gamedata/src/inkri/generated.rs` を書き出し、対象装備のアイコンを
`apps/desktop/src/assets/inkri/items/<ItemId>.png` へ複製する。

## データソース
- 装備の一覧・部位: クライアント展開データ `<tw_assets>/db/dm_*_0*.csv`
  (`EquippableItemTemplate` 行。合成回数を持つ `c51_995dd080` 列がある = インクリ対象になり得る
  装備だけを拾う。9xxx 側(`dm_*_9*.csv`)は 0xxx の複製 = ItemId +90,000,000 なので読まない)。
  合成回数の上限そのものは使わない — シミュレータはインクリ回数を積み上げる形式で、
  合成回数は追わない(ユーザー判断 2026-09-18)。
- 表示名・アイコン: `<tw_assets>/item_icons/items.csv`(クライアント DB の `Name` 列は
  一部が文字化けしているため使わない)。
- ビアヌのインクリ費用(SEED): wiki「装備システム/インクリ」の表(2026-09-12 更新、
  ユーザー確認 2026-09-17)をそのまま転記。ロード/加護/祝福/王室の費用は資料が無く常に `None`。
- エタインクリ費用(SEED、呪文書 1 枚は別): 同ページ「エタインクリ費用」節。エタレベルが装備条件の
  セイクリッド / 改・セイクリッドだけにあり、他の系列は `None`(= エタインクリ不可)。

## 部位(PartSlot)の決め方
`c35_EquipSlot` の値を `domain::PartSlot` に機械的に対応させる(既存
`tools/gamedata/import_client_db.py` の `EQUIP_SLOT_TO_PART_SLOT` と同じ対応。
1=Helm, 2=Weapon, 3=Armor, 4=Shield, 5=Head, 6=Hand, 7=Body, 8=Leg, 18=Artifact)。

## 系列ごとの収録・費用の割り当て
wiki のインクリ費用表は系列によって「武器/防具/AF/エフェクト」の粒度がバラバラ:
- アクィルス相当(Lv300)・アビス相当(Lv310)は 4 区分(武器・防具・AF・エフェクト)が別価格。
  実データでは「エフェクト」は Body スロットの「〜ウィング」1 品、「AF」は Head/Hand/Leg の
  3 品(アミュレット/ガントレット/ブーツ)、「防具」は Helm/Armor/Shield(ヘルム・メイル系・
  リスト/シールド/クリスタル等)にあたる(id を突き合わせて確認、2026-09-17)。
- エクリプスは「武器・防具」と「AF」の 2 区分。ウィング(Body)は wiki 表に単独の行が無いため、
  AF と同額として扱う([仮]。区分自体が無い可能性がある)。
- セイクリッド・改・セイクリッドは「武器・防具・AF」が同一価格 1 本(部位を問わず同額)。
- モモンズ・グリーブ・地神装備・ネニャフル学院の鎧は単一価格(部位を問わず同額、あるいは対象が単一部位)。
  デックストシューズ・アベルシューズ・サクヤの雪駄・真ブリニクル武器は収録しない(ユーザー判断 2026-09-18)。
- Lv185AF・Lv250コラボ靴は実アイテム名が確認できず対象外(判定が曖昧な系列は入れない)。

## id の絞り込み
名前の前方一致だけでは無関係な同名接頭辞(例: 「†アビスフード」)を拾ってしまうため、
アクィルス/アビス/エクリプス/セイクリッド/改・セイクリッドは ItemId の範囲でも絞る
(該当世代の武器・防具一式が連番で入っている範囲。2026-09-17 に実データで確認)。

使い方:
    python tools/gamedata/import_inkri_targets.py [--assets PATH]
`--assets` 省略時は環境変数 `TW_ASSETS`、それも無ければ `C:\\github\\private\\tw_assets`。
"""

from __future__ import annotations

import argparse
import csv
import glob
import os
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT_PATH = ROOT / "crates/gamedata/src/inkri/generated.rs"
ICON_OUT_DIR = ROOT / "apps/desktop/src/assets/inkri/items"

RETRIEVED_ON = "2026-09-17"

# c35_EquipSlot → domain::PartSlot バリアント名(tools/gamedata/import_client_db.py と同じ対応)
EQUIP_SLOT_TO_PART_SLOT = {
    "1": "Helm",
    "2": "Weapon",
    "3": "Armor",
    "4": "Shield",
    "5": "Head",
    "6": "Hand",
    "7": "Body",
    "8": "Leg",
    "18": "Artifact",
}


def default_assets_dir() -> Path:
    env = os.environ.get("TW_ASSETS")
    return Path(env) if env else Path(r"C:\github\private\tw_assets")


@dataclass
class DbItem:
    item_id: int
    equip_slot: str


def load_db_items(assets: Path) -> dict[int, DbItem]:
    """EquippableItemTemplate かつ c51(合成回数)が入っている行だけを ItemId で引けるようにする。"""
    items: dict[int, DbItem] = {}
    for path in sorted(glob.glob(str(assets / "db" / "dm_*_0*.csv"))):
        with open(path, encoding="utf-8-sig", newline="") as f:
            r = csv.reader(f)
            header = next(r)
            next(r)  # #type 行
            template_col = _col(header, "c1_Template")
            id_col = _col(header, "c2_ItemId")
            slot_col = _col(header, "c35_EquipSlot")
            synth_col = _col(header, "c51_995dd080")
            if None in (template_col, id_col, slot_col, synth_col):
                continue
            for row in r:
                if row[template_col] != "EquippableItemTemplate":
                    continue
                try:
                    item_id = int(row[id_col])
                    int(row[synth_col].strip("[]").split(",")[-1])
                except (ValueError, IndexError):
                    continue
                items[item_id] = DbItem(item_id, row[slot_col])
    return items


def _col(header: list[str], prefix: str) -> int | None:
    for i, h in enumerate(header):
        if h.startswith(prefix):
            return i
    return None


def load_item_names(assets: Path) -> dict[int, tuple[str, str]]:
    """ItemId → (表示名, icon_file)。"""
    names: dict[int, tuple[str, str]] = {}
    with open(assets / "item_icons" / "items.csv", encoding="utf-8-sig", newline="") as f:
        for row in csv.DictReader(f):
            try:
                item_id = int(row["id"])
            except ValueError:
                continue
            names[item_id] = (row["name"], row.get("icon_file", ""))
    return names


# ── 系列定義 ──
# price は part(PartSlot バリアント名文字列) → 価格(SEED)の関数。全部位同額なら定数を返すだけでよい。

Series = tuple[str, object, object]  # (label, matcher(name,id,part)->bool, price(part)->int|None)


def prefix(*prefixes: str):
    return lambda name, item_id, part: any(name.startswith(p) for p in prefixes)


def exact(*names: str):
    return lambda name, item_id, part: name in names


def id_range(lo: int, hi: int):
    """[lo, hi] 両端含む。"""
    return lambda name, item_id, part: lo <= item_id <= hi


def all_of(*matchers):
    return lambda name, item_id, part: all(m(name, item_id, part) for m in matchers)


def any_of(*matchers):
    return lambda name, item_id, part: any(m(name, item_id, part) for m in matchers)


def part_is(*parts: str):
    return lambda name, item_id, part: part in parts


def flat(price: int):
    return lambda part: price


def by_part(prices: dict[str, int], default: int | None = None):
    return lambda part: prices.get(part, default)


# 系列ラベル → エタインクリ費用(SEED)。wiki「エタインクリ費用」節(2026-09-18 転記)
ETA_SEED_COST: dict[str, int] = {
    "セイクリッド": 296_680_000,
    "改・セイクリッド": 313_680_000,
}

SERIES: list[Series] = [
    ("モモンズ・グリーブ", exact("†モモンズ・グリーブ"), flat(3_000_000)),
    (
        "地神装備",
        prefix("†地神の", "†真・地神の"),
        flat(15_787_500),
    ),
    (
        "ネニャフル学院の鎧",
        lambda name, item_id, part: "ネニャフル学院の" in name,
        flat(28_650_000),
    ),
    (
        "アクィルス",
        all_of(
            prefix("†アクィルス"),
            any_of(id_range(1039515, 1039566), id_range(1045792, 1045792)),
        ),
        by_part(
            {
                "Weapon": 10_500_000,
                "Helm": 11_250_000,
                "Armor": 11_250_000,
                "Shield": 11_250_000,
                "Head": 9_000_000,
                "Hand": 9_000_000,
                "Leg": 9_000_000,
                "Body": 8_250_000,
            }
        ),
    ),
    (
        "アビス",
        all_of(prefix("†アビス"), id_range(1040995, 1041046)),
        by_part(
            {
                "Weapon": 11_325_000,
                "Helm": 12_075_000,
                "Armor": 12_075_000,
                "Shield": 12_075_000,
                "Head": 10_575_000,
                "Hand": 10_575_000,
                "Leg": 10_575_000,
                "Body": 8_325_000,
            }
        ),
    ),
    (
        "エクリプス",
        all_of(prefix("†エクリプス"), id_range(1044260, 1044310)),
        by_part(
            {
                "Weapon": 25_575_000,
                "Helm": 25_575_000,
                "Armor": 25_575_000,
                "Shield": 25_575_000,
                # AF(Head/Hand/Leg)。ウィング(Body)は表に無いので同額を仮に当てる([仮])
                "Head": 14_325_000,
                "Hand": 14_325_000,
                "Leg": 14_325_000,
                "Body": 14_325_000,
            }
        ),
    ),
    (
        "セイクリッド",
        all_of(prefix("†セイクリッド"), id_range(1060370, 1060421)),
        flat(36_825_000),
    ),
    (
        "改・セイクリッド",
        all_of(prefix("†改・セイクリッド"), id_range(1060794, 1060845)),
        flat(38_700_000),
    ),
]


@dataclass
class Target:
    item_id: int
    name: str
    series: str
    part: str
    price: int | None
    eta_price: int | None
    icon_file: str


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--assets", type=Path, default=None)
    args = parser.parse_args()
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

    assets = args.assets or default_assets_dir()
    if not assets.exists():
        print(f"tw_assets が見つかりません: {assets}", file=sys.stderr)
        sys.exit(1)

    db_items = load_db_items(assets)
    names = load_item_names(assets)

    targets: list[Target] = []
    seen_ids: set[int] = set()
    per_series_count: dict[str, int] = {}
    for item_id, db_item in sorted(db_items.items()):
        part = EQUIP_SLOT_TO_PART_SLOT.get(db_item.equip_slot)
        if part is None:
            continue
        name_entry = names.get(item_id)
        if name_entry is None:
            continue
        name, icon_file = name_entry
        for label, matcher, price_fn in SERIES:
            if item_id in seen_ids:
                break
            if matcher(name, item_id, part):
                targets.append(
                    Target(
                        item_id=item_id,
                        name=name,
                        series=label,
                        part=part,
                        price=price_fn(part),
                        eta_price=ETA_SEED_COST.get(label),
                        icon_file=icon_file,
                    )
                )
                seen_ids.add(item_id)
                per_series_count[label] = per_series_count.get(label, 0) + 1
                break

    targets.sort(key=lambda t: t.item_id)
    write_rust(targets)
    copy_icons(assets, targets)

    print(f"収録件数: {len(targets)} 件", file=sys.stderr)
    for label, _matcher, _price in SERIES:
        count = per_series_count.get(label, 0)
        print(f"  {label}: {count} 件", file=sys.stderr)
    uncosted = [t for t in targets if t.price is None]
    if uncosted:
        print(f"ビアヌ費用が未収録: {len(uncosted)} 件", file=sys.stderr)


def write_rust(targets: list[Target]) -> None:
    lines = []
    lines.append("//! 生成物。手で編集しない。")
    lines.append("//! 再生成: `python tools/gamedata/import_inkri_targets.py`")
    lines.append("//! 系列・費用の割り当ては `tools/gamedata/import_inkri_targets.py` 冒頭のコメント参照。")
    lines.append("")
    lines.append("use super::InkriTarget;")
    lines.append("use domain::PartSlot;")
    lines.append("")
    lines.append(f"pub(super) static INKRI_TARGETS: &[InkriTarget] = &[")
    for t in targets:
        name_escaped = t.name.replace("\\", "\\\\").replace('"', '\\"')
        series_escaped = t.series.replace("\\", "\\\\").replace('"', '\\"')
        cost = f"Some({t.price})" if t.price is not None else "None"
        lines.append("    InkriTarget {")
        lines.append(f"        client_item_id: {t.item_id},")
        lines.append(f'        name: "{name_escaped}",')
        lines.append(f'        series: "{series_escaped}",')
        lines.append(f"        part: PartSlot::{t.part},")
        lines.append(f"        bianu_seed_cost: {cost},")
        eta = f"Some({t.eta_price})" if t.eta_price is not None else "None"
        lines.append(f"        eta_seed_cost: {eta},")
        lines.append("    },")
    lines.append("];")
    lines.append("")
    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUT_PATH.write_text("\n".join(lines), encoding="utf-8")


def copy_icons(assets: Path, targets: list[Target]) -> None:
    icon_dir = assets / "item_icons"
    # 収録から外れた装備のアイコンを残さないため、出力先を作り直す
    shutil.rmtree(ICON_OUT_DIR, ignore_errors=True)
    ICON_OUT_DIR.mkdir(parents=True, exist_ok=True)
    copied = 0
    missing: list[int] = []
    for t in targets:
        source = icon_dir / t.icon_file if t.icon_file else None
        if source is None or not source.is_file():
            missing.append(t.item_id)
            continue
        shutil.copyfile(source, ICON_OUT_DIR / f"{t.item_id}.png")
        copied += 1
    print(f"アイコン複製 {copied} 件 / 無し {len(missing)} 件", file=sys.stderr)
    for item_id in missing:
        print(f"  no icon: ItemId {item_id}", file=sys.stderr)


if __name__ == "__main__":
    main()
