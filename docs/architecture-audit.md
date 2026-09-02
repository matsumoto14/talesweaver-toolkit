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

| # | 場所 | 症状 | 簡単な形 |
|---|---|---|---|
| 21 | `defense.rs:517-744 accuracy_growth / evasion_growth`、`stat_sources.rs:741 buff_accuracy_point_room` | 伸びしろの材料が「合成後の `stat_cap` との差 1 本」「未選択バフの単純合計(排他枠を見ない)」「エンチャント枠」だけで、**源ごと**(ペット S・ルーン・クラウン・カード・聖物 / DEX 増加バフ / 装備アビリティの命中の空き枠・上限未満 / ランダム OP の命中P の空き枠・ランク上げ)には分解できない。文言(label / detail)も domain が文字列で組む | 「源 → 現在値 → 上限 → 積んだ後の値」を返す列挙 API を `stat_sources` / `equipment` / `random_option` に置き、`GrowthRoom` はそれを費用順に並べるだけにする。文言は画面側([versus-next-actions.md](versus-next-actions.md)) |

見送り(理由つき):
- B17 `RecordOnly` バフのダミー `target/layer`: 型で分けると `BuffDefinition` を読む画面(バフタブ)の分岐が全部変わる。5 件のためにその範囲を動かす価値が無い。注記だけ直した
- B18 `Skill::power` / `power_per_second` の保存: 画面の並び・表示が使う派生値で、TS に再計算させないために持たせている(A1-10 の並びも Rust 側)。`effective_*` はコンボ変種を解決したあとの値で入力の写しではない
- B4 のうち `BaseStats` / `EffectiveStats` / `ElementValues`: 構造体リテラルが多く(テスト 60 か所)、値のままのほうが読みやすい

問題なし: `category.rs`(enum + kind/cap/label の表でデータ駆動)、`rounding.rs`、`stats.rs::effective_stat`、`attack_power.rs`、`actual_delay.rs`、`critical_rate.rs`、`common_skill.rs`、`ultimate_skill.rs`、`soul_link.rs`、`title.rs`、`thesis_core.rs`、`siena.rs`(`SienaEffect` で効き先を 1 か所に集約)、`candidate.rs::rank_candidates`、`content.rs::ContentRequirement::check`。domain / commands にキャラ名・バフ id の文字列比較は的中剣以外 0 件。

## C. 着手順

1. B1(補正パイプライン一本化 → `build_stat_modifiers`)と B2(`DamageInput` → `DamageMaterial` + `DamageTarget`)は済み。フロント写経を Rust に移す受け皿になる。
2. **A1**(Rust に無い規則)を domain / gamedata に新設し、コマンドで配る。到達 4 段・装備可否・エンチャントプランは `ContentEvaluation` と装備検証の拡張で収まる。
3. **A2**(写経)も済み。Rust から値・上限・候補を返して TS を削除した。
4. **B8**(`StatLimits` 分割)は済み。並び・ラベル・部位ルール・段階表は `get_game_tables`(`domain::GameTables`)に移し、`StatLimits` は数値だけになった。B3(的中剣のデータ化)と B4(`PerStat`)も済み。
5. 残り(B5〜B7、B9〜B20)は触った箇所から順に。
6. **B21**(伸びしろの材料を源ごとに列挙する API)。対人タブの「次にできること」([versus-next-actions.md](versus-next-actions.md))の受け皿で、A1-2 / A1-16(アビリティ枠の規則)を Rust に移した結果を使う。
