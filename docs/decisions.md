# 決定記録(decisions)

形式: **決定** / 理由 / 確認方法。仮決定は `[仮]` を付ける。wiki で裏が取れたら `[仮]` を外し出典を追記する。

## 2026-08-21 最小 E2E(docs/goals/2026-08-21-minimal-e2e.md)

### 環境・ライブラリ

- **Rust stable(1.98, MSVC)+ VS 2022 Build Tools(VCTools)** を開発機に導入 / Tauri の Windows 公式要件が MSVC。WebView2 は OS 同梱(151.x)を確認済み / `cargo run` で Hello World がリンクできること
- **Tauri 2** / CLAUDE.md 指定。CLI は `cargo install tauri-cli` ではなく npm の `@tauri-apps/cli` を使う(ビルド時間短縮、Node 22 が既にある) / `npm run tauri dev` で起動
- **rusqlite(features = ["bundled"])** / CLAUDE.md 指定。bundled で SQLite 同梱、実行環境に依存しない / `cargo test -p storage`
- **serde + serde_json** / Tauri コマンドの入出力と gamedata の型に必須。事実上の標準 / —
- **thiserror** / storage のエラー型。定番で薄い / —
- **Svelte 5 + Vite + TypeScript** / CLAUDE.md 指定。`create-tauri-app` 相当の構成を手で置く / `npm run build`
- テストは Rust 標準 `#[test]` のみ。追加のテストフレームワークは入れない / 現状の要件に十分

### ドメインモデル

- **素ステータスは 7 種(STAB/HACK/INT/DEF/MR/DEX/AGI)を登録する** / goal は「素ステ6種」と書くが wiki(damage-formula.md §1)は 7 種。ゲーム仕様は wiki が正。DEF は与ダメージに寄与しないが、将来の防御側計算・装備条件判定で必要になる / wiki §1 の表
- **能力値計算(§2)は 5 レイヤー(割合増加(素ステ比)/固定値/倍率A/倍率B/最終固定値)を `StatModifiers` として実装し、今回はすべて中立値(= 素ステがそのまま最終能力値)** / 器を先に作っておけばバフ導入時に式を触らなくて済む / 中立値で恒等になること、各レイヤーに値を入れた単体テスト
- **ダメージカテゴリは wiki §4 の全カテゴリ(A〜Y, New1/New2, V1/V2, E1/E2, S・X のサブカテゴリは親のみ)を 1 つの enum `DamageCategory` で定義し、`CategoryKind`(代入/固定値/割合)とキャップを列挙する** / goal の指示。未使用は中立値(割合=1.0、固定=0、代入=明示) / `all()` が全カテゴリを返し、集計結果トレースに全件出ること
- **丸め関数**: `floor_int(f64)->i64`(`[]`)、`trunc2(f64)->f64`(`{}`、小数第3位以下切捨)を `rounding` モジュールに置き、式には裸の `floor` を書かない / wiki §3 / 単体テスト(負数・境界)
- **`MAX(…, K)` は常に適用する(K=0 のときは下限 0)** `[仮]` / wiki は「K が無い場合は負値になり得る」とするが、負値はその後の正の倍率を通っても ≦0 → 対モンスター下限 1 になるため、今回のスコープ(W=0)では結果が同じ。分岐を増やさない / W>0 のケースで挙動差が出るので、W を使うとき再検討
- **ダメージ上限(与ダメージのキャップ)は今回実装しない** `[仮]` / 上限値の出典(エタの意志ページ)が未取込。旧リポ値(300万〜1800万)は裏取り前 / wiki「エタの意志」取込後に `DamageCap` を追加
- **クリティカル時ダメージ = 最大乱数(B=max)で F・G を適用した値** `[仮]` / 「クリティカル時」の表示値として最大値ベースが一般的(旧リポ・Excel も同様) / UI 表示の定義として docs に明記済み

### カテゴリ A(攻撃力)の内訳 `[仮]`

- **ステ由来攻撃力 = 旧リポ `rawStatCoefficients.json` の係数(例 STAB: 1.08×HACK + 2.1×STAB)** `[仮]` / wiki の Skill#formula が未取込。旧リポの Excel v4.00 由来の係数を暫定採用し、gamedata に出典を記録 / 「計算式まとめ#BaseAttackPower」「Skill#formula」を取り込んで置換
- **装備攻撃力 = 0、装備補正強化係数 = 0** `[仮]` / 今回のスコープに装備登録が無い / 装備モデル導入時に `[装備攻撃力/25 × 係数] × 25` 項が効くこと(式自体は実装済み・テスト済み)
- **スキル依存種別は 6 種(STAB / HACK / INT / MR / STAB+HACK / HACK+INT)** / 旧リポのスキル 373 件がこの 6 種で分類されている / wiki Skill#formula で確認

### カテゴリ B(乱数)

- **最大 = `{(ステ由来攻撃力 + DEX×3)/18} + 1`、最小 = 1** / wiki §4 B(旧リポの `INT(...)` ではなく wiki の 2 位切捨+1 を採用) / 単体テスト
- **最小ダメージは B=1、最大ダメージは B=最大値で式を評価する** / B は「攻撃力乱数部分」で A に加算される / —

### カテゴリ I(属性差)

- **`I = 1 + floor((キャラ属性値 − 敵閾値) × 0.625) / 100`、範囲 1.00〜1.50。キャラ属性値は今回 0 固定(→ I = 1.0)** `[仮]` / wiki「属性差1あたり+0.625%、小数点以下切捨、下限+0%、上限+50%」。旧リポの `threshold` をそのまま敵データの閾値として使用。キャラの属性強化は登録項目に無い / 属性システムページ取込後に確認
- 敵の `threshold` の意味(属性差の起点)は旧リポ由来 `[仮]` / 同上

### カテゴリ N(覚醒)

- **覚醒倍率は旧リポ `awakening.json` の表(stage0/1=1.0, 2=1.2, 3=1.4, 4=1.6, 5=エタLv 0→2.00 … 40→2.24 … 80→2.49)を gamedata に転記** `[仮]` / wiki の数値ページ(Quest/覚醒クエスト、エタの意志)未取込 / wiki 取込後に差分検証

### 敵データ `[仮]`

- **敵 3 体(トゥタトゥール / 兄弟の鍛冶場 / オーディン(ランク))の防御力(C)・被害減少(M)・カット率A(V1)・属性閾値を旧リポ `monsters.json` から転記** / 狩り場情報一覧が未取込 / 取込後に検証
- 旧リポ `af63` → M(被害減少・固定値)、`af64` → V1(カット率A・割合) と解釈 / SPEC_CELL_MAP の式上の位置(`firstInt × S54 × af64 − af63`)が wiki 式の `× L × V1 + M` と一致する(符号は M を負値で持つ) / 同上

### スキル・キャラ `[仮]`

- **キャラは「ボリス」1 体、スキルは旧リポ boris.json から 5 件(極・横斬り / 極・縦斬り / 極・アイスブレイク / 極・残影斬 / 極・連)。倍率 D・段数・Cri倍率 F を転記** / 係数が STAB+HACK / HACK / HACK+INT を網羅し、依存種別の分岐を検証できる / wiki のボリススキルページ取込後に検証
- スキル Lv は持たない(倍率はスキル 1 件につき 1 値) / 旧リポのデータ構造と同じ。Lv 別倍率は wiki 取込時に導入 / —

### コンボ H

- **コンボは計算リクエストの入力(コンボ数)とし、3 以上で H = 1.15** / wiki §4 H。UI には「コンボ 3 以上」のトグルを置く(数値入力ではない) / 単体テスト

### 構成・運用

- **gamedata は JSON ではなく Rust のリテラル(`const`/関数)で持つ** / 今回のシードは十数件で、ローダとスキーマ検証を作るより型で持つ方が短い。スクレイパー導入時に JSON + ローダへ移す(その時点で `Source` メタデータの形は据え置き) / —
- **storage は `characters` 1 テーブル**(id, name, game_character_id, 7 ステ, awakening_stage, eta_level, created_at) / CRUD のみのスコープ / `cargo test -p storage`
- **DB ファイルは Tauri の `app_data_dir`/`talesweaver-toolkit.sqlite`** / Tauri 標準の置き場所 / 再起動後の永続化をスクリーンショットで確認
- **CLAUDE.md の `researcher`/`implementer`/`reviewer` は専用エージェント定義が無いため general-purpose エージェントをその役割で使う** / 定義ファイル(.claude/agents)が未整備 / 今後 .claude/agents に定義を置けば差し替え

### 実装時の仮決定(implementer 報告より、司令塔が承認)

- **`DamageInput` は `BaseStats` + `StatModifierSet` を受け取り、能力値計算を domain 内で行う** / トレースに `StatTrace` を含めるため。4 段パイプラインが domain 内で完結する / `trace.stats` が 7 行出ること
- **非クリティカル時は `{F×G}` 全体を 1.0 とする(G クリダメ増加もクリ時のみ)** `[仮]` / G は「クリティカルダメージ増加」であり非クリに乗るのは意味的に不自然 / wiki 計算式まとめ#CriticalChance 取込時に確認
- **カテゴリ集計の内部表現は Σ%(0.15 = +15%)。減算系 Q/S/U/New2 は `factor = 1 − Σ%`、それ以外の割合は `1 + Σ%`** / wiki §3 の種別ルールそのまま / トレースの `value`(生値)と `factor`(式で使う値)を併記
- **N(覚醒)・V1(カット率A)は割合カテゴリとして `rate − 1.0` を加算** / wiki の種別(割合)を保ちつつ、gamedata は乗数で持てる / —
- **C(敵防御力)は固定値種別** / wiki 表記どおり / —
- **`floor_int` / `trunc2` は 1e-9 の許容誤差を足してから floor。負数は負の無限大方向(Excel INT と同じ)** / 浮動小数の 0.9999999 問題の回避 / 境界テスト
- **覚醒 stage は 0..=5、エタ Lv は 0..=80 を storage の validate で拒否** / wiki の範囲 / storage テスト
- **`trace.categories` は最大乱数(B=max)時の集計** / B 以外は min/max で共通。B=1 は `steps_min` の式文字列で確認できる / —
- **`.gitattributes` で `* text=auto eol=lf`** / Windows 開発機で CRLF 警告が出るため。リポジトリは LF で統一 / —
