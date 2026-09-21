# チャネリングスキル(押している間、一定間隔で攻撃を繰り返す技)を wiki から拾って
# Rust の表にする。
#
# 出典: wiki 各キャラの `Skill/<キャラ名>` ページ「スキル性能一覧」。
# 1. **区分** 列に `続` を含む行がチャネリングスキル(凡例は wiki「Skill#f8e303fb」)。
#    区分は `QA続` のように他の印と連なり `BGCOLOR(...):` が付く行もあるので**部分一致**で見る。
# 2. **攻撃力** 列の `492%x10&br;(1s毎)` から **1 回の攻撃(tick)の間隔**を取る。
#    `(0.8s間隔)` 表記のページもある。`x10` は 1 tick の段数で、`Skill` の段数(hit_count)と同じ。
# 3. **動作** 列(持続秒)÷ tick 間隔 = **1 回の使用で何 tick 撃つか**。
#    動作が秒として読めない行(スパークリングカイトの `0`)は **対象指定** 列の `持続Ns` に
#    フォールバックし、生成物のコメントに `[対象指定]` と印を付ける。
#
# 表の `~` は「上の行と同じ」なので直前の値を引き継ぐ(継承を誤ると隣の技に漏れる)。
#
# 入力は wiki に取りに行かず、ローカルミラー(`tools/gamedata/wiki/store.py` の `Store`)を読む。
#
# 使い方(リポジトリルートで):
#   python tools/gamedata/skill_channeling.py > crates/gamedata/src/skill_channeling.rs
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")
sys.stderr.reconfigure(encoding="utf-8")

from pathlib import Path  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent))
from skill_cooldowns import cells_of, characters, is_noise, known_skills  # noqa: E402
from wiki.store import Store  # noqa: E402

TICK = re.compile(r"\((\d+(?:\.\d+)?)s\s*(?:毎|間隔)\)")
SECONDS = re.compile(r"^(\d+(?:\.\d+)?)s$")
LASTS = re.compile(r"持続(\d+(?:\.\d+)?)s")


def rows_of(source: str):
    """スキル性能一覧から、区分に `続` を含む行の (表示名, 攻撃力, 動作, 対象指定) を返す。"""
    found = []
    header: list[str] | None = None
    previous: dict[str, str] = {}
    for line in source.split("\n"):
        if is_noise(line):
            continue
        if line.startswith("|スキル|区分|依存|") and line.rstrip().endswith("h"):
            header = cells_of(line.rstrip().rstrip("h"))
            previous = {}
            continue
        if header is None or not line.startswith("|") or "[[" not in line:
            continue
        cells = cells_of(line)
        if len(cells) != len(header) or ">" in cells:
            continue
        name = re.search(r"\[\[(.+?)>#(\w+)\]\]", cells[0])
        if name is None:
            continue
        value = {}
        for column in ("区分", "攻撃力", "動作", "対象指定"):
            raw = cells[header.index(column)] if column in header else ""
            value[column] = previous.get(column, "") if raw == "~" else raw
            previous[column] = value[column]
        if "続" in value["区分"]:
            found.append((name.group(1), value))
    return found


def clean(value: str) -> str:
    return re.sub(r"(COLOR|BGCOLOR)\([^)]*\):", "", value).strip()


def ticks_of(value: dict[str, str]) -> tuple[int, float, str] | str:
    """(tick 数, tick 間隔, 持続の出どころ)。読めない表記はその旨を文字列で返す。"""
    tick = TICK.search(clean(value["攻撃力"]))
    if tick is None:
        return f"攻撃力の tick 間隔が読めない: {value['攻撃力']!r}"
    interval = float(tick.group(1))
    if interval <= 0:
        return f"tick 間隔が 0: {value['攻撃力']!r}"
    note = ""
    lasts = SECONDS.match(clean(value["動作"]))
    if lasts is None:
        fallback = LASTS.search(clean(value["対象指定"]))
        if fallback is None:
            return f"持続が読めない: 動作 {value['動作']!r} / 対象指定 {value['対象指定']!r}"
        seconds = float(fallback.group(1))
        note = "[対象指定] "
    else:
        seconds = float(lasts.group(1))
    count = round(seconds / interval)
    if count < 1:
        return f"tick 数が 1 未満: 持続 {seconds}s / 間隔 {interval}s"
    return count, interval, note


def main() -> None:
    store = Store()
    skills = known_skills()
    out: list[tuple[str, int, float, str]] = []
    problems: list[str] = []
    for character_id, character_name in characters():
        page = store.db.execute(
            "SELECT source FROM page WHERE name = ?", (f"Skill/{character_name}",)
        ).fetchone()
        if page is None or page["source"] is None:
            print(f"// ミラーに無いページ: Skill/{character_name}", file=sys.stderr)
            continue
        for name, value in rows_of(page["source"]):
            sid = skills.get((character_id, name))
            if sid is None:
                problems.append(f"{character_id} {name}: id が分からない")
                continue
            resolved = ticks_of(value)
            if isinstance(resolved, str):
                problems.append(f"{character_id}_{sid} {name}: {resolved}")
                continue
            count, interval, note = resolved
            out.append(
                (
                    f"{character_id}_{sid}",
                    count,
                    interval,
                    f"{note}{character_name} {name}({clean(value['攻撃力'])} 持続 {clean(value['動作'])})",
                )
            )

    print("//! チャネリングスキル(押している間、一定間隔で攻撃を繰り返す技)の tick。")
    print("//!")
    print("//! 出典: wiki 各キャラの `Skill/<キャラ名>` ページ「スキル性能一覧」の区分列に")
    print("//! `続` を含む行(凡例は wiki「Skill#f8e303fb」)。攻撃力列の `(Ns毎)` が 1 回の攻撃")
    print("//! (tick)の間隔で、動作列の持続秒 ÷ 間隔 = **1 回の使用で撃つ tick 数**。")
    print("//! `[対象指定]` 印の行は動作列が秒として読めないので対象指定列の `持続Ns` から採った。")
    print("//! `tools/gamedata/skill_channeling.py` が生成する。手で編集しない。")
    print("//!")
    print("//! 表記の `492%x10` の `x10` は **1 tick の段数**(`Skill::hit_count`)で、")
    print("//! 1 回の使用ぶんの合計は `段数 × tick 数`。")
    print("")
    print("/// `(スキル id, 1 回の使用で撃つ tick 数, tick の間隔(秒))`")
    print("#[rustfmt::skip]")
    print("pub(crate) const SKILL_CHANNELING: &[(&str, u32, f64)] = &[")
    for sid, count, interval, note in sorted(out):
        print(f'    ("{sid}", {count}, {interval}),  // {note}')
    print("];")
    if problems:
        print(f"// 読めなかった行: {len(problems)} 件", file=sys.stderr)
        for m in problems:
            print(f"//   {m}", file=sys.stderr)


if __name__ == "__main__":
    main()
