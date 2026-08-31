"""コンボインターバル(CI)を wiki から取り込み、Rust の表を出力する。

出典: Tale Wiki「計算式まとめ」#g7881516 の CI 値表(通常攻撃ごとの ms)。
CI は「通常攻撃の中ディレイが終わってから、次の行動が撃てるまで」の待ち時間で、
最速コンボでは実質「次に使うスキルの中ディレイの下限」として効く。

wiki の表はキャラ名 + 通常攻撃名 + ms なので、gamedata の † スキル(基本攻撃)と
名前で突き合わせて `(skill_id, 秒)` の表にする。突き合わせできなかった行は
**捨てずに標準エラーへ出す**(黙って落とすと、CI が無い = 下限なしとして
DPS が実際より速く出てしまう)。

使い方:
    python tools/gamedata/import_combo_intervals.py > /tmp/combo_intervals.txt
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WIKI_PAGE = "計算式まとめ"


def fetch_source(page: str) -> str:
    """PukiWiki の ?cmd=source を EUC-JP から UTF-8 にして返す。"""
    url = f"https://talewiki.com/?cmd=source&page={requote(page)}"
    raw = subprocess.run(["curl", "-s", url], capture_output=True, check=True).stdout
    return raw.decode("euc_jp", errors="replace")


def requote(text: str) -> str:
    return "".join(f"%{b:02X}" for b in text.encode("euc_jp"))


def parse_intervals(source: str) -> list[tuple[str, str, float]]:
    """CI 値表の行を (キャラ名, 通常攻撃名, 秒) にする。"""
    rows: list[tuple[str, str, float]] = []
    in_region = False
    for line in source.splitlines():
        if line.startswith("#region(CI値)"):
            in_region = True
            continue
        if in_region and line.startswith("#endregion"):
            break
        if not in_region:
            continue
        cells = [c.strip() for c in line.split("\t") if c.strip()]
        if len(cells) < 3:
            continue
        character, attack, value = cells[0], cells[1], cells[2]
        # 「270(実測250)」は実測値を採る(表の設定値と実際が違う旨は wiki 自身の注記)
        measured = re.search(r"実測(\d+)", value)
        number = measured.group(1) if measured else re.match(r"\d+", value)
        if number is None:
            continue
        ms = int(number if isinstance(number, str) else number.group(0))
        rows.append((character, attack, ms / 1000.0))
    return rows


def normal_attacks() -> dict[tuple[str, str], str]:
    """gamedata の † スキル(基本攻撃)を {(キャラ名, 攻撃名): skill_id} で返す。"""
    characters = dict(
        re.findall(r'id: "([a-z_]+)",\s*\n\s*name: "([^"]+)"', (ROOT / "crates/gamedata/src/characters.rs").read_text(encoding="utf-8"))
    )
    catalog: dict[tuple[str, str], str] = {}
    for character_id, skill_id, name in re.findall(
        r's\("([a-z_]+)", "([a-z0-9_]+)", "†[^・]*・([^"]+)"', (ROOT / "crates/gamedata/src/skills.rs").read_text(encoding="utf-8")
    ):
        catalog[(characters[character_id], normalize(name))] = f"{character_id}_{skill_id}"
    return catalog


def main() -> None:
    # Windows の既定は cp932 なので、生成した Rust コードが文字化けしないよう UTF-8 に固定する
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    rows = parse_intervals(fetch_source(WIKI_PAGE))
    catalog = normal_attacks()
    matched: list[tuple[str, float, str, str]] = []
    for character, attack, seconds in rows:
        # 半角カナ表記(ﾏｼﾞｶﾙﾃﾞｭｱﾙｼｮｯﾄ)は wiki 側の省略。全角に直して照合する
        key = (character, normalize(attack))
        skill_id = catalog.get(key)
        if skill_id is None:
            print(f"未対応: {character} {attack} {seconds}s", file=sys.stderr)
            continue
        matched.append((skill_id, seconds, character, attack))

    for key, skill_id in sorted(catalog.items()):
        if not any(m[0] == skill_id for m in matched):
            print(f"CI 未収録: {key[0]} {key[1]} ({skill_id})", file=sys.stderr)

    print("#[rustfmt::skip]")
    print("const COMBO_INTERVALS: &[(&str, f64)] = &[")
    for skill_id, seconds, character, attack in matched:
        print(f'    ("{skill_id}", {seconds}),  // {character} {attack}')
    print("];")
    print(f"// {len(matched)} 件", file=sys.stderr)


def normalize(name: str) -> str:
    """表記ゆれを吸収する。半角カナ(ﾏｼﾞｶﾙﾃﾞｭｱﾙｼｮｯﾄ)と中黒(ダークネス・フレア)は
    CI 値表と スキル性能一覧 で揺れるので、照合の前に落とす。"""
    import unicodedata

    return unicodedata.normalize("NFKC", name).replace("・", "")


if __name__ == "__main__":
    main()
