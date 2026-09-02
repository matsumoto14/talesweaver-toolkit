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
| 4 | `equipment.rs:360-414`(SienaStatBonus)、`stats.rs`(BaseStats / EffectiveStats)、`element.rs` | ステ別 7 フィールド構造体の手書き `get/get_mut`。stat_sources.rs 側の 6 型は `PerStat<T>`(stats.rs)にした(済み) | 残りも `PerStat<T>` に寄せる。`BaseStats` / `EffectiveStats` はテストの構造体リテラルが多いので値のまま可 |
| 5 | `equipment.rs:98-166, 1634-1650, 1690-1759`、`thesis_core.rs:260-277`、`siena.rs:231-255`、`candidate.rs:152-188`、`commands/lib.rs:1450-1474` | 装備補正 9 値のフィールド対応表が 7 通り。文字列キー `"thrust"` が domain → commands → TS まで走る | `EquipmentValueKind` を 9 種にし `EquipmentValues::get/get_mut(kind)` を 1 か所。候補 id は `(PartSlot, EquipmentValueKind)` |
| 6 | `commands/lib.rs` `preview_defense` / `preview_versus` | 2 値のためにフルプレビュー(`list_upgrade_candidates` / `list_enchant_gains` の同文と `preview_effective_stats` の 4 回複写は `CandidateContext` / `stat_preview_of` で解消済み) | 必要な値だけ出す軽い domain 関数に |
| 8 | `stat_sources.rs:1929-2080, 2097-2213` | `StatLimits` が 150 項目 × 2 リスト。ラベル・部位ルール(`equipment.rs:336-355` は `PartSlot` メソッドの写し)は上限ではなくカタログ | ラベル・部位ルールは `list_*` 系へ。`StatLimits` は数値上限だけ |
| 11 | `random_option.rs:230-244`、`candidate.rs:58, 74, 91, 344`、`skill.rs:166` | 裸の `as i64` / `.round() as` / `.floor() as`(stat_sources.rs 側は `trunc_int` に寄せた) | `trunc_int` / `round_int` に寄せる |
| 17 | `gamedata/src/buffs.rs:290-293`、`stat_sources.rs:1005` | `RecordOnly` バフにダミーの `target/layer`、マスタリー分離後の注記残骸 | 型で分ける・注記削除 |
| 18 | `skill.rs:130-175`、`damage.rs:1031-1032` | `power` / `power_per_second` を保存して再計算、`effective_*` は入力の写し | 片方を消す |
| 21 | `defense.rs:517-744 accuracy_growth / evasion_growth`、`stat_sources.rs:741 buff_accuracy_point_room` | 伸びしろの材料が「合成後の `stat_cap` との差 1 本」「未選択バフの単純合計(排他枠を見ない)」「エンチャント枠」だけで、**源ごと**(ペット S・ルーン・クラウン・カード・聖物 / DEX 増加バフ / 装備アビリティの命中の空き枠・上限未満 / ランダム OP の命中P の空き枠・ランク上げ)には分解できない。文言(label / detail)も domain が文字列で組む | 「源 → 現在値 → 上限 → 積んだ後の値」を返す列挙 API を `stat_sources` / `equipment` / `random_option` に置き、`GrowthRoom` はそれを費用順に並べるだけにする。文言は画面側([versus-next-actions.md](versus-next-actions.md)) |

問題なし: `category.rs`(enum + kind/cap/label の表でデータ駆動)、`rounding.rs`、`stats.rs::effective_stat`、`attack_power.rs`、`actual_delay.rs`、`critical_rate.rs`、`common_skill.rs`、`ultimate_skill.rs`、`soul_link.rs`、`title.rs`、`thesis_core.rs`、`siena.rs`(`SienaEffect` で効き先を 1 か所に集約)、`candidate.rs::rank_candidates`、`content.rs::ContentRequirement::check`。domain / commands にキャラ名・バフ id の文字列比較は的中剣以外 0 件。

## C. 着手順

1. B1(補正パイプライン一本化 → `build_stat_modifiers`)と B2(`DamageInput` → `DamageMaterial` + `DamageTarget`)は済み。フロント写経を Rust に移す受け皿になる。
2. **A1**(Rust に無い規則)を domain / gamedata に新設し、コマンドで配る。到達 4 段・装備可否・エンチャントプランは `ContentEvaluation` と装備検証の拡張で収まる。
3. **A2**(写経)も済み。Rust から値・上限・候補を返して TS を削除した。
4. **B8**(`StatLimits` 分割)は独立して進められる。B3(的中剣のデータ化)と B4(`PerStat`)は済み。
5. 残り(B5〜B7、B9〜B20)は触った箇所から順に。
6. **B21**(伸びしろの材料を源ごとに列挙する API)。対人タブの「次にできること」([versus-next-actions.md](versus-next-actions.md))の受け皿で、A1-2 / A1-16(アビリティ枠の規則)を Rust に移した結果を使う。
