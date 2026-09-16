#!/usr/bin/env python3
"""デザインシステム(docs/design-system.html)適合の機械監査。

**候補を出すだけで、判定はしない。**閾値での自動修正もしない。
出てきたものは goal 文書(docs/claude/goals/2026-08-25-design-conformance.md)の
「② 分類」にかけて、違反 / 規格の穴 / 決定待ち に振り分けること。

    python tools/design-audit/run.py            # 全ルール
    python tools/design-audit/run.py R2 R4      # ルールを絞る
    python tools/design-audit/run.py --list     # ルール一覧
"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "apps" / "desktop" / "src"
APP_CSS = SRC / "app.css"

# --- 規格の定数(docs/design-system.html の各節) ---------------------------

RADIUS_STEPS = {"12px": "--r-window", "9px": "--r-panel", "6px": "--r-inset", "999px": "--r-pill"}
RADIUS_FREE = {"50%", "0", "0px", "inherit", "initial", "unset"}  # 段に数えない形
ICON_SIZES = {20, 28, 40, 64}
MAX_DURATION_S = 0.5

# §03 状態の 6 系統。bg / border の組で 1 系統。
STATE_SYSTEMS = [
    ("余裕・目標", "#dcebff", "#426dd6"),
    ("足りている", "#dff3e6", "#6fa98a"),
    ("ぎりぎり・操作待ち", "#fdf3de", "#c2a057"),
    ("届かない・危険", "#f6e8e5", "#b08480"),
    ("対象外・判定不能", "#eceef2", "#a9b4c4"),
    ("一時・チーム条件", "#efeef8", "#6d6aa8"),
]
STATE_OF: dict[str, tuple[int, str]] = {}
for _i, (_name, _bg, _bd) in enumerate(STATE_SYSTEMS):
    STATE_OF[_bg] = (_i, _name)
    STATE_OF[_bd] = (_i, _name)

RULES = {
    "R1": ("トークンがあるのに色を直書き", "§03 / §15"),
    "R2": ("4 段以外の border-radius", "§04"),
    "R3": ("--r-* を使わない border-radius の直書き", "§04"),
    "R4": ("数値書体に tabular-nums が無い", "§05"),
    "R5": ("アイコンサイズが 20 / 28 / 40 / 64 以外", "§06"),
    "R6": ("0.5s を超える transition / animation", "§10"),
    "R7": ("状態 6 系統をまたいだ色の組み合わせ", "§03"),
    "R8": ("TS / Svelte に色の実値を直書き", "§15"),
    "R9": ("枠のある操作部品に border-radius が無い", "§04"),
    "R10": ("font-size が実寸スケールの外", "§05"),
    "R11": ("format.ts の外で toLocaleString / toFixed を直呼び", "§08"),
    "R12": ("ui/ の外に生の input / select / textarea", "§07"),
    "R13": ("fmtSigned / fmtPct を通らない符号付き・% の数値", "§08"),
    "R14": ("ページ側で面・未収録・主役数字を独自に作る", "§01 / §05 / §08"),
    "R15": ("--dur-* / DUR を通らない生の時間", "§10"),
    "R16": ("動きのクラスを JS で手で付け外し", "§10"),
    "R17": ("開閉ブロックを手書き(disclosurePane / disclosureCaret を使わず)", "§10"),
    "R18": ("<details> が開閉するのに動きが無い", "§00 / §10"),
    "R19": ("scrollIntoView の behavior: \"smooth\" を直書き", "§10"),
    "R20": ("開閉する面に動きが無い", "§00 / §10"),
}

# §05 の実寸スケール。v4 が使っている実寸で、役割トークン 4 段の外にもある。
# 密度は意識的な選択なので「役割トークンに寄せる」のではなく、この集合に収まるかを見る
FONT_SCALE = {44, 40, 27, 19, 17, 15, 14, 13, 12.5, 12, 11.5, 11, 10.5, 10, 9.5, 9, 8.5, 8}

# R11: 桁区切り・小数桁は format.ts だけが決める(§08「数値の 3 段」)。画面側の直呼びは違反
FORMAT_TS = SRC / "format.ts"
RAW_FORMAT_CALL = re.compile(r"\.(toLocaleString|toFixed)\s*\(")

# R12: 入力は ui/ の 5 形態(StatInput / TextField / Picker / StepSelect / ToggleRow)だけが生の要素を持つ(§07)。
# file / checkbox / radio / range は 5 形態の外(ファイル選択・ON/OFF)なので見ない
UI_DIR = SRC / "ui"
RAW_INPUT = re.compile(r"<(input|select|textarea)\b([^>]*)>", re.S)
INPUT_TYPE = re.compile(r'type\s*=\s*"([^"]*)"')
RAW_INPUT_TYPES = {"text", "number", "search"}

# R13: 符号と % は format.ts が付ける(§08)。手書きの符号(`+${…}` / 三項の "+")と Math.round(x * 100)}% は違反。
# 強化段のラベル(装備 +7 / Lv)は段の名前であって符号ではないので除く
RAW_PCT = re.compile(r"Math\.round\([^\n]*?\*\s*100\)\}%")
RAW_SIGN = re.compile(r"[+−]\$\{([^}]*)\}")
RAW_SIGN_TERNARY = re.compile(r"""\?\s*["']\+["']\s*:\s*["']["']\s*\}\$\{""")
STEP_LABEL = re.compile(r"enhance|level|\blv\b|\+ 1\b|\.join\(")

# R14: 面は app.css の .inset、未収録は .badge.unknown、18px 超の数字は役割トークン(§01 / §05 / §08)。
# ページ側(ui/ と app.css 以外)で塗り直したり作り直したりしない
SURFACE_INSET = re.compile(r"var\(--surface-inset\)")
UNKNOWN_NAMES = {"unknown", "unk", "missing", "unrecorded"}
DASHED_BORDER = re.compile(r"(?:^|[;\s])border(?:-style)?\s*:\s*[^;{}]*\bdashed\b")
MAX_BODY_FONT_PX = 18

# R15: 動きの時間は役割で 9 段(§10「操作の応答」)。出どころは app.css の :root --dur-* と
# ui/motion.svelte.ts の DUR の 2 つだけで、値は一致していること。画面側に 0.18s / 220 を直接書かない。
# 例外は 0(動かさない指定)と、終わりのない回転(infinite)— 後者は「変化を見せる動き」ではない
MOTION_TS = SRC / "ui" / "motion.svelte.ts"
DUR_TOKEN = re.compile(r"--dur-([a-z]+)\s*:\s*([0-9.]+)(m?s)\s*;")
DUR_CONST = re.compile(r"^\s*([a-z]+):\s*(\d+),", re.M)
TIMED_DECL = re.compile(r"\b(transition|animation)(?:-duration)?\s*:\s*([^;{}\n]+)")
ZERO_TIME = {"0", "0s", "0ms", "0.01ms"}  # 0.01ms は reduced-motion の打ち消し
# Svelte の transition / animate に渡す時間。DUR.* を通らない数値リテラルが違反
JS_DURATION = re.compile(r"duration:\s*(\d+)")

# R16: 段階 1 で見つかった不具合の実体(旧 Workspace.svelte)は
#   let movedId = $state<string | null>(null);
#   movedTimer = setTimeout(() => (movedId = null), DUR.badge);   ← タイマーで消す
#   class:badge-in={movedId === s.id}                             ← 動きのクラスを駆動
# 「同じ変数が動きのクラスを駆動しながらタイマーで消されている」= 時間を二重に持っていた。
#
# 禁止パターンを並べる形は試したが、必ず兄弟の形ですり抜けた(三項 `class={cond ? "…" : …}`、
# `$derived` で一段挟んだ変数、名前付きコールバックを切り出した setTimeout、classList の
# 引数をリテラルから条件式にずらす、のすべてで実際にすり抜けを確認)。文字列一致では
# 「時間を二重に持っていない」ことを証明できないので、R16 は禁止ではなく許可を 1 つに決める:
#
#   画面側のコード(ui/motion.svelte.ts 以外)が動きのクラス名を書いてよいのは、
#   素の静的な `class="… swap-in …"` 属性の中だけ。それ以外(class:束縛・class={式}・
#   classList・変数経由 等、経由の形は問わない)に動きのクラス名が現れたら候補に出す。
#
# `apps/desktop/src/pages/chars/sources/EquipmentPane.svelte:632` の
# `class:swap-in={fresh}` は既知の 1 件として残す。`fresh` は呼び出し側でリテラル固定
# (1052 行 false / 1060 行 true)なので実際には無害だが、この規則は静的にしか見ないので
# 危険な形と区別できない——本ツールは冒頭の docstring どおり「候補を出すだけで判定はしない」。
# ここを静的な形に書き換えて候補を消すのは、規則のためにコードの形を変える先例になるので
# しない。R16 が 2 件以上になったら、誰かが新しい動的な形を書いたということ。
#
# コメント(HTML の <!-- --> ・JS/CSS の /* */ と //)と <style> ブロックの中は、要素の
# class を実際には駆動しない(コメントの説明文・CSS セレクタ・keyframes 名としての言及)ので
# スキャン対象から外す。マスクは改行だけ残して空白に置き換えるので、これで得た行番号は
# 元のファイルの行番号と一致する。
#
# 文字列リテラルの中の `/*` `//` をコメント開始と誤認しない: `import.meta.glob("*/*.png")`
# (apps/desktop/src/ui/Icon.svelte)の `"*/*.png"` の中の `/*` を開始と見なすと、非貪欲
# マッチが次に現れる本物の `*/`(JSDoc の終端)まで実行文ごとマスクしてしまい、その区間に
# 動きのクラス名が入っても静かに検出漏れになる。そこで `/*` `//` の直前が行頭・空白・
# `; { } ( ,` のときだけコメント開始と見なす(`"*/*.png"` の `/*` は直前が `*` なので除外)。
#
# この規則が拾えないもの:
#   - コメント・<style> の外側で、動きのクラス名を文字列連結やテンプレートリテラルの
#     部分文字列として組み立てて何かに埋め込むような、きわめて遠回りな書き方
#   - コメント開始の直前判定に無い文字(`>` など)の直後に書かれたコメント
MOTION_TOGGLE_CLASSES = ("badge-in", "open-in", "swap-in", "pane-in", "pop-in", "delta-in", "bump-up", "bump-down")
MOTION_CLASS_NAME = re.compile(r"\b(?:" + "|".join(MOTION_TOGGLE_CLASSES) + r")\b")
# 素の静的な class="..." / class='...' 属性の値だけを許可領域として拾う。
# `class:foo=`(コロン)や `class={...}`(波括弧)はこの正規表現に一致しないので許可されない。
# 許可するのは式(`{...}`)を一切含まない値だけ——`class="row {fresh ? 'swap-in' : ''}"` の
# ようにクォートの中にマスタッシュ式を書く形は、三項回避がクォートの中に移っただけなので
# 許可しない(呼び出し側で "{" の有無を見て弾く)。
STATIC_CLASS_ATTR = re.compile(r"(?<![\w:-])class\s*=\s*(?:\"([^\"]*)\"|'([^']*)')")
# STYLE_BLOCK は下(ソースの切り出し)で定義しているものを使う — 同じ名前で 2 回定義すると
# 後勝ちになり、上を直しても効かない(実際に 2 つあった)
HTML_COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)
_COMMENT_START = r"(?:(?<=[\s;{}(,])|^)"
BLOCK_COMMENT = re.compile(_COMMENT_START + r"/\*.*?\*/", re.DOTALL | re.MULTILINE)
LINE_COMMENT = re.compile(_COMMENT_START + r"//[^\n]*", re.MULTILINE)


def _mask(text: str, pattern: re.Pattern) -> str:
    """pattern に一致した範囲を、改行だけ残して空白に置き換える(行番号を保つ)。"""
    return pattern.sub(lambda m: "".join(c if c == "\n" else " " for c in m.group(0)), text)

# R17: 「開閉するブロック」の中身の面は ui/motion.svelte.ts の disclosurePane がやること
# (段階 2)。動的束縛 `class:open-in={...}` に加えて、`{#if 条件}` でマウント/アンマウント
# する形の直後に静的な `class="... open-in ..."`(`class:` ではない)を置くやり方も同じ
# 手書き(段階 2 のレビューで判明 — 動的束縛だけを見ていると、常に開いている面の入場と
# 区別がつかず、候補一覧のような「押すと {#if} で出し入れする」6 箇所を素通りしていた)。
# 常に開いている面の入場に使う固定の `class="... open-in"`(`{#if}` を伴わないもの)は
# 開閉ではないので、いまも対象外。
#
# キャレットの `class:rot={...}` は検出対象に含めない。回すキャレットは disclosurePane と
# 対のもの以外に、ポップオーバー(Picker / CalcPage の targetOpen・skillOpen。開閉は
# `.pop-in` + 重ねる面が別方式)や `transition:collapse` の折りたたみ(VersusPage。
# `{#if}` の出入りを Svelte transition が動かす、別方式)でも正しく使われている。
# `class:rot=` 単独で拾うと、それらの正しい使い方まで毎回候補に出て R17 が恒久的に
# 0 件にならない(段階 2 で確認 — 対象は disclosurePane と組む class:open-in だけ)。
DISCLOSURE_HANDWRITTEN = re.compile(r"class:open-in\s*=")
# {#if}(コメントを 1 個だけ挟んでもよい)の直後に静的な class="... open-in ..." が来る形
IF_STATIC_OPEN_IN = re.compile(
    r"\{#if\b[^}]*\}(?:\s*<!--.*?-->)?\s*<[a-zA-Z][^>]*\bclass=\"[^\"]*\bopen-in\b[^\"]*\"",
    re.S,
)

# R18: ネイティブ <details> は既定で瞬時に開閉する(§00 04 / §10 型 6)。段階 2.5 で最初に
# 試した .details-anim(grid-template-rows: 0fr⇄1fr で本文を包む)は実機で動かなかった —
# ブラウザが本文を ::details-content という UA 生成ボックスに持たせていて、grid の行サイズの
# transition がその境界を越えられない(実測: 開始サンプルから最終値のまま、途中経過が無い)。
# 効くのは details::details-content 自体の block-size を動かす形だけで、包む要素は要らず
# 全 <details> に一律で効くので、この規則は「app.css のその 1 か所が生きているか」だけを見る。
# クラスの有無で判定すると grid 版のような『付いているのに動かない』実装にも 0 件を出してしまう
# (段階 2.5 やり直しで判明)ため、実際に効く CSS の形(本文の高さと --dur-open)を見る。
# **並び順に依存させない** — `transition: block-size var(--dur-open)` も
# `transition: var(--dur-open) block-size` も CSS として正しいので、
# 「transition の中に高さのプロパティと --dur-open が両方ある」だけを見る。
# 高さは block-size / height のどちらでもよい(横書き固定のこのアプリでは同義)
DETAILS_CONTENT_CLOSED = re.compile(r"details::details-content\s*\{([^}]*)\}", re.S)
DETAILS_CONTENT_OPEN = re.compile(r"details\[open\]::details-content\s*\{([^}]*)\}", re.S)
# transition 宣言の中に、高さのプロパティと --dur-open が(順不同で)両方あること
DETAILS_TRANSITION = re.compile(
    r"transition\s*:[^;{}]*(?:(?:\bblock-size\b|\bheight\b)[^;{}]*var\(--dur-open\)"
    r"|var\(--dur-open\)[^;{}]*(?:\bblock-size\b|\bheight\b))",
    re.S,
)

# R19: `scrollIntoView({ behavior: "smooth" })` は CSS の scroll-behavior と違って
# prefers-reduced-motion を見ない。app.css の `scroll-behavior: auto !important` は
# CSS 発火のスクロール(アンカー移動など)にしか効かず、JS からのこの呼び出しは
# 素通りして滑り続ける(段階 3 で見つかった穴)。ui/motion.svelte.ts の `reveal` が
# 動きを消す設定を見て "auto" に落とすので、直書きの "smooth" は必ずそちらへ寄せる。
#
# R18 の反省(クラス名の有無だけを見て「書いてあれば通す」形にすると、実際には効いて
# いない実装にも 0 件を返した)を踏まえ、ここは**現物のリテラルそのもの**を検出する —
# 「reveal を通したか」という体裁ではなく、"smooth" が生で渡っているかどうかは
# 静的に確定できる事実なので、代理指標を挟まずに直接それを見る。
# ただし CSS 側の `scroll-behavior: smooth` は対象外 — そちらは全称セレクタ `*` に付いた
# `scroll-behavior: auto !important`(prefers-reduced-motion)で実際に無効化されるので、
# JS の behavior: "smooth" とは違って穴になっていない。
#
# 拾えない形が 1 つある: 変数を挟むと(`const b = "smooth"; el.scrollIntoView({ behavior: b })`)
# リテラルが `behavior:` と同じ行に出ないので見逃す。いまそう書いている箇所は無いが、
# 「この規則を通ったから安全」ではなく「生の直書きは必ず止まる」までの規則だと理解すること。
SMOOTH_SCROLL = re.compile(r"""behavior\s*:\s*["']smooth["']""")

# R20: 「押すと出し入れされる面なのに、動きの部品が何も無い」箇所(AvatarPane.svelte の
# アバター強化欄で発覚 — .open-in も use:disclosurePane も無く、R15〜R19 のどれも反応しない
# 穴だった)。R15〜R19 は「動きの書き方が間違っている」を見る規則で、「動きが無い」を見るのは
# <details> 限定の R18 だけ。手書きで {#if} を使う開閉には対応する規則が無かった。
#
# 「すべての {#if} を対象にすると候補が溢れる」問題への線引き:
#   1. 対象は「押して開閉するもの」だけ。{#if} の条件に出てくる識別子が、同じファイルの
#      どこかで `x = !x`(真偽反転)か `x = x === v ? null : v`(選択トグル)の形で
#      自分自身に代入されていること — その変数は「押して開閉する状態」だと機械的に言える。
#      データの有無(`{#if items.length}`)や読み込み中の出し分けはこの形にならないので
#      自然に対象外になる。
#   2. 対象は「面」だけ。{#if} の直後に来る要素が span / small / b / i / em / strong / a /
#      label / code など 1 行の中身(バッジ・小さなラベル)なら対象外とする。中に複数要素を
#      持つブロック(div / section / li / ...)だけを見る。
#   3. 動きの有無は **その {#if} 分岐が直接生む、入れ子の無い兄弟要素の開始タグ**だけを見る。
#      子要素の中まで見ると、面の中の値だけが動く `use:bump` / `use:flash`(例: AvatarPane の
#      `5部位計` 集計スパン)を「面の開閉が動いている」と誤認して見逃す。逆に「面」1 個だけを
#      見ると、`{#if targetOpen}` の中の「押した場所を塞ぐ透明な `<button class="overlay">` +
#      本体の `<div class="pop pop-in">`」のように**兄弟 2 個で 1 組**の形(CalcPage /
#      ThesisCorePane / Picker のポップオーバー)を見逃す(overlay 側だけを見て「無い」と
#      誤判定した)。開閉そのものを動かす部品は必ずどれかの兄弟の開始タグに付くので、
#      分岐内の兄弟をすべて見て、どれか 1 つでも持っていれば通す。
#
# この規則が拾えないもの:
#   - トグル変数への代入がローカル変数を経由する遠回りな形
#     (例: `const next = current === kind ? null : kind; obj.field = next;`)。
#     代入先と比較対象が同じ識別子でないと拾えない
#   - `{#if}` を経由しない出し分け(`{#each}` に渡す配列を $derived で真偽値によって
#     入れ替える等)。AvatarPane.svelte の `showOtherStats` はこの形 —
#     `visibleStats = $derived(showOtherStats ? [...] : [...])` で行を増減させているが
#     `{#if}` ではないので機械的には検出できない。実際には開閉と同じ体験(行が瞬時に
#     増減する)なので目視では要確認だが、この規則の対象ではない
#   - **分岐の中に、無関係な別条件の要素がたまたま動きのクラスを持っていると、本体の面が
#     動いていなくても「兄弟のどれかに動きがある」と誤って通してしまう**(線引き 3 の
#     ホワイトリスト方式そのものの裏返し)。実例: `CalcPage.svelte` の
#     `openMaterial === "ultimate"` 枝は `.ultimate-chips` 自体は動かないが、枝の中の
#     無関係な `{#if ultimateFull}` の `<p class="eq-note dim badge-in">`(別の注記の
#     出し分けにたまたま `badge-in` が付いている)を拾って非検出になった。独立レビューで
#     全数確認した結果、この型の見逃しはこの 1 件だけ(他に非検出だった 12 箇所は
#     いずれも分岐が直接生む面自身に動きが付いている正当な非検出)
#
# 将来の火種として残っているが、いまのコードベースには実害が無いことを確認済みのもの:
#   - トグル変数名は単語境界だけでマッチするので、同名のプロパティアクセス
#     (`result?.combo` の `combo` など)と衝突しうる。いまは衝突する組み合わせが無い
#   - 自己終了しない void 要素(`<input>` をスラッシュ無しで書く等)が来ると
#     top_level_open_tags のタグ深さ計算が崩れうる。いまのテンプレートは
#     void 要素をコンポーネント経由(StatInput 等)でしか使っておらず、生の
#     `<input>` 等を分岐の直下に書いている箇所が無いので実害は出ていない
TOGGLE_ASSIGN = re.compile(r"\b([A-Za-z_$][\w]*)\s*=\s*(?:!\1\b|\1\s*===?\s*[^?;{}]+\?\s*null\s*:)")
# {#if} だけでなく {:else if} も同じ形の分岐なので、同じトグル変数を連鎖させた
# 2 枝目以降(HomePage.svelte の openTile チェーンで実例あり)も見る
IF_COND = re.compile(r"\{(?:#if|:else if)\s+([^}]*)\}")
# 分岐(1 つの枝)の終わりを、{#if} の入れ子を数えて見つける。{:else ...} は
# 深さ 1(自分の枝)で出会ったところが終わり
IF_BLOCK_TOKEN = re.compile(r"\{#if\b|\{/if\}|\{:else\b")
TAG_TOKEN = re.compile(r"<(/?)([a-zA-Z][\w-]*)\b([^>]*)>")
# 1 行の中身(バッジ・小さなラベル)は対象外。中に複数要素を持つブロックだけを見る
INLINE_TAGS = {"span", "small", "b", "i", "em", "strong", "a", "label", "code", "abbr", "time", "sup", "sub", "br", "kbd", "mark"}
# 分岐内の兄弟要素の開始タグのどれかに動きの部品があるか
OPEN_CLOSE_MOTION = re.compile(
    r"\b(?:" + "|".join(MOTION_TOGGLE_CLASSES) + r")\b"
    r"|use:(?:disclosurePane|disclosureCaret|collapse|swap|pulse|flash|bump|delta)\b"
    r"|\btransition:\w+"
    r"|\banimate:\w+"
)
SCRIPT_BLOCK = re.compile(r"<script[^>]*>.*?</script>", re.S)


def branch_end(scan: str, after: int) -> int:
    """`after`(ある `{#if ...}` の直後)から、その枝の終わり({:else...} か対応する
    {/if})の開始位置を返す。入れ子の {#if}/{/if} は深さで数える。
    """
    depth = 1
    for m in IF_BLOCK_TOKEN.finditer(scan, after):
        token = m.group(0)
        if token.startswith("{#if"):
            depth += 1
        elif token == "{/if}":
            depth -= 1
            if depth == 0:
                return m.start()
        elif depth == 1:  # {:else ...} — 自分の枝の終わり
            return m.start()
    return len(scan)


def top_level_open_tags(text: str) -> list[str]:
    """text の中で、要素の入れ子の深さ 0 で開くタグの開始タグ文字列を出現順に返す。
    Svelte のブロック構文({#if} / {#each} / {:else} など)は DOM 上は透過なので
    深さに数えない——タグの入れ子だけを見る。
    """
    stack: list[str] = []
    out: list[str] = []
    for m in TAG_TOKEN.finditer(text):
        closing, tag, attrs = m.group(1), m.group(2), m.group(3)
        if closing:
            if tag in stack:
                while stack and stack[-1] != tag:
                    stack.pop()
                if stack:
                    stack.pop()
            continue
        if not stack:
            out.append(m.group(0))
        if not attrs.rstrip().endswith("/"):
            stack.append(tag)
    return out


def check_open_close_motion(path: Path, text: str, out: list[Finding]) -> None:
    """R20。押して開閉する面({#if} でトグル変数を見ている)の分岐が、動きの部品
    (動きのクラス・use:action・transition:/animate:)を兄弟要素のどれにも持たない箇所。
    """
    if path.suffix != ".svelte":
        return
    # トグル変数の代入は <script> に書かれるので、そこはマスクしない。コメントだけ落とす
    no_comment = _mask(_mask(_mask(text, HTML_COMMENT), BLOCK_COMMENT), LINE_COMMENT)
    toggles = {m.group(1) for m in TOGGLE_ASSIGN.finditer(no_comment)}
    if not toggles:
        return
    toggle_re = re.compile(r"\b(?:" + "|".join(re.escape(t) for t in toggles) + r")\b")

    # {#if} とタグの対応付けはテンプレートだけを見る。<script> / <style> は式の中身が
    # 紛れ込むので落とす(行番号を保つため空白化)
    scan = _mask(no_comment, SCRIPT_BLOCK)
    scan = _mask(scan, STYLE_BLOCK)

    for m in IF_COND.finditer(scan):
        if not toggle_re.search(m.group(1)):
            continue
        end = branch_end(scan, m.end())
        tags = top_level_open_tags(scan[m.end():end])
        tag_names = [TAG_TOKEN.match(t).group(2).lower() for t in tags]
        if not any(name not in INLINE_TAGS for name in tag_names):
            continue  # バッジ・小さなラベルだけの分岐は「面」ではないので対象外
        if any(OPEN_CLOSE_MOTION.search(t) for t in tags):
            continue
        line_no = 1 + scan.count("\n", 0, m.start())
        rep = next(t for t, name in zip(tags, tag_names) if name not in INLINE_TAGS)
        rep_tag = TAG_TOKEN.match(rep).group(2)
        keyword = "{:else if" if scan[m.start():m.start() + 8] == "{:else i" else "{#if"
        out.append(Finding("R20", path, line_no, f"{keyword} {m.group(1).strip()}}} → <{rep_tag}> 他 {len(tags) - 1} 兄弟",
                           "動きの部品(open-in / disclosurePane / transition: など)が無い"))


# R9 の対象。押す・打ち込む部品だけを見る(地や区切りまで見ると候補が溢れる)
CONTROL_SEL = re.compile(r"(?:^|[\s,>])(?:input|button|select|textarea)\b|\.(?:btn|chip|tab|field|check|toggle|max-btn|num-field|pill|badge)(?![\w-])")

HEX = re.compile(r"#[0-9A-Fa-f]{3,8}\b")
# 面としてべた塗りしている背景。グラデーションや影の中の色は「面」ではない
FLAT_BG = re.compile(r"background(?:-color)?\s*:\s*#[0-9A-Fa-f]{3,8}\b", re.I)


def norm_hex(h: str) -> str:
    """#abc → #aabbcc、大文字小文字と #rrggbbff の ff を吸収して比較用に正規化する。"""
    s = h.lower()
    body = s[1:]
    if len(body) in (3, 4):
        body = "".join(c * 2 for c in body)
    if len(body) == 8 and body.endswith("ff"):
        body = body[:6]
    return "#" + body


@dataclass
class Finding:
    rule: str
    path: Path
    line: int
    value: str
    detail: str

    @property
    def where(self) -> str:
        return f"{self.path.relative_to(ROOT).as_posix()}:{self.line}"


# --- ソースの切り出し -------------------------------------------------------


@dataclass
class Chunk:
    """行番号を保ったままの部分文字列。CSS 相当のテキストを表す。"""

    text: str
    start_line: int
    path: Path
    kind: str  # "css" | "inline"

    def line_of(self, offset: int) -> int:
        return self.start_line + self.text.count("\n", 0, offset)


STYLE_BLOCK = re.compile(r"<style[^>]*>(.*?)</style>", re.S)
INLINE_STYLE = re.compile(r'style\s*=\s*"([^"]*)"', re.S)


def css_chunks(path: Path, text: str) -> list[Chunk]:
    """CSS ファイルは全体、Svelte は <style> ブロックと inline style 属性。"""
    if path.suffix == ".css":
        return [Chunk(text, 1, path, "css")]
    out: list[Chunk] = []
    for m in STYLE_BLOCK.finditer(text):
        out.append(Chunk(m.group(1), 1 + text.count("\n", 0, m.start(1)), path, "css"))
    for m in INLINE_STYLE.finditer(text):
        out.append(Chunk(m.group(1), 1 + text.count("\n", 0, m.start(1)), path, "inline"))
    return out


def script_text(path: Path, text: str) -> str:
    """色の直書き検査から <style> を除いた本体を返す(CSS 側は R1 が別に見る)。"""
    if path.suffix == ".ts":
        return text
    return STYLE_BLOCK.sub(lambda m: "\n" * m.group(0).count("\n"), text)


def rule_blocks(chunk: Chunk):
    """CSS の { … } を素朴に対応付けて (セレクタ, 本体, 本体の開始 offset) を返す。"""
    text = chunk.text
    depth = 0
    sel_start = 0
    body_start = 0
    for i, ch in enumerate(text):
        if ch == "{":
            depth += 1
            if depth == 1:
                body_start = i + 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                yield text[sel_start:body_start - 1].strip(), text[body_start:i], body_start
                sel_start = i + 1


DECL = re.compile(r"([-a-zA-Z]+)\s*:\s*([^;{}]+)")


# --- トークン表 -------------------------------------------------------------


def load_tokens() -> dict[str, str]:
    """app.css の :root から「値 → トークン名」を作る。同値が複数なら最初のもの。"""
    text = APP_CSS.read_text(encoding="utf-8")
    root = text.split(":root", 1)[1]
    root = root[root.index("{") + 1 : root.index("\n}")]
    table: dict[str, str] = {}
    for name, value in re.findall(r"(--[-a-z0-9]+)\s*:\s*([^;]+);", root):
        v = value.strip()
        if HEX.fullmatch(v):
            table.setdefault(norm_hex(v), name)
    return table


# --- ルール -----------------------------------------------------------------


def check_colors(path: Path, text: str, tokens: dict[str, str], out: list[Finding]) -> None:
    """R1(トークン同値の直書き)と R8(TS / Svelte への色直書き)。"""
    is_script_file = path.suffix in (".ts", ".svelte")
    css_lines: set[int] = set()
    for chunk in css_chunks(path, text):
        if chunk.kind == "css":
            css_lines.update(range(chunk.start_line, chunk.start_line + chunk.text.count("\n") + 1))

    for i, line in enumerate(text.split("\n"), 1):
        if path == APP_CSS and line.lstrip().startswith("--"):
            continue  # トークン定義そのもの
        for m in HEX.finditer(line):
            value = norm_hex(m.group(0))
            token = tokens.get(value)
            # 白と黒は面のトークン(--bg-field)と同値になるが、対象は面としてべた塗りするときだけ。
            # 文字色の白、グラデーションの明側、box-shadow のハイライトは面ではないので拾わない
            if token and value in ("#ffffff", "#000000") and not FLAT_BG.search(line):
                continue
            if token:
                out.append(Finding("R1", path, i, m.group(0), f"var({token}) と同値"))
            elif is_script_file and i not in css_lines:
                out.append(Finding("R8", path, i, m.group(0), "CSS 変数に寄せられないか"))


def check_raw_format(path: Path, text: str, out: list[Finding]) -> None:
    """R11。TS / Svelte で数値書式を直接呼んでいる箇所。format.ts 自身は書式の実装なので除く。"""
    if path == FORMAT_TS or path.suffix not in (".ts", ".svelte"):
        return
    for i, line in enumerate(text.split("\n"), 1):
        for m in RAW_FORMAT_CALL.finditer(line):
            out.append(Finding("R11", path, i, m.group(1), "format.ts の fmtInt / fmtNum / fmtPct / fmtSigned / fmtRate に寄せる"))


def is_page_side(path: Path) -> bool:
    """ページ側 = ui/ と app.css の外。共有部品と土台は規格の実装そのものなので対象外。"""
    return path != APP_CSS and UI_DIR not in path.parents


def check_raw_inputs(path: Path, text: str, out: list[Finding]) -> None:
    """R12。ui/ の外の .svelte にある生の入力要素。"""
    if path.suffix != ".svelte" or not is_page_side(path):
        return
    for m in RAW_INPUT.finditer(text):
        tag, attrs = m.group(1), m.group(2)
        if tag == "input":
            t = INPUT_TYPE.search(attrs)
            kind = t.group(1) if t else "text"
            if kind not in RAW_INPUT_TYPES:
                continue
            value = f"<input type={kind}>"
        else:
            value = f"<{tag}>"
        out.append(Finding("R12", path, 1 + text.count("\n", 0, m.start()), value,
                           "ui/ の StatInput / TextField / Picker / StepSelect / ToggleRow に寄せる"))


def check_raw_sign(path: Path, text: str, out: list[Finding]) -> None:
    """R13。.svelte で符号や % を手で組み立てている箇所。"""
    if path.suffix != ".svelte":
        return
    for i, line in enumerate(text.split("\n"), 1):
        if RAW_PCT.search(line):
            out.append(Finding("R13", path, i, "Math.round(x * 100)}%", "fmtPct(rate) に寄せる"))
        if RAW_SIGN_TERNARY.search(line):
            out.append(Finding("R13", path, i, '${x >= 0 ? "+" : ""}${…}', "fmtSigned / fmtSignedPct に寄せる"))
        for m in RAW_SIGN.finditer(line):
            if STEP_LABEL.search(m.group(1)):
                continue  # 強化段のラベル
            out.append(Finding("R13", path, i, m.group(0), "fmtSigned / fmtSignedPct に寄せる"))


def check_page_surfaces(chunk: Chunk, out: list[Finding]) -> None:
    """R14。ページ側の CSS で面・未収録・主役数字を独自に作っている箇所。"""
    if not is_page_side(chunk.path):
        return
    for m in SURFACE_INSET.finditer(chunk.text):
        out.append(Finding("R14", chunk.path, chunk.line_of(m.start()), "var(--surface-inset)",
                           "塗り直さず app.css の .inset を付ける"))
    for m in FONT_SIZE.finditer(chunk.text):
        if float(m.group(1)) > MAX_BODY_FONT_PX:
            out.append(Finding("R14", chunk.path, chunk.line_of(m.start(1)), f"{m.group(1)}px",
                               "18px 超は --t-result / --t-result-inline / --t-heading だけ"))
    if chunk.kind != "css":
        return
    for sel, body, offset in rule_blocks(chunk):
        names = sel_names(sel)
        if "badge" in names or not (names & UNKNOWN_NAMES) or not DASHED_BORDER.search(body):
            continue
        out.append(Finding("R14", chunk.path, chunk.line_of(offset), sel,
                           "未収録は .badge.unknown で出す(独自の破線 class を作らない)"))


def check_radius(chunk: Chunk, out: list[Finding]) -> None:
    for m in re.finditer(r"border-radius\s*:\s*([^;{}\n]+)", chunk.text):
        raw = m.group(1).strip()
        if "var(--r-" in raw:
            continue
        line = chunk.line_of(m.start(1))
        for value in raw.split():
            v = value.strip().rstrip("!important").strip()
            if not v or v in RADIUS_FREE or v.startswith("var("):
                continue
            if v in RADIUS_STEPS:
                out.append(Finding("R3", chunk.path, line, raw, f"var({RADIUS_STEPS[v]}) と同値"))
            else:
                out.append(Finding("R2", chunk.path, line, raw, "4 段(12 / 9 / 6 / 999)の外"))
            break


def uses_num_font(decls: str) -> bool:
    """font-family(font 略記)で数値書体を当てているか。--font-num の定義そのものは除く。"""
    for prop, value in DECL.findall(decls):
        if prop in ("font-family", "font") and ("--font-num" in value or "monospace" in value):
            return True
    return False


def check_tabular(chunk: Chunk, out: list[Finding]) -> None:
    if chunk.kind == "inline":
        if uses_num_font(chunk.text) and "tabular-nums" not in chunk.text:
            out.append(Finding("R4", chunk.path, chunk.start_line, "inline style",
                               "数値書体を当てているが tabular-nums が無い"))
        return
    for sel, body, offset in rule_blocks(chunk):
        if not uses_num_font(body):
            continue
        if "font-variant-numeric" in body or "tabular-nums" in body:
            continue
        out.append(Finding("R4", chunk.path, chunk.line_of(offset), sel or "(rule)",
                           "数値書体だが tabular-nums が無い"))


def check_icon_size(chunk: Chunk, out: list[Finding]) -> None:
    if chunk.kind != "css":
        return
    for sel, body, offset in rule_blocks(chunk):
        if "icon" not in sel.lower():
            continue
        for prop, value in DECL.findall(body):
            if prop not in ("width", "height", "min-width", "min-height", "font-size"):
                continue
            for num in re.findall(r"(\d+(?:\.\d+)?)px", value):
                if prop == "font-size":
                    continue
                if float(num) not in ICON_SIZES:
                    out.append(Finding("R5", chunk.path, chunk.line_of(offset), f"{sel} {{ {prop}: {value.strip()} }}",
                                       "4 段(20 / 28 / 40 / 64)の外"))


SEL_NAMES = re.compile(r"[.#]?[A-Za-z][-\w]*")
# 枠も地も無い(リセットや透明化)ルールは角丸の対象にならない
NO_SURFACE = {"0", "none", "transparent", "0px", "inherit", "initial", "unset"}


def sel_names(sel: str) -> set[str]:
    """セレクタに出てくるクラス名・要素名の集合。疑似クラスと結合子は落とす。"""
    return {m.group(0).lstrip(".#") for m in SEL_NAMES.finditer(re.sub(r"::?[a-z-]+(\([^)]*\))?", " ", sel))}


FONT_SIZE = re.compile(r"font-size\s*:\s*([0-9.]+)px")


def check_font_scale(chunk: Chunk, out: list[Finding]) -> None:
    """§05 の実寸スケールの外にある font-size。calc() は §06 のアイコン比率なので見ない。"""
    for m in FONT_SIZE.finditer(chunk.text):
        size = float(m.group(1))
        if size not in FONT_SCALE:
            out.append(Finding("R10", chunk.path, chunk.line_of(m.start(1)), f"{m.group(1)}px",
                               "44 / 40 / 27 / 19 / 17 / 15 / 14 / 13 / 12.5 / 12 / 11.5 / 11 / 10.5 / 10 / 9.5 / 9 / 8.5 / 8 の外"))


def check_control_radius(chunk: Chunk, out: list[Finding]) -> None:
    """押す・打ち込む部品に枠や地があるのに角丸が無い(§04 は 0 を段に数えない)。

    同じ面に対する別ルール(`:hover` や修飾クラス)で角丸が付いていれば違反ではないので、
    ファイル内で角丸を宣言しているセレクタの名前を先に集めて突き合わせる。
    """
    if chunk.kind != "css":
        return
    blocks = list(rule_blocks(chunk))
    rounded: set[str] = set()
    for sel, body, _ in blocks:
        if "border-radius" in body:
            rounded |= sel_names(sel)
    for sel, body, offset in blocks:
        if not CONTROL_SEL.search(sel) or "border-radius" in body:
            continue
        decls = DECL.findall(body)
        surface = [v.strip() for p, v in decls if p in ("border", "background", "background-color")]
        if not surface or all(v.split()[0] in NO_SURFACE for v in surface):
            continue
        if sel_names(sel) & rounded:
            continue
        out.append(Finding("R9", chunk.path, chunk.line_of(offset), sel, "角丸が 4 段のどれでもない(0)"))


DURATION = re.compile(r"(?<![-\w.])(\d*\.?\d+)(ms|s)(?![-\w])")


def check_duration(chunk: Chunk, out: list[Finding]) -> None:
    for m in re.finditer(r"\b(transition|animation)(?:-duration)?\s*:\s*([^;{}\n]+)", chunk.text):
        for d in DURATION.finditer(m.group(2)):
            seconds = float(d.group(1)) / (1000 if d.group(2) == "ms" else 1)
            if seconds > MAX_DURATION_S:
                out.append(Finding("R6", chunk.path, chunk.line_of(m.start(2)), m.group(0).strip(),
                                   f"{seconds}s > {MAX_DURATION_S}s"))


def check_duration_tokens(chunk: Chunk, out: list[Finding]) -> None:
    """R15(CSS 側)。transition / animation の時間が --dur-* を通っていない。"""
    for m in TIMED_DECL.finditer(chunk.text):
        value = m.group(2)
        if "infinite" in value:
            continue
        for d in DURATION.finditer(value):
            raw = d.group(0)
            if raw in ZERO_TIME:
                continue
            out.append(Finding("R15", chunk.path, chunk.line_of(m.start(2)), f"{m.group(1)}: {value.strip()}",
                               f"{raw} の直書き。var(--dur-*) から取る"))
            break


def check_duration_js(path: Path, text: str, out: list[Finding]) -> None:
    """R15(JS 側)。Svelte の transition / animate に数値リテラルを渡している。"""
    if path == MOTION_TS:
        return
    for m in JS_DURATION.finditer(text):
        if m.group(1) == "0":  # 動かさない指定(主要部位など)は段の外
            continue
        out.append(Finding("R15", path, 1 + text.count("\n", 0, m.start()), f"duration: {m.group(1)}",
                           "ui/motion.svelte.ts の DUR.* から取る"))


def check_manual_motion_class(path: Path, text: str, out: list[Finding]) -> None:
    """R16。許可を 1 つに決めるホワイトリスト方式(MOTION_TOGGLE_CLASSES 手前のコメント参照)。
    動きのクラス名の出現位置が、素の静的な class="..." / class='...' 属性の値の中に
    収まっていなければ候補に出す。コメント・<style> ブロックはあらかじめマスクして
    スキャン対象から外す(要素の class を実際には駆動しないため)。

    ui/motion.svelte.ts 自身はこのクラスの実装なので対象外。
    """
    if path.suffix not in (".svelte", ".ts") or path == MOTION_TS:
        return

    scan = text
    if path.suffix == ".svelte":
        scan = _mask(scan, STYLE_BLOCK)
    scan = _mask(scan, HTML_COMMENT)
    scan = _mask(scan, BLOCK_COMMENT)
    scan = _mask(scan, LINE_COMMENT)

    allowed_spans: list[tuple[int, int]] = []
    for m in STATIC_CLASS_ATTR.finditer(scan):
        group = 1 if m.group(1) is not None else 2
        value = m.group(group)
        if "{" in value:
            continue  # マスタッシュ式を含む -> 動的なので許可しない
        allowed_spans.append((m.start(group), m.end(group)))

    for m in MOTION_CLASS_NAME.finditer(scan):
        if any(start <= m.start() < end for start, end in allowed_spans):
            continue
        line_no = 1 + scan.count("\n", 0, m.start())
        out.append(Finding("R16", path, line_no, m.group(0),
                           "ui/motion.svelte.ts の action(bump / flash / swap / pulse / delta)に寄せる"))


def check_manual_disclosure(path: Path, text: str, out: list[Finding]) -> None:
    """R17。開閉ブロックを disclosurePane / disclosureCaret を使わず手書きしている箇所。

    ui/motion.svelte.ts 自身は実装なので対象外。動的束縛(class:open-in=)は行単位、
    「{#if} の直後に静的な class="... open-in"」はテンプレート全体を見る(2 行にまたがる
    ことがあるため)。
    """
    if path.suffix != ".svelte":
        return
    for i, line in enumerate(text.split("\n"), 1):
        m = DISCLOSURE_HANDWRITTEN.search(line)
        if m:
            out.append(Finding("R17", path, i, m.group(0),
                               "ui/motion.svelte.ts の disclosurePane / disclosureCaret を使う"))
    for m in IF_STATIC_OPEN_IN.finditer(text):
        line = 1 + text.count("\n", 0, m.start())
        out.append(Finding("R17", path, line, "{#if} + 静的 open-in",
                           "ui/motion.svelte.ts の disclosurePane を使う"))


def check_smooth_scroll(path: Path, text: str, out: list[Finding]) -> None:
    """R19。scrollIntoView の behavior: "smooth" を直書きしている箇所。

    ui/motion.svelte.ts 自身は reveal の実装なので対象外。
    """
    if path.suffix not in (".svelte", ".ts") or path == MOTION_TS:
        return
    for i, line in enumerate(text.split("\n"), 1):
        if SMOOTH_SCROLL.search(line):
            out.append(Finding("R19", path, i, 'behavior: "smooth"',
                               "ui/motion.svelte.ts の reveal を通す(動きを消す設定で落ちないため)"))


def check_details_motion(out: list[Finding]) -> None:
    """R18(app.css そのもの)。<details> の本文を動かす仕組みが app.css で生きているか。

    全 <details> に一律で効く仕組みなので、ファイルごとではなく app.css を 1 回だけ見る。
    閉じた側の transition に「本文の高さ(block-size / height)」と `--dur-open` が両方あること、
    開いた側が高さを auto に戻していることの 2 つを見る。どちらか欠けると、閉じたまま /
    開いたまま瞬時に切り替わるだけになる。並び順・改行・プロパティ名の選び方には依存させない。
    """
    css = APP_CSS.read_text(encoding="utf-8")
    closed = DETAILS_CONTENT_CLOSED.search(css)
    opened = DETAILS_CONTENT_OPEN.search(css)
    closed_ok = bool(closed and DETAILS_TRANSITION.search(closed.group(1)))
    opened_ok = bool(opened and re.search(r"\b(?:block-size|height)\s*:\s*auto\b", opened.group(1)))
    if not (closed_ok and opened_ok):
        out.append(Finding("R18", APP_CSS, 1, "details::details-content",
                           "block-size を var(--dur-open) で動かす transition が app.css に無い"
                           "(grid-template-rows は ::details-content の境界を越えられず効かない)"))


def check_duration_scale(out: list[Finding]) -> None:
    """R15(段そのもの)。app.css の --dur-* と motion.svelte.ts の DUR が食い違っていないか。"""
    css: dict[str, int] = {}
    for name, num, unit in DUR_TOKEN.findall(APP_CSS.read_text(encoding="utf-8")):
        css[name] = round(float(num) * (1 if unit == "ms" else 1000))
    parts = MOTION_TS.read_text(encoding="utf-8").split("export const DUR = {", 1)
    ts = {k: int(v) for k, v in DUR_CONST.findall(parts[1].split("} as const;", 1)[0])} if len(parts) > 1 else {}
    for name in sorted(set(css) | set(ts)):
        if css.get(name) != ts.get(name):
            out.append(Finding("R15", MOTION_TS, 1, f"--dur-{name}",
                               f"app.css {css.get(name)}ms / DUR {ts.get(name)}ms — 段が 2 つに割れている"))


def check_state_pairs(chunk: Chunk, out: list[Finding]) -> None:
    """背景と枠が別々の系統から来ていないか。"""
    def scan(decls: str, line: int) -> None:
        found: dict[str, tuple[int, str]] = {}
        for prop, value in DECL.findall(decls):
            if prop not in ("background", "background-color", "border", "border-color"):
                continue
            for h in HEX.finditer(value):
                sysinfo = STATE_OF.get(norm_hex(h.group(0)))
                if sysinfo:
                    found["bg" if prop.startswith("background") else "bd"] = sysinfo
        if len(found) == 2 and found["bg"][0] != found["bd"][0]:
            out.append(Finding("R7", chunk.path, line,
                               f'{found["bg"][1]} の地 + {found["bd"][1]} の枠',
                               "6 系統をまたいでいる"))

    if chunk.kind == "inline":
        scan(chunk.text, chunk.start_line)
    else:
        for _sel, body, offset in rule_blocks(chunk):
            scan(body, chunk.line_of(offset))


# --- 実行 -------------------------------------------------------------------


def collect() -> list[Finding]:
    tokens = load_tokens()
    out: list[Finding] = []
    files = sorted(p for p in SRC.rglob("*") if p.suffix in (".css", ".svelte", ".ts"))
    for path in files:
        text = path.read_text(encoding="utf-8")
        check_colors(path, text, tokens, out)
        check_raw_format(path, text, out)
        check_raw_inputs(path, text, out)
        check_raw_sign(path, text, out)
        check_duration_js(path, text, out)
        check_manual_motion_class(path, text, out)
        check_manual_disclosure(path, text, out)
        check_smooth_scroll(path, text, out)
        check_open_close_motion(path, text, out)
        for chunk in css_chunks(path, text):
            check_page_surfaces(chunk, out)
            check_radius(chunk, out)
            check_tabular(chunk, out)
            check_icon_size(chunk, out)
            check_duration(chunk, out)
            check_state_pairs(chunk, out)
            check_control_radius(chunk, out)
            check_font_scale(chunk, out)
            check_duration_tokens(chunk, out)
    check_duration_scale(out)
    check_details_motion(out)
    return out


def main(argv: list[str]) -> int:
    # Windows のコンソール既定(cp932)だと日本語の一部が落ちるので UTF-8 に固定する
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(encoding="utf-8", errors="replace")
    if "--list" in argv:
        for rid, (title, section) in RULES.items():
            print(f"{rid}  {section:10s} {title}")
        return 0
    wanted = {a for a in argv if a in RULES} or set(RULES)

    findings = [f for f in collect() if f.rule in wanted]
    by_rule: dict[str, list[Finding]] = defaultdict(list)
    for f in findings:
        by_rule[f.rule].append(f)

    for rid in RULES:
        if rid not in wanted:
            continue
        title, section = RULES[rid]
        items = by_rule.get(rid, [])
        print(f"\n=== {rid} {title}  ({section})  {len(items)} 件 ===")
        if not items:
            print("  なし")
            continue
        groups: dict[tuple[str, str], list[Finding]] = defaultdict(list)
        for f in items:
            groups[(f.value, f.detail)].append(f)
        for (value, detail), fs in sorted(groups.items(), key=lambda kv: -len(kv[1])):
            print(f"  {value}  — {detail}  ({len(fs)} 箇所)")
            for f in fs:
                print(f"      {f.where}")

    print(f"\n--- 合計 {len(findings)} 件。これは候補であって違反ではない。goal の「② 分類」にかけること ---")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
