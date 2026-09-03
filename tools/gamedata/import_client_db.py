"""クライアント展開データ(dm_NNNNN_NNNN.csv)から装備カタログを取り込み、
`crates/gamedata/src/equipment_catalog/client.rs` を生成する。

出典: `tw_assets/README.md`(リポジトリ外置き場)。`db/dm_00000_NNNN.csv` は UTF-8 BOM 付き、
1 行目=列名(`c<番号>_<名前>_<hash>` / `c<番号>_<hash>`)、2 行目=`#type`、3 行目以降データ。
`dm_*_9xxx` は `0xxx` の複製(ItemId +90,000,000)なので読まない。パッケージは dm_00000 と dm_00001 の 2 つ。

## 列対応(hash 付き列名 → ドメインの項目。hash は列名由来で安定、番号は将来変わりうる)
- `c1_Template_*`            : `EquippableItemTemplate` の行だけを装備として扱う
- `c2_ItemId_*`              : アイテム ID(id 生成・保存データ突き合わせのキー)
- `c3_Name_*`                : 表示名(既存カタログとの突き合わせキー)
- `c7_EquipType_*`           : 装備種別 ID → 表 0309(`dm_00000_0309.csv`)の `c3_Name` で
  短剣/細剣/…/ライトアーマー/メイル/マジックアーマー/スーツ/ローブ/リスト/バンド/バングル/盾/
  スペルブック/クリスタル/物理双剣(sub)/魔法双剣(sub)/物理弾倉/魔法弾倉/ペンデュラム 等 61 種。
  武器種・腕装備種・鎧の補正式(`EquipmentEnhanceType`)への対応は本ファイル冒頭の辞書で持つ。
- `c35_EquipSlot_*`          : 装備部位。1=兜, 2=武器, 3=鎧, 4=盾(腕), 5=頭, 6=手, 7=体, 8=脚,
  13=その他, 15=乗り物, 18=アーティファクト, 20/21=レリック右/左(表 0309 `c4_Slot` と同じ値)。
  本スクリプトが対象にするのは `EQUIP_SLOT_TO_PART_SLOT` に載る 1/2/3/4/5/6/7/8/18 だけ
  (13/15 は実データ 0 件、20/21 はレリックで段成長という別モデルのため対象外 = 既存の
  `relic_item` ハードコードをそのまま使う)。
- `c39_SetId_*`              : セット ID(今回未使用。セット効果は次段)
- `c42_Thrust_* .. c50_Agility_*` : 突/斬/物防/魔攻/魔防/命中/Cri/回避/敏捷の 9 値。各列は
  文字列 `"[min, max, cap]"`(JSON 配列)。**cap==255 または cap==1000 は番兵**で、
  「エンチャント総上限の追加情報なし」を表す(下記 SENTINEL 節)。
- `c53_Req1Type_* .. c60_Req4Val_*` : 装備条件 4 組。Type==1 が Lv 条件で Val がそのレベル。Lv 条件は
  どの組にも入りうる(セイクリッド以降は Req1 が Type 10(エタの意志 Lv 21 / 30)で Lv 310 は Req2)。
- `c11_CharMask_*`, `c5_Icon_*` : 今回未使用(装備可能キャラのビット・アイコン資産)。

## 9 値の三つ組と番兵(SENTINEL)
[min, max, cap] の `cap` は「エンチャント込みの総上限」のはずだが、実データでは
cap が 255 か 1000 になっている行がある。†アクィルスウィング(ItemId 1039564)で
wiki カタログ(values_min/max/enchant_total_caps = 76/86/116)と全 9 値が完全一致したのに対し、
†デモニックウィング(ItemId 1039368、wiki: `wiki-9bf855a9cd26`)は命中/Cri/回避/敏捷が
`[1,65,1000]` 等でありながら、既存 wiki 行の enchant_total_caps はその 4 値とも 0
(= 総上限 == values_max、エンチャント枠なし)だった。ここから **cap が 255 or 1000 のときは
「total_cap 情報なし」を意味し、`total_cap = values_max`(エンチャント枠 0 相当)として扱う**
と確定した(`load_equip_table` の `resolve_triple` 参照)。cap がそれ以外の実数値のときは
そのまま「エンチャント込みの総上限」として使う(アクィルスウィングで確認済み)。

## 収録の絞り込み
(a) 既存カタログ(手書き `items.rs` / `generated.rs` / `sacred_kr.rs`)に同名の行がある、または
(b) Lv 条件(Req1〜4 のいずれかが Type 1)が 280 以上で、EquipSlot が上記の対象部位に対応する。
のいずれかを満たす行だけを client.rs に書き出す。

同名で複数行あるもの(例: †デモニックウィング はステごとに 5 行、命中/Cri/回避/敏捷/物防各1個の
特化違い)は、既存カタログに同名行があればその values_min/max と完全一致する行を採用し、
無ければ ItemId が最小の行を代表として採用する(残りは捨てる。実装側は名前でも重複排除するため
複数行を残しても後勝ちにしかならず、この場で 1 行に絞るほうが「どれが採用されたか」を追える)。

## id の引き継ぎ
既存カタログ(手書き / `generated.rs` / `sacred_kr.rs`)の同名アイテムは、そこで使われている
id をそのまま使う(保存データ・アイコン資産のキーなので変えない)。新規行は `client-<ItemId>`。

使い方:
    python tools/gamedata/import_client_db.py [--assets PATH]
`--assets` 省略時は環境変数 `TW_ASSETS`、それも無ければ `C:\\github\\private\\tw_assets`。
"""
from __future__ import annotations

import argparse
import csv
import glob
import json
import os
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CATALOG_DIR = ROOT / "crates/gamedata/src/equipment_catalog"
OUT_PATH = CATALOG_DIR / "client.rs"

STAT_COLUMNS = [
    ("c42_Thrust", "thrust"),
    ("c43_Slash", "slash"),
    ("c44_PhysDef", "physical_defense"),
    ("c45_MagAtk", "magic_attack"),
    ("c46_MagDef", "magic_defense"),
    ("c47_Hit", "accuracy"),
    ("c48_Crit", "critical"),
    ("c49_Evade", "evasion"),
    ("c50_Agility", "agility"),
]
SENTINEL_CAPS = {255, 1000}

# 表 0309 `c3_Name` → 武器種(`domain::WeaponClass` のバリアント名)。
EQUIP_TYPE_TO_WEAPON_CLASS = {
    "短剣": "Dagger", "細剣": "Rapier", "長剣": "LongSword", "大剣": "GreatSword",
    "短刀": "ShortSword", "刀": "Katana", "太刀": "Tachi", "槍": "Spear", "棒": "Rod",
    "鞭": "Whip", "カーラ": "Kara", "クロー": "Claw", "スモールソード": "SmallSword",
    "連接棍": "Nunchaku", "斧": "Axe", "魔杖": "MagicWand", "聖杖": "HolyStaff",
    "戦杖": "WarStaff", "ワンド": "Wand", "セプター": "Scepter", "ハンドベル": "Handbell",
    "物理双剣": "DualBladePhysical", "魔法双剣": "DualBladeMagic", "サイズ": "Scythe",
    "ハンマー": "Hammer", "トーテム": "Totem", "ハンドランチャー": "HandLauncher",
    "物理銃": "PhysicalGun", "魔法銃": "MagicGun", "アーミングソード": "ArmingSword",
    "ソードシェイプ": "SwordShape",
}
# 表 0309 `c3_Name` → 腕装備種(`domain::WristType` のバリアント名)。EquipSlot=4(盾(腕))の行にだけ適用。
EQUIP_TYPE_TO_WRIST_TYPE = {
    "盾": "Shield", "スペルブック": "Spellbook", "リスト": "Knuckle", "バンド": "Band",
    "バングル": "Bracelet", "ペンデュラム": "Pendulum", "クリスタル": "CrystalBall",
    "物理双剣(sub)": "DualBladePhysical", "魔法双剣(sub)": "DualBladeMagic",
    "物理弾倉": "PhysicalMagazine", "魔法弾倉": "MagicMagazine",
}
# 表 0309 `c3_Name` → 鎧の補正式(`domain::EquipmentEnhanceType` のバリアント名)。EquipSlot=3(鎧)の行にだけ適用。
EQUIP_TYPE_TO_ARMOR_ENHANCE = {
    "ライトアーマー": "ArmorLight", "メイル": "ArmorHeavy", "マジックアーマー": "ArmorMagic",
    "スーツ": "ArmorSuit", "ローブ": "ArmorRobe",
}
# EquipSlot → PartSlot バリアント名。20/21(レリック)・13(その他、実データ0件)・15(乗り物)は対象外。
EQUIP_SLOT_TO_PART_SLOT = {
    "1": "Helm", "2": "Weapon", "3": "Armor", "4": "Shield",
    "5": "Head", "6": "Hand", "7": "Body", "8": "Leg", "18": "Artifact",
}


def default_assets_dir() -> Path:
    env = os.environ.get("TW_ASSETS")
    if env:
        return Path(env)
    return Path(r"C:\github\private\tw_assets")


@dataclass
class Row:
    item_id: str
    name: str
    equip_type: str
    equip_slot: str
    values: dict  # stat -> (min, max, cap)
    req1_type: str
    req1_val: str
    source_file: str


def load_csv(path: Path) -> tuple[list[str], list[list[str]]]:
    csv.field_size_limit(10_000_000)
    with open(path, encoding="utf-8-sig", newline="") as f:
        r = csv.reader(f)
        header = next(r)
        next(r)  # #type 行
        rows = list(r)
    return header, rows


def col_index(header: list[str], prefix: str) -> int | None:
    for i, h in enumerate(header):
        if h.startswith(prefix):
            return i
    return None


def load_equip_type_names(assets: Path) -> dict[str, str]:
    header, rows = load_csv(assets / "db" / "dm_00000_0309.csv")
    id_col = col_index(header, "c2_EquipTypeId")
    name_col = col_index(header, "c3_Name")
    return {row[id_col]: row[name_col] for row in rows}


def load_equipment_rows(assets: Path) -> list[Row]:
    rows: list[Row] = []
    # 全 DB パッケージ(dm_00000 / dm_00001 …)を読む。セイクリッド・テネブリスは dm_00001 側にある
    for path in sorted(glob.glob(str(assets / "db" / "dm_*_0*.csv"))):
        path = Path(path)
        header, data = load_csv(path)
        template_col = col_index(header, "c1_Template")
        item_id_col = col_index(header, "c2_ItemId")
        name_col = col_index(header, "c3_Name")
        equip_type_col = col_index(header, "c7_EquipType")
        equip_slot_col = col_index(header, "c35_EquipSlot")
        # Lv 条件は Req1〜Req4 のどこにでも入る(セイクリッド以降は Req1 がエタ条件(Type 10)で Lv は Req2)
        req_cols = [
            (col_index(header, f"c{t}_Req{n}Type"), col_index(header, f"c{v}_Req{n}Val"))
            for n, t, v in ((1, 53, 54), (2, 55, 56), (3, 57, 58), (4, 59, 60))
        ]
        stat_cols = {key: col_index(header, prefix) for prefix, key in STAT_COLUMNS}
        if None in (template_col, item_id_col, name_col, equip_type_col, equip_slot_col):
            continue
        for row in data:
            if row[template_col] != "EquippableItemTemplate":
                continue
            values = {}
            ok = True
            for key, idx in stat_cols.items():
                if idx is None:
                    ok = False
                    break
                try:
                    triple = json.loads(row[idx])
                except (json.JSONDecodeError, IndexError):
                    ok = False
                    break
                values[key] = tuple(triple)
            if not ok:
                continue
            level = "0"
            for type_col, val_col in req_cols:
                if type_col is not None and val_col is not None and row[type_col] == "1":
                    level = row[val_col]
            rows.append(Row(
                item_id=row[item_id_col],
                name=row[name_col],
                equip_type=row[equip_type_col],
                equip_slot=row[equip_slot_col],
                values=values,
                req1_type="1" if level != "0" else "0",
                req1_val=level,
                source_file=path.name,
            ))
    return rows


# ── 既存カタログ(手書き / generated.rs / sacred_kr.rs)からの名前→id・値の抽出 ──

STRUCT_LITERAL_RE = re.compile(
    r'id:\s*"([^"]+)",\s*(?:icon_id:\s*"[^"]+",\s*)?slot:\s*PartSlot::(\w+),\s*name:\s*"([^"]+)"'
    r'(?:.*?values_min:\s*v\(([^)]*)\),\s*values_max:\s*v\(([^)]*)\))?',
    re.S,
)
# items.rs のヘルパー呼び出し(`effect_item("id", "name", ...)` 等)は values を別引数で取らないので
# 名前→id の対応だけ拾う。
CALL_RE = re.compile(r'\b\w+\(\s*"([a-z][a-z0-9\-]*)",\s*"([^"]+)"')


def parse_existing_catalog_names(text: str) -> dict[str, str]:
    """名前 → id。構造体リテラルとヘルパー呼び出しの両方から拾う。"""
    names: dict[str, str] = {}
    for m in STRUCT_LITERAL_RE.finditer(text):
        item_id, _slot, name = m.group(1), m.group(2), m.group(3)
        names.setdefault(name, item_id)
    for m in CALL_RE.finditer(text):
        item_id, name = m.group(1), m.group(2)
        names.setdefault(name, item_id)
    return names


def parse_existing_catalog_values(text: str) -> dict[str, tuple[tuple[int, ...], tuple[int, ...]]]:
    """名前 → (values_min, values_max)。構造体リテラル形式のものだけ(重複名の判定用)。"""
    values: dict[str, tuple[tuple[int, ...], tuple[int, ...]]] = {}
    for m in STRUCT_LITERAL_RE.finditer(text):
        name, vmin, vmax = m.group(3), m.group(4), m.group(5)
        if vmin is None or vmax is None:
            continue
        key = name
        parsed = (
            tuple(int(x) for x in vmin.split(",")),
            tuple(int(x) for x in vmax.split(",")),
        )
        values.setdefault(key, parsed)
    return values


def load_existing_catalog() -> tuple[dict[str, str], dict[str, tuple]]:
    names: dict[str, str] = {}
    values: dict[str, tuple] = {}
    for fname in ("items.rs", "generated.rs", "sacred_kr.rs"):
        text = (CATALOG_DIR / fname).read_text(encoding="utf-8")
        for name, item_id in parse_existing_catalog_names(text).items():
            names.setdefault(name, item_id)
        for name, v in parse_existing_catalog_values(text).items():
            values.setdefault(name, v)
    return names, values


def resolve_triple(triple: tuple[int, int, int]) -> tuple[int, int, int]:
    """[min, max, cap] → (min, max, total_cap)。cap が番兵(255/1000)なら
    total_cap = max(エンチャント枠 0 相当)にする。"""
    lo, hi, cap = triple
    if cap in SENTINEL_CAPS:
        cap = hi
    return lo, hi, cap


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

    equip_type_names = load_equip_type_names(assets)
    rows = load_equipment_rows(assets)
    old_names, old_values = load_existing_catalog()

    # 対象部位の行だけに絞る
    candidates = [r for r in rows if r.equip_slot in EQUIP_SLOT_TO_PART_SLOT]

    selected: list[Row] = []
    reasons: dict[str, str] = {}  # item_id -> "name_match" | "level"
    by_name: dict[str, list[Row]] = {}
    for r in candidates:
        is_name_match = r.name in old_names
        all_zero = all(triple[1] == 0 for triple in r.values.values())
        is_high_level = (
            r.req1_type == "1" and r.req1_val.isdigit() and int(r.req1_val) >= 280 and not all_zero
        )
        if not (is_name_match or is_high_level):
            continue
        by_name.setdefault(r.name, []).append(r)
        reasons[r.item_id] = "name_match" if is_name_match else "level"

    duplicate_report: list[str] = []
    for name, group in by_name.items():
        if len(group) == 1:
            selected.append(group[0])
            continue
        chosen = None
        if name in old_values:
            want_min, want_max = old_values[name]
            for r in group:
                vmin = tuple(resolve_triple(r.values[key])[0] for _p, key in STAT_COLUMNS)
                vmax = tuple(resolve_triple(r.values[key])[1] for _p, key in STAT_COLUMNS)
                if vmin == want_min and vmax == want_max:
                    chosen = r
                    break
        if chosen is None:
            chosen = sorted(group, key=lambda r: int(r.item_id))[0]
            duplicate_report.append(
                f"{name}: {len(group)} 行中 ItemId {chosen.item_id} を代表として採用(既存値と一致する行なし)"
            )
        else:
            duplicate_report.append(
                f"{name}: {len(group)} 行中 ItemId {chosen.item_id} を採用(既存 wiki 値と一致)"
            )
        selected.append(chosen)

    # 突き合わせ集計(既存カタログと同名の行だけ対象)
    match_total = 0
    match_exact = 0
    mismatches: list[str] = []
    for r in selected:
        if r.name not in old_values:
            continue
        match_total += 1
        want_min, want_max = old_values[r.name]
        got_min = tuple(resolve_triple(r.values[key])[0] for _p, key in STAT_COLUMNS)
        got_max = tuple(resolve_triple(r.values[key])[1] for _p, key in STAT_COLUMNS)
        if got_min == want_min and got_max == want_max:
            match_exact += 1
        else:
            mismatches.append(f"{r.name} (ItemId {r.item_id}): wiki min={want_min} max={want_max} / client min={got_min} max={got_max}")

    # id 引き継ぎ
    used_ids: set[str] = set()
    entries = []
    for r in sorted(selected, key=lambda r: int(r.item_id)):
        item_id = old_names.get(r.name)
        if item_id is None or item_id in used_ids:
            item_id = f"client-{r.item_id}"
        used_ids.add(item_id)

        part_slot = EQUIP_SLOT_TO_PART_SLOT[r.equip_slot]
        type_name = equip_type_names.get(r.equip_type, "")
        weapon_class = EQUIP_TYPE_TO_WEAPON_CLASS.get(type_name) if part_slot == "Weapon" else None
        wrist_type = EQUIP_TYPE_TO_WRIST_TYPE.get(type_name) if part_slot == "Shield" else None
        enhance_type = EQUIP_TYPE_TO_ARMOR_ENHANCE.get(type_name) if part_slot == "Armor" else None

        resolved = {key: resolve_triple(r.values[key]) for _p, key in STAT_COLUMNS}
        vmin = ", ".join(str(resolved[key][0]) for _p, key in STAT_COLUMNS)
        vmax = ", ".join(str(resolved[key][1]) for _p, key in STAT_COLUMNS)
        vcap = ", ".join(str(resolved[key][2]) for _p, key in STAT_COLUMNS)

        entries.append({
            "id": item_id,
            "slot": part_slot,
            "name": r.name,
            "vmin": vmin,
            "vmax": vmax,
            "vcap": vcap,
            "weapon_class": weapon_class,
            "wrist_type": wrist_type,
            "enhance_type": enhance_type,
            "source_file": r.source_file,
            "item_id": r.item_id,
            "reason": reasons[r.item_id],
        })

    write_rust(entries)

    print(f"抽出行(対象部位・EquippableItemTemplate): {len(rows)} 件(uniq item_id 抽出は行っていない、部位フィルタ後 {len(candidates)} 件)", file=sys.stderr)
    print(f"収録件数: {len(entries)} 件", file=sys.stderr)
    print(f"既存カタログと同名で突き合わせ対象: {match_total} 件 / 完全一致: {match_exact} 件 / 不一致: {len(mismatches)} 件", file=sys.stderr)
    for m in mismatches[:10]:
        print(f"  不一致: {m}", file=sys.stderr)
    if duplicate_report:
        print(f"同名重複({len(duplicate_report)} 組):", file=sys.stderr)
        for d in duplicate_report:
            print(f"  {d}", file=sys.stderr)

    new_names = {e["name"] for e in entries}
    uncovered_generated = [n for n in re.findall(r'name: "([^"]+)"', (CATALOG_DIR / "generated.rs").read_text(encoding="utf-8")) if n not in new_names]
    print(f"generated.rs の名前で client 側に無いもの: {len(set(uncovered_generated))} 件", file=sys.stderr)
    for n in sorted(set(uncovered_generated))[:20]:
        print(f"  未カバー: {n}", file=sys.stderr)


def write_rust(entries: list[dict]) -> None:
    lines = []
    lines.append("//! クライアント展開データ(dm_NNNNN_NNNN.csv)から抽出した装備カタログ。")
    lines.append("//! 生成元・列対応・番兵の扱いは `tools/gamedata/import_client_db.py` 冒頭のコメント参照。")
    lines.append("//! 再生成: `python tools/gamedata/import_client_db.py`")
    lines.append("")
    lines.append("use super::*;")
    lines.append("")
    lines.append("pub(super) fn client_equipment_catalog() -> Vec<WikiEquipmentItem> {")
    lines.append("    vec![")
    for e in entries:
        weapon_class = f"Some(WeaponClass::{e['weapon_class']})" if e["weapon_class"] else "None"
        enhance_type = f"Some(EquipmentEnhanceType::{e['enhance_type']})" if e["enhance_type"] else "None"
        note = f"収録理由: {'既存カタログと同名' if e['reason'] == 'name_match' else 'Lv280以上'}。EquipType {e['source_file']}"
        name_escaped = e["name"].replace("\\", "\\\\").replace('"', '\\"')
        lines.append("        WikiEquipmentItem {")
        lines.append(f'            id: "{e["id"]}",')
        lines.append(f"            slot: PartSlot::{e['slot']},")
        lines.append(f'            name: "{name_escaped}",')
        lines.append(f"            values_min: v({e['vmin']}),")
        lines.append(f"            values_max: v({e['vmax']}),")
        lines.append("            growth_cap: None,")
        lines.append(f"            enchant_total_caps: v({e['vcap']}),")
        lines.append(f"            weapon_class: {weapon_class},")
        lines.append(f"            enhance_type: {enhance_type},")
        lines.append("            damage_effects: &[],")
        lines.append("            no_ability_or_random_option_slots: false,")
        lines.append("            survival_effects: &[],")
        lines.append("            recommended_dependency: None,")
        lines.append("            damage_dependency: None,")
        lines.append("            source: Source {")
        lines.append(f'                page: "client DB {e["source_file"]} ItemId {e["item_id"]}",')
        lines.append('                retrieved_on: "2026-09-03",')
        lines.append(f'                note: "{note}",')
        lines.append("            },")
        lines.append("        },")
    lines.append("    ]")
    lines.append("}")
    lines.append("")
    lines.append("/// 腕装備(サブアーム)の分類。client DB の EquipType 名から機械的に決まる値で、")
    lines.append("/// `wrist_type_from_page` はページ文字列を見るため client DB 由来の source では効かない")
    lines.append("/// (`items.rs` の `build_equipment_catalog` がこの関数で変換後に上書きする)。")
    lines.append("pub(super) fn client_wrist_type(id: &str) -> Option<WristType> {")
    lines.append("    Some(match id {")
    for e in entries:
        if e["wrist_type"]:
            lines.append(f'        "{e["id"]}" => WristType::{e["wrist_type"]},')
    lines.append("        _ => return None,")
    lines.append("    })")
    lines.append("}")
    lines.append("")
    OUT_PATH.write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    main()
