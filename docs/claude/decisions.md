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

## 2026-08-24 コンテンツカタログ拡充(docs/claude/goals/2026-08-24-contents-expansion.md)

敵カタログを 3 体 → 33 体、コンテンツを 2 エリア 3 件 → 8 エリア 33 件に拡充した。

1. **シードは旧リポ monsters.json の実データ 28 体(写像は従来どおり: af63 → 被害減少を符号反転、af64 → カット率A、threshold → 属性閾値)。hp はモデルに追加しない** / 現行の到達判定は per-hit 目安のみで hp を消費する機能が無い(YAGNI)。必要になったら追加する / `crates/gamedata/src/enemies.rs`
2. **wiki「狩り場情報一覧」(取得 2026-08-24)で裏取りし、完全に値が取れた 5 体(プシーキーの虚像・マーキュリアル洞窟ボス・アビスN/H・次元の隙間)を wiki ソースで追加。重複分は旧リポ値と wiki 値の一致を確認した**(兄弟の鍛冶場 7050/-59.5%、アビスヘル 9600/-75%、コアマスター 10200、オーディン ≦53500/-51%、討伐戦 -68.5%、セリニアコス(H) -65%、ゴイティア(H) -68.5%、キシニク(H) -72%、アフェティリアN -51%)。矛盾は無く上書きは発生しなかった / 検証結果は各エントリのコメントと `wiki裏取り値のスポットチェック` テスト
3. **被害減少は旧リポの実測値(4550/5850)を保持 `[仮]`** / wiki は「有/無」のみ記載で「6500 固定と思われる」と推測記述。数値ソースとしては旧リポの方が具体的 / enemies.rs 冒頭コメント
4. **エリア構成は wiki の節構成ベースの 8 エリア(リンゴの島 / アークロン要塞・アビス / レイドボス / シオカンヘイム / エクリプス / エタ / アフェティリア / 古代レリックの聖域)。目安ダメージは全 33 件 `[仮]`(相対難度から按分)、入場条件は判明済み 3 件のみ収録し不明分は requirements 空(火力のみで判定)** / 条件不明を「常に未達」にする嘘を避ける(domain/content.rs の設計方針を踏襲) / `crates/gamedata/src/contents.rs`
5. **モンスターアイコン(旧リポ public/monsters/)の同梱は見送り** / goal で任意扱い、UI 側は無くても動く。必要になったら別 goal で
6. **リンゴボスはルミナスと推定(カット率 -48% 一致、防 8700=1800+6900 で整合)だが確証が無いため旧リポ名のまま保持** / enemies.rs コメント

出典・確認方法: wiki 狩り場情報一覧(2026-08-24 取得)、旧リポ monsters.json・monster-params.md。`cargo test --workspace`(143 件: domain 82 / gamedata 35 / storage 22 / desktop 4)、`npm run build && npx svelte-check`(138 files, 0 errors)。フロントは無変更(データはコマンド経由で流れる)。

### 実測表「モンスター能力値リスト」の反映(2026-08-24 追記)

ユーザー提供の実測表(画像 2 枚。レイティア・設計者 4 行は「一部修正」版が正)を第 3 のソースとして取り込んだ。

1. **防御力の分解が判明: 防御力 = ステータス防御+固定防御。既存 28 体の合計値はすべて実測表と一致**(トゥタトゥール 990+0 〜 キシニク(EX) 1500+118050)
2. **被害減少の実値が確定(0 / 2925 / 3250 / 4550 / 5850)。wiki の「6500 固定と思われる」は不採用のまま、`[仮]` を解消**
3. **最後の決戦2 のカット率を 0.315 → 0.30(実測表 70%)に上書き。3 ソース中唯一の差分**
4. **新規 14 体を追加: レリックの聖域 10〜19(10 段のみカット率 51%、11 段以降 72%)、ゆがんだ村のレイティアN/設計者N(106950・-3250・67.5%)とレイティアH(132150)/設計者H(132300)(-2925・70.75%)** / 敵 47 体・9 エリア 47 コンテンツに
5. **HP(ソロ)は実測表に全件あるが未収録のまま** / 消費する機能が無い(YAGNI、前項の決定を維持)

出典・確認方法: 実測表「モンスター能力値リスト」の画像 2 枚(2026-08-24 ユーザー提供。リポには含めない)。`cargo test --workspace` 全パス(実測表スポットチェックのテストを追加)。

### wiki のみの 5 体を削除(2026-08-24 追記、ユーザー決定)

**収録範囲は実測表「モンスター能力値リスト」の実データ全件に統一し、wiki 狩り場情報一覧にしか無い 5 体(プシーキーの虚像・マーキュリアル洞窟ボス・アビスボスN/H・次元の隙間)を削除した(敵 42 体・9 エリア 42 コンテンツ)** / wiki のみの敵は被害減少が不明(0 `[仮]`)で実測表と精度差がある。wiki は値の裏取り(カット率・防御力の一致確認)にのみ使う / `crates/gamedata/src/enemies.rs`・`contents.rs`、`cargo test --workspace` 全パス

## 2026-08-24 コンテンツ入場条件の実データ化(swiki)

swiki「コンテンツ入場条件」(<https://erumisutoburvip.swiki.jp/>、取得 2026-08-24)を入場条件の正とし、コンテンツカタログを 4 エリア 59 件に再構成した。

1. **装備条件は「使うスキルの依存種別で比較先が決まる」1 件の条件にした(`ContentRequirement::EquipmentBySkill { single, mr, composite }`)** / 表の S/H/I・M・複合列は別条件で「いずれかを満たせば OK」(表の概要)。S=突き・H=斬り・I=魔攻・M=魔防・複合=突斬 or 斬魔で、どれが効くかはスキルで決まる(ユーザー確認)。3 条件を AND で並べると全部満たさないと通らない嘘になり、OR で並べると UI に無関係な行が 3 つ出る / `crates/domain/src/content.rs`、判定は `evaluate_contents` が最大ダメージスキルの `dependency` を渡す
2. **旧 `EquipmentThrust` / `EquipmentThrustSlash` は削除。代わりに `AwakeningStage` を追加** / 後方互換を残さない原則。覚醒段階は下位コンテンツの主条件(3〜5 次)で swiki にある
3. **敵データが無いコンテンツ(神鳥の塒・ルミナスEX・混乱した大地等)も到達一覧に載せる(ユーザー決定)。`enemy_id`/`need_per_hit` を `Option` にし、None なら入場条件のみで判定する** / 表の全コンテンツを一覧に載せたい。火力の話をしない行はバッジ「入場OK / 条件未達」・バーは条件充足率にして、火力判定と混同させない / `Content`、HomePage の rowState 8/9
4. **判定できない条件(ルーンレベル・共通スキルコンプ・コア・カフス上限・前提クリア)は `entry_note` に表示専用で持つ** / キャラモデルに値が無く、条件にすると「常に未達」か「常に無視」の嘘になる(既存の設計方針を踏襲)。黄色の注記カードで「(判定対象外)」と明示 / `Content::entry_note`、HomePage `.sel-entry-note` / CalcPage `.entry-note`
5. **`required` が 0 の装備条件はチェックを生成しない** / 表の "-"(例: リンゴの複合列)は「その系統は条件なし」。0 のまま出すと「0 / 0 OK」という無意味な行になる
6. **計算タブの対象一覧は `enemy_id` を持つコンテンツだけに絞る** / ダメージ計算には敵データが要る。選べても計算できない対象を出さない
7. **ゆがんだ村の対応(ユーザー確認): 追従する喜び = レイティア、見つめる悲しみ = 設計者**

出典・確認方法: swiki コンテンツ入場条件(2026-08-24 取得、cp932 ではなく UTF-8 の PukiWiki)。`cargo test --workspace`(147 件: domain 85 / gamedata 36 / storage 22 / desktop 4)、`npm run build && npx svelte-check`(138 files, 0 errors)。

### 実機確認での修正 2 件(2026-08-24 追記)

スモークテスト(docs/screenshots/50〜62)で 2 件を修正した。

1. **計算タブの対象ピッカーが `app.areas` を直接描画していたため、敵データなしコンテンツが選択不可のグレー行として残っていた**。対象リスト本体と同じ絞り込み(`enemy_id !== null`)をピッカーにも適用し、敵が 0 件のエリアは見出しごと落とす / 選べない行を一覧に置かない / `CalcPage.svelte` の `targetAreas`
2. **計算タブの装備条件が「キャラの最大ダメージスキル」固定で、スキルを切り替えてもラベルが変わらなかった**。`evaluate_contents` に `dependency_skill_id: Option<String>` を追加し、計算タブは選択中スキルを渡すようにした(ホームは None のままコンテンツごとの最大ダメージスキルで判定) / 計算タブは「今このスキルで戦う」文脈なので、表示中のスキルと判定に使うスキルが食い違うのは嘘になる。注記も「選択中のスキル『〇〇』の依存で判定しています」に変更 / `commands.rs::evaluate_contents`、`commands.ts::evaluateContents`

なお実機 GUI で単体依存(突き/斬り/魔攻/魔防)のラベルを再現するには、その依存のスキルを持つキャラの収録が要る(現状スキルデータはボリスのみで全て複合依存)。分岐自体は `crates/domain/src/content.rs` の単体テストで 6 依存すべて検証済み。

## 2026-08-24 シエナのオーラ・テシスコア(docs/claude/goals/2026-08-24-siena-thesis.md)

キャラタブでグレー「これから」だったシエナのオーラとテシスコアを wiki 出典でモデル化した。

### wiki 調査で goal の前提が変わった点

goal の「やること(案)」は「シエナのオーラの Lv 別値」を wiki から取る想定だったが、wiki(装備システム/シエナのオーラ、取得 2026-08-24)の実データは**段階ごとの解放スロット数だけが固定で、中身は再抽選のランダム値**だった。静的データとして同梱できる値が無いため、部位ごとの実測値をユーザーが入れるモデルにした(UX ガイドライン原則1 の例外。wiki から取れない値なので入力させるしかない)。

1. **シエナのオーラは装備部位に持つ(`EquipmentPart::siena`)。専用の補正源ペインから 8 部位を編集する** / wiki「装備システム」冒頭の表の「オーラ」行が対象部位(兜/鎧/武/盾/頭/体/手/足)で、盾+/効果/AF/レリックは対象外。部位に属するのがドメインの実体で、部位制約も既存の `allows_enhance`/`allows_abilities` と同じ形で書ける。UI の入り口は補正源リストに 1 つだけ置き、装備ペインには出さない(二重の入り口を作らない) / `PartSlot::allows_siena`、`SourcePane` の siena ペイン
2. **配線は wiki の記述どおり 3 経路**: 武器/盾の能力値は「装備補正(エンチャント扱い)」→ **強化能力値**、その他部位の STAB〜AGI と追加オプション「全ステータス増加」は**最終固定値層**、追加オプション「攻撃力増加」は wiki が「実際は与ダメージ割合増加(新規カテゴリ)」と書いているとおり **New1**(既存の `SienaAuraAttackRate`) / docs/damage-formula.md §4 A の「強化能力値(エンチャント・テシスコア・シエナのオーラ等)」の記述とも一致 / `Equipment::enhanced_totals` / `stat_sources::apply_siena_stats` / `damage.rs` の New1 テスト
3. **入力粒度は「部位ごとの合計値」(ユーザー決定)。能力値スロット 1 個ずつは持たない** / 段階 10 で最大 10 スロット+追加オプション 3 個を個別に持たせても計算結果は合計と同じで、入力コストだけ増える。段階は記録して解放スロット数の目安として表示する / `SienaAura { stage, values, stats, attack_rate_percent }`
4. **武器/盾は `stats` を、その他部位は `values` を持てないように検証する** / wiki が「能力値一覧(武器/盾)」と「能力値一覧(その他の部位)」で表を分けている。どちらにも入れられると二重計上の温床になる / `SienaAura::validate`、`EquipmentError::SienaValuesNotAllowed`/`SienaStatsNotAllowed`
5. **防御側の追加オプション(物理/魔法ダメージ耐性・クリティカル被撃率・命中/回避・防御力増加・防御無視・中ディレイ・HP/MP/SP)は未収録** / 与ダメージ式に入らない。UI に「未収録」と明示する
6. **追加オプション「全ステータス増加」は独立した入力にし、STAB〜AGI の全ステに同じ値を加算する**(ユーザー確認 2026-08-24: 「30 ならステータス全部 +30 加算されるだけ」) / 当初は能力値スロットのステ加算(`stats`)と同じ枠に混ぜていたため、ユーザーが 7 ステすべてに手入力する必要があった。wiki の構造(能力値スロットの STAB〜AGI と、追加オプションの全ステータス増加は別物)にも合わせる / `SienaAura::all_stats`・`SienaAura::stat_bonus()`、`全ステータス増加は全ステに同じ値が乗る` テスト
7. **値域は wiki の実データから確定した: 能力値スロットのステ加算 0〜100 / 部位・ステ(1〜10 × 最大 10 スロット)、全ステータス増加 0〜30 / 部位(最大帯 21〜30)、攻撃力増加 0〜10% / 部位** / いずれも「同じ種類のオプションは同じ装備の別スロットには登場しない」= 1 部位 1 個から上限が決まる。当初の 999 `[仮]` は解消 / `SIENA_STAT_BONUS_MAX`・`SIENA_ALL_STATS_BONUS_MAX`・`SIENA_ATTACK_RATE_PERCENT_MAX`

### テシスコア

7. **swiki の入場条件「コア 60/120/210/300/480」の正体は 6 枠の火力補正合計だと判明した** / wiki「テシスコア」の進化強化表(火力列: 進化1強化4 = 10、進化2強化4 = 20、進化3強化4 = 35、進化4強化1 = 50、進化4強化4 = 80)× 6 枠が 60/120/210/300/480 に完全一致する。これで「判定できないので注記」だった条件を実判定に昇格できた(2026-08-24「コンテンツ入場条件の実データ化」#4 の対象外リストからコアを外す) / `ContentRequirement::ThesisCoreTotal`、`crates/gamedata/src/contents.rs` の `core()`、`合計値はswikiのコア要求値と一致する` テスト
8. **地域別に 6 枠を持ち、対象コンテンツの地域で自動的に切り替える(ユーザー決定)** / wiki は「各ダンジョン内でキャラクターに追加の特殊能力を付与する」= 能力値増加は対象ダンジョン限定と明記している。1 セットを全地域に効かせると、行き先の違うコンテンツで同じ火力が出る嘘になる / `ThesisCores`(4 地域)、`Equipment::enhanced_totals(region)`
9. **地域は 4 つ(マーキュリアル洞窟/アビス/エクリプス/ルビコナ)。シオカンヘイムは持たない** / wiki: シオカンヘイムのコアは経験値獲得量タイプのみで、セット効果も経験値。与ダメージに一切効かない / `CoreRegion`
10. **タイプは火力 4 種(突き/斬り/魔攻/魔防)+ 補助 4 種(物防/回避/敏捷/命中)の 8 種。経験値タイプは持たない**(ユーザー決定 2026-08-24) / 補助タイプは与ダメージ式に効かないが、装着状態を記録したい。補正値は wiki の「補助」列(進化3 までは火力と同じ、進化4 の強化1 以降だけ 45/50/55/60)。要求値 480 は火力でしか届かない(補助は 6 枠でも 360)。セット効果はタイプを問わず発動する(wiki が区別していない) / `CoreType::is_power`、`POWER_BONUS`/`SUPPORT_BONUS`
11. **補助タイプの値は捨てず「装備値の合計」として保持する(`SupportValues`)**(ユーザー決定 2026-08-24) / 与ダメージ式には渡さないが、防御側・命中/回避を実装するときの入力になる。捨てると後から復元できない。シエナのオーラの命中率・回避率(未収録)も将来ここに合流させる / `crates/domain/src/equipment.rs::SupportValues`、`Equipment::support_totals(region)`、`CoreSet::support_values`
12. **コンテンツ → 地域の対応は wiki「実装済みダンジョンコア」表の「その他発動場所」+ ユーザー確認分(リンゴ = マーキュリアル / 月の女王の軍の訓練所・最後の決戦1〜3・異界の峡谷 兵士 = エクリプス / 混乱した大地・異界の峡谷防衛戦・喜びの残像 = ルビコナ)。それ以外はコア効果なし** / 当初は「表と対応が取れないので地域不明」としていたが、**コア効果が無いコンテンツが実在する**とユーザーが確認したので、地域なしは「不明」ではなく確定値になった(`[仮]` ではない)。**「異界の峡谷」は防衛戦がルビコナ、通常の峡谷(兵士)がエクリプスで分かれる**(同確認。名前が同じでも同一地域とは限らないので、段階違い・派生コンテンツを機械的に揃えない)。地域なしでもセット効果は全地域発動なので K/L には乗る / `CORE_REGIONS`(contents.rs)、`テシスコアの地域はwikiの発動場所どおり` テスト
13. **入場条件「コア N」は、そのコンテンツの地域のコアだけを数える(地域が無ければ 0)**(ユーザー確認 2026-08-24) / 当初は「swiki の表がどの地域か書いていない」ため全地域の最大値で判定していたが、地域のコアだけで判定するのが正。地域が無いまま要求だけあると「常に未達」の嘘になるので、gamedata のテストで弾く / `ThesisCores::total_bonus(None) == 0`、`コア要求のあるコンテンツは必ず地域を持つ` テスト
14. **セット効果は「強化4 のコアが 3 個そろうとその進化段階の効果が発生する」。混在時は成立する最も高い進化段階を採る**(ユーザー確認 2026-08-24: 「基本的に 3-4 の 3 個とかでセット効果発生するイメージ」) / 当初は「成立する候補のうち最も強いものを採る」としていたが、この比較は誤りだった: K(固定値、V1 の前に加算)と L(割合、乗算)はダメージ規模で優劣が逆転する(損益分岐は product ≈ 40,000。実測キャラの product 17,628 では「進化0 の 6セット +800」が「進化4 の 3セット +2%」より 232 高かった)。強さで選ぶのをやめ、進化段階の高い順に最初に成立したものを採ることで、ゲーム仕様の再現になり比較の恣意性も消えた / `CoreSet::set_bonus`
15. **セット効果は地域をまたいで重複する(全地域の合計)**(ユーザー確認 2026-08-24: 「セット効果は重複します。すなわち 4-4 がいっぱいあると強いです」) / 当初は wiki の「セット効果は全ての地域で発動します」を「どの地域にいても効く」とだけ読み、重複しない(最大 1 つ)としていた。実際は地域ごとに発動して加算される。K の上限 1000・L の上限 45% はカテゴリ集計側がクランプする / `ThesisCores::set_bonus`、`セット効果は地域をまたいで加算する` テスト
16. **テシスコアは `StatSources` ではなく `Equipment` に持つ** / 効くのは装備の強化能力値で、`enhanced_totals` に合流させるのが自然。既存の `equipment` 列(JSON)に `#[serde(default)]` で相乗りできるので DB migration も要らない / `Equipment::thesis_cores`

### 影響範囲

17. **ダメージ計算コマンドの引数を `enemy_id` から `content_id` に変えた** / テシスコアの地域はコンテンツに紐づくので、敵 id だけでは解決できない。計算タブもホームも元々コンテンツを選んでいて、そこから敵を引いていた(フロントに地域を解決させるより、コマンドがコンテンツから敵と地域の両方を引くほうが「計算・判定は Rust 側」に沿う) / `commands.rs::find_content`、`previewDamage`/`calculateDamage`
18. **`preview_effective_stats` が装備を受け取るようになった** / シエナのオーラのステ加算が最終能力値に乗るため。キャラタブの「いまの実力」も装備を変えると更新される / `domain::preview_effective_stats(base, sources, equipment, catalog, id)`
19. **装備ペインのエンチャント欄に「シエナ/テシスコアの分をここに含めない」注記を出した** / 従来はこの 2 つが未実装だったため「強化能力値に手で含める」運用だった。移行時に二重計上になる / `SourcePane` エンチャントカード

### 実機確認(docs/screenshots/63〜68)

シエナのオーラ 2 部位(武器: 段階10・突6/斬4・攻撃力+10%、兜: 段階5・STAB+20・攻撃力+5%)とアビス地域のコア 6 枠(進化4強化4 の斬り = 合計 480)を入れて確認した。

- 補正源サマリ「2 部位 ・ 攻撃力 +15% ・ ステ +20」/「最大 合計 480」、地域タブの合計「アビス 480」が出る
- カテゴリトレース: New1 +15%(1.15)、L +5%(1.05)。K は 0(進化4 の 6 セットは割合効果)
- ホームの到達一覧: アビスEX 314,591 → 320,811、リンゴ(地域なし)319,005 → 320,845、エクリプスボス 311,221 → 変化なし
- **エクリプスボスが変わらないのは正しい**: この敵は式部分(A−C 以降)がほぼ 0 に潰れていて、per-hit の大半が武器強化の追加固定ダメージ(式の外、§5)。New1/L は式部分にしか掛からないため見かけ上の差が出ない。リンゴの式部分は 7,785 → 9,625(New1×L = 1.2075 とシエナの強化能力値分で説明がつく)
- ルミナスEX の入場条件が「テシスコア 合計 あと 60」と表示され、注記ではなく実判定として一覧に出る

セット効果の重複と全ステータス増加の確認(docs/screenshots/70〜71): 兜に「全ステータス増加 30」を入れると HACK 185 → 215、INT/DEX/AGI/MR 169 → 199 と 7 ステすべてに +30 が乗る(STAB は pin 中のため 1,000 のまま)。アビス(進化4強化4 ×5 + 命中 ×1)とエクリプス(進化4強化4 ×6)の 2 地域でセットを完成させると、カテゴリ L が **+10%**(5% + 5%)になり重複が効いている。

補助タイプと地域確定の追加確認(docs/screenshots/69): タイプ選択肢が火力 4 +「〜(補助)」4 の 8 種になり、アビス 6 枠目を斬り(進化4強化4)から命中率補正に替えると合計は 480 → 460(補助は 60)、アビスEX のダメージは 320,811 → 319,791(火力 −80 分だけ下がる = 補助は装備攻撃力に入っていない)。ルビコナにコアが無い状態で喜びの残像は「テシスコア 合計 あと 60」、マーキュリアルのリンゴは 320,845 のまま変わらず、地域別判定が効いている。

出典・確認方法: wiki「テシスコア」「装備システム/シエナのオーラ」「装備システム」(いずれも取得 2026-08-24)、swiki コンテンツ入場条件。`cargo test --workspace`(168 件: domain 103 / gamedata 39 / storage 22 / desktop 4)、`cd apps/desktop && npm run build && npx svelte-check`(138 files, 0 errors / 0 warnings)、実機スモークテスト(docs/screenshots/63〜71、page/console エラーなし)。


## 2026-08-25 キャラ画面の攻撃力(A)可視化(docs/claude/goals/2026-08-24-mock-alignment.md 段階1)

1. **キャラに「主軸スキル」(`main_skill_id`)を持たせ、キャラ画面の攻撃力はその依存種別で出す**(ユーザー決定 2026-08-24) / 攻撃力 A は依存種別(突き/斬り/魔攻/魔防/複合)で装備係数が変わるので、スキルを決めないと 1 つの数字にできない。モック 12a は「補正源をいじるとこの数字が動く」ことを画面の軸にしており、そこにスキル選択を毎回持ち込むと軸がぶれる / `storage::NewCharacter::main_skill_id`(SQLite v5)、`RegisterPane`/`SourcePane` の主軸スキル欄
2. **主軸スキルは未選択(`NULL`)を許し、そのときは攻撃力を出さず「主軸スキルを選ぶと攻撃力が出ます」と書く** / スキル未収録のキャラ(gamedata はボリス 5 件のみ)がある。空欄にすると「0 なのか未実装なのか」が区別できない / `StatPreview::attack: Option<AttackPreview>`、`.attack-foot.empty`
3. **攻撃力の算出はダメージ計算と同じ経路(`attack_power_breakdown`)を通す。計算を二重に書かない** / `preview_effective_stats` 側に別式を書くと、片方だけ直す事故が起きる。内訳を出すために `equipment_attack_power` を `equipment_values_attack`(4 値 × 係数の内積)に分解し、基本/強化それぞれを単独で出せるようにした / `domain::attack_power::attack_power_breakdown`、`damage.rs` の `calculate_damage` も同関数を通る
4. **キャラ画面の A は「地域なし」の値にし、その旨を画面に書く** / テシスコアの能力値増加は対象ダンジョン限定(`enhanced_totals(region)`)で、キャラ画面はコンテンツを選ばない。地域を勝手に 1 つ選ぶと嘘になる / `attack_power_of` は `enhanced_totals(None)`、フッタとシートに注記
5. **部位の寄与は「その部位を未装備にした状態を丸ごと計算し直した A」との差にする** / 装備は装備攻撃力だけでなくシエナのオーラのステ加算経由で最終能力値にも効き、pin があるとその分は消える。装備値の差分だけを引くと合わない / `Equipment::without_part`、`PartAttackContribution`、`部位の寄与は外したときの攻撃力との差に一致する` テスト
6. **主軸スキルの所有チェックは commands 側で行う(`validate_main_skill`)。キャラ種の変更時は UI が同期的に選択を外す** / `game_character_id` の存在チェックが既に commands にあり、storage を gamedata のスキル一覧に依存させたくない。UI 側は `$effect` ではなく Select のセッターで外す(非同期の取りこぼしを作らない) / `commands.rs::validate_main_skill`、`SourcePane::setGameCharacterId`
7. **storage は v5。`main_skill_id TEXT`(NULL 可)を列の実在確認つきで `ALTER TABLE` する** / v4 と同じ方式。`PRAGMA user_version` だけで判定しない(この DB は列を持ちながら 0 のままになりうる) / `main_skill_id列の無いdbを開くと既存キャラは未選択で読める` テスト

### 実機確認(docs/screenshots/72〜74)

検証ボリス(既存 DB・主軸スキル未選択で読めることを確認)で:

- 未選択のときフッタは「主軸スキル未選択 / 主軸スキルを選ぶと攻撃力が出ます」。極・残影斬(複合)を選ぶと **A = 24,091**、内訳はステ 2,187 / 装備基本 6,467 / 装備強化 11,787.5 / 強化倍率 +20%
- 極・縦斬り(斬り依存)に変えると A = 20,601 に変わる(依存種別で装備係数が変わる)
- 武器の部位詳細の「この枠の寄与 −21,904」が、実際に未装備にしたときの A(24,091 → 2,187)の差と一致
- パワーウェポンを切ると A 24,091 → 23,716(補正源を触ると即時に動く)
- 登録ペインで主軸スキルを選んで登録 → 変更 → 保存 → リロードで値が残る(v5 の往復)

## 2026-08-25 装備ペインの密度(docs/claude/goals/2026-08-24-mock-alignment.md 段階2)

1. **武器アビリティの系統(`EquipmentAbilityFamily`)をデータとして持たせ、UI は系統ごとの Select 4 行にする** / 従来は 28 件の全チェックボックスで、同系統(尖った刃の (下) と E- など)を同時に選べてしまった。系統を id 接頭辞や `values` の非ゼロ位置から推測すると、カタログを増やしたときに壊れる / `domain::EquipmentAbilityFamily`、`アビリティは4系統各7段で系統と加算先が対応する` テスト
2. **同系統の重複は storage の検証でも弾く** / UI の形だけで保証すると、旧データや将来の別経路で壊れる。`validate_equipment_catalog` はカタログ整合性を見る唯一の場所なのでそこに置く / `同じ系統のアビリティを2つ持つと拒否する` テスト
3. **旧データの同系統重複は「部位詳細を開いたとき」に 1 つへ畳む** / 検証だけ足すと、重複を持つ既存キャラが保存できないまま UI から直せない詰みになる。開いた時点で正規化すれば未保存バッジが出て、そのまま保存できる / `SourcePane::openPartDetail`
4. **パワーウェポン ON・ストロングウェポン Lv6(合計 +20%)を新規登録キャラの既定にする**(ユーザー決定 2026-08-24 決定2) / 値は人によって変わるが、取っていないユーザーはほぼいない。ux-guidelines「入力欄は自動値を上書きする例外操作」に従い、表示は 1 行に畳んで変更は `details` の中へ入れる。**保存済みキャラの値は書き換えない**(既定値を使うのは新規登録の経路だけ) / `draft.ts::defaultEquipment`(旧 `neutralEquipment` から改名)
5. **補正源の並びを 12a の指定順(キャラステータス / 装備 / シエナ / テシスコア / 神鳥の聖物 / クラウン / スキル / モンスターカード / ペット)にした。12a に無いルーン・調整はその後ろ** / 従来はペット・ルーンが上に来ていて、火力への効きが大きい装備・シエナ・テシスコアが下に沈んでいた / `Workspace.svelte` の `sources`
6. **`Select` は選択肢側が空値を持つとき placeholder を出さない** / 「なし」「未装着」と disabled な placeholder が両方 `value=""` になり、なしを選んでいるのに「選択してください」と表示されていた(ペット S スキル・テシスコアのペインにも同じ問題があった) / `ui/Select.svelte`

### 実機確認(docs/screenshots/75〜78)

- 補正源の並びが 12a どおり。新規登録キャラでヘッダに「未設定 8 件」が出る
- 装備ペイン先頭が「装備攻撃力強化 +20% / パワーウェポン ・ ストロングウェポン Lv6」の 1 行(折りたたみ)。新規登録キャラも同じ既定値で始まる
- 部位一覧の武器行が「武器 | †アビスシミター | +12 | アビリティ 2 | 突122 / 斬315」
- 部位詳細は左=基本 / 右=エンチャントの 2 列。エンチャント突き 400/400 に「満」が出て MAX ボタンが無効
- アビリティは系統ごとの Select 4 行。(下)尖った刃 → E-尖った刃 と選び直しても 1 つのまま、鋭い刃を足して合計 2 件
- 「いまの実力」の装備値が 基本 / 強化 × 突き/斬り/魔攻/魔防 の表 +「テシスコア・シエナの分はこの表に入らない」注記

## 2026-08-25 デザイントークン(docs/claude/goals/2026-08-24-mock-alignment.md 段階3)

1. **角丸は 4 段(`--r-window` 12 / `--r-panel` 9 / `--r-inset` 6 / `--r-pill` 999)に固定し、直書きを全廃した**(2026-08-24 決定3) / 2〜13px の 13 種類が混在していた。規格シート 3a の 16px 系列は「親しみを増す」ための探索で v4 が採らなかった方向なので採らない。`border-radius: 50%`(丸ドット)だけは形が違うので残す / `app.css` の `:root`、`.svelte` 全ファイル(79 箇所を置換)
2. **タイポは 4 つの役割トークン(`--t-result` 44 / `--t-heading` 19 / `--t-body` 12.5 / `--t-label` 10.5)を入れ、役割が一致する箇所だけ置き換える**(ユーザー決定 2026-08-25) / 結果値・本文・ラベルは既に実値が規格と一致していた。一方カード見出し(11px)・小ラベル(9〜9.5px)を 19 / 10.5 に寄せると 3 タブすべてでレイアウトが伸び、1 画面に収まる情報量が減る。v4 の密度を捨てる判断は別に必要なので、ここでは **役割の名前を与えるところまで**にした / `app.css` の `--t-*`、`CalcPage:.hero-num`、`HomePage:.sel-name`
3. **19px の見出しはホーム右カラムの「選択中」コンテンツ名(`.sel-name`)に当てた** / 現状ここが「カード群の親」なのに 11px でカード見出しと同格だった。ついでにこの要素が使っていた `#4A4780`(ラベンダー系)をやめた: ラベンダーは「保存されない・一時」専用の色で、保存済みデータの名前に使うのは誤用
4. **インセット面 `--surface-inset: #C1D3E6`(2a の窓 3 層目)を追加し、表・読み取り専用の数値をそこに載せる** / 読み取り専用と編集可(白 = `--bg-field`)の区別が面で付いていなかった。`table.grid` のヘッダ、キャラ画面の「最終能力値」「装備値」「攻撃力の内訳」に適用 / `app.css:.inset`
5. **ラベンダー `#6D6AA8` を `--sim` としてトークン化した(`--sim-strong` / `--sim-fg` / `--sim-bg` も)** / 「保存されない・一時」専用の色。用途が限定された色ほど直書きだと意味が失われる / `App.svelte:.sim-note`、`CalcPage:.panel.purple`/`.chip-diff`/`.chip-x`
6. **`candidates.ts:COST_COLORS` を CSS 変数参照にした** / inline style で使うので `var(--...)` がそのまま通る。ここだけ色の実値が TS 側にあると、テーマを触るときに見落とす / `--good-bg`/`--good-border`/`--danger-bg`/`--danger-border` を追加

### 実機確認(docs/screenshots/79〜81)

3 タブとも崩れなし(page/console エラーなし)。ホームは「リンゴ」が 19px の大見出しになり、カード群との親子が付いた。キャラ画面の「最終能力値」「装備値」がインセット面になり、白い入力欄との区別が面で付く。
インセット面の内側余白が増えた分だけ「いまの実力」の 5 枚(4 カード + 削除ボタン)が 1 行に収まらなくなったので、`.sheet-card` の flex-basis を 240px → 210px に詰めた。

## 2026-08-25 アイコン規格(docs/claude/goals/2026-08-24-mock-alignment.md 段階4)

1. **アイコンは `ui/Icon.svelte` 1 部品に集約し、4 系統(キャラ / Mob / スキル / バフ)を枠の色と角丸で識別する**(規格シート 3a) / キャラ `#798CAC`・Mob `#A98B86` は `--r-window`、スキル `#C2A057` は `--r-panel`、バフは `--sim` の円形。3a の r12 / r10 は段階3 の 4 段トークンに丸めた(12 / 9) / `apps/desktop/src/ui/Icon.svelte`
2. **画像は `src/assets/icons/<系統>/<id>.png` を gamedata の id から機械的に解決する。手動のマッピング表は作らない** / Vite の `import.meta.glob` で解決するので、ファイルを置くだけで反映される。マッピング表を作ると gamedata を増やすたびに 2 箇所直すことになる / `Icon.svelte` の `FILES`、`src/assets/icons/README.md`
3. **解決できない id は破線 + `?`。空白にしない。console に warn 1 行(id ごとに 1 回だけ)** / 空白だと「アイコンが無い」のか「対象が無い」のか区別できない。warn を毎描画出すとログが埋まるので `Set` で重複を止める / `Icon.svelte` の `warned`
4. **`id === null`(そもそも対象が無い枠)は縞プレースホルダのままにし、warn も `?` も出さない** / 敵データ未収録のコンテンツ行がこれに当たる。「データが無い(`?`)」と「対象が無い(縞)」は別の状態 / `HomePage` の Mob アイコン(`content.enemy_id`)
5. **サイズは 20 / 28 / 40 / 64 の 4 段を型(`IconSize`)で縛り、CSS 側で固定する** / 画像 0 枚でもレイアウトが動かないことがこの段階の受け入れ条件 / `Icon.svelte` の `--icon-size`
6. **アイコン単独表示は禁止(名前と併記)。例外はキャラレールを畳んだときだけで、そのとき呼び出し側が title を付ける** / 実画像が無い状態で単独表示すると `?` だけが並んで何も分からない / `CharacterRail.svelte`(畳んだとき size 40 + ボタンの title)

適用先: キャラレール(名前 1 文字だった枠)/ ホームの到達一覧の行(空の四角だった枠、Mob)/ 登録ペインの 19 職の選択カード(キャラ 40)/ 計算タブのスキル行・スキル一覧(スキル 20)/ 選択中バフの詳細(バフ 20)。

### 実機確認(docs/screenshots/82〜85)

実画像 0 枚で 3 タブとも崩れなし(page/console エラーなし)。ホームで表示中のアイコン 37 個のうち 20 個が破線 `?`、17 個(敵データ未収録のコンテンツ)は縞のまま。console の warn は 68 行 = 表示された id の種類数と一致(重複なし)。レールを畳むとキャラアイコン 40px + クリア数だけになり、title でどのキャラか分かる。

## 2026-08-25 上限ロスと防御側パネル(docs/claude/goals/2026-08-24-mock-alignment.md 段階5)

1. **上限で捨てられた分は `CategoryTrace` の `raw − value` で出す。`CategoryTotals` に新しい持ち物は足さない**(goal の当初案からの変更) / `CategoryTotals` は既にキャップ適用前の Σ を `values` に持ち、`trace()` が `raw` と `value` を両方返していた。捨てた量を別に持たせるのは同じ情報の二重管理になる。フロントの型定義に `raw` が無かっただけなので、そこを足した / `CalcPage:catLoss`、`api/types.ts:CategoryTrace.raw`
2. **上限に届いた枠は「満」バッジに加えて `生値 → 上限値 / N は無効` の行を出す** / 「満」だけでは、あと少しで上限なのか大幅に無駄なのかが分からない。積んだのに効いていない量が数値で見えないと、次に何を伸ばすべきか決められない / `CalcPage:.capped`
3. **防御側は `crates/domain/src/defense.rs` に置き、与ダメージ式(`damage.rs`)とは別経路にする** / 防御力・カット率・回避は「自分がどれだけ耐えるか」で、攻撃力(A)と違って与ダメージ式には入らない。同じモジュールに混ぜると `DamageInput` に無関係な入力が増える / `domain::defense_profile`、`preview_defense` コマンド
4. **未実装で値を出せない項目は `Option::None` にして UI が破線 +「未実装」で示す。0 で埋めない** / 「防御力 0」と「まだ計算していない」が区別できないと画面が嘘をつく。対象は **装備物防**(`EquipmentValues` が持たない)・**通常回避**(算出式が wiki 未取込)・**最終被弾率**(通常回避が出せないので合成不能)/ `DefenseProfile` の `Option` 3 つ、`DefensePanel:.na`
5. **装備防御力倍率は 100% 固定** / wiki §6「初期 100%。リンゴの島・ベリネンルミでは常に 100%」。コートアーマー等の増加は未収録なので倍率を入力させない / `EQUIPMENT_DEFENSE_RATE`
6. **複合カット率の `a` は `3 + [(DEF+装備物防+MR+装備魔防−1)/20]` と読んだ `[仮]`** / wiki のカテゴリJ 欄は物理・魔法を `3 + [...]` と書いたあと複合だけ `(...)/20` と書いており、`3 +` が掛かるか読み取れない。物理・魔法と同形と解釈した / `defense.rs::cut_rate_a`
7. **攻撃 / 防御は同列タブにする**(規格シート 5c) / 防御を攻撃の中の折りたたみにすると「おまけ」に見える。実際は別軸の情報なので同格に置く / `CalcPage:.side-tabs`
8. **未実装の補正源(称号・等級・ランダムOP・属性値)が中立値で計算されていることを計算画面に明記した** / goal スコープ外の項目でも「黙って 0」は嘘になる。段階5 の検討事項として 1 行の注記で解決した / `CalcPage:.mat-note`

### 実機確認(docs/screenshots/86〜87)

- 防御タブ: 物理 645 / 魔法 807(MR×3 + 装備魔防 35×6)/ 複合 726、カット率 物理 76.9% / 魔法 75.5% / 複合 76.2%、特殊回避 49.8%。装備物防・通常回避・最終被弾率が破線「未実装」で 0 と区別できる
- 上限ロス: マーキュリアル洞窟とルビコナに進化0強化4 のコアを 6 枠ずつ入れると K が 800 + 800 = 1,600 になり、「最終ダメージ(固定値) 1,600 → 上限 1,000 / 600 は無効」と出る(確認後、コアは未装着へ戻した)

## 2026-08-25 小粒 3 件(docs/claude/goals/2026-08-24-mock-alignment.md 段階6)

1. **スキルピッカーは対象への合計ダメージの降順に並べる**(v4 指定) / `skillTotals` は既に計算していて、登録順のままにしていただけ。合計が未取得のもの(取得中)は末尾に置く / `CalcPage:pickerSkills`
2. **段数違いのコンテンツは `ContentSeries` としてドメインに持たせ、一覧は 1 行 + 難易度ステッパーに畳む** / レリックの聖域が 10 行あって一覧のノイズになっていた。UI 側で「名前の接頭辞が同じ行をまとめる」と、名前を変えた瞬間に壊れる / `domain::ContentSeries`、`Content::series`
3. **系列は id の接頭辞 + 末尾の数値で機械的に決める。系列の一覧表は作らない** / `relic_sanctuary_10`〜`_19` は数値で拾い、`relic_sanctuary_shinchou` / `_kisinik`(別コンテンツ)は数値でないので系列に入らない。段を足しても gamedata の 1 箇所だけで済む / `contents.rs::series_of`、`レリックの聖域は10段から19段の系列になる` テスト
4. **バフの 3 状態は「保存済みかどうか」から導出する。バフ選択にスコープ用のフィールドを足さない** / `常時(マイセット)` = キャラに保存済み、`追加` = 試し変更にだけある、`使わない` = どちらにも無い。これで DB migration も新しい状態管理も要らない / `CalcPage:buffState`
5. **常時への昇格はチップのクリックではなく保存操作(「キャラに保存」)で行う** / チップを押した瞬間に DB を書くのは、他の「押した瞬間に数字が動く」チップと同じ見た目で挙動だけ違い、取り消しにくい。3 状態は**表示**で解決し、遷移は既存の保存導線に寄せた。`常時 → 使わない` も同様に、外した状態を保存して初めて確定する

**当初案との差**: goal は「3 状態化(常時 → 追加枠 → 使わない)」をクリックで巡回させる書き方だったが、`追加 → 常時` は保存を伴うため巡回に混ぜなかった。解決したかった「常用セットが保存操作でしか表現されない」は、チップに 常時 / 追加 のバッジを出すことで解消している。

### 実機確認(docs/screenshots/88〜90)

- スキルピッカー: 極・残影斬 924,286 → 極・連 789,580 → 極・横斬り 321,344 → 極・縦斬り 319,838 → 極・アイスブレイク 311,221 の降順
- レリックの聖域: 25 行あったエリアが 16 行になり、`◀ 難易度 10 / 19 ▶` で段を切り替えると目安・倍率が追従する(13 段で目安 9,000 / ×34.6)。畳んだエリアの「全部クリア可 — …」も代表 1 行の名前だけになる
- バフ: 保存済み 6 件に「常時」、試し変更で足した 1 件に「追加」バッジ。凡例が件数付きで出る

## 2026-08-25 命中・回避の取込(docs/claude/goals/2026-08-25-wiki-gaps.md 段階1)

出典: wiki 計算式まとめ `#HitRate` / `#AccuracyPoint` / `#EvasionPoint` / `#HitRateCap`、ステータス「命中率/回避率」、狩り場情報一覧(いずれも取得 2026-08-25)。

1. **防御タブに出すのは「通常回避率」ではなく「回避P」(物理/魔法/複合)** / wiki の通常回避は率ではなく `命中率 = 敵命中P − 回避P` の裏返しで、能力値だけでは率が決まらない。能力値から確定するのは回避P までなので、そこを実値で出す。率が要るコンテンツ側の入力(上限回避P)は wiki が全行未記載 / `defense.rs::evasion_point`、`回避Pは15足すAGI1_2倍足す攻撃タイプ別増加` テスト
2. **通常回避の上限 85%(= 命中率下限 15%)と、上限回避時の最終被弾率 `(1 − 85%) × (1 − 特殊回避)` を実値で出す** / wiki ステータスの例(`85% + コンボ50% → 92.5%`)と同じ合成。「敵命中P ≦ 回避P+15 で上限、回避P+100 以上で必中」を注記に置き、上限を取れているかはユーザーが判断できるようにした / `hit_taken_rate_at_cap`、`上限回避時の最終被弾率は特殊回避と合成する` テスト
3. **`normal_evasion(回避P, 敵命中P)` は汎用形で domain に置き、コンテンツ側の敵命中Pが入った時点で画面に繋ぐ** / 式は wiki から確定しているので実装する。入力が無いだけ。段階5(狩り場情報一覧)で敵データを持たせるときに接続する / `通常回避は敵命中Pとの差で下限0上限85` テスト
4. **コンテンツごとの上限回避Pは `Enemy` に持たせない** / wiki 狩り場情報一覧の「上限回避P」列は全行 `?`(未記載)。全件 `None` のフィールドを先に足しても情報が増えない。必中命中P(攻撃側)は値があるが、これは敵の回避Pであって敵の命中Pではないので通常回避には使えない
5. **装備回避率・装備敏捷度・命中P増加バフは 0 として計算し、下振れであることを画面に明示する** / `EquipmentValues` は 4 値(突き/斬り/魔攻/魔防)しか持たない。装備の防御側 9 値化は段階2 のスコープ / DefensePanel の注記(装備物防と同じ扱い)
6. **命中P(攻撃側)は実装しない** / 式は取り込んだが、`装備命中率補正` と `スキル命中` を持つデータが無い(装備 4 値・スキルは倍率/段数/Cri倍率のみ)。片方欠けた命中Pを出すと必中判定を誤らせる。装備 9 値化(段階2)とスキル取込(段階7)の後に入れる

### 実機確認(docs/screenshots/91)

- 防御タブ: 回避P 物理 316 / 魔法 310 / 複合 312(DEF215・MR199・AGI199)、通常回避(上限)85%、特殊回避 49.8%、最終被弾率 7.5%(= 0.15 × (1 − 0.498))。破線「未実装」は装備物防の 1 件だけになった

## 2026-08-25 装備の防御側 9 値化(docs/claude/goals/2026-08-25-wiki-gaps.md 段階2)

出典: wiki 装備システム / 装備システム/エンチャント / Item/武器/刀・太刀 / Item/防具/兜・鎧/軽鎧・腕/シールド・腕/盾＋ / Item/アクセサリ/顔・体・手・足 / ステータス#Defense(取得 2026-08-25)。

1. **`EquipmentValues` を 4 値 → 9 値(突き/斬り/物防/魔攻/魔防/命中/Cri補正/回避/敏捷)にする** / wiki Item 各ページの表がこの 9 列で、1 行から全部取れる。エンチャント呪文書も同じ 9 種(+4〜6 が攻撃/防御 5 種、+2〜3 が命中/クリ/回避/敏捷)。並び順は wiki の列順をそのまま `EquipmentValues::fields()` に持たせ、検証・UI ラベル・合計表の唯一の並びにした / `装備値の値域は9種すべてを検証する`、カタログ 20 件の値
2. **`SupportValues` を廃止して `EquipmentValues` に合流する** / 9 値になった時点で「持ち場が無い補正」が消えたので、型を 2 つ持つ理由が無くなった。テシスコアの補助タイプ(物防/回避/敏捷/命中)は強化能力値へ合流し、与ダメージ式の装備係数がこの 4 種で 0 なので攻撃力には効かず、防御力・カット率・回避Pにだけ効く / `thesis_core` の混在テスト、`cargo test --workspace`
3. **`defense_profile` は装備補正 9 値の合計を受け取る** / 引数が `equipment_magic_defense: i64` だと物防・回避率・敏捷度を足すたびに増える。合計 1 個を渡す形にした / `commands::preview_defense`、`防御力はステ3倍と装備防御6倍`
4. **カット率 J の `a` に足す装備防御は倍率を掛けない生の値** / wiki カテゴリJ は `a = 3 + [(DEF+装備物防−1)/10]` で倍率の記載が無い(防御力の式にだけ倍率がある)。魔法側の既存実装と揃えた / `カット率の装備防御は生の値で足す`
5. **`EQUIPMENT_VALUE_MAX = 9999` の `[仮]` は外さない** / wiki は装備ごとの「上限」行(= エンチャント上限。カタログの `enchant_caps` に収録済み)しか持たず、全装備共通の上限は未記載。実データの最大は 400(アビス武器の突き/斬り)、255・310・320 の混在もあり共通値が導けない。9999 はカタログ外のカスタム入力に掛ける安全域として残す
6. **装備防御力倍率(コートアーマー等)は実装しない** / 倍率の増加手段はすべて共通スキル/バフで、共通スキル自体が補正源として未実装(`StatSources` はペットS/ルーン/クラウン/神鳥/常用バフ/調整のみ)。バフカタログは `BuffTarget` が `StatKind` 単位で、装備防御力倍率という対象を持たない。値は docs/damage-formula.md §6 に表で記録し、共通スキルを補正源に足すときに実装する。現状は 100% 固定(wiki の初期値、リンゴの島・ベリネンルミでは常にこの値)
7. **刀のアクィルスとアビスで Cri補正 と 回避 が入れ替わって見えるのは wiki の記載どおりに転記する** / アクィルス Cri27-30 / 回避30-31、アビス Cri30-31 / 回避27-30。同じ列順のヘッダの下でこうなっている。実測で確認できるまで書き換えない `[仮]`

### 実機確認(docs/screenshots/93〜94)

- 装備ペイン: 鎧に †アビスアーマー を選ぶとエンチャント欄が 9 行になり、基本欄に wiki レンジ(物防 260–280 / 魔防 230–260 / 回避 100–120)、エンチャント上限が 物防 310 / 魔防 290 / 回避 150 と出る。値が 0 の 6 種は `満` 表示のまま
- 部位行・候補行の要約を「値が大きい上位 2 種」に変えた(鎧が `突0-0 / 斬0-0` と読めていたのが `物防270 / 魔防245` になる)
- 防御タブ: 物理防御力 645 → 2,265(DEF215×3 + 装備物防 270×6)、魔法 2,277、複合 2,271、カット率 物理 61.1% / 魔法 61.5%、回避P 複合 444(装備回避率 110×1.2 を含む)。破線「未実装」は 0 件
- 確認後、検証ボリスの鎧は未装備に戻した

## 2026-08-25 属性システム(docs/claude/goals/2026-08-25-wiki-gaps.md 段階3)

出典: wiki 属性システム / 装備システム/属性強化 / 狩り場情報一覧(取得 2026-08-25)。docs/damage-formula.md §8。

1. **属性は `Element`(8 種)+ `ElementValues`(属性ごとの値)として domain に置く** / 属性値は「キャラの基礎値」「装備の付与分」「その合計」の 3 か所で同じ形をしているので型を 1 つにした。合計は 255 で頭打ち(wiki 属性システム「属性値の上限は255です」)/ `属性値は属性ごとに足して255で頭打ち`
2. **装備の属性は部位ごとに 1 属性・0〜9。盾+ とレリックは対象外、無属性は付与不可** / wiki 装備システム/属性強化「1属性のみ装着可能(火、水、風、土、雷、白、黒)」+ 装備システム冒頭の表の「属性強化」行。上限 9 は費用表(0→7 / 0→8 / 0→9)の最大 / `装備の属性は部位ごとに1属性で合計される`、`属性の値域と部位制約`
3. **キャラの基礎属性値は wiki の各キャラ表で属性名に付く括弧の数値** / 例: ボリス「水属性(10)」「雷属性(5)」。括弧が無い属性は 0(スキルは載っているが数値が無い行がある)。ロアミニ・ノクターン・リーチェ・イェフネンは表そのものが無いので全属性 0 `[仮]` / `gamedata::element_base`
4. **スキルの属性は読み取れないものを `None` にする(無属性で埋めない)** / ボリスの無属性行は「共通系、剣系、刀系(縦斬り、連、円)」で、刀系の横斬り・残影斬はどの属性の行にも無い。無属性と決め打つと属性差ボーナスが乗ってしまう。`None` はカテゴリI = 1.0 / `gamedata::skills` の element 列
5. **`Enemy::element_threshold` の `[仮]` を外す** / wiki 狩り場情報一覧が「敵属性値: 敵に設定されている属性値。攻撃スキルの属性値が上回っていれば与ダメージが増加する」と定義していて、旧リポ由来の 120 / 125 / 90 と一致する。意味が確定したので `[仮]` 不要
6. **属性値の供給源が足りない件は `[仮]` として残す** / モデル化できる範囲(基礎 ≤10 + 属性強化 9 × 10 部位)では攻撃側の属性値が最大 100 で、敵属性値 120 / 125 を上回れない。wiki は上限 255・差 +80 で 1.5 倍と書いているので、属性強化に 9 より上があるか未取込の供給源がある。勝手に埋めず docs/damage-formula.md §8 に未解決として記録した

### 実機確認(docs/screenshots/95〜96)

- 装備の 10 部位(武器/鎧/兜/盾/頭/体/手/足/効果/AF)に 水9 を付与 → 部位行に「水9」バッジ、装備値カードに「属性(装備の付与分): 水90」
- 極・アイスブレイク(水属性)× トゥタトゥール(敵属性値 90)で「属性差ボーナス +6%」= floor((10+90−90)×0.625)/100。カテゴリI が 1.0 以外になった
- 確認後、検証ボリスの属性は全部位「属性なし」に戻した

## 2026-08-25 覚醒・エタの意志の上限(docs/claude/goals/2026-08-25-wiki-gaps.md 段階4)

出典: wiki Quest/覚醒クエスト / エタの意志(Ver8.14 の表、取得 2026-08-25)。docs/damage-formula.md §10。

1. **覚醒倍率の `[仮]` を外す** / wiki「極限後の各値の増加効果」の二次極限+N次覚醒(+20/+40/+60/+100%)が段階 2〜5 の 1.2/1.4/1.6/2.0 と一致し、エタの成長の覚醒ダメージ列から `1.0 + %/100` で作った Lv0〜80 が旧リポ `awakening.json` と**完全一致**した。出典を wiki に差し替え、表を Lv100(MAX)まで伸ばした / `極限はエタの意志lvごとの表を引く`(Lv100 = 2.59 を追加)
2. **エタの意志 Lv の上限を 80 → 100 にする** / wiki の表が Lv100(MAX)まである。`Awakening::MAX_ETERNAL_LEVEL` と UI の選択肢を `StatLimits` 経由で揃えた
3. **ダメージ上限は 1 段あたりに適用する** / wiki Quest/覚醒クエスト「ダメージ上限は多段スキルでも1段ごとに適用」。合計は 上限 × 段数になる / `ダメージ上限は1段ごとに適用され捨てられた分を残す`
4. **上限で捨てられた分は `DamageResult::capped_loss` として残し、0 と区別する** / 段階5(mock-alignment)の「上限で捨てられた分」と同じ出し方に揃えた。計算画面に `生値 → 上限 / N は無効` で出る
5. **防御力の上限も同じ形で入れる** / `DefenseProfile::defense_cap` + 物理/魔法/複合それぞれの `_loss`。防御タブに「防御力の上限」行を出し、当たっていれば注記で捨てた分を出す / `防御力は上限で頭打ちになり捨てられた分を残す`
6. **覚醒段階 0〜4 の上限は wiki の進行どおりの行を引く** `[仮]` / 倍率のほうは段階 2〜5 が**二次極限済み**前提(wiki の「二次極限+N次覚醒」行)なのに、上限のほうは wiki が段階の進行(初期値 → 1次 → … → 一次極限 → 二次極限)でしか書いておらず、「二次極限+2次覚醒」の上限が無い。前提が食い違うが、wiki に無い組み合わせを作らない方を採った
7. **最終能力値の上限 2,400 の根拠が確定した** / エタの意志 Lv100 の「最大ステータス開放」。素の上限は 1,500。`ADJUSTMENT_PIN_MAX = 2400` は理論上限として正しい。**能力値そのものへの上限適用は入れていない** — `effective_stats` は覚醒段階を知らず、渡すには `stat_sources` の全経路に覚醒を通す必要がある。現状のキャラは 1,000 前後で上限に遠く、実害が無いので別途とする `[仮]`

### 実機確認(docs/screenshots/97〜98)

- 計算タブ: 検証ボリス(覚醒 0)で「ダメージ上限(1 段ごと) 321,344 → 上限 7,000 / 314,344 は無効」。ヒーローの数字も 7,000 になる
- 防御タブ: 「防御力の上限 4,300」(覚醒 0)。防御力 645 / 807 / 726 は上限に当たっていないので注記は出ない

## 2026-08-25 敵データの出典整理(docs/claude/goals/2026-08-25-wiki-gaps.md 段階5)

出典: wiki 狩り場情報一覧(取得 2026-08-25)。docs/damage-formula.md §11。

1. **敵 42 体を wiki 再取得版と再照合し、全件一致を確認した** / 2026-08-24 の収録時と同じ結果(最後の決戦2 のカット率のみ実測表 70% を採用)。`ENEMIES_SOURCE` の出典順を wiki 先頭に変え、取得日を 2026-08-25 にした
2. **目安ダメージ(`need_per_hit`)の `[仮]` は「コミュニティ知識」に読み替える** / wiki 狩り場情報一覧に目安ダメージの列は無く、再取得しても埋まらない。`[仮]` は「wiki で裏が取れたら外す」印なので、埋まらない項目に付け続けると未解決に見え続ける。UI の文言も「仮値」→「wiki に無い値です(コミュニティ知識・実測)」に変えた / `contents.rs` のモジュールコメント、ホーム画面・キャラレールの注記
3. **wiki にあって未消費の列を docs に一覧化した** / 必中命中P・敵AGI・クリティカル被撃率・被ダメージ倍率 I/II/XXX・カット率B。どれも消費側(プレイヤーの命中P・クリティカル率・敵ごとの倍率枠)が無いのでフィールドを先に足さない
4. **wiki にしか無い敵は今回も収録しない** / 空虚の領域・次元の隙間・ヴェスティージ一般・プシーキーの虚像などはデータが揃っているが、収録するとコンテンツ側(入場条件・目安)の追加が要る。別 goal
5. **武器ダメージ無効(キマイラ)は `[仮]` のまま** / wiki 狩り場情報一覧に該当する列が無い

## 2026-08-25 クリティカルまわりの `[仮]` 解消(docs/claude/goals/2026-08-25-wiki-gaps.md 段階6)

出典: wiki 計算式まとめ `#CriticalChance` / ステータス「ダメージ増加/減少カテゴリー」(取得 2026-08-25)。docs/damage-formula.md §9。

1. **非クリティカル時に `{F×G}` ごと 1.0 とする `[仮]` を外す** / wiki ステータスの [G] クリティカルダメージ増加の供給源は スコープアイ / 致命のルーン / ソウルリンク / 称号 / プシーキーの刻印 で、すべて名称どおりクリティカル時のダメージを増やすもの。F(Cri倍率)も代入でクリティカル時にだけ入り、wiki の式が `{F×G}` と 1 つの中カッコにまとめている。非クリの一撃に G だけ乗る根拠が無い / `damage.rs::evaluate` のコメント
2. **複合カット率の `[仮]` を外す(U4 解消)** / wiki ステータス `#DamageReductionPlayer` に「複合 : a = 3 + [(DEF+装備DEF+MR+装備MR-1)/20]」と `3 +` 込みで明記されていた。実装は最初からこの形なので値は変わらない。防御タブの `[仮]` バッジを外した
3. **クリティカル率は式だけ docs に記録して実装しない** / 式に要る「対象のAGI」「対象のクリティカル被撃率」は狩り場情報一覧の多くの行が `?`、「スキルクリティカル率」はスキルデータそのものが無い(段階7)。装備クリティカル補正は段階2 で入ったので、残りが揃えば出せる
