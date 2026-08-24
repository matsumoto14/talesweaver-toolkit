# 決定記録(decisions)

形式: **決定** / 理由 / 確認方法。仮決定は `[仮]` を付ける。wiki で裏が取れたら `[仮]` を外し出典を追記する。
後の決定で置き換えられた項目は削除せず、先頭に `[更新済 → 節名 #番号]` を付けて残す(現行の仕様は指し先を見る)。

## 2026-08-21 最小 E2E(docs/claude/goals/2026-08-21-minimal-e2e.md)

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

### カテゴリ A(攻撃力)の内訳

- `[更新済 → 2026-08-22 装備攻撃力 #1]` **ステ由来攻撃力 = 旧リポ `rawStatCoefficients.json` の係数(例 STAB: 1.08×HACK + 2.1×STAB)** `[仮]` / wiki の Skill#formula が未取込。旧リポの Excel v4.00 由来の係数を暫定採用し、gamedata に出典を記録 / 「計算式まとめ#BaseAttackPower」「Skill#formula」を取り込んで置換
- `[更新済 → 2026-08-22 装備攻撃力 #2]` **装備攻撃力 = 0、装備補正強化係数 = 0** `[仮]` / 今回のスコープに装備登録が無い / 装備モデル導入時に `[装備攻撃力/25 × 係数] × 25` 項が効くこと(式自体は実装済み・テスト済み)
- `[更新済 → 2026-08-22 装備攻撃力 #3]` **スキル依存種別は 6 種(STAB / HACK / INT / MR / STAB+HACK / HACK+INT)** / 旧リポのスキル 373 件がこの 6 種で分類されている / wiki Skill#formula で確認

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
- `[更新済 → 2026-08-22 Claude Code エージェント運用の整理]` **CLAUDE.md の `researcher`/`implementer`/`reviewer` は専用エージェント定義が無いため general-purpose エージェントをその役割で使う** / 定義ファイルが未整備 / 同日中に `~/.claude/agents/`(ユーザー単位)へ定義を置き解消

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

### レビュー後の追記

- **カテゴリのキャップは同一カテゴリの Σ に対して適用する(add ごとではない)** `[仮]` / wiki §3「同一カテゴリ内は加算」+ §4 の上限の自然な解釈。add ごとだと順序依存になる / `L +0.5 → −0.1 = 0.40` のテスト
- **覚醒 stage5 のエタ Lv 表は Lv 0〜80 の全 81 点を旧リポから転記** `[仮]` / 5 点間引きはレビューで却下(Lv30 で 6% の過小) / Lv 30→2.19、60→2.38 のテスト
- `[更新済 → 2026-08-21 キャラステータス補正源(素ステ 310 / 最終 2400)]` **能力値の上限 1500/2000・下限 1(§1)は能力値計算には未適用。素ステ登録時の値域 1..=2000 は storage の validate で拒否** `[仮]` / 今回バフが無いので能力値は素ステと同じ / エタの意志で上限 2000 に変わる仕様を取り込むとき再検討
- **G(クリダメ増加)の「小数点以下切り捨て」(§4)は未実装** / G の入力経路(バフ)がまだ無い / バフ導入時にバフごと floor を入れる
- **敵データの単位規約: M(被害減少)は式の符号そのまま(負値)、V1(カット率A)は乗数(1.0 = 減少なし)で持ち集計時に Σ% へ変換** / 旧リポの af63/af64 の形に合わせた / —
- **装備なし(装備攻撃力 0)のため、中〜終盤の敵(兄弟の鍛冶場 防御 7050 等)には A+B−C が負になり与ダメージ下限 1 になる** / 既知の制約。実証スクリーンショットはトゥタトゥール(防御 990)で撮影 / 装備モデル導入後に再確認
- **トレースの式文字列の中間値は小数 4 桁表示** / 生の f64 表示(`32620.344434999995`)は読めない。値そのものは `value` フィールドに保持 / —
- **GUI 検証手順**: `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` で `npm run tauri dev` を起動し、Playwright(`chromium.connectOverCDP("http://127.0.0.1:9222")`)で操作・`page.screenshot` / 専用ドライバを書かずに済む。手順は `gui-smoke` skill に集約 / `tauri dev` の Rust 再ビルドによる自動再起動後も登録キャラが残ることを確認(当時のスクリーンショットは旧 UI のため削除済み。現行は docs/screenshots/20〜33)

## 2026-08-21 キャラステータス補正源(docs/claude/goals/2026-08-21-character-stat-sources.md)

- **素ステ(振り分け分)の上限は 310、最終能力値の上限はエタの意志で 2400** / ユーザー確認済み(2026-08-21)。310 はレベル上限でもある。旧 docs の「2000」は誤りとして修正 / 出典: エタの意志ページ Lv100「最大ステータス開放」2,400、取得 2026-08-21。docs/damage-formula.md §1
- **バフは個別にコードで if 分岐せず、`BuffDefinition`(id/対象/レイヤー/値/排他枠)のデータで解決する。`build_modifiers` は catalog を線形探索するだけの汎用関数** / CLAUDE.md 原則、`crates/domain/src/category.rs` の設計思想を踏襲。新しいバフの追加はカタログにデータを 1 件足すだけで済む / `crates/domain/src/stat_sources.rs` の `build_modifiers` にバフ固有分岐が無いこと(コードレビューで確認)
- **ペット S スキル・ルーンスキル・クラウン・神鳥の聖物は gamedata に「表」として持たず、domain の型(`PetSkillTier`/`RuneLevels`/`Crown`/`SacredRelic`)がそのままステごとの値・上限を表現する** / これらは「ステごとに 1 段階/Lv/数値を選ぶだけ」でカタログ的な複数エントリの一覧ではない(バフのような「同時に複数選べる項目の集合」と構造が違う)。段階→値の変換式自体が唯一のデータなので、型のメソッド(`bonus()`/`value()`)に持たせる方が「カタログを持ってきて検索する」間接化より単純 / `crates/domain/src/stat_sources.rs`
- **常用バフカタログ(16件)は gamedata に置く(`crates/gamedata/src/buffs.rs`)。出典は wiki「ステータス」#jc16a054** / gamedata は静的データ、domain は型のみという既存の分離を踏襲 / 取得日 2026-08-21。`buff_catalog().len() == 16` のテスト
- **storage の `characters` テーブルに `stat_sources TEXT NOT NULL` 列を追加(JSON、serde_json でシリアライズ)。`MIGRATION` 定数自体は `stat_sources` 列を含まない旧スキーマ(v1)の `CREATE TABLE IF NOT EXISTS` のまま据え置き、`CharacterRepository::from_connection` が `PRAGMA table_info(characters)` で `stat_sources` 列の実在を直接確認し、無ければ `ALTER TABLE characters ADD COLUMN stat_sources TEXT NOT NULL DEFAULT '{}'` を実行する。列の有無に関わらず最後に `PRAGMA user_version = 2` を設定する。`'{}'` を `StatSources::default()` として読めるよう、`StatSources` の全フィールドに `#[serde(default)]` を追加した** / 2026-08-22 の PR レビューで、「dev DB を削除すれば再生成される」という運用は開発機以外(将来のユーザー環境)では成立しない指摘を受けた。初版実装は `PRAGMA user_version < 2` だけで「列が無い」と判定していたが、独立レビューでこの前提自体が誤りだと判明した: このブランチ以前の実スキーマ(2026-08-21 キャラステータス補正源セクション時点)は `stat_sources` 列を `ALTER TABLE` ではなく `CREATE TABLE` に直接持っていたため、一度でも起動した DB は「列は既にあるが `user_version` は未設定(0)」という状態になっており、`user_version` だけを見ると `ALTER TABLE` が `duplicate column name` で失敗しリポジトリの初期化自体が壊れる実運用バグがあった。列の実在という一次情報を直接見ることでこの前提エラーを解消した。`PRAGMA user_version` は追加のマイグレーションライブラリを要らない SQLite 組み込みの仕組みとして今後のバージョン管理用に引き続き設定する / `crates/storage/src/character_repository.rs` の「旧スキーマからでも自動マイグレーションしてstat_sourcesが中立値で読める」テスト(列の無い v1 スキーマ)、「列は既にあるがuser_version未設定のdbも開ける」テスト(実際に踏んだ状態を再現。列ありで `user_version=0` の DB を開いても `ALTER TABLE` を試みず `list`/`get`/`create` が成功することを確認)
- **`BaseStats::validate()` を domain に実装し、storage の `validate()` はそれを呼ぶだけにする(310 の値域チェックを domain 側に一本化)。storage 独自の `STAT_RANGE` 手書きチェックは削除** / 値域はドメイン規則(ゲーム仕様)であり、storage 固有の制約ではない。domain 単体でも検証できる方が正しい層に置ける / `crates/domain/src/stats.rs` の `BaseStats::validate()` テスト、storage 側は 310 超過が `InvalidValue` になることのみ確認
- **`DamageInput::new` は `stat_modifiers`/`stat_contributions` を必須引数として受け取り、内部で `StatModifierSet::default()` を決め打ちしない** / 「未実装要素の中立値をコマンドに書かせない」設計(docs/architecture.md)に従い、ステータス補正が実装された今回、コマンド側(`calculate_damage`)が `build_modifiers` で組み立てて渡す形に昇格させた / `apps/desktop/src-tauri/src/commands.rs` の `calculate_damage`、domain のテストは `DamageInput` の構造体リテラルを直接組み立てる形のまま(`stat_modifiers: StatModifierSet::default()` を明示)
- `[更新済 → 2026-08-21 キャラ画面 v2 #10(StatInput に統一、両部品とも削除)]` **`ui/NumberField.svelte` を新設した(既存 `Stepper` は流用しない)** / `Stepper` は自由入力を意図的に排除した部品(コメントに明記)。改・信頼の薬(手入力最大+33)・固定増加系・クラウン(0〜300)・キャラスキル%・調整値のように「wiki から取れない、人や検証条件で変わる値」は自由入力が要件そのものであり、`Stepper` を改造すると既存箇所(素ステ等)の「自由入力を禁止する」設計意図を壊す / `apps/desktop/src/ui/NumberField.svelte`。素ステ・ルーン・聖物は引き続き `Stepper` のまま
- **バフ選択 UI は「1 バフ ID につき 1 `BuffChoice`」に固定する。クラブ効果・固定増加系のような `BuffTarget::UserSelected` を「複数ステへ同時適用する UI」は今回作らない** / domain の `BuffSelection.choices: Vec<BuffChoice>` は同じ `buff_id` を複数件持てる形だが、UI 側でチェックボックス1行=1エントリの単純な状態管理にした方が排他枠判定・トグル処理が素直になる。クラブ効果を複数ステに配りたいケースは実運用頻度が低いと判断(登録は 1 キャラにつき通常 1〜2 ステの強化枠) / 2026-08-21 の独立レビュー指摘の修正で `build_modifiers` が同一 `buff_id` の重複選択を常にエラーにするようになったため、この拡張パス(同一 `buff_id` の行を複数追加できる UI)は塞がった。将来複数ステに配りたい場合は、カタログ側でステごとに別 id を用意する等、別の設計が必要になる
- **バフの手入力値(`BuffValue::UserInput`)は `layer` が `percent_of_base`/`multiplier_b` のときだけ画面表示を%スケールにする(表示値=保存値×100、保存時は/100)。`fixed` 層(改・信頼の薬・固定増加系)は変換しない** / レイヤーの単位がそもそも異なる(`percent_of_base`/`multiplier_b` は比率 0.1=10%、`fixed` は素の加算値)。ユーザー入力は「%」という自然な単位、`fixed` 系は「+100」のような整数がそのまま自然な単位なので、レイヤー種別で分岐する / `apps/desktop/src/format.ts` の `formatLayerValue`、`CharacterForm.svelte` の常用バフ手入力欄
- `[更新済 → 2026-08-21 キャラ画面 v2 #11(CharacterWorkspace が同方式を継承)]` **キャラ編集フォームは `CharacterForm` を毎回作り直す({#key editingCharacter?.id ?? 'new'})方式にし、`$effect` によるフィールド再同期ロジックは書かない** / Svelte 5 では `$state` の初期化子は初回マウント時の値しか捕まえない(`editing` prop が後から変わっても再初期化されない)。`{#key}` でコンポーネントごと作り直せば「編集対象が変わったら全フィールドを editing の内容から組み立て直す」が初期化コード 1 箇所で済み、$effect での差分同期(何をリセットし何を残すかの分岐)が要らない / `apps/desktop/src/pages/character/CharacterPage.svelte`。新規登録に戻ったときも同じ仕組みで空フォームに戻る
- **`BuffValue` に `#[serde(rename_all = "snake_case")]` が漏れていたバグを修正した(フロント実装パスで `{"Fixed": 0.3}` 等の PascalCase が実出力されることが判明。`BuffTarget`/`StatLayer`/`PetSkillTier` は同属性が付いており `"all_stats"` 等の snake_case)。属性を追加して `{"fixed": 0.3}` / `{"choice": [...]}` / `"user_input"` に統一し、`apps/desktop/src/api/types.ts` の `BuffValue` 型・`CharacterForm.svelte` の判定ロジックも snake_case に合わせて修正** / 同じ enum 群の中で 1 つだけ表現が違うのは API の一貫性を損なう事故であり、フロント側で恒久的に吸収する理由が無い(ワークアラウンドを積むより発生源を直す) / `cargo test --workspace`、`npm run build && npx svelte-check`

### レビュー後の追記(2026-08-21、独立レビュー指摘の修正)

- **`StatSources::validate()` を domain に追加し、ルーンスキル(0..=20)/クラウン(0..=300)/神鳥の聖物(0..=40段階)の値域チェックを一本化した(`BaseStats::validate()` と同じ思想の拡張)。ペットは `Option<PetSkillTier>` という enum で構造的に制約済みのため対象外、調整値(`Adjustments`)は「検証・未収録バフ用の自由加算」が要件のため範囲チェックしない** / これまで定数(`RuneLevels::MAX_LEVEL` 等)が定義だけされ、どこからも参照されておらず、Tauri コマンドを直接叩けばクラウン9999のような wiki 仕様外の値がそのまま保存・計算に使われる穴があった / `crates/domain/src/stat_sources.rs` の `StatSources::validate()` テスト(各補正源の境界値・0 は OK、上限+1 は NG)
- **`storage::CharacterRepository::create`/`update`(および内部の `validate`)に `catalog: &domain::BuffCatalog` を追加し、保存時に `domain::stat_sources::build_modifiers` を呼んでバフの排他枠・未知ID・選択肢範囲・手入力欠落を検証するようにした(戻り値は破棄、エラーのみ `StorageError::InvalidValue` に変換)。同時に `StatSources::validate()` も呼ぶ** / 従来は `build_modifiers` が `calculate_damage` 実行時にしか呼ばれず、矛盾したバフ選択(排他枠が重複する2つのバフ)を保存すること自体は成功し、ダメージ計算を実行して初めて気づく状態だった。storage は gamedata を知らない設計を保ったまま(`BuffCatalog` は domain の型)、呼び出し元(`apps/desktop/src-tauri/src/commands.rs`)が `gamedata::buff_catalog()` を渡す形にした / `crates/storage/src/character_repository.rs` の `create`/`update` が範囲超過・排他枠重複を拒否するテスト。フロントから見た `create_character`/`update_character` コマンドの引数は変化しない(catalog はサーバ側で取得するだけ)

### 独立レビュー指摘の修正(2026-08-21、2回目)

- `[更新済 → StatInput に同パターンを継承(キャラ画面 v2 #10・#14)]` **`ui/NumberField.svelte` を「表示用の `text`(ローカル `$state`)」と「コミット済みの `value`(bindable prop)」に分離した。`bind:value` を `<input type="number">` に直結せず、`oninput` で `text` を更新しつつ有限数として解釈できる間だけ `value` を書き換え、`onblur` で最終確定・無効値や空欄は `0` にフォールバックする(`value` に `NaN` が入ることはない)** / 空欄の `<input type="number">` は Svelte 側で `NaN` になり、`JSON.stringify(NaN)` は `null` になる。クラウン(`u32`)・調整値(`i64`)は `Option` でないフィールドなので、空欄のまま保存すると Tauri のデシリアライズが失敗しトーストに出ていた。text/value 分離により「入力途中(空欄・`-`単独・`1.`)ではまだ確定しない」状態を表現でき、負数入力中にカーソルや符号が消える問題も避けられる / 常用バフの `UserInput` 手入力(`BuffChoice.value`、`Option<f64>`)は影響を受けないことを確認済み(`toggleBuff` がチェック時に `0` で初期化するため、フォールバックが発生しても問題ない)
- **上記の外部同期用 `$effect` は当初 `if (Number(text) !== value) text = String(value);` という判定にしていたが、独立レビューでバグが見つかり `lastSyncedValue`(非リアクティブな比較用変数、`value` を書いた側で必ず同時更新する)で `value` の変化だけを見る形に修正した** / `Number("")` は `NaN` ではなく `0` を返すため、`value` が非 0 のときにフィールドを全選択して空にすると、`handleInput` 完了直後に効果が再実行され `0 !== value` と判定して `text` が直前の値へ強制的に巻き戻ってしまう(そのまま入力を続けると数字が末尾に追記される)バグがあった。同様に `value` が非 0 のときの `-` 単独入力でも `Number("-")` が `NaN` になり同じ経路で巻き戻る。`lastSyncedValue` は `handleInput`/`handleBlur` が `value` を書くたびに同時更新するため、`$effect` は「外部から `value` が変わった(編集フォーム初期化・排他枠トグル等)」場合にのみ `text` を上書きし、ユーザーの入力途中の空欄・符号入力を巻き戻さない / クラウン・調整値のように初期値が 0 の項目では旧実装でも症状が出ないため、`cargo test`/`svelte-check` は通っていたが機械的テストでは検出されなかった。手動確認: 非 0 の既存値を全選択して打ち直す操作、先頭に `-` を単独入力する操作
- **`build_modifiers` に同一 `buff_id` の重複選択チェックを追加した(`StatSourceError::DuplicateBuff`)。排他枠の有無に関係なく、同じ `buff_id` が `choices` に2回以上現れたら常にエラーにする** / 排他枠を持たないバフ(`tales_weaver_energy` 等)は、同じ ID を複数回選択しても従来は効果が重複計上されてしまっていた。これにより「バフ選択 UI は1バフID=1BuffChoiceに固定する」という UI 側の制限を domain 側でも強制する形になり、上記の拡張パス(同一 `buff_id` の複数行 UI)は塞がった / `crates/domain/src/stat_sources.rs` の重複選択テスト(排他枠が空の `tales_weaver_energy` を2回選ぶと `DuplicateBuff`)
- **`BuffValue::UserInput` をユニットバリアントから `{ min: f64, max: f64 }` を持つ構造体バリアントに変更し、`build_modifiers` が範囲外の入力値を `StatSourceError::ValueOutOfRange` で拒否するようにした。カタログ値は `trust_potion`(改・信頼の薬)= 0..=33、`fixed_increase`(固定増加系)= 0..=999、`character_skill`(キャラスキル)= -0.30..=1.0** / `trust_potion` の 0..=33 は wiki 出典(最大+33)。`fixed_increase` の上限 999 は wiki に明記の上限が無いため実用上の安全域として暫定採用した値であり、実際の上限が判明したら差し替える。`character_skill` の下限 -0.30 は倍率Bの下限(wiki §2)にそのまま合わせたもの、上限 1.0(画面表示 +100%)はキャラスキルの実効果として妥当な範囲の暫定採用であり、こちらも実際の上限が判明したら差し替える / `crates/domain/src/stat_sources.rs` の範囲外拒否テスト(`trust_potion` に 34.0 で `ValueOutOfRange`、境界値 33.0 は成功)。フロント側は `apps/desktop/src/api/types.ts` の `BuffValue` 型・`CharacterForm.svelte` の `userInputRange` ヘルパーで追従し、`NumberField` に `min`/`max`(%表示レイヤーはスケール後の値)を渡す

## 2026-08-21 キャラ画面のUXガイドライン適用(docs/claude/goals/2026-08-21-ux-guidelines-character-screen.md)

1. **登録フォームは名前+キャラ種のみにし、素ステ全種1・覚醒0/0・stat_sources 中立値はフロント(`CharacterRegisterForm.svelte`)で組み立てて送る。domain/storage の `NewCharacter` 型は変更しない** / docs/ux-guidelines.md 原則3(作成と詳細設定を分離する)。8セクションを一度に登録させる旧 `CharacterForm.svelte` は登録の入口を重くしていた / `CharacterRegisterForm.svelte` から `createCharacter` を呼ぶと、素ステ全種1・覚醒 stage0/eternal_level0・pet_skills 全 null・rune_levels/crown/sacred_relic 全 0・buffs.choices 空・adjustments 全ステ0 の `NewCharacter` が送られること(コードレビュー)。`cargo test --workspace` で `NewCharacter` 側の受け入れが壊れていないことを確認
2. `[更新済 → 2026-08-21 キャラ画面 v2 #11(3 カラムに再構成)]` **キャラ詳細は「基本(素ステ・覚醒)」「恒常補正(ペット/ルーン/クラウン/聖物)」「常用バフ」「調整値」の4グループのアコーディオンにし、要約行を常時表示・1度に1グループ展開** / 原則2(構造化する)。全セクションを同じ重みで縦に並べると認知負荷が高い。要約行は「未設定(中立値で計算)」を明示し、原則3の「登録直後でも計算が動く」ことが編集画面からも分かるようにする / `CharacterDetail.svelte` の `basicSummary`/`permanentSummary`/`buffsSummary`/`adjustmentsSummary`(`$derived.by`)。`openGroup` の切替で他グループが自動的に閉じることを目視確認
3. **保存方式は自動保持+一括保存**: グループ切替(アコーディオンの開閉)では入力を破棄しない(全フィールドがコンポーネントの `$state` として保持され続けるため)。どのグループの「保存」ボタンを押しても `update_character` を1回呼び、フォーム全体を送る。理由: グループ単位の部分保存(PATCH的な差分更新)は `update_character` が全フィールド必須のRust側APIと噛み合わず複雑化する。全フィールドを常に保持する設計なら「切替時に破棄しない」は追加のリセット処理を書かないだけで自然に満たせる / `CharacterDetail.svelte` の4箇所の「保存」ボタンがすべて同じ `save()` を呼ぶこと(コードレビュー)。`cargo test --workspace`
4. **一時調整の経路**: `calculate_damage` コマンドに `temporary_adjustments: Option<Adjustments>` を追加。domain 側は `stat_sources::apply_temporary_adjustments`(内部で `build_modifiers` の調整値ループと共通の `apply_adjustments` ヘルパーを再利用、source名だけ「一時調整」)。`StatSources`/`Adjustments` 型自体は変更しない / 原則4(通常操作は選択、シミュレーション用途の一時調整は自由入力を許可する例外)。キャラデータを汚さずに「もしステが+50なら」を試せるようにする / `crates/domain/src/stat_sources.rs` の `apply_temporary_adjustments` テスト(source が「一時調整」になること・中立値では何も積まないこと)。`cargo test -p domain`
5. `[更新済 → 2026-08-21 キャラ画面 v2 #10]` **素ステ入力は `NumberField`(1–310)に統一(旧 `Stepper` のスライダーをやめる)。ルーン・聖物は引き続き `Stepper`(理由は既存の decisions.md の記載を踏襲)** / 原則4。素ステは範囲が広く(1–310)、Stepper のスライダー+1刻みボタンでは目的の値に合わせにくい。ルーン(0–20)・聖物(0–40)は範囲が狭くスライダーで十分 / `CharacterDetail.svelte` の基本グループが `NumberField` を使うこと(コードレビュー)。`npm run build && npx svelte-check`

出典・確認方法(共通): `cargo test --workspace`、`cd apps/desktop && npm run build && npx svelte-check` をすべて実行し通過を確認した。

## 2026-08-21 キャラ画面 v2(docs/claude/goals/2026-08-21-character-screen-v2.md)

ユーザーの実使用フィードバック(入力方式が不統一、バフの効果がその場で見えない、調整値の意味が伝わらない)を受けた再設計。「入力方式は 1 種類・設定を触ると即時に最終能力値が変わる・初期値は実用値・設定列に専門用語を出さない」を判断基準にした。

1. **`Adjustments` を「加算 `add: i64`(固定値層への加算)」と「固定 `pin: Option<i64>`(最終能力値をこの値に置換)」に変更し、`final_fixed` 調整は廃止した。`pin` は `StatModifierSet` の層には乗らず、`effective_stats` の計算結果を事後に上書きする(`stat_sources::apply_pins`)。`StatTrace::pinned_from: Option<i64>` に上書き前の値を残す** / 旧「固定値/最終固定値」という層の名前がユーザーに意味が伝わらないという指摘(CLAUDE.md 最重要要望)。「このステに+Nする」「最終能力値をNに固定する」という 2 つの操作は意味的に別物(前者は補正の一種、後者は補正計算を無視した上書き)であり、同じ `StatModifiers` の層として扱うより「計算後に上書きする」方が実装も意味も単純 / `crates/domain/src/stat_sources.rs` の `apply_pins`/`merge_pins`/`preview_effective_stats` のテスト、`crates/domain/src/damage.rs` の pin 適用テスト
2. `[更新済 → 2026-08-22 PR レビュー指摘の修正 #3(game_character_id は必須引数に)]` **`preview_effective_stats(base_stats, stat_sources) -> StatPreview` コマンドを新設した。`game_character_id` と `awakening` は引数に含めない** / goal 案では `game_character_id`/`awakening` も引数候補だったが、両方とも能力値計算(`effective_stats`)に影響しない(`awakening_rate` はダメージ計算のカテゴリNにのみ効く。`game_character_id` はキャラスキルバフのカタログ絞り込みに使うだけで、絞り込みはフロント側が `BuffDefinition.group` を見て行う)。使わない引数を足すと「現在の要件を満たす最もシンプルな実装」(CLAUDE.md)に反するため削った / `crates/domain/src/stat_sources.rs::preview_effective_stats`、`apps/desktop/src-tauri/src/commands.rs::preview_effective_stats`
3. `[更新済 → 2026-08-22 PR レビュー指摘の修正 #7(merge_pins 削除、apply_pins に統合)]` **一時調整(計算画面)の pin は `merge_pins(character.stat_sources.adjustments, temporary)` でステごとに合成し、一時調整側の pin があればそちらを優先する(`temporary.get(kind).pin.or(base.get(kind).pin)`)。`add` は従来通り両方が加算で積み上がる** / pin は「上書き」なので両方を同時に適用する意味が無く、一時調整は「一時的に試したい値」であるべきなので優先させた / `crates/domain/src/stat_sources.rs::merge_pins` のテスト(temporary 優先・base フォールバック)
4. **`BuffDefinition` に `default_value: Option<f64>`(`UserInput` の初期値)と `group: BuffGroup { Consumable, CharacterSkill { game_character_id }, AllySkill }` を追加した。`BuffTarget` に `Stats(&'static [StatKind])`(複数の特定ステに同じ値を適用)を追加した** / 「バフの初期値を必ず入れる」「キャラスキルをキャラ別に切り替える」という goal の要件を、個別バフのコードで分岐せずデータで表現するため(CLAUDE.md「バフは個別にコードで分岐しない」原則)。`Stats` はロアミニ・ボリス・ジョシュアのキャラスキルが DEF/MR や STAB/DEF など 2 ステに同じ値で効くケースに必要だった(`Stat(単一)`/`UserSelected(ユーザー選択)`のどちらにも当てはまらない) / `crates/gamedata/src/buffs.rs` のキャラスキルテスト
5. **`BuffTarget`/`BuffDefinition` から `Deserialize` を外した(`Serialize` のみ)** `[仮]` / `Stats(&'static [StatKind])` は serde の借用スライスデシリアライズに対応しない型で、`Deserialize` を導出すると型検査が通らない。コードベース内でこれらの型が実際にデシリアライズされる箇所(JSON 文字列からの復元)は無く、Tauri コマンドの戻り値として一方向にシリアライズされるだけなので実害は無いと判断した / 将来これらの型を JSON からデシリアライズする必要が生じたら(手動 `Deserialize` 実装、または `Stats` を `Vec<StatKind>` に変える等の)再設計が必要
6. **イザベルを 4 行(秘法(比率)/秘法(固定)/特選秘薬(割合)/特選秘薬(固定))に分割した。`isabelle_fixed`(秘法固定)と `isabelle_rare_fixed`(特選秘薬固定)は排他枠を分けた(`blessing_potion_a`/`blessing_potion_b`)** / 初版実装では goal 本文の実装指示の字面(「`blessing_potion`(祝福のポーション系: イザベル秘法(固定)・特選秘薬(固定))」)を優先し両者を同一排他枠にしていたが、goal の wiki 調査結果表は「特選秘薬(固定)…秘法(固定)とは併用可」と明記しており、同一排他枠では併用できず矛盾していた。実機スモークテスト・独立レビューで指摘を受け、wiki の記載(併用可)を優先して排他枠を分離した。どちらも将来追加しうる「祝福のポーション」自体とは排他にしたいため、2つの枠名にして両方を1アイテムに持たせられる形にした / `crates/gamedata/src/buffs.rs` の「イザベル4行の値_層_排他枠」テスト(2026-08-21 追加、`exclusive_slots` が異なることを明示的に確認)
7. `[更新済 → 2026-08-22 PR レビュー指摘の修正 #5(1..=2400)]` **調整の「固定(pin)」入力欄の範囲を `0..=99999` にした** `[仮]` / wiki に明記の上限が無く、`fixed_increase`(固定増加系)の暫定上限 999 と同様に実用上の安全域として採用した。実測値記録用途(pin の主目的)なら十分な範囲という判断 / 実際の最終能力値の上限(エタの意志で 2400 まで、将来変わりうる)が判明したら差し替える
8. **キャラ種(`game_character_id`)を切り替えたとき、旧キャラ専用のキャラスキルバフ(`BuffGroup::CharacterSkill`)の選択を `draft.statSources.buffs.choices` から自動的に落とす(`CharacterWorkspace.svelte`)** / UI(`CharacterSettings.svelte`)は選択中キャラのスキルだけを表示するが、選択自体を消さないと非表示のまま計算に効き続ける「幽霊バフ」になる(独立レビューで指摘)。`AllySkill` は誰のキャラでも有効なので対象外 / `CharacterWorkspace.svelte` の `gameCharacterId` 変更監視ロジック(コードレビュー、自動テストは未整備)
9. **プレイアブルキャラを 19 名(gamedata)に拡張し、スキル(ダメージ計算用)はボリス以外空のままにした** / goal のスコープ。ダメージ計算用スキルデータは別途 wiki 取込みが必要なため、今回はキャラスキルバフ(ステ補正)のみ対応した / `crates/gamedata/src/characters.rs` のテスト(19名・ID重複無し)
10. **数値入力を `ui/StatInput.svelte`(ラベル|数値欄|range スライダー|MAX ボタン)の1部品に統一し、`ui/Stepper.svelte`/`ui/NumberField.svelte` を削除した** / CLAUDE.md 最重要要望「入力方式は1種類」。範囲がある値(素ステ・ルーン・クラウン・聖物・バフ手入力・調整の加算/固定)はすべてこの部品を使う。ペット S スキルのような離散的な段階選択は `Select` のまま(数値範囲ではないため対象外) / `apps/desktop/src/ui/StatInput.svelte`、grep で `Stepper`/`NumberField` への参照が無いことを確認済み
11. **画面を「一覧|キャラデータ|設定」の3カラムに再構成し、`CharacterDetail.svelte` を `CharacterWorkspace.svelte`(draft管理・即時プレビュー)+ `CharacterData.svelte`(中央、素ステ・能力値表)+ `CharacterSettings.svelte`(右、恒常補正/常用バフ/キャラスキル/調整のアコーディオン)に分割した。draft は `CharacterWorkspace` が1つの `$state` オブジェクトとして持ち、子コンポーネントへは通常の prop で渡す(ネストしたプロパティの変更は `$state` プロキシ経由で自動的に親へ反映されるため `bind:` は不要)** / goal の画面構成そのもの。draft を1箇所に集約することで「設定列を触るたびにキャラデータ列が即時更新される」が自然に実現できる(`CharacterWorkspace` の `$effect` が draft の変更を検知し 100ms debounce で `preview_effective_stats` を呼ぶ) / `apps/desktop/src/pages/character/CharacterWorkspace.svelte`/`CharacterData.svelte`/`CharacterSettings.svelte`/`draft.ts`。`npm run build && npx svelte-check`
12. **実機スモークテスト・スクリーンショット更新(受け入れ条件6)は本 goal の完了条件から明示的に除外し、別途行う** / ユーザー指示(タスク依頼時に明記) / 2026-08-22 に実施済み(docs/screenshots/20〜33、旧 UI の画像は削除)

出典・確認方法(共通): `cargo test --workspace`(90件)、`cd apps/desktop && npm run build && npx svelte-check`(0 errors）をすべて実行し通過を確認した。独立レビュー(reviewer エージェント)を実施し、指摘のうち #8 は本節の決定として反映済み。#12(スクリーンショット未更新)は上記の通りスコープ外。

### 独立レビュー・実機スモークテスト指摘の修正(2026-08-21)

13. **`CharacterWorkspace.svelte` の「未保存」基準スナップショットを `$state` にし、保存成功のたびに現在の draft で更新するようにした。保存ボタンの活性条件にも `dirty` を追加した** / 旧実装は基準が `const` で固定されており、保存成功後も「未保存」表示・保存ボタンが消えなかった(実機確認で発覚)。「未保存変更があるときだけ有効」という元々の仕様(本節#2)を満たせていなかった / `apps/desktop/src/pages/character/CharacterWorkspace.svelte` の `initialSnapshot`/`canSubmit`
14. **`ui/StatInput.svelte` の空欄・無効値の blur フォールバック先を `min` から「直前の確定値(= 現在の `value`)」に変更した。スライダーは `text`(表示用文字列)ではなく `value`(確定済み数値)に束縛するようにした** / `min` へのフォールバックは、`min` が負の項目(調整「加算」= -999)で、単に全選択して打ち直そうとしただけの操作が `-999` に化ける実害があった。スライダーを `text` に束縛すると、数値欄を空欄にした瞬間に `<input type="range" value="">` 扱いになりつまみが不自然な位置へ飛ぶ不具合があった / `apps/desktop/src/ui/StatInput.svelte`
15. **設定列(`CharacterSettings.svelte`「調整」)とダメージ計算画面の一時調整(`DamagePage.svelte`)で、説明文+`StatInput` を横並びから縦積みに変更した。`StatInput` 自体にも `min-width: 0`・スライダーの `min-width` 縮小を入れた** / 340px/296px という狭い列幅に「加算 — このステに+Nする(検証・仮定用)」のような長い説明文と `StatInput` を横並びで置くと、内容がはみ出し横スクロールが発生していた(実機確認で発覚)。中央の能力値表(`CharacterData.svelte`)は元から `.tbl { overflow-x: auto }` の専用スクロールコンテナ内にあり(`TracePanel` の各テーブルと同じ確立済みパターン)、ページ全体の横スクロールにはならないことを確認し、そちらは変更していない / 目視確認(実機スモークテスト時に再確認予定)
16. **`crates/gamedata/src/buffs.rs` の `note` フィールドから開発メモ(「旧カタログの値は誤りだったため修正」「goal の実装指示に従い…」「URL の percent-encode 未検証」)を削除し、該当箇所の Rust コメントへ移した。`note` はユーザー向けの短い注記のみにする** / `note` は Tauri コマンド経由でそのまま画面(常用バフ・キャラスキルの行)に表示される値であり、実装の経緯を説明する文章はユーザーに見せるべき情報ではない / `apps/desktop/src/pages/character/CharacterSettings.svelte` での表示を目視確認(注記が短くなったことを確認)
17. **`CharacterSettings.svelte` のキャラスキルグループを「このキャラのスキル」(`BuffGroup::CharacterSkill`、選択中キャラのみ表示)と「味方から受けるスキル」(`BuffGroup::AllySkill`、常時表示)の2小見出しに分けた** / 実機確認で「エンカレッジや魅力発散のような味方スキルと、キャラ本人のスキルが同じリストに混在していて分かりにくい」との指摘。domain 側の `BuffGroup` の2種をそのまま画面の見出しに対応させるだけで済み、新しい概念を増やさずに解決できる / `apps/desktop/src/pages/character/CharacterSettings.svelte`
18. **`BuffValue::Fixed` の値を常用バフ・キャラスキルの行に「値: +7」のように表示するようにした(`isFixedValue` ヘルパー、対象は個別バフを特別扱いせず `Fixed` 値を持つ全バフに一律適用)** / クラブ効果(+7固定)を選択しても効果量がどこにも表示されない指摘への対応。クラブ効果だけを特別扱いすると CLAUDE.md「バフは個別にコードで分岐しない」原則に反するため、`BuffValue::Fixed` を持つバフ全般に一律で値表示を出す形にした(結果としてテイルズウィーバーのエネルギー等にも「値: ×1.10」等が出るようになった) / `apps/desktop/src/pages/character/CharacterSettings.svelte`
19. **イザベルの秘法(固定)・特選秘薬(固定)の排他枠を分離した(決定 #6 を参照・更新済み)。テスト用カタログ(`crates/domain/src/stat_sources.rs`)の `club_effect` を実データ(`BuffValue::Fixed(7.0)`)に合わせた。`crates/gamedata/src/buffs.rs` にイザベル4行の値・層・排他枠を固定するテストを追加した** / 独立レビュー指摘(#8・#9 に相当)。テスト用カタログが実データと乖離するとテストの意味が薄れるため揃えた / `crates/domain/src/stat_sources.rs`・`crates/gamedata/src/buffs.rs` のテスト(`cargo test --workspace` で確認)
20. `[更新済(後半のみ) → 2026-08-22 PR レビュー指摘の修正 #7(出所はサーバの pin_source を使う)]` **ダメージ計算画面の一時「固定」トグルの初期値を、直近の計算結果(`result.trace.stats[k].effective`)に変更した(無ければ素ステにフォールバック)。`TracePanel` の「固定」バッジの title(「固定前: X」)は、キャラに保存済みの固定があり、かつ今回その値とは異なる値が最終的に適用されているときは保存済みの固定値を、それ以外は素の計算値(`pinned_from`)を表示するようにした** / 前者は「初期値は実用値」の原則(0 埋めや素ステへ戻すと、既に計算済みの実用的な数字から乖離する)。後者は、キャラに保存済みの固定がある状態で一時調整がそれを上書きしているとき、「固定前」に見せるべきは「自分が普段固定している値」であり、深い生の計算値(ユーザーが普段目にしない数字)ではないという判断 / `apps/desktop/src/pages/damage/DamagePage.svelte` の `toggleTemporaryPin`、`apps/desktop/src/pages/damage/TracePanel.svelte` の `pinnedBeforeLabel`

出典・確認方法: `cargo test --workspace`(91件)、`cd apps/desktop && npm run build && npx svelte-check`(0 errors）をすべて実行し通過を確認した。

## 2026-08-21 画面レイアウトの可変化

サイドバー折りたたみと列幅リサイズを導入した。

1. **リサイズ可能ペインライブラリ(`paneforge` 等)は採用せず、自作の `ui/Splitter.svelte` にした** / `paneforge` の `Pane` は `minSize`/`maxSize`/`defaultSize` がグループ幅に対するパーセント指定のみで、要件(px 単位の最小幅)を満たせない。ダブルクリックでの既定幅リセットも組み込みでない。キャラ管理画面は `CharacterPage`(一覧|detail)の中に `CharacterWorkspace`(データ|設定)がネストする構造で、ライブラリを跨いだ状態受け渡しがかえって複雑になる。ドラッグ処理自体(pointerdown/move/up + clamp + localStorage)は薄いので自作した / `apps/desktop/src/ui/Splitter.svelte`
2. **列幅は `ui/persistedState.svelte.ts` の `persisted(key, initial)` で localStorage に永続化した。呼び出しは各画面コンポーネントの `<script>` 初期化中に限定する(モジュールトップレベルからは `$effect` が使えず `effect_orphan` になるため)** / 既存の状態共有パターン(`toast.svelte.ts` のモジュールスコープ `$state` export)を踏襲しつつ、画面ごとに異なる永続キーを持たせたいので生成関数の形にした / `apps/desktop/src/ui/persistedState.svelte.ts`
3. **各画面のグリッドを、区切り線トラック(6px)を明示的な grid カラムとして持つ形に変更し、`gap: 1px; background: var(--border)` によるトラックレス区切りを廃止した** / `Splitter` 自身が境界線(中央 1px の `var(--border)`)とドラッグ領域を兼ねるため、`gap` による暗黙の区切りと共存させると二重に隙間ができる。`grid-template-columns` を `$derived` で動的に組み立て、幅を持つ列は px、可変列は `minmax(px, 1fr)` にした / `DamagePage.svelte`・`CharacterPage.svelte`・`CharacterWorkspace.svelte` の `.layout`/`.workspace`
4. **列の最小幅・既定幅**: ダメージ計算画面 INPUT(min 240 / 既定 336px)・TARGET(min 220 / 既定 296px)・RESULT(`minmax(320px, 1fr)`)。キャラ管理画面 一覧(min 200 / 既定 280px)・detail(`minmax(0, 1fr)`)。キャラワークスペース データ(`minmax(320px, 1fr)`)・設定(min 280 / 既定 340px)。既存の初期レイアウト(旧 `grid-template-columns` の固定値)をそのまま既定値として引き継いだ / 各画面のソース
5. **localStorage キー**: `tw-sidebar-collapsed`(`boolean`)、`tw-layout-damage`(`{ input, target }`)、`tw-layout-character-list`(`{ list }`)、`tw-layout-character-workspace`(`{ settings }`)** / 画面ごとに独立させ、他画面の値と衝突しないようにした
6. **サイドバー(`App.svelte`)は折りたたみ時に幅を 208px → 56px にし、ナビ項目のラベル・未実装バッジ・ブランド文字列を非表示にして `title` 属性でラベルを補う形にした** / アイコンのみでも操作を維持しつつ、狭い画面でメイン領域を確保できるようにするため。既存の `disabled`/`.soon` ロジックは変更していない / `apps/desktop/src/App.svelte`
7. **`.panel-head .title`(`app.css`)・`CharacterSettings.svelte` の `.group-summary` の省略記号(ellipsis)を廃止し、折り返し(`min-width: 0; overflow-wrap: break-word`、必要に応じ `white-space: normal`)に変更した** / 列を狭くリサイズしたときに情報が省略されて消えるより、折り返して全文を読めるほうを優先する方針(ユーザー指示)。`.panel-head` には `flex-wrap: wrap` も追加した
8. **各画面の grid 直下の `section` に `min-width: 0` を明示し、`.layout`/`.workspace` に `overflow-x: auto` を追加した** / grid アイテム・flex アイテムは既定で `min-width: auto`(コンテンツ幅未満に縮まない)ため、これが無いと列を最小幅近くまで縮めたときにグリッド全体がはみ出す。`overflow-x: auto` は、それでも収まらない場合(極端に狭いウィンドウ等)の安全弁として、グリッド内だけでスクロールさせ画面全体の破綻を防ぐ

出典・確認方法: `cargo test --workspace`(91件、既存差分含め全通過)、`cd apps/desktop && npm run build && npx svelte-check`(0 errors / 0 warnings)をすべて実行し通過を確認した。

- **列幅は `minmax(最小幅, 保存幅)` で組み立て、コンテナが狭いときは保存幅を保ったまま縮む(横スクロールを出さない)。最小幅: ダメージ INPUT 200 / TARGET 180 / RESULT 240、キャラ 一覧 160 / detail 466(= データ 240 + 6 + 設定 220)** / 当初は保存幅を px 固定にしていたため、900×700(サイドバー展開でコンテンツ幅 692px)で 3 列目が画面外に出て横スクロールになった。ネストしたグリッド(一覧 | detail(データ | 設定))では外側の detail 列に内側の最小合計を下限として与えないと、一覧列が先に保存幅を取って内側が潰れる / 900×700・1100×700・1280×840 で `.layout`/`.workspace` の scrollWidth == clientWidth、1280 で既定幅 336/296 に戻ることを実機確認(docs/screenshots/32, 33)

## 2026-08-22 PR レビュー指摘の修正

feature/character-stat-sources-ui の PR レビューで挙がった 10 件の指摘を修正した。

1. **storage のマイグレーション方針を「dev DB を削除すれば再生成される」運用から `PRAGMA user_version` による自動マイグレーション(v1→v2)に変更した** / 上記(2026-08-21 キャラステータス補正源セクション、`characters` テーブルの記述)を参照・上書き。詳細・確認方法は同セクションの更新箇所を参照
2. **`ui/StatInput.svelte` の `handleInput` で、確定する `value` を整数ステップ(`Number.isInteger(step)`、既定 `step=1`)のときは `Math.round` してから `clamp` するようにした(`text` 自体は生の入力文字列のまま保持)** / 独立レビューで「数値欄に `12.5` のような小数を直接入力すると素ステ等の整数項目に小数が入る」指摘。`text`/`value` 分離パターン(decisions.md 既存記載)を維持したまま、確定値だけ丸める / `apps/desktop/src/ui/StatInput.svelte`
3. **`stat_sources::build_modifiers`/`preview_effective_stats` に `game_character_id: &str` を追加し、`BuffGroup::CharacterSkill { game_character_id }` のバフを選択した際、選択中キャラと所有者が一致しない場合は `StatSourceError::ForeignCharacterSkill` で拒否するようにした。`storage::CharacterRepository` の保存時検証・Tauri コマンド(`preview_effective_stats`/`calculate_damage`)もこの引数を渡すよう追従した** / 上記「2026-08-21 キャラ画面 v2」セクション #2 の決定(「`preview_effective_stats` は `game_character_id`/`awakening` を引数に含めない」)を、この点についてのみ明示的に上書きする。サーバ側でキャラスキルの所有者検証をするようになったため、能力値計算そのものには影響しなくても `game_character_id` が必須引数になった(`awakening` は今回も引数に加えていない、能力値計算に無関係なのは変わらないため)。従来はフロント側(`CharacterSettings.svelte` の `isCharacterSkillFor`)でのみ選択肢を絞り込んでおり、Tauri コマンドを直接叩けば他キャラのスキルバフを選択・保存・計算できる穴があった / `crates/domain/src/stat_sources.rs` の「キャラスキルは一致するキャラなら成功する」「キャラスキルは異なるキャラだとエラーになる」テスト
4. **`preview_effective_stats` の冒頭で `base.validate()?`・`sources.validate()?` を呼び、`BaseStats`/`StatSources` の値域検証を必ず通すようにした(`StatSourceError` に `BaseStats(#[from] BaseStatsError)` を追加)** / 従来はキャラ保存時(`storage::CharacterRepository::create`/`update`)にしか値域検証が無く、Tauri コマンドを直接叩けば素ステ 9999 のような値でプレビューが通ってしまう穴があった。`storage` と同じ検証を domain の入口(`preview_effective_stats`)にも一本化した / `crates/domain/src/stat_sources.rs` の `preview_effective_stats` テストを、全ステが 1..=310 の範囲内になるよう修正した上で確認
5. **`Adjustments::validate()` を追加し(`add` は -999..=999、`pin` は 1..=2400)、`StatSources::validate()` から呼ぶようにした。`calculate_damage` コマンドは `temporary_adjustments` を `apply_temporary_adjustments` に渡す前に `validate()` を呼ぶ** / 調整値(`Adjustments`)は「検証・未収録バフ用の自由加算」という位置づけからこれまで値域チェック対象外だったが(2026-08-21 独立レビュー指摘セクション参照)、Tauri コマンドを直接叩けば `pin` に負値や極端に大きい値を入れて計算・保存できる穴があった。`pin` の上限 2400 は最終能力値の理論上限(エタの意志 Lv80、本ファイル冒頭「2026-08-21 キャラステータス補正源」セクション参照)に合わせた(旧来の暫定上限 99999 から差し替え) / `crates/domain/src/stat_sources.rs` の「調整値のaddとpinは境界値を許容し範囲外を拒否する」テスト(境界値 OK・範囲外 NG)
6. **`preview_effective_stats` コマンドのエラーをトースト(画面上部の共通エラー帯)ではなく、`CharacterData.svelte` の能力値表の直前に控えめな1行(`previewError`)として表示するようにした** / 独立レビューで「入力の値域を試している最中に毎回トーストが出るのはノイズが多い」指摘。エラーの原因(入力値)がすぐ上にある画面(能力値表)の近くに出す方が、ユーザーが原因箇所を見失わない / `apps/desktop/src/pages/character/CharacterWorkspace.svelte`(preview の `$effect` が失敗時に `previewError` を設定、トースト呼び出しは削除)、`apps/desktop/src/pages/character/CharacterData.svelte` の `previewError` prop・`.preview-error` スタイル
7. **`stat_sources::apply_pins` のシグネチャを `(stats, traces, base: &Adjustments, temporary: Option<&Adjustments>)` に変更し、内部で base/temporary の優先順位と `PinSource`(`Saved`/`Temporary`)出所判定を同時に行うようにした。`merge_pins` は削除した。`StatTrace` に `pin_source: Option<PinSource>` を追加し、`DamageInput` に `temporary_pins: Option<Adjustments>` を追加(`pins` はキャラの保存済み調整値のまま名前は変えない)。フロント(`api/types.ts` の `PinSource`/`StatTrace.pin_source`)・`TracePanel.svelte` の `pinnedBeforeLabel` も追従させ、「保存済み pin と一時 pin の値が一致するかどうか」で出所を推測していたロジックをやめ、サーバが返す `pin_source` をそのまま見る形にした** / 独立レビューで「フロント側の値一致比較による pin 出所の推測(2026-08-21 キャラ画面 v2 セクション #20 参照)は、保存済み pin と一時 pin がたまたま同じ値のとき出所を誤判定しうる」指摘。出所の決定はサーバ側(`apply_pins` を呼ぶ唯一の場所)に一本化する方が正しい層に置ける。`merge_pins` で事前に pin だけ合成してから `apply_pins` に渡す二段構えは、`pin_source` を追加すると経路が二重管理になるため `apply_pins` に統合した / `crates/domain/src/stat_sources.rs` の「apply_pinsはtemporaryを優先しなければbaseにフォールバックし出所を記録する」テスト、`crates/domain/src/damage.rs` の「temporary_pinsが保存済みpinを一時的に上書きしpin_sourceがtemporaryになる」テスト
8. **ダメージ計算画面(`DamagePage.svelte`)の「選択が揃ったら自動計算」`$effect` に 100ms debounce を追加した(`CharacterWorkspace.svelte` の preview `$effect` と同じパターン)** / 独立レビューで「一時調整の数値欄をドラッグ・連打すると、その都度 `calculate_damage` が呼ばれてバックエンドに負荷をかける」指摘。`requestSeq` による古い応答の破棄はそのまま残し、debounce と併用した / `apps/desktop/src/pages/damage/DamagePage.svelte`
9. **重複整理: (a) `StatSources` のディープコピーを `draft.ts` の `cloneStatSources`/`neutralStatSources` に一元化し、`CharacterWorkspace.svelte`(`buildDraft`・`save()` は `$state.snapshot(draft.statSources)` を使用)・`CharacterRegisterForm.svelte` の重複実装を削除した。(b) 調整(加算/固定)の入力UIを `ui/AdjustmentEditor.svelte` に切り出し、`CharacterSettings.svelte`(キャラ編集の「調整」グループ)・`DamagePage.svelte`(一時調整)の重複マークアップ・`togglePin`/`toggleTemporaryPin` ロジックを削除した** / 独立レビューで「同じ組み立てロジック・同じ調整UIが複数箇所に手書きで重複しており、片方だけ直す事故が起きやすい」指摘。CLAUDE.md「複雑さを減らすなら実績あるライブラリを使う。一般的な機能を理由なく再実装しない」の精神に沿い、社内的な重複も1箇所に集約した / `apps/desktop/src/pages/character/draft.ts`、`apps/desktop/src/ui/AdjustmentEditor.svelte`。`npm run build && npx svelte-check`
10. **domain の値域上限一覧を返す `get_stat_limits` コマンド(`domain::stat_sources::stat_limits() -> StatLimits`)を新設し、フロントは起動時(`App.svelte` の `onMount`)に1回取得して `limits.svelte.ts` のモジュールスコープ `$state` に格納する。`CharacterData.svelte`(素ステ上限)・`CharacterSettings.svelte`(ルーン/クラウン/聖物上限)・`AdjustmentEditor.svelte`(加算の下限/上限、固定の下限/上限)はこれまでのリテラル値(310/20/300/40/-999..999/1..99999 等)ではなく `limits` を参照する。`AdjustmentEditor.svelte` は `pinMax` だけでなく `pinMin` も props で受け取り、コンポーネント内にハードコードしない** / 独立レビューで「フロント側に散らばる上限リテラルが Rust 側の定数(`BASE_STAT_MAX` 等)と手動同期に頼っており、片方だけ変えるとズレる」指摘。取得完了までの一瞬は `limits.svelte.ts` の `FALLBACK`(既存リテラルと同値)を使うため、初期表示が壊れることはない / `crates/domain/src/stat_sources.rs` の `stat_limits`/`StatLimits`、`apps/desktop/src/limits.svelte.ts`、`apps/desktop/src-tauri/src/lib.rs` の `generate_handler!` に `commands::get_stat_limits` を追加

出典・確認方法: `cargo test --workspace`(98件)、`cd apps/desktop && npm run build && npx svelte-check`(0 errors / 0 warnings)を実行し通過を確認した。

### 独立レビュー指摘の修正(2026-08-22、2回目)

上記10項目の実装後、独立レビューエージェントによる再レビューで3件の指摘を受け、修正した。

11. **`CharacterRepository::from_connection` のマイグレーション判定を `PRAGMA user_version` の値だけに頼らず、`PRAGMA table_info(characters)` で `stat_sources` 列の実在を直接確認する形に修正した** / 上記1番の記述を参照。独立レビューで発見された重大なバグ(実運用で確実に踏む: 一度でも起動した DB は `duplicate column name` でリポジトリ初期化に失敗する)の修正 / `crates/storage/src/character_repository.rs` の「列は既にあるがuser_version未設定のdbも開ける」テストを新規追加
12. **`ui/AdjustmentEditor.svelte` の pin 下限(`PIN_MIN = 1` のハードコード)を props の `pinMin` に変更し、`DamagePage.svelte`/`CharacterSettings.svelte` から `limits.adjustment_pin_min` を渡すようにした** / 独立レビューで「加算の下限(`addMin`)は `limits` から渡しているのに pin の下限だけハードコードのままで非対称」指摘。上記10番の decisions と整合させた / `npm run build && npx svelte-check`
13. **`DamagePage.svelte` のキャラ切替検知(`let lastCharacterId = character?.id;`)を `untrack(() => character?.id)` に変更した** / 独立レビューで、`character` が `$derived` のためトップレベル初期化式で読むと初回値しか捕捉されないという `svelte-check` 警告(`state_referenced_locally`)が新規に出ていることを指摘された。`CharacterWorkspace.svelte` が同じ理由で既に `untrack` を使っているパターンに倣った / `npx svelte-check` が 0 warnings で通ることを確認

出典・確認方法: `cargo test --workspace`(98件、全通過)、`cd apps/desktop && npm run build && npx svelte-check`(0 errors / 0 warnings)を実行し通過を確認した。

## 2026-08-22 Claude Code エージェント運用の整理

直近セッションの transcript(`~/.claude/projects/<repo>/`)を集計した結果に基づく決定。

- **Subagent 内で CLAUDE.md の実行ワークフローを適用しない(再委譲禁止)** / 旧 CLAUDE.md の「毎回 researcher → implementer → reviewer」を Sonnet の implementer も読み、内部で同じ 3 役を再帰起動していた(メインからの Agent 起動 19 回に対し Subagent transcript 49 本、implementer 5 回起動に対し transcript 18 本。最大 4 段ネスト)。researcher の fork も調査を 6 並列 × 2 段で複製 / `~/.claude/agents/*.md` に `disallowedTools: Agent`、CLAUDE.md に「この節はメインセッションにのみ適用」
- **変更を Small / Normal / Complex の 3 段階に分類し、フルワークフローは Complex のみ** / 「/code-review 指摘 10 件の修正」ですら reviewer(検証)→ implementer(内部で researcher → implementer → reviewer → implementer)→ general-purpose(実機確認)と流れていた / CLAUDE.md「実行ワークフロー」
- **`reviewer` と `/code-review` を同一変更に重ねない** / `/code-review` は内部で 1 オーケストレータ + 16 Agent(Fable モデル)を起動し指摘を検証済み。その後 reviewer で再検証していた / docs/claude/workflow.md「レビューの重複禁止」
- **実機 GUI 確認は `smoke-tester`(Sonnet / medium)** / general-purpose は親の Fable モデルで動き、スモークテスト 5 回で Subagent 出力トークンの約 1/4 を占めた / `~/.claude/agents/smoke-tester.md`
- **implementer は effort medium、researcher / reviewer は high 維持** / 実装は依頼に受け入れ条件と対象ファイルが付くため high の余地が小さい。調査・独立レビューは Complex 限定なので品質優先 / `~/.claude/agents/*.md`
- **Context 管理は運用推奨(goal ごとに `/clear`、150k 超で `/compact`、`/code-review` は専用セッション)** / 13.5 時間・3 goal・最大 276k context のセッションが発生。Claude Code は自動で `/clear` `/compact` できない / docs/claude/workflow.md「Context 管理」

## 2026-08-22 装備攻撃力(docs/claude/goals/2026-08-22-equipment-attack.md)

カテゴリ A(攻撃力)の「装備攻撃力」項を実装し、登録キャラに装備補正を持たせた(中盤以降の敵で `A+B−C` が負になり下限1固定になる既知の制約を解消)。

1. **`Equipment { base: EquipmentValues, enhanced: EquipmentValues, power_weapon: bool, strong_weapon_level: u8 }`(`crates/domain/src/equipment.rs`)を新設した。`EquipmentValues` は突き/斬り/魔攻/魔防の4値。装備品を部位ごとに登録せず、ゲーム内ステータス画面の「基本能力値」「強化能力値」の合計値のみを持つ** / goal のスコープ(装備品の個別登録は対象外)。`stat_sources.rs`(ペット/ルーン/クラウン/聖物/バフ/調整値)と同じ「キャラに紐づく補正源一式」という構造を踏襲した / `crates/domain/src/equipment.rs`
2. **装備攻撃力係数(`EquipmentCoefficients { base: EquipmentRates, enhanced: EquipmentRates }`)はステ由来攻撃力係数(`AttackCoefficients`)と対になる別の型にし、gamedata の `equipment_coefficients(dependency)` で持つ。値は wiki「計算式まとめ#BaseAttackPower」(取得 2026-08-22)の表をそのまま転記した** / ステ係数(`attack_coefficients`)と装備係数は wiki 表で行を共有するが、対象(ステ vs 装備補正)・単位が異なるため同じ構造体に混ぜず分離した。MR 依存の魔攻強化係数は wiki 注記どおり 19.25(韓国情報 16.75 とは異なる)を採用した / `crates/gamedata/src/characters.rs::equipment_coefficients` の「依存種別ごとの装備係数」テスト(6種すべて)
3. **装備攻撃力強化倍率 = パワーウェポン +2%(Lv1 のみ)+ ストロングウェポン Lv×3%(Lv1〜6 = 3/6/9/12/15/18%)。両者は重複可** / 出典: wiki「Skill/共通」(取得 2026-08-22)のパワーウェポン「自身の装備補正を2%増加して与ダメージを算出する(ストロングウェポンと重複可)」、ストロングウェポン「3%/6%/9%/12%/15%/18%」。旧 docs の値とも一致 / `crates/domain/src/equipment.rs` の `Equipment::enhance_rate()` テスト
4. **装備補正 4 値の値域上限は 0..=9999** `[仮]` / wiki に明記の上限が無く、`fixed_increase`(固定増加系)等これまでの暫定上限と同じ考え方(実用上の安全域)で採用した。実際の上限が判明したら差し替える / `crates/domain/src/equipment.rs::EQUIPMENT_VALUE_MAX`
5. **storage の `characters` に `equipment TEXT NOT NULL DEFAULT '{}'` 列を追加し、`stat_sources` と同じ方式(`PRAGMA table_info` で列の実在を直接確認して個別に `ALTER TABLE`、`SCHEMA_VERSION` を 3 に更新)でマイグレーションした** / 2026-08-21 のレビューで確立した「`user_version` だけに頼らず列の実在を見る」方式をそのまま再利用。`stat_sources` 列だけ既にあり `equipment` 列が無い状態(このブランチ以前の DB)を実際にテストで再現した / `crates/storage/src/character_repository.rs` の「列は既にあるがuser_version未設定のdbも開ける」テストの `equipment` アサーション追加・「equipmentはjsonで往復する」「装備の値域違反は拒否する」テスト新規追加
6. **`DamageInput` の `equipment_attack: f64`/`equipment_enhance_rate: f64`(中立値決め打ちフィールド)を廃止し、`equipment: Equipment`/`equipment_coefficients: EquipmentCoefficients` を `DamageInput::new` の必須引数に昇格させた。`calculate_damage`(コマンド)が `character.equipment` と `gamedata::equipment_coefficients(skill.dependency)` を渡す** / 2026-08-21 の「未実装要素の中立値をコマンドに書かせない」設計(docs/architecture.md、`stat_modifiers`/`stat_contributions` で確立済みのパターン)をそのまま踏襲。装備が実装された以上、中立値を domain 内で決め打ちする理由が無くなった / `crates/domain/src/damage.rs` の `攻撃力_乱数_防御力_スキル倍率_cri倍率` 等の既存テストを `Equipment::default()`/`EquipmentCoefficients::default()` で再構成、`apps/desktop/src-tauri/src/commands.rs::calculate_damage`
7. **トレースの攻撃力(A)の内訳は `evaluate()`(カテゴリ集計 `totals` からしか式を作れない既存関数)を変更せず、`calculate_damage` が「ステ攻撃力」「装備攻撃力」「装備攻撃力強化倍率」「攻撃力(A)」の4段の `FormulaStep` を組み立てて `steps_min`/`steps_max`/`steps_critical` の先頭に付け足す形にした** / `evaluate()` は `CategoryTotals`(合算済みの A の値)しか受け取らず、ステ/装備の内訳を式として表現できない。A は B(乱数)を含まないため min/max/critical で同じ内訳になり、3箇所に同じ4段を差し込むだけで済む。`evaluate()` 自体のテスト(「全カテゴリが式に配線されている」等、`totals` を直接組み立てて呼ぶテスト)は無変更で通る / `crates/domain/src/damage.rs::attack_power_breakdown_steps`、「トレースに全カテゴリが出る」テスト(`steps_min.len()` を 10→14 に更新し先頭4件の `name` を確認)
8. **UI の「装備」グループは、装備補正8値(基本/強化 × 突き/斬り/魔攻/魔防)を `StatInput`(min 0、max は `limits.equipment_value_max`)で、装備攻撃力強化倍率をチェックボックス(パワーウェポン)+ `Select`(ストロングウェポン Lv なし/1〜6、選択肢に % を併記)で入力させる。係数(wiki 由来の数値)は一切入力させない** / docs/ux-guidelines.md 原則1(既知の情報を入力させない)・原則4(通常操作は選択)。装備補正の実測値(基本/強化の8値)はゲーム内ステータス画面を見ないと分からない個人差のある値なので入力欄が適切だが、強化倍率(パワーウェポン/ストロングウェポン)は「持っているかどうか・Lv いくつか」という離散的な既知の選択なので Select/チェックボックスにした / `apps/desktop/src/pages/character/CharacterSettings.svelte` の「装備」アコーディオン
9. **装備強化(+1〜+15)による武器の追加固定ダメージは今回のスコープに含めない** / goal の wiki 調査結果で「これはカテゴリA(装備攻撃力)ではなく§5の追加ダメージであり、丸め・適用可否が個別の別項目」と判明した。カテゴリAの装備攻撃力(本 goal のスコープ)とは算出の位置づけが異なるため、別 goal で扱う / docs/damage-formula.md §9(未実装として明記)、§5(既存の「武器強化」行)

出典・確認方法: `cargo test --workspace`(106 件: domain 70 / gamedata 19 / storage 17)、`cd apps/desktop && npm run build && npx svelte-check`(133 files, 0 errors / 0 warnings)を実行し通過を確認した。独立レビュー(reviewer)は指摘なしで合格。

## 2026-08-24 v4 UI 全面刷新(docs/claude/goals/2026-08-24-v4-ui-redesign.md)

デザインモック「TW Toolkit Prototype v4」(claude.ai/design プロジェクト)を実データで動く形で実装した。

1. **画面構成を「上部タブ(ホーム/ダメージ計算/キャラ)+ 左キャラレール」に変更し、旧サイドバー+3カラムのキャラ画面・ダメージ画面を削除した(後方互換なし)** / v4 の中心思想は「どのキャラの話をしているか」をレールが常に持つこと。旧 `pages/character`・`pages/damage` は削除し、`pages/home`・`pages/calc`・`pages/chars` に置き換えた / 実機スモークテスト・`npx svelte-check`
2. **ライトテーマ + M PLUS フォントの同梱**: `app.css` のトークンを v4 の配色(水色地・白カード)に置換し、`@fontsource/m-plus-rounded-1c`・`@fontsource/m-plus-1-code` を npm 依存としてバンドルした / デスクトップアプリはオフラインでも起動するため Google Fonts のリンク読込は使わない / `npm run build` でフォントが dist に含まれること
3. **`domain::content` を新設**(`Content` / `ContentArea` / `ContentRequirement` / `RequirementCheck` / `evaluate_content`)。入場条件は登録キャラのデータから判定できる値のみ(装備 突き(基本)・突き+斬り合計・エタの意志 Lv)とし、テシスコア等モデルに無い値は条件データに持たせない / 判定できない条件をデータに置くと「常に未達」か「常に無視」のどちらかの嘘になる。ロードマップ機能(docs/architecture.md)の最小版 / `crates/domain/src/content.rs` の境界値・クリア判定テスト
4. **gamedata に `contents.rs` を追加 `[仮]`**: 既存の敵 3 体(トゥタトゥール / 兄弟の鍛冶場 / オーディン(ランク))を流用したコンテンツ 3 件(エリア: 狩り場 / ボス)。目安ダメージ(`need_per_hit` = 実用的に周回できる 1 ヒット最大の目安)と入場条件の数値は暫定 / v4 モックの AREAS(20 件超)は数値がモック用の架空値で実式とスケールが合わないため転記しない。wiki「狩り場情報一覧」取込後に本データへ置換する / `crates/gamedata/src/contents.rs` のテスト(id 一意・enemy_id 実在)
5. **コマンド追加**: `list_contents` / `preview_damage`(保存前のキャラデータで計算する。試し変更・もし〜だったら・候補比較に使用)/ `evaluate_contents`(全コンテンツ判定。ホームの一覧とレールのクリア数)。保存前データの検証は `storage::validate_new_character`(従来 private だった `validate` を公開)で保存時と同一にした。共通の入力組み立ては `commands.rs::build_damage_input` に一本化 / 「UI は表示と入力のみ、計算・判定は Rust 側」(docs/architecture.md)。sim の計算をフロントで再実装しない / `cargo test --workspace`(コマンド自体は薄い合成。構成要素は既存テストでカバー)
6. **試し変更(sim)は `state.svelte.ts` の `sim: NewCharacter | null` 1 個で持つ**。編集は「JSON クローン → 書換 → 差し替え」、差分チップは knob(PW / SW / 装備 8 値 / バフ選択 / 調整)単位の比較で出し、✕ でその knob だけ保存値へ戻す。「キャラに保存」は `update_character`。キャラ切替・削除に加え、選択中キャラが(どのタブからでも)保存されたときも `upsertCharacter` が破棄する。sim は保存時点のスナップショットなので、残すと「キャラタブで保存 → 計算タブの古い sim を『キャラに保存』」の順で最新の保存が黙って巻き戻るデータ消失があった(独立レビュー指摘 #1)。差分チップの knob も素ステ・覚醒・恒常補正・名前/キャラ種まで網羅し、「試し変更中なのにチップが空」の状態を無くした(同 #2) / v4 の「試し変更 → キャラに保存」フローを、既存の draft/保存パターンと矛盾しない形にした / 実機確認(チップ表示・ぜんぶ戻す・保存)+ クロスタブ保存で sim が破棄されること
7. **強化候補(もし〜だったら / 次に変えるなら)は固定 4 候補(パワーウェポン ON・ストロングウェポン Lv6・強化能力値+100・基本能力値+100)をフロントで列挙し(`candidates.ts`)、効果は `preview_damage` で再計算する** / 現行モデルで実際に表現できる変更だけを提示する(モックにあるコア・称号などの候補は元データが無いので出さない)。候補の列挙は表示の問題、ダメージへの効果は Rust / ホームの候補押下 → 計算タブで sim 適用の動線を実機確認
8. **常用バフの選択 UI は計算タブ「計算の材料」へ移動**(チップのトグル、試し変更として反映 → キャラに保存)。キャラタブは補正源ドリルダウン(キャラステータス / 装備 / ペット / ルーン / クラウン / 聖物 / キャラスキル / 調整)+「いまの実力」シートにした。バフの細かい値(対象ステ・選択肢・手入力)の編集 UI はチップでは表現しないため、当面は既定値でのトグルのみ(既定値 = `defaultChoice`、従来の toggle と同じ) / v4 の配置(「常用バフはダメージ計算タブで」)。バフ詳細の編集は必要になったら計算タブに追加する `[仮]` / 実機確認
9. **スコープ外(v4 モックにあるが実装しない)**: 部位別装備(14 枠・エンチャント・等級/強化値・ランダム OP・アビリティ)= 装備モデルは合計 8 値 + PW/SW のまま(2026-08-22 装備攻撃力 #1 の決定を維持。モックの部位・アイテム数値は架空値)/ シエナのオーラ・テシスコア・称号・モンスターカードはキャラタブにグレーの「これから」行として表示 / スキルの単体・範囲区分(gamedata に区分が無い)/ 聖域の難易度スケール(対象コンテンツ未収録)/ ホーム右カラムの A 攻撃力内訳カード(計算タブ「なぜこの数字?」に集約) / いずれも対応する静的データ・モデルの整備が先(wiki 取込の別 goal)

出典・確認方法: `cargo test --workspace`(111 件)、`cd apps/desktop && npm run build && npx svelte-check`(137 files, 0 errors / 0 warnings)。実機スモークテスト(smoke-tester、docs/screenshots/40〜43)。
10. **ホームの火力は「最大ダメージのスキル」で判定し、その旨とスキル名を選択中カードに明示する。ホーム → 計算タブの遷移(計算シートで試す / 次に変えるなら)では判定に使ったスキルを `calcSkillId` で引き継ぐ** / 実機スモークテストで「同一キャラ・同一対象なのにホーム(最大スキル: 極・残影斬)と計算タブ(既定 = 先頭スキル: 極・横斬り)で数値が食い違って見える」指摘。どちらの数値も式としては正しく、経路の違いを明示+引き継ぎで解消した(計算タブのスキルはその後自由に変更できる) / 実機確認(遷移後のスキル・数値一致)

### 方針更新(2026-08-24、ユーザー確認後)

v4 実装時に司令塔が独断で決めたスコープ 4 点をユーザーに確認し、次のとおり方針を更新した。

- **部位別装備**: #9 の「実装しない」を変更。「モックが正しい前提で wiki 取り込みも行い詰める」(ユーザー決定)。構造はモック準拠・数値は wiki 裏取り → docs/claude/goals/2026-08-24-equipment-parts.md
- **コンテンツ拡充**: wiki 取込ベース+Mob データは旧リポ monsters.json(28 体)をシードにする(ユーザー決定)→ docs/claude/goals/2026-08-24-contents-expansion.md
- **常用バフの詳細編集**: 計算タブに追加する(ユーザー決定)。#8 の `[仮]` を解消: 計算の材料のバフカードに、ON のバフの対象ステ Select・効果量 Select・手入力 StatInput(%スケールはレイヤーで分岐、既存 formatLayerValue/isPercentLayer を再利用)を表示し、編集は試し変更(editSim)として反映 →「キャラに保存」で常用セット化 / `apps/desktop/src/pages/calc/CalcPage.svelte` の buff-detail
- **シエナのオーラ・テシスコア**: 次の goal でモデル追加(ユーザー決定)→ docs/claude/goals/2026-08-24-siena-thesis.md(入場条件 ThesisCoreTotal の追加を含む)

### PR #4 レビュー指摘の修正(2026-08-24)

ユーザーのレビュー 8 件(正しさ 7 / 効率 1)をすべて妥当と判定し修正した。

1. **計算タブの「調整」を一時調整に戻した**(sim から分離し、`preview_damage` の `temporary_adjustments` 引数に乗せる)。「キャラに保存」に含まれない旨を UI に明記 / sim 経由だと「もしステ+50なら」が保存で永続化され、以後の到達判定が水増しされる。旧仕様(2026-08-21 #4「一時調整は計算リクエスト専用」)を踏襲 / `CalcPage.svelte` の `temporaryAdjustments`(キャラ切替でリセット、main/what-if/スキル一覧の全計算に適用)。装備・バフの試し変更は「キャラに保存」対象のまま(チップで可視・明示操作のため意図どおり)
2. **キャラ切替時に skillId を同期的に空へ戻す**(`skillsGid` ガード。listSkills の古い応答も破棄)/ 応答まで「別キャラのステ × 前キャラのスキル」で計算・表示され、そのまま保存フローも動けた / 保存等でキャラのオブジェクトだけ変わった場合は選択を保つ
3. **ホームの「次に変えるなら」は既存の試し変更の上に候補を重ねる**(app.sim を無確認で作り直さない)
4. **`refreshEvaluation` にキャラ id ごとの seq ガードと存在チェックを追加**(連続保存の古い応答・削除済みキャラの評価復活を防ぐ)
5. **計算タブのバッジは評価未取得の間「判定中」**(`entryKnown`)。入場条件が不明のまま「通る/余裕」を出さない
6. **ホームの行状態に「判定中…」を追加**(`!r.ev` と「スキル未収録」を区別)。評価未取得時の「再判定」ボタンを追加(失敗時のリトライ経路)
7. **スキル一覧のキャッシュ `skillTotals` を対象・キャラ・試し変更の変化で即クリア**(前の敵の数値を出さない)
8. **`evaluate_contents` のループ不変値(バフカタログ・ステ補正)を 1 回だけ構築**し、コンテンツ×スキルのループでは clone を使う

出典・確認方法: `cargo test --workspace`、`cd apps/desktop && npm run build && npx svelte-check`。

## 2026-08-24 部位別装備モデル(docs/claude/goals/2026-08-24-equipment-parts.md)

装備を部位別 12 スロットに刷新し、武器強化の追加固定ダメージ(§5)を実装した。

1. **部位は 12 スロット(兜/鎧/武器/盾/盾+/頭/体/手/足/効果/AF/レリック)。モックの「カフス」は wiki の「盾+」と同定した** / wiki「装備システム」冒頭の表が正。カフスという部位は wiki に存在しない。効果/AF/レリックも装備値(基本能力値)の一部なので部位として持つが、カタログは当面カスタム入力のみ `[仮]` / `crates/domain/src/equipment.rs::PartSlot`、goal 文書「wiki 調査結果」
2. **モックの「等級(最下〜最上)」は採用しない。部位の基本能力値をカタログ既定値(レンジ中央)から上書きできる形にした** / wiki に等級概念は無く、実体は MR(アイテム表のレンジ内で基本補正値を振り直す)。追加固定ダメージも実際の補正値から計算するため wiki 式と一致する / `EquipmentPart::base`、UI のレンジヒント表示
3. **武器強化の追加固定ダメージ(§5)= `INT(INT(補正)×倍率)`、奇数なら−1、per-hit は `INT(追加/Hit数)×Hit数`。補正は武器系統ごとの一次式(6 系統、装着アビリティ除外)。+12〜15 はレンジ振りのため実測値の上書き(既定はレンジ下限)** / wiki「装備システム/装備強化」(取得 2026-08-24)をそのまま転記。与ダメージ式の外なのでカテゴリに入れず、`calculate_damage` が per-hit に加算しトレース末尾に段を足す / `crates/domain/src/equipment.rs::weapon_added_damage`・`damage.rs` の Hit 分割テスト(wiki 例 2488/9hit→276)
4. **鎧の強化 Lv は記録のみ(計算未反映)** / wiki: 鎧強化の効果は最大 HP で、HP は未モデル / UI に注記
5. **ランダム OP はスコープ外にした** / wiki の武器 OP はすべて条件付き(後方から/ボス限定/確率発動)の % 追加ダメージ(§5 新-割合、称号と同枠)で、無条件で火力式に入る OP が存在しない。期待値でしか反映できないため称号・新-割合の goal でまとめて扱う。モックの無条件「攻撃ダメージ+X%」OP は架空 / goal 文書「wiki 調査結果」
6. **カタログ seed はアクィルス/アビス系列(全部位)+ 刀・太刀の武器のみ。他武器種はカスタム武器(名前+4値+系統なし)で運用 `[仮]`** / 武器 26 種×全アイテムの転記は非現実的。武器系統→強化式の対応表(30 武器種)は全部入れた / `crates/gamedata/src/equipment_catalog.rs`(出典コメントつき、20 件)
7. **storage v4: 旧 `equipment` JSON(合計 8 値)は破棄し、power_weapon / strong_weapon_level のみ引き継ぐ** / 合計値から部位を再構成できない。判定は「JSON に `parts` キーが無ければ旧形式」。移行前の実 DB はバックアップ済み(talesweaver-toolkit.sqlite.bak-v3-*) / `character_repository.rs::migrate_equipment_to_parts` と移行テスト
8. **`DamageInput` は装備の集計値(`equipment_base_totals`/`enhanced_totals`)を受け、集計は commands 層で行う** / domain は gamedata(武器アビリティカタログ)に依存できないため。バフの `build_modifiers(catalog)` と同じ流儀 / `commands.rs::build_damage_input`
9. **独立レビュー指摘 5 件を修正した**: (a) 武器カタログ 4 件の魔防が wiki の命中列の誤転記(列順 突|斬|物防|魔攻|魔防|命中。goal 文書の seed 表自体の誤りで、レビューが原典突き合わせで発見)(b) 候補「武器を更新」でエンチャントを新上限に clamp せず、1 候補の検証エラーが Promise.all で候補一覧全体を消していた → clamp + `Promise.allSettled` (c) `enhance_added_damage` の値域検証追加(0〜9,999,999 `[仮]`、`ENHANCE_ADDED_DAMAGE_MAX`。UI 上限も専用値に。+12 実測値は 30 万超になり得るため 9999 では不足)(d) カスタム武器の強化ヒント文言 (e) `commands.rs::weapon_added_damage` の 5 分岐テスト追加 / wiki 転記は必ず節ヘッダで列順を確認する(今回の教訓)

出典・確認方法: `cargo test --workspace`(139 件: domain 82 / gamedata 31 / storage 22 / desktop 4)、`cd apps/desktop && npm run build && npx svelte-check`(138 files, 0 errors / 0 warnings)、実機スモークテスト(smoke-tester、docs/screenshots/44〜49、全シナリオ OK)。
