"""ゲームデータ名の韓国語辞書(`apps/desktop/src/i18n/ko/names.json`)を作る(docs/adr/022-i18n.md 段階 4)。

出典が確実なものだけ入れる。出典・照合方法:

1. 装備名(最優先): 韓国公式の確率公示「装備アイテム検索」API
   (`POST https://static.tales.nexon.com/Probability/filterItemData`)を、
   **ItemID = 日本版クライアントの ItemId** として照合する。crates/gamedata の装備行のうち
   client.rs 由来(出典に `ItemId <id>` を持つ)は出典から直接、それ以外(generated.rs / items.rs /
   sacred_kr.rs 等)は日本版クライアント DB(tw_assets、`TW_ASSETS` 環境変数で場所を切替。
   既定 `C:/github/private/tw_assets/db`)の装備テーブル(ItemId・日本語名の列を持つ CSV、37 個)から
   **日本語名が一意に引ける場合だけ** ItemId を得て照合する。API で取れなかったものは
   旧来のシート照合(下記 2)にフォールバックする。食い違えば API を採り、報告する。
2. 装備名(フォールバック): 韓国コミュニティ装備整理シート(Google スプレッドシート
   `1rT24bRdfsqcX3N4JbRx1dZhqyEAf5OcAPnwPerd18Ds`、sacred_kr.rs と同じ出典)の武器・防具タブを、
   9 値(values_min/max)の完全一致 + 系列名の音の対応で日本 Tale Wiki 名へ照合する。
   候補が 0 件・複数残る・系列が合わないものは入れずに報告する。
3. キャラ名: 韓国公式のキャラ一覧 `https://tales.nexon.com/About/Character` の 19 人と
   crates/gamedata/src/characters.rs の 19 人を id で突き合わせる。
4. 装備アビリティ名(シートの「어빌리티」タブ、公式確率公示 `Probability/Game/6-1`〜`6-5`):
   確実な照合キーが無いため今回は見送り(下記参照)。
5. 敵名: 韓国のファンサイト talesdb.xyz の `assets/monsters.json`(42 件)を、
   crates/gamedata/src/enemies.rs の 42 件と **HP の完全一致**(スタット防御+固定防御の合計が
   `defense` に、固定被ダメ減の符号反転が `damage_reduction` に、(1 - 被ダメ率/100) が `cut_rate_a` に、
   属性値が `element_threshold` に、それぞれ一致することも確認)で照合する。一意に決まるものだけ。
   ライセンス表記が無いサイトなので、原本 JSON はキャッシュにだけ置き(gitignore)、
   names.json に入るのは名前だけ。
6. 攻撃スキル名: 韓国公式のキャラ別スキル記事(`https://tales.nexon.com/About/Character/<Slug>` の
   タブ一覧 `<ul id="character_about_list">` から「스킬」を含むタブの記事番号を取り、
   `https://tales.nexon.com/About/Character/<記事番号>` の JSON `resultValue.content` を解析)を、
   キャラ + **기본 공격력(基本攻撃力%)・타격횟수(ヒット数)の完全一致**で crates/gamedata の
   skills.rs(`†極・<name>` 形式)へ照合する。候補が 0 件・複数残るものは入れない。
   キャラスキル・マスタリーは記事に数値表を持たない説明文だけの節がほとんどで、確実な照合キー
   (固有の数値)が無いため今回は見送り(下記参照)。

前提: `cargo test -p gamedata --test dump_names -- --ignored dump_names` で
`tools/i18n/out/*.json`(統合後の日本語名。gitignore)を作ってあること。このスクリプトは
無ければ自動で実行する。

使い方: `python tools/i18n/import_names.py`
"""

from __future__ import annotations

import csv
import json
import os
import re
import subprocess
import sys
import time
import unicodedata
import urllib.parse
import urllib.request
from pathlib import Path

from bs4 import BeautifulSoup

ROOT = Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "tools" / "i18n" / "out"
CACHE_DIR = ROOT / "tools" / "i18n" / "cache"
NAMES_JSON = ROOT / "apps" / "desktop" / "src" / "i18n" / "ko" / "names.json"
NOTES_JSON = ROOT / "apps" / "desktop" / "src" / "i18n" / "ko" / "notes.json"

SHEET_ID = "1rT24bRdfsqcX3N4JbRx1dZhqyEAf5OcAPnwPerd18Ds"
SHEET_URL = f"https://docs.google.com/spreadsheets/d/{SHEET_ID}/export?format=xlsx"
SHEET_CACHE = CACHE_DIR / "sheet.xlsx"
CHAR_URL = "https://tales.nexon.com/About/Character"
CHAR_CACHE = CACHE_DIR / "characters.html"

# 韓国公式の確率公示「装備アイテム検索」API(段階 4-A、2026-09-25 確認)。ItemID = 日本版
# クライアントの ItemId。ブラウザの User-Agent + この 4 ヘッダが無いと空配列 [] が返る。
KR_ITEM_API_URL = "https://static.tales.nexon.com/Probability/filterItemData"
KR_ITEM_API_HEADERS = {
    "X-Requested-With": "XMLHttpRequest",
    "Origin": "https://static.tales.nexon.com",
    "Referer": "https://static.tales.nexon.com/Probability/Game/8-3",
    "Content-Type": "application/x-www-form-urlencoded; charset=UTF-8",
}
KR_ITEM_WEAR_PARTS = [
    "갑옷", "다리", "렐릭(오른쪽)", "렐릭(왼쪽)", "머리", "몸", "무기", "방패", "손", "투구",
]
KR_ITEM_API_CACHE = CACHE_DIR / "kr_items_api.json"

# 日本版クライアント DB(tw_assets)の装備テーブル(ItemId・日本語名の列を持つ CSV)の場所。
# リポジトリ外の私的な解析用コピー(docs/adr/022-i18n.md 参照)。
TW_ASSETS_DIR = Path(os.environ.get("TW_ASSETS", r"C:\github\private\tw_assets"))
TW_ASSETS_DB_DIR = TW_ASSETS_DIR / "db"

# 敵名の出典。ライセンス表記が無いサイトなので原本はキャッシュにだけ置く(gitignore)。
TALESDB_MONSTERS_URL = "https://talesdb.xyz/assets/monsters.json"
TALESDB_MONSTERS_CACHE = CACHE_DIR / "talesdb_monsters.json"

USER_AGENT = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/120.0 Safari/537.36"
)

# EquipmentValues の並び(crates/domain/src/equipment.rs)。シートは
# …명중,회피,민첩함,크리 の順であり、accuracy の後ろが critical/evasion/agility の順とずれる
# (docs/damage-formula.md、旧 sacred_kr.rs でも同じ並べ替えを行っている)。
STAT_ORDER = [
    "thrust", "slash", "physical_defense", "magic_attack", "magic_defense",
    "accuracy", "critical", "evasion", "agility",
]
# シートの列見出し → EquipmentValues のフィールド名
HEADER_LABELS = {
    "찌르기": "thrust",
    "베기": "slash",
    "방어": "physical_defense",
    "마공": "magic_attack",
    "마방": "magic_defense",
    "명중": "accuracy",
    "크리": "critical",
    "회피": "evasion",
    "민첩함": "agility",
}

# 系列名の音の対応(韓国語の頭 → 日本語の頭)。長い方から試す(改-세크리드 が 세크리드 より先)。
# 実データで確かめた対応(2026-09-24)。
SERIES_MAP = {
    "改-세크리드": "改・セイクリッド",
    "세크리드": "セイクリッド",
    "아노마라드 공화국": "アノマラド共和国",
    "아노마라드 왕국": "アノマラド王国",
    "데모닉": "デモニック",
    "인퍼널": "インファーナル",
    "아퀼루스": "アクィルス",
    "어비스": "アビス",
    "이클립스": "エクリプス",
}
SERIES_ORDER = sorted(SERIES_MAP, key=len, reverse=True)

# 武器・防具の数値タブ。目次・アビリティ・拡張表・レリック(神鳥のブレスレット等、命名規則が
# 別)は対象外。펜듈럼・핸드런처・아밍소드・소드셰이프・해머・물리탄창・마법탄창 は配置が特殊で
# 汎用パーサでは 0 件(試作で確認済み)。読めた分だけ採る。
ITEM_TABS = [
    "세검", "장검", "평도", "대검", "태도", "스태프", "로드", "메이스", "단검", "단도", "도끼",
    "창", "봉", "채찍", "플레일", "스몰소드", "완드", "물리총", "마법총", "클로", "카라", "셉터",
    "핸드벨", "물리검", "마법검", "사이드", "해머", "토템", "핸드런처", "아밍소드", "소드셰이프",
    "펜듈럼", "물리탄창", "마법탄창", "물리검(sub)", "마법검(sub)",
    "로브", "아머", "메일", "마법갑옷", "슈츠",
    "아노마라드 공화국 시리즈", "아노마라드 왕국 시리즈",
    "데모닉 장비 세트", "인퍼널 장비 세트", "아퀼루스 장비 세트", "어비스 장비 세트",
    "이클립스 장비 세트", "세크리드 장비 세트", "改-세크리드 장비 세트",
    "방패", "리스트", "밴드", "암릿", "수정구", "스펠북",
]


def ensure_cache() -> None:
    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    if not SHEET_CACHE.exists():
        print(f"シートを取得: {SHEET_URL}")
        urllib.request.urlretrieve(SHEET_URL, SHEET_CACHE)
    if not CHAR_CACHE.exists():
        print(f"キャラ一覧を取得: {CHAR_URL}")
        req = urllib.request.Request(CHAR_URL, headers={"User-Agent": USER_AGENT})
        with urllib.request.urlopen(req) as resp:
            CHAR_CACHE.write_bytes(resp.read())
    if not TALESDB_MONSTERS_CACHE.exists():
        print(f"敵名を取得: {TALESDB_MONSTERS_URL}")
        req = urllib.request.Request(TALESDB_MONSTERS_URL, headers={"User-Agent": USER_AGENT})
        with urllib.request.urlopen(req, timeout=20) as resp:
            TALESDB_MONSTERS_CACHE.write_bytes(resp.read())


def ensure_dump() -> None:
    if (OUT_DIR / "equipment_catalog.json").exists() and (OUT_DIR / "characters.json").exists():
        return
    print("crates/gamedata の名前を抽出: cargo test -p gamedata --test dump_names -- --ignored dump_names")
    subprocess.run(
        ["cargo", "test", "-p", "gamedata", "--test", "dump_names", "--", "--ignored", "dump_names"],
        cwd=ROOT, check=True,
    )


# ---- 装備名 ----

def parse_min_max(cell) -> tuple[int, int] | None:
    """シートの 1 セルを (min, max) に。'-'/空 は (0, 0)。日付型はシートの入力ミス(バグ)で
    数値として読めないので None(この項目は取り込まない)。"""
    if cell is None:
        return (0, 0)
    if isinstance(cell, bool):
        return None
    if isinstance(cell, (int, float)):
        return (int(cell), int(cell))
    if isinstance(cell, str):
        text = cell.strip()
        if text in ("", "-"):
            return (0, 0)
        if re.fullmatch(r"-?\d+", text):
            v = int(text)
            return (v, v)
        m = re.fullmatch(r"(-?\d+)\s*-\s*(-?\d+)", text)
        if m:
            return (int(m.group(1)), int(m.group(2)))
        return None
    return None  # datetime 等、シート側のバグ


def parse_tab(ws) -> list[dict]:
    rows = list(ws.iter_rows(values_only=True))
    if not rows:
        return []
    header_map: dict[str, int] = {}
    for idx, cell in enumerate(rows[0]):
        if isinstance(cell, str) and cell.strip() in HEADER_LABELS:
            header_map[HEADER_LABELS[cell.strip()]] = idx

    items: list[dict] = []
    pending_name: str | None = None
    for row in rows[1:]:
        first = row[0].strip() if isinstance(row[0], str) else None
        is_option_row = any(isinstance(c, str) and c.strip() == "기본 옵션" for c in row)
        if is_option_row:
            if pending_name is None:
                continue
            mins: list[int] = []
            maxs: list[int] = []
            ok = True
            for stat in STAT_ORDER:
                idx = header_map.get(stat)
                cell = row[idx] if idx is not None else None
                parsed = parse_min_max(cell)
                if parsed is None:
                    ok = False
                    break
                mins.append(parsed[0])
                maxs.append(parsed[1])
            if ok:
                items.append({"name": pending_name, "min": tuple(mins), "max": tuple(maxs)})
            pending_name = None
        elif first and not any(isinstance(c, str) and c.strip() == "한계치" for c in row):
            pending_name = first
    return items


def load_kr_items() -> list[tuple[str, list[dict]]]:
    import openpyxl

    wb = openpyxl.load_workbook(SHEET_CACHE, read_only=True, data_only=True)
    items: list[tuple[str, list[dict]]] = []
    for tab in ITEM_TABS:
        if tab not in wb.sheetnames:
            print(f"  (タブが無い: {tab})")
            continue
        found = parse_tab(wb[tab])
        for it in found:
            it["tab"] = tab
        items.append((tab, found))
    return items


def series_of(kr_name: str) -> tuple[str, str] | None:
    for kr_prefix in SERIES_ORDER:
        if kr_name.startswith(kr_prefix):
            return kr_prefix, SERIES_MAP[kr_prefix]
    return None


# 同じ 9 値を複数の武器種・部位が共有する組(双剣本体/Sub、カーラ/アーミングソード、
# ガントレット/ブーツ 等)がある。系列名だけでは絞れないので、系列の後ろの語で追加照合する
# (実データで確かめた分だけ。2026-09-24)。
KR_TYPE_TO_JP = {
    "카라": "カーラ",
    "아밍소드": "アーミングソード",
    "건틀렛": "ガントレット",
    "부츠": "ブーツ",
}


def narrow_candidates(tab: str, kr_name: str, series_prefix: str, candidates: list[str]) -> list[str]:
    """系列名だけで複数残るとき、タブ・語尾で絞れる分だけ絞る。絞れなければそのまま返す。"""
    if tab in ("물리검(sub)", "마법검(sub)"):
        narrowed = [c for c in candidates if "sub" in c.lower()]
        return narrowed or candidates
    if tab in ("물리검", "마법검"):
        narrowed = [c for c in candidates if "sub" not in c.lower()]
        return narrowed or candidates
    extra = kr_name[len(series_prefix):].strip()
    jp_word = KR_TYPE_TO_JP.get(extra)
    if jp_word:
        narrowed = [c for c in candidates if jp_word in c]
        return narrowed or candidates
    return candidates


def import_equipment(jp_catalog: list[dict], kr_by_tab: list[tuple[str, list[dict]]]) -> dict[str, str]:
    # (min, max) -> 候補(日本語名のリスト)
    jp_index: dict[tuple, list[str]] = {}
    for it in jp_catalog:
        key = (tuple(it["values_min"][s] for s in STAT_ORDER), tuple(it["values_max"][s] for s in STAT_ORDER))
        jp_index.setdefault(key, []).append(it["name"])

    result: dict[str, str] = {}
    counts = {"採用": 0, "候補なし": 0, "候補複数": 0, "系列不明": 0, "系列不一致": 0, "パース失敗": 0}
    conflicts: list[str] = []

    for tab, items in kr_by_tab:
        for it in items:
            key = (it["min"], it["max"])
            candidates = jp_index.get(key, [])
            if not candidates:
                counts["候補なし"] += 1
                continue
            series = series_of(it["name"])
            if series is None:
                counts["系列不明"] += 1
                continue
            kr_prefix, jp_prefix = series
            filtered = [c for c in candidates if c.lstrip("†").startswith(jp_prefix)]
            if len(filtered) > 1:
                filtered = narrow_candidates(tab, it["name"], kr_prefix, filtered)
            if len(filtered) == 0:
                counts["系列不一致"] += 1
                continue
            if len(filtered) > 1:
                counts["候補複数"] += 1
                continue
            jp_name = filtered[0]
            prefix = "†" if jp_name.startswith("†") else ""
            kr_value = prefix + it["name"]
            if jp_name in result and result[jp_name] != kr_value:
                conflicts.append(f"{jp_name!r}: {result[jp_name]!r} != {kr_value!r} ({tab})")
                continue
            result[jp_name] = kr_value
            counts["採用"] += 1

    if conflicts:
        print("衝突(同じ日本語名に違う訳):")
        for c in conflicts:
            print(f"  {c}")
        raise SystemExit(1)

    print(f"装備名: {counts}")
    return result


# ---- 装備名(公式 API、最優先) ----

CLIENT_ITEM_ID_RE = re.compile(r"ItemId (\d+)")


def fetch_kr_official_items() -> list[dict]:
    """韓国公式の確率公示「装備アイテム検索」API から全装備を取得する(キャッシュ優先)。"""
    if KR_ITEM_API_CACHE.exists():
        return json.loads(KR_ITEM_API_CACHE.read_text(encoding="utf-8"))

    def post(data: dict) -> list:
        body = urllib.parse.urlencode(data).encode("utf-8")
        headers = {"User-Agent": USER_AGENT, **KR_ITEM_API_HEADERS}
        req = urllib.request.Request(KR_ITEM_API_URL, data=body, headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=20) as resp:
            return json.loads(resp.read().decode("utf-8"))

    items: list[dict] = []
    for part in KR_ITEM_WEAR_PARTS:
        equip_types = post({"wearPart": part})
        time.sleep(0.3)
        for equip_type in equip_types:
            found = post({"wearPart": part, "equipType": equip_type})
            time.sleep(0.3)
            for it in found:
                items.append({"part": part, "type": equip_type, **it})
        print(f"  {part}: {len(equip_types)} 種類")

    KR_ITEM_API_CACHE.write_text(json.dumps(items, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return items


def load_kr_official_items_by_id() -> dict[str, str]:
    items = fetch_kr_official_items()
    return {it["ItemID"]: it["ItemName"] for it in items}


def load_client_item_ids(jp_catalog: list[dict]) -> dict[str, str]:
    """出典に `ItemId <id>` を持つ行(client.rs 由来)だけ、日本語名 -> ItemId を返す。"""
    result: dict[str, str] = {}
    for it in jp_catalog:
        m = CLIENT_ITEM_ID_RE.search(it["source"]["page"])
        if m:
            result[it["name"]] = m.group(1)
    return result


def load_tw_assets_name_to_id() -> tuple[dict[str, str], int]:
    """tw_assets のクライアント DB(装備テーブル群)から、日本語名 -> ItemId(一意な分だけ)を返す。

    ItemId・日本語名の列を持つ CSV(37 個、`c2_ItemId_*` / `c3_Name_*` が列名)を全部読む。
    同じ名前が複数の ItemId に付くもの(曖昧)は入れない。
    """
    if not TW_ASSETS_DB_DIR.is_dir():
        print(f"  (tw_assets が見つからない: {TW_ASSETS_DB_DIR}。名前→ItemId の照合を見送る)")
        return {}, 0

    # `dm_00000_9NNNN.csv`(テーブル番号が 9 万番台。ItemId も 9000万オフセット)は本編テーブルの
    # 影の複製(開発/検証用と見られる)で、ほぼ全アイテムが本編と同名のまま別 ItemId を持つため
    # 名前の一意性を壊す。本編だけを対象にする。
    SHADOW_TABLE_RE = re.compile(r"_9\d{4}\.csv$")

    name_to_ids: dict[str, set[str]] = {}
    n_files = 0
    for csv_path in sorted(TW_ASSETS_DB_DIR.glob("*.csv")):
        if SHADOW_TABLE_RE.search(csv_path.name):
            continue
        with csv_path.open(encoding="utf-8-sig", newline="") as f:
            reader = csv.reader(f)
            header = next(reader, None)
            if not header or len(header) < 5:
                continue
            # csv の先頭列は連番の `row`(dm_NNNNN_NNNN.d2d ダンプの通し番号)で、
            # 列名(`c2_ItemId_*` 等)はその 1 つ後ろにずれる。
            if not (header[3].startswith("c2_ItemId_") and header[4].startswith("c3_Name_")):
                continue
            n_files += 1
            next(reader, None)  # #type 行
            for row in reader:
                if len(row) < 5:
                    continue
                item_id, name = row[3], row[4]
                if not item_id or not name:
                    continue
                name_to_ids.setdefault(name, set()).add(item_id)
    print(f"  tw_assets: {n_files} ファイル、名前 {len(name_to_ids)} 件")
    ambiguous = sum(1 for ids in name_to_ids.values() if len(ids) > 1)
    unique = {name: next(iter(ids)) for name, ids in name_to_ids.items() if len(ids) == 1}
    return unique, ambiguous


def import_equipment_official(jp_catalog: list[dict]) -> tuple[dict[str, str], dict]:
    """韓国公式の確率公示 API で装備名を照合する(最優先。ItemID = 日本版 ItemId)。"""
    print("韓国公式の装備アイテム検索 API を取得中…")
    kr_by_id = load_kr_official_items_by_id()
    print(f"  韓国公式: {len(kr_by_id)} 件")

    direct = load_client_item_ids(jp_catalog)
    print("tw_assets クライアント DB を読み込み中…")
    db_name_to_id, ambiguous = load_tw_assets_name_to_id()

    result: dict[str, str] = {}
    counts = {"ItemId直接": 0, "DBで名前→ItemId": 0, "候補なし": 0}
    for it in jp_catalog:
        jp_name = it["name"]
        item_id = direct.get(jp_name)
        via = "ItemId直接"
        if item_id is None:
            item_id = db_name_to_id.get(jp_name)
            via = "DBで名前→ItemId"
        if item_id is None:
            continue
        kr_name = kr_by_id.get(item_id)
        if kr_name is None:
            counts["候補なし"] += 1
            continue
        prefix = "†" if jp_name.startswith("†") else ""
        result[jp_name] = prefix + kr_name
        counts[via] += 1

    stats = {"counts": counts, "tw_assets_ambiguous_names": ambiguous, "kr_total": len(kr_by_id)}
    print(f"装備名(公式 API): {counts}(tw_assets で名前が曖昧なため見送り {ambiguous} 件)")
    return result, stats


# ---- 敵名 ----

def import_enemies(jp_enemies: list[dict]) -> dict[str, str]:
    """talesdb.xyz の敵データを HP の完全一致で照合する(一意に決まるものだけ)。"""
    kr_monsters = json.loads(TALESDB_MONSTERS_CACHE.read_text(encoding="utf-8"))["몬스터"]
    hp_to_names: dict[int, list[str]] = {}
    for m in kr_monsters:
        hp_to_names.setdefault(m["HP"], []).append(m["이름"])

    result: dict[str, str] = {}
    counts = {"採用": 0, "候補なし": 0, "候補複数": 0, "HPなし": 0}
    for enemy in jp_enemies:
        hp = enemy.get("hp")
        if hp is None:
            counts["HPなし"] += 1
            continue
        candidates = hp_to_names.get(hp, [])
        if len(candidates) == 0:
            counts["候補なし"] += 1
            continue
        if len(candidates) > 1:
            counts["候補複数"] += 1
            continue
        result[enemy["name"]] = candidates[0]
        counts["採用"] += 1

    print(f"敵名: {counts} / 全 {len(jp_enemies)}")
    return result


# ---- キャラ名 ----

# crates/gamedata/src/characters.rs の id → tales.nexon.com の URL スラッグ(2026-09-24 実データで確認)
CHARACTER_SLUGS = {
    "lucian": "Lucian", "boris": "Boris", "ispin": "Ispin", "maximin": "Maximin",
    "tichiel": "Tichiel", "nayatorei": "Naya", "siberin": "Sivelin", "mira": "Mila",
    "joshua": "Josua", "chloe": "Cloe", "ranjie": "Lanziee", "isaac": "Issac",
    "anais": "Anais", "isolet": "Isolet", "benya": "Benya", "roamini": "Roamini",
    "nocturne": "Nocturne", "leeche": "Clarice", "yefnen": "Yevgnen",
}


def import_characters(jp_characters: list[dict]) -> dict[str, str]:
    html = CHAR_CACHE.read_text(encoding="utf-8")
    by_slug: dict[str, str] = {}
    for m in re.finditer(
        r'href="/About/Character/([^"]+)".*?headline">([^<]+)</span>', html, re.S,
    ):
        slug, kr_full = m.groups()
        by_slug[slug] = kr_full.strip()

    result: dict[str, str] = {}
    missing: list[str] = []
    for char in jp_characters:
        slug = CHARACTER_SLUGS.get(char["id"])
        kr_full = by_slug.get(slug) if slug else None
        if not kr_full:
            missing.append(char["id"])
            continue
        # JP 側は姓を持たないので、韓国語も先頭の名だけを取る(例 "루시안 칼츠" -> "루시안")
        kr_first = kr_full.split()[0]
        result[char["name"]] = kr_first
    if missing:
        print(f"キャラ名: 照合できなかった id = {missing}")
    print(f"キャラ名: 採用 {len(result)} / 全 {len(jp_characters)}")
    return result


# ---- スキル名 ----

# tales.nexon.com はブラウザの Referer が無いと 403 を返す(2026-09-24 実データで確認)。
SKILL_REFERER = "https://tales.nexon.com/"


def skill_headers(referer: str) -> dict:
    return {
        "User-Agent": USER_AGENT,
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "ko-KR,ko;q=0.9,en;q=0.8",
        "Referer": referer,
    }


def fetch_char_page(slug: str) -> str:
    cache = CACHE_DIR / f"skill_char_{slug}.html"
    if not cache.exists():
        print(f"  キャラページ取得: {slug}")
        req = urllib.request.Request(
            f"https://tales.nexon.com/About/Character/{slug}", headers=skill_headers(SKILL_REFERER)
        )
        with urllib.request.urlopen(req, timeout=20) as resp:
            cache.write_bytes(resp.read())
        time.sleep(0.5)
    return cache.read_text(encoding="utf-8")


def skill_article_numbers(html: str) -> list[tuple[int, str]]:
    """タブ一覧から「스킬」を含むものの記事番号とタイトルを返す。"""
    m = re.search(r'<ul id="character_about_list">(.*?)</ul>', html, re.S)
    if not m:
        return []
    tabs = []
    for li in re.finditer(r'data-value="(\d+)"[^>]*>.*?class="text">([^<]+)</span>', m.group(1), re.S):
        no, title = li.groups()
        if "스킬" in title:
            tabs.append((int(no), title))
    return tabs


def fetch_article(no: int, referer: str) -> str:
    cache = CACHE_DIR / f"skill_article_{no}.json"
    if not cache.exists():
        req = urllib.request.Request(
            f"https://tales.nexon.com/About/Character/{no}", headers=skill_headers(referer)
        )
        with urllib.request.urlopen(req, timeout=20) as resp:
            cache.write_bytes(resp.read())
        time.sleep(0.5)
    data = json.loads(cache.read_text(encoding="utf-8"))
    return data["resultValue"]["content"]


# タイトルの先頭に付く丸囲み記号(共通スキル印 ⓒ・パッシブ印 ⓟ 等)。
CIRCLED_PREFIX = re.compile(r"^[①-⓿]+\s*")


def parse_skill_blocks(html: str) -> list[dict]:
    """記事本文からスキル単位のブロック(名前 + 数値表の行)を取り出す。

    タイトル段落(15pt の <span> を持ち、同じ <p> 内に <img> がある)ごとに新しいブロックを始め、
    直後に現れる最初の <table> をその行データとする。表を持たない見出し(説明文だけの技)は
    行が空のブロックとして残す。
    """
    soup = BeautifulSoup(html, "html.parser")
    blocks: list[dict] = []
    current: dict | None = None
    consumed = True
    for el in soup.find_all(["p", "table"]):
        if el.name == "p":
            span = el.find("span", style=lambda s: s and "15pt" in s)
            if span is not None and el.find("img") is not None:
                # 公式の本文は互換漢字(連 = U+F99A など)を含むので NFC にそろえる。
                # 「마법_사격」のように語の区切りが下線になっている見出しがあるので空白に戻す
                text = unicodedata.normalize("NFC", CIRCLED_PREFIX.sub("", span.get_text(strip=True)).strip())
                text = text.replace("_", " ")
                if text:
                    current = {"name": text, "rows": []}
                    blocks.append(current)
                    consumed = False
            continue
        # table
        if current is None or consumed:
            continue
        rows = el.find_all("tr")
        if not rows:
            continue
        header = [c.get_text(strip=True) for c in rows[0].find_all("td")]
        if not header:
            continue
        col: dict[str, int] = {}
        for i, h in enumerate(header):
            if "기본 공격력" in h:
                col["atk"] = i
            elif "타격횟수" in h:
                col["hits"] = i
            elif "크리티컬 배율" in h:
                col["crit"] = i
        if "atk" not in col or "hits" not in col:
            consumed = True
            continue
        for r in rows[1:]:
            cells = [c.get_text(strip=True) for c in r.find_all("td")]
            if len(cells) != len(header):
                continue
            atk_raw, hits_raw = cells[col["atk"]], cells[col["hits"]]
            if not re.fullmatch(r"-?\d+", atk_raw) or not re.fullmatch(r"\d+", hits_raw):
                continue
            row = {"atk": int(atk_raw), "hits": int(hits_raw)}
            if "crit" in col and re.fullmatch(r"\d+", cells[col["crit"]]):
                row["crit"] = int(cells[col["crit"]])
            current["rows"].append(row)
        consumed = True
    return blocks


def load_kr_skill_blocks() -> dict[str, list[dict]]:
    """キャラ id(gamedata 側)-> スキルブロックのリスト。"""
    slugs = list(CHARACTER_SLUGS.items()) + [(None, "Common")]
    by_char: dict[str, list[dict]] = {}
    for char_id, slug in slugs:
        page = fetch_char_page(slug)
        tabs = skill_article_numbers(page)
        referer = f"https://tales.nexon.com/About/Character/{slug}"
        blocks: list[dict] = []
        for no, _title in tabs:
            content = fetch_article(no, referer)
            blocks.extend(parse_skill_blocks(content))
        if char_id is not None:
            by_char[char_id] = blocks
        print(f"  {slug}: {len(tabs)} タブ / {len(blocks)} ブロック")
    return by_char


SKILL_PAIRS_JSON = ROOT / "tools" / "i18n" / "skill_pairs.json"
# 日本語名 = 記号(†)+ 極限(極・)+ 本体 + 形態の接尾。本体だけを公式の韓国語名に置き換える
SKILL_NAME_RE = re.compile(r"^(†?)(極・)?(.+?)(\(味方\)|\(ペナルティ\))?$")
SKILL_SUFFIX_KO = {"(味方)": "(아군)", "(ペナルティ)": "(페널티)"}


def import_skills(
    jp_skills: list[dict], jp_character_skills: list[dict], kr_by_char: dict[str, list[dict]]
) -> dict[str, str]:
    """攻撃スキル・キャラスキルの名前を、韓国公式のキャラ別スキル記事の名前で訳す。

    日韓で倍率・打撃数が合わないことが多く(公式記事が古い・サーバーごとの調整)、数値の一致では
    照合できない。そこで「どの日本語スキルが公式のどの名前か」の対応だけを skill_pairs.json
    (スキル id -> 公式の韓国語名)に持つ。韓国語の文字列はすべて公式由来で、表の名前が
    そのキャラの公式記事に実在しなければ止める。
    """
    pairs: dict[str, str] = json.loads(SKILL_PAIRS_JSON.read_text(encoding="utf-8"))
    by_id: dict[str, tuple[str, str]] = {}
    for sk in jp_skills:
        by_id[sk["id"]] = (sk["name"], sk["id"].split("_")[0])
    for sk in jp_character_skills:
        by_id[sk["id"]] = (sk["name"], sk["game_character_id"])

    errors: list[str] = []
    result: dict[str, str] = {}
    for skill_id, kr_core in pairs.items():
        kr_core = unicodedata.normalize("NFC", kr_core)
        if skill_id not in by_id:
            errors.append(f"{skill_id}: gamedata に無い id")
            continue
        jp_name, char_id = by_id[skill_id]
        official = {b["name"] for b in kr_by_char.get(char_id, [])}
        if kr_core not in official:
            errors.append(f"{skill_id}: {kr_core!r} が {char_id} の公式記事に無い")
            continue
        m = SKILL_NAME_RE.match(jp_name)
        assert m is not None
        star, kyoku, _core, suffix = m.groups()
        kr_value = star + ("극·" if kyoku else "") + kr_core + (SKILL_SUFFIX_KO[suffix] if suffix else "")
        if jp_name in result and result[jp_name] != kr_value:
            errors.append(f"{jp_name!r}: {result[jp_name]!r} と {kr_value!r} で訳が割れる")
            continue
        result[jp_name] = kr_value
    if errors:
        for e in errors:
            print(f"  ✗ {e}")
        raise SystemExit("skill_pairs.json に問題がある")

    attack_ids = {sk["id"] for sk in jp_skills}
    n_attack = sum(1 for i in pairs if i in attack_ids)
    print(
        f"スキル名: 攻撃スキル {n_attack} / {len(jp_skills)}、"
        f"キャラスキル {len(pairs) - n_attack} / {len(jp_character_skills)}(公式記事に無いものは日本語のまま)"
    )
    return result


def replace_names_in_notes(new_skill_names: dict[str, str]) -> int:
    """notes.json の訳文に残っている日本語のスキル名を、今回入った韓国語名に置き換える。

    鍵(日本語原文)はそのまま。値(韓国語訳)の中に日本語のスキル名がそのまま残っている
    部分だけを対象にする。長い名前から試す(短い名前が長い名前の中に含まれる誤爆を避ける)。
    「鍛造」「弱化」のような短い名前は普通の語として文中に出るので、極・付きか 4 字以上の名前に限る。
    """
    if not new_skill_names or not NOTES_JSON.exists():
        return 0
    notes = json.loads(NOTES_JSON.read_text(encoding="utf-8"))
    ordered = sorted(
        ((jp, kr) for jp, kr in new_skill_names.items() if "極・" in jp or len(jp) >= 4),
        key=lambda kv: len(kv[0]),
        reverse=True,
    )
    replaced = 0
    for key, kr_val in notes.items():
        new_val = kr_val
        for jp_name, kr_name in ordered:
            if jp_name in new_val:
                count = new_val.count(jp_name)
                new_val = new_val.replace(jp_name, kr_name)
                replaced += count
        if new_val != kr_val:
            notes[key] = new_val
    if replaced:
        NOTES_JSON.write_text(
            json.dumps(notes, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
            newline="\n",
        )
    return replaced


def main() -> int:
    sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    ensure_cache()
    ensure_dump()

    jp_catalog = json.loads((OUT_DIR / "equipment_catalog.json").read_text(encoding="utf-8"))
    jp_characters = json.loads((OUT_DIR / "characters.json").read_text(encoding="utf-8"))
    jp_skills = json.loads((OUT_DIR / "skills.json").read_text(encoding="utf-8"))
    jp_character_skills = json.loads((OUT_DIR / "character_skills.json").read_text(encoding="utf-8"))
    jp_masteries = json.loads((OUT_DIR / "masteries.json").read_text(encoding="utf-8"))
    jp_enemies = json.loads((OUT_DIR / "enemies.json").read_text(encoding="utf-8"))

    print("装備整理シートを解析中…")
    kr_by_tab = load_kr_items()
    for tab, items in kr_by_tab:
        print(f"  {tab}: {len(items)} 件")

    names: dict[str, str] = {}
    equipment_sheet = import_equipment(jp_catalog, kr_by_tab)
    equipment_official, _official_stats = import_equipment_official(jp_catalog)
    equipment_conflicts = [
        f"{jp!r}: シート {equipment_sheet[jp]!r} != API {kr!r}"
        for jp, kr in equipment_official.items()
        if jp in equipment_sheet and equipment_sheet[jp] != kr
    ]
    if equipment_conflicts:
        print("装備名: シートと公式 API で食い違い(API を採用):")
        for c in equipment_conflicts:
            print(f"  {c}")
    names.update(equipment_sheet)
    names.update(equipment_official)
    names.update(import_characters(jp_characters))
    names.update(import_enemies(jp_enemies))

    print(
        "装備アビリティ名(어빌리티 タブ・公式確率公示 6-1〜6-5): "
        "確実な照合キー(id・client アイテム ID)が無いため見送り(docs/adr/022-i18n.md 参照)"
    )

    print("韓国公式のキャラ別スキル記事を取得中…")
    kr_by_char = load_kr_skill_blocks()
    skill_names = import_skills(jp_skills, jp_character_skills, kr_by_char)
    names.update(skill_names)
    print(f"マスタリー名: 公式記事に載っていないため見送り(全 {len(jp_masteries)})")

    sorted_names = {k: names[k] for k in sorted(names)}
    NAMES_JSON.parent.mkdir(parents=True, exist_ok=True)
    NAMES_JSON.write_text(
        json.dumps(sorted_names, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    print(f"{NAMES_JSON.relative_to(ROOT)}: {len(sorted_names)} 件")

    replaced = replace_names_in_notes(skill_names)
    print(f"notes.json: スキル名の置き換え {replaced} 件")
    return 0


if __name__ == "__main__":
    sys.exit(main())
