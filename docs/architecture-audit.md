# アーキテクチャ監査(2026-09-02)

docs/architecture.md の「UI は表示と入力のみ。計算・判定は必ず Rust 側」と AGENTS.md の原則
(最もシンプルな実装・投機的な間接化をしない・個別バフをコードで分岐しない)に照らして、
フロント(apps/desktop/src)と domain / commands / storage を読んだ結果。行番号は監査時点のもの。

対処したら該当行を消す。全部消えたらこのファイルも消す。

## A. フロントに残っているドメインロジック

### A1. Rust に対応物がなく、フロントだけが規則を持っているもの

10 件すべて済み(2026-09-02)。装備可否 / アビリティ適合 / エンチャントプラン / レリック段 / レバー /
到達 4 段 / 称号属性 / 共通スキル既定 / 覚醒正規化 / スキル並びは domain・gamedata に置き、コマンドで配る。

### A2. Rust に同じ計算があり、TS が写経しているもの(二重化)

7 件すべて済み(2026-09-02)。ランダム OP の候補列挙 / 神鳥の聖物と シエナの段階表 / カタログ品の適用と
強化 Lv の等級 / アビリティ枠の置換・枠超過 / アビリティの並びと等級 / enum の並びと分類 /
装備画像の id と実測の逆算可否は domain・gamedata に置き、コマンドか `StatLimits` で配る。
TS に残るのは表示ラベルと、返ってきた集合での絞り込みだけ。

確認して問題なし: `state.svelte.ts`、`api/transfer.ts`、`api/browserStore.ts`、`candidates.ts`、`format.ts`、`limits.svelte.ts`、`TracePanel.svelte`、`MeasurePage.svelte`、`ui/critChance.ts`、ActualDelay / CriticalRate / SoulLink の各ペイン。

## B. domain / commands / storage で無理をしている箇所

21 件のうち見送り 3 件を除きすべて済み(2026-09-02)。補正パイプラインは `build_stat_modifiers` 1 本、
`DamageInput` は `DamageMaterial` + `DamageTarget`、的中剣は `SkillEffect::AccuracyRate` のデータ、ステ別 7 値は
`PerStat<T>`、装備補正 9 値は `EquipmentStatKind`、`StatLimits` は数値だけ(表は `GameTables`)、保存前検証は
`NewCharacter::validate`、伸びしろは源ごとの列挙 API(`stat_fixed_rooms` / `accuracy_buff_rooms` /
`stat_buff_rooms` / `ability_value_rooms` / `random_option_rooms`)を `GrowthRoom` が費用順に並べる。

見送り(理由つき):
- B17 `RecordOnly` バフのダミー `target/layer`: 型で分けると `BuffDefinition` を読む画面(バフタブ)の分岐が全部変わる。5 件のためにその範囲を動かす価値が無い。注記だけ直した
- B18 `Skill::power` / `power_per_second` の保存: 画面の並び・表示が使う派生値で、TS に再計算させないために持たせている(A1-10 の並びも Rust 側)。`effective_*` はコンボ変種を解決したあとの値で入力の写しではない
- B4 のうち `BaseStats` / `EffectiveStats` / `ElementValues`: 構造体リテラルが多く(テスト 60 か所)、値のままのほうが読みやすい

問題なし: `category.rs`(enum + kind/cap/label の表でデータ駆動)、`rounding.rs`、`stats.rs::effective_stat`、`attack_power.rs`、`actual_delay.rs`、`critical_rate.rs`、`common_skill.rs`、`ultimate_skill.rs`、`soul_link.rs`、`title.rs`、`thesis_core.rs`、`siena.rs`(`SienaEffect` で効き先を 1 か所に集約)、`candidate.rs::rank_candidates`、`content.rs::ContentRequirement::check`。domain / commands にキャラ名・バフ id の文字列比較は的中剣以外 0 件。

## C. 着手順

全件消化済み。残す理由のある見送りは §B 末尾。行番号は監査時点のもので現状とは合わない。
