// 装備値の共通ヘルパー(部位の装備本体レンジ中央・エンチャント許容量・表示用合計)。
// 計算・判定ロジックは Rust 側(crates/domain/src/equipment.rs)にあり、ここは表示・編集用の
// 単純な値組み立てのみ(CLAUDE.md「計算・判定は Rust 側」)。
import type {
  CoreRegion, CoreSet, CoreType, Element, Equipment, EquipmentPart, EquipmentPartList, EquipmentValues,
  EquipmentAbilityDef, RandomOptionDef, RandomOptionEffect, RandomOptionSlot, RegisteredSienaAura,
  SienaAura, SienaAuraList, SienaAuras, SienaExtraKind, ThesisCores, TitleDef,
} from "./api/types";
import {
  CORE_POWER_TYPES, CORE_REGIONS, CORE_SLOT_COUNT, ELEMENT_ALLOWED_SLOTS, ELEMENTS, EQUIPMENT_STAT_KINDS,
  EQUIPMENT_STAT_SHORT, PART_SLOTS, SIENA_ALLOWED_SLOTS, SKILL_DEPENDENCY_LABELS, STAT_KINDS,
} from "./labels";

const EQUIPMENT_VALUE_KEYS = EQUIPMENT_STAT_KINDS;

export const zeroValues = (): EquipmentValues =>
  Object.fromEntries(EQUIPMENT_VALUE_KEYS.map((k) => [k, 0])) as unknown as EquipmentValues;

/** カタログのレンジ(min/max)の中央値(floor)。カタログ選択時の基本能力値の既定セットに使う。 */
export const midpointValues = (min: EquipmentValues, max: EquipmentValues): EquipmentValues =>
  Object.fromEntries(
    EQUIPMENT_VALUE_KEYS.map((k) => [k, Math.floor((min[k] + max[k]) / 2)]),
  ) as unknown as EquipmentValues;

/** 値を指定された上限まで clamp する。 */
export const clampToCaps = (values: EquipmentValues, caps: EquipmentValues): EquipmentValues =>
  Object.fromEntries(
    EQUIPMENT_VALUE_KEYS.map((k) => [k, Math.min(values[k], caps[k])]),
  ) as unknown as EquipmentValues;

/** 9 値の合計(候補の「上位品」判定など、大小比較の目安にのみ使う)。 */
export const sumValues = (v: EquipmentValues): number =>
  EQUIPMENT_VALUE_KEYS.reduce((s, k) => s + v[k], 0);

/** 値が大きい上位 2 種の要約(部位行の見出し用)。武器なら「突き122 / 斬り315」、鎧なら「物防270 / 魔防245」。 */
export const valuesSummary = (v: EquipmentValues): string => {
  const top = EQUIPMENT_VALUE_KEYS.filter((k) => v[k] > 0).sort((a, b) => v[b] - v[a]).slice(0, 2);
  return top.length === 0 ? "—" : top.map((k) => `${EQUIPMENT_STAT_SHORT[k]}${v[k]}`).join(" / ");
};

/** カタログ候補のレンジ要約。max が大きい上位 2 種を「物防260-280 / 魔防230-260」の形で出す。 */
export const rangeSummary = (min: EquipmentValues, max: EquipmentValues): string => {
  const top = EQUIPMENT_VALUE_KEYS.filter((k) => max[k] > 0).sort((a, b) => max[b] - max[a]).slice(0, 2);
  return top.length === 0
    ? "—"
    : top.map((k) => `${EQUIPMENT_STAT_SHORT[k]}${min[k]}-${max[k]}`).join(" / ");
};

export const neutralSienaAura = (): SienaAura => ({ slots: [], extras: [] });

export const cloneSienaAura = (src: SienaAura): SienaAura => ({
  slots: src.slots.map((s) => ({ ...s })),
  extras: src.extras.map((e) => ({ ...e })),
});
export const neutralSienaAuraList = (): SienaAuraList => ({ registered: [], selected_id: null });
export const neutralSienaAuras = (): SienaAuras =>
  Object.fromEntries(SIENA_ALLOWED_SLOTS.map((slot) => [slot, neutralSienaAuraList()])) as unknown as SienaAuras;
export const cloneRegisteredSienaAura = (src: RegisteredSienaAura): RegisteredSienaAura => ({
  id: src.id, label: src.label, aura: cloneSienaAura(src.aura),
});
export const cloneSienaAuras = (src: SienaAuras): SienaAuras =>
  Object.fromEntries(SIENA_ALLOWED_SLOTS.map((slot) => [slot, {
    selected_id: src[slot].selected_id,
    registered: src[slot].registered.map(cloneRegisteredSienaAura),
  }])) as unknown as SienaAuras;
export const selectedSienaAuraRegistration = (list: SienaAuraList): RegisteredSienaAura | null =>
  list.registered.find((entry) => entry.id === list.selected_id) ?? null;
export const selectedSienaAura = (list: SienaAuraList): SienaAura | null =>
  selectedSienaAuraRegistration(list)?.aura ?? null;

/** 増幅段階 = 能力値スロットの数(wiki: 段階ごとに 1 個解放)。 */
export const sienaStage = (siena: SienaAura): number => siena.slots.length;

/** いま解放されている追加オプションの枠数(段階 3/7/10 で 1/2/3)。 */
export const sienaExtraCapacity = (siena: SienaAura, unlockStages: readonly number[]): number =>
  unlockStages.filter((stage) => sienaStage(siena) >= stage).length;

/** 追加オプションの合計 %(同じ種類は 1 部位 1 個なので実質その値)。 */
export const sienaExtraValue = (siena: SienaAura, kind: SienaExtraKind): number =>
  siena.extras.filter((e) => e.kind === kind).reduce((sum, e) => sum + e.value, 0);

/** 能力値スロットのうち STAB〜AGI の合計。 */
const SIENA_STAT_KINDS = new Set<string>(STAT_KINDS);
export const sienaSlotStatTotal = (siena: SienaAura): number =>
  siena.slots.filter((s) => SIENA_STAT_KINDS.has(s.kind)).reduce((sum, s) => sum + s.value, 0);

/** 能力値スロットの装備補正合計(武器/盾)。表示用。
 *  複合の内訳(物理複合5 = 突き3 + 斬り2)は crates/domain/src/siena.rs と同じ分け方。 */
export const sienaPartValues = (siena: SienaAura): EquipmentValues => {
  const values = zeroValues();
  for (const s of siena.slots) {
    const major = Math.ceil(s.value / 2);
    if (s.kind === "thrust") values.thrust += s.value;
    else if (s.kind === "slash") values.slash += s.value;
    else if (s.kind === "magic_attack") values.magic_attack += s.value;
    else if (s.kind === "magic_defense") values.magic_defense += s.value;
    else if (s.kind === "physical_composite") { values.thrust += major; values.slash += s.value - major; }
    else if (s.kind === "magic_slash") { values.magic_attack += major; values.slash += s.value - major; }
  }
  return values;
};

/** 部位ごとのステ加算合計(能力値スロット + 全ステータス増加 × 7 ステ)。表示用 */
export const sienaPartStatTotal = (siena: SienaAura): number =>
  sienaSlotStatTotal(siena) + sienaExtraValue(siena, "all_stats") * STAT_KINDS.length;

export const neutralCoreSet = (): CoreSet => ({ slots: Array(CORE_SLOT_COUNT).fill(null) });

export const cloneCoreSet = (src: CoreSet): CoreSet => ({
  slots: Array.from({ length: CORE_SLOT_COUNT }, (_, i) => {
    const core = src.slots[i] ?? null;
    return core ? { ...core } : null;
  }),
});

export const neutralThesisCores = (): ThesisCores =>
  Object.fromEntries(CORE_REGIONS.map((r) => [r, neutralCoreSet()])) as unknown as ThesisCores;

export const cloneThesisCores = (src: ThesisCores): ThesisCores =>
  Object.fromEntries(CORE_REGIONS.map((r) => [r, cloneCoreSet(src[r])])) as unknown as ThesisCores;

export const neutralEquipmentPart = (): EquipmentPart => ({
  id: 0,
  label: "",
  item_id: null,
  custom_name: null,
  base: zeroValues(),
  enchant: zeroValues(),
  enhance_level: 0,
  enhance_type: null,
  enhance_grade: null,
  abilities: [],
  ability_values: [],
  ability_additions: [],
  random_options: [],
});

export const cloneEquipmentPart = (src: EquipmentPart): EquipmentPart => ({
  id: src.id,
  label: src.label,
  item_id: src.item_id,
  custom_name: src.custom_name,
  base: { ...src.base },
  enchant: { ...src.enchant },
  enhance_level: src.enhance_level,
  enhance_type: src.enhance_type,
  enhance_grade: src.enhance_grade,
  abilities: [...src.abilities],
  ability_values: (src.ability_values ?? []).map((a) => ({ ...a })),
  ability_additions: (src.ability_additions ?? []).map((a) => ({ ...a })),
  random_options: (src.random_options ?? []).map((o) => ({ ...o })),
});

export const selectedEquipmentPart = (list: EquipmentPartList): EquipmentPart | null =>
  list.registered.find((part) => part.id === list.selected_id) ?? null;

export const selectedEquipmentPartOrNeutral = (list: EquipmentPartList): EquipmentPart =>
  selectedEquipmentPart(list) ?? neutralEquipmentPart();

/** 装備に付与した属性値の合計(属性ごと)。表示用(計算は Rust 側)。 */
export const equipmentElementValues = (equipment: Equipment, element: Element | null): Record<Element, number> => {
  const total = Object.fromEntries(ELEMENTS.map((e) => [e, 0])) as Record<Element, number>;
  if (element === null) return total;
  for (const slot of ELEMENT_ALLOWED_SLOTS) {
    const part = selectedEquipmentPart(equipment.parts[slot]);
    const isEquipment = part?.item_id !== null || (part?.custom_name?.trim().length ?? 0) > 0;
    if (part && isEquipment) total[element] += 9;
  }
  return total;
};

/** テシスコアの補正値(wiki: 進化強化表)。判定・計算は Rust 側。ここは表示用。 */
const CORE_POWER_BONUS: number[][] = [
  [1, 2, 3, 4, 5],
  [6, 7, 8, 9, 10],
  [12, 14, 16, 18, 20],
  [23, 26, 29, 32, 35],
  [40, 50, 60, 70, 80],
];
/** 補助タイプは進化4 の強化1 以降だけ火力と値が分かれる。 */
const CORE_SUPPORT_BONUS: number[][] = [
  [1, 2, 3, 4, 5],
  [6, 7, 8, 9, 10],
  [12, 14, 16, 18, 20],
  [23, 26, 29, 32, 35],
  [40, 45, 50, 55, 60],
];

/** コア 1 個の補正値(表示用) */
export const coreBonus = (type: CoreType, evolution: number, enhancement: number): number => {
  const table = CORE_POWER_TYPES.includes(type) ? CORE_POWER_BONUS : CORE_SUPPORT_BONUS;
  return table[evolution]?.[enhancement] ?? 0;
};

/** 6 枠の補正値合計(入場条件「コア N」と同じ値。表示用) */
export const coreSetTotalBonus = (set: CoreSet): number =>
  set.slots.reduce((sum, core) => sum + (core ? coreBonus(core.core_type, core.evolution, core.enhancement) : 0), 0);

/** 6 枠の補助タイプの合計(装備値 9 種のうち 物防/回避/敏捷/命中)。表示用 */
export const coreSetSupportValues = (set: CoreSet): EquipmentValues => {
  const total = zeroValues();
  for (const core of set.slots) {
    if (!core || CORE_POWER_TYPES.includes(core.core_type)) continue;
    total[core.core_type] += coreBonus(core.core_type, core.evolution, core.enhancement);
  }
  return total;
};

/** コアセット効果(表示用)。判定と値は domain/thesis_core.rs `CoreSet::set_bonus` と同じ。
 *  セットは**同じ進化段階の強化4 コア**で組み、進化段階ごとに成立した分を**合算**する。
 *  例: 進化3 ×3 + 進化4 ×3 = +1% と +2% で +3%。同じ段階が 6 個なら 6 セット効果になり、
 *  3 セット効果は重ねて数えない(進化4 ×6 = +5%)。 */
export interface CoreSetGroup {
  evolution: number;
  count: number;
  fixed: number;
  rate: number;
}
export interface CoreSetEffect {
  /** 成立しているセット(進化段階ごと)。空なら未発動 */
  groups: CoreSetGroup[];
  /** 進化を問わず強化 4 に達しているコアの数(あと何個で 1 セット目かを言うのに使う) */
  ready: number;
  /** 最終ダメージの固定加算(合算後) */
  fixed: number;
  /** 最終ダメージの割合(合算後。0.03 = +3%) */
  rate: number;
}
const CORE_SET_BONUS: Record<number, [number, number][]> = {
  // [進化段階]: [3〜5 個, 6 個] の順に [固定, 割合]
  0: [[500, 0], [800, 0]],
  1: [[700, 0], [1400, 0]],
  2: [[1000, 0], [0, 0.01]],
  3: [[0, 0.01], [0, 0.02]],
  4: [[0, 0.02], [0, 0.05]],
};
export const coreSetEffect = (set: CoreSet): CoreSetEffect => {
  const ready = set.slots.filter((c) => c !== null && c.enhancement >= 4).length;
  const groups: CoreSetGroup[] = [];
  for (let evolution = 0; evolution <= 4; evolution++) {
    const count = set.slots.filter((c) => c !== null && c.enhancement >= 4 && c.evolution === evolution).length;
    if (count < 3) continue;
    const [fixed, rate] = CORE_SET_BONUS[evolution][count >= 6 ? 1 : 0];
    groups.push({ evolution, count, fixed, rate });
  }
  return {
    groups,
    ready,
    fixed: groups.reduce((n, g) => n + g.fixed, 0),
    rate: groups.reduce((n, g) => n + g.rate, 0),
  };
};

/** 全地域のコア合計のうち最大(補正源リストのサマリ表示用) */
export const thesisCoresBestTotal = (cores: ThesisCores): number =>
  Math.max(0, ...CORE_REGIONS.map((r: CoreRegion) => coreSetTotalBonus(cores[r])));

function sumParts(equipment: Equipment, pick: (p: EquipmentPart) => EquipmentValues): EquipmentValues {
  const total = zeroValues();
  for (const slot of PART_SLOTS) {
    const part = selectedEquipmentPart(equipment.parts[slot]);
    if (!part) continue;
    const v = pick(part);
    for (const k of EQUIPMENT_VALUE_KEYS) total[k] += v[k];
  }
  return total;
}

/**
 * 基本能力値の合計(表示用)。Rust 側 `Equipment::base_totals` と**同じ顔ぶれ**にする:
 * Σ part.base + 装備アビリティ + 表示中の称号。カタログを引く必要があるので呼び出し側が渡す。
 * (片方だけ欠けると「装備値の表は 0 なのに攻撃力には乗っている」というズレになる)
 */
export const equipmentBaseTotal = (
  equipment: Equipment,
  abilities: EquipmentAbilityDef[] = [],
  titles: TitleDef[] = [],
): EquipmentValues => {
  const total = sumParts(equipment, (p) => p.base);
  for (const slot of PART_SLOTS) {
    const part = selectedEquipmentPartOrNeutral(equipment.parts[slot]);
    for (const id of part.abilities) {
      const def = abilities.find((a) => a.id === id && a.slot === slot);
      if (def) for (const k of EQUIPMENT_VALUE_KEYS) total[k] += def.values[k];
    }
    for (const value of part.ability_values ?? []) {
      if (value.kind === "thrust") total.thrust += value.value;
      else if (value.kind === "slash") total.slash += value.value;
      else if (value.kind === "physical_defense") total.physical_defense += value.value;
      else if (value.kind === "magic_attack") total.magic_attack += value.value;
      else if (value.kind === "magic_defense") total.magic_defense += value.value;
      else if (value.kind === "accuracy") total.accuracy += value.value;
      else if (value.kind === "critical") total.critical += value.value;
      else if (value.kind === "evasion") total.evasion += value.value;
    }
    for (const addition of part.ability_additions ?? []) {
      if (addition.kind === "thrust") total.thrust += addition.value;
      else if (addition.kind === "slash") total.slash += addition.value;
      else if (addition.kind === "physical_defense") total.physical_defense += addition.value;
      else if (addition.kind === "magic_attack") total.magic_attack += addition.value;
      else if (addition.kind === "magic_defense") total.magic_defense += addition.value;
      else if (addition.kind === "accuracy") total.accuracy += addition.value;
      else if (addition.kind === "critical") total.critical += addition.value;
      else if (addition.kind === "evasion") total.evasion += addition.value;
    }
  }
  const title = titles.find((t) => t.id === equipment.title);
  if (title) for (const k of EQUIPMENT_VALUE_KEYS) total[k] += title.values[k];
  return total;
};
/** Σ part.enchant(表示用。実際の集計は Rust 側 Equipment::enhanced_totals)。 */
export const equipmentEnchantTotal = (equipment: Equipment): EquipmentValues => sumParts(equipment, (p) => p.enchant);

/** ランダムOP の中ディレイ減少の合計 %(表示用)。ほかの補正源から入る分の内訳に使う。 */
export const randomOptionActualDelayPercent = (
  equipment: Equipment,
  defs: RandomOptionDef[],
): number =>
  PART_SLOTS.reduce(
    (sum, slot) =>
      sum +
      selectedEquipmentPartOrNeutral(equipment.parts[slot]).random_options.reduce((n, option) => {
        const def = defs.find((d) => d.id === option.option_id);
        return def?.effect === "actual_delay_reduction" ? n + randomOptionValue(option, def) : n;
      }, 0),
    0,
  );

/** シエナのオーラの追加オプションの合計 %(全部位。表示用)。 */
export const sienaExtraTotal = (equipment: Equipment, kind: SienaExtraKind): number =>
  SIENA_ALLOWED_SLOTS.reduce((sum, slot) => sum + sienaExtraValue(selectedSienaAura(equipment.siena[slot]) ?? neutralSienaAura(), kind), 0);

/** シエナのオーラの攻撃力増加(New1)の合計 %(表示用)。 */
export const sienaAttackRatePercent = (equipment: Equipment): number =>
  sienaExtraTotal(equipment, "attack_rate");

/** シエナのオーラのステ加算の合計(全部位・全ステ。表示用)。 */
export const sienaStatTotal = (equipment: Equipment): number =>
  SIENA_ALLOWED_SLOTS.reduce((sum, slot) => sum + sienaPartStatTotal(selectedSienaAura(equipment.siena[slot]) ?? neutralSienaAura()), 0);

// --- ランダムオプション ---------------------------------------------------
// 判定・集計は Rust 側(crates/domain/src/random_option.rs)。ここは表示・編集用。

/** この枠の効果値。上書きが無ければレンジ上限(Rust の RandomOptionSlot::value と同じ規則)。 */
export const randomOptionValue = (slot: RandomOptionSlot, def: RandomOptionDef): number =>
  slot.value ?? (def.tiers.find((t) => t.rank === slot.rank)?.max ?? 0);

/**
 * 一覧のバッジに出す値。効き先で単位が違う —
 * 割合(与ダメージ増加・攻撃ダメージ・追加ダメージ・耐性)は `%`、命中P/回避P は実数、
 * 中ディレイは減る側なので `−`。
 */
export const randomOptionValueLabel = (slot: RandomOptionSlot, def: RandomOptionDef): string => {
  const value = randomOptionValue(slot, def);
  const effect = def.effect;
  if (typeof effect === "object") return `+${value}%`;
  switch (effect) {
    case "accuracy_point":
    case "evasion_point":
    case "accuracy_and_evasion_point":
      return `+${value}`;
    case "actual_delay_reduction":
      return `−${value}%`;
    default:
      return `+${value}%`;
  }
};

/**
 * その部位に付いている OP を**効き先ごとに合計**した要約。
 * 同じ系統(追加ダメージどうし・与ダメージ増加どうし)は足して 1 つに見せる —
 * 枠ごとの値を並べても、火力にいくら効いているかは読み取れない。
 */
export const randomOptionPartSummary = (
  part: EquipmentPart,
  defs: RandomOptionDef[],
): string => {
  const total = new Map<string, number>();
  const add = (label: string, value: number) => total.set(label, (total.get(label) ?? 0) + value);
  for (const slot of part.random_options) {
    const def = defs.find((d) => d.id === slot.option_id);
    if (def === undefined || !randomOptionIsApplied(def.effect)) continue;
    const value = randomOptionValue(slot, def);
    const effect = def.effect;
    if (typeof effect === "object") {
      add("与ダメ", value);
    } else if (effect === "attack_damage_rate") {
      add("攻撃ダメ", value);
    } else if (effect === "added_damage_rate") {
      add("追加ダメ", value);
    } else if (effect === "accuracy_point") {
      add("命中P", value);
    } else if (effect === "evasion_point") {
      add("回避P", value);
    } else if (effect === "accuracy_and_evasion_point") {
      add("命中P", value);
      add("回避P", value);
    } else if (effect === "actual_delay_reduction") {
      add("中ディレイ", -value);
    }
  }
  const unit = (label: string) => (label === "命中P" || label === "回避P" ? "" : "%");
  return [...total]
    .map(([label, value]) => `${label} ${value > 0 ? "+" : ""}${value}${unit(label)}`)
    .join(" ・ ");
};

/**
 * 全部位のランダムOP を**効き先ごとに合計**した一覧。結果の置き場所(「いまの実力」)で使う。
 * 入力の行に混ぜると、どの部位の話か分からないまま数字だけが並ぶ。
 */
export const randomOptionTotals = (
  equipment: Equipment,
  defs: RandomOptionDef[],
): { label: string; value: string }[] => {
  const total = new Map<string, number>();
  const add = (label: string, value: number) => total.set(label, (total.get(label) ?? 0) + value);
  for (const slot of PART_SLOTS) {
    for (const option of selectedEquipmentPartOrNeutral(equipment.parts[slot]).random_options) {
      const def = defs.find((d) => d.id === option.option_id);
      if (def === undefined || !randomOptionIsApplied(def.effect)) continue;
      const value = randomOptionValue(option, def);
      const effect = def.effect;
      if (typeof effect === "object") {
        add(`与ダメージ増加(${SKILL_DEPENDENCY_LABELS[effect.dependency_damage_rate]})`, value);
      } else if (effect === "attack_damage_rate") {
        add("攻撃ダメージ増加", value);
      } else if (effect === "added_damage_rate") {
        add("割合追加ダメージ", value);
      } else if (effect === "accuracy_point") {
        add("命中P", value);
      } else if (effect === "evasion_point") {
        add("回避P", value);
      } else if (effect === "accuracy_and_evasion_point") {
        add("命中P", value);
        add("回避P", value);
      } else if (effect === "actual_delay_reduction") {
        add("中ディレイ", -value);
      }
    }
  }
  const unit = (label: string) => (label === "命中P" || label === "回避P" ? "" : "%");
  return [...total].map(([label, value]) => ({
    label,
    value: `${value > 0 ? "+" : ""}${value}${unit(label)}`,
  }));
};

/** 効き先の表示名。「記録するだけ」の OP はそう分かる文言にする。 */
export const randomOptionEffectLabel = (effect: RandomOptionEffect): string => {
  if (typeof effect === "object") {
    return `与ダメージ増加 ${SKILL_DEPENDENCY_LABELS[effect.dependency_damage_rate]}`;
  }
  switch (effect) {
    case "attack_damage_rate": return "攻撃ダメージ増加";
    case "added_damage_rate": return "割合追加ダメージ";
    case "accuracy_point": return "命中P";
    case "evasion_point": return "回避P";
    case "accuracy_and_evasion_point": return "命中P・回避P";
    case "actual_delay_reduction": return "中ディレイ減少";
    case "record_only": return "記録するだけ(計算に入りません)";
  }
};

export const randomOptionIsApplied = (effect: RandomOptionEffect): boolean => effect !== "record_only";

/** 全部位のランダムOP の枠数(補正源リストのサマリ用)。 */
export const randomOptionCount = (equipment: Equipment): number =>
  PART_SLOTS.reduce((n, slot) => n + selectedEquipmentPartOrNeutral(equipment.parts[slot]).random_options.length, 0);

/** 計算に入らない(記録するだけの)枠数。 */
export const randomOptionRecordOnlyCount = (equipment: Equipment, defs: RandomOptionDef[]): number =>
  PART_SLOTS.reduce(
    (n, slot) =>
      n +
      selectedEquipmentPartOrNeutral(equipment.parts[slot]).random_options.filter((o) => {
        const def = defs.find((d) => d.id === o.option_id);
        return def !== undefined && !randomOptionIsApplied(def.effect);
      }).length,
    0,
  );

/** シエナのオーラを発現している部位数(表示用)。 */
export const sienaPartCount = (equipment: Equipment): number =>
  SIENA_ALLOWED_SLOTS.filter((slot) => sienaStage(selectedSienaAura(equipment.siena[slot]) ?? neutralSienaAura()) > 0).length;
