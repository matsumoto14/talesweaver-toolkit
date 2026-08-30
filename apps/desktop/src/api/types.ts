// Tauri コマンドの入出力の型。Rust の serde 構造体(crates/domain, storage, gamedata)の写し。
// 手で同期しているため、Rust 側の構造体を変えたらここも必ず変える。

/** 情報パネルに出すアプリ情報(desktop の `AppInfo`。camelCase) */
export interface AppInfo {
  version: string;
  /** 登録キャラの保存先。「データは端末内だけ」を裏づけるために出す */
  databasePath: string;
}

/** 起動時の復元などの通知(desktop の `StartupNoticePayload`。camelCase) */
export interface StartupNotice {
  message: string;
  /** false のとき、この起動で加えた変更は保存されない */
  persistsChanges: boolean;
}

export type StatKind = "stab" | "hack" | "int" | "def" | "mr" | "dex" | "agi";
export type BaseStats = Record<StatKind, number>;

export interface Awakening {
  stage: number;
  eternal_level: number;
}

export interface GameCharacter {
  id: string;
  name: string;
  armor_classes: ArmorClass[];
  wrist_types: WristType[];
}

export type SkillDependency = "stab" | "hack" | "int" | "mr" | "stab_hack" | "hack_int";

// 対象指定(crates/domain/src/skill.rs の SkillTarget)。
export type SkillTarget = "single" | "area";

export type ComboSkillType = "general" | "instant" | "chain";

export interface ComboSkillVariant {
  combo_type: ComboSkillType;
  multiplier: number;
  hit_count: number;
  base_actual_delay: number;
}

export interface Skill {
  id: string;
  name: string;
  dependency: SkillDependency;
  multiplier: number;
  hit_count: number;
  critical_multiplier: number;
  /** スキルの属性 */
  element: Element;
  /** 単体 / 範囲(wiki スキル性能一覧の対象指定)。null = wiki と突き合わせできなかった */
  target: SkillTarget | null;
  /** スキル命中(wiki 表記 +15 済みの実値)。null = wiki 未記載 */
  accuracy: number | null;
  /** スキルクリティカル率(wiki スキル性能一覧の Cri値)。null = wiki 未記載 */
  critical_rate: number | null;
  /** スキル Lv(wiki スキル性能一覧の SLv) */
  level: number;
  /** 単体チャネリングスキルか。極限スキル「フルスロットル」の段数増加はこれにだけ乗る */
  single_target_channeling: boolean;
  /** 基本中ディレイ(秒)。wiki スキル性能一覧の「動作」列。null = 秒として読めない */
  base_actual_delay: number | null;
  /** 中ディレイが固定で減少が効かない(wiki の「(固定)」表記) */
  actual_delay_fixed: boolean;
  /** 対応するコンボスキルタイプ。空ならタイプ選択非対応 */
  combo_variants: ComboSkillVariant[];
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
  /** 対象のAGI(wiki 狩り場情報一覧「敵AGI+固定値」)。null = wiki 未記載 */
  agi: number | null;
  /** 対象のクリティカル被撃率A(負値)。null = wiki 未記載 */
  critical_taken_rate: number | null;
}

// ペット S スキルの段階(wiki: PET)。crates/domain/src/stat_sources.rs の PetSkillTier(snake_case)。
export type PetSkillTier = "basic" | "true_lv1" | "true_lv2" | "true_lv3" | "true_lv4";
// ペット S スキル。ステごとに 1 つ(上位段階を選ぶと置き換わる)。未選択は null。
export type PetSkills = Record<StatKind, PetSkillTier | null>;
// ルーンスキル。ステごと 0..=20。
export type RuneLevels = Record<StatKind, number>;
// クラウン。通常はステごと 0..=100、選択報酬の 1 ステだけ 0..=300。値は +10 刻み。
export type Crown = Record<StatKind, number> & { selected_stat: StatKind | null };
// モンスターカード(カード装着)。ステごと 0..=70、固定値層。
export type MonsterCards = Record<StatKind, number>;
// 神鳥の聖物。ステごと 0..=40 段階(実加算値は段階×10)。
export type SacredRelic = Record<StatKind, number>;

// 能力値計算の5レイヤー(wiki §2)。crates/domain/src/stats.rs の StatLayer(snake_case)。
export type StatLayer = "percent_of_base" | "fixed" | "multiplier_a" | "multiplier_b" | "final_fixed";

// バフの対象ステ。crates/domain/src/stat_sources.rs の BuffTarget(rename_all snake_case、外部タグ付け)。
export type BuffTarget = "all_stats" | { stat: StatKind } | "user_selected" | { stats: StatKind[] };

// バフの値の決め方。crates/domain/src/stat_sources.rs の BuffValue(rename_all snake_case、外部タグ付け)。
export type BuffValue =
  | { fixed: number }
  | { choice: number[] }
  | { user_input: { min: number; max: number } }
  /** 記録するだけ(wiki に効果はあるが未配線)。マスタリーの段の状態を表すために選べる */
  | "record_only";

// crates/domain/src/candidate.rs の CandidateCost。「おすすめ強化」候補の手間タグ(表示専用)。
export type CandidateCost = "quick_win" | "enchant" | "equipment_update" | "enhance" | "aura";

// list_upgrade_candidates の戻り値 1 件(src-tauri の UpgradeCandidate)。
// 列挙・並び順は Rust 側(domain::candidate)。UI は上位 N 件を表示し、行クリックで
// `applied` をそのまま app.sim に入れて計算タブへ遷移する。
export interface UpgradeCandidate {
  id: string;
  label: string;
  cost: CandidateCost;
  per_hit_primary: number;
  delta_pct: number;
  /** 必要 /hit 以上か。need_per_hit の無いコンテンツでは常に false */
  reaches: boolean;
  applied: NewCharacter;
}

/** 選ぶ人の目的。1つのバフが複数に所属できる。 */
export type BuffPurpose = "stats" | "damage" | "durability";
/** 効果を得る場所の手掛かり。 */
export type BuffOrigin = "item" | "event" | "club" | "skill" | "rune" | "soul_link" | "battle_state" | "minigame";

export interface BuffDefinition {
  id: string;
  name: string;
  purposes: BuffPurpose[];
  origin: BuffOrigin;
  target: BuffTarget;
  layer: StatLayer;
  value: BuffValue;
  exclusive_slots: string[];
  source_url: string;
  note: string;
  /** BuffValue::UserInput の初期値。それ以外は null */
  default_value: number | null;
  /** ステ増加以外の効き先(与ダメージ式のカテゴリ)。同じバフが 2 か所に効くことがある */
  damage_effects: SkillEffect[];
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

export interface BuffSet {
  id: number;
  name: string;
  choices: BuffSelection;
}

// crates/storage/src/damage_snapshot_repository.rs。前回起動時のダメージ計算記録(1キャラ1件、履歴なし)。
export interface DamageSnapshot {
  character_id: number;
  skill_id: string;
  content_id: string;
  per_hit: number;
  taken_at: string;
}

export interface StatAdjustment {
  /** このステに +N する(固定値層への加算) */
  add: number;
  /** Some のとき最終能力値をこの値に固定する */
  pin: number | null;
}
export type Adjustments = Record<StatKind, StatAdjustment>;

// crates/domain/src/soul_link.rs。リンクステータス 1〜10 の現在 Lv。
export interface SoulLinkStatus {
  thrust_level: number;
  slash_level: number;
  magic_attack_level: number;
  magic_defense_level: number;
  critical_damage_level: number;
  final_damage_level: number;
  weapon_enhance_level: number;
  armor_enhance_level: number;
}

export interface SoulLinkPreview {
  equipment_values: EquipmentValues;
  critical_damage_rate: number;
  final_damage_rate: number;
  weapon_added_damage_multiplier: number;
  armor_added_hp_rate: number;
}

export interface StatSources {
  pet_skills: PetSkills;
  rune_levels: RuneLevels;
  crown: Crown;
  /** モンスターカード(wiki: ステータス「カード装着」)。ステごと 0〜70、固定値層 */
  monster_cards: MonsterCards;
  sacred_relic: SacredRelic;
  /** 装備の属性強化以外の属性値の供給源 */
  elements: ElementSources;
  /** ON にしているキャラスキル(パッシブ・自己バフ・味方バフ) */
  character_skills: CharacterSkills;
  /** 選んでいるマスタリー(段ごとに 1 つ) */
  masteries: Masteries;
  /** クリティカル率の供給源(wiki: 計算式まとめ #CriticalChance) */
  critical_rate: CriticalRateSources;
  /** ソウルリンク 1〜10。1〜4は装備基本能力、5〜7は戦闘計算、8〜10は記録用 */
  soul_link: SoulLinkStatus;
}

// crates/domain/src/critical_rate.rs の CriticalRateSources。
export interface CriticalRateSources {
  /** ペット会心(クリティカル率 ×1.1) */
  pet: boolean;
  /** 極のルーン(+20) */
  ultimate_rune: boolean;
  /** 設計者の研究室 B グループ「クリティカル率増加」の研究段階 0..=10(1 段階 +3) */
  architect_lab_stage: number;
  /** 致命打(+100) */
  deadly_blow: boolean;
}

// crates/domain/src/critical_rate.rs の CriticalRate。
export interface CriticalRate {
  equipment_critical: number;
  agi: number;
  target_agi: number;
  /** AGI 由来の部分 */
  from_agi: number;
  /** シエナのオーラの追加オプション「クリティカル確率」の Σ%(小数表現) */
  siena_rate: number;
  /** スキルクリティカル率(Cri値) */
  skill: number;
  /** クリティカル率増加(上限 +100%) */
  bonus: number;
  /** 対象のクリティカル被撃率A(負値) */
  target_taken_rate: number;
  /** 下限 0% / 上限 100% を掛ける前 */
  raw: number;
  /** クリティカル率(%) */
  value: number;
}

// 与ダメージ式のカテゴリ。crates/domain/src/category.rs の DamageCategory(rename_all snake_case、
// ALL の 36 variant)と正確に一致させる(任意文字列を許すエスケープハッチは持たず exhaustive check を効かせる)。
// 表示名は Rust 由来(StatLimits.damage_category_labels / CategoryTrace.label)を使う。
export type DamageCategory =
  | "attack_power"
  | "attack_random"
  | "target_defense"
  | "skill_multiplier"
  | "skill_multiplier_rate"
  | "skill_multiplier_fixed"
  | "critical_multiplier"
  | "critical_damage_rate"
  | "combo_bonus"
  | "element_bonus"
  | "player_cut_rate"
  | "siena_aura_attack_rate"
  | "final_damage_fixed"
  | "final_damage_rate"
  | "cut_rate_a"
  | "damage_reduction"
  | "attack_damage_legacy"
  | "awakening_damage"
  | "physical_magic_damage_rate"
  | "dependency_damage_rate"
  | "damage_absorb"
  | "taken_damage_rate"
  | "taken_damage_reduction"
  | "damage_amplify"
  | "damage_resistance"
  | "damage_mitigation"
  | "cut_rate_b"
  | "basic_trigger_damage_fixed"
  | "attack_damage_rate"
  | "attack_damage_isabel"
  | "attack_damage_general"
  | "attack_damage_basic_trigger"
  | "attack_damage_skill"
  | "attack_damage_special"
  | "attack_damage_japan"
  | "pvp_correction";

// スキル・マスタリー・バフの効き先。crates/domain/src/character_skill.rs の SkillEffect。
export type SkillEffect =
  | { stat_rate: { stats: StatKind[]; percent: number; layer: StatLayer } }
  | { actual_delay: { percent: number } }
  /** 与ダメージ式のカテゴリへの加算(上限はカテゴリ側が持つ) */
  | { damage: { category: DamageCategory; percent: number } }
  /** 記録するだけ(防御側・確率発動・条件付きで未配線) */
  | "record_only";

// マスタリー 1 つ。crates/domain/src/mastery.rs の MasteryDef。list_masteries の戻り値。
export interface MasteryDef {
  id: string;
  game_character_id: string;
  /** 段 1..=4(wiki のスキル表の (M1)〜(M4))。段ごとに 1 つだけ選ぶ */
  tier: number;
  name: string;
  effect: SkillEffect;
  note: string;
}

// 選んでいるマスタリー。crates/domain/src/mastery.rs の Masteries。
export interface Masteries {
  /** MasteryDef.id。段ごとに 1 つ */
  picked: string[];
}

// 誰に効くか。crates/domain/src/character_skill.rs の SkillAudience。
export type SkillAudience = "self_only" | "ally";

// マスタリーによる効果の差し替え。crates/domain/src/character_skill.rs の MasteryOverride。
export interface MasteryOverride {
  mastery_id: string;
  effects: SkillEffect[];
}

// キャラスキル 1 つ。crates/domain/src/character_skill.rs の CharacterSkillDef。
// list_character_skills の戻り値。
export interface CharacterSkillDef {
  id: string;
  game_character_id: string;
  name: string;
  audience: SkillAudience;
  /** マスタリー未取得のときの効果。空 = マスタリーを取ってはじめて効果が出る */
  effects: SkillEffect[];
  /** マスタリーを取ると効果が差し替わる(上から順に最初に一致したもの) */
  mastery_overrides: MasteryOverride[];
  source_url: string;
  note: string;
}

export interface CharacterSkills {
  /** ON にしている CharacterSkillDef の id */
  skill_ids: string[];
}

// crates/domain/src/actual_delay.rs の ActualDelayContribution / ActualDelay。
export interface ActualDelayContribution {
  source: string;
  /** Σ% の小数表現(−5% → 0.05) */
  rate: number;
}

export interface ActualDelay {
  /** 基本中ディレイ(秒) */
  base: number;
  /** 上限前の中ディレイ減少値 */
  reduction_raw: number;
  /** 上限(70%)適用後の中ディレイ減少値 */
  reduction: number;
  /** 倍率A(コンボボーナス)。2 コンボ以上で 0.5 */
  combo_rate: number;
  /** 下限 0.3s を掛ける前の中ディレイ(秒) */
  raw: number;
  /** 中ディレイ(秒)。下限 0.3s 適用後 */
  value: number;
  /** 下限 0.3s で頭打ちになったか */
  floored: boolean;
  /** wiki が「(固定)」と書いている中ディレイ(減少が効かない) */
  fixed: boolean;
  contributions: ActualDelayContribution[];
  /** 60 秒あたりのスキル回数。DPS はこれから出す */
  uses_per_minute: number;
  /** 実測表(計測データ)由来か。false = 60 / 中ディレイ の式から出した */
  uses_measured: boolean;
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
  /** この要因がそのステの最終能力値を何ポイント動かしたか(実数) */
  effect: number;
}

/** ステ攻撃力に実際に使っている依存ステ 1 つぶん(crates/domain/src/attack_power.rs) */
export interface StatAttackPart {
  kind: StatKind;
  /** そのステの最終能力値 */
  effective: number;
  coefficient: number;
  /** 最終能力値 × 係数 */
  contribution: number;
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

// crates/domain/src/validation.rs の ValidationLocation。検証エラーが「どこの話か」を運ぶ。
// エラー帯はこれを使って該当部位の詳細まで飛ぶ。
export type ValidationLocation = {
  slot: PartSlot;
  partId: number;
  abilityId: string | null;
  randomOptionId: string | null;
};

// 装備部位。crates/domain/src/equipment.rs の PartSlot(snake_case)。
export type PartSlot =
  | "weapon" | "armor" | "helm" | "shield" | "shield_plus"
  | "head" | "body" | "hand" | "leg" | "effect" | "artifact"
  | "relic_pendant" | "relic_bracelet";

// シエナのオーラのステ加算。crates/domain/src/equipment.rs の SienaStatBonus。
export type SienaStatBonus = Record<StatKind, number>;

// シエナのオーラの能力値スロットの種類。crates/domain/src/siena.rs の SienaValueKind。
export type SienaValueKind =
  | "thrust" | "slash" | "magic_attack" | "magic_defense"
  | "physical_composite" | "magic_slash"
  | "physical_resist" | "magic_resist" | "critical_taken_reduction"
  | "accuracy" | "evasion"
  | "stab" | "hack" | "int" | "def" | "mr" | "dex" | "agi";

// シエナのオーラの追加オプションの種類。crates/domain/src/siena.rs の SienaExtraKind。
export type SienaExtraKind =
  | "attack_rate" | "defense_rate" | "defense_ignore_chance" | "actual_delay"
  | "all_stats" | "critical_rate" | "hp" | "mp" | "sp";

// 能力値スロット 1 個。crates/domain/src/siena.rs の SienaSlot。
export interface SienaSlot {
  kind: SienaValueKind;
  value: number;
}

// 追加オプションスロット 1 個。crates/domain/src/siena.rs の SienaExtraSlot。
export interface SienaExtraSlot {
  kind: SienaExtraKind;
  value: number;
}

// シエナのオーラ(部位ごと)。crates/domain/src/siena.rs の SienaAura。
// **増幅段階は slots.length**(別に持たない)。
export interface SienaAura {
  /** 解放済み能力値スロットの中身。並び順は入力順 */
  slots: SienaSlot[];
  /** 解放済み追加オプションスロットの中身 */
  extras: SienaExtraSlot[];
}
export interface RegisteredSienaAura { id: number; label: string; aura: SienaAura; }
export interface SienaAuraList { registered: RegisteredSienaAura[]; selected_id: number | null; }
export interface SienaAuras {
  weapon: SienaAuraList;
  armor: SienaAuraList;
  helm: SienaAuraList;
  shield: SienaAuraList;
  head: SienaAuraList;
  body: SienaAuraList;
  hand: SienaAuraList;
  leg: SienaAuraList;
}

// 画面が選択肢を並べるためのカタログ。crates/domain/src/siena.rs の SienaCatalog。
export interface SienaValueKindDef {
  kind: SienaValueKind;
  label: string;
  /** 一覧の行に並べる短い名前 */
  short: string;
  /** 武器/盾の一覧か(false = その他の部位の一覧) */
  is_equipment_value: boolean;
  min: number;
  max: number;
  unit: string;
  /** 与ダメージ式に入るか(false = 記録するだけ) */
  is_modeled: boolean;
  note: string;
}
export interface SienaExtraKindDef {
  kind: SienaExtraKind;
  label: string;
  /** 一覧の行に並べる短い名前 */
  short: string;
  /** 取りうる値そのもの(飛び飛びなので min/max では表せない) */
  choices: number[];
  unit: string;
  is_modeled: boolean;
  note: string;
}
export interface SienaCatalog {
  values: SienaValueKindDef[];
  extras: SienaExtraKindDef[];
  /** 追加オプションが 1 個ずつ解放される段階 */
  extra_unlock_stages: [number, number, number];
  stage_max: number;
}

// ランダムオプションのランク。crates/domain/src/random_option.rs の RandomOptionRank。
export type RandomOptionRank = "normal" | "valuable" | "rare" | "special" | "s_true";

// ランダムオプションの効き先。crates/domain/src/random_option.rs の RandomOptionEffect。
// タプル variant(依存別)は serde が { dependency_damage_rate: SkillDependency } になる。
export type RandomOptionEffect =
  | { dependency_damage_rate: SkillDependency }
  | "attack_damage_rate"
  | "added_damage_rate"
  | "physical_added_damage_rate"
  | "magic_added_damage_rate"
  | "physical_damage_amplify"
  | "magic_damage_amplify"
  | "accuracy_point"
  | "evasion_point"
  | "accuracy_and_evasion_point"
  | "actual_delay_reduction"
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
  /** 一覧のバッジに出す短い名前(「一般ボス」「魔攻」など) */
  short: string;
  slot: PartSlot;
  /** wiki 一覧表のカテゴリー番号。同じ番号は 1 部位に 1 つまで(0 は制約なし) */
  category: number;
  effect: RandomOptionEffect;
  tiers: RandomOptionTier[];
  note: string;
  /** 実際によく付ける OP。画面はこれをチップで先に出す */
  common: boolean;
}

// キャラが付けている 1 枠。crates/domain/src/random_option.rs の RandomOptionSlot。
export interface RandomOptionSlot {
  option_id: string;
  rank: RandomOptionRank;
  /** 実測値の上書き。null = レンジ上限 */
  value: number | null;
}

// 極限スキル(wiki: Skill/極限)。crates/domain/src/ultimate_skill.rs の UltimateSkill。
export type UltimateSkill = "scope_eye" | "full_throttle" | "wide_focus";

// 極限スキル一式。crates/domain/src/ultimate_skill.rs の UltimateSkills。
export interface UltimateSkills {
  /** 選んだ極限スキル(2 枠)。同じスキルは 2 枠に入れられない */
  slots: (UltimateSkill | null)[];
  /** スーパーリミット(ハイパーアタックの極限形。Lv1 のみ) */
  super_limit: boolean;
  /** ハイパーリミットの Lv(0〜6)。Lv2 以降はオーグメントの Lv が要る */
  hyper_limit_level: number;
}

// 共通スキル(wiki: Skill/共通)。crates/domain/src/common_skill.rs の CommonSkills。
export interface CommonSkills {
  /** パワーウェポン(Lv1)。装備攻撃力強化倍率 +2% */
  power_weapon: boolean;
  /** ストロングウェポンの Lv(0〜6)。Lv2 以降はオーグメントの Lv が要る */
  strong_weapon_level: number;
  /** コートアーマー(Lv1)。装備防御力倍率 物+18% / 魔+12% */
  coat_armor: boolean;
  /** プロテクトアーマーの Lv(0〜6)。Lv2 以降はオーグメントの Lv が要る */
  protect_armor_level: number;
  /** 改・プロテクトアーマーの Lv(0〜5) */
  kai_protect_armor_level: number;
  /** シャープネスビジョンの Lv(0〜10)。割合追加ダメージ */
  sharpness_vision_level: number;
  /** オーグメントの Lv(0〜5)。前提スキル */
  augment_level: number;
  /** アンリーシュ(能力解放)の 2 枠。選んだステの能力値倍率B に乗る */
  unleash: [UnleashSlot, UnleashSlot];
  /** レインフォースの Lv(0〜5)。前提スキルで、アンリーシュの Lv6 以降に要る */
  reinforce_level: number;
  /** 極限スキル(wiki: Skill/極限)。2 枠 + スーパーリミット / ハイパーリミット */
  ultimate: UltimateSkills;
}

// アンリーシュ(能力解放)の 1 枠。crates/domain/src/common_skill.rs の UnleashSlot。
export interface UnleashSlot {
  /** 解放するステ。null = この枠は未使用 */
  stat: StatKind | null;
  /** Lv(0〜10)。Lv6 以降はレインフォースの Lv が要る */
  level: number;
}

// 装備防御力倍率。crates/domain/src/common_skill.rs の DefenseRates。
export interface DefenseRates {
  physical: number;
  magic: number;
}

// 称号の区分。crates/domain/src/title.rs の TitleKind。
export type TitleKind = "normal" | "special" | "event";

// 称号定義(gamedata のカタログ)。crates/domain/src/title.rs の TitleDef。
export interface TitleDef {
  id: string;
  name: string;
  kind: TitleKind;
  /** wiki の見出し(グループボーナスの単位。ボーナス自体は未実装) */
  group: string;
  /** 習得 Lv。wiki が `-` の行は null */
  level: number | null;
  /** 装備の基本能力値への加算 */
  values: EquipmentValues;
  /** 無条件の「ダメージ n% 増加」。カテゴリX(攻撃ダメージ)の X3 基本発動に入る。単位は % */
  attack_damage_percent: number;
  /** 特定地域または敵でだけ発動する割合追加ダメージ */
  conditional_added_damage: ConditionalAddedDamage | null;
  /** 入手方法・備考 */
  note: string;
}

export type GameRegion = "lost_island" | "shinchou_nest" | "arklon_underground" | "praba";
export type AddedDamageCondition = { region: GameRegion } | { enemy: string };
export interface ConditionalAddedDamage {
  percent: number;
  condition: AddedDamageCondition;
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

// セット効果(wiki: コアセット効果)。最終ダメージの固定加算(K)と割合(L)。
// crates/domain/src/thesis_core.rs の CoreSetBonus。
export interface CoreSetBonus {
  /** 最終ダメージの固定加算(合算後) */
  final_damage_fixed: number;
  /** 最終ダメージの割合。Σ% の小数表現(合算後。0.03 = +3%) */
  final_damage_rate: number;
}

// 進化段階ごとに成立したセット効果の内訳。crates/domain/src/thesis_core.rs の CoreSetGroup。
export interface CoreSetGroup {
  evolution: number;
  /** 成立に使った枚数(3〜6) */
  count: number;
  bonus: CoreSetBonus;
}

// 装備部位 1 つ。crates/domain/src/equipment.rs の EquipmentPart。
export interface EquipmentPart {
  id: number;
  label: string;
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
  /** 固定ダメージ補正式。カタログ品は自動、カタログ外はユーザー選択。 */
  enhance_type: EquipmentEnhanceType | null;
  /** +12 以上の追加固定ダメージ実測値の上書き。+11 以下は null 固定 */
  enhance_grade: EnhanceGrade | null;
  /** 装備アビリティ id */
  abilities: string[];
  /** カフス・レリック等のアビリティ本体の実測値。 */
  ability_values: EquipmentAbilityAdditional[];
  /** カテゴリー4アビリティで実際に抽選された追加アビリティ。 */
  ability_additions: EquipmentAbilityAdditional[];
  /** ランダムオプション。同じカテゴリーは 1 部位に 1 つまで */
  random_options: RandomOptionSlot[];
}

export type EnhanceGrade = "lowest" | "low" | "middle" | "high" | "highest";
export type EquipmentEnhanceType =
  | "weapon_stab" | "weapon_stab_hack" | "weapon_hack" | "weapon_int" | "weapon_int_hack" | "weapon_mr"
  | "armor_light" | "armor_heavy" | "armor_magic" | "armor_suit" | "armor_robe";
export interface EquipmentPartList { registered: EquipmentPart[]; selected_id: number | null; }

// 12 部位。crates/domain/src/equipment.rs の EquipmentParts(named field)。
export interface EquipmentParts {
  weapon: EquipmentPartList;
  armor: EquipmentPartList;
  helm: EquipmentPartList;
  shield: EquipmentPartList;
  shield_plus: EquipmentPartList;
  head: EquipmentPartList;
  body: EquipmentPartList;
  hand: EquipmentPartList;
  leg: EquipmentPartList;
  effect: EquipmentPartList;
  artifact: EquipmentPartList;
  relic_pendant: EquipmentPartList;
  relic_bracelet: EquipmentPartList;
}

// 装備補正一式(部位別装備 13 スロット + パワーウェポン/ストロングウェポン)。
// crates/domain/src/equipment.rs の Equipment。
export interface Equipment {
  parts: EquipmentParts;
  /** 抽出・注入で装備とは独立して付け替える、部位別の登録オーラ。 */
  siena: SienaAuras;
  /** テシスコア(地域ごとに 6 枠) */
  thesis_cores: ThesisCores;
  /** 表示中の称号(TitleDef.id)。1 枠だけ・補正は基本能力値へ合流。null = 未装備 */
  title: string | null;
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
  | "katana" | "axe" | "whip" | "kara" | "dual_blade_physical" | "scythe" | "arming_sword" | "sword_shape"
  | "magic_wand" | "wand" | "magic_gun" | "scepter" | "totem"
  | "great_sword"
  | "holy_staff" | "handbell" | "dual_blade_magic" | "hammer";
export type WeaponSystem = "stab" | "stab_hack" | "hack" | "int" | "int_hack" | "mr";
export type ArmorClass = "light" | "heavy" | "magic" | "suit" | "robe";
// crates/domain/src/equipment.rs の PartEquipmentValues。部位キー付きの装備補正値(表示用)。
export interface PartEquipmentValues {
  slot: PartSlot;
  values: EquipmentValues;
}
// crates/domain/src/equipment.rs の PartSlotRule。部位ごとの枠数・可否ルール(ドラフト非依存、
// PartSlot の同名メソッドの写し)。唯一の正 — labels.ts の可否テーブルはここを参照する。
export interface PartSlotRule {
  slot: PartSlot;
  label: string;
  ability_slots: number;
  allows_ability: boolean;
  allows_enhance: boolean;
  allows_siena: boolean;
  siena_counts_as_equipment: boolean;
  allows_random_option: boolean;
  random_option_slots: number | null;
  allows_element: boolean;
}
export type WristType =
  | "shield" | "spellbook" | "knuckle" | "band" | "bracelet" | "pendulum" | "crystal_ball"
  | "dual_blade_physical" | "physical_magazine" | "magic_magazine" | "dual_blade_magic";

export type EquipmentSurvivalEffect =
  | { damage_mitigation: { percent: number } }
  | { defense_rate: { percent: number } }
  | { defense_fixed: { value: number } };

// 装備カタログの 1 アイテム。crates/gamedata/src/equipment_catalog.rs の EquipmentItem。
export interface EquipmentItem {
  id: string;
  slot: PartSlot;
  name: string;
  /** 基本能力値のレンジ下限(wiki: Item ページの MR レンジ) */
  values_min: EquipmentValues;
  /** 基本能力値のレンジ上限 */
  values_max: EquipmentValues;
  /** 成長装備の各基本能力値の入力上限。通常装備は null */
  growth_cap: number | null;
  /** 補正ごとの成長上限。通常装備は null */
  growth_caps: EquipmentValues | null;
  /** この装備品に付与できるアビリティ枠数。 */
  ability_slots: number;
  /** この装備品に付与できるランダムオプション枠数。対象外は null。 */
  random_option_slots: number | null;
  /** 装備固有の固定エンチャント枠。実物の装備本体補正には左右されない。 */
  enchant_caps: EquipmentValues;
  /** 腕装備の区分。バンド固有パッシブの判定元。腕以外は null。 */
  wrist_type: WristType | null;
  /** 武器のみ非 null */
  weapon_class: WeaponClass | null;
  /** gamedata WeaponSystem。主軸スキルとの候補照合に使う単一ソース。 */
  weapon_system: WeaponSystem | null;
  /** 装備強化の固定ダメージ補正式。 */
  enhance_type: EquipmentEnhanceType | null;
  /** 鎧のみ非 null(`armor_class_for_type(enhance_type)`)。キャラの装備可能クラスとの突き合わせに使う。 */
  armor_class: ArmorClass | null;
  /** 装着時効果(wiki: Item ページ備考の「装着時 …」)。与ダメージ式のカテゴリに入る */
  damage_effects: SkillEffect[];
  /** 被ダメージ側へ効く耐久効果。与ダメージ計算とは分離する。 */
  survival_effects: EquipmentSurvivalEffect[];
  /** 主軸スキルに合う候補を先に出すための依存種別。 */
  recommended_dependency: SkillDependency | null;
  /** 装着時効果がこの依存のスキルだけに効く場合の条件。 */
  damage_dependency: SkillDependency | null;
  source: Source;
}

// 武器アビリティの効果系統。候補を武器系統へ絞るために使う。
export type EquipmentAbilityFamily =
  | "pointed_blade" | "sharp_blade" | "intelligence" | "magic_resistance" | "weapon_delay"
  | "armor_polish" | "vitality" | "mana" | "evasion" | "shield_polish" | "critical"
  | "accuracy" | "element" | "agility" | "skill_attack";
export type EquipmentAbilityAdditionalKind =
  | "fixed_damage" | "damage_rate" | "thrust" | "slash" | "magic_attack" | "magic_defense"
  | "hp_recovery" | "mp_recovery" | "accuracy" | "physical_defense" | "critical" | "evasion"
  | "damage_resistance" | "physical_damage_reduction" | "magic_damage_reduction" | "sp_recovery"
  | "evasion_rate" | "fire_element" | "water_element" | "wind_element" | "earth_element"
  | "lightning_element" | "white_element" | "dark_element";
export interface EquipmentAbilityAdditional {
  ability_id: string;
  kind: EquipmentAbilityAdditionalKind;
  value: number;
}
export interface EquipmentAbilityAdditionalDef {
  kind: EquipmentAbilityAdditionalKind;
  min: number;
  max: number;
}

// 武器アビリティ定義。crates/domain/src/equipment.rs の EquipmentAbilityDef。
export interface EquipmentAbilityDef {
  slot: PartSlot;
  value_option: EquipmentAbilityAdditionalDef | null;
  exclusive_group: string;
  additional_slots: number;
  additional_effects: string;
  additional_options: EquipmentAbilityAdditionalDef[];
  record_only: boolean;
  family: EquipmentAbilityFamily;
  /** 同じカテゴリーは同一装備に1つまで。 */
  category: number;
  id: string;
  name: string;
  effect_summary: string;
  /** 装備攻撃力(基本能力値)への加算値 */
  values: EquipmentValues;
  /** 追加効果(R- 以上に付く「ダメージ増加 +n%」。カテゴリX3) */
  damage_effects: SkillEffect[];
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
  /** 共通スキル(wiki: Skill/共通) */
  common_skills: CommonSkills;
  default_buff_set_id: number | null;
  /** 最終保存日時(ISO8601 UTC)。この列より前に作られたキャラは null(表示しない) */
  updated_at: string | null;
}

export interface NewCharacter {
  name: string;
  game_character_id: string;
  base_stats: BaseStats;
  awakening: Awakening;
  stat_sources: StatSources;
  equipment: Equipment;
  /** 共通スキル(wiki: Skill/共通) */
  common_skills: CommonSkills;
  main_skill_id: string | null;
  default_buff_set_id: number | null;
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
  /** 最終能力値(上限適用後) */
  effective: number;
  /** 最終能力値の上限(覚醒段階 + エタの意志 Lv で 1,500〜2,400) */
  stat_cap: number;
  /** 上限で捨てられた分。0 なら上限に当たっていない */
  capped_loss: number;
  /** pin(能力値の固定。計算タブの一時調整のみ)が適用された場合の上書き前の値。未適用は null */
  pinned_from: number | null;
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
  /** 強化倍率で足される分 [装備攻撃力/25 × 倍率] × 25 */
  enhance_bonus: number;
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
  /** 共通スキル(Skill/共通・Skill/極限)の効き先サマリ */
  common_skill: CommonSkillPreview;
  /** クリティカル率増加(計算式まとめ #CriticalChance)の合計 */
  critical_rate_bonus: CriticalRateBonusPreview;
  /** 神鳥の聖物の段階→最終固定値換算の合計(Σ) */
  sacred_relic_total: number;
  /** ソウルリンク 1〜10 の Rust 計算済み派生値 */
  soul_link: SoulLinkPreview;
  /** 基本能力値の合計(Σ part.base + 装備アビリティ + 表示中の称号 + ソウルリンク) */
  equipment_base_total: EquipmentValues;
  /** 基本能力値のうち装備アビリティ由来の分だけを部位別に割ったもの(表示用の内訳) */
  part_ability_values: PartEquipmentValues[];
  /** シエナのオーラの能力値スロットの装備補正(部位別。武器/盾以外は常に 0) */
  siena_part_values: PartEquipmentValues[];
  /** テシスコアの地域別プレビュー(CoreRegion 4 件) */
  thesis_cores: ThesisCoreRegionPreview[];
}

// crates/domain/src/stat_sources.rs の ThesisCoreRegionPreview。テシスコア 1 地域ぶんの表示用プレビュー。
export interface ThesisCoreRegionPreview {
  region: CoreRegion;
  /** 6 枠の補正値合計(入場条件「コア N」と同じ値) */
  total_bonus: number;
  /** 強化能力値への加算(火力 + 補助) */
  values: EquipmentValues;
  /** 成立しているセット(進化段階ごと)。空なら未発動 */
  set_groups: CoreSetGroup[];
  /** 進化を問わず強化 4 に達しているコアの数 */
  ready: number;
  /** この地域のセット効果の合計(合算後) */
  set_bonus: CoreSetBonus;
}

// crates/domain/src/stat_sources.rs の CommonSkillPreview。
export interface CommonSkillPreview {
  /** 装備防御力倍率(コートアーマー + プロテクトアーマー + 改・プロテクトアーマー + シエナのオーラの防御力増加)。1.0 が中立値の乗数 */
  defense_rates: DefenseRates;
  /** 装備攻撃力強化倍率(パワーウェポン + ストロングウェポン)。Σ% の小数表現 */
  equipment_attack_rate: number;
  ultimate: UltimateSkillPreview;
}

// crates/domain/src/stat_sources.rs の UltimateSkillPreview。
export interface UltimateSkillPreview {
  /** スコープアイのクリティカルダメージ増加。Σ% の小数表現 */
  critical_damage_rate: number;
  /** フルスロットルの中ディレイ減少。Σ% の小数表現 */
  actual_delay_reduction: number;
  /** フルスロットルの単体チャネリングスキル段数増加 */
  added_hit_count: number;
  /** ワイドフォーカスのスキル範囲増加(火力には効かない) */
  skill_range_bonus: number;
}

// crates/domain/src/stat_sources.rs の CriticalRateBonusPreview。
export interface CriticalRateBonusPreview {
  /** 上限を掛ける前の合計(「頭打ち」表示に使う) */
  raw: number;
  /** 上限 +100% を掛けた合計 */
  value: number;
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
  /** 適用した装備防御力倍率(共通スキル + シエナのオーラの防御力増加) */
  defense_rates: DefenseRates;
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
  /** この段を終えた時点の到達値(式の途中積)。式の外の段は value と同じ */
  reached: number;
  /** この段が消費したカテゴリ(wiki §3)。式の外で足す段(攻撃力の内訳など)は空 */
  categories: DamageCategory[];
}

// crates/domain/src/damage.rs の DamageContribution。カテゴリ集計 1 行ぶんの供給源
// (スキル名・マスタリー名・バフ名・アビリティ名など)。「なぜこの数字?」パネルの
// カテゴリ材料行を掘り下げたときの供給源表に使う
export interface DamageContribution {
  source: string;
  category: DamageCategory;
  value: number;
}

// crates/domain/src/common_skill.rs の RateContribution。割合供給源 1 行(トレース表示用)。
export interface RateContribution {
  source: string;
  value: number;
}

// crates/domain/src/equipment.rs の EquipmentAttackLayer。
export type EquipmentAttackLayer = "base" | "enhanced";

// crates/domain/src/equipment.rs の EquipmentValueKind。装備攻撃力に効く装備値種別。
export type EquipmentValueKind = "thrust" | "slash" | "magic_attack" | "magic_defense";

// crates/domain/src/equipment.rs の EquipmentAttackSource。装備攻撃力の内訳 1 行に効いた
// 供給源 1 件(部位実測値・部位アビリティ・称号・手首補正・エンチャント・シエナのオーラ・
// テシスコア)。Σamount = part.amount、Σcontribution = part.contribution
export interface EquipmentAttackSource {
  source: string;
  amount: number;
  contribution: number;
}

// crates/domain/src/equipment.rs の EquipmentAttackPart。装備攻撃力の内訳 1 行
// (層 × 装備値種別)。Σcontribution = 装備攻撃力
export interface EquipmentAttackPart {
  layer: EquipmentAttackLayer;
  value: EquipmentValueKind;
  amount: number;
  coefficient: number;
  contribution: number;
  /** この値に効いた供給源(非 0 のみ) */
  sources: EquipmentAttackSource[];
}

export interface DamageTriple {
  min: number;
  max: number;
  critical: number;
}

export interface DamageTrace {
  stats: StatTrace[];
  /** 攻撃力(A)の内訳 */
  attack: AttackPowerBreakdown;
  /** ステ補正源(ペット/ルーン/クラウン/聖物/バフ/調整値)の寄与内訳 */
  stat_contributions: StatContribution[];
  /** ステ攻撃力に効いている依存ステごとの内訳(合計 = ステ攻撃力) */
  stat_attack_parts: StatAttackPart[];
  categories: CategoryTrace[];
  /** カテゴリ集計に実際に値を足した供給源の一覧(非 0 のみ)。カテゴリ材料行の掘り下げに使う */
  category_contributions: DamageContribution[];
  /** 装備攻撃力の内訳(層 × 装備値種別)。Σcontribution = 装備攻撃力 */
  equipment_attack_parts: EquipmentAttackPart[];
  /** 装備攻撃力強化倍率の供給源(パワーウェポン/ストロングウェポン)。Σvalue = 強化倍率 */
  equipment_enhance_sources: RateContribution[];
  steps_min: FormulaStep[];
  steps_max: FormulaStep[];
  steps_critical: FormulaStep[];
}

export interface DamageResult {
  /**
   * 1 段あたりの与ダメージ(スキル分のみ。ダメージ上限を適用したあと)。
   * ゲームの表記ダメージに相当し、武器強化の追加固定ダメージは含まない
   */
  per_hit: DamageTriple;
  /** 実際に敵へ入る総量: per_hit × 段数 + weapon_added_per_hit × 段数 + 割合追加ダメージ */
  total: DamageTriple;
  /**
   * 武器の装備強化による追加固定ダメージ(1 段あたり)。与ダメージ式の外・ダメージ上限の対象外で、
   * ゲームは表記ダメージ(per_hit)と別枠で表示する
   */
  weapon_added_per_hit: number;
  /** 与ダメージ(表記ダメージ)の合計 = per_hit × 段数 */
  skill_total: DamageTriple;
  /** 武器強化の追加固定ダメージの合計 = weapon_added_per_hit × 段数 */
  weapon_added_total: number;
  /**
   * 主役の 1 段あたりダメージ(クリ発生率 > 0 ならクリティカル、0 なら非クリ最大。
   * ユーザー判断 2026-08-29)。ゲームの表記ダメージ(スキル分のみ)。計算タブ・ホームの表示、
   * コンテンツ到達判定はこの値を使う
   */
  per_hit_primary: number;
  /** 主役の合計ダメージ */
  total_primary: number;
  hit_count: number;
  /** コンボスキルタイプ解決後の倍率 */
  effective_skill_multiplier: number;
  /** コンボスキルタイプ解決後の基本中ディレイ */
  effective_base_actual_delay: number | null;
  /** 与ダメージの上限(1 段ごとに適用) */
  damage_cap: number;
  /** 上限で捨てられた分(1 段あたり)。すべて 0 なら上限に当たっていない */
  capped_loss: DamageTriple;
  /** 割合追加ダメージ(新-割合)の Σ%。いまの供給源はシャープネスビジョンのみ */
  added_damage_rate: number;
  /** 割合追加ダメージの実額。合計ダメージにだけ乗る */
  added_damage: DamageTriple;
  /** 命中P。敵の回避Pを 100 上回ると必中。null = スキル命中が wiki 未記載で出せない */
  accuracy_point: number | null;
  /** クリティカル率。null = 敵の AGI / クリティカル被撃率 / スキルの Cri値 のどれかが wiki 未記載 */
  critical_rate: CriticalRate | null;
  /** 中ディレイ。null = スキルの「動作」列が秒で取れず出せない */
  actual_delay: ActualDelay | null;
  /** 1 秒あたりの与ダメージ(合計 / 中ディレイ)。null = 中ディレイが出せない */
  dps: DpsTriple | null;
  /**
   * クリティカル率(0..1)。critical_rate が null(wiki 未記載)のときは
   * 1.0(クリティカル確定扱い。未記載は確定扱い(ユーザー判断 2026-08-29))
   */
  critical_chance: number;
  /** クリ率を考慮した DPS の期待値(dps.max × (1 − p) + dps.critical × p)。dps が null なら null */
  expected_dps: number | null;
  trace: DamageTrace;
}

export interface DpsTriple {
  min: number;
  max: number;
  critical: number;
}

// crates/domain/src/stat_sources.rs の StatLimits。get_stat_limits コマンドの戻り値。
export interface StatLimits {
  base_stat_max: number;
  rune_level_max: number;
  crown_base_max: number;
  crown_selected_max: number;
  crown_step: number;
  /** モンスターカードの 1 ステあたり上限 */
  monster_card_max: number;
  sacred_relic_stage_max: number;
  /** ソウルリンクのリンクステータス 1〜4 の Lv 上限 */
  soul_link_equipment_level_max: number;
  soul_link_critical_damage_level_max: number;
  soul_link_final_damage_level_max: number;
  soul_link_weapon_enhance_level_max: number;
  soul_link_armor_enhance_level_max: number;
  adjustment_add_min: number;
  adjustment_add_max: number;
  adjustment_pin_min: number;
  adjustment_pin_max: number;
  equipment_value_max: number;
  strong_weapon_level_max: number;
  /** 装備強化 Lv 上限(wiki: 装備システム/装備強化。+1〜+15) */
  enhance_level_max: number;
  /** +12 以上の追加固定ダメージ実測値の上限(実用上の安全域)`[仮]` */
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
  protect_armor_level_max: number;
  kai_protect_armor_level_max: number;
  sharpness_vision_level_max: number;
  augment_level_max: number;
  /** アンリーシュ(能力解放)の Lv 上限 */
  unleash_level_max: number;
  /** アンリーシュの枠数 */
  unleash_slots: number;
  /** レインフォースの Lv 上限(アンリーシュ Lv6 以降の前提) */
  reinforce_level_max: number;
  hyper_limit_level_max: number;
  /** クリティカル率増加の上限 %(wiki: 計算式まとめ #CriticalChance) */
  critical_rate_bonus_max: number;
  /** 設計者の研究室の研究段階の上限 */
  architect_lab_stage_max: number;
  /** 設計者の研究室 1 段階あたりのクリティカル率増加 */
  architect_lab_per_stage: number;
  /** 極のルーンのクリティカル率増加(最大レベル時) */
  ultimate_rune_bonus_max: number;
  /** 致命打のクリティカル率増加 */
  deadly_blow_bonus_max: number;
  /** パワーウェポンの装備攻撃力強化倍率。Σ% の小数表現 */
  power_weapon_rate: number;
  /** ストロングウェポン 1Lv あたりの装備攻撃力強化倍率。Σ% の小数表現 */
  strong_weapon_rate_per_level: number;
  /** コートアーマーの装備防御力倍率(物理 / 魔法)。Σ% の小数表現 */
  coat_armor_physical_rate: number;
  coat_armor_magic_rate: number;
  /** プロテクトアーマー Lv1〜6 の装備防御力倍率(物理 / 魔法)。Σ% の小数表現。index 0 = Lv1 */
  protect_armor_physical_rates: number[];
  protect_armor_magic_rates: number[];
  /** 改・プロテクトアーマー Lv1〜5 の装備防御力倍率(物理 / 魔法)。Σ% の小数表現。index 0 = Lv1 */
  kai_protect_armor_physical_rates: number[];
  kai_protect_armor_magic_rates: number[];
  /** シャープネスビジョン Lv1〜10 の割合追加ダメージ。Σ% の小数表現。index 0 = Lv1 */
  sharpness_vision_rates: number[];
  /** アンリーシュ Lv1〜10 の能力値倍率B。Σ% の小数表現。index 0 = Lv1 */
  unleash_rates: number[];
  /** ペット S スキルの段階ごとの固定値ボーナス */
  pet_skill_tier_bonus: PetSkillTierBonus[];
  /** 神鳥の聖物 1 段階あたりの最終固定値 */
  sacred_relic_value_per_stage: number;
  /** テシスコア・火力タイプの補正値テーブル(wiki: 進化強化表「火力」列)。添字は [進化段階][強化段階] */
  core_power_bonus_table: number[][];
  /** テシスコア・補助タイプの補正値テーブル(wiki: 進化強化表「補助」列) */
  core_support_bonus_table: number[][];
  /** 部位ごとの枠数ルール(装着アビリティ・ランダムオプション)。13 部位ぶん */
  part_slot_rules: PartSlotRule[];
  /** 与ダメージ式カテゴリの日本語名。36 カテゴリぶん、DamageCategory::ALL の順 */
  damage_category_labels: DamageCategoryLabel[];
  /** 装備補正 9 値の表示名。EquipmentValues::FIELD_LABELS の順(CoreType の表示名も同じ) */
  equipment_stat_labels: EquipmentStatLabel[];
}

// crates/domain/src/stat_sources.rs の DamageCategoryLabel。
export interface DamageCategoryLabel {
  category: DamageCategory;
  label: string;
}

// crates/domain/src/stat_sources.rs の EquipmentStatLabel。kind は EquipmentStatKind と同じ文字列。
export interface EquipmentStatLabel {
  kind: string;
  label: string;
}

// crates/domain/src/stat_sources.rs の PetSkillTierBonus。
export interface PetSkillTierBonus {
  tier: PetSkillTier;
  bonus: number;
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
  /** 称号などの地域限定効果を判定する、テシスコアとは別のゲーム内地域 */
  game_region: GameRegion | null;
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
  /** 1 ヒットの主役値(クリ発生率 > 0 ならクリティカル、0 なら非クリ最大) */
  per_hit_primary: number;
  /** 合計の主役値 = 1 ヒットの主役値 × 段数 */
  total_primary: number;
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
