---
name: talewiki-fetch
description: ゲーム仕様の一次ソース Tale Wiki(talewiki.com、EUC-JP の PukiWiki)からページソースを UTF-8 で取得する。スキル倍率・敵ステータス・バフ数値・キャラ一覧などを wiki から調べる・抽出する・docs や gamedata に反映するときに使う。
---

# Tale Wiki の取得

WebFetch / ブラウザは使わない(ポリシーでブロックされる、または要約で計算式が崩れる)。同梱スクリプトで wiki ソースを取る。

```
python .claude/skills/talewiki-fetch/scripts/fetch_page.py <ページ名> [出力ファイル]
```

- ページ名は表示名そのまま(例: `ステータス`、`Skill/ボリス`、`PET`)。スクリプトが EUC-JP で URL エンコードする
- 出力は PukiWiki 記法のソース(`|a|b|` の表、`**見出し`、`&ref(...)`)。表は `|` 区切りで `re` / `str.split` で読める
- 出力ファイルはスクラッチパッドに置く。リポには整理結果だけを docs/ に書き、**出典ページ名・アンカー・取得日**を明記する
- 大きいページは全文を context に読み込まず、スクリプトの出力をファイルに落として `grep` / Python で必要な節・表だけ抽出する

## 既知の落とし穴

- NEC 拡張文字(丸数字①②、先頭バイト 0xAD)は `iconv -c` や素の `euc_jp` デコードで欠落し、以降が文字化けする。スクリプトはバイト境界を守って 0xAD 行だけ cp932 にマップして復元している(`decode_euc_jp_with_nec`)
- `<pre>` が見つからない場合はページ名の誤り(大文字小文字・全角半角)か、ページが存在しない
- 「ステータス」ページの `#jc16a054`「能力値増加/減少カテゴリー」節にレイヤー別バフ一覧がある(割合増加 / 固定値 / 倍率 A / 倍率 B / 最終固定値)

## 調査結果の置き場所

- 計算式: docs/damage-formula.md
- キャラ・スキル・バフの数値: docs/claude/goals/<goal>.md の「wiki 調査結果」節、または gamedata クレートのリテラル
- 数値の採用判断は docs/claude/decisions.md に記録する
