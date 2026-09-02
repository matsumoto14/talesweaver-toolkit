# アーキテクチャ監査(2026-09-02)

docs/architecture.md の「UI は表示と入力のみ。計算・判定は必ず Rust 側」と AGENTS.md の原則
(最もシンプルな実装・投機的な間接化をしない・個別バフをコードで分岐しない)に照らして、
フロント(apps/desktop/src)と domain / commands / storage を読んだ結果。行番号は監査時点のもの。

対処したら該当行を消す。全部消えたらこのファイルも消す。

## A. フロントに残っているドメインロジック

### A1. Rust に対応物がなく、フロントだけが規則を持っているもの(最優先)

ドメインに規則が存在しないので、写経の削除ではなく **domain / gamedata への新設**が要る。

| # | 場所 | 中身 | 受け皿 |
|---|---|---|---|
| 1 | `pages/chars/sources/EquipmentPane.svelte:150-247` | 依存種別→武器系統、`boris_*` スキル→武器種、キャラの weapon/armor/wrist クラスによる装備可否と候補絞り込み | gamedata にクラス表はある(`characters.rs`)が判定関数がない。`Character` × `EquipmentItem` の可否を domain に置き、`list_equipment_items` が絞って返す |
| 2 | `EquipmentPane.svelte:442-466` | 武器アビリティの系統適合表(`abilityFitsWeapon`)と enhance_type→系統(`abilityWeaponSystem`) | Rust の検証は `equipment.rs:1131` の `exclusive_group` 重複のみで、系統不一致は通る。装備検証に加え、候補列挙も Rust から |
| 3 | `EquipmentPane.svelte:341-370` | エンチャント完了プラン(+17 / +20 巻物の最小回数探索)。`enchantPlanStatsFor` は依存種別→ステ表を再写経 | domain に `enchant_plan(remaining) -> Plan`。ステ表は `candidate.rs::enchant_dependency_keys` を返す |
| 6 | `HomePage.svelte:792-880` / `EquipmentPane.svelte:261-270` | レリック育成順序: item id 文字列 `godbird-pendant-plus{n}` の解析、上限到達で +Lv 解禁、段上げ後は `values_min` に戻す | Rust は `equipment.rs:1044` で下限検証のみ。レリック段の遷移を domain に |

### A2. Rust に同じ計算があり、TS が写経しているもの(二重化)

Rust から値・上限・候補を返す形にして TS 側を消す。

| # | TS | Rust | 備考 |
|---|---|---|---|
| 11 | `buffs.ts:43-86`、`BuffsPage.svelte:95-103, 436-448` | `stat_sources.rs:995-1027, 1686-1694` | 排他枠の衝突・ON 時の初期選択・値解決。**既定値が食い違う**: Rust は `default_value.unwrap_or(max)` を clamp、TS は `default_value ?? 0`。ダメージ系グループの id(`attack_damage_general/isabel/japan`)も TS 直書き |
| 12 | `equipment.ts:273-291`、`RandomOptionPane.svelte:105-119` | `random_option.rs:244-266`、`equipment.rs:1313` | ランダム OP の物理/魔法発動条件、同カテゴリ 1 部位 1 つ |
| 13 | `equipment.ts:140-225`、`HomePage.svelte:633-661` | `stat_sources.rs:255-265`、`siena.rs:476-487`、`random_option.rs:137` | 神鳥の聖物 段階↔値、シエナ段階→枠数、OP 既定値。コメントで「ミラー」と自認 |
| 14 | `equipment.ts:54-84` | `commands/lib.rs:1205`、`candidate.rs:134` | カタログ品適用(base=max、enchant clamp、枠切り詰め)、強化 Lv ≥ 12 で等級「最上」。ability / random_option 枠の切り詰めは TS にしかない |
| 15 | `CommonSkillPane.svelte:39-90` | `common_skill.rs:210, 216` | 前提スキルのゲート(reinforce + unleash、augment + 1)。limits で上限を返せば済む |
| 16 | `EquipmentPane.svelte:597-730` | `equipment.rs:1131` | アビリティ枠の置換・枠超過・武器の hp/mp 回復除外(Rust 側なし) |
| 17 | `EquipmentPane.svelte:466-482, 518-590` | なし | `preferred = ["storm-blade", …]` の id 直書き、`ABILITY_TIERS` を名前の接頭辞(N-/R-/L-/E-/G-、古代精霊 < 深淵 < 喪失 < 夜星)から解析。等級は gamedata の属性にする |
| 18 | `labels.ts:8, 41-44, 64, 85-89, 118-120, 129, 140, 154, 163-170`、`enchant.ts:127` | `thesis_core.rs:22, 131`、`element.rs:38`、`equipment.rs:721`、`random_option.rs:34`、`StatKind::ALL` | enum の並び・分類(コア攻撃/補助種別、属性、アビリティ系統、OP ランク、依存種別、エンチャント 8 部位)のリテラル複製。`part_slot_rules` のように limits 経由のものと二重基準 |
| 19 | `EquipmentPane.svelte:781-785` / `StatusPane.svelte:102` / `CommonSkillPane.svelte:125` | `ENHANCE_LEVEL_MAX`、`limits.awakening_stage_max`、`limits.sharpness_vision_level_max` | 強化 Lv 候補 `[0,10..15]`・覚醒 6 段・シャープネス 10 段の直書き |
| 20 | `CalcPage.svelte:393-413` | `damage.rs:397-460` | 「なぜこの数字?」の帯が Rust の段名文字列を Set で持ち `reached / running` で倍率を再導出。段に `kind: factor \| running` を返せば消える |
| 21 | `CalcPage.svelte:1300-1302` | — | 極限の効果値のため `scope_eye/full_throttle` / `wide_focus` を強制セットして preview を 2 回叩く。効果表を返す API に |
| 22 | `VersusPage.svelte:170-178`、`Workspace.svelte:477`、`equipment.ts:21-30`、`measurement.ts:350-366` | `defense.rs:268`、`critical_rate.rs:24`、— | `1 + rate × level` の再計算、"ペット会心 ×1.1" 文字列、「†改・セイクリッド は通常版と同じ画像」の名前規則(gamedata の icon id 属性に)、逆算可否 |

確認して問題なし: `state.svelte.ts`、`api/transfer.ts`、`api/browserStore.ts`、`candidates.ts`、`format.ts`、`limits.svelte.ts`、`TracePanel.svelte`、`MeasurePage.svelte`、`ui/critChance.ts`、ActualDelay / CriticalRate / SoulLink の各ペイン。

## B. domain / commands / storage で無理をしている箇所

| # | 場所 | 症状 | 簡単な形 |
|---|---|---|---|
| 3 | `defense.rs:213-286`、`character_skill.rs:60-70, 179-221`、`commands/lib.rs:746-762` | 極・的中剣を名指しで分岐: 専用 enum `AccuracyBoost`、`PRECISION_SWORD_*` 定数 4 つ、`disabled_with_precision_sword` フラグ、1 スキルのためだけの `skill_levels` Map、既定 Lv7 の `unwrap_or`、伸びしろの文言を文字列で組む。「個別バフをコードで分岐しない」に最も反する | カタログ側に `SkillEffect::AccuracyRate { per_level, shift, max_level }` と汎用 `exclusive_with` を持たせ、`AccuracyBoost` は解決済み値だけに。SLv は `CharacterSkills` の一般機能に |
| 4 | `stat_sources.rs`(PetSkills / RuneLevels / Crown / MonsterCards / SacredRelic / Adjustments / BuffStatAmplification)、`equipment.rs:360-414`、`stats.rs`、`element.rs` | ステ別 7 フィールド構造体 × 11 個の手書き `get/get_mut`、属性 8 フィールド版も同型。`build_modifiers`(`stat_sources.rs:886-963`)は同型ループ 5 つ | `PerStat<T>`(`[T; 7]` を `StatKind` で添字)1 型。約 250 行が消える |
| 5 | `equipment.rs:98-166, 1634-1650, 1690-1759`、`thesis_core.rs:260-277`、`siena.rs:231-255`、`candidate.rs:152-188`、`commands/lib.rs:1450-1474` | 装備補正 9 値のフィールド対応表が 7 通り。文字列キー `"thrust"` が domain → commands → TS まで走る | `EquipmentValueKind` を 9 種にし `EquipmentValues::get/get_mut(kind)` を 1 か所。候補 id は `(PartSlot, EquipmentValueKind)` |
| 6 | `commands/lib.rs:1223-1338` と `:1355-1446` | `list_upgrade_candidates` と `list_enchant_gains` がほぼ同文。`preview_effective_stats` の 13 引数呼び出しを 4 回複写(`:341, 405, 459, 479`)。`preview_defense` / `preview_versus` は 2 値のためにフルプレビュー | `stat_preview_of(...)` と `enchant_outcomes(...)` を切り出す |
| 8 | `stat_sources.rs:1929-2080, 2097-2213` | `StatLimits` が 150 項目 × 2 リスト。ラベル・部位ルール(`equipment.rs:336-355` は `PartSlot` メソッドの写し)は上限ではなくカタログ | ラベル・部位ルールは `list_*` 系へ。`StatLimits` は数値上限だけ |
| 9 | `defense.rs:516-537, 661-675, 873-943` | `AccuracyGrowth` 18 引数 / `evasion_growth` 13 引数。`min_hit_rate.unwrap_or(0)` を 6 回書き、未収録を 0 に潰したうえで `_recorded: bool` を別に返す | `VersusAttacker` / `VersusDefender` をそのまま渡す。`Recorded(i64) \| Unrecorded` の enum |
| 10 | `defense.rs:89-98`、`random_option.rs:249-269` | 物理/魔法の依存分類が 3 か所 | `SkillDependency::attack_type()` |
| 11 | `stat_sources.rs:1051, 1054, 1406, 1539, 1542`、`random_option.rs:230-244`、`candidate.rs:58, 74, 91, 344`、`skill.rs:166` | 裸の `as i64` / `.round() as` / `.floor() as` | `trunc_int` / `round_int` に寄せる。バフ固定値層は最初から `i64` |
| 12 | `content_evaluation.rs:284-320, 203-222`、`commands/lib.rs:648-650, 836` | 同文フォールバック 2 つ(後者は `commands/lib.rs:1060` で事前検証済みで到達不能)、`ALL` から作った配列への `unwrap_or_default`、`weapon_added_damage().unwrap_or(0)` が未収録を 0 に見せる | 到達不能分は削除。未収録は `Option` のまま結果へ |
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
4. **B3 / B4 / B8**(的中剣のデータ化、`PerStat`、`StatLimits` 分割)は独立して進められる。
5. 残り(B5〜B7、B9〜B20)は触った箇所から順に。
6. **B21**(伸びしろの材料を源ごとに列挙する API)は B3 / B4 の後。対人タブの「次にできること」([versus-next-actions.md](versus-next-actions.md))の受け皿で、A1-2 / A1-16(アビリティ枠の規則)を Rust に移した結果を使う。
