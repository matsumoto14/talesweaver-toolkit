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

Rust から値・上限・候補を返す形にして TS 側を消す。

| # | TS | Rust | 備考 |
|---|---|---|---|
| 12 | `equipment.ts:273-291`、`RandomOptionPane.svelte:105-119` | `random_option.rs:244-266`、`equipment.rs:1313` | ランダム OP の物理/魔法発動条件、同カテゴリ 1 部位 1 つ |
| 13 | `equipment.ts:140-225`、`HomePage.svelte:633-661` | `stat_sources.rs:255-265`、`siena.rs:476-487`、`random_option.rs:137` | 神鳥の聖物 段階↔値、シエナ段階→枠数、OP 既定値。コメントで「ミラー」と自認 |
| 14 | `equipment.ts:54-84` | `commands/lib.rs:1205`、`candidate.rs:134` | カタログ品適用(base=max、enchant clamp、枠切り詰め)、強化 Lv ≥ 12 で等級「最上」。ability / random_option 枠の切り詰めは TS にしかない |
| 16 | `EquipmentPane.svelte:597-730` | `equipment.rs:1131` | アビリティ枠の置換・枠超過・武器の hp/mp 回復除外(Rust 側なし) |
| 17 | `EquipmentPane.svelte:466-482, 518-590` | なし | `preferred = ["storm-blade", …]` の id 直書き、`ABILITY_TIERS` を名前の接頭辞(N-/R-/L-/E-/G-、古代精霊 < 深淵 < 喪失 < 夜星)から解析。等級は gamedata の属性にする |
| 18 | `labels.ts:8, 41-44, 64, 85-89, 118-120, 129, 140, 154, 163-170`、`enchant.ts:127` | `thesis_core.rs:22, 131`、`element.rs:38`、`equipment.rs:721`、`random_option.rs:34`、`StatKind::ALL` | enum の並び・分類(コア攻撃/補助種別、属性、アビリティ系統、OP ランク、依存種別、エンチャント 8 部位)のリテラル複製。`part_slot_rules` のように limits 経由のものと二重基準 |
| 22 | `Workspace.svelte:477`、`equipment.ts:21-30`、`measurement.ts:350-366` | — | "ペット会心 ×1.1" 文字列、「†改・セイクリッド は通常版と同じ画像」の名前規則(gamedata の icon id 属性に)、逆算可否。対人の `1 + rate × level` は B3 で解消済み |

確認して問題なし: `state.svelte.ts`、`api/transfer.ts`、`api/browserStore.ts`、`candidates.ts`、`format.ts`、`limits.svelte.ts`、`TracePanel.svelte`、`MeasurePage.svelte`、`ui/critChance.ts`、ActualDelay / CriticalRate / SoulLink の各ペイン。

## B. domain / commands / storage で無理をしている箇所

| # | 場所 | 症状 | 簡単な形 |
|---|---|---|---|
| 4 | `equipment.rs:360-414`(SienaStatBonus)、`stats.rs`(BaseStats / EffectiveStats)、`element.rs` | ステ別 7 フィールド構造体の手書き `get/get_mut`。stat_sources.rs 側の 6 型は `PerStat<T>`(stats.rs)にした(済み) | 残りも `PerStat<T>` に寄せる。`BaseStats` / `EffectiveStats` はテストの構造体リテラルが多いので値のまま可 |
| 5 | `equipment.rs:98-166, 1634-1650, 1690-1759`、`thesis_core.rs:260-277`、`siena.rs:231-255`、`candidate.rs:152-188`、`commands/lib.rs:1450-1474` | 装備補正 9 値のフィールド対応表が 7 通り。文字列キー `"thrust"` が domain → commands → TS まで走る | `EquipmentValueKind` を 9 種にし `EquipmentValues::get/get_mut(kind)` を 1 か所。候補 id は `(PartSlot, EquipmentValueKind)` |
| 6 | `commands/lib.rs:1223-1338` と `:1355-1446` | `list_upgrade_candidates` と `list_enchant_gains` がほぼ同文。`preview_effective_stats` の 13 引数呼び出しを 4 回複写(`:341, 405, 459, 479`)。`preview_defense` / `preview_versus` は 2 値のためにフルプレビュー | `stat_preview_of(...)` と `enchant_outcomes(...)` を切り出す |
| 8 | `stat_sources.rs:1929-2080, 2097-2213` | `StatLimits` が 150 項目 × 2 リスト。ラベル・部位ルール(`equipment.rs:336-355` は `PartSlot` メソッドの写し)は上限ではなくカタログ | ラベル・部位ルールは `list_*` 系へ。`StatLimits` は数値上限だけ |
| 11 | `random_option.rs:230-244`、`candidate.rs:58, 74, 91, 344`、`skill.rs:166` | 裸の `as i64` / `.round() as` / `.floor() as`(stat_sources.rs 側は `trunc_int` に寄せた) | `trunc_int` / `round_int` に寄せる |
| 12 | `commands/lib.rs:648-650, 836` | 事前検証済みで到達不能なフォールバック、`weapon_added_damage().unwrap_or(0)` が未収録を 0 に見せる(content_evaluation.rs 側の同文フォールバックと `unwrap_or_default` は消した) | 到達不能分は削除。未収録は `Option` のまま結果へ |
| 13 | `commands/lib.rs:632-637, 657-662, 1272-1276`、`character_repository.rs:504-508` | `enhance_type.or_else(item_id → catalog)` の解決が 4 回 | `EquipmentPart::enhance_type_resolved(catalog)` を domain に 1 つ |
| 14 | `commands/lib.rs:70-123` と `character_repository.rs:956-1006` | 保存前検証が別のエラー型で二重。storage 側は `_catalog` 未使用、commands 側は `"不正な値: "` 接頭辞を後付けで真似る | `NewCharacter::validate(catalogs)` を domain に置き両方が呼ぶ |
| 15 | `equipment.rs:860-872` | `EquipmentPartList` の `DerefMut` が未選択部位を黙って登録・選択する。読み取りのつもりの `&mut` で保存データが増える | Deref を外し `selected_mut()` を明示 |
| 16 | `equipment.rs:1560, 1576-1584`、`stat_sources.rs:520` | `+= 9` 直書き(`EQUIPMENT_ELEMENT_VALUE_MAX` がある)、`without_part` は `without_selected_part` の別名、`for_new_character` = `default` | 削除・定数参照 |
| 17 | `gamedata/src/buffs.rs:290-293`、`stat_sources.rs:1005` | `RecordOnly` バフにダミーの `target/layer`、マスタリー分離後の注記残骸 | 型で分ける・注記削除 |
| 18 | `skill.rs:130-175`、`damage.rs:1031-1032` | `power` / `power_per_second` を保存して再計算、`effective_*` は入力の写し | 片方を消す |
| 19 | `gamedata/src/damage_inputs.rs:37-56` | gamedata に「カタログ解決 + domain 呼び出し」の接着層。architecture.md の gamedata 定義(型とローダ)から外れる | commands へ移す |
| 20 | `character_repository.rs:250-737` | migration 10 本。ADR-008 でユーザー環境の DB のため残すと決めているので違反ではない。ただし `migrate_equipment_to_registered_lists` は gamedata の倍率表で等級を逆算しており domain 計算の storage 内再現 | 逆算は domain の関数を呼ぶ |
| 21 | `defense.rs:517-744 accuracy_growth / evasion_growth`、`stat_sources.rs:741 buff_accuracy_point_room` | 伸びしろの材料が「合成後の `stat_cap` との差 1 本」「未選択バフの単純合計(排他枠を見ない)」「エンチャント枠」だけで、**源ごと**(ペット S・ルーン・クラウン・カード・聖物 / DEX 増加バフ / 装備アビリティの命中の空き枠・上限未満 / ランダム OP の命中P の空き枠・ランク上げ)には分解できない。文言(label / detail)も domain が文字列で組む | 「源 → 現在値 → 上限 → 積んだ後の値」を返す列挙 API を `stat_sources` / `equipment` / `random_option` に置き、`GrowthRoom` はそれを費用順に並べるだけにする。文言は画面側([versus-next-actions.md](versus-next-actions.md)) |

問題なし: `category.rs`(enum + kind/cap/label の表でデータ駆動)、`rounding.rs`、`stats.rs::effective_stat`、`attack_power.rs`、`actual_delay.rs`、`critical_rate.rs`、`common_skill.rs`、`ultimate_skill.rs`、`soul_link.rs`、`title.rs`、`thesis_core.rs`、`siena.rs`(`SienaEffect` で効き先を 1 か所に集約)、`candidate.rs::rank_candidates`、`content.rs::ContentRequirement::check`。domain / commands にキャラ名・バフ id の文字列比較は的中剣以外 0 件。

## C. 着手順

1. B1(補正パイプライン一本化 → `build_stat_modifiers`)と B2(`DamageInput` → `DamageMaterial` + `DamageTarget`)は済み。フロント写経を Rust に移す受け皿になる。
2. **A1**(Rust に無い規則)を domain / gamedata に新設し、コマンドで配る。到達 4 段・装備可否・エンチャントプランは `ContentEvaluation` と装備検証の拡張で収まる。
3. **A2**(写経)は Rust から値・上限・候補を返して TS を削除。A2-11 のバフ既定値の食い違いはここで解消。
4. **B8**(`StatLimits` 分割)は独立して進められる。B3(的中剣のデータ化)と B4(`PerStat`)は済み。
5. 残り(B5〜B7、B9〜B20)は触った箇所から順に。
6. **B21**(伸びしろの材料を源ごとに列挙する API)。対人タブの「次にできること」([versus-next-actions.md](versus-next-actions.md))の受け皿で、A1-2 / A1-16(アビリティ枠の規則)を Rust に移した結果を使う。
