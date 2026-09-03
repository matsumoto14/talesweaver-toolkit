"""クライアント展開データ(dm_00000_0264.csv)から称号カタログを取り込み、
`crates/gamedata/src/titles_client.rs` を生成する。

出典: `tw_assets/README.md`(リポジトリ外置き場)。表 0264 が称号(TitleTemplate)テーブル。
CSV は UTF-8 BOM 付き、1 行目=列名(`c<番号>_<名前>_<hash>` / `c<番号>_<hash>`)、
2 行目=`#type`、3 行目以降データ。**CSV の先頭列は `row` なので列 cN は index N+1**。

## 列対応(hash 付き列名 → ドメインの項目。hash は列名由来で安定、番号は将来変わりうる)
- `c2_8df77ce5`(num:I) : 称号 Id(1099 件で一意。id 生成・保存データ突き合わせのキー)
- `c4_9865747d`        : 表示名(既存カタログとの突き合わせキー)
- `c5_Desc_942f9373`   : 説明文(未使用。新規行の group フォールバックにだけ使う)
- `c11_a881b03b`       : 入手条件の文(未使用)
- `c22_99428adf`       : 備考。「ダメージN%増加」(無条件)、「<地域>関連マップで追加ダメージ+N%」、
  「<敵>に追加ダメージ+N%」等。`</n>` は複数行区切り。
- `c13_Thrust_4f358397 .. c21_Agility_21e11676` : 突/斬/物防/魔攻/魔防/命中/Cri/回避/敏捷の 9 値
  (各列はそのまま整数 1 個。装備の `[min,max,cap]` 三つ組とは違う)。
- `c3_08340367`, `c6_36528f56`, `c8_7d71e42d`, `c9_25f2c050`, `c10_65b510fc` : 未同定。
  称号の区分(wiki のページ分け normal / special / event)はクライアントに列が無く、計算にも使わない
  ので持たない(ユーザー決定 2026-09-03)。

## 名前の突き合わせ(正規化)
既存カタログの称号名とクライアントの表示名は素の文字列比較では一致しない行がある:
- 全角/半角の丸括弧・数字、ローマ数字(全角 Ⅰ〜Ⅹ と半角 I/II/III… の混在。
  例: 既存「ルーンの絆Ⅱ」↔ client「ルーンの絆II」、既存「ルーンの絆Ⅲ」↔ client も「ルーンの絆Ⅲ」)
- 「名誉の証(◯◯)」(既存カタログの命名)↔ client は中身の「◯◯」だけを表示名にしている
  (例: 既存「名誉の証（ぼのぼの）」↔ client「ぼのぼの」)。ピリオドの有無も割れる
  (既存「名誉の証（D.D.D）」↔ client「D.D.D.」)。
`normalize()` で 全角→半角・ローマ数字→ASCII・空白/ピリオド除去のうえ比較し、
「名誉の証(...)」形は中身だけでも突き合わせる。**この突き合わせで既存 120 件は全件が
client 側に対応行を持つ**(不一致 0 件)。

## 収録基準(いずれか 1 つで収録)
(a) 既存カタログと同名(上記正規化後)
(b) 9 値合計が 15 以上
(c) 備考(c22)に「ダメージ」を含む
同名で複数行あるもの(称号は Id 違いでレベル違いの複製が無いはずが、実際には
「ルーンの絆」等が名前は同じで異なる行として 2 件存在する組が複数ある)は、
既存カタログに同名行があればその 9 値と完全一致する行を採用し、無ければ Id 最小の行を代表とする。

## 既存行の扱い
既存 120 件の id・group・level・common・conditional_added_damage・note は
`titles.rs` の手書きのまま変えない。**9 値だけ**この生成物の
`CLIENT_TITLE_VALUE_OVERRIDES`(id → EquipmentValues)で `titles.rs::title_catalog()` が
上書きする(装備の `client_equipment_catalog()` と同じ「id/名前を引き継いだうえで数値だけ正にする」
考え方だが、称号は id で直接引くほうが、装備のような「同名なら丸ごと差し替え」より
既存メタデータを壊さないため id 引きにした)。

## 新規行の扱い(`client_title_catalog()`)
既存カタログに無い行は `TitleDef` をまるごと生成する。
- id: `client-<Id>`
- group: 名前が `<シリーズ名> - <サフィックス>` の形(依存別サフィックス違いの組。
  wiki 由来の命名慣習と同じ)なら `<シリーズ名>` 部分、それ以外は名前そのもの
  (対応表が無いための既定。c5_Desc は文が長く見出しに使えないため採用しなかった)
- level: c10 が素の整数のときだけ、それ以外は None
- common: 無条件ダメージ増加が 20% 以上のときだけ true(既存規則)
- conditional_added_damage: 備考が「<地域>関連マップで追加ダメージ+N%」または
  「<敵>に追加ダメージ+N%」の形で、対応する `GameRegion` / 既存の敵条件 id
  (shirairon/silvan/serion/sereana/luminous/deep_apostle。新規称号にはこれ以外の
  敵条件が出現しなかったため、新しい敵カタログは追加していない)に一致するときだけ設定。
  それ以外(「経験値獲得量+150%」「被ダメージ10%減少」等、器に無い効果)は note の文に残すのみ。
- attack_damage_percent: 備考が「ダメージN%増加」「攻撃ダメージ(基本発動)N%増加」
  「追加ダメージ+N%」等、地域・敵の指定が無い無条件表現のときに設定。
- note: 備考(`</n>` は " / " に変換)をそのまま。

使い方:
    python tools/gamedata/import_client_titles.py [--assets PATH]
`--assets` 省略時は環境変数 `TW_ASSETS`、それも無ければ `C:\\github\\private\\tw_assets`。
"""
from __future__ import annotations

import argparse
import csv
import os
import re
import sys
import unicodedata
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GAMEDATA_SRC = ROOT / "crates/gamedata/src"
TITLES_RS = GAMEDATA_SRC / "titles.rs"
OUT_PATH = GAMEDATA_SRC / "titles_client.rs"

STAT_COLUMNS = [
    "c13_Thrust", "c14_Slash", "c15_", "c16_MagAtk", "c17_",
    "c18_", "c19_Crit", "c20_", "c21_Agility",
]

# 備考中の「地域関連マップで追加ダメージ」を GameRegion に対応づける(既存 4 地域のみ)。
REGION_KEYWORDS = {
    "喪失の島": "LostIsland",
    "神鳥の塒": "ShinchouNest",
    "アークロン地下要塞": "ArklonUnderground",
    "アークロン要塞": "ArklonUnderground",
    "プラバ": "Praba",
}
# 備考中の「<敵>に追加ダメージ」を既存の敵条件 id に対応づける(新規称号で他の敵は出現しなかった)。
ENEMY_KEYWORDS = {
    "シライロン": "shirairon",
    "シルバン": "silvan",
    "セリオン": "serion",
    "セレアナ": "sereana",
    "ルミナス": "luminous",
    "深淵の使徒": "deep_apostle",
}


def default_assets_dir() -> Path:
    env = os.environ.get("TW_ASSETS")
    if env:
        return Path(env)
    return Path(r"C:\github\private\tw_assets")


def signed_byte(value: int) -> int:
    """9 値の列(num:B)は符号付き 1 バイト。128 以上は負値(255 → -1)。"""
    return value - 256 if value >= 128 else value


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


ROMAN_MAP = str.maketrans({
    "Ⅰ": "I", "Ⅱ": "II", "Ⅲ": "III", "Ⅳ": "IV", "Ⅴ": "V",
    "Ⅵ": "VI", "Ⅶ": "VII", "Ⅷ": "VIII", "Ⅸ": "IX", "Ⅹ": "X",
})


def normalize(name: str) -> str:
    s = unicodedata.normalize("NFKC", name)  # 全角英数・括弧・空白を半角化
    s = s.translate(ROMAN_MAP)
    s = s.replace(".", "").replace(" ", "")
    return s


HONOR_RE = re.compile(r"^名誉の証[（(](.+)[）)]$")


def normalize_keys(name: str) -> list[str]:
    """比較に使う正規化キー(複数)。「名誉の証(...)」形は中身だけのキーも返す。"""
    n = normalize(name)
    keys = [n]
    m = HONOR_RE.match(n)
    if m:
        keys.append(normalize(m.group(1)))
    return keys


@dataclass
class TitleRow:
    item_id: str
    name: str
    c3: str
    c8: str
    c9: str
    c10: str
    remark: str
    values: tuple[int, ...]

    @property
    def sum(self) -> int:
        return sum(self.values)


def load_title_rows(assets: Path) -> list[TitleRow]:
    header, data = load_csv(assets / "db" / "dm_00000_0264.csv")
    id_col = col_index(header, "c2_")
    name_col = col_index(header, "c4_")
    c3_col = col_index(header, "c3_")
    c8_col = col_index(header, "c8_")
    c9_col = col_index(header, "c9_")
    c10_col = col_index(header, "c10_")
    remark_col = col_index(header, "c22_")
    stat_cols = [col_index(header, p) for p in STAT_COLUMNS]
    rows = []
    for row in data:
        try:
            # num:B は符号付き 1 バイト。マスター系称号の「敏捷 -1」等が 255 / 254 / 253 で入っている
            values = tuple(signed_byte(int(row[c])) for c in stat_cols)
        except ValueError:
            values = tuple(0 for _ in stat_cols)
        rows.append(TitleRow(
            item_id=row[id_col],
            name=row[name_col],
            c3=row[c3_col],
            c8=row[c8_col],
            c9=row[c9_col],
            c10=row[c10_col],
            remark=row[remark_col],
            values=values,
        ))
    return rows


# ── 既存カタログ(titles.rs)からの名前→id・値の抽出 ──

# t("id", "name", "group", level, v(...), "note") / td(..., attack_damage_percent, "note")
CALL_RE = re.compile(
    r'\b(?:t|td)\(\s*"([^"]+)",\s*"((?:[^"\\]|\\.)*)",\s*"[^"]*",\s*[^,]+,\s*'
    r'v\(([^)]*)\)',
    re.S,
)


def load_existing_catalog() -> dict[str, tuple[str, tuple[int, ...]]]:
    """正規化キー → (id, values)。1 キーに複数一致する場合は最初のものを残す。"""
    text = TITLES_RS.read_text(encoding="utf-8")
    out: dict[str, tuple[str, tuple[int, ...]]] = {}
    for m in CALL_RE.finditer(text):
        item_id, name, vlist = m.group(1), m.group(2), m.group(3)
        name = name.replace('\\"', '"').replace("\\\\", "\\")
        values = tuple(int(x.strip()) for x in vlist.split(","))
        for key in normalize_keys(name):
            out.setdefault(key, (item_id, values))
    return out


def parse_remark(remark: str) -> tuple[float, str | None, str | None, str]:
    """備考 → (attack_damage_percent, region_variant, enemy_id, note)。
    region_variant は GameRegion のバリアント名文字列。"""
    note = remark.replace("</n>", " / ")
    segments = remark.split("</n>")
    attack_damage_percent = 0.0
    region_variant = None
    enemy_id = None
    for seg in segments:
        # 地域条件: 「<地域>関連マップで追加ダメージ+N%」
        region_match = None
        for kw, variant in REGION_KEYWORDS.items():
            if kw in seg and "関連マップ" in seg and ("追加ダメージ" in seg or "ダメージ" in seg):
                region_match = variant
                break
        enemy_match = None
        for kw, eid in ENEMY_KEYWORDS.items():
            if kw in seg and "追加ダメージ" in seg:
                enemy_match = eid
                break
        pct_match = re.search(r"(\d+(?:\.\d+)?)[%％]", seg)
        if region_match and pct_match:
            region_variant = region_match
        elif enemy_match and pct_match:
            enemy_id = enemy_match
        elif pct_match and ("ダメージ" in seg) and "被ダメージ" not in seg and "減少" not in seg:
            # 無条件のダメージ増加(「ダメージN%増加」「攻撃ダメージ(基本発動)N%増加」「追加ダメージ+N%」「与ダメージ N%増加」)
            attack_damage_percent = float(pct_match.group(1))
    return attack_damage_percent, region_variant, enemy_id, note


SUFFIX_RE = re.compile(r"^(.+?)\s*[-－]\s*(.+)$")


def derive_group(name: str) -> str:
    m = SUFFIX_RE.match(name)
    if m:
        return m.group(1).strip()
    return name


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

    rows = load_title_rows(assets)
    old_catalog = load_existing_catalog()  # 正規化キー -> (id, old_values)
    old_ids = {v[0] for v in old_catalog.values()}

    # 収録基準: (a) 既存と同名、(b) 9値合計>=15、(c) 備考に「ダメージ」を含む
    selected: list[TitleRow] = []
    for r in rows:
        keys = normalize_keys(r.name)
        is_name_match = any(k in old_catalog for k in keys)
        is_sum15 = r.sum >= 15
        is_damage = "ダメージ" in r.remark
        if is_name_match or is_sum15 or is_damage:
            selected.append(r)

    # 同名重複の解決(生の name で group化。正規化キーでの重複も別途チェック)
    by_name: dict[str, list[TitleRow]] = {}
    for r in selected:
        by_name.setdefault(r.name, []).append(r)

    duplicate_report: list[str] = []
    resolved: list[TitleRow] = []
    for name, group in by_name.items():
        if len(group) == 1:
            resolved.append(group[0])
            continue
        chosen = None
        for key in normalize_keys(name):
            if key in old_catalog:
                _id, want = old_catalog[key]
                for r in group:
                    if r.values == want:
                        chosen = r
                        break
            if chosen:
                break
        if chosen is None:
            chosen = sorted(group, key=lambda r: int(r.item_id))[0]
            duplicate_report.append(
                f"{name}: {len(group)} 行中 ItemId {chosen.item_id} を代表として採用(既存値と一致する行なし)"
            )
        else:
            duplicate_report.append(
                f"{name}: {len(group)} 行中 ItemId {chosen.item_id} を採用(既存値と一致)"
            )
        resolved.append(chosen)

    # 既存カタログとの突き合わせ: 値の上書き表
    overrides: list[tuple[str, tuple[int, ...]]] = []
    matched_old_ids: set[str] = set()
    match_exact = 0
    match_diff = 0
    for r in resolved:
        found_id = None
        for key in normalize_keys(r.name):
            if key in old_catalog:
                found_id = old_catalog[key][0]
                break
        if found_id is None:
            continue
        if found_id in matched_old_ids:
            continue
        matched_old_ids.add(found_id)
        old_values = next(v for k, v in old_catalog.items() if v[0] == found_id)[1]
        if old_values == r.values:
            match_exact += 1
        else:
            match_diff += 1
        overrides.append((found_id, r.values))

    unmatched_old = old_ids - matched_old_ids
    for oid in sorted(unmatched_old):
        print(f"警告: 既存 id '{oid}' に対応する client 行が見つかりません(9値は据え置き)", file=sys.stderr)

    # 新規行(既存に無いもの)
    new_rows = [r for r in resolved if not any(k in old_catalog for k in normalize_keys(r.name))]

    structured = 0
    unstructured = 0
    new_entries = []
    used_ids: set[str] = set()
    for r in sorted(new_rows, key=lambda r: int(r.item_id)):
        item_id = f"client-{r.item_id}"
        if item_id in used_ids:
            continue
        used_ids.add(item_id)

        attack_damage_percent, region_variant, enemy_id, note = parse_remark(r.remark)
        conditional = None
        if region_variant:
            conditional = ("Region", f"GameRegion::{region_variant}")
            structured += 1
        elif enemy_id:
            conditional = ("Enemy", f'"{enemy_id}"')
            structured += 1
        elif "ダメージ" in r.remark and attack_damage_percent == 0.0:
            unstructured += 1

        common = attack_damage_percent >= 20.0
        level = int(r.c10) if r.c10.isdigit() else None
        group = derive_group(r.name)

        new_entries.append({
            "id": item_id,
            "name": r.name,
            "group": group,
            "level": level,
            "values": r.values,
            "attack_damage_percent": attack_damage_percent,
            "conditional": conditional,
            "note": note,
            "common": common,
            "item_id": r.item_id,
        })

    write_rust(overrides, new_entries, structured > 0)

    print(f"クライアント側総行数: {len(rows)}", file=sys.stderr)
    print(f"収録基準を満たす行(重複解決前): {len(selected)}", file=sys.stderr)
    print(f"重複解決後: {len(resolved)}", file=sys.stderr)
    if duplicate_report:
        print(f"同名重複({len(duplicate_report)} 組):", file=sys.stderr)
        for d in duplicate_report:
            print(f"  {d}", file=sys.stderr)
    print(f"既存カタログ突き合わせ: {len(old_ids)} 件中 {len(matched_old_ids)} 件が client に対応 "
          f"(完全一致 {match_exact} / 値差分 {match_diff} / 対応なし {len(unmatched_old)})", file=sys.stderr)
    print(f"新規行: {len(new_entries)} 件(条件付きダメージを構造化: {structured} 件 / 備考のまま: {unstructured} 件)",
          file=sys.stderr)


def rust_escape(s: str) -> str:
    return s.replace("\\", "\\\\").replace('"', '\\"')


def write_rust(overrides: list[tuple[str, tuple[int, ...]]], new_entries: list[dict], needs_conditional: bool) -> None:
    lines = []
    lines.append("//! クライアント展開データ(dm_00000_0264.csv)から抽出した称号カタログ。")
    lines.append("//! 列対応・区分の規則・収録基準は `tools/gamedata/import_client_titles.py` 冒頭の")
    lines.append("//! コメント、および docs/adr/004-equipment-model.md 決定17追記参照。")
    lines.append("//! 再生成: `python tools/gamedata/import_client_titles.py`")
    lines.append("")
    if needs_conditional:
        lines.append("use domain::content::GameRegion;")
        lines.append("use domain::{AddedDamageCondition, ConditionalAddedDamage, EquipmentValues, TitleDef};")
    else:
        lines.append("use domain::{EquipmentValues, TitleDef};")
    lines.append("")
    lines.append("")
    lines.append("#[rustfmt::skip]")
    lines.append("const fn v(")
    lines.append("    thrust: i64, slash: i64, physical_defense: i64, magic_attack: i64, magic_defense: i64,")
    lines.append("    accuracy: i64, critical: i64, evasion: i64, agility: i64,")
    lines.append(") -> EquipmentValues {")
    lines.append("    EquipmentValues {")
    lines.append("        thrust, slash, physical_defense, magic_attack, magic_defense,")
    lines.append("        accuracy, critical, evasion, agility,")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append("/// 既存カタログ id → クライアントの9値。`titles::title_catalog()` が id で引いて")
    lines.append("/// 既存定義(group/level/note/common/conditional_added_damage)は変えずに")
    lines.append("/// 9値だけ上書きする。")
    lines.append("pub(super) const CLIENT_TITLE_VALUE_OVERRIDES: &[(&str, EquipmentValues)] = &[")
    for item_id, values in overrides:
        vstr = ", ".join(str(x) for x in values)
        lines.append(f'    ("{item_id}", v({vstr})),')
    lines.append("];")
    lines.append("")
    lines.append("/// クライアントにしか無い新規称号(既存カタログに同名が無い行)。")
    lines.append("pub(super) fn client_title_catalog() -> Vec<TitleDef> {")
    lines.append("    vec![")
    for e in new_entries:
        vstr = ", ".join(str(x) for x in e["values"])
        name = rust_escape(e["name"])
        group = rust_escape(e["group"])
        note = rust_escape(e["note"])
        level = f'Some({e["level"]})' if e["level"] is not None else "None"
        if e["conditional"] is None:
            cond = "None"
        else:
            kind_, val = e["conditional"]
            cond = (
                f"Some(ConditionalAddedDamage {{ percent: {e['attack_damage_percent'] if e['attack_damage_percent'] else 10.0}, "
                f"condition: AddedDamageCondition::{kind_}({val}) }})"
            )
        lines.append("        TitleDef {")
        lines.append(f'            id: "{e["id"]}",')
        lines.append(f'            name: "{name}",')
        lines.append(f'            group: "{group}",')
        lines.append(f"            level: {level},")
        lines.append(f"            values: v({vstr}),")
        lines.append(f"            attack_damage_percent: {e['attack_damage_percent']},")
        lines.append(f"            conditional_added_damage: {cond},")
        lines.append(f'            note: "{note}",')
        lines.append(f"            common: {'true' if e['common'] else 'false'},")
        lines.append(f'            // client DB dm_00000_0264.csv Id {e["item_id"]}')
        lines.append("        },")
    lines.append("    ]")
    lines.append("}")
    lines.append("")
    OUT_PATH.write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    main()
