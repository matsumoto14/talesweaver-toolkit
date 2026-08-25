// Tauri コマンドの入出力の型。Rust の serde 構造体(crates/domain, storage, gamedata)の写し。
// 手で同期しているため、Rust 側の構造体を変えたらここも必ず変える。

export type StatKind = "stab" | "hack" | "int" | "def" | "mr" | "dex" | "agi";
export type BaseStats = Record<StatKind, number>;

export interface Awakening {
  stage: number;
  eternal_level: number;
}

export interface GameCharacter {
  id: string;
  name: string;
}

export type SkillDependency = "stab" | "hack" | "int" | "mr" | "stab_hack" | "hack_int";

export interface Skill {
  id: string;
  name: string;
  dependency: SkillDependency;
  multiplier: number;
  hit_count: number;
  critical_multiplier: number;
  /** スキルの属性 */
  element: Element;
  /** スキル命中(wiki 表記 +15 済みの実値)。null = wiki 未記載 */
  accuracy: number | null;
  /** スキルクリティカル率(wiki スキル性能一覧の Cri値)。null = wiki 未記載 */
  critical_rate: number | null;
  /** スキル Lv(wiki スキル性能一覧の SLv) */
  level: number;
}

// 属性 8 種。crates/domain/src/element.rs の Element(snake_case)。
export type Element =
  | "fire" | "water" | "wind" | "earth" | "thunder" | "white" | "black" | "neutral";

export interface Enemy {
  id: string;
  name: string;
  defense: number;
  damage_reduction: number;
  cut_rate_a: number;
  element_threshold: number;
}

// ペット S スキルの段階(wiki: PET)。crates/domain/src/stat_sources.rs の PetSkillTier(snake_case)。
export type PetSkillTier = "basic" | "true_lv1" | "true_lv2" | "true_lv3" | "true_lv4";
// ペット S スキル。ステごとに 1 つ(上位段階を選ぶと置き換わる)。未選択は null。
export type PetSkills = Record<StatKind, PetSkillTier | null>;
// ルーンスキル。ステごと 0..=20。
export type RuneLevels = Record<StatKind, number>;
// クラウン。ステごと 0..=300。
export type Crown = Record<StatKind, number>;
// 神鳥の聖物。ステごと 0..=40 段階(実加算値は段階×10)。
export type SacredRelic = Record<StatKind, number>;

// 能力値計算の5レイヤー(wiki §2)。crates/domain/src/stats.rs の StatLayer(snake_case)。
export type StatLayer = "percent_of_base" | "fixed" | "multiplier_a" | "multiplier_b" | "final_fixed";

// バフの対象ステ。crates/domain/src/stat_sources.rs の BuffTarget(rename_all snake_case、外部タグ付け)。
export type BuffTarget = "all_stats" | { stat: StatKind } | "user_selected" | { stats: StatKind[] };

// バフの値の決め方。crates/domain/src/stat_sources.rs の BuffValue(rename_all snake_case、外部タグ付け)。
export type BuffValue = { fixed: number } | { choice: number[] } | { user_input: { min: number; max: number } };

// バフの分類。crates/domain/src/stat_sources.rs の BuffGroup(rename_all snake_case、外部タグ付け)。
export type BuffGroup =
  | "consumable"
  | { character_skill: { game_character_id: string } }
  | "ally_skill";

export interface BuffDefinition {
  id: string;
  name: string;
  target: BuffTarget;
  layer: StatLayer;
  value: BuffValue;
  exclusive_slots: string[];
  source_url: string;
  note: string;
  /** BuffValue::UserInput の初期値。それ以外は null */
  default_value: number | null;
  group: BuffGroup;
}

export interface BuffChoice {
  buff_id: string;
  stat: StatKind | null;
  choice_index: number | null;
  value: number | null;
}

export interface BuffSelection {
  choices: BuffChoice[];
}

export interface StatAdjustment {
  /** このステに +N する(固定値層への加算) */
  add: number;
  /** Some のとき最終能力値をこの値に固定する */
  pin: number | null;
}
export type Adjustments = Record<StatKind, StatAdjustment>;

export interface StatSources {
  pet_skills: PetSkills;
  rune_levels: RuneLevels;
  crown: Crown;
  sacred_relic: SacredRelic;
  buffs: BuffSelection;
  adjustments: Adjustments;
  /** 装備の属性強化以外の属性値の供給源 */
  elements: ElementSources;
}

// 属性値の供給源の種別。crates/domain/src/element.rs の ElementSourceId(snake_case)。
export type ElementSourceId = "pet" | "monster_card" | "rune" | "helm_ability" | "cuffs_ability";

// 供給源ごとに「どの属性に乗せているか」。null = 使っていない。
export interface ElementSources {
  pet: Element | null;
  monster_card: Element | null;
  rune: Element | null;
  helm_ability: Element | null;
  cuffs_ability: Element | null;
}

// 供給源 1 つ分の定義(表示名と加算値)。crates/domain/src/element.rs の ElementSourceDef。
export interface ElementSourceDef {
  id: ElementSourceId;
  name: string;
  value: number;
}

// 属性値の内訳。crates/domain/src/element.rs の ElementPreview。
export interface ElementPreview {
  base: ElementValues;
  equipment: ElementValues;
  sources: ElementValues;
  /** 3 つを足して上限 255 で頭打ちにした値 */
  total: ElementValues;
}

// 属性ごとの値。crates/domain/src/element.rs の ElementValues。
export type ElementValues = Record<Element, number>;

export interface StatContribution {
  source: string;
  kind: StatKind;
  layer: StatLayer;
  value: number;
}

// 装備補正 9 種(wiki Item ページの列順: 突き/斬り/物防/魔攻/魔防/命中/Cri/回避/敏捷)。
// crates/domain/src/equipment.rs の EquipmentValues。
export interface EquipmentValues {
  thrust: number;
  slash: number;
  physical_defense: number;
  magic_attack: number;
  magic_defense: number;
  accuracy: number;
  critical: number;
  evasion: number;
  agility: number;
}

// 装備部位。crates/domain/src/equipment.rs の PartSlot(snake_case)。
export type PartSlot =
  | "weapon" | "armor" | "helm" | "shield" | "shield_plus"
  | "head" | "body" | "hand" | "leg" | "effect" | "artifact" | "relic";

// シエナのオーラのステ加算。crates/domain/src/equipment.rs の SienaStatBonus。
export type SienaStatBonus = Record<StatKind, number>;

// シエナのオーラ(部位ごと)。crates/domain/src/equipment.rs の SienaAura。
export interface SienaAura {
  /** 増幅段階 0..=10(= 解放される能力値スロット数)。計算には使わない */
  stage: number;
  /** 能力値の合計(武器/盾のみ)。強化能力値へ合流する */
  values: EquipmentValues;
  /** 能力値スロットのステ加算(武器/盾以外)。最終固定値層へ合流する */
  stats: SienaStatBonus;
  /** 追加オプション「全ステータス増加」。STAB〜AGI の全ステに同じ値が乗る(部位を問わない) */
  all_stats: number;
  /** 追加オプション「攻撃力増加」の %(カテゴリ New1) */
  attack_rate_percent: number;
}

// ランダムオプションのランク。crates/domain/src/random_option.rs の RandomOptionRank。
export type RandomOptionRank = "normal" | "valuable" | "rare" | "special" | "s_true";

// ランダムオプションの効き先。crates/domain/src/random_option.rs の RandomOptionEffect。
// タプル variant(依存別)は serde が { dependency_damage_rate: SkillDependency } になる。
export type RandomOptionEffect =
  | { dependency_damage_rate: SkillDependency }
  | "attack_damage_rate"
  | "accuracy_point"
  | "evasion_point"
  | "accuracy_and_evasion_point"
  | "record_only";

// ランクごとの効果値レンジ。crates/domain/src/random_option.rs の RandomOptionTier。
export interface RandomOptionTier {
  rank: RandomOptionRank;
  min: number;
  max: number;
}

// ランダムオプション定義(gamedata のカタログ)。crates/domain/src/random_option.rs の RandomOptionDef。
export interface RandomOptionDef {
  id: string;
  name: string;
  slot: PartSlot;
  /** wiki 一覧表のカテゴリー番号。同じ番号は 1 部位に 1 つまで(0 は制約なし) */
  category: number;
  effect: RandomOptionEffect;
  tiers: RandomOptionTier[];
  note: string;
}

// キャラが付けている 1 枠。crates/domain/src/random_option.rs の RandomOptionSlot。
export interface RandomOptionSlot {
  option_id: string;
  rank: RandomOptionRank;
  /** 実測値の上書き。null = レンジ上限 */
  value: number | null;
}

// テシスコアの地域。crates/domain/src/thesis_core.rs の CoreRegion(snake_case)。
export type CoreRegion = "mercurial" | "abyss" | "eclipse" | "rubicona";

// テシスコアのタイプ(火力 4 + 補助 4)。crates/domain/src/thesis_core.rs の CoreType(snake_case)。
// 補助タイプは記録と入場条件「コア N」の合計にのみ効く(与ダメージ式には入らない)。
export type CoreType =
  | "thrust" | "slash" | "magic_attack" | "magic_defense"
  | "physical_defense" | "evasion" | "agility" | "accuracy";

// コア 1 個。crates/domain/src/thesis_core.rs の ThesisCore。
export interface ThesisCore {
  core_type: CoreType;
  /** 進化段階 0..=4 */
  evolution: number;
  /** 強化段階 0..=4 */
  enhancement: number;
}

// 1 地域分の 6 枠。未装着は null。crates/domain/src/thesis_core.rs の CoreSet。
export interface CoreSet {
  slots: (ThesisCore | null)[];
}

// 地域ごとのコアセット。crates/domain/src/thesis_core.rs の ThesisCores。
export type ThesisCores = Record<CoreRegion, CoreSet>;

// 装備部位 1 つ。crates/domain/src/equipment.rs の EquipmentPart。
export interface EquipmentPart {
  /** カタログ参照(EquipmentItem.id)。null = 未装備またはカスタム */
  item_id: string | null;
  /** カタログ外アイテムの表示名 `[仮]` */
  custom_name: string | null;
  /** 実測の基本能力値 */
  base: EquipmentValues;
  /** エンチャント値(強化能力値) */
  enchant: EquipmentValues;
  /** 装備強化 Lv(0..=15)。武器・鎧のみ 0 超を許可 */
  enhance_level: number;
  /** +12 以上の追加固定ダメージ実測値の上書き。+11 以下は null 固定 */
  enhance_added_damage: number | null;
  /** 装備アビリティ id(武器のみ非空を許可) */
  abilities: string[];
  /** シエナのオーラ(発現できるのは 8 部位。未発現は中立値) */
  siena: SienaAura;
  /** 付与した属性(1 部位 1 属性)。null = 属性なし */
  element: Element | null;
  /** 付与した属性値(0..=9) */
  element_value: number;
  /** ランダムオプション。同じカテゴリーは 1 部位に 1 つまで */
  random_options: RandomOptionSlot[];
}

// 12 部位。crates/domain/src/equipment.rs の EquipmentParts(named field)。
export interface EquipmentParts {
  weapon: EquipmentPart;
  armor: EquipmentPart;
  helm: EquipmentPart;
  shield: EquipmentPart;
  shield_plus: EquipmentPart;
  head: EquipmentPart;
  body: EquipmentPart;
  hand: EquipmentPart;
  leg: EquipmentPart;
  effect: EquipmentPart;
  artifact: EquipmentPart;
  relic: EquipmentPart;
}

// 装備補正一式(部位別装備 12 スロット + パワーウェポン/ストロングウェポン)。
// crates/domain/src/equipment.rs の Equipment。
export interface Equipment {
  parts: EquipmentParts;
  /** パワーウェポン(自身の装備補正を2%増加) */
  power_weapon: boolean;
  /** ストロングウェポンの Lv(0 = 未使用、1〜6) */
  strong_weapon_level: number;
  /** テシスコア(地域ごとに 6 枠) */
  thesis_cores: ThesisCores;
}

// gamedata の出典。crates/gamedata/src/lib.rs の Source。
export interface Source {
  page: string;
  retrieved_on: string;
  note: string;
}

// 武器種(wiki: 装備システム/装備強化「系統」表)。crates/gamedata/src/equipment_catalog.rs の WeaponClass(snake_case)。
export type WeaponClass =
  | "rapier" | "dagger" | "spear" | "small_sword" | "physical_gun" | "claw" | "hand_launcher"
  | "long_sword" | "tachi" | "war_staff" | "short_sword" | "rod" | "nunchaku"
  | "katana" | "axe" | "whip" | "kara" | "dual_blade_physical" | "scythe" | "arming_sword"
  | "magic_wand" | "wand" | "magic_gun" | "scepter" | "totem"
  | "great_sword"
  | "holy_staff" | "handbell" | "dual_blade_magic" | "hammer";

// 装備カタログの 1 アイテム。crates/gamedata/src/equipment_catalog.rs の EquipmentItem。
export interface EquipmentItem {
  id: string;
  slot: PartSlot;
  name: string;
  /** 基本能力値のレンジ下限(wiki: Item ページの MR レンジ) */
  values_min: EquipmentValues;
  /** 基本能力値のレンジ上限 */
  values_max: EquipmentValues;
  /** エンチャント上限(エンチャント不可は全 0) */
  enchant_caps: EquipmentValues;
  /** 武器のみ非 null */
  weapon_class: WeaponClass | null;
  source: Source;
}

// 武器アビリティの系統。crates/domain/src/equipment.rs の EquipmentAbilityFamily。
// 同じ系統は 1 部位に 1 つだけ(段が違っても併用できない)。
export type EquipmentAbilityFamily =
  | "pointed_blade" | "sharp_blade" | "intelligence" | "magic_resistance";

// 武器アビリティ定義。crates/domain/src/equipment.rs の EquipmentAbilityDef。
export interface EquipmentAbilityDef {
  family: EquipmentAbilityFamily;
  id: string;
  name: string;
  /** 装備攻撃力(基本能力値)への加算値 */
  values: EquipmentValues;
}

export interface RegisteredCharacter {
  id: number;
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
  stat_sources: StatSources;
  equipment: Equipment;
  /** 主軸スキル(攻撃力の依存種別を決める)。未選択は null */
  main_skill_id: string | null;
}

export interface NewCharacter {
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
  stat_sources: StatSources;
  equipment: Equipment;
  main_skill_id: string | null;
}

export type CategoryKind = "assigned" | "fixed" | "rate";

export interface CategoryCap {
  min: number | null;
  max: number | null;
}

export interface CategoryTrace {
  category: string;
  symbol: string;
  label: string;
  kind: CategoryKind;
  /** 上限適用前の生の合算値(割合は Σ%)。`raw − value` が上限で捨てられた分 */
  raw: number;
  value: number;
  factor: number;
  cap: CategoryCap | null;
}

// pin(能力値の固定)の出所。crates/domain/src/stats.rs の PinSource(snake_case)。
export type PinSource = "saved" | "temporary";

export interface StatTrace {
  kind: StatKind;
  base: number;
  percent_of_base_total: number;
  fixed: number;
  multiplier_a: number;
  basic: number;
  multiplier_b: number;
  multiplier_b_bonus: number;
  final_fixed: number;
  effective: number;
  /** pin(能力値の固定)が適用された場合の上書き前の値。未適用は null */
  pinned_from: number | null;
  /** pin の出所。未適用は null */
  pin_source: PinSource | null;
}

// 7 ステータスすべての最終能力値。crates/domain/src/stats.rs の EffectiveStats。
export type EffectiveStats = Record<StatKind, number>;

// crates/domain/src/attack_power.rs の AttackPowerBreakdown。攻撃力(A)の内訳。
export interface AttackPowerBreakdown {
  /** ステ由来攻撃力(切捨て前) */
  stat_attack: number;
  /** 装備の基本能力値に係数を掛けた分 */
  equipment_base_attack: number;
  /** 装備の強化能力値に係数を掛けた分(テシスコアは地域なしのため含まない) */
  equipment_enhanced_attack: number;
  /** 装備攻撃力強化倍率(パワーウェポン + ストロングウェポン) */
  enhance_rate: number;
  /** 攻撃力(A) */
  value: number;
}

// crates/domain/src/stat_sources.rs の PartAttackContribution。
export interface PartAttackContribution {
  slot: PartSlot;
  /** A −(その部位を未装備にした A)= 外すと減る量 */
  value: number;
}

// crates/domain/src/stat_sources.rs の AttackPreview。主軸スキルが選ばれているときだけ返る。
export interface AttackPreview {
  breakdown: AttackPowerBreakdown;
  part_contributions: PartAttackContribution[];
}

// crates/domain/src/stat_sources.rs の StatPreview。preview_effective_stats コマンドの戻り値(保存しない)。
export interface StatPreview {
  stats: EffectiveStats;
  traces: StatTrace[];
  contributions: StatContribution[];
  /** 主軸スキル未選択なら null */
  attack: AttackPreview | null;
}

// crates/domain/src/defense.rs の DefenseProfile。割合は小数表現(50% → 0.5)。
export interface DefenseProfile {
  physical_defense: number;
  magic_defense: number;
  composite_defense: number;
  /** 防御力の上限(覚醒段階とエタの意志 Lv で決まる) */
  defense_cap: number;
  /** 上限で捨てられた分。すべて 0 なら上限に当たっていない */
  physical_defense_loss: number;
  magic_defense_loss: number;
  composite_defense_loss: number;
  physical_cut_rate: number;
  magic_cut_rate: number;
  composite_cut_rate: number;
  /** 特殊回避(コンボ回避) */
  combo_evasion: number;
  /** 攻撃タイプ別の回避P。通常回避「率」は敵の命中Pが取れないので出さない */
  evasion_point: EvasionPoints;
  /** 装備物防(基本 + 強化) */
  equipment_physical_defense: number;
  /** 装備魔防(基本 + 強化) */
  equipment_magic_defense: number;
  /** 装備回避率補正(基本 + 強化) */
  equipment_evasion: number;
  /** 装備敏捷度補正(基本 + 強化) */
  equipment_agility: number;
}

// crates/domain/src/defense.rs の EvasionPoints(wiki 計算式まとめ#EvasionPoint)。
export interface EvasionPoints {
  physical: number;
  magic: number;
  composite: number;
}

export interface FormulaStep {
  name: string;
  expression: string;
  value: number;
}

export interface DamageTriple {
  min: number;
  max: number;
  critical: number;
}

export interface DamageTrace {
  stats: StatTrace[];
  /** ステ補正源(ペット/ルーン/クラウン/聖物/バフ/調整値)の寄与内訳 */
  stat_contributions: StatContribution[];
  categories: CategoryTrace[];
  steps_min: FormulaStep[];
  steps_max: FormulaStep[];
  steps_critical: FormulaStep[];
}

export interface DamageResult {
  /** 1 段あたりの与ダメージ(ダメージ上限を適用したあと) */
  per_hit: DamageTriple;
  total: DamageTriple;
  hit_count: number;
  /** 与ダメージの上限(1 段ごとに適用) */
  damage_cap: number;
  /** 上限で捨てられた分(1 段あたり)。すべて 0 なら上限に当たっていない */
  capped_loss: DamageTriple;
  /** 命中P。敵の回避Pを 100 上回ると必中。null = スキル命中が wiki 未記載で出せない */
  accuracy_point: number | null;
  trace: DamageTrace;
}

// crates/domain/src/stat_sources.rs の StatLimits。get_stat_limits コマンドの戻り値。
export interface StatLimits {
  base_stat_max: number;
  rune_level_max: number;
  crown_max: number;
  sacred_relic_stage_max: number;
  adjustment_add_min: number;
  adjustment_add_max: number;
  adjustment_pin_min: number;
  adjustment_pin_max: number;
  equipment_value_max: number;
  strong_weapon_level_max: number;
  /** 装備強化 Lv 上限(wiki: 装備システム/装備強化。+1〜+15) */
  enhance_level_max: number;
  /** +12 以上の追加固定ダメージ実測値の上限(実用上の安全域)`[仮]` */
  enhance_added_damage_max: number;
  /** シエナのオーラの増幅段階の上限 */
  siena_stage_max: number;
  /** シエナのオーラの追加オプション「攻撃力増加」の 1 部位あたり上限 % */
  siena_attack_rate_percent_max: number;
  /** シエナのオーラの能力値スロットによるステ加算の 1 部位・1 ステあたり上限 */
  siena_stat_bonus_max: number;
  /** シエナのオーラの追加オプション「全ステータス増加」の 1 部位あたり上限 */
  siena_all_stats_bonus_max: number;
  /** テシスコアの装着枠数 */
  core_slot_count: number;
  core_evolution_max: number;
  core_enhancement_max: number;
  /** 装備 1 部位に付与できる属性値の上限 */
  equipment_element_value_max: number;
  /** キャラの属性値の上限 */
  element_value_max: number;
  /** 覚醒段階の上限 */
  awakening_stage_max: number;
  /** エタの意志 Lv の上限 */
  eternal_level_max: number;
  /** ランダムオプションの効果値の上限 `[仮]` */
  random_option_value_max: number;
}


// --- コンテンツ(crates/domain/src/content.rs) ---

// 入場条件。serde の外部タグ付け enum の写し。
// equipment_by_skill は「使うスキルの依存種別で比較先が決まる」条件(swiki の S/H/I・M・複合列)。
export type ContentRequirement =
  | { awakening_stage: number }
  | { eternal_level: number }
  | { equipment_by_skill: { single: number; mr: number; composite: number } }
  | { thesis_core_total: number };

export interface RequirementCheck {
  label: string;
  current: number;
  required: number;
  ok: boolean;
}

// crates/domain/src/content.rs の ContentSeries。段数違いの同一コンテンツをまとめる系列。
export interface ContentSeries {
  id: string;
  name: string;
  /** この Content の段(難易度) */
  step: number;
}

export interface Content {
  /** 段数違いの系列に属するなら系列情報。単独のコンテンツは null */
  series: ContentSeries | null;
  id: string;
  name: string;
  /** 敵データが無い(入場条件のみ判定する)コンテンツは null */
  enemy_id: string | null;
  /** 実用的に周回できる 1 ヒット(最大)の目安ダメージ。敵データが無ければ null */
  need_per_hit: number | null;
  requirements: ContentRequirement[];
  /** このコンテンツで効くテシスコアの地域。対応が取れないコンテンツは null */
  core_region: CoreRegion | null;
  /** 判定対象外の入場条件の注記(ルーン Lv・共通スキル等。表示専用) */
  entry_note: string | null;
  team_note: string | null;
}

export interface ContentArea {
  id: string;
  name: string;
  contents: Content[];
}

export interface BestSkillDamage {
  skill_id: string;
  per_hit_max: number;
  total_max: number;
}

export interface ContentEvaluation {
  content_id: string;
  /** スキル未収録キャラ・敵データなしコンテンツは null */
  damage: BestSkillDamage | null;
  checks: RequirementCheck[];
  entry_ok: boolean;
  /** 敵データなし(目安なし)は火力不問で true */
  reaches_need: boolean;
  clear: boolean;
}
