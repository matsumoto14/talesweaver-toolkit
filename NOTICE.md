# 権利表記と出典

## 非公式ツールです

TW Context は、TalesWeaver のプレイヤーが個人的に作っている**非公式**のツールです。
ゲームの開発元・運営元とは一切関係がなく、公認・提携・後援を受けていません。
"TalesWeaver" をはじめとするゲーム内の名称・用語は、それぞれの権利者の商標または登録商標です。

Copyrights (C) NEXON Corporation and NEXON Co., Ltd. All Rights Reserved.

[テイルズウィーバー公式サイト](https://talesweaver.nexon.co.jp/)

この表記は、ネクソンの
[「ファンサイトで公式サイトの画像などを使用できますか?」](https://support.nexon.co.jp/faq/show/247?category_id=49&site_domain=default)
に従って掲げています。同 FAQ では、ゲームに関連するコンテンツ(スクリーンショット・イラスト・音楽など)を
含むページにこの表記を入れることが求められています。

このツールはゲームクライアントに接続せず、ゲームのファイルも読み書きしません。
プレイヤーが自分で入力したキャラクター情報をもとに計算するだけの、独立したアプリケーションです。

## 同梱しているデータ

### ゲーム内の数値データ

スキル倍率・敵ステータス・装備補正値・コンテンツ入場条件などは、
コミュニティ運営の [Tale Wiki](https://talewiki.com/) を一次ソースとして取り込んでいます。
各データの取得元ページと取得日は、`crates/gamedata/` の各モジュール先頭と
[docs/claude/decisions.md](docs/claude/decisions.md) に記録しています。

一部の数値は wiki に記載がなく、コミュニティの実測値・プレイヤー提供の計測結果に依っています。
該当箇所はアプリ内で `[仮]` と表示しています。

### アイコン画像

`apps/desktop/src/assets/icons/` 以下のスキル・マスタリー・装備・キャラクターのアイコンは、
Tale Wiki の各ページに添付された画像を取り込んだものです。
**これらの画像の著作権はゲームの権利者に帰属します**(Copyrights (C) NEXON Corporation and NEXON Co., Ltd. All Rights Reserved.)。
本ツールはゲームの解説・計算補助という目的の範囲で、識別のために表示しています。

取り込み手順は `tools/gamedata/import_*_icons.py` にあり、
どのページのどの添付から取ったかを再現できる形にしてあります。

### コンテンツ画像

`apps/desktop/src/assets/icons/contents/` のコンテンツ画像は、**ゲーム画面の
スクリーンショットから切り出したもの**です。ゲーム内「Content information →
コンテンツクリア状況」の一覧は 1 行 = 1 コンテンツで、行頭にそのコンテンツの絵が付きます。
Tale Wiki の「ミニゲーム/*」にはコンテンツ単位の絵が無いため、こちらを出典にしています。

スクリーンショットの利用は、上に挙げたネクソンの FAQ が
「ゲームに関連するコンテンツ(スクリーンショット・イラスト・音楽など)を含むページに
`Copyrights (C) NEXON Corporation and NEXON Co., Ltd. All Rights Reserved.` を掲げること」
を条件に認めているものです。本ツールはこの表記を、この文書・アプリ内の情報パネル・
紹介ページに掲げています。**画像の著作権はゲームの権利者に帰属します**。

切り出し手順は `tools/gamedata/import_content_images.py` にあり、元のスクリーンショットも
`tools/gamedata/screenshots/` に同梱して、どの画面のどの行から取ったかを再現できるように
してあります。

## 削除・修正のご要望

権利者の方で、同梱している画像(スクリーンショットから切り出したコンテンツ画像を含みます)・
データの削除や表示方法の修正をご希望の場合は、
[GitHub の Issue](https://github.com/matsumoto14/talesweaver-toolkit/issues) からご連絡ください。
確認のうえ、該当データを配布物から取り除きます。

なお本ツールは、アイコン画像が 1 枚も無い状態でも動作するよう作られています
(未収録のアイコンは破線枠と `?` で表示され、レイアウトは崩れません)。
同梱をやめる判断が必要になった場合でも、機能を落とさずに対応できます。

## 使用しているソフトウェア

| 名前 | ライセンス |
|---|---|
| [Tauri](https://tauri.app/) | MIT / Apache-2.0 |
| [Svelte](https://svelte.dev/) | MIT |
| [rusqlite](https://github.com/rusqlite/rusqlite) / SQLite | MIT / Public Domain |
| [M PLUS Rounded 1c](https://fonts.google.com/specimen/M+PLUS+Rounded+1c) | SIL Open Font License 1.1 |
| [M PLUS 1 Code](https://fonts.google.com/specimen/M+PLUS+1+Code) | SIL Open Font License 1.1 |

Rust クレートと npm パッケージの完全な一覧は `Cargo.lock` / `package-lock.json` にあります。
