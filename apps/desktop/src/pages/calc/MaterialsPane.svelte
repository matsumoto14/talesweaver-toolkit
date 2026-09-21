<script lang="ts">
  /**
   * 「計算の材料」— 計算タブの右カラム。ここでの変更はすべて**試し変更**(保存されない。
   * 保存は帯の「キャラに保存」= calc/simStore)。§00 03「押した場所は動かない」のため、
   * 帯・印・上限の注記は件数が 0 でも枠を残し、カードは押した見出しの**下**にだけ開く。
   * 何を 1 操作として戻すかは calc/simStore の KNOBS が持つ。
   */
  import { errorMessage, listEnchantGains, previewPotentialEffects } from "../../api/commands";
  import type {
    Adjustments, BuffDefinition, CommonSkills, ComboSkillType, DamageResult, EquipmentPart,
    EquipmentStatKind, NewCharacter, PartSlot, Skill, SoulLinkPreview, StatSources,
    TitleDef, UltimateSkill,
  } from "../../api/types";
  import { isBuffOn, toggleBuff } from "../../buffs";
  import { ETERNAL_MILESTONES } from "../../draft";
  import {
    enchantCap, enchantDepKeysFor, enchantRows as enchantRowsOf, ENCHANT_SLOT_LABELS, setEnchantValue,
    type EnchantDepKey,
  } from "../../enchant";
  import { equipmentIconId, polishAmount, selectedEquipmentPartOrNeutral, selectedWeapon } from "../../equipment";
  import { fmtInt, fmtRate, fmtSigned, fmtSignedPct } from "../../format";
  import {
    EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT, PART_SLOT_LABELS, PART_SLOTS,
    POLISH_ALLOWED_SLOTS, POLISH_KIND_LABELS, ULTIMATE_SKILLS, ULTIMATE_SKILL_LABELS,
  } from "../../labels";
  import { limits } from "../../limits.svelte";
  import { tables } from "../../tables.svelte";
  import { app, focusCharacterSource } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import Chip from "../../ui/Chip.svelte";
  import Choose from "../../ui/Choose.svelte";
  import Disclosure from "../../ui/Disclosure.svelte";
  import Icon from "../../ui/Icon.svelte";
  import NumberField from "../../ui/NumberField.svelte";
  import Picker from "../../ui/Picker.svelte";
  import ToggleRow from "../../ui/ToggleRow.svelte";
  import Value from "../../ui/Value.svelte";
  import { latest } from "../../ui/latest.svelte";
  import BuffCard from "./BuffCard.svelte";
  import { changed } from "../../ui/motion.svelte";
  import { deltaText } from "./damageDetail";
  import { SIM_LIMIT, type SimStore } from "./simStore.svelte";

  interface Props {
    /** いま計算に使っているキャラ(試し変更中はその値) */
    payload: NewCharacter;
    /** キャラが変わったら伸びしろの一覧を撮り直す */
    characterId: number | undefined;
    /** 計算中のスキル(エンチャントの主軸ステを決める) */
    skill: Skill | null;
    skillId: string;
    targetId: string | null;
    result: DamageResult | null;
    /** 登録どおりからの伸び率(表記ダメージ)。帯に出す */
    deltaPct: number;
    comboCount: number;
    comboType: ComboSkillType | null;
    adjustments: Adjustments;
    sim: SimStore;
    combo: boolean;
    normalAttackId: string | null;
    normalAttackOverride: string | null;
    normalAttackOptions: { value: string; label: string }[];
  }
  let {
    payload, characterId, skill, skillId, targetId, result, deltaPct, comboCount, comboType,
    adjustments, sim,
    combo = $bindable(),
    normalAttackId,
    normalAttackOverride = $bindable(),
    normalAttackOptions,
  }: Props = $props();

  // --- バフ。チップ・排他・寄与は calc/BuffCard.svelte が持つ。ここで要るのは
  //     研磨のスイッチがバフ「装備研磨」と同じものだ、という 1 点だけ
  const toggleCalcBuff = (def: BuffDefinition) => {
    app.calcBuffs = { choices: toggleBuff(app.calcBuffs.choices, def, !isBuffOn(app.calcBuffs.choices, def.id)) };
  };

  // --- 装備の切り替え(試し変更)。登録済みの装備(EquipmentPartList.registered)のうち
  //     どれを装着するか(selected_id)だけを動かす。登録・編集はキャラタブの装備ペイン。
  //     2 件以上ある部位だけ出す(1 件の部位は切り替える先がない。§00 02)
  const partDisplayName = (part: EquipmentPart): string =>
    part.label
      || app.equipmentCatalog.find((i) => i.id === part.item_id)?.name
      || (part.custom_name ? `${part.custom_name} [仮]` : `装備 ${part.id}`);
  /** 切り替え候補の「選ぶのに要る値」: 強化 Lv とアビリティ数(同名の 2 本を見分ける手がかり) */
  const partSwitchMeta = (part: EquipmentPart): string =>
    `${part.enhance_level > 0 ? `+${part.enhance_level}` : "強化なし"} ・ アビ ${part.abilities.length}`;
  const switchableSlots = $derived(
    payload ? PART_SLOTS.filter((slot) => payload.equipment.parts[slot].registered.length > 1) : [],
  );
  const equipmentHeadNote = $derived(
    switchableSlots.length === 0 ? "切替なし" : `${switchableSlots.length} 部位で切替可`,
  );
  function selectEquipmentPart(slot: PartSlot, id: number) {
    sim.edit((p) => (p.equipment.parts[slot].selected_id = id));
  }

  // --- 称号(試し変更)。1 枠だけ・補正は基本能力値へ合流(キャラタブの称号ペインと同じ)。
  //     ここに並べるのは**所持している称号**(equipment.owned_titles)。持っていない称号は
  //     キャラタブの称号ペインで所持に入れてから選ぶ(検索付きの全一覧はあちらにある)
  const currentTitle = $derived(payload ? app.titles.find((t) => t.id === payload.equipment.title) ?? null : null);
  const titleChoices = $derived.by(() => {
    if (!payload) return [];
    const owned = payload.equipment.owned_titles
      .map((id) => app.titles.find((t) => t.id === id))
      .filter((t): t is TitleDef => t !== undefined);
    // 装着中の称号が所持の外でも、外す先として行に残す
    return currentTitle && !owned.some((t) => t.id === currentTitle.id) ? [currentTitle, ...owned] : owned;
  });
  const titleHeadNote = $derived(currentTitle?.name ?? "なし");
  const titleNote = (t: TitleDef): string => {
    const vals = EQUIPMENT_STAT_KINDS.filter((k) => t.values[k] !== 0).map((k) => `${EQUIPMENT_STAT_SHORT[k]}${fmtInt(t.values[k])}`);
    if (t.attack_damage_percent > 0) vals.push(`ダメ ${fmtSigned(t.attack_damage_percent, { max: 2 }, "%")}`);
    if (t.added_damage_percent > 0) vals.push(`追加ダメ ${fmtSigned(t.added_damage_percent, { max: 2 }, "%")}`);
    return vals.join(" ") || "—";
  };
  function selectTitle(id: string | null) {
    sim.edit((p) => (p.equipment.title = id));
  }

  // --- 研磨(試し変更)。記録はキャラタブの研磨ペイン、効かせるかはバフ「装備研磨」と同じ
  //     スイッチ(calcBuffs の equipment_polish)。装備に付けたものなので、探す先はバフ一覧では
  //     なく材料列(シャープネス・エンチャントと同じ段)。ここは表示の顔で、正は Rust 側
  //     (`equipment_polish_active` / `Equipment::base_sources`)。加算量のプレビューは
  //     PolishPane と同じ polishAmount(表示のみ)で出す
  const polishDef = $derived(app.catalog.find((def) => def.id === "equipment_polish") ?? null);
  const polishOn = $derived(polishDef !== null && isBuffOn(app.calcBuffs.choices, polishDef.id));
  const polishRows = $derived.by(() => {
    if (!payload) return [];
    return POLISH_ALLOWED_SLOTS.flatMap((slot) => {
      const entry = payload.equipment.polish.entries.find((e) => e.slot === slot);
      if (!entry) return [];
      const part = selectedEquipmentPartOrNeutral(payload.equipment.parts[slot]);
      return [{ slot, entry, amount: polishAmount(entry.kind, slot, part.base[entry.stat]) }];
    });
  });
  const polishTotalsLabel = $derived.by(() => {
    const sum = new Map<EquipmentStatKind, number>();
    for (const row of polishRows) sum.set(row.entry.stat, (sum.get(row.entry.stat) ?? 0) + row.amount);
    return [...sum.entries()].map(([k, v]) => `${EQUIPMENT_STAT_SHORT[k]} ${fmtSigned(v)}`).join(" ・ ");
  });
  const polishHeadNote = $derived(
    polishRows.length === 0 ? "未登録" : `${polishOn ? "ON" : "OFF"} ・ ${polishTotalsLabel}`,
  );
  // --- 極限スキル(試し変更)。2 枠のうち何を選ぶかだけをこの画面で切り替える -----------
  // スーパーリミット・ハイパーリミットの Lv はキャラタブ(共通スキル)の設定が正。ここでは触らない。
  const ultimatePickedCount = $derived(
    payload?.common_skills.ultimate.slots.filter((s) => s !== null).length ?? 0,
  );
  // 枠数は Rust 側の定数(ULTIMATE_SKILL_SLOTS)そのまま。写経せず、データの配列長から引く
  const ultimateSlotCount = $derived(payload?.common_skills.ultimate.slots.length ?? 0);
  const ultimateFull = $derived(ultimateSlotCount > 0 && ultimatePickedCount >= ultimateSlotCount);
  function toggleUltimate(skillId: UltimateSkill) {
    sim.edit((p) => {
      const slots = p.common_skills.ultimate.slots;
      const at = slots.indexOf(skillId);
      if (at !== -1) {
        slots[at] = null;
        return;
      }
      const empty = slots.indexOf(null);
      if (empty !== -1) slots[empty] = skillId;
    });
  }
  /** チップに併記する効果値。写経しない — Rust 側 preview_potential_effects(3 種すべてを
   *  付けたとしたときの効果)から引く。 */
  let ultimateEffects = $state<{
    critical_damage_rate: number; actual_delay_reduction: number; skill_range_bonus: number;
  } | null>(null);
  /** ソウルリンクの効いている量(preview_effective_stats の soul_link)。同じ応答から取る */
  let soulLinkPreview = $state<SoulLinkPreview | null>(null);
  const ultimateLatest = latest({ debounce: 150 });
  $effect(() => {
    const p = payload;
    if (!p) {
      ultimateLatest.cancel();
      ultimateEffects = null;
      soulLinkPreview = null;
      return;
    }
    const statSources = JSON.parse(JSON.stringify(p.stat_sources)) as StatSources;
    const commonSkills = JSON.parse(JSON.stringify(p.common_skills)) as CommonSkills;
    ultimateLatest.run(async (isCurrent) => {
      try {
        const potential = await previewPotentialEffects(statSources, commonSkills);
        if (isCurrent()) {
          ultimateEffects = potential.ultimate;
          // ソウルリンクの効いている量も同じ応答から取る。写経せず Rust 側の preview を正にする
          soulLinkPreview = potential.soul_link;
        }
      } catch (e) {
        if (isCurrent()) reportError(errorMessage(e));
      }
    });
    return () => ultimateLatest.cancel();
  });
  function ultimateChipNote(skillId: UltimateSkill): string {
    const e = ultimateEffects;
    if (!e) return "";
    if (skillId === "scope_eye") return `クリダメ ${fmtSignedPct(e.critical_damage_rate)}`;
    if (skillId === "full_throttle") return `中ディレイ ${fmtSignedPct(-e.actual_delay_reduction)}`;
    return `範囲 ${fmtSigned(e.skill_range_bonus)}(火力には効きません)`;
  }

  // --- 地力(試し変更)。装備ではなく育てて上がるもののうち、効きが大きい 3 つ -----------
  // 覚醒・エタの意志(N と各種上限)/ シャープネスビジョン(§5 新-割合)/ ソウルリンク 5〜7。
  // どれもキャラタブでしか触れず、「盛ったらどこまで届くか」を計算タブで試せなかった。
  // 入力形はキャラタブと同じ(節目の段 + 数値)にして、押した瞬間に結果が動くようにする。

  /** エタの意志は覚醒 5 の先にあるもの。Lv を入れた時点で覚醒は 5 で確定する(キャラタブと同じ) */
  function setSimEternalLevel(level: number) {
    sim.edit((p) => {
      p.awakening.eternal_level = level;
      if (level > 0) p.awakening.stage = limits.eternal_awakening_stage;
    });
  }
  /** 節目を超えると上限の増え方が一段上がる地点(draft.ts の ETERNAL_MILESTONES) */
  const eternalMilestoneOptions = $derived(
    ETERNAL_MILESTONES.filter((lv) => lv <= limits.eternal_level_max).map((lv) => ({
      value: String(lv),
      label: String(lv),
    })),
  );
  // 覚醒段階は 4 と 5 しか使わない(キャラタブと同じ)。それ以外は開いたときだけ出す(§00 02)
  const stageAllOptions = $derived(
    Array.from({ length: limits.awakening_stage_max + 1 }, (_, i) => ({
      value: String(i),
      label: String(i),
    })),
  );
  const STAGE_MAIN_OPTIONS = [4, 5].map((i) => ({ value: String(i), label: String(i) }));
  let stageAllOpen = $state(false);
  const stageIsLow = $derived((payload?.awakening.stage ?? 0) < 4);
  const stageOptionsNow = $derived(stageAllOpen || stageIsLow ? stageAllOptions : STAGE_MAIN_OPTIONS);
  /** 覚醒ダメージ(カテゴリN)の倍率。表は gamedata なので写経せず計算結果のカテゴリから引く */
  const awakeningFactor = $derived(
    result?.trace.categories.find((c) => c.category === "awakening_damage")?.factor ?? null,
  );
  const awakeningHeadNote = $derived(
    payload ? `覚醒 ${payload.awakening.stage} / エタ Lv${payload.awakening.eternal_level}` : "",
  );

  /** 正は crates/domain/src/common_skill.rs の SHARPNESS_VISION(limits 経由で引く) */
  const SHARPNESS_RATES = $derived(tables.sharpness_vision_rates.map((r) => Math.round(r * 100)));
  const sharpnessLevel = $derived(payload?.common_skills.sharpness_vision_level ?? 0);
  const sharpnessRatePercent = $derived(
    sharpnessLevel === 0 ? 0 : (SHARPNESS_RATES[sharpnessLevel - 1] ?? 0),
  );
  // 段の名前は Lv だけ、効いている値は行の右に出す。**Lv5 まではほぼ全員が同じ**(そこで
  // 止まる)なので、ふだんは 5〜10 だけ出す(キャラタブの共通スキルペインと同じ)
  const sharpnessAllOptions = $derived(
    Array.from({ length: limits.sharpness_vision_level_max }, (_, i) => ({
      value: String(i + 1),
      label: String(i + 1),
    })),
  );
  let sharpnessAllOpen = $state(false);
  const sharpnessIsLow = $derived(sharpnessLevel > 0 && sharpnessLevel < 5);
  const sharpnessOptionsNow = $derived(
    sharpnessAllOpen || sharpnessIsLow ? sharpnessAllOptions : sharpnessAllOptions.slice(4),
  );

  /** ダメージ式に効くリンクステータス 5〜7 だけ(1〜4 は装備値、8 は追加HPなのでここでは出さない) */
  const SOUL_LINK_ROWS = [
    { field: "critical_damage_level", label: "クリダメ", max: limits.soul_link_critical_damage_level_max },
    { field: "final_damage_level", label: "最終ダメ", max: limits.soul_link_final_damage_level_max },
    { field: "weapon_enhance_level", label: "武器強化", max: limits.soul_link_weapon_enhance_level_max },
  ] as const;
  type SoulLinkDamageField = (typeof SOUL_LINK_ROWS)[number]["field"];
  /** 効いている量は Rust の preview(soulLinkPreview)から引く。倍率の式をここに写経しない */
  const soulLinkEffect = (field: SoulLinkDamageField): number | null => {
    const p = soulLinkPreview;
    if (!p) return null;
    if (field === "critical_damage_level") return p.critical_damage_rate;
    if (field === "final_damage_level") return p.final_damage_rate;
    return p.weapon_added_damage_multiplier;
  };
  const soulLinkEffectText = (field: SoulLinkDamageField): string => {
    const value = soulLinkEffect(field);
    if (value === null) return "—";
    return field === "weapon_enhance_level"
      ? fmtRate(value, 1)
      : fmtSignedPct(value, { max: 1 });
  };
  /** 最終ダメージ(カテゴリL)の上限。値は Rust のカテゴリ定義が正なのでトレースから引く */
  const finalDamageCapPercent = $derived.by(() => {
    const cap = result?.trace.categories.find((c) => c.category === "final_damage_rate")?.cap?.max;
    return cap == null ? null : Math.round(cap * 100);
  });
  const soulLinkHeadNote = $derived(
    payload
      ? `Lv ${SOUL_LINK_ROWS.map((r) => payload.stat_sources.soul_link[r.field]).join("/")}`
      : "",
  );

  // --- エンチャントの伸びしろ(試し変更)。選択中スキルの依存ステだけを部位横断で見る ------
  // 考え方はホーム(HomePage.svelte)の enchantRows と同じで、こちらは enchant.ts を共用する。
  const enchantDepKeys = $derived<EnchantDepKey[]>(skill ? enchantDepKeysFor(skill.dependency) : []);
  const enchantRowsList = $derived.by(() => {
    if (!payload) return [];
    const keys = enchantDepKeys;
    if (keys.length === 0) return [];
    return enchantRowsOf(payload.equipment, keys, app.equipmentCatalog).filter(
      (r) =>
        r.capUnknown ||
        keys.some((k) => r.part.enchant[k] < (enchantCap(r.part, k, app.equipmentCatalog) ?? 0)) ||
        // 一度出した行は、伸びしろを使い切っても消さない(§00 03 押した場所は動かない)。
        // ここを見落として「ステ単位」だけ覚えていたため、依存ステが 1 本しか無い部位(兜)は
        // その 1 本を MAX にすると**行ごと**消え、下の行が同じ位置へ繰り上がっていた。
        // 実機の clickall.js が NG(0,46) で検出した。
        keys.some((k) => enchantShownKeys.has(`${r.slot}:${k}`)),
    );
  });
  /** 「MAX まで積むと +x%」。行 × ステごとの伸び率は Rust(list_enchant_gains)がまとめて返す
   *  (伸び率の式・丸めをフロントで組み立て直さない。エンチャント候補も選択中スキルの依存ステに
   *  Rust 側で絞られている)。 */
  let enchantGains = $state<Record<string, number>>({});
  const enchantLatest = latest({ debounce: 200 });
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null;
    const t = targetId;
    const sid = skillId;
    const rowCount = enchantRowsList.length;
    const count = comboCount;
    const type = comboType;
    const tempJson = JSON.stringify(adjustments);
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson || !t || !sid || rowCount === 0) {
      enchantLatest.cancel();
      enchantGains = {};
      return;
    }
    enchantLatest.run((isCurrent) =>
      listEnchantGains(
        JSON.parse(pJson) as NewCharacter, sid, t, count,
        type, JSON.parse(tempJson), JSON.parse(buffsJson),
      )
        .then((gains) => {
          if (isCurrent()) {
            enchantGains = Object.fromEntries(gains.map((g) => [`${g.slot}:${g.key}`, g.delta_pct]));
          }
        })
        .catch((e) => reportError(errorMessage(e))),
    );
    return () => enchantLatest.cancel();
  });
  /**
   * 行の中で実際に伸びしろがあるステだけを残す(その時点の cur/gain で見た「素の」判定)。
   * - 上限に達したステ(cur >= cap)は出さない(「上限」の文字も出さない)
   * - MAX まで積んでも最終ダメージが動かない(改善しない)ステも出さない
   *   (list_enchant_gains は改善しない組を rank_candidates が既に除外して返すので、
   *   マップに無い = 伸びしろ無し。試算が返る前は undefined のまま残す)
   */
  function enchantVisibleKeys(row: (typeof enchantRowsList)[number]): EnchantDepKey[] {
    return enchantDepKeys.filter((k) => {
      const cap = enchantCap(row.part, k, app.equipmentCatalog) ?? 0;
      if (cap <= 0 || row.part.enchant[k] >= cap) return false;
      const gain = enchantGains[`${row.slot}:${k}`];
      return gain !== 0;
    });
  }
  /**
   * §00 03「押した場所は動かない」/ §09 規則 1: 一度この一覧に出た「行 × ステ」は、この画面を
   * 開いている(= このキャラ・対象・スキルを見ている)あいだは消さない。MAX を押した直後に
   * その行が一覧から消えて下の行が繰り上がり、繰り上がった別の行を誤って書き換える実害が
   * 実機検証で確認された。ここで積んだ集合は「見えたことがある」だけを覚え、伸びしろが
   * 復活しても・失っても行の位置は動かさない。
   *
   * 撮り直す(空にする)タイミング:
   * - キャラ・対象・スキルが変わったとき(下の $effect。見ている一覧の意味自体が変わる)
   * - 「ぜんぶ戻す」で試し変更を全部捨てたとき(resetSim。伸びしろの土台が登録値に戻るため)
   */
  let enchantShownKeys = $state<Set<string>>(new Set());
  $effect(() => {
    // 依存トリガーだけを読む(値は使わない) — 新しい一覧として撮り直す
    void characterId;
    void targetId;
    void skillId;
    // 「ぜんぶ戻す」でも土台(登録値)が戻るので撮り直す
    void sim.resetCount;
    enchantShownKeys = new Set();
  });
  $effect(() => {
    let next: Set<string> | null = null;
    for (const row of enchantRowsList) {
      if (row.capUnknown) continue;
      for (const k of enchantVisibleKeys(row)) {
        const id = `${row.slot}:${k}`;
        if (!enchantShownKeys.has(id)) {
          if (!next) next = new Set(enchantShownKeys);
          next.add(id);
        }
      }
    }
    if (next) enchantShownKeys = next;
  });
  /** 表示用の可視判定。「いま伸びしろがある」に加えて「見えたことがある」も可視の理由にする
   *  (enchantShownKeys)。cap <= 0(そもそも枠が無い)だけは無条件で出さない。 */
  function enchantVisibleKeysStable(row: (typeof enchantRowsList)[number]): EnchantDepKey[] {
    const live = new Set(enchantVisibleKeys(row));
    return enchantDepKeys.filter((k) => {
      const cap = enchantCap(row.part, k, app.equipmentCatalog) ?? 0;
      if (cap <= 0) return false;
      return live.has(k) || enchantShownKeys.has(`${row.slot}:${k}`);
    });
  }
  /** ステが全部落ちた行は行ごと出さない(§00 02・05: 押しても何も起きない欄を並べない)。
   *  上限が未収録の行(capUnknown)は例外 — 落とすと「なぜ出ないか」が分からなくなるので、
   *  伸びしろの代わりに「上限未入力」の案内行として残す。 */
  const visibleEnchantRows = $derived(
    enchantRowsList
      .map((row) => ({ row, keys: row.capUnknown ? [] : enchantVisibleKeysStable(row) }))
      .filter((x) => x.row.capUnknown || x.keys.length > 0),
  );

</script>

        <!-- 試し変更バー -->
        <div class="sim-bar" class:active={sim.dirty}>
          <div class="sim-line">
            <span class="sim-dot" class:active={sim.dirty}></span>
            <span class="sim-title">{sim.dirty ? "装備・スキルを試し変更中" : "装備・スキルはキャラ登録どおり"}</span>
            <!-- 差分の枠も常に確保する。出た瞬間に行が 1px 伸びて下がずれる(§09 規則 4) -->
            <span class="num sim-delta" class:on={sim.dirty} class:up={deltaPct > 0} class:down={deltaPct < 0}
            >{sim.dirty ? deltaText(deltaPct) : ""}</span>
          </div>
          <!-- 主語をタイトルに置く。「登録どおり」だけだと、何が登録どおりなのか分からず
               初見で止まる(ユーザー指摘 2026-08-31)。主語は「材料」ではなく**装備・スキル** —
               この帯が見ているのは KNOBS(パワーW / ストロングW / エンチャント / 極限スキル)
               だけで、同じペインにあるバフは別枠(app.calcBuffs)。「材料は登録どおり」と書くと、
               バフを足した状態でも登録どおりだと言ってしまう。
               文言は状態で変えない。「登録どおり」は 2 行・「試し変更中」は 1 行に折り返して
               いたため、切り替えた瞬間に 1 行ぶん縮んで**下の材料が丸ごと上へ吸い上げられた**
               (実機で押した MAX ボタン自身が 11px 逃げた。§00 03 / §09 規則 1)。
               高さを確保する手も試したが、1 行の状態で下に空きが出て §00 02 を崩す。
               状態は上のタイトルと帯の色が伝えるので、ここは動かない 1 行だけ置く -->
          <div class="sim-note-text dim">ここでの変更は保存されません。</div>
          <!-- ボタンは常に置き、登録どおりのときは隠すだけにする(§00 03 / §09 規則 1)。
               {#if} で出し入れすると、枠の min-height とボタンの実高(padding 7+7 + border 1+1
               + 行高 ≒ 33px)が食い違い、出た瞬間に 3px 伸びて**下の材料が流れる**。実機の
               clickall.js が NG(0,3) で検出した。高さを数値で合わせても書体が変われば再発する
               ので、本物のボタンで枠を満たして構造的に一致させる。
               inert は隠しているあいだフォーカスもクリックも通さない(押せるものは見せない) -->
          <div class="sim-actions" class:idle={!sim.dirty} inert={!sim.dirty}>
            <button type="button" class="btn" onclick={() => sim.reset()}>ぜんぶ戻す</button>
            <button type="button" class="btn primary" disabled={sim.saving} onclick={() => sim.save()}>{sim.saving ? "保存中…" : "キャラに保存"}</button>
          </div>
        </div>

        <!-- 試し変更の印。**チップではない**(押して選ぶものではなく、いま変えている印と
             その取り消し)ので `.chip` の名前も見た目も借りない。高さを固定し、3 件までなので
             1 行に収め、溢れたら横にスクロールさせる(行が増えて下をずらさない) -->
        <div class="sim-marks">
          {#each sim.changed as k (k.id)}
            <span class="sim-mark badge-in" use:changed={() => k.get(app.sim!)}>
              <span>{k.label(app.sim!, sim.saved)}</span>
              <button type="button" class="sim-revert" title="この変更だけ戻す" onclick={() => sim.revert(k)}>✕</button>
            </span>
          {/each}
        </div>
        <!-- 上限の注記は固定領域。登録どおり(0 件)のときは中身だけ出さず、枠は常に確保する
             (§09 規則 1・4)。ここは押した材料(装備・バフ)より上なので、丸ごと差し込むと
             押したものが流れる -->
        <div class="sim-limit" class:hit={sim.limited}>
          {#if sim.limited}
            試し変更は同時 {SIM_LIMIT} 件までです。どれかを ✕ で戻すか、「キャラに保存」で確定してください。
          {:else if sim.changed.length > 0}
            同時に試せるのは {sim.changed.length} / {SIM_LIMIT} 件です。
          {/if}
        </div>

        <!-- 極限スキル(試し変更)。3 種から 2 つ選ぶ(§07 形態 3: チップで入れる/外す) -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <!-- 見出しの顔。中身を代表する 1 つを置く(名前と併記なので §08 の単独表示にあたらない)。
                 画像が未収録なら破線 + ? になるだけで、幅は変わらない -->
            <Icon kind="skill" id="scope_eye" size={20} label="極限スキル" />
            <span class="card-title">極限スキル</span>
            <Value class="dim small" motion={() => ultimatePickedCount} value={`${ultimatePickedCount} / ${ultimateSlotCount}`} />
          {/snippet}
          {#snippet children(open)}
          {#if open}
          <div class="ultimate-chips">
            {#each ULTIMATE_SKILLS as u (u)}
              {@const on = payload.common_skills.ultimate.slots.includes(u)}
              <ToggleRow
                name={ULTIMATE_SKILL_LABELS[u]}
                value={ultimateChipNote(u)}
                {on}
                tone="temp"
                disabled={!on && ultimateFull}
                title={!on && ultimateFull ? `${ultimateSlotCount} 枠まで選べます。ほかを外してから選んでください。` : undefined}
                onToggle={() => toggleUltimate(u)}
              >
                {#snippet icon()}<Icon kind="skill" id={u} size={20} label={ULTIMATE_SKILL_LABELS[u]} />{/snippet}
              </ToggleRow>
            {/each}
          </div>
          {#if ultimateFull}
            <p class="eq-note dim badge-in">{ultimateSlotCount} 枠まで選べます。ほかを外してから選んでください。</p>
          {/if}
          <p class="eq-note dim">スーパーリミット・ハイパーリミットの Lv は<b>キャラ</b>タブ(共通スキル)の設定を使います。</p>
          {/if}
          {/snippet}
        </Disclosure>

        <!-- 覚醒・エタの意志(試し変更)。カテゴリN の倍率と、ダメージ・能力値の上限を動かす -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <Icon kind="skill" id="awakening" size={20} label="覚醒・エタの意志" />
            <span class="card-title">覚醒・エタの意志</span>
            <Value class="dim small" value={awakeningHeadNote} />
          {/snippet}
          {#snippet children(open)}
          {#if open}
          <div class="basics-rows">
            <div class="basics-row">
              <span class="basics-label">エタ Lv</span>
              <NumberField
                label="エタの意志 Lv"
                max={limits.eternal_level_max}
                bind:value={
                  () => payload.awakening.eternal_level,
                  (v) => setSimEternalLevel(v)
                }
              />
            </div>
            <div class="basics-row">
              <span class="basics-label">節目</span>
              <div class="basics-seg">
                <Choose
                  label="エタの意志の節目"
                  options={eternalMilestoneOptions}
                  cols={eternalMilestoneOptions.length}
                  bind:value={
                    () => String(payload.awakening.eternal_level),
                    (v) => setSimEternalLevel(Number(v))
                  }
                />
              </div>
            </div>
            <div class="basics-row">
              <span class="basics-label">覚醒段階</span>
              <div class="basics-seg">
                <Choose
                  label="覚醒段階"
                  options={stageOptionsNow}
                  cols={stageOptionsNow.length}
                  bind:value={
                    () => String(payload.awakening.stage),
                    (v) => sim.edit((p) => (p.awakening.stage = Number(v)))
                  }
                />
              </div>
              <!-- 段そのものは消さない。4 / 5 以外を出す切り替えは段の外に置く(§09 規則 4)。
                   4 未満を選んでいるあいだは全段が出ていて切り替える意味が無いが、**消すと
                   隣の段の幅が動く**ので、置いたまま押せなくする -->
              <Chip class="quiet" on={stageAllOpen} disabled={stageIsLow}
 onToggle={() => (stageAllOpen = !stageAllOpen)}
              >{stageAllOpen || stageIsLow ? "4 / 5 だけ" : "それ以外"}</Chip>
            </div>
          </div>
          <p class="eq-note dim">
            覚醒ダメージ <b><Value value={String(awakeningFactor)}>{#snippet children()}{awakeningFactor !== null ? fmtRate(awakeningFactor) : "—"}{/snippet}</Value></b>
            ・ ダメージ上限 <b><Value motion={() => result?.damage_cap ?? null} value={result ? fmtInt(result.damage_cap) : "—"} delta={{}} /></b>。
            節目(20 / 40 / 60 / 80 / 90)を超えると上限の伸びが一段上がります。Lv を入れると覚醒は 5 になります。
          </p>
          {/if}
          {/snippet}
        </Disclosure>

        <!-- シャープネスビジョン(試し変更)。§5「新-割合」の割合追加ダメージ -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <Icon kind="skill" id="sharpness_vision" size={20} label="シャープネスビジョン" />
            <span class="card-title">シャープネスビジョン</span>
            <Value
              class="dim small" motion={() => sharpnessRatePercent}
              value={sharpnessLevel === 0 ? "未習得" : `Lv${sharpnessLevel} ${fmtSigned(sharpnessRatePercent, { max: 2 }, "%")}`}
            />
          {/snippet}
          {#snippet children(open)}
          {#if open}
          <div class="basics-rows">
            <!-- 段は 6 個(5〜10)にも 10 個にもなる。補助操作を同じ行に置くと、段が 10 個に
                 なったときチップが段の上に重なって押せなくなる(実機で検出)。行を分ける -->
            <div class="basics-row">
              <span class="basics-label">Lv</span>
              <div class="basics-seg">
                <Choose
                  label="シャープネスビジョン Lv"
                  options={sharpnessOptionsNow}
                  cols={sharpnessOptionsNow.length}
                  bind:value={
                    () => String(sharpnessLevel),
                    (v) => sim.edit((p) => (p.common_skills.sharpness_vision_level = Number(v)))
                  }
                />
              </div>
            </div>
            <div class="basics-row">
              <span class="basics-label"></span>
              <!-- Lv1〜4 を選んでいるあいだは全段が出ていて切り替える意味が無いが、**消すと
                   隣のチップが動く**ので、置いたまま押せなくする(§09 規則 4) -->
              <Chip class="quiet" on={sharpnessAllOpen} disabled={sharpnessIsLow}
 onToggle={() => (sharpnessAllOpen = !sharpnessAllOpen)}
              >{sharpnessAllOpen || sharpnessIsLow ? "5 以上" : "1〜4"}</Chip>
              <Chip class="quiet"
 disabled={sharpnessLevel === 0}
 onclick={() => sim.edit((p) => (p.common_skills.sharpness_vision_level = 0))}
              >未習得</Chip>
            </div>
          </div>
          <p class="eq-note dim">
            割合追加ダメージは<b>合計ダメージ</b>に乗ります(1 発ごとではないので、表記ダメージ = この一発は動きません)。
          </p>
          {/if}
          {/snippet}
        </Disclosure>

        <!-- ソウルリンク(試し変更)。ダメージ式に効くリンクステータス 5〜7 だけ -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <Icon kind="skill" id="soul_link" size={20} label="ソウルリンク" />
            <span class="card-title">ソウルリンク</span>
            <Value class="dim small" value={soulLinkHeadNote} />
          {/snippet}
          {#snippet children(open)}
          {#if open}
          <div class="basics-rows">
            {#each SOUL_LINK_ROWS as row (row.field)}
              <div class="basics-row">
                <span class="basics-label">{row.label}</span>
                <NumberField
                  label="{row.label}リンクステータス Lv"
                  max={row.max}
                  bind:value={
                    () => payload.stat_sources.soul_link[row.field],
                    (v) => sim.edit((p) => (p.stat_sources.soul_link[row.field] = v))
                  }
                />
                <Value class="basics-val" motion={() => soulLinkEffect(row.field)} value={soulLinkEffectText(row.field)} />
              </div>
            {/each}
          </div>
          <p class="eq-note dim">
            クリダメはクリティカル時だけ、最終ダメージはカテゴリL の上限{finalDamageCapPercent !== null ? ` ${fmtSigned(finalDamageCapPercent, { max: 2 }, "%")}` : ""}まで。
            武器強化は追加固定ダメージに掛かるので、<b>合計ダメージ</b>だけが動きます。
          </p>
          {/if}
          {/snippet}
        </Disclosure>

        <!-- 装備の切り替え(試し変更)。登録済みの装備から装着するものを選ぶ。登録・編集はキャラタブ -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <!-- 見出しの顔は装着中の武器(名前と併記) -->
            <Icon kind="equipment" id={equipmentIconId(selectedWeapon(payload).item_id, app.equipmentCatalog)} size={20} label="装備の切り替え" />
            <span class="card-title">装備の切り替え</span>
            <Value class="dim small" value={equipmentHeadNote} />
          {/snippet}
          {#snippet children(open)}
          {#if open}
          {#if switchableSlots.length === 0}
            <button type="button" class="source-jump" onclick={() => focusCharacterSource("equipment")}>
              <span class="dim small">2 件以上登録した部位がありません。登録はキャラタブの装備ペイン</span>
              <span class="chev dim">›</span>
            </button>
          {:else}
            <div class="switch-rows">
              {#each switchableSlots as slot (slot)}
                {@const list = payload.equipment.parts[slot]}
                <div class="switch-row">
                  <span class="enchant-row-label">{PART_SLOT_LABELS[slot]}</span>
                  <div class="switch-picker">
                    <Picker
                      label="{PART_SLOT_LABELS[slot]}に装着する登録"
                      options={list.registered.map((part) => ({
                        value: String(part.id), name: partDisplayName(part), meta: partSwitchMeta(part),
                        iconId: equipmentIconId(part.item_id, app.equipmentCatalog), iconKind: "equipment" as const,
                      }))}
                      bind:value={() => String(list.selected_id ?? ""), (v) => selectEquipmentPart(slot, Number(v))}
                    />
                  </div>
                </div>
              {/each}
            </div>
          {/if}
          <p class="eq-note dim">装着中の装備を登録済みの別の 1 件に替えます。登録・編集はキャラタブの装備ペイン。</p>
          {/if}
          {/snippet}
        </Disclosure>

        <!-- エンチャントの伸びしろ(試し変更)。選択中スキルの依存ステだけを部位横断で見る -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <!-- †エクリプスウィング(体)。エンチャントは装備を伸ばす話なので、その顔として置く -->
            <Icon kind="equipment" id="wiki-af444f9bf21d" size={20} label="エンチャントの伸びしろ" />
            <span class="card-title">エンチャントの伸びしろ</span>
            <!-- 見出しの注記は件数 1 つだけ。アイコンぶん幅が減っていて、主軸を並べると
                 右端の件数が切れる(§00 05: 読めない文字は出さない)。主軸は中に出す -->
            <Value class="dim small" motion={() => visibleEnchantRows.length} value={`${visibleEnchantRows.length} 件`} />
          {/snippet}
          {#snippet children(open)}
          {#if open}
            <p class="enchant-dep dim">主軸: {enchantDepKeys.map((k) => EQUIPMENT_STAT_SHORT[k]).join("・") || "—"}</p>
          {#if visibleEnchantRows.length === 0}
            <p class="eq-note dim">主軸スキルの依存ステを盛れる部位がないか、すでに上限です。</p>
          {:else}
            <div class="enchant-rows">
              {#each visibleEnchantRows as { row, keys } (row.slot)}
                <div class="enchant-row">
                  <span class="enchant-row-label">{ENCHANT_SLOT_LABELS[row.slot]}</span>
                  {#if row.capUnknown}
                    <button type="button" class="source-jump" onclick={() => focusCharacterSource("equipment", row.slot)}>
                      <span class="badge unknown" title="カタログ外(カスタム名)装備でエンチャント上限が未入力です">上限未入力</span>
                      <span class="chev dim">›</span>
                    </button>
                  {:else}
                  <div class="enchant-row-cols">
                    {#each keys as k (k)}
                      {@const cap = enchantCap(row.part, k, app.equipmentCatalog) ?? 0}
                      {@const gain = enchantGains[`${row.slot}:${k}`]}
                      <div class="enchant-stat">
                        <span class="enchant-stat-label">{EQUIPMENT_STAT_SHORT[k]}</span>
                        <NumberField
                          label="{EQUIPMENT_STAT_SHORT[k]}のエンチャント"
                          max={cap}
                          bind:value={
                            () => row.part.enchant[k],
                            (v) => sim.edit((p) => setEnchantValue(p.equipment, row.slot, k, v))
                          }
                        />
                        <Value
                          class="enchant-gain" tone={(gain ?? 0) > 0 ? "up" : null} motion={() => gain ?? null}
                          value={gain !== undefined ? `MAX で ${fmtSigned(gain, { max: 2 }, "%")}` : ""}
                        />
                      </div>
                    {/each}
                  </div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
          {/if}
          {/snippet}
        </Disclosure>

        <!-- 研磨(試し変更)。記録はキャラタブの研磨ペインで、ここでは効かせるかだけ切り替える。
             スイッチの実体はバフ「装備研磨」(calcBuffs。保存は「試し変更を保存」で)だが、装備の話なので顔はここ -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <!-- 職人の装備研磨剤(クライアント資産 item 1044778)。バフ「装備研磨」と同じ絵 -->
            <Icon kind="buff" id="equipment_polish" size={20} label="研磨" />
            <span class="card-title">研磨</span>
            <Value class="dim small" value={polishHeadNote} />
          {/snippet}
          {#snippet children(open)}
          {#if open}
          <ToggleRow
            name="研磨を効かせる"
            on={polishOn}
            tone="temp"
            disabled={polishDef === null}
            onToggle={() => { if (polishDef) toggleCalcBuff(polishDef); }}
          />
          {#if polishRows.length === 0}
            <button type="button" class="source-jump" onclick={() => focusCharacterSource("polish")}>
              <span class="badge unknown">未登録</span>
              <span class="dim small">登録はキャラタブの研磨ペイン</span>
              <span class="chev dim">›</span>
            </button>
          {:else}
            <div class="polish-rows">
              {#each polishRows as row (row.slot)}
                <button type="button" class="polish-row" class:off={!polishOn} onclick={() => focusCharacterSource("polish")}>
                  <span class="enchant-row-label">{PART_SLOT_LABELS[row.slot]}</span>
                  <span class="polish-row-kind dim">{POLISH_KIND_LABELS[row.entry.kind]} {EQUIPMENT_STAT_SHORT[row.entry.stat]}</span>
                  <Value class="polish-row-amount" motion={() => row.amount} value={fmtSigned(row.amount)} />
                  <span class="chev dim">›</span>
                </button>
              {/each}
            </div>
          {/if}
          <p class="eq-note dim">
            研磨は<b>基本能力値</b>に合流します。記録の追加・変更はキャラタブの研磨ペイン。ON/OFF はバフ「装備研磨」と同じで、この計算だけの試し変更です(残すなら「試し変更を保存」)。
          </p>
          {/if}
          {/snippet}
        </Disclosure>


        <!-- 称号(試し変更)。所持している称号(キャラタブの称号ペインで登録)を並べる -->
        <Disclosure class="card" summaryClass="card-head toggle">
          {#snippet summary()}
            <Icon kind="title" id="title" size={20} label="称号" />
            <span class="card-title">称号</span>
            <span class="dim small title-head-note" use:changed={() => titleHeadNote}>{titleHeadNote}</span>
          {/snippet}
          {#snippet children(open)}
          {#if open}
          {#if titleChoices.length === 0}
            <button type="button" class="source-jump" onclick={() => focusCharacterSource("title")}>
              <span class="badge unknown">未登録</span>
              <span class="dim small">所持称号はキャラタブの称号ペインで登録</span>
              <span class="chev dim">›</span>
            </button>
          {:else}
            <!-- 所持称号から 1 つ選ぶ(§07「1 つ選ぶ」)。「なし」も候補の 1 行 -->
            <div class="title-picker">
              <Picker
                label="付ける称号"
                options={[
                  { value: "", name: "なし", meta: "称号を付けない" },
                  ...titleChoices.map((t) => ({ value: t.id, name: t.name, meta: titleNote(t) })),
                ]}
                bind:value={() => currentTitle?.id ?? "", (v) => selectTitle(v === "" ? null : v)}
              />
            </div>
            <button type="button" class="source-jump" onclick={() => focusCharacterSource("title")}>
              <span class="dim small">所持称号の追加・変更はキャラタブの称号ペインで</span>
              <span class="chev dim">›</span>
            </button>
          {/if}
          <p class="eq-note dim">称号は<b>基本能力値</b>に合流します。条件付き効果は記録するだけで計算に入りません。</p>
          {/if}
          {/snippet}
        </Disclosure>

        <!-- バフ -->
        <BuffCard {result} />

        <!-- コンボ。倍率A のコンボボーナスと中ディレイ半減は「{limits.combo_bonus_threshold} コンボ以上」で付くが、
             ユーザーが決めるのは「コンボするかどうか」なので、コンボ数は出さない。
             代わりに**成立条件**(間に通常攻撃を挟む)を ON のときだけ添える(§00 05 考えさせない) -->
        <div class="combo">
          <ToggleRow
            name="コンボする"
            value={fmtSignedPct(limits.combo_bonus_rate)}
            on={combo}
            tone="temp"
            onToggle={() => (combo = !combo)}
          />
          <p class="combo-note dim">
            ダメージ {fmtSignedPct(limits.combo_bonus_rate)} ・ 中ディレイ半分。
            <b>スキル → 通常攻撃 → スキル</b>のように、間に通常攻撃を挟むと成立します。
          </p>
          {#if combo}
            {#if normalAttackOptions.length > 0}
              <div class="combo-normal">
                <div class="field">
                  <span class="field-label">挟む通常攻撃</span>
                  <Choose
                    label="挟む通常攻撃"
                    options={normalAttackOptions}
                    cols={2}
                    bind:value={
                      () => normalAttackId ?? "",
                      (v) => (normalAttackOverride = v)
                    }
                  />
                </div>
              </div>
            {:else}
              <p class="combo-note dim">
                <span class="badge unknown">未収録</span>
                このキャラの通常攻撃は未収録なので、挟む通常攻撃ぶんの時間とダメージを出せません。
              </p>
            {/if}
            <p class="combo-note dim">
              1 秒あたりの火力は、挟む通常攻撃ぶんの時間とダメージも入れた 1 サイクルで出しています。
            </p>
          {/if}
        </div>

<style>
  /* コンボの成立条件と、挟む通常攻撃の段。ON のときだけ出るので、
     押した場所より下にしか増えない(§00 03) */
  .combo-normal { margin-top: 8px; }
  .combo-note { margin: 6px 0 0; font-size: 10px; line-height: 1.6; }

  /* 右カラム */
  .sim-bar { padding: 10px 11px; border-radius: var(--r-window); background: var(--bg-panel); border: 1px solid var(--border-soft); }
  .sim-bar.active { background: #F7F6FC; border-color: var(--sim); }
  .sim-line { display: flex; align-items: center; gap: 8px; }
  .sim-dot { flex-shrink: 0; width: 7px; height: 7px; border-radius: 50%; background: #9FB4D0; }
  .sim-dot.active { background: var(--sim); }
  /* 折り返させない。状態で文字数が違うので、折り返すと 1 行ぶん高さが変わって
     下の材料(装備・バフ)が吸い上げられる(§00 03 / §09 規則 1) */
  .sim-title {
    min-width: 0; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    font-size: var(--t-label); font-weight: 700; color: var(--fg-sub);
  }
  .sim-bar.active .sim-title { color: var(--sim-fg); }
  .sim-delta { flex-shrink: 0; min-width: 34px; text-align: right; font-size: 11px; font-weight: 700; color: transparent; }
  .sim-delta.on { color: var(--fg-dim); }
  .sim-delta.on.up { color: var(--good); }
  .sim-delta.on.down { color: var(--danger); }
  .sim-note-text { margin-top: 3px; font-size: 9px; line-height: 1.6; white-space: nowrap; }
  .sim-actions { margin-top: 8px; display: flex; gap: 7px; }
  /* 登録どおりのあいだは見せない。枠(高さ)だけ残す */
  .sim-actions.idle { visibility: hidden; }
  .sim-actions .btn { flex: 1; }

  /* 高さを固定する。件数で行が増えると、下にある材料(装備・バフ)がずれる */
  .sim-marks {
    display: flex; flex-wrap: nowrap; gap: 5px; min-height: 24px;
    overflow-x: auto; overscroll-behavior-x: contain;
  }
  .sim-mark {
    display: inline-flex; align-items: center; gap: 7px; padding: 3px 4px 3px 9px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--sim); box-shadow: 0 1px 0 rgba(109, 106, 168, 0.25);
    font-size: 10px; font-weight: 500;
  }
  .sim-revert {
    width: 16px; height: 16px; display: flex; align-items: center; justify-content: center;
    border-radius: 50%; background: var(--state-temp-bg); font-size: 9px; color: var(--sim);
  }
  .sim-revert:hover { background: var(--sim); color: #fff; }
  /* 上限の注記。色ではなく文言で伝える(ラベンダーに 2 つ目の意味を持たせない。§14 決定 6) */
  .sim-limit { min-height: 15px; padding: 0 11px 7px; font-size: 9.5px; color: var(--fg-dim); }
  .sim-limit.hit { color: var(--fg); font-weight: 700; }

  /* .card-head / .small は app.css */

  .eq-note { margin: 8px 0 0; font-size: 9.5px; line-height: 1.6; }

  /* 極限スキル(2 枠の行チップ)。§07 行チップ: 押して入れる / 押して外す */
  .ultimate-chips { margin-top: 8px; display: flex; flex-direction: column; gap: 6px; }

  /* 地力(覚醒・エタ / シャープネスビジョン / ソウルリンク)。ラベル幅と値幅を固定して、
     桁が増えても入力面が動かないようにする(§09 規則 4) */
  .basics-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 6px; }
  .basics-row { display: flex; align-items: center; gap: 7px; min-width: 0; }
  .basics-label { flex-shrink: 0; width: 46px; font-size: 9.5px; color: var(--fg-muted); }
  .basics-seg { min-width: 0; flex: 1; }
  /* .basics-val は ui/Value.svelte が描く子要素 */
  .basics-row :global(.basics-val) { flex-shrink: 0; min-width: 62px; text-align: right; font-size: 9.5px; font-weight: 700; color: var(--fg-dim); }

  /* 装備の切り替え。部位ごとに登録済み装備から 1 つ選ぶ(§07「1 つ選ぶ」= Picker。
     少なければチップ、増えたら候補面)。名前はチップ内で省略する(幅が動かない) */
  .switch-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 6px; }
  .switch-row { display: flex; align-items: flex-start; gap: 7px; min-width: 0; }
  .switch-row .enchant-row-label { flex-shrink: 0; width: 46px; padding-top: 5px; }
  .switch-picker { min-width: 0; flex: 1; }
  .title-head-note { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .title-picker { margin-top: 8px; }

  /* エンチャントの伸びしろ。部位ごとに 現在/上限/MAX での伸び幅を横並びで見せる */
  .enchant-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 6px; }
  .enchant-row { display: flex; flex-direction: column; gap: 4px; }
  .enchant-row-label { font-size: 10px; font-weight: 700; color: var(--fg-muted); }
  .enchant-row-cols { display: flex; flex-direction: column; gap: 5px; }
  .enchant-stat { display: flex; align-items: center; gap: 7px; }
  .enchant-stat-label { flex-shrink: 0; width: 30px; font-size: 9.5px; color: var(--fg-muted); }
  /* .enchant-gain は ui/Value.svelte が描く子要素 */
  .enchant-stat :global(.enchant-gain) { flex-shrink: 0; min-width: 84px; text-align: right; font-size: 9.5px; color: var(--fg-dim); }
  .enchant-stat :global(.enchant-gain.tone-up) { color: var(--good); font-weight: 700; }
  /* 未登録・上限未入力の行。押すとキャラタブの該当ペインへ飛ぶ(欠けは badge.unknown で言う) */
  .source-jump {
    display: flex; align-items: center; gap: 8px; padding: 2px 0; background: none; border: none;
    cursor: pointer; text-align: left; width: 100%;
  }
  .polish-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 4px; }
  .polish-row {
    display: flex; align-items: center; gap: 8px; padding: 2px 0; background: none; border: none;
    cursor: pointer; text-align: left; width: 100%; min-width: 0;
  }
  .polish-row .enchant-row-label { flex-shrink: 0; width: 46px; }
  .polish-row-kind { min-width: 0; flex: 1; font-size: 9.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* .polish-row-amount は ui/Value.svelte が描く子要素 */
  .polish-row :global(.polish-row-amount) { flex-shrink: 0; min-width: 40px; text-align: right; font-size: 10px; font-weight: 700; }
  .polish-row.off :global(.polish-row-amount) { color: var(--fg-muted); text-decoration: line-through; }

  .enchant-dep { margin: 6px 0 0; font-size: 9px; }
  /* カード見出しの開閉(ui/Disclosure の <summary>)。押しても見出し自身は動かず、
     中身がその下に生えるだけ。キャレットは部品が置くので、ここは字寸も向きも持たない */
  :global(summary.card-head.toggle) {
    width: 100%; padding: 0; border: 0; background: none; text-align: left; cursor: pointer;
  }
  :global(summary.card-head.toggle:hover .card-title) { color: var(--accent); }

</style>
