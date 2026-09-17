<script lang="ts">
  // ダメージ計算: v4 の縦フロー「相手を選ぶ → この一発 → もし〜だったら → なぜこの数字？」。
  // 右カラムは「計算の材料」(試し変更・バフ・入場条件)。計算はすべて Rust 側(preview_damage)。
  import { tick, untrack } from "svelte";
  import {
    errorMessage, evaluateContents, listEnchantGains, listSkills, listUpgradeCandidates, previewDamage,
    listBlockedBuffs, previewDefense, previewPotentialEffects,
  } from "../../api/commands";
  import type {
    BlockedBuff, BuffSelection,
    StatSources, CommonSkills,
    Adjustments, BuffChoice, BuffDefinition, BuffPurpose, CategoryTrace, ComboSkillType, ContentEvaluation, DamageCategory,
    DamageContribution, DamageResult, DefenseProfile, EquipmentPart, EquipmentStatKind, EquipmentValues, FormulaStep, NewCharacter, PartSlot, Skill, TitleDef,
    SoulLinkPreview, StatKind, UltimateSkill, UpgradeCandidate,
  } from "../../api/types";
  import {
    BUFF_PURPOSES, buffSetOptions, isChoiceValue, isMultiTarget, isPercentLayer, isUserSelectedTarget,
    matchesPurpose,
    pickedStats, toggleBuff, toggleBuffStat, userInputRange,
  } from "../../buffs";
  import {
    enchantCap, enchantDepKeysFor, enchantRows as enchantRowsOf, ENCHANT_SLOT_LABELS, setEnchantValue,
    type EnchantDepKey,
  } from "../../enchant";
  import { ETERNAL_MILESTONES } from "../../draft";
  import { equipmentIconId, polishAmount, selectedEquipmentPartOrNeutral } from "../../equipment";
  import { fmtDuration, fmtInt, fmtNum, fmtPct, fmtRate, fmtSigned, fmtSignedPct, formatLayerValue, topRowsText } from "../../format";
  import {
    ELEMENT_LABELS, EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_LABELS, EQUIPMENT_STAT_SHORT, PART_SLOT_LABELS, PART_SLOTS,
    POLISH_ALLOWED_SLOTS, POLISH_KIND_LABELS, STAT_KINDS, STAT_LABELS,
    STAT_LAYER_LABELS, ULTIMATE_SKILLS, ULTIMATE_SKILL_LABELS,
  } from "../../labels";
  import { limits } from "../../limits.svelte";
  import { tables } from "../../tables.svelte";
  import {
    app, enqueueCharacterSave, flatContents, focusCharacterSource, payloadOf, selectedCharacter, simIsDirty,
    upsertCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import Chip from "../../ui/Chip.svelte";
  import Disclosure from "../../ui/Disclosure.svelte";
  import Icon from "../../ui/Icon.svelte";
  import Value from "../../ui/Value.svelte";
  import DefensePanel from "./DefensePanel.svelte";
  import Picker from "../../ui/Picker.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";
  import Choose from "../../ui/Choose.svelte";
  import ToggleRow from "../../ui/ToggleRow.svelte";
  import Popover from "../../ui/Popover.svelte";
  import SplitPage from "../../ui/SplitPage.svelte";
  import { latest } from "../../ui/latest.svelte";
  import { changed } from "../../ui/motion.svelte";
  import { ChangeMemo, PresenceMemo, swapNote, type Presence } from "../../ui/presence";
  import { critChanceStage } from "../../ui/critChance";
  import { badgeStyle, REACH_BADGES, REACH_STATE, reachOk, STATE, type Badge } from "../../ui/states";
  import NumberField from "../../ui/NumberField.svelte";
  import TracePanel from "./TracePanel.svelte";

  const DEFAULT_RIGHT_WIDTH = 380;

  const COMBO_SKILL_TYPE_OPTIONS = [
    { value: "general", label: "一般" },
    { value: "instant", label: "瞬撃" },
    { value: "chain", label: "連撃" },
  ];

  const character = $derived(selectedCharacter());
  const savedPayload = $derived(character ? payloadOf(character) : null);
  const payload = $derived(app.sim ?? savedPayload);
  const weaponOf = (p: NewCharacter) => selectedEquipmentPartOrNeutral(p.equipment.parts.weapon);

  // --- 対象(コンテンツ) --------------------------------------------------
  // ダメージ計算には敵データが要るので、enemy_id を持つコンテンツだけを対象に出す
  // (敵未収録のコンテンツはホームで入場条件のみ判定する)。
  const contents = $derived(
    flatContents().filter(
      (x): x is typeof x & { content: { enemy_id: string } } => x.content.enemy_id !== null,
    ),
  );
  // 対象ピッカーも同じ絞り込みで描画する(選べない行を一覧に残さない)。
  // 敵が 1 件も無いエリアは見出しごと落とす。
  const targetAreas = $derived(
    app.areas
      .map((a) => ({ ...a, contents: a.contents.filter((c) => c.enemy_id !== null) }))
      .filter((a) => a.contents.length > 0),
  );
  const targetIndex = $derived(
    Math.max(0, contents.findIndex((x) => x.content.id === app.calcTargetId)),
  );
  const target = $derived(contents[targetIndex] ?? null);
  function stepTarget(dir: number) {
    if (contents.length === 0) return;
    app.calcTargetId = contents[(targetIndex + dir + contents.length) % contents.length].content.id;
  }

  // --- スキル(キャラタブの主軸スキルが正) --------------------------------
  let skills = $state<Skill[]>([]);
  // 取得済みスキルが属するキャラ種(非リアクティブ)。キャラ種が変わった瞬間に一覧を
  // 同期的に空へ戻す。残すと listSkills の応答まで「別キャラのステ × 前キャラのスキル」で
  // 計算・表示されてしまう(Rust 側はスキル所有チェックをしない。PR レビュー指摘)。
  let skillsGid: string | null = null;
  $effect(() => {
    const gid = character?.game_character_id ?? null;
    if (gid === skillsGid) return; // 保存等でキャラのオブジェクトだけ変わった場合は選択を保つ
    skillsGid = gid;
    skills = [];
    skillOverride = null;
    if (!gid) return;
    listSkills(gid)
      .then((list) => {
        if (skillsGid !== gid) return; // 切替済みの古い応答は捨てる
        skills = list;
      })
      .catch((e) => reportError(errorMessage(e)));
  });
  /** キャラタブで選んだ主軸スキル。この画面のスキルはこれが正 */
  const mainSkill = $derived(skills.find((s) => s.id === character?.main_skill_id) ?? null);
  /**
   * この画面での選び直し(例外操作)。null = 主軸に従う。
   * キャラを替えたとき・キャラタブで主軸を変えたときは主軸に揃え直す(下の $effect)。
   * 保存はしない ＝ ラベンダー(--sim)で見せる。
   */
  let skillOverride = $state<string | null>(null);
  let lastMainSkillId = untrack(() => character?.main_skill_id ?? null);
  $effect(() => {
    const id = character?.main_skill_id ?? null;
    if (id === lastMainSkillId) return;
    lastMainSkillId = id;
    skillOverride = null;
  });
  // 主軸が未設定・未収録のときだけ先頭スキルにフォールバックする
  const skillId = $derived(
    (skillOverride !== null && skills.some((s) => s.id === skillOverride) ? skillOverride : null)
      ?? mainSkill?.id
      ?? skills[0]?.id
      ?? "",
  );
  const skill = $derived(skills.find((s) => s.id === skillId) ?? null);
  let comboSkillType = $state<ComboSkillType>("general");
  const selectedComboSkillType = $derived<ComboSkillType | null>(
    skill && skill.combo_variants.length > 0 ? comboSkillType : null,
  );
  let lastComboSkillId = untrack(() => skillId);
  $effect(() => {
    if (skillId === lastComboSkillId) return;
    lastComboSkillId = skillId;
    comboSkillType = "general";
  });
  /** 主軸と違うスキルで計算している状態 */
  const skillOverridden = $derived(mainSkill !== null && skillId !== mainSkill.id);
  let skillOpen = $state(false);
  /** ピッカーの並びは合計ダメージの降順(v4 指定)。合計が未取得のものは登録順で末尾 */
  const pickerSkills = $derived(
    [...skills].sort((a, b) => (skillTotals[b.id]?.total ?? -1) - (skillTotals[a.id]?.total ?? -1)),
  );

  // スキル一覧の対象別ダメージ(ドロップダウンを開いたときに計算)
  let skillTotals = $state<Record<string, { perHit: number; total: number }>>({});
  const skillLatest = latest();
  $effect(() => {
    // 対象・キャラ・試し変更が変わったら古い合計を出さない(PR レビュー指摘)
    skillTotals = {};
    if (!skillOpen || !payload || !target || skills.length === 0) return;
    const p = JSON.parse(JSON.stringify(payload)) as NewCharacter;
    const temp = JSON.parse(JSON.stringify(NEUTRAL_ADJUSTMENTS)) as Adjustments;
    const contentId = target.content.id;
    const comboCount = combo ? limits.combo_bonus_threshold : 0;
    const selectedSkillId = skillId;
    const comboType = selectedComboSkillType;
    const buffs = JSON.parse(JSON.stringify(app.calcBuffs));
    skillLatest.run((isCurrent) =>
      Promise.all(
        skills.map(async (s) => [
          s.id,
          await previewDamage(
            p, s.id, contentId, comboCount, temp,
            s.id === selectedSkillId ? comboType : (s.combo_variants.length > 0 ? "general" : null),
            buffs,
          ),
        ] as const),
      )
        .then((rs) => {
          if (!isCurrent()) return;
          skillTotals = Object.fromEntries(
            rs.map(([id, r]) => [id, { perHit: r.per_hit_primary, total: r.total_primary }]),
          );
        })
        .catch((e) => reportError(errorMessage(e))),
    );
  });

  let combo = $state(false);

  // --- コンボで挟む通常攻撃 --------------------------------------------------
  // コンボボーナス(倍率A・中ディレイ半減)は通常攻撃を挟んで初めて成立する。
  // どれを挟むかで CI(次のスキルの中ディレイの下限)が変わるので、速い順に段で並べ、
  // **既定は一番速いもの**にする(ユーザーに選ばせないのが既定。§ux 原則 1)。
  let normalAttackOverride = $state<string | null>(null);
  const normalAttacks = $derived(
    [...skills.filter((s) => s.normal_attack)].sort(
      (a, b) => (a.combo_interval ?? Infinity) - (b.combo_interval ?? Infinity),
    ),
  );
  const normalAttackOptions = $derived(
    normalAttacks.map((s) => ({
      value: s.id,
      // 「†極・突き」の飾りは段では邪魔なので落とし、CI を添える(未収録は ?)
      label: `${s.name.replace(/^†[^・]*・/, "")} ${s.combo_interval !== null ? fmtNum(s.combo_interval, 2, "s") : "?"}`,
    })),
  );
  const normalAttackId = $derived(
    normalAttacks.some((s) => s.id === normalAttackOverride)
      ? normalAttackOverride
      : (normalAttacks[0]?.id ?? null),
  );
  const comboNormalAttackId = $derived(combo ? normalAttackId : null);

  // --- 一時調整 -------------------------------------------------------------
  // この画面に編集 UI は無い(「調整(一時)」カードは削除済み)。previewDamage 系コマンドは
  // Adjustments を必須パラメータとして取るため、中立値を渡す。
  const NEUTRAL_ADJUSTMENTS: Adjustments =
    Object.fromEntries(STAT_KINDS.map((k) => [k, { add: 0, pin: null }])) as Adjustments;
  // キャラを切り替えたらスキルの選び直しをリセット(前のキャラの選択を引き継がない)
  let lastCharacterId = untrack(() => character?.id);
  $effect(() => {
    const id = character?.id;
    if (id === lastCharacterId) return;
    lastCharacterId = id;
    skillOverride = null;
  });

  // --- 計算(payload と saved の両方) -------------------------------------
  let result = $state<DamageResult | null>(null);
  let savedResult = $state<DamageResult | null>(null);
  let calculating = $state(false);
  const requestLatest = latest({ debounce: 120 });
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null; // sim のネスト変更も拾う
    const sp = savedPayload;
    const t = target;
    const sid = skillId;
    const comboCount = combo ? limits.combo_bonus_threshold : 0;
    const comboType = selectedComboSkillType;
    const normalId = comboNormalAttackId;
    const simActive = app.sim !== null;
    const tempJson = JSON.stringify(NEUTRAL_ADJUSTMENTS);
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson || !sp || !t || !sid) {
      requestLatest.cancel();
      result = null;
      savedResult = null;
      return;
    }
    calculating = true;
    requestLatest.run(async (isCurrent) => {
      try {
        const main = await previewDamage(
          JSON.parse(pJson), sid, t.content.id, comboCount, JSON.parse(tempJson), comboType, JSON.parse(buffsJson),
          normalId,
        );
        const saved = simActive
          ? await previewDamage(
              sp, sid, t.content.id, comboCount, JSON.parse(tempJson), comboType, JSON.parse(buffsJson),
              normalId,
            )
          : main;
        if (isCurrent()) {
          result = main;
          savedResult = saved;
        }
      } catch (e) {
        if (isCurrent()) {
          result = null;
          reportError(errorMessage(e));
        }
      } finally {
        if (isCurrent()) calculating = false;
      }
    });
    return () => requestLatest.cancel();
  });

  // --- 入場条件・通るのは(payload 基準、Rust 側で判定) --------------------
  let evals = $state<ContentEvaluation[]>([]);
  const evalLatest = latest({ debounce: 200 });
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null;
    // 計算タブは「今このスキルで戦う」文脈なので、装備条件も選択中スキルの依存で判定する
    // (ホームはコンテンツごとの最大ダメージスキルで判定する)。
    const sid = skillId;
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson) {
      evalLatest.cancel();
      evals = [];
      return;
    }
    evalLatest.run((isCurrent) => {
      evaluateContents(JSON.parse(pJson), sid || undefined, JSON.parse(buffsJson))
        .then((rs) => {
          if (isCurrent()) evals = rs;
        })
        .catch((e) => reportError(errorMessage(e)));
    });
    return () => evalLatest.cancel();
  });
  const targetEval = $derived(target ? (evals.find((e) => e.content_id === target.content.id) ?? null) : null);

  // --- 表示値 -------------------------------------------------------------
  // 主役の値の選び方(クリ発生率 > 0 ならクリティカル、0 なら非クリ最大)は Rust 側
  // (DamageTriple::primary)に一元化済み。ここは per_hit_primary / total_primary を読むだけ。
  // critMode はトレースの段・内訳表示の切替(表示都合)にだけ使う。
  const critMode = $derived((result?.critical_chance ?? 0) > 0);
  const pick = <T extends { max: number; critical: number }>(t: T | null | undefined): number | null =>
    t ? (critMode ? t.critical : t.max) : null;
  const perHit = $derived(result?.per_hit_primary ?? null);
  const savedPerHit = $derived(savedResult?.per_hit_primary ?? null);
  const totalValue = $derived(result?.total_primary ?? null);
  const dpsValue = $derived(pick(result?.dps));
  const deltaPct = $derived(
    perHit !== null && savedPerHit !== null && savedPerHit > 0
      ? Math.round((perHit / savedPerHit - 1) * 100)
      : 0,
  );
  // 討伐時間の目安。段の境目は Rust が配る(tables.reach_seconds)。画面は写経しない
  const closeSeconds = $derived(tables.reach_seconds.close);
  const defeatSeconds = $derived(result?.defeat_seconds ?? null);
  /** メーター比。討伐時間が長いほど伸びる(0 = 一瞬、100% = 目安ぴったり) */
  const meterRatio = $derived(defeatSeconds !== null ? Math.min(1, defeatSeconds / closeSeconds) : 0);
  const hasReqs = $derived((target?.content.requirements.length ?? 0) > 0);
  // 評価が未取得の間は入場条件を「不明」として扱い、未達コンテンツに「通る/余裕」を
  // 出さない(ダメージ 120ms・評価 200ms のデバウンス差で毎回この窓が開く。PR レビュー指摘)
  const entryKnown = $derived(!hasReqs || targetEval !== null);
  const entryOk = $derived(!hasReqs || (targetEval?.entry_ok ?? false));
  /** 目安に対する到達段(Rust 側の判定。目安なしは null) */
  const reach = $derived(result?.reach ?? null);
  const reached = $derived(reachOk(reach));
  const badgeState = $derived.by(() => {
    if (perHit === null || !entryKnown || reach === null) return 6;
    if (hasReqs && !entryOk) return reached ? 5 : 4;
    return REACH_STATE[reach];
  });
  // 言葉はこの画面のもの、色は 6 系統から選ぶ(design-system §03)。先頭 6 件は共通(ui/states.ts)
  const BADGE: Badge[] = [...REACH_BADGES, { label: "判定中", state: "unknown" }];

  // --- なぜこの数字?(トレースの式から組み立て) ---------------------------
  // 主役がクリティカル前提なら、トレースの段もクリティカル側の到達値で揃える。
  const steps = $derived(critMode ? (result?.trace.steps_critical ?? []) : (result?.trace.steps_max ?? []));
  const stepValue = (name: string): number | null =>
    steps.find((s) => s.name === name)?.value ?? null;
  // 攻撃力(A)の内訳は Rust の AttackPowerBreakdown をそのまま使う(UI で式を持たない)
  const atk = $derived(result?.trace.attack ?? null);
  const atkA = $derived(atk?.value ?? null);
  const atkRows = $derived.by(() => {
    if (atk === null) return [];
    const raw = [
      { k: "ステ攻撃力", v: atk.stat_attack, c: "var(--flow-base)", note: "素ステ・補正源から" },
      { k: "装備攻撃力", v: atk.equipment_attack, c: "var(--flow-1)", note: "基本/強化 × 依存別係数" },
      { k: "装備攻撃力強化倍率", v: atk.enhance_bonus, c: "var(--flow-2)", note: "パワーW・ストロングW" },
    ].filter((x) => x.v > 0);
    const total = raw.reduce((a, x) => a + x.v, 0) || 1;
    let running = 0;
    return raw.map((x, i) => {
      running += x.v;
      return {
        ...x,
        pct: fmtNum(Math.max(1.5, (x.v / total) * 100), 2, "%"),
        share: fmtPct(x.v / total),
        // 最後の段は必ず A に着地させる(切捨ての端数で足し算が合わなくなるのを防ぐ)
        to: i === raw.length - 1 ? atk.value : running,
      };
    });
  });
  const defenseValue = $derived(
    result?.trace.categories.find((c) => c.symbol === "C")?.value ?? null,
  );
  const pierced = $derived(stepValue("攻撃力−防御力"));
  const noPierce = $derived(pierced !== null && pierced <= 0);
  const defShare = $derived(
    atkA !== null && defenseValue !== null && atkA > 0
      ? Math.min(97, (defenseValue / atkA) * 100)
      : 0,
  );

  interface FlowRow {
    k: string;
    add: number;
    mult: string;
    /** 倍率の実数(前段との比)。段の順序に依存しない「効き」の指標 */
    factor: number;
    c: string;
    /** その段までの到達値 */
    to: number;
    /** 対応する Rust の段名。材料(カテゴリ)はこの段の `categories` から引く */
    step: string;
  }
  const FLOW_COLORS: Record<string, string> = {
    "スキル倍率": "var(--flow-1)",
    "クリティカル": "var(--flow-2)",
    "コンボ・属性・カット率・オーラ": "var(--flow-3)",
    "最終ダメージ固定値(下限)": "var(--flow-4)",
    "最終ダメージ・カット率A・被害減少": "var(--flow-5)",
    "各種ダメージ増減": "var(--flow-6)",
    "攻撃ダメージ・PVP補正": "var(--flow-7)",
  };
  const flowRows = $derived.by<FlowRow[]>(() => {
    if (pierced === null) return [];
    let running = pierced;
    const rows: FlowRow[] = [
      { k: "防御を抜けた攻撃力(素通り)", add: pierced, mult: "—", factor: 1, c: "var(--fg-dim)", to: pierced, step: "攻撃力−防御力" },
    ];
    for (const s of steps) {
      if (s.kind !== "factor" && s.kind !== "running") continue;
      // 段の種別は Rust の FormulaStep.kind。到達値は FormulaStep.reached。
      // 倍率列は倍率の段はその値、到達値で返る段は前段との比(表示用)
      const isFactor = s.kind === "factor";
      const factor = isFactor ? s.value : running > 0 ? s.reached / running : 1;
      const mult = isFactor || running > 0 ? fmtRate(factor) : "—";
      rows.push({ k: s.name, add: s.reached - running, mult, factor, c: FLOW_COLORS[s.name] ?? "var(--fg-dim)", to: s.reached, step: s.name });
      running = s.reached;
    }
    return rows;
  });
  const flowTotal = $derived(flowRows.reduce((a, r) => a + Math.max(0, r.add), 0) || 1);
  const flowMultLabel = $derived(
    pierced !== null && pierced > 0 && perHit !== null ? fmtRate(perHit / pierced, 1) : "—",
  );
  let flowOpen = $state(false);
  /** 直近の計算で変わった段(鎖の ↑ から辿る先)。副作用で親 → 子 を控える。
   *  段の「足した分」は前段が変われば全部変わる(結果)ので、段自身の倍率で判定する(原因)。
   *  倍率を持たない先頭の段(素通り)だけは値で判定する */
  const changedFlowKeys = $derived.by(() => {
    const own = (f: FlowRow) => (f.mult === "—" ? Math.round(f.add) : Math.round(f.factor * 10000));
    const keys = flowRows.filter((f) => changes.touch(`flow:${f.k}`, own(f), result)).map((f) => `flow:${f.k}`);
    changedChildren.set("perHit", keys);
    return keys;
  });
  const changedAtkKeys = $derived.by(() => {
    const keys = atkRows.filter((a) => changes.touch(`atk:${a.k}`, Math.round(a.v), result)).map((a) => `atk:${a.k}`);
    changedChildren.set("atkA", keys);
    return keys;
  });

  // 倍率の材料(非中立カテゴリ)
  const activeCategories = $derived(
    (result?.trace.categories ?? []).filter((c) => c.kind !== "assigned" && c.value !== 0),
  );
  const catAtCap = (c: (typeof activeCategories)[number]) =>
    !!c.cap && c.cap.max !== null && c.value >= c.cap.max - 1e-9;
  const fmtCatValue = (c: (typeof activeCategories)[number]) =>
    c.kind === "rate" ? fmtSignedPct(c.value, { max: 4 }) : fmtNum(c.value);
  /** 上限で捨てられた分(生の合算値 − 上限適用後)。0 なら捨てていない */
  const catLoss = (c: (typeof activeCategories)[number]) => c.raw - c.value;
  const fmtCatRaw = (c: (typeof activeCategories)[number]) =>
    c.kind === "rate" ? fmtSignedPct(c.raw, { max: 4 }) : fmtNum(c.raw);
  const fmtCatLoss = (c: (typeof activeCategories)[number]) => {
    const loss = catLoss(c);
    return c.kind === "rate" ? fmtSignedPct(-loss, { max: 4 }) : fmtSigned(-loss, { max: 4 });
  };
  const cappedCategories = $derived(activeCategories.filter((c) => catLoss(c) > 1e-9));
  // 「一番効いている / 次に伸ばす」の規則は Rust 側(`damage_levers` / `DamageCategory::is_effort`)。
  // ここは結果をカテゴリ行に引き当てて出すだけ
  const categoryById = (id: DamageCategory | null) =>
    id === null ? null : (activeCategories.find((c) => c.category === id) ?? null);
  const topLever = $derived(categoryById(result?.levers.top ?? null));
  const bestLevers = $derived(
    (result?.levers.candidates ?? []).flatMap((lc) => {
      const c = categoryById(lc.category);
      return c ? [{ ...c, gain: lc.gain_percent, headroom: lc.headroom }] : [];
    }),
  );
  const bestLever = $derived(bestLevers[0] ?? null);
  /** 次の候補(2 位以降)。押した行の下に開く */
  const nextLevers = $derived(bestLevers.slice(1));
  let nextLeversOpen = $state(false);
  /** 積み上げの助言(いま効いている / 次に伸ばす)を開いているか。ふだんは畳む */
  let leverOpen = $state(false);
  type Lever = (typeof bestLevers)[number];
  /** +1% 足したときの最終ダメージの伸び(%)(Rust `LeverCandidate::gain_percent`) */
  const leverGain = (c: Lever) => c.gain;
  const bestLeverGain = $derived(bestLever ? leverGain(bestLever) : 0);
  const fmtHeadroom = (c: Lever) =>
    c.headroom !== null ? `上限まで あと ${fmtNum(c.headroom * 100)}%` : "上限なし";
  /** topLever が乗っている段(帯の行を太字にするため) */
  const topLeverStep = $derived(
    topLever ? (steps.find((s) => s.categories.includes(topLever.category as DamageCategory))?.name ?? null) : null,
  );

  // --- 数値を開いて詳細を確認する(§00 03: 開くのは押した行の下だけ) ----------
  // 値はすべて Rust 由来(DamageTrace / DamageResult)。UI で作るのは 2 値の差分だけ。
  /** 内訳 1 行。列は band-row と同じ段(ラベル / 倍率 / 実数 / 補足) */
  interface Mat {
    label: string;
    mult?: string;
    value: string;
    sub?: string;
    /** `value` の数値(表示単位。% 表示なら 100 倍した値)。変わったら動かす + いくつ変わったかを出す(§00 04)ためだけに使う */
    n?: number;
    /** 差分タグに付ける単位("%" など)。`n` が `value` と同じ単位のときだけ */
    unit?: string;
    /** 押すと直下に `subs` が開く行。省略なら開かない行 */
    key?: string;
    subs?: Mat[];
    /** 供給源の行の出入り。抜けた行は次に集合が変わるまで残す(ui/presence.ts) */
    state?: Presence;
    /** 供給源の行の一意キー(`{#each}` 用) */
    id?: string;
    /** 直近の計算で値か供給源が変わった行。↑ を押して辿る先(ui/presence.ts ChangeMemo) */
    changed?: boolean;
    /** 直近で入れ替わった供給源(「称号【A】 → 称号【B】」)。閉じたままでもどこに効いたか分かる */
    note?: string;
  }
  /** 供給源の行の出入りをカテゴリごとに覚える(reactive にしない) */
  const contributionPresence = new PresenceMemo<DamageContribution>();
  // --- 緑を辿る: ↑↓ を押すと、その下で変わった行だけを順に開く(ユーザー要望 2026-09-15)。
  //     何が変わったかは描画時に ChangeMemo で判定し、親キー → 変わった子キー を控えておく
  const changes = new ChangeMemo();
  const changedChildren = new Map<string, string[]>();
  const register = (parentKey: string, d: Detail): Detail => {
    changedChildren.set(parentKey, d.mats.filter((m) => m.changed && m.key).map((m) => m.key!));
    return d;
  };
  async function followChange(key: string) {
    if (key === "perHit" || key === "atkA") flowOpen = true;
    else if (!isDetailOpen(key)) openDetails = [...openDetails, key];
    await tick(); // 開いて描画されてから、その中で控えた「変わった子」を読む
    for (const child of changedChildren.get(key) ?? []) await followChange(child);
  }
  interface Detail {
    /** この段の倍率(×n) */
    mult: string;
    /** この段が足した実数(+n / −n)。倍率だけの段は null */
    delta: number | null;
    /** その段までの到達値 */
    to: number | null;
    mats: Mat[];
    /** 中立(±0)で出さなかったカテゴリ枠の数 */
    idle: number;
    /** Rust の式(FormulaStep.expression) */
    expr: string | null;
  }
  /** 開いている内訳。行ごとに独立させる(1 つ開いても他は閉じない ＝ 押した行が動かない) */
  let openDetails = $state<string[]>([]);
  const isDetailOpen = (key: string) => openDetails.includes(key);
  /** ui/Disclosure の bind:open 用。開閉そのものは <details> が持ち、ここは覚えるだけ */
  function setDetailOpen(key: string, open: boolean) {
    if (open === isDetailOpen(key)) return;
    openDetails = open ? [...openDetails, key] : openDetails.filter((k) => k !== key);
  }
  function toggleDetail(key: string) {
    openDetails = isDetailOpen(key) ? openDetails.filter((k) => k !== key) : [...openDetails, key];
  }
  const stepOf = (name: string): FormulaStep | null => steps.find((s) => s.name === name) ?? null;
  const categoryOf = (c: DamageCategory): CategoryTrace | null =>
    result?.trace.categories.find((x) => x.category === c) ?? null;
  /** カテゴリX 攻撃ダメージは子(X1〜X6)の合計で、供給源は子に積まれる(domain category.rs ATTACK_DAMAGE_CHILDREN) */
  const ATTACK_DAMAGE_CHILDREN: DamageCategory[] = [
    "attack_damage_isabel", "attack_damage_general", "attack_damage_basic_trigger",
    "attack_damage_skill", "attack_damage_special", "attack_damage_japan",
  ];
  /** カテゴリに実際に値を足した供給源(トレースの category_contributions から)。X は子の供給源をまとめて返す */
  const catContributions = (c: string) => {
    const all = result?.trace.category_contributions ?? [];
    return c === "attack_damage_rate"
      ? all.filter((x) => ATTACK_DAMAGE_CHILDREN.includes(x.category))
      : all.filter((x) => x.category === c);
  };
  const fmtContributionValue = (kind: CategoryTrace["kind"], v: number) =>
    kind === "rate" ? fmtSignedPct(v, { max: 4 }) : fmtNum(v);
  const catMat = (c: CategoryTrace): Mat => {
    // A 攻撃力は ① と同じ構成(ステ攻撃力 / 装備攻撃力 / 強化倍率)で開く。供給源 1 行では読めない
    if (c.category === "attack_power" && atkRows.length > 0) {
      return {
        label: `${c.symbol} ${c.label}`,
        value: fmtCatValue(c),
        n: c.value,
        changed: changes.touch("cat:attack_power", c.value, result),
        key: "cat:attack_power",
        subs: atkRows.map((a) => ({
          label: a.k,
          value: fmtInt(Math.round(a.v)),
          sub: a.note,
          n: Math.round(a.v),
        })),
      };
    }
    const contributions = contributionPresence.mark(c.category, catContributions(c.category), (x) => x.source);
    const names = (st: Presence) => contributions.filter((x) => x.state === st).map((x) => x.item.source);
    const n = c.kind === "rate" ? c.value * 100 : c.value;
    return {
      changed: changes.touch(`cat:${c.category}`, n, result) || contributions.some((x) => x.state !== "same"),
      label: `${c.symbol} ${c.label}`,
      mult: c.kind === "rate" ? `×${fmtNum(c.factor)}` : undefined,
      value: fmtCatValue(c),
      n,
      unit: c.kind === "rate" ? "%" : undefined,
      sub: catLoss(c) > 1e-9 ? `上限で ${fmtCatLoss(c)}` : undefined,
      note: swapNote(names("gone"), names("added")),
      key: contributions.length > 0 ? `cat:${c.category}` : undefined,
      subs:
        contributions.length > 0
          ? contributions.map(({ item: x, state, key }) => ({
              id: key,
              label: x.source,
              value: fmtContributionValue(c.kind, x.value),
              n: c.kind === "rate" ? x.value * 100 : x.value,
              unit: c.kind === "rate" ? "%" : undefined,
              sub: x.category === c.category ? undefined : categoryOf(x.category)?.label,
              state,
            }))
          : undefined,
    };
  };
  /** 段の内訳。材料は Rust が段ごとに申告したカテゴリ(FormulaStep.categories)から引く */
  function stepDetail(name: string, mult: string, delta: number | null, to: number | null): Detail {
    const step = stepOf(name);
    const cats = (step?.categories ?? []).map(categoryOf).filter((c): c is CategoryTrace => c !== null);
    const active = cats.filter((c) => c.kind === "assigned" || c.value !== 0);
    return {
      mult, delta, to,
      mats: active.map(catMat),
      idle: cats.length - active.length,
      expr: step?.expression ?? null,
    };
  }
  /**
   * ステ 1 つに効かせている要因の一覧(素ステ + 補正源 + 上限で捨てた分)。
   * 実数(何ポイント動かしたか)は Rust の `StatSourceEffect.effect`(上限込み)。UI で再計算しない。
   * 素ステ + Σ実数 = 最終能力値。
   */
  function statFactorMats(kind: StatKind): Mat[] {
    const st = result?.trace.stats.find((s) => s.kind === kind);
    if (!st) return [];
    const mats: Mat[] = [
      { label: "素ステ(振り分け)", value: fmtInt(st.base), n: st.base },
    ];
    for (const c of result?.trace.stat_source_effects ?? []) {
      if (c.kind !== kind) continue;
      mats.push({
        label: c.source,
        value: fmtSigned(c.effect, { max: 3 }),
        sub: `${STAT_LAYER_LABELS[c.layer]} ${formatLayerValue(c.layer, c.value)}`,
        n: c.effect,
      });
    }
    if (st.capped_loss > 0) {
      mats.push({
        label: "上限で捨てた分",
        value: fmtSigned(-st.capped_loss, { max: 3 }),
        sub: `上限 ${fmtInt(st.stat_cap)}`,
        n: -st.capped_loss,
      });
    }
    if (st.pinned_from !== null) {
      mats.push({
        label: "一時調整で固定",
        value: fmtInt(st.effective),
        sub: `固定前 ${fmtInt(st.pinned_from)}`,
        n: st.effective,
      });
    }
    return mats;
  }
  const EQUIPMENT_ATTACK_LAYER_LABELS: Record<string, string> = { base: "基本", enhanced: "強化" };
  /** 攻撃力の構成行の内訳。ステ攻撃力は「実際に使っている依存ステ」だけを並べ、押すと要因まで開く */
  function atkDetail(a: (typeof atkRows)[number]): Detail {
    const mats: Mat[] = [];
    if (a.k === "ステ攻撃力") {
      for (const p of result?.trace.stat_attack_parts ?? []) {
        mats.push({
          label: STAT_LABELS[p.kind],
          mult: `×${fmtNum(p.coefficient)}`,
          value: fmtInt(Math.round(p.contribution)),
          sub: `能力値 ${fmtInt(p.effective)}`,
          n: Math.round(p.contribution),
          key: `atkstat:${p.kind}`,
          subs: statFactorMats(p.kind),
        });
      }
    } else if (a.k === "装備攻撃力") {
      for (const p of result?.trace.equipment_attack_parts ?? []) {
        mats.push({
          label: `${EQUIPMENT_ATTACK_LAYER_LABELS[p.layer]} ${EQUIPMENT_STAT_LABELS[p.value]}`,
          mult: `×${fmtNum(p.coefficient)}`,
          value: fmtInt(Math.round(p.contribution)),
          sub: `装備値 ${fmtInt(p.amount)}`,
          n: Math.round(p.contribution),
          key: `eqatk:${p.layer}:${p.value}`,
          subs: p.sources.map((s) => ({
            label: s.source,
            mult: `×${fmtNum(p.coefficient)}`,
            value: fmtInt(Math.round(s.contribution)),
            sub: `装備値 ${fmtInt(s.amount)}`,
            n: Math.round(s.contribution),
          })),
        });
      }
    } else if (a.k === "装備攻撃力強化倍率") {
      for (const s of result?.trace.equipment_enhance_sources ?? []) {
        mats.push({
          label: s.source,
          mult: fmtSignedPct(s.value, { max: 4 }),
          value: fmtSignedPct(s.value, { max: 4 }),
          n: s.value * 100,
          unit: "%",
        });
      }
    }
    return {
      mult: a.k === "装備攻撃力強化倍率" && atk ? fmtSignedPct(atk.enhance_rate, { max: 4 }) : "—",
      delta: a.v,
      to: a.to,
      mats,
      idle: 0,
      expr: a.k === "装備攻撃力" ? (stepOf("装備攻撃力")?.expression ?? null) : null,
    };
  }
  /** 鎖「1 発」: 抜けた分から 1 発までの各段(倍率・実数・到達値) */
  const perHitDetail = $derived.by<Detail | null>(() => {
    const r = result;
    if (r === null || perHit === null || pierced === null) return null;
    const mats: Mat[] = flowRows.map((f) => ({
      label: f.k,
      mult: f.mult === "—" ? undefined : f.mult,
      value: fmtSigned(f.add),
      sub: `ここまで ${fmtInt(Math.round(f.to))}`,
      n: Math.round(f.add),
    }));
    if (r.capped_loss.max > 0) {
      mats.push({
        label: "ダメージ上限(1 段ごと)",
        value: fmtInt(r.damage_cap),
        n: r.damage_cap,
        sub: `上限で ${fmtSigned(-r.capped_loss.max, { max: 3 })}`,
      });
    }
    return {
      mult: flowMultLabel,
      delta: perHit - pierced,
      to: perHit,
      mats,
      idle: 0,
      expr: "ゲームの表記ダメージ(スキル分のみ)。武器強化の追加固定ダメージは含まない(合計の内訳を見る)",
    };
  });
  /** 鎖「合計」: (1 発 × 段数) ＋ (武器強化の追加固定 × 段数) ＋ 割合追加ダメージ。クリ率は段階表示で読む */
  const totalDetail = $derived.by<Detail | null>(() => {
    const r = result;
    if (r === null || perHit === null || totalValue === null) return null;
    const added = pick(r.added_damage) ?? 0;
    const skillTotal = pick(r.skill_total) ?? 0;
    const mats: Mat[] = [
      {
        label: `1 発(表記ダメージ) ${fmtInt(perHit)} × ${r.hit_count} 段`,
        mult: `×${r.hit_count}`,
        value: fmtInt(skillTotal),
        n: skillTotal,
      },
    ];
    if (r.weapon_added_per_hit !== 0) {
      // 段ごとの分割は内部の丸めの都合で、使う側は「1 スキルに 1 回乗る固定値」と捉えている。
      // 段数や 1 段あたりの値は見せず、スキルに乗る総額だけ出す
      mats.push({
        label: "武器強化(追加固定)",
        mult: "+",
        value: fmtInt(r.weapon_added_total),
        n: r.weapon_added_total,
        sub: "上限なし・表記ダメージとは別枠",
      });
    }
    if (added !== 0) {
      mats.push({
        label: "割合追加ダメージ(合計に乗る)",
        mult: fmtSignedPct(r.added_damage_rate, { max: 4 }),
        value: fmtInt(added),
        n: added,
        sub: "シャープネスビジョン・ランダムOP・称号",
      });
    }
    if (!critMode) {
      mats.push({
        label: "クリティカルなら",
        mult: skill ? `×${fmtNum(skill.critical_multiplier)}` : undefined,
        value: fmtInt(r.total.critical),
        n: r.total.critical,
      });
    }
    mats.push({ label: "乱数が最小のとき", value: fmtInt(r.total.min), n: r.total.min });
    return {
      mult: `×${r.hit_count} 段`,
      delta: totalValue - perHit,
      to: totalValue,
      mats,
      idle: 0,
      expr: stepOf("割合追加ダメージ(合計に乗る)")?.expression ?? null,
    };
  });
  /** 鎖「1 秒あたり」: 合計 × 回/分 ÷ 60 と、実ディレイの内訳。期待値はクリ率で按分した材料も足す */
  const dpsDetail = $derived.by<Detail | null>(() => {
    const r = result;
    const d = r?.actual_delay ?? null;
    if (r === null || d === null || r.dps === null || dpsValue === null || totalValue === null) return null;
    const mats: Mat[] = [
      { label: "合計ダメージ", value: fmtInt(totalValue), n: totalValue },
      {
        label: "基本中ディレイ",
        value: fmtNum(d.base, 2, "s"),
        n: d.base, unit: "s",
        sub: d.fixed ? "固定(減少が効かない)" : undefined,
      },
    ];
    for (const c of d.contributions) {
      mats.push({ label: `↳ ${c.source}`, value: fmtSignedPct(-c.rate), n: -Math.round(c.rate * 100), unit: "%" });
    }
    mats.push({
      label: `中ディレイ減少(上限 ${fmtPct(limits.actual_delay_reduction_max)})`,
      value: fmtPct(d.reduction),
      n: Math.round(d.reduction * 100), unit: "%",
      sub: d.reduction_raw > d.reduction ? `選択中は ${fmtPct(d.reduction_raw)}` : undefined,
    });
    if (d.combo_rate < 1) {
      mats.push({ label: "コンボ(倍率A。間に通常攻撃を挟む)", mult: `×${fmtNum(d.combo_rate)}`, value: "" });
    }
    mats.push({
      label: "中ディレイ",
      value: fmtNum(d.value, 2, "s"),
      n: d.value, unit: "s",
      sub: d.floored ? `下限 ${fmtNum(limits.actual_delay_min, 1, "s")} で頭打ち` : undefined,
    });
    const cycle = r.combo;
    if (cycle) {
      // コンボは 1 サイクル(通常攻撃 → スキル)で割る。実測表はコンボなしの計測なので使わない
      mats.push({
        label: `通常攻撃(${cycle.normal_attack_name})`,
        // 合計ダメージ(total_primary)と同じ側を出す。ここだけ非クリだと足し算が合わなく見える
        value: fmtInt(pick(cycle.normal_attack_total) ?? 0),
        n: pick(cycle.normal_attack_total) ?? 0,
        sub: `中ディレイ ${fmtNum(cycle.normal_delay, 2, "s")}`,
      });
      mats.push({
        label: "コンボインターバル",
        value: cycle.interval !== null ? fmtNum(cycle.interval, 2, "s") : "?",
        n: cycle.interval ?? undefined, unit: "s",
        sub: cycle.interval === null
          ? "wiki 未収録。スキルの中ディレイをそのまま使っています"
          : cycle.interval_binding
            ? "スキルの中ディレイより長いので、こちらが下限になります"
            : "スキルの中ディレイのほうが長いので効きません",
      });
      mats.push({
        label: "1 サイクル",
        value: fmtNum(cycle.seconds, 2, "s"),
        n: cycle.seconds, unit: "s",
        sub: `通常攻撃 ${fmtNum(cycle.normal_delay, 2, "s")} + ${fmtNum(cycle.skill_gap, 2, "s")}`,
      });
    } else {
      mats.push({
        label: "スキル回数",
        value: `${Math.round(d.uses_per_minute)} 回/分`,
        n: Math.round(d.uses_per_minute),
        sub: d.uses_measured ? "実測表から" : "式 60 ÷ 中ディレイ",
      });
    }
    if (r.expected_dps !== null && r.critical_chance > 0 && r.critical_chance < 1) {
      mats.push({
        label: "期待値(クリ率で按分)",
        value: fmtInt(Math.round(r.expected_dps)),
        n: Math.round(r.expected_dps),
        sub: `合計(非クリ) × ${fmtPct(1 - r.critical_chance, 1)} + 合計(クリ) × ${fmtPct(r.critical_chance, 1)}`,
      });
    }
    return {
      mult: `÷ ${fmtNum(cycle?.seconds ?? d.value, 2, "s")}`,
      delta: null,
      to: Math.round(dpsValue),
      mats,
      idle: 0,
      expr: cycle
        ? "1 秒あたり = (スキルの合計 + 通常攻撃の合計) ÷ 1 サイクル"
        : "1 秒あたり = 合計 × スキル回数(回/分) ÷ 60",
    };
  });

  // --- 効いていない分の棚卸し(design-system §14 決定 2)---------------------
  // 上限で捨てた分は 能力値上限 / カテゴリ上限 / ダメージ上限 / 防御力上限 / 中ディレイ
  // の 5 階層に散っている。5 箇所を回らないと総ロスが分からない状態は
  // 「効いていない量を見せるのがこの道具の価値」(§00)を薄めるので、1 箇所に集める。
  // 斜線の記号は各所で使ったまま、棚卸しだけをここに寄せる。新しい画面は作らない。
  interface LostRow {
    /** どの上限か */
    k: string;
    /** 上限にぶつかる前の値 */
    raw: string;
    /** 実際に効いている値 */
    val: string;
    /** 捨てている量 */
    loss: string;
    /** 効いている割合(0〜1)。塗り = 効いている量、斜線 = 捨てた量 */
    kept: number;
  }
  const lostRows = $derived.by<LostRow[]>(() => {
    const out: LostRow[] = [];
    // 能力値上限(覚醒段階 + エタの意志 Lv)
    for (const s of result?.trace.stats ?? []) {
      if (s.capped_loss > 1e-9) {
        const before = s.effective + s.capped_loss;
        out.push({
          k: `能力値上限 ${STAT_LABELS[s.kind]}`,
          raw: fmtInt(before),
          val: fmtInt(s.stat_cap),
          loss: fmtInt(s.capped_loss),
          kept: before > 0 ? s.effective / before : 1,
        });
      }
    }
    // カテゴリ上限。合算してから切るので、積んだのに効いていない量が数値で見えないと詰み手前が分からない
    for (const c of cappedCategories) {
      out.push({
        k: `カテゴリ上限 ${c.label}`,
        raw: fmtCatRaw(c),
        val: fmtCatValue(c),
        loss: fmtCatLoss(c),
        kept: Math.abs(c.raw) > 1e-9 ? Math.abs(c.value) / Math.abs(c.raw) : 1,
      });
    }
    // ダメージ上限(wiki: Quest/覚醒クエスト。多段スキルでも 1 段ごとに適用)
    if (result !== null && perHit !== null) {
      const loss = pick(result.capped_loss) ?? 0;
      if (loss > 0) {
        const before = perHit + loss;
        out.push({
          k: "ダメージ上限(1 段ごと)",
          raw: fmtInt(before),
          val: fmtInt(result.damage_cap),
          loss: fmtInt(loss),
          kept: before > 0 ? perHit / before : 1,
        });
      }
    }
    // 防御力上限。防御タブと同じ値だが、棚卸しのために回らせない
    if (defense !== null) {
      const d = defense;
      const rows: [string, number, number][] = [
        ["物理", d.physical_defense, d.physical_defense_loss],
        ["魔法", d.magic_defense, d.magic_defense_loss],
        ["複合", d.composite_defense, d.composite_defense_loss],
      ];
      for (const [name, value, loss] of rows) {
        if (loss > 1e-9) {
          const before = value + loss;
          out.push({
            k: `防御力上限 ${name}`,
            raw: fmtInt(before),
            val: fmtInt(d.defense_cap),
            loss: fmtInt(loss),
            kept: before > 0 ? value / before : 1,
          });
        }
      }
    }
    // 中ディレイ。減少値の上限と秒そのものの下限は別の捨て方なので分けて出す
    const ad = result?.actual_delay ?? null;
    if (ad !== null) {
      if (ad.reduction_raw > ad.reduction + 1e-9) {
        out.push({
          k: `中ディレイ減少の上限(${fmtPct(limits.actual_delay_reduction_max)})`,
          raw: fmtPct(ad.reduction_raw),
          val: fmtPct(ad.reduction),
          loss: fmtPct(ad.reduction_raw - ad.reduction),
          kept: ad.reduction_raw > 0 ? ad.reduction / ad.reduction_raw : 1,
        });
      }
      if (ad.floored) {
        const want = ad.raw;
        out.push({
          k: `中ディレイの下限(${fmtNum(limits.actual_delay_min, 1, "s")})`,
          raw: fmtNum(want, 2, "s"),
          val: fmtNum(ad.value, 2, "s"),
          loss: `${fmtNum(ad.value - want, 2, "s")} ぶん遅い`,
          kept: ad.value > 0 ? want / ad.value : 1,
        });
      }
    }
    return out;
  });

  // --- 攻撃 / 防御タブ(規格シート 5c) --------------------------------------
  let side = $state<"attack" | "defense">("attack");
  let defense = $state<DefenseProfile | null>(null);
  let defenseError = $state<string | null>(null);
  const defenseLatest = latest();
  $effect(() => {
    // 防御側は対象コンテンツに依らない。キャラ(試し変更込み)が変わったときだけ引き直す
    const p = payload;
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!p) {
      defense = null;
      return;
    }
    defenseLatest.run((isCurrent) =>
      previewDefense(p, JSON.parse(buffsJson))
        .then((d) => {
          if (isCurrent()) {
            defense = d;
            defenseError = null;
          }
        })
        .catch((e) => {
          if (isCurrent()) defenseError = errorMessage(e);
        }),
    );
  });

  // --- 試し変更(sim) ------------------------------------------------------
  /**
   * 同時に試せる変更の上限(design-system §14 決定 6)。
   * 5〜6 個同時に動くとチップ列が読めなくなる。「試しセットに名前を付けて保存」に逃げると
   * ラベンダー = 保存されない の意味が壊れるので、機能側に制約を置く。
   * 上限に達したことは**色ではなく文言**で伝える(ラベンダーに 2 つ目の意味を持たせない)。
   */
  const SIM_LIMIT = 3;
  /** 上限で弾いた直後だけ立てる。次の操作が通ったら下ろす */
  let simLimited = $state(false);
  function editSim(fn: (p: NewCharacter) => void) {
    if (!payload) return;
    const p = JSON.parse(JSON.stringify(payload)) as NewCharacter;
    fn(p);
    if (savedPayload !== null && KNOBS.filter((k) => k.get(p) !== k.get(savedPayload)).length > SIM_LIMIT) {
      simLimited = true;
      return;
    }
    simLimited = false;
    app.sim = p;
  }
  // 保存値との差分があるかどうかの判定は state.svelte.ts の simIsDirty に一本化
  // (JSON.stringify の比較をここでもう一度書かない。上部バーと同じ関数を読む)。
  const simDirty = $derived(simIsDirty());
  function resetSim() {
    app.sim = null;
    simLimited = false;
    // 伸びしろの土台(登録値)が変わるので、エンチャント一覧の「見えたことがある」記録も
    // 撮り直す(enchantShownKeys)。
    enchantShownKeys = new Set();
  }
  let saving = $state(false);
  async function saveSim() {
    if (!character || !app.sim) return;
    saving = true;
    try {
      // 試し変更の保存もキャラ単位の保存キューへ通す(ホームの直更新・キャラタブの保存と
      // 同じ full-overwrite なので、直列化しないと互いの変更を巻き戻す)。payload はユーザーが
      // 明示した試し変更のスナップショットなので、ここで確定させてからキューに載せる。
      const payload = JSON.parse(JSON.stringify(app.sim)) as NewCharacter;
      const saved = await enqueueCharacterSave(character.id, () => payload);
      upsertCharacter(saved);
      app.sim = null;
    } catch (e) {
      reportError(errorMessage(e));
    } finally {
      saving = false;
    }
  }

  // 差分チップ(1変更 = 1チップ、✕ でその変更だけ戻す)
  interface Knob {
    id: string;
    label: (p: NewCharacter) => string;
    get: (p: NewCharacter) => string;
    set: (p: NewCharacter, v: string) => void;
  }
  const KNOBS: Knob[] = [
    {
      id: "pw",
      label: (p) => `パワーW ${p.common_skills.power_weapon ? "ON" : "OFF"}`,
      get: (p) => String(p.common_skills.power_weapon),
      set: (p, v) => (p.common_skills.power_weapon = v === "true"),
    },
    {
      id: "sw",
      label: (p) => `ストロングW ${p.common_skills.strong_weapon_level > 0 ? `Lv${p.common_skills.strong_weapon_level}` : "なし"}`,
      get: (p) => String(p.common_skills.strong_weapon_level),
      set: (p, v) => (p.common_skills.strong_weapon_level = Number(v)),
    },
    {
      // 部位・ステをまたぐので 1 チップに束ねる(全部位ぶんをまとめて 1 操作として戻す)
      // 登録 ID で持つ(装着中の 1 件ではなく)。装着を切り替えても、切り替え先のエンチャントが
      // 「変えた」ことにならないようにする — 切り替えは equipment_select の 1 チップで戻す
      id: "enchant",
      label: () => "エンチャント",
      get: (p) => JSON.stringify(PART_SLOTS.map((s) => p.equipment.parts[s].registered.map((x) => [x.id, x.enchant]))),
      set: (p, v) => {
        const values = JSON.parse(v) as [number, EquipmentValues][][];
        PART_SLOTS.forEach((s, i) => {
          for (const [id, enchant] of values[i]) {
            const part = p.equipment.parts[s].registered.find((x) => x.id === id);
            if (part) part.enchant = enchant;
          }
        });
      },
    },
    {
      // 登録済み装備の装着切り替え(部位ごとの selected_id)。全部位まとめて 1 チップで戻す
      id: "equipment_select",
      label: (p) => {
        const changed = savedPayload
          ? PART_SLOTS.filter((s) => p.equipment.parts[s].selected_id !== savedPayload.equipment.parts[s].selected_id)
          : [];
        return `装備切替 ${changed.map((s) => PART_SLOT_LABELS[s]).join("・")}`.trim();
      },
      get: (p) => JSON.stringify(PART_SLOTS.map((s) => p.equipment.parts[s].selected_id)),
      set: (p, v) => {
        const ids = JSON.parse(v) as (number | null)[];
        PART_SLOTS.forEach((s, i) => (p.equipment.parts[s].selected_id = ids[i]));
      },
    },
    {
      id: "title",
      label: (p) => `称号 ${app.titles.find((t) => t.id === p.equipment.title)?.name ?? "なし"}`,
      get: (p) => String(p.equipment.title),
      set: (p, v) => (p.equipment.title = v === "null" ? null : v),
    },
    {
      id: "ultimate",
      label: (p) =>
        `極限 ${
          p.common_skills.ultimate.slots
            .filter((s): s is UltimateSkill => s !== null)
            .map((s) => ULTIMATE_SKILL_LABELS[s])
            .join("・") || "未選択"
        }`,
      get: (p) => JSON.stringify(p.common_skills.ultimate.slots),
      set: (p, v) => (p.common_skills.ultimate.slots = JSON.parse(v)),
    },
    {
      // 覚醒段階とエタの意志 Lv は 1 つの育ち方(エタは覚醒 5 の先)なので 1 チップで戻す
      id: "awakening",
      label: (p) => `覚醒 ${p.awakening.stage} / エタ Lv${p.awakening.eternal_level}`,
      get: (p) => JSON.stringify(p.awakening),
      set: (p, v) => (p.awakening = JSON.parse(v)),
    },
    {
      id: "sharpness",
      label: (p) =>
        p.common_skills.sharpness_vision_level > 0
          ? `シャープネス Lv${p.common_skills.sharpness_vision_level}`
          : "シャープネス 未習得",
      get: (p) => String(p.common_skills.sharpness_vision_level),
      set: (p, v) => (p.common_skills.sharpness_vision_level = Number(v)),
    },
    {
      // リンクステータスは 8 種まとめて 1 チップ。この画面で触るのはダメージ式に効く 5〜7 だけ
      id: "soul_link",
      label: () => "ソウルリンク",
      get: (p) => JSON.stringify(p.stat_sources.soul_link),
      set: (p, v) => (p.stat_sources.soul_link = JSON.parse(v)),
    },
    {
      // 「次に変えるなら」の武器更新。基本値まで一緒に替わる 1 操作なので 1 チップで戻す。
      id: "weapon_item",
      label: (p) => {
        const weapon = weaponOf(p);
        const name = app.equipmentCatalog.find((i) => i.id === weapon.item_id)?.name
          ?? weapon.custom_name
          ?? "未装着";
        return `武器 ${name}`;
      },
      // 登録 ID ごとに持つ(装着の切り替えで「武器が変わった」ことにしない。enchant と同じ)
      get: (p) => JSON.stringify(p.equipment.parts.weapon.registered.map((w) => [w.id, w.item_id, w.custom_name, w.base])),
      set: (p, v) => {
        for (const [id, itemId, customName, base] of JSON.parse(v) as [number, string | null, string | null, EquipmentValues][]) {
          const weapon = p.equipment.parts.weapon.registered.find((w) => w.id === id);
          if (!weapon) continue;
          weapon.item_id = itemId;
          weapon.custom_name = customName;
          weapon.base = base;
        }
      },
    },
    // 以下は計算タブの編集 UI からは変わらないが、sim が他の経路で差分を持ったときに
    // 「試し変更中なのにチップが空」にならないよう網羅する(独立レビュー指摘)。
    {
      id: "base_stats",
      label: () => "素ステータス",
      get: (p) => JSON.stringify(p.base_stats),
      set: (p, v) => (p.base_stats = JSON.parse(v)),
    },
    {
      id: "permanent",
      label: () => "恒常補正(ペット/ルーン/クラウン/聖物)",
      get: (p) =>
        JSON.stringify([
          p.stat_sources.pet_skills,
          p.stat_sources.rune_levels,
          p.stat_sources.crown,
          p.stat_sources.sacred_relic,
        ]),
      set: (p, v) => {
        const [pet, rune, crown, relic] = JSON.parse(v);
        p.stat_sources.pet_skills = pet;
        p.stat_sources.rune_levels = rune;
        p.stat_sources.crown = crown;
        p.stat_sources.sacred_relic = relic;
      },
    },
    {
      id: "identity",
      label: (p) => `名前・キャラ種(${p.name})`,
      get: (p) => JSON.stringify([p.name, p.game_character_id]),
      set: (p, v) => {
        const [name, gid] = JSON.parse(v);
        p.name = name;
        p.game_character_id = gid;
      },
    },
  ];
  const changedKnobs = $derived(
    app.sim !== null && savedPayload !== null
      ? KNOBS.filter((k) => k.get(app.sim!) !== k.get(savedPayload))
      : [],
  );
  function revertKnob(k: Knob) {
    if (!savedPayload || !app.sim) return;
    const p = JSON.parse(JSON.stringify(app.sim)) as NewCharacter;
    k.set(p, k.get(savedPayload));
    app.sim = JSON.stringify(p) === JSON.stringify(savedPayload) ? null : p;
    simLimited = false;
  }

  // --- もし〜だったら ------------------------------------------------------
  // 列挙・試算・並び順(+0 除外)は list_upgrade_candidates(Rust 側 domain::candidate)。
  // コンボ・一時調整は「この一発」表示と同条件(現在のコンボ・一時調整)で試算する。
  let whatIf = $state<UpgradeCandidate[]>([]);
  /** 押した候補は、移動先の差分チップと同時に短く退出させる(§10「移った」)。 */
  let leavingWhatIfId = $state<string | null>(null);
  /** 「足りない分をどう埋める?」1 行の 2 位以降。押した行の直下に開く(§00 03) */
  let fillMoreOpen = $state(false);
  const whatIfLatest = latest({ debounce: 250 });
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null;
    const t = target;
    const sid = skillId;
    const base = perHit;
    const comboCount = combo ? limits.combo_bonus_threshold : 0;
    const comboType = selectedComboSkillType;
    const tempJson = JSON.stringify(NEUTRAL_ADJUSTMENTS);
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson || !t || !sid || base === null) {
      whatIfLatest.cancel();
      whatIf = [];
      return;
    }
    whatIfLatest.run(async (isCurrent) => {
      try {
        const current = JSON.parse(pJson) as NewCharacter;
        const rs = await listUpgradeCandidates(
          current, sid, t.content.id, comboCount, comboType, JSON.parse(tempJson), JSON.parse(buffsJson),
        );
        if (isCurrent()) {
          whatIf = rs;
          leavingWhatIfId = null;
        }
      } catch (e) {
        if (isCurrent()) reportError(errorMessage(e));
      }
    });
    return () => whatIfLatest.cancel();
  });
  /** 伸び率の表示。**表記ダメージと合計ダメージの 2 本**を並べる — シャープネスビジョンや
   *  武器強化のように「表記は動かないのに合計は伸びる」ものがあり、片方だけだと
   *  「効いていない」と読めてしまう(ユーザー判断 2026-09-01)。 */
  const deltaText = (pct: number) => (pct === 0 ? "±0%" : fmtSigned(pct, { max: 2 }, "%"));
  function applyWhatIf(w: UpgradeCandidate) {
    leavingWhatIfId = w.id;
    // editSim と同じ SIM_LIMIT ガード(w.applied は列挙時点の payload + 候補 1 件ぶんの変更)
    if (savedPayload !== null && KNOBS.filter((k) => k.get(w.applied) !== k.get(savedPayload)).length > SIM_LIMIT) {
      simLimited = true;
      return;
    }
    simLimited = false;
    app.sim = w.applied;
  }

  // --- 右カラム: バフ・装備の編集(試し変更として) -------------------------
  // バフカタログは常用バフ専用(キャラスキルは補正源のキャラスキル欄)
  const consumableBuffs = $derived(app.catalog);
  const buffOn = (def: BuffDefinition) =>
    app.calcBuffs.choices.some((c) => c.buff_id === def.id);
  /**
   * バフの 3 状態(v4)。保存済みかどうかで「常時(マイセット)」と「追加枠」を分ける。
   * - `always`: キャラに保存済み = 毎回のっている常用セット
   * - `extra`: この計算だけの追加(試し変更。保存されない)
   * - `off`: 使わない(保存済みバフを一時的に外した場合も含む)
   * 常時への昇格は保存操作(「試し変更を保存」)で行う。チップのクリックで DB を書かない。
   */
  const buffState = (def: BuffDefinition): "always" | "extra" | "off" => {
    const saved = (app.buffSets.find((set) => set.id === app.calcBuffSetId)?.choices.choices ?? [])
      .some((c) => c.buff_id === def.id);
    if (!buffOn(def)) return "off";
    return saved ? "always" : "extra";
  };
  const alwaysBuffCount = $derived(consumableBuffs.filter((d) => buffState(d) === "always").length);
  const extraBuffCount = $derived(consumableBuffs.filter((d) => buffState(d) === "extra").length);
  function chooseCalcBuffSet(value: string) {
    const id = value === "" ? null : Number(value);
    app.calcBuffSetId = id;
    const set = app.buffSets.find((item) => item.id === id);
    app.calcBuffs = JSON.parse(JSON.stringify(set?.choices ?? { choices: [] }));
  }
  /* 「計算の材料」と、その中のバフの目的グループは、どちらも同時に 1 つしか開かない。
     排他は <details name>(ui/Disclosure の group)がブラウザ側で持つので、開いている
     まとまりを覚える状態は要らない — 全部開くと 3512px(表示域の 4.6 画面ぶん)になり、
     目的のものまでスクロールで探すことになる */
  function toggleBuffChip(def: BuffDefinition) {
    app.calcBuffs = { choices: toggleBuff(app.calcBuffs.choices, def, !buffOn(def)) };
  }
  // ON のバフのうち、対象ステ・効果量の選択肢・手入力を持つものの詳細編集(試し変更として反映)
  const statOptions = STAT_KINDS.map((k) => ({ value: k, label: STAT_LABELS[k] }));
  const buffChoiceOf = (buffId: string) =>
    app.calcBuffs.choices.find((c) => c.buff_id === buffId) ?? null;
  const buffChoiceOfStat = (buffId: string, stat: StatKind) =>
    app.calcBuffs.choices.find((c) => c.buff_id === buffId && c.stat === stat) ?? null;
  /** この画面で**触れる**ものがあるか。固定値のバフは読むだけなので含めない —
   *  値はチップ本体の寄与表示で既に見えていて、開いても「値: +30%」と出るだけだった */
  const hasDetail = (def: BuffDefinition) =>
    isUserSelectedTarget(def.target) || isChoiceValue(def.value) || userInputRange(def.value) !== null;
  function editBuffChoice(buffId: string, fn: (c: BuffChoice) => void, stat?: StatKind) {
    const choices = app.calcBuffs.choices.map((choice) => ({ ...choice }));
    const choice = choices.find(
      (item) => item.buff_id === buffId && (stat === undefined || item.stat === stat),
    );
    if (choice) fn(choice);
    app.calcBuffs = { choices };
  }
  /** 複数ステ対象バフ(クラブ効果)の、1 ステぶんの ON/OFF。試し変更なので保存はしない */
  function toggleBuffStatChip(def: BuffDefinition, stat: StatKind, next: boolean) {
    app.calcBuffs = { choices: toggleBuffStat(app.calcBuffs.choices, def, stat, next) };
  }

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
    editSim((p) => (p.equipment.parts[slot].selected_id = id));
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
    editSim((p) => (p.equipment.title = id));
  }

  // --- 研磨(試し変更)。記録はキャラタブの研磨ペイン、効かせるかはバフ「装備研磨」と同じ
  //     スイッチ(calcBuffs の equipment_polish)。装備に付けたものなので、探す先はバフ一覧では
  //     なく材料列(シャープネス・エンチャントと同じ段)。ここは表示の顔で、正は Rust 側
  //     (`equipment_polish_active` / `Equipment::base_sources`)。加算量のプレビューは
  //     PolishPane と同じ polishAmount(表示のみ)で出す
  const polishDef = $derived(app.catalog.find((def) => def.id === "equipment_polish") ?? null);
  const polishOn = $derived(polishDef !== null && buffOn(polishDef));
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
    editSim((p) => {
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
    critical_damage_rate: number; actual_delay_reduction: number; added_hit_count: number; skill_range_bonus: number;
  } | null>(null);
  /** ソウルリンクの効いている量(preview_effective_stats の soul_link)。同じ応答から取る */
  let soulLinkPreview = $state<SoulLinkPreview | null>(null);
  const ultimateLatest = latest({ debounce: 150 });
  /** 排他枠の衝突で選べないバフ。判定は Rust(`blocked_buffs`)、ここは応答を持つだけ */
  let calcBlockedBuffs = $state<BlockedBuff[]>([]);
  const blockedLatest = latest({ debounce: 0 });
  $effect(() => {
    const buffs = JSON.parse(JSON.stringify(app.calcBuffs)) as BuffSelection;
    blockedLatest.run(async (isCurrent) => {
      const blocked = await listBlockedBuffs(buffs);
      if (isCurrent()) calcBlockedBuffs = blocked;
    });
    return () => blockedLatest.cancel();
  });
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
    if (skillId === "full_throttle") {
      return `中ディレイ ${fmtSignedPct(-e.actual_delay_reduction)} ・段数 ${fmtSigned(e.added_hit_count)}`;
    }
    return `範囲 ${fmtSigned(e.skill_range_bonus)}(火力には効きません)`;
  }

  // --- 地力(試し変更)。装備ではなく育てて上がるもののうち、効きが大きい 3 つ -----------
  // 覚醒・エタの意志(N と各種上限)/ シャープネスビジョン(§5 新-割合)/ ソウルリンク 5〜7。
  // どれもキャラタブでしか触れず、「盛ったらどこまで届くか」を計算タブで試せなかった。
  // 入力形はキャラタブと同じ(節目の段 + 数値)にして、押した瞬間に結果が動くようにする。

  /** エタの意志は覚醒 5 の先にあるもの。Lv を入れた時点で覚醒は 5 で確定する(キャラタブと同じ) */
  function setSimEternalLevel(level: number) {
    editSim((p) => {
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
    const t = target;
    const sid = skillId;
    const rowCount = enchantRowsList.length;
    const comboCount = combo ? limits.combo_bonus_threshold : 0;
    const comboType = selectedComboSkillType;
    const tempJson = JSON.stringify(NEUTRAL_ADJUSTMENTS);
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson || !t || !sid || rowCount === 0) {
      enchantLatest.cancel();
      enchantGains = {};
      return;
    }
    enchantLatest.run((isCurrent) =>
      listEnchantGains(
        JSON.parse(pJson) as NewCharacter, sid, t.content.id, comboCount,
        comboType, JSON.parse(tempJson), JSON.parse(buffsJson),
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
    void character?.id;
    void target?.content.id;
    void skillId;
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

  // --- バフ 1 件ごとの寄与(ON にしたチップに併記)。実計算(previewDamage)のトレースが
  //     供給源名で内訳を持っている(catContributions と同じデータ)ので、そこから該当バフの
  //     行だけ拾う。写経で数字を作らない。ステ側は stat_contributions ではなく
  //     stat_source_effects を使う(cap 込みで倍率A/B の増幅も一貫して割り振られる帰属)。
  //     全ステに乗るバフは 7 件そのまま並べるとチップ幅から欠けるので、ステ側は効きの大きい
  //     上位 2 件 + ほか n に絞る(バフタブの statDeltaText と同じ topRowsText を使い、
  //     二重管理にしない)。 ---
  function buffContributionText(def: BuffDefinition): string {
    const statRows = (result?.trace.stat_source_effects ?? [])
      .filter((c) => c.source === def.name && c.effect !== 0)
      .map((c) => ({ label: `${STAT_LABELS[c.kind]} ${fmtSigned(c.effect, { max: 3 })}`, value: c.effect }));
    const parts: string[] = [];
    if (statRows.length > 0) parts.push(topRowsText(statRows));
    for (const c of result?.trace.category_contributions ?? []) {
      if (c.source === def.name && c.value !== 0) {
        const cat = result?.trace.categories.find((x) => x.category === c.category);
        const label = cat ? `${cat.symbol}` : c.category;
        parts.push(`${label} ${fmtSignedPct(c.value, { max: 4 })}`);
      }
    }
    return parts.join(" ・ ");
  }

</script>

<!-- 押した数値の内訳。押した行の直下にだけ開く(§00 03)。
     列は band-row と同じ段にそろえ、面はインセット = 読み取り専用(§02)。 -->
<!-- 閉じていても DOM に置いたまま隠す(hidden)。{#if} で外すと、閉じている間に称号などを切り替えた
     ↑↓・追加/削除 が、開いたときには消えている(差分は要素が前回値を覚えている。§00 04) -->
{#snippet detailBox(d: Detail, open: boolean)}
  <div class="detail inset open-in" hidden={!open}>
    {@render detailBody(d)}
  </div>
{/snippet}

<!-- 内訳の中身だけ。トリガと面が並ぶ場所では <Disclosure> の中に直接置く -->
{#snippet detailBody(d: Detail)}
  <div class="detail-body">
    <div class="dt-head">
      <span class="dt-hk dim">倍率</span>
      <span class="num dt-hv">{d.mult}</span>
      <span class="dt-hk dim">実数</span>
      <Value
        class={`dt-hv ${(d.delta ?? 0) < 0 ? "bad" : ""}`}
        motion={() => (d.delta === null ? null : Math.round(d.delta))}
        value={d.delta === null ? "—" : fmtSigned(d.delta)}
        delta={{}}
      />
      <span class="dt-hk dim">結果</span>
      <Value class="dt-hv big" motion={() => d.to} value={d.to === null ? "—" : fmtInt(Math.round(d.to))} delta={{}} />
    </div>
    {#each d.mats as m, i (i)}
      {#if m.key}
        {@const key = m.key}
        <!-- 押すと要因の一覧が直下に開く(押した行は動かない)。段は内訳の grid のままなので
             <details> は段に溶かす(display: contents) -->
        <Disclosure
          class="dt-fold" summaryClass="dt-row dt-row-btn"
          bind:open={() => isDetailOpen(key), (v) => setDetailOpen(key, v)}
        >
          {#snippet summary()}
          <span class="dt-label">{m.label}{#if m.note}<span class="dt-swap dim" use:changed={() => m.note ?? ""}>{m.note}</span>{/if}</span>
          <span class="num dt-mult dim">{m.mult ?? ""}</span>
          <Value
            class="dt-val"
            motion={() => m.n ?? null}
            value={m.value}
            delta={{ unit: m.unit }}
            deltaClass={m.changed ? "follow" : ""}
            onDelta={(e) => { e.stopPropagation(); followChange(key); }}
          />
          <span class="num dt-sub dim">{m.sub ?? ""}</span>
          {/snippet}
          <div class="dt-subs">
            <!-- 出典名でキーにする。入れ替わった出典は「抜けた行(取り消し線)+ 入った行」で残る -->
            {#each m.subs ?? [] as sm, j (sm.id ?? j)}
              <div class="dt-row" class:gone={sm.state === "gone"}>
                <span class="dt-label">{sm.label}</span>
                <span class="num dt-mult dim">{sm.mult ?? ""}</span>
                <Value
                  class={`dt-val ${(sm.n ?? 0) < 0 ? "bad" : ""}`}
                  motion={() => sm.n ?? null}
                  value={sm.value}
                  delta={sm.state === "gone" || sm.state === "added" ? null : { unit: sm.unit }}
                />
                {#if sm.state === "gone"}<span class="delta num down delta-in">削除</span>{:else if sm.state === "added"}<span class="delta num up delta-in">追加</span>{/if}
                <span class="num dt-sub dim">{sm.sub ?? ""}</span>
              </div>
            {/each}
          </div>
        </Disclosure>
      {:else}
        <div class="dt-row">
          <span class="dt-label">{m.label}</span>
          <span class="num dt-mult dim">{m.mult ?? ""}</span>
          <Value class="dt-val" motion={() => m.n ?? null} value={m.value} delta={{ unit: m.unit }} />
          <span class="num dt-sub dim">{m.sub ?? ""}</span>
        </div>
      {/if}
    {/each}
    {#if d.idle > 0}
      <p class="dt-note dim">他 {d.idle} 枠は中立(±0)なので、この段では効いていません。</p>
    {/if}
    {#if d.expr}<p class="dt-expr dim">{d.expr}</p>{/if}
  </div>
{/snippet}

  {#snippet buffChip(def: BuffDefinition)}
                {@const state = buffState(def)}
                {@const blocked = state === "off" && calcBlockedBuffs.some((b) => b.buff_id === def.id)}
                {@const detail = state !== "off" && hasDetail(def)}
                <!-- ON にしたチップの実際の寄与(供給源ごとの実数。写経しない)。
                     値の調整(設定・ポップオーバー)は押せる面の外(extra)に置く —
                     押した名前の上に段を差し込まない(§00 03) -->
                <ToggleRow
                  name={def.name}
                  value={state !== "off" ? buffContributionText(def) : undefined}
                  on={state !== "off"}
                  tone={state === "extra" ? "temp" : "saved"}
                  disabled={blocked}
                  title={blocked ? "同枠の他バフと排他です" : def.note || undefined}
                  onToggle={() => { if (!blocked) toggleBuffChip(def); }}
                >
                  {#snippet icon()}
                    <!-- 未収録の id は破線 + ? になり、その場でも幅は変わらない -->
                    <Icon kind="buff" id={def.id} size={20} label={def.name} />
          {/snippet}
                  {#snippet extra()}
                    {#if detail}
                      {@const choice = buffChoiceOf(def.id)}
                      {#if choice}
                        <Popover
                          label={`${def.name} の設定`}
                          triggerLabel={`${def.name} の設定`}
                          triggerClass="chip-config"
                          panelClass="buff-editor"
                        >
                          {#snippet trigger()}設定{/snippet}
                          {#snippet children(close)}
                        {#if isMultiTarget(def.target)}
                        <!-- クラブエフェクトはステごとに 1 つずつ併用できる。ここでは対象ステの
                             出し入れだけを試せるようにし、値はバフタブ側の設定を引き継ぐ -->
                        <div class="field">
                          <span class="field-head">
                            <span class="field-label">対象ステ</span>
                            <Value
                              class="field-count"
                              motion={() => pickedStats(app.calcBuffs.choices, def).length}
                              value={`${pickedStats(app.calcBuffs.choices, def).length}/${STAT_KINDS.length}`}
                            />
                          </span>
                          <Choose
                            label="対象ステ"
                            options={statOptions}
                            max={STAT_KINDS.length}
                            values={pickedStats(app.calcBuffs.choices, def)}
                            onToggle={(v, next) => toggleBuffStatChip(def, v as StatKind, next)}
                          />
                        </div>
                      {:else if isUserSelectedTarget(def.target)}
                        <div class="field">
                          <span class="field-label">対象ステ</span>
                          <Choose
                            label="対象ステ"
                            options={statOptions}
                            bind:value={
                              () => choice.stat ?? STAT_KINDS[0],
                              (v) => editBuffChoice(def.id, (c) => (c.stat = v as StatKind))
                            }
                          />
                        </div>
                      {/if}
                      {#if isChoiceValue(def.value)}
                        {@const options = def.value.choice.map((v, i) => ({ value: String(i), label: formatLayerValue(def.layer, v) }))}
                        <!-- 値の候補は小さい順に並ぶ(順序あり)ので段(§07「1 つ選ぶ」) -->
                        <div class="field">
                          <span class="field-label">値</span>
                          <Choose
                            label="値"
                            {options}
                            full
                            bind:value={
                              () => String(choice.choice_index ?? 0),
                              (v) => editBuffChoice(def.id, (c) => (c.choice_index = Number(v)))
                            }
                          />
                        </div>
                      {/if}
                      {#if userInputRange(def.value)}
                        {@const range = userInputRange(def.value)!}
                        {@const scale = isPercentLayer(def.layer) ? 100 : 1}
                        {#if isMultiTarget(def.target)}
                          {#each pickedStats(app.calcBuffs.choices, def) as stat (stat)}
                            <div class="stat-value-row">
                              <span class="stat-value-label">{STAT_LABELS[stat]}</span>
                              <NumberField
                                label="{STAT_LABELS[stat]}の値"
                                min={range.min * scale}
                                max={range.max * scale}
                                bind:value={
                                  () => (buffChoiceOfStat(def.id, stat)?.value ?? def.default_value ?? range.min) * scale,
                                  (v) => editBuffChoice(def.id, (c) => (c.value = v / scale), stat)
                                }
                              />
                            </div>
                          {/each}
                        {:else}
                          <div class="stat-value-row">
                            <span class="stat-value-label">{isPercentLayer(def.layer) ? "値 (%)" : "値"}</span>
                            <NumberField
                              label={isPercentLayer(def.layer) ? "値 (%)" : "値"}
                              min={range.min * scale}
                              max={range.max * scale}
                              bind:value={
                                () => (choice.value ?? 0) * scale,
                                (v) => editBuffChoice(def.id, (c) => (c.value = v / scale))
                              }
                            />
                          </div>
                        {/if}
                      {/if}
                            <button type="button" class="popover-close" onclick={close}>閉じる</button>
                          {/snippet}
                        </Popover>
                      {/if}
                    {/if}
          {/snippet}
                </ToggleRow>
  {/snippet}

<SplitPage
  midTitle="行ける？"
  midNote="→ なぜこの数字？"
  rightTitle="計算の材料"
  rightNote={character?.name ?? ""}
  persistKey="tw-v4-calc"
  defaultRight={DEFAULT_RIGHT_WIDTH}
  minMid={320}
  minRight={280}
  splitterLabel="計算シートと材料の境界"
  midScrollStyle="scrollbar-gutter: stable;"
  rightScrollStyle="padding: 11px;"
>
  {#snippet mid()}
      {#if !character}
        <p class="empty dim">キャラを登録するとダメージ計算ができます。</p>
      {:else if !target}
        <p class="empty dim">コンテンツデータがありません。</p>
      {:else}
        <!-- 攻撃 / 防御(同列タブ) -->
        <Choose
          label="攻撃 / 防御"
          class="chiprow side-tabs"
          options={[{ value: "attack", label: "攻撃" }, { value: "defense", label: "防御" }]}
          bind:value={() => side, (v) => (side = v as "attack" | "defense")}
        />
      {/if}
      {#if !character || !target}
        <!-- 上のブロックで案内済み -->
      {:else if side === "defense"}
        <!-- 攻撃 / 防御 は面ごと入れ替わる。入ってくる面を短く動かす(§10 型 3b) -->
        <div class="swap-in"><DefensePanel profile={defense} error={defenseError} /></div>
      {:else}
        <!-- 行ける?カード -->
        <div class="swap-in">
        <SheetCard tone="gold" title="行ける？" note={character.name} busy={calculating}>
          <!-- 対象プレート -->
          <div class="target-row">
            <button type="button" class="step" onclick={() => stepTarget(-1)}>◀</button>
            <Popover label="計算する対象" triggerClass="target-trigger" panelClass="target-pop">
              {#snippet trigger(open)}
              <span class="t-line1">
                <Icon kind="content" id={target.content.id} fallback={{ kind: "mob", id: target.content.enemy_id }} size={28} label={target.content.name} />
                <span class="t-name">{target.content.name}</span>
                <span class="caret" class:rot={open}>▼</span>
                <span class="t-index num dim">{targetIndex + 1} / {contents.length}</span>
              </span>
              <span class="t-line2">
                <span class="t-area dim">{target.areaName}</span>
                <span class="t-def num">防御 {defenseValue !== null ? fmtInt(defenseValue) : "—"}</span>
                <span class="t-need num">目安 {fmtDuration(closeSeconds)}以内</span>
              </span>
              {/snippet}
              {#snippet children(close)}
              {#each targetAreas as area (area.id)}
                <div class="pop-head"><span class="pop-diamond"></span><span>{area.name}</span><span class="num dim">{area.contents.length} 件</span></div>
                {#each area.contents as c (c.id)}
                  {@const ev = evals.find((e) => e.content_id === c.id)}
                  <!-- 収録度は行頭に 1 つだけ(§14 決定 5)。分かっている行には出さない -->
                  {@const cov = !ev ? "判定中" : c.enemy_id === null ? "敵データなし" : !ev.damage ? "スキル未収録" : null}
                  <button
                    type="button"
                    class="pop-row"
                    class:on={c.id === target.content.id}
                    onclick={() => {
                      app.calcTargetId = c.id;
                      close();
                    }}
                  >
                    <span class="dot" style="background: {ev?.clear ? STATE.met.bd : ev?.entry_ok === false ? STATE.short.bd : STATE.unknown.bd};"></span>
                    <!-- コンテンツの絵が無ければそのコンテンツの敵の絵。サイズ固定なので行の高さは動かない -->
                    <Icon kind="content" id={c.id} fallback={{ kind: "mob", id: c.enemy_id }} size={20} label={c.name} />
                    {#if cov !== null}<span class="badge unknown">{cov}</span>{/if}
                    <span class="pop-name">{c.name}</span>
                    <span class="num dim">{ev?.damage ? fmtInt(ev.damage.per_hit_primary) : "—"}</span>
                  </button>
                {/each}
              {/each}
              {/snippet}
            </Popover>
            <button type="button" class="step" onclick={() => stepTarget(1)}>▶</button>
          </div>

          <!-- スキル行 -->
          <div class="skill-row">
            {#if skills.length === 0}
              <span class="dim">このキャラのスキルデータは未収録です(仮スキルはありません)。</span>
            {:else}
              <Popover
                label="計算するスキル"
                triggerClass="skill-trigger"
                panelClass="skill-pop"
                disabled={skills.length <= 1}
                onToggle={(open) => (skillOpen = open)}
              >
                {#snippet trigger(open)}
                <span class="sk-line1">
                  <Icon kind="skill" id={skill?.id ?? null} size={20} label={skill?.name ?? "スキル"} />
                  <span class="sk-name">{skill?.name ?? ""}</span>
                  {#if skills.length > 1}<span class="caret" class:rot={open}>▼</span>{/if}
                  <!-- 主軸(キャラタブ)と違うスキルで計算している例外状態。保存されないので
                       ラベンダー(--sim)。行の高さは変えない -->
                  {#if skillOverridden}
                    <span class="sk-override badge-in" use:changed={() => skillId}>ここで上書き中</span>
                  {/if}
                </span>
                <span class="sk-meta num dim">
                  ×<Value value={result ? fmtNum(result.effective_skill_multiplier) : "—"} />
                  ・ <Value value={result ? String(result.hit_count) : "—"} />段
                  ・ 中 <Value value={result?.effective_base_actual_delay != null ? `${fmtNum(result.effective_base_actual_delay)}s` : "?"} />
                  ・ Cri×{skill ? fmtNum(skill.critical_multiplier) : "—"}
                  {#if skill}・ {ELEMENT_LABELS[skill.element]}属性{/if}
                  {#if result?.accuracy_point != null}・ 命中P {fmtInt(result.accuracy_point)}{/if}
                </span>
                {/snippet}
                {#snippet children(close)}
                <div class="pop-head gold"><span>スキル {skills.length} 種 ／ この対象への合計ダメージ順</span></div>
                {#each pickerSkills as s (s.id)}
                  {@const d = skillTotals[s.id]}
                  <button
                    type="button"
                    class="pop-row"
                    class:on={s.id === skillId}
                    onclick={() => {
                      skillOverride = s.id;
                      close();
                    }}
                  >
                    <Icon kind="skill" id={s.id} size={20} label={s.name} />
                    <span class="pop-name">{s.name}</span>
                    <span class="num dim">×{fmtNum(s.multiplier)} / {s.hit_count}段</span>
                    <span class="num strong">{d ? fmtInt(d.total) : "…"}</span>
                  </button>
                {/each}
                {/snippet}
              </Popover>
              <!-- 1 クリックで主軸に戻す。ボタン in ボタンにできないので行の中の兄弟に置く -->
              {#if skillOverridden}
                <button
                  type="button" class="sk-reset badge-in"
                  title={mainSkill ? `主軸スキル「${mainSkill.name}」に戻す` : ""}
                  onclick={() => (skillOverride = null)}
                >主軸に戻す</button>
              {/if}
            {/if}
          </div>

          {#if skill && skill.combo_variants.length > 0}
            <div class="combo-type-row inset">
              <div class="field">
                <span class="field-label">コンボタイプ</span>
                <Choose
                  label="コンボタイプ"
                  options={COMBO_SKILL_TYPE_OPTIONS}
                  full
                  bind:value={
                    () => comboSkillType,
                    (v) => (comboSkillType = v as ComboSkillType)
                  }
                />
              </div>
              <span class="combo-type-note dim">
                {comboSkillType === "chain"
                  ? "シエナのオーラの中ディレイ減少に応じて倍率・段数も変わります"
                  : "タイプを押すと倍率・段数・中ディレイへすぐ反映します"}
              </span>
            </div>
          {/if}

          <!-- この一発 -->
          <div class="hero">
            <!-- 鎖(§14 決定 1)。1 発は**ゲート**(防御を抜けるか・目安を超えるかの閾値判定)、
                 DPS は**レート**(どれくらいの速さで削れるか)で、種類の違う量。ゲートを通らな
                 ければレートに意味が無いので、軸を切り替えず因果の順に繋ぐ。
                 判定(バッジ)はゲートの位置だけに置き、レートには付けない — 「何秒までなら
                 合格」の基準がゲーム側に存在しないので、付けたら嘘になる。
                 44px の主役数値は増やさない(金の帯 = 答えは 1 つ。§02)。鎖が右に伸びるだけ。 -->
            <div class="chain">
              <button
                type="button" class="node gate"
                aria-expanded={isDetailOpen("perHit")} onclick={() => toggleDetail("perHit")}
              >
                <span class="nl">表記ダメージ(1 発)</span>
                <!-- 差分(前回の値からいくつ動いたか)。数値の行には置かない — 44px の数値の横は
                     枠(248px)に入らず、右の節に被る(実機 2026-09-15)。空でも行を取り、出た瞬間に下が動かない -->
                <Value class="hero-num nv" motion={() => perHit} value={perHit !== null ? fmtInt(perHit) : "—"} />
                <span class="nsub num">
                  <!-- 副行は空でも .nsub-line で行を取る。取らないと、差分枠が出た瞬間に節が 11px 伸び、
                       鎖は下ぞろえなので 44px の主役数字がその分だけ持ち上がる(実機 2026-09-17、§00 ③) -->
                  <span class="nsub-line">
                    <Value
                      motion={() => perHit} delta={{}}
                      deltaClass={changedFlowKeys.length > 0 ? "follow" : ""}
                      onDelta={() => followChange("perHit")}
                    />
                  </span>
                </span>
              </button>
              <button
                type="button" class="node mid"
                aria-expanded={isDetailOpen("total")} onclick={() => toggleDetail("total")}
              >
                <span class="nl">合計ダメージ <span class="num">(×<Value motion={() => result?.hit_count ?? null} value={String(result?.hit_count ?? 1)} /> 段)</span></span>
                <Value class="nv" motion={() => totalValue} value={totalValue !== null ? fmtInt(totalValue) : "—"} />
                <!-- クリ率はバッジではなく文で(バッジは要らない — ユーザー判断 2026-09-15)。
                     副行は 3 節とも 1 行にそろえ、数値の縦位置を合わせる -->
                <!-- 副行は縦に積む(差分 → クリ率。「クリなら」は内訳にあるので出さない — ユーザー判断 2026-09-15)。数値の横に並べると節が横に伸びて
                     鎖が折り返す(ユーザー指摘 2026-09-15)。数値の縦位置は .nv の行高で合わせる -->
                <span class="nsub num">
                  <span class="nsub-line"><Value motion={() => totalValue} delta={{}} /></span>
                  <span class="nsub-line">
                    {#if result}
                      {#if result.critical_rate === null}
                        <span>クリ率 未記載 → 確定扱い</span>
                      {:else}
                        <!-- クリが出ないときは「出ない」を言わず、率だけ(ユーザー判断 2026-09-15) -->
                        <!-- 0% は赤(届かない)、100% 未満は黄(ぎりぎり)の状態色。100% は地の色(ユーザー判断 2026-09-15) -->
                        <Value
                          class={`${result.critical_chance <= 0 ? "crit-none" : ""} ${result.critical_chance > 0 && result.critical_chance < 1 ? "crit-partial" : ""}`}
                          value={critChanceStage(result!.critical_chance * 100).label}
                        >
                          {#snippet children()}クリ率 {fmtNum(result!.critical_rate!.value, 1, "%")}{critMode ? ` ・ ${critChanceStage(result!.critical_chance * 100).label}` : ""}{/snippet}
                        </Value>
                      {/if}
                    {/if}
                  </span>
                </span>
              </button>
              <button
                type="button" class="node rate"
                aria-expanded={isDetailOpen("dps")} onclick={() => toggleDetail("dps")}
              >
                <span class="nl">DPS <span class="num">(÷ <Value motion={() => result?.actual_delay?.value ?? null} value={result?.actual_delay ? fmtNum(result.actual_delay.value, 2, "s") : "—"} />)</span></span>
                <Value class="nv" motion={() => dpsValue} value={dpsValue !== null ? fmtInt(Math.round(dpsValue)) : "—"} />
                <span class="nsub dim">
                  <span class="nsub-line"><Value motion={() => (dpsValue === null ? null : Math.round(dpsValue))} delta={{}} /></span>
                  <span class="nsub-line">
                    <!-- コンボ中は「スキルを何回撃てるか」= 1 分 ÷ サイクル。
                         スキルの中ディレイだけで数えると、通常攻撃を挟むぶんを落として速く見える -->
                    <span>
                      {#if result?.combo}
                        {Math.round(result.combo.uses_per_minute)} 回/分 ・
                      {:else if result?.actual_delay}
                        {Math.round(result.actual_delay.uses_per_minute)} 回/分 ・
                      {/if}{critMode ? "クリ確定" : "非クリ"}
                    </span>
                  </span>
                  {#if result && result.expected_dps !== null && result.critical_chance > 0 && result.critical_chance < 1}
                    <span class="nsub-line">
                      期待値 <Value motion={() => result?.expected_dps ?? null} value={fmtInt(Math.round(result.expected_dps))} />(クリ率 {fmtPct(result.critical_chance, 1)})
                    </span>
                  {/if}
                </span>
              </button>
              <!-- 討伐時間。敵 HP か中ディレイが未収録なら出せないので、ノードごと出さない
                   (§00 02。0 や「—」で埋めると画面が嘘をつく)。HP はソロの値 -->
              {#if result && result.defeat_seconds !== null && result.enemy_hp !== null}
                <div class="node rate">
                  <span class="nl">討伐時間 <span class="num">(HP {fmtInt(result.enemy_hp)})</span></span>
                  <Value class="nv" motion={() => result?.defeat_seconds ?? null} value={fmtDuration(result.defeat_seconds)} />
                  <span class="nsub dim">
                    <!-- クリ確定 / 非クリは隣の DPS 節に出ている(重ねない。§00 02) -->
                    <span class="nsub-line">ソロ</span>
                  </span>
                </div>
              {/if}
            </div>
            <!-- 鎖の各数値の内訳。押した節は動かず、鎖の直下に増える(§00 03) -->
            {#if perHitDetail}{@render detailBox(perHitDetail, isDetailOpen("perHit"))}{/if}
            {#if totalDetail}{@render detailBox(totalDetail, isDetailOpen("total"))}{/if}
            {#if dpsDetail}{@render detailBox(dpsDetail, isDetailOpen("dps"))}{/if}
            <!-- 討伐時間が出せない(敵 HP か中ディレイが未収録)ときはメーターも文言も出さない
                 (§00 02。0 や「届かない」で埋めると嘘になる)。計算中・防御力を抜けていない
                 は討伐時間の有無に関わらず伝えるべき事実なので、その 2 つだけは別枠で出す -->
            {#if perHit === null}
              <div class="meter big"><div class="fill" style="width: 0%; background: {STATE.unknown.bar};"></div></div>
              <div class="hero-sentence"><span class="sentence">計算中…</span></div>
            {:else if noPierce}
              <div class="meter big"><div class="fill" style="width: 0%; background: {STATE.short.bar};"></div></div>
              <div class="hero-sentence">
                <span class="sentence ng">防御力を抜けていません(攻撃力 {atkA !== null ? fmtInt(atkA) : "—"} ≤ 防御力 {defenseValue !== null ? fmtInt(defenseValue) : "—"})</span>
              </div>
            {:else if defeatSeconds !== null && reach !== null}
              <div class="meter big"><div class="fill" style="width: {meterRatio * 100}%; background: {STATE[BADGE[badgeState].state].bar};"></div></div>
              <div class="hero-sentence">
                <span class="sentence" class:ok={reached} class:ng={!reached}>
                  {#if reach === "comfortable"}
                    {fmtDuration(tables.reach_seconds.comfortable)}かからずに倒せます。
                  {:else if reach === "reached"}
                    {fmtDuration(defeatSeconds)} で倒せます。
                  {:else if reach === "close"}
                    {fmtDuration(defeatSeconds)}。{fmtDuration(closeSeconds)}の目安ぎりぎりです。
                  {:else}
                    {fmtDuration(defeatSeconds)}。{fmtDuration(closeSeconds)}を超えるので厳しいです。
                  {/if}
                </span>
                <span class="num dim">目安 {fmtDuration(closeSeconds)}以内</span>
              </div>
            {/if}
            <!-- 足りない分をどう埋める? を 1 行に(旧: 紫のパネル)。候補が無い・すでに目安に
                 届いているときは行ごと消す(§00 02) -->
            {#if perHit !== null && !reached && whatIf.length > 0}
              {@const top = whatIf[0]}
              <div class="fill-line">
                <button
                  type="button" class="fill-btn"
                  class:whatif-leaving={leavingWhatIfId === top.id}
                  disabled={leavingWhatIfId === top.id}
                  onclick={() => applyWhatIf(top)}
                >
                  <span class="dim">→ 一番効くのは</span>
                  <span class="fill-label">{top.label}</span>
                  <span class="num fill-pct" class:flat={top.delta_pct === 0}>表記 {deltaText(top.delta_pct)}</span>
                  <span class="num fill-total dim">合計 {deltaText(top.delta_total_pct)}</span>
                </button>
                {#if whatIf.length > 1}
                  <!-- トリガは上の行の中、一覧は行の下。<details> を行に溶かして(display: contents)
                       両方を .fill-line の直接の子にする -->
                  <Disclosure class="fill-fold" summaryClass="fill-more-toggle" bind:open={fillMoreOpen}>
                    {#snippet summary(open)}{open ? "閉じる" : `他 ${whatIf.length - 1} 件`}{/snippet}
                    {#snippet children(open)}
                    {#if open}
                <div class="fill-list inset">
                  {#each whatIf.slice(1) as w (w.id)}
                    <button
                      type="button" class="fill-more-row"
                      class:whatif-leaving={leavingWhatIfId === w.id}
                      disabled={leavingWhatIfId === w.id}
                      onclick={() => applyWhatIf(w)}
                    >
                      <span class="dt-label">{w.label}</span>
                      <span class="num dt-val">表記 {deltaText(w.delta_pct)}</span>
                      <span class="num fill-total dim">合計 {deltaText(w.delta_total_pct)}</span>
                    </button>
                  {/each}
                </div>
                    {/if}
                    {/snippet}
                  </Disclosure>
                {/if}
              </div>
            {/if}
            {#if result?.actual_delay}
              {@const d = result.actual_delay}
              <div class="delay-note dim">
                中ディレイ {fmtNum(d.base, 2, "s")}
                {#if d.fixed}
                  ×(固定・減少が効かない)
                {:else if d.reduction > 0}
                  × (1 − {fmtPct(d.reduction)}){#if d.reduction_raw > d.reduction}<span class="warn"> ※減少値は上限 {fmtPct(limits.actual_delay_reduction_max)}({fmtPct(d.reduction_raw)} ぶん選択中)</span>{/if}
                {/if}
                {#if d.combo_rate < 1}× {fmtNum(d.combo_rate)}(コンボ){/if}
                = {fmtNum(d.value, 2, "s")}{#if d.floored}<span class="warn"> ※下限 {fmtNum(limits.actual_delay_min, 1, "s")}</span>{/if}
                {#if d.contributions.length > 0}
                  ／ 減少源: {d.contributions.map((c) => `${c.source} ${fmtPct(c.rate)}`).join(" ・ ")}
                {/if}
                <br />
                {#if result?.combo}
                  {@const c = result.combo}
                  1 サイクル = 通常攻撃 {fmtNum(c.normal_delay, 2, "s")} + max(スキル {fmtNum(c.skill_delay, 2, "s")},
                  CI {c.interval !== null ? fmtNum(c.interval, 2, "s") : "?"}) = {fmtNum(c.seconds, 2, "s")}
                  ／ 1 秒あたり = (スキル + {c.normal_attack_name})の合計 ÷ 1 サイクル
                {:else}
                  1 秒あたり = 合計 × {Math.round(d.uses_per_minute)} 回/分 ÷ 60
                  {#if d.uses_measured}
                    (<b>実測表</b>: 総減少 {fmtPct(d.reduction)} × 基本 {fmtNum(d.base, 2, "s")})
                  {:else}
                    (実測表の範囲外なので 60 ÷ 中ディレイ の式で算出)
                  {/if}
                {/if}
              </div>
            {/if}
            {#if result?.critical_rate}
              {@const c = result.critical_rate}
              <div class="delay-note dim">
                クリティカル率 (装備クリ補正 {fmtInt(c.equipment_critical)} + 1) × 2 × (AGI {fmtInt(c.agi)} / (AGI + 対象AGI {fmtInt(c.target_agi)}))
                {#if c.siena_rate > 0}× シエナのオーラ {fmtNum(1 + c.siena_rate, 2)}{/if}
                = {fmtNum(c.from_agi, 1, "%")}
                ＋ スキル Cri値 {fmtInt(c.skill)}%{#if c.bonus > 0} ＋ 増加 {fmtInt(c.bonus)}%{/if}
                − 対象のクリティカル被撃率 {fmtInt(-c.target_taken_rate)}%
                = <b>{fmtNum(c.value, 1, "%")}</b>{#if c.raw < 0}<span class="warn"> ※下限 0%</span>{:else if c.raw > 100}<span class="warn"> ※上限 100%</span>{/if}
              </div>
            {:else if result && skill}
              <div class="delay-note dim">
                クリティカル率は出せません(この敵の AGI / クリティカル被撃率、またはスキルの Cri値が wiki 未記載)。
              </div>
            {/if}
            {#if result && result.effective_base_actual_delay === null}
              <div class="delay-note dim">このスキルは wiki に基本中ディレイ(「動作」列)が無いため、1 秒あたりの火力を出せません。</div>
            {/if}
          </div>
        </SheetCard>
        </div>

        <!-- なぜこの数字? -->
        <div class="panel">
          <button type="button" class="panel-head blue" aria-expanded={flowOpen} onclick={() => (flowOpen = !flowOpen)}>
            <span class="panel-title dark">なぜこの数字？</span>
            <span class="panel-note dark">{flowOpen ? "閉じる" : "内訳をひらく"}</span>
            <span class="caret" class:rot={flowOpen}>▼</span>
          </button>
          <div class="panel-body">
            <div class="flow-line">
              <span class="dim">防御を抜けた攻撃力</span>
              <Value
                class="strong"
                motion={() => (pierced === null ? null : Math.max(0, Math.trunc(pierced)))}
                value={pierced !== null ? fmtInt(Math.max(0, Math.trunc(pierced))) : "—"}
                delta={{}}
              />
              <span class="arrow num dim">→</span>
              <span class="dim">倍率</span>
              <!-- 材料を変えると倍率も変わる。跳ねないと 1 つだけ古い値に見える(§00 04。
                   実機の tools/design-audit/live/motion.js が検出した) -->
              <Value class="good strong" value={flowMultLabel} />
              <span class="arrow num dim">→</span>
              <Value class="final" motion={() => perHit} value={perHit !== null ? fmtInt(perHit) : "—"} delta={{}} />
            </div>
            <!-- 積み上げの助言はトグル(ユーザー指示 2026-08-31)。ふだんは畳んでおき、押したときだけ
                 下に開く。押すボタンは上の行に居座るので、開いても押した場所は動かない(§00 03)。
                 ただし「攻撃力が届いていない」「倍率ゼロ」は畳まない — 数字が伸びない理由そのもので、
                 畳むと「なぜこの数字?」に答えないまま閉じることになる -->
            {#if noPierce}
              <div class="lever-note">
                攻撃力が相手の防御力に届いていないので、倍率は何もかかりません。まず攻撃力を上げる必要があります。
              </div>
            {:else if topLever}
              <Disclosure class="lever-toggle" summaryClass="chip quiet" bind:open={leverOpen}>
                {#snippet summary(open)}{open ? "閉じる" : "どこが効いてる？"}{/snippet}
              <div class="lever-note">
                いま一番効いている積み上げは「{topLever.symbol} {topLever.label}」の {fmtCatValue(topLever)}(×{fmtNum(topLever.factor)}){catAtCap(topLever) ? "。上限に達しています" : ""}。
                {#if bestLever}
                  <br />伸ばすなら「{bestLever.symbol} {bestLever.label}」。+1% ごとに最終ダメージが <Value motion={() => bestLeverGain} value={fmtSigned(bestLeverGain, 2, "%")} delta={{ unit: "%", digits: 2 }} /> 伸びます({fmtHeadroom(bestLever)})。
                  {#if nextLevers.length > 0}
                    <!-- 次の候補。押した行は動かず、直下に増える(§00 03)。列は内訳と同じ段。
                         チップは文中に居るので <details> は段に溶かす(display: contents) -->
                    <Disclosure class="next-levers" summaryClass="chip quiet" bind:open={nextLeversOpen}>
                      {#snippet summary()}次の候補 {nextLevers.length}{/snippet}
                  <div class="lever-list inset">
                    {#each nextLevers as c, i (c.category)}
                      <div class="dt-row">
                        <span class="dt-label"><span class="dim">{i + 2}.</span> {c.symbol} {c.label}</span>
                        <span class="num dt-mult dim">{fmtCatValue(c)}</span>
                        <Value class="dt-val" motion={() => leverGain(c)} value={fmtSigned(leverGain(c), 2, "%")} delta={{ unit: "%", digits: 2 }} />
                        <span class="num dt-sub dim">{fmtHeadroom(c)}</span>
                      </div>
                    {/each}
                    <p class="dt-note dim">+1% 足したときの最終ダメージの伸び。いま積んでいる量が少ないカテゴリほど 1% の価値が高い。</p>
                  </div>
                    </Disclosure>
                  {/if}
                {/if}
              </div>
              </Disclosure>
            {:else}
              <div class="lever-note">倍率はまだ何もかかっていません。</div>
            {/if}

            <!-- 閉じていても描画して隠す。閉じている間の変更でも材料の前回値が残り、開いたとき・↑ を辿るときに
                 「何が変わったか」が出せる(detailBox と同じ理由) -->
              <div class="flow-body open-in" hidden={!flowOpen}>
              <!-- ① 攻撃力をつくる -->
              <div class="stage">
                <span class="stage-no" style="background: var(--flow-1);">1</span>
                <span class="stage-title">攻撃力をつくる</span>
                <Value
                  class="strong stage-val" motion={() => atkA} value={atkA !== null ? fmtInt(atkA) : "—"}
                  delta={{}} deltaClass={changedAtkKeys.length > 0 ? "follow" : ""}
                  onDelta={() => followChange("atkA")}
                />
              </div>
              <div class="band">
                {#each atkRows as a (a.k)}
                  <div style="width: {a.pct}; background: {a.c};"></div>
                {/each}
              </div>
              <div class="band-rows">
                {#each atkRows as a (a.k)}
                  <Disclosure
                    class="band-fold" summaryClass="band-row"
                    bind:open={() => isDetailOpen(`atk:${a.k}`), (v) => setDetailOpen(`atk:${a.k}`, v)}
                  >
                    {#snippet summary()}
                    <span class="swatch" style="background: {a.c};"></span>
                    <span class="br-label">{a.k}</span>
                    <span class="br-note dim">{a.note}</span>
                    <Value class="br-val" motion={() => Math.round(a.v)} value={fmtInt(Math.round(a.v))} delta={{}} />
                    <Value class="br-share dim" motion={() => parseFloat(a.share)} value={a.share} />
                    {/snippet}
                    <div class="detail inset">{@render detailBody(register(`atk:${a.k}`, atkDetail(a)))}</div>
                  </Disclosure>
                {/each}
              </div>

              <!-- ② 防御力を抜く -->
              <div class="stage">
                <span class="stage-no" style="background: var(--danger);">2</span>
                <span class="stage-title">相手の防御力を抜く</span>
                <Value
                  class="strong stage-val"
                  motion={() => (pierced === null ? null : Math.max(0, Math.trunc(pierced)))}
                  value={pierced !== null ? fmtInt(Math.max(0, Math.trunc(pierced))) : "—"}
                  delta={{}}
                />
              </div>
              <div class="band">
                <div style="width: {100 - defShare}%; background: var(--flow-pierce);"></div>
                <div style="width: {defShare}%; background: var(--hatch-lost);"></div>
              </div>
              <div class="pierce-note num">
                <span>攻撃力 {atkA !== null ? fmtInt(atkA) : "—"}</span>
                <span class="bad">− 防御 {defenseValue !== null ? fmtInt(defenseValue) : "—"}</span>
                <span class="def-warn" class:bad={defShare >= 60}>
                  {defShare >= 60 ? `攻撃力の ${Math.round(defShare)}% が防御力で消えています` : `防御で消えるのは ${Math.round(defShare)}%`}
                </span>
              </div>

              <!-- ③ 倍率で伸ばす -->
              <div class="stage">
                <span class="stage-no" style="background: var(--flow-3);">3</span>
                <span class="stage-title">倍率で伸ばす</span>
                <span class="stage-note dim">帯の幅＝足した分(赤字は減る倍率)</span>
                <Value
                  class="strong stage-val" motion={() => perHit} value={perHit !== null ? fmtInt(perHit) : "—"}
                  delta={{}} deltaClass={changedFlowKeys.length > 0 ? "follow" : ""}
                  onDelta={() => followChange("perHit")}
                />
              </div>
              <div class="band">
                {#each flowRows.filter((r) => r.add > 0) as f (f.k)}
                  <div style="width: {(Math.max(0, f.add) / flowTotal) * 100}%; background: {f.c};"></div>
                {/each}
              </div>
              <div class="band-rows">
                {#each flowRows as f (f.k)}
                  <Disclosure
                    class="band-fold" summaryClass="band-row"
                    bind:open={() => isDetailOpen(`flow:${f.k}`), (v) => setDetailOpen(`flow:${f.k}`, v)}
                  >
                    {#snippet summary()}
                    <span class="swatch" style="background: {f.c};"></span>
                    <span class="br-label" class:strong={topLeverStep === f.k} class:bad={f.add < 0}>{f.k}</span>
                    <span class="num br-mult dim">{f.mult}</span>
                    <Value
                      class={`br-val ${f.add < 0 ? "bad" : ""}`} motion={() => Math.round(f.add)} value={fmtSigned(f.add)}
                      delta={{}} deltaClass={changedFlowKeys.includes(`flow:${f.k}`) ? "follow" : ""}
                      onDelta={(e) => { e.stopPropagation(); followChange(`flow:${f.k}`); }}
                    />
                    <Value class="br-share dim" motion={() => Math.round((Math.abs(f.add) / flowTotal) * 100)} value={fmtPct(Math.abs(f.add) / flowTotal)} />
                    {/snippet}
                    <div class="detail inset">{@render detailBody(register(`flow:${f.k}`, stepDetail(f.step, f.mult, f.add, f.to)))}</div>
                  </Disclosure>
                {/each}
              </div>

              <!-- 効いていない分(§14 決定 2)。5 階層に散っていた「捨てた量」をここに集める -->
              <div class="materials">
                <div class="mat-head">
                  <span class="mat-title">どこで頭打ち？</span>
                  <span class="dim">積んだのに上限で捨てている量</span>
                </div>
                {#if lostRows.length === 0}
                  <p class="lost-none dim">まだどの上限にも当たっていません。積んだ分はすべて効いています。</p>
                {:else}
                  <div class="lost">
                    {#each lostRows as r (r.k)}
                      <div class="lost-row inset">
                        <span class="lost-label">{r.k}</span>
                        <Value class="lost-raw" value={r.raw} />
                        <span class="lost-arrow dim">→ 上限</span>
                        <Value class="lost-val" value={r.val} />
                        <span class="lost-bar" aria-hidden="true">
                          <i style="width: {r.kept * 100}%"></i>
                          <i class="cut" style="width: {100 - r.kept * 100}%"></i>
                        </span>
                        <Value class="lost-loss" value={`${r.loss} は無効`} />
                      </div>
                    {/each}
                  </div>
                  <p class="lost-note dim">斜線が捨てている量です。ここが太い枠は、伸ばしても数字が動きません。</p>
                {/if}
              </div>

              <!-- 倍率の材料 -->
              <div class="materials">
                <div class="mat-head">
                  <span class="mat-title">倍率の材料</span>
                  <span class="dim">上限に届いた枠は「満」</span>
                </div>
                <div class="mat-chips">
                  {#each activeCategories as c (c.category)}
                    <span class="mat-chip" class:cap={catAtCap(c)} use:changed={() => (catAtCap(c) ? "cap" : "open")}>
                      <span class="dim">{c.label}</span>
                      <Value class="strong" value={fmtCatValue(c)} />
                      {#if catAtCap(c)}<span class="full">満</span>{/if}
                    </span>
                  {/each}
                  {#if activeCategories.length === 0}
                    <span class="dim">まだ倍率の材料がありません(バフ・称号などを設定すると増えます)。</span>
                  {/if}
                </div>
                <!-- 計算に入らないものを明示する(黙って 0 にしない) -->
                <p class="mat-note dim">
                  称号・ランダムOP は<b>キャラ</b>タブで選んだものが入ります(発動条件付きの
                  ランダムOP と称号の条件付き効果は記録するだけで計算に入りません)。
                  属性値はキャラの基礎値 + 装備の属性強化で計算しますが、wiki の一覧から属性を読み取れない
                  スキルは属性差ボーナスなし(×1.00)で出ます。
                  防御側(防御力・カット率・回避)は<b>防御</b>タブに出しています。
                </p>
              </div>

              {#if result}
                <TracePanel trace={result.trace} />
              {/if}
              </div>
          </div>
        </div>
      {/if}
  {/snippet}
  {#snippet right()}
      {#if character && payload}
        <!-- 試し変更バー -->
        <div class="sim-bar" class:active={simDirty}>
          <div class="sim-line">
            <span class="sim-dot" class:active={simDirty}></span>
            <span class="sim-title">{simDirty ? "装備・スキルを試し変更中" : "装備・スキルはキャラ登録どおり"}</span>
            <!-- 差分の枠も常に確保する。出た瞬間に行が 1px 伸びて下がずれる(§09 規則 4) -->
            <span class="num sim-delta" class:on={simDirty} class:up={deltaPct > 0} class:down={deltaPct < 0}
            >{simDirty ? deltaText(deltaPct) : ""}</span>
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
          <div class="sim-actions" class:idle={!simDirty} inert={!simDirty}>
            <button type="button" class="btn" onclick={resetSim}>ぜんぶ戻す</button>
            <button type="button" class="btn primary" disabled={saving} onclick={saveSim}>{saving ? "保存中…" : "キャラに保存"}</button>
          </div>
        </div>

        <!-- 試し変更の印。**チップではない**(押して選ぶものではなく、いま変えている印と
             その取り消し)ので `.chip` の名前も見た目も借りない。高さを固定し、3 件までなので
             1 行に収め、溢れたら横にスクロールさせる(行が増えて下をずらさない) -->
        <div class="sim-marks">
          {#each changedKnobs as k (k.id)}
            <span class="sim-mark badge-in" use:changed={() => k.get(app.sim!)}>
              <span>{k.label(app.sim!)}</span>
              <button type="button" class="sim-revert" title="この変更だけ戻す" onclick={() => revertKnob(k)}>✕</button>
            </span>
          {/each}
        </div>
        <!-- 上限の注記は固定領域。登録どおり(0 件)のときは中身だけ出さず、枠は常に確保する
             (§09 規則 1・4)。ここは押した材料(装備・バフ)より上なので、丸ごと差し込むと
             押したものが流れる -->
        <div class="sim-limit" class:hit={simLimited}>
          {#if simLimited}
            試し変更は同時 {SIM_LIMIT} 件までです。どれかを ✕ で戻すか、「キャラに保存」で確定してください。
          {:else if changedKnobs.length > 0}
            同時に試せるのは {changedKnobs.length} / {SIM_LIMIT} 件です。
          {/if}
        </div>

        <!-- 極限スキル(試し変更)。3 種から 2 つ選ぶ(§07 形態 3: チップで入れる/外す) -->
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
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
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
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
                    (v) => editSim((p) => (p.awakening.stage = Number(v)))
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
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
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
                    (v) => editSim((p) => (p.common_skills.sharpness_vision_level = Number(v)))
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
 onclick={() => editSim((p) => (p.common_skills.sharpness_vision_level = 0))}
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
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
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
                    (v) => editSim((p) => (p.stat_sources.soul_link[row.field] = v))
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
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
          {#snippet summary()}
            <!-- 見出しの顔は装着中の武器(名前と併記) -->
            <Icon kind="equipment" id={equipmentIconId(weaponOf(payload).item_id, app.equipmentCatalog)} size={20} label="装備の切り替え" />
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
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
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
                            (v) => editSim((p) => setEnchantValue(p.equipment, row.slot, k, v))
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
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
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
            onToggle={() => { if (polishDef) toggleBuffChip(polishDef); }}
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
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
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
        <Disclosure class="card" summaryClass="card-head toggle" group="material">
          {#snippet summary()}
            <Icon kind="buff" id="illumination_drink" size={20} label="バフ" />
            <span class="card-title">バフ</span>
            <Value class="dim small" motion={() => alwaysBuffCount + extraBuffCount} value={`${alwaysBuffCount + extraBuffCount} 件`} />
          {/snippet}
          {#snippet children(open)}
          {#if open}
          <div class="calc-buff-set">
            <span>使うセット</span>
            <Picker
              label="使うバフセット"
              options={buffSetOptions(app.buffSets, "追加だけで計算")}
              bind:value={() => (app.calcBuffSetId === null ? "" : String(app.calcBuffSetId)), chooseCalcBuffSet}
            />
          </div>
          <p class="buff-legend dim">
            <span class="lg always">常</span> セット内({alwaysBuffCount} 件)
            ／ <span class="lg extra">追</span> 追加 = この計算だけ({extraBuffCount} 件・保存されません)
            ／ 無印 使わない。
          </p>
          <!-- 目的ごとに畳む。35 個を全部並べると 31 行(1177px)になり、ペインの大半を
               バフが占める。見出しは常に同じ場所にあり、開いても**その下に生えるだけ**で
               上は動かない(§09 規則 2)。どこに何件入れているかは見出しの n/m で分かるので、
               閉じたままでも「使い忘れ」に気づける -->
          {#each BUFF_PURPOSES as purpose (purpose.id)}
            {@const defs = consumableBuffs.filter((d) => matchesPurpose(d, purpose.id))}
            {@const picked = defs.filter((d) => buffState(d) !== "off").length}
            {#if defs.length > 0}
              <Disclosure summaryClass="buff-group-head inset" group="buffPurpose">
                {#snippet summary()}
                  <span class="bg-label">{purpose.label}</span>
                  <Value class="bg-count" motion={() => picked} value={`${picked}/${defs.length}`} />
                {/snippet}
                <div class="buff-chips">
                  {#each defs as def (def.id)}{@render buffChip(def)}{/each}
                </div>
              </Disclosure>
            {/if}
          {/each}
          <p class="buff-note dim">変更はこの計算だけに反映され、バフセットやキャラには保存されません。</p>
          {/if}
          {/snippet}
        </Disclosure>

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
      {:else}
        <p class="empty dim">キャラを選択してください。</p>
      {/if}
  {/snippet}
</SplitPage>

<style>
  /* .layout / section / .scroll は ui/SplitPage.svelte(padding の差は rightScrollStyle で指定) */
  .empty { font-size: 12px; }

  /* 行ける?カード。.sheet-card/.sheet-head/.gem/.sheet-title/.sheet-char は ui/SheetCard.svelte */

  .target-row { position: relative; z-index: 3; display: flex; align-items: center; gap: 8px; padding: 10px 11px 0; background: linear-gradient(180deg, #F4F9FE, #fff); }
  .step {
    flex-shrink: 0; width: 25px; height: 25px; display: flex; align-items: center; justify-content: center;
    border-radius: var(--r-inset); background: linear-gradient(180deg, #fff, #E9F1FB); border: 1px solid #9FB4D0;
    font-size: 9px; font-weight: 700; color: var(--fg-sub);
  }
  .step:hover { background: var(--bg-active); }
  /* トリガは ui/Popover.svelte が描くので、祖先経由でスコープ付き CSS を届かせる */
  .target-row :global(.target-trigger) { min-width: 0; flex: 1; padding: 3px 8px; border-radius: var(--r-panel); border: 1px solid transparent; text-align: left; }
  .target-row :global(.target-trigger:hover), .target-row :global(.target-trigger[aria-expanded="true"]) { background: var(--bg-rail); border-color: #9FB4D0; }
  .t-line1 { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .t-name { min-width: 0; font-size: 15px; font-weight: 800; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .t-index { flex-shrink: 0; margin-left: auto; font-size: 8.5px; }
  .t-line2 { margin-top: 1px; display: flex; align-items: baseline; gap: 9px; min-width: 0; }
  .t-area { min-width: 0; font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .t-def { flex-shrink: 0; font-size: 8.5px; color: var(--danger); }
  .t-need { flex-shrink: 0; font-size: 8.5px; color: var(--fg-sub); }

  /* 対象・スキルの候補面。重なり方と閉じ方は ui/Popover.svelte ＋ app.css の .popover が持つので、
     ここは行を端まで使うための余白なしと幅・高さだけ。トップレイヤに乗るので祖先が無く、:global で書く */
  :global(.target-pop), :global(.skill-pop) {
    gap: 0; padding: 0; max-height: 262px; border-radius: var(--r-window);
    min-width: anchor-size(width); font-size: var(--t-body);
  }
  :global(.skill-pop) { border-color: #A9821F; }
  .pop-head {
    position: sticky; top: 0; z-index: 1; display: flex; align-items: center; gap: 7px;
    padding: 6px 13px 6px 11px;
    background: linear-gradient(180deg, #DBE6F8, #C6D8F0); border-bottom: 1px solid var(--border);
    font-size: 9.5px; font-weight: 800; letter-spacing: 0.1em; color: var(--fg-head);
  }
  .pop-head.gold { background: linear-gradient(180deg, #F2E3BD, #DCC27E); border-bottom-color: #BFA155; color: #4A3C12; }
  .pop-head .num { margin-left: auto; font-weight: 400; }
  .pop-diamond { width: 6px; height: 6px; transform: rotate(45deg); background: var(--head-bar); }
  .pop-row {
    width: 100%; display: flex; align-items: center; gap: 9px; padding: 7px 13px 7px 11px;
    border-bottom: 1px solid #EDF2F9; text-align: left;
  }
  .pop-row:hover { background: #F1F7FE; }
  .pop-row.on { background: var(--sel-card); }
  .pop-row .dot { width: 7px; height: 7px; flex-shrink: 0; border-radius: 50%; }
  .pop-name { min-width: 0; flex: 1; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pop-row.on .pop-name { font-weight: 700; }
  .pop-row .strong { font-weight: 700; }

  .skill-row { position: relative; z-index: 2; padding: 8px 11px 0; display: flex; align-items: center; gap: 7px; }
  /* トリガは ui/Popover.svelte が描く。スキルが 1 つのときは開かない(disabled)が、
     行としては同じ面なので薄くしない */
  .skill-row :global(.skill-trigger) {
    min-width: 0; flex: 1; display: flex; flex-direction: column; align-items: stretch; gap: 1px;
    padding: 5px 9px; border-radius: var(--r-panel); background: #F4F9FE; border: 1px solid #D6E2F0; text-align: left;
  }
  .skill-row :global(.skill-trigger:hover:not(:disabled)) { background: var(--bg-rail); }
  .skill-row :global(.skill-trigger:disabled) { opacity: 1; cursor: default; }
  .sk-line1 { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  .sk-name { min-width: 0; flex: 1; font-size: 11.5px; font-weight: 700; color: #3E2B26; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sk-meta { font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* 主軸を上書き中の印と戻し先。保存されない状態なのでラベンダー(--sim)。
     どちらも行の高さを変えない小物で、主軸どおりのときは出ない(§00 02) */
  .sk-override {
    flex-shrink: 0; padding: 0 6px; border-radius: var(--r-pill);
    background: var(--state-temp-bg); border: 1px solid var(--sim); color: var(--sim-fg);
    font-size: 8.5px; font-weight: 700; white-space: nowrap;
  }
  .sk-reset {
    flex-shrink: 0; padding: 4px 9px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--sim); color: var(--sim-fg);
    font-size: 9.5px; font-weight: 700; white-space: nowrap;
  }
  .sk-reset:hover { background: var(--state-temp-bg); }
  .combo-type-row {
    margin: 8px 11px 0; padding: 8px 10px;
    display: grid; grid-template-columns: minmax(220px, 320px) minmax(0, 1fr); align-items: end; gap: 10px;
  }
  .combo-type-note { min-width: 0; padding-bottom: 3px; font-size: 9px; line-height: 1.45; }
  /* コンボの成立条件と、挟む通常攻撃の段。ON のときだけ出るので、
     押した場所より下にしか増えない(§00 03) */
  .combo-normal { margin-top: 8px; }
  .combo-note { margin: 6px 0 0; font-size: 10px; line-height: 1.6; }
  @media (max-width: 720px) {
    .combo-type-row { grid-template-columns: minmax(0, 1fr); align-items: stretch; }
  }

  .hero { padding: 11px 13px 12px; }
  /* .hero-num は ui/Value.svelte が描く(子コンポーネントの要素なのでスコープ付き CSS が届かない) */
  .hero :global(.hero-num) { font-size: var(--t-result); line-height: 1; font-weight: var(--w-strong); }
  .meter.big { margin-top: 10px; height: 12px; border-radius: var(--r-inset); }
  .hero-sentence { margin-top: 7px; display: flex; align-items: baseline; gap: 9px; min-width: 0; }
  .sentence { min-width: 0; flex: 1; font-size: 11px; font-weight: 700; text-wrap: pretty; }
  .sentence.ok { color: var(--good); }
  .sentence.ng { color: var(--danger); }
  .hero-sentence .num { flex-shrink: 0; font-size: 9.5px; }
  /* 鎖(§14 決定 1)。44px の主役数値は増やさない — 金の帯 = 答えは 1 つ(§02)を
     壊さず、鎖が右に伸びるだけ。狭いときは折り返す(桁で隣が動かないよう各段に min-width) */
  /* 節の高さは 3 つとも同じ(stretch)。タイトル行は天井に固定し、数値と副行は底に寄せる
     (= 下ぞろえ。タイトルの位置は固定 — ユーザー判断 2026-09-15) */
  /* 押せる範囲(節)は 3 つとも同じ高さ(stretch)。内側の余白 4px / 6px を節に持たせ、
     hover の塗りが節の形そのものになるようにする。左端は余白ぶん外に出して、上の行と文字位置をそろえる */
  .chain { display: flex; align-items: stretch; gap: 4px; flex-wrap: wrap; margin: 0 -6px; }
  /* 枠は付けない(付けると窮屈 — ユーザー判断 2026-09-15)。見出しはタイトル行として太字で主張させる。
     余白: タイトル行 → 数値 6px、数値 → 差分 4px、差分 → 補足 4px(8 / 4 / 4 の一段階トーンダウン。
     5 / 2 だと差分と補足が密着して 1 塊に読める — レビュー 2026-09-15) */
  .chain .node { display: flex; flex-direction: column; gap: 0; min-width: 0; padding: 4px 6px 5px; }
  .chain .nl {
    line-height: 14px; font-size: 10px; font-weight: 700; color: var(--fg-head); white-space: nowrap;
  }
  /* .num は ui/Value.svelte が描く子要素にも付くので :global で届かせる */
  .chain .nl :global(.num) { font-weight: 600; color: var(--fg-muted); }
  .chain :global(.nv) { margin-top: auto; padding-top: 6px; }
  .chain .nsub { margin-top: 4px; gap: 4px; }
  .chain :global(.nv) { font-weight: 700; color: var(--fg); white-space: nowrap; }
  .chain .node.gate :global(.nv) { min-width: 120px; }
  .chain .node.mid :global(.nv) { font-size: 15px; min-width: 68px; }
  /* DPS は主役(1 発 = --t-result)の隣の見出し数字。--t-result だと 1 発と並んで大きすぎる(ユーザー 2026-09-16) */
  .chain .node.rate :global(.nv) { font-size: var(--t-heading); min-width: 68px; }
  /* 押せるノードは桁・状態で動かない。実測(tools/design-audit/live/digits.js)では
     1 発 180〜220px・バッジ 39〜106px・合計 75〜83px と揺れ、右のノードが最大 29px 逃げていた
     (§09 規則 4)。幅を取り切り、中身が短いときは空けておく。値の上限
     (与ダメージ 29,500,000 / 合計 10 桁)が入る幅にしてあるので、桁が溢れて切れることはない */
  .chain .node.gate { width: 260px; }
  .chain .node.mid { min-width: 136px; }
  .chain .nsub { font-size: 9px; color: var(--fg-dim); white-space: nowrap; display: flex; flex-direction: column; }
  .chain .nsub-line { display: flex; align-items: baseline; gap: 5px; min-height: 14px; }
  /* ui/Value.svelte が描く crit-none / crit-partial は子コンポーネントの要素 */
  .chain :global(.crit-none) { color: var(--state-short-fg); font-weight: 700; }
  .chain :global(.crit-partial) { color: var(--state-edge-fg); font-weight: 700; }
  /* 差分(いくつ変わったか)は数値の真下。9px だと 44px の数値の下で読めないので 11px。
     差分枠は ui/Value.svelte が描く(スコープ付き CSS が届かないので :global) */
  .chain .nsub :global(.delta) { font-size: 11px; line-height: 14px; }
  /* 押すと内訳が鎖の直下に開く。hover は塗りだけで、余白を足して隣を動かさない(§00 03) */
  .chain button.node { text-align: left; border-radius: var(--r-inset); }
  .chain button.node:hover { background: var(--bg-active); }
  .chain button.node:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .delay-note { margin-top: 6px; font-size: 9px; line-height: 1.5; }
  .delay-note .warn { color: var(--danger, #B5443A); }

  /* パネル(もし〜/なぜ) */
  .panel { margin-top: 11px; border-radius: var(--r-window); overflow: hidden; border: 1px solid var(--border-strong); background: var(--bg-field); }
  .panel-head { width: 100%; display: flex; align-items: center; gap: 8px; padding: 7px 12px; text-align: left; }
  .panel-head.blue { background: linear-gradient(180deg, #DBE6F8, #AEC7F0); border-bottom: 1px solid var(--border-strong); cursor: pointer; }
  .panel-title { font-size: var(--t-label); font-weight: 800; letter-spacing: 0.08em; color: #fff; white-space: nowrap; }
  .panel-title.dark { color: var(--fg); }
  .panel-note { min-width: 0; flex: 1; text-align: right; font-size: 9px; color: #E4E3F4; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .panel-note.dark { color: #40536F; }
  .panel-body { padding: 11px 13px 12px; }

  /* 足りない分をどう埋める? を 1 行に(旧: 紫のパネル)。押した場所は動かない(§00 03) */
  .fill-line { margin-top: 8px; display: flex; flex-wrap: wrap; align-items: baseline; gap: 8px; min-width: 0; }
  .fill-btn {
    min-width: 0; flex: 1; display: flex; align-items: baseline; gap: 6px; padding: 3px 6px;
    border-radius: var(--r-inset); text-align: left; font-size: 10.5px;
  }
  .fill-btn:hover:not(:disabled) { background: var(--state-temp-bg); }
  .fill-label { min-width: 0; flex-shrink: 1; font-weight: 700; color: var(--sim-fg); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .fill-pct { flex-shrink: 0; font-weight: 700; color: var(--good); }
  /* ±0% は「増えた」ではない。色で増減を言っているので、動かないときは色も外す */
  .fill-pct.flat { color: var(--fg-dim); }
  /* 合計ダメージ側の伸び率。桁が変わっても表記側の位置が動かないよう幅を固定する */
  .fill-total { flex-shrink: 0; min-width: 74px; text-align: right; font-size: 9.5px; font-weight: 700; }
  /* 段に溶かした <details> の本文は ::details-content という箱を 1 枚挟む(display: block)。
     行の flex item はその箱なので、全幅に落とすのは箱のほう(実測 2026-09-17) */
  .fill-line :global(details.fill-fold) { display: contents; }
  .fill-line :global(details.fill-fold::details-content) { flex-basis: 100%; }
  .fill-line :global(summary.fill-more-toggle) { flex-shrink: 0; display: flex; align-items: center; gap: 3px; padding: 2px 6px; border-radius: var(--r-inset); font-size: 9px; color: var(--fg-muted); }
  .fill-line :global(summary.fill-more-toggle:hover) { color: var(--fg); background: var(--bg-active); }
  .fill-list {
    margin-top: 4px; padding: 5px 7px; display: flex; flex-direction: column; gap: 2px;
  }
  .fill-more-row { width: 100%; display: flex; align-items: center; gap: 8px; padding: 2px 3px; border-radius: var(--r-inset); text-align: left; }
  .fill-more-row:hover:not(:disabled) { background: var(--bg-active); }

  .flow-line { display: flex; align-items: center; gap: 7px; flex-wrap: wrap; font-size: 9px; }
  /* strong / good.strong / final は ui/Value.svelte が描く子要素なので :global で届かせる */
  .flow-line :global(.strong) { font-size: 13px; font-weight: 700; color: var(--fg-sub); }
  .flow-line :global(.good.strong) { color: var(--flow-3); }
  .flow-line :global(.final) { font-size: 15px; font-weight: 700; color: var(--fg); }
  .lever-note :global(.chip) { margin-left: 6px; vertical-align: middle; }
  .lever-list {
    margin-top: 6px; padding: 6px 8px; display: flex; flex-direction: column; gap: 3px;
  }
  :global(details.lever-toggle) { margin-top: 9px; }
  /* 文中のチップなので段に溶かす。一覧は .lever-note の子として並ぶ */
  .lever-note :global(details.next-levers) { display: contents; }
  /* トグルの直下に開くので、上マージンは詰める(帯が二重に空かない) */
  :global(details.lever-toggle) .lever-note { margin-top: 6px; }
  .lever-note {
    margin-top: 9px; padding: 8px 10px; border-radius: var(--r-panel);
    background: #F4F9FE; border: 1px solid var(--border-soft);
    font-size: var(--t-label); font-weight: 500; line-height: 1.6; color: var(--fg-sub); text-wrap: pretty;
  }

  .stage { margin-top: 14px; padding-top: 12px; border-top: 1px dashed var(--border-soft); display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .stage-no { flex-shrink: 0; width: 15px; height: 15px; border-radius: 50%; color: #fff; font-size: 9px; line-height: 16px; text-align: center; font-family: var(--font-num); font-variant-numeric: tabular-nums; font-weight: 700; }
  .stage-title { font-size: 11px; font-weight: 700; white-space: nowrap; }
  .stage-note { min-width: 0; flex: 1; font-size: 9px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* .stage-val は ui/Value.svelte が描く */
  .stage :global(.stage-val) { margin-left: auto; font-size: 15px; font-weight: 700; }
  /* 段の数値と行の数値の右端をそろえる: 数値(64)+ 差分(64)+ 割合(32)の 3 列を段にも持たせる。
     差分枠の幅を固定しないと、文言の幅ぶん数値が左右にずれる(ユーザー指摘 2026-09-15) */
  .stage :global(.delta), .band-rows :global(summary.band-row .delta) { flex-shrink: 0; width: 64px; font-size: 10px; }
  .stage::after { content: ""; flex-shrink: 0; width: 32px; }
  .band { margin-top: 7px; display: flex; height: 11px; border-radius: var(--r-inset); overflow: hidden; border: 1px solid var(--border-soft); background: #EDF2F9; }
  .band > div { flex-shrink: 0; transition: width var(--dur-bar) var(--ease-in-out); }
  .band-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 5px; }
  .band-rows :global(summary.band-row) { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .swatch { flex-shrink: 0; width: 8px; height: 8px; border-radius: var(--r-inset); }
  .br-label { min-width: 0; flex: 1; font-size: var(--t-label); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .br-label.strong { font-weight: 700; }
  .br-label.bad { color: var(--danger); }
  .br-note { min-width: 0; flex: 1.2; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .br-mult { flex-shrink: 0; width: 48px; text-align: right; font-size: 10px; }
  /* br-val / br-share は ui/Value.svelte が描く子要素 */
  .band-rows :global(.br-val) { flex-shrink: 0; width: 64px; text-align: right; font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .band-rows :global(.br-val.bad) { color: var(--danger); }
  .band-rows :global(.br-share) { flex-shrink: 0; width: 32px; text-align: right; font-size: 9.5px; }
  /* 構成行・段フローの行は押すと内訳が直下に開く。行の位置・高さは変わらない */
  .band-rows :global(details.band-fold) { display: contents; }
  .band-rows :global(summary.band-row) { width: 100%; text-align: left; border-radius: var(--r-inset); }
  .band-rows :global(summary.band-row:hover) { background: var(--bg-active); box-shadow: 0 0 0 3px var(--bg-active); }
  .band-rows :global(summary.band-row:focus-visible) { outline: 2px solid var(--accent); outline-offset: 2px; }

  /* 押した数値の内訳。読み取り専用なのでインセット面、列は band-row と同じ段にそろえる。
     この 2 つはトリガ(鎖の節 / 帯の見出し)が別の入れ物にいて <details> に載らないので、
     面だけを §10 型 6 の .open-in + hidden で出し入れする。display を上書きしているので、
     隠れているあいだは自分で消す */
  .detail[hidden], .flow-body[hidden] { display: none; }
  /* 内訳の行の中の開閉(ui/Disclosure)。段は .detail の縦並びのままにする */
  .detail-body :global(details.dt-fold) { display: contents; }
  .detail { margin: 6px 0 2px; padding: 7px 9px; }
  /* 中身は <Disclosure> の子としても使う(トリガと面が並ぶ band-row)ので、段は中身側が持つ */
  .detail-body { display: flex; flex-direction: column; gap: 4px; }
  .dt-head { display: flex; align-items: baseline; gap: 6px 7px; flex-wrap: wrap; }
  .dt-hk { font-size: 9px; letter-spacing: 0.06em; }
  /* dt-hv は ui/Value.svelte が描く子要素 */
  .dt-head :global(.dt-hv) { min-width: 62px; font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .dt-head :global(.dt-hv.big) { font-size: 13px; color: var(--fg); }
  .dt-head :global(.dt-hv.bad) { color: var(--danger); }
  .dt-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .dt-label { min-width: 0; flex: 1; font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dt-mult { flex-shrink: 0; width: 48px; text-align: right; font-size: 9.5px; }
  /* dt-val も ui/Value.svelte が描く子要素 */
  .dt-row :global(.dt-val) { flex-shrink: 0; width: 64px; text-align: right; font-size: 10px; font-weight: 700; color: var(--fg-sub); }
  .dt-sub { flex-shrink: 0; width: 112px; text-align: right; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dt-row :global(.dt-val.bad) { color: var(--danger); }
  /* 入れ替わった供給源はラベルの隣に残す(次の変化で書き換わる) */
  .dt-swap { margin-left: 8px; font-size: 9px; }
  /* 抜けた供給源の行。次に集合が変わるまで取り消し線で残す(消すと「どこが変わったか」が消える) */
  .dt-row.gone .dt-label, .dt-row.gone :global(.dt-val) { text-decoration: line-through; color: var(--fg-dim); }
  /* 差分枠は実数と同じ書体サイズ・同じ幅の列にして、行ごとに 到達 の位置がずれないようにする */
  .dt-row :global(.delta), .dt-head :global(.delta) { flex-shrink: 0; min-width: 64px; font-size: 10px; }
  .dt-head :global(.delta) { font-size: 11px; }
  /* ステの行は押すと要因が直下に開く。行の位置・高さは変わらない */
  .detail-body :global(summary.dt-row-btn) {
    width: 100%; text-align: left; padding: 1px 3px; margin: 0 -3px;
    border: 0; background: none; color: inherit; font: inherit; border-radius: var(--r-inset);
  }
  .detail :global(summary.dt-row-btn:hover) { background: var(--bg-active); }
  .detail :global(summary.dt-row-btn:focus-visible) { outline: 2px solid var(--accent); outline-offset: 1px; }
  .dt-subs {
    display: flex; flex-direction: column; gap: 3px;
    margin: 2px 0 3px 8px; padding-left: 8px; border-left: 1px solid var(--border-soft);
  }
  .dt-note, .dt-expr { margin: 2px 0 0; font-size: 9px; line-height: 1.6; }
  .dt-expr { font-family: var(--font-num); font-variant-numeric: tabular-nums; word-break: break-all; }
  .pierce-note { margin-top: 7px; display: flex; align-items: center; gap: 10px; font-size: 9.5px; color: var(--fg-muted); min-width: 0; }
  .def-warn { min-width: 0; flex: 1; text-align: right; font-family: var(--font); font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .def-warn.bad { color: var(--danger); }

  .materials { margin-top: 12px; padding-top: 10px; border-top: 1px dashed var(--border-soft); }
  .mat-head { display: flex; align-items: baseline; gap: 8px; }
  .mat-title { font-size: 10px; font-weight: 700; letter-spacing: 0.06em; color: var(--fg-muted); }
  .mat-head .dim { margin-left: auto; font-size: 9px; }
  .mat-chips { margin-top: 7px; display: flex; flex-wrap: wrap; gap: 5px; }
  .mat-chip {
    display: inline-flex; align-items: center; gap: 6px; padding: 4px 9px; border-radius: var(--r-panel);
    background: var(--bg-panel); border: 1px solid var(--border-soft); font-size: 9.5px;
  }
  .mat-chip.cap { background: var(--state-short-bg); border-color: var(--state-short-bd); }
  /* 効いていない分の棚卸し(§14 決定 2)。塗り = 効いている量、斜線 = 捨てた量(§03) */
  .lost { margin-top: 7px; display: flex; flex-direction: column; gap: 4px; }
  .lost-row {
    display: flex; align-items: center; gap: 8px; min-width: 0;
    padding: 4px 9px;
  }
  .lost-label { min-width: 0; flex: 1; font-size: 10px; font-weight: 700; color: var(--fg-head); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* lost-raw / lost-val / lost-loss は ui/Value.svelte が描く子要素 */
  .lost-row :global(.lost-raw) { flex-shrink: 0; min-width: 62px; text-align: right; font-size: 10px; color: var(--fg-muted); text-decoration: line-through; }
  .lost-arrow { flex-shrink: 0; font-size: 9px; }
  .lost-row :global(.lost-val) { flex-shrink: 0; min-width: 62px; text-align: right; font-size: 10px; font-weight: 700; }
  .lost-bar { flex-shrink: 0; width: 96px; height: 7px; display: flex; border-radius: var(--r-inset); overflow: hidden; border: 1px solid var(--border-soft); }
  .lost-bar > i { display: block; background: var(--flow-1); }
  .lost-bar > i.cut { background: var(--hatch-lost); }
  .lost-row :global(.lost-loss) { flex-shrink: 0; min-width: 104px; text-align: right; font-size: 10px; color: var(--danger); }
  .lost-none, .lost-note { margin: 4px 0 0; font-size: 10px; line-height: 1.6; }
  /* .strong は ui/Value.svelte が描く子要素(倍率の材料チップの値) */
  .mat-chip :global(.strong) { font-size: 10px; font-weight: 700; }
  .mat-chip .full { font-size: 8.5px; font-weight: 700; color: var(--danger); }

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
  .enchant-stat :global(.enchant-gain.up) { color: var(--good); font-weight: 700; }
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

  /* 攻撃 / 防御。粒の並び(`.chips`)の上に、この画面だけの大きさと地を重ねる */
  :global(.side-tabs) { gap: 6px; margin-bottom: 9px; }
  :global(.side-tabs > .chip) {
    padding: 6px 18px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, #fff, #E9F1FB); border: 1px solid var(--border-strong);
    font-size: 11.5px; font-weight: 700; color: #2B3C57;
  }
  :global(.side-tabs > .chip.on) {
    border-radius: var(--r-panel);
    background: var(--sel-card); border-color: var(--accent); color: var(--sel-fg);
    box-shadow: inset 0 1px 0 #fff;
  }

  .mat-note { margin: 7px 0 0; font-size: 9px; line-height: 1.6; }

  /* 35 個を pill で流すと幅がまちまちで、251px の器では 31 行中 26 行が 1 個だけだった
     (実測)。横に並ぶ利点が出ないうえ、名前の頭が縦に揃わず探しにくい。**1 列の行**にして、
     頭を揃える。丸(--r-pill)は「小さな状態の印」に使う形なので、行にはインセットの角丸 */
  .buff-chips {
    margin-top: 5px; margin-bottom: 3px; display: flex; flex-direction: column; gap: 3px;
  }
  .enchant-dep { margin: 6px 0 0; font-size: 9px; }
  /* カード見出しの開閉(ui/Disclosure の <summary>)。押しても見出し自身は動かず、
     中身がその下に生えるだけ。キャレットは部品が置くので、ここは字寸も向きも持たない */
  :global(summary.card-head.toggle) {
    width: 100%; padding: 0; border: 0; background: none; text-align: left; cursor: pointer;
  }
  :global(summary.card-head.toggle:hover .card-title) { color: var(--accent); }

  /* 目的グループの見出し。押しても見出し自身は動かず、中身がその下に生えるだけ */
  :global(summary.buff-group-head) {
    width: 100%; margin-top: 5px; padding: 5px 8px;
    display: flex; align-items: center; gap: 7px; color: var(--fg-sub);
    font-size: 10px; font-weight: 700; text-align: left; cursor: pointer;
  }
  :global(summary.buff-group-head:hover) { border-color: var(--accent); }
  :global(details[open] > summary.buff-group-head) { background: var(--sel-card); border-color: var(--sel-bd); color: var(--sel-fg); }
  .bg-label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* .bg-count は ui/Value.svelte が描く子要素。summary.buff-group-head も ui/Disclosure が描くので両方 :global */
  :global(summary.buff-group-head .bg-count) { flex: none; min-width: 5ch; text-align: right; font-size: 9px; font-weight: 500; }
  .calc-buff-set { margin-top: 8px; display: flex; align-items: flex-start; gap: 8px; font-size: 10px; color: var(--fg-muted); }
  .calc-buff-set > span { flex-shrink: 0; padding-top: 6px; }
  .calc-buff-set :global(.picker) { min-width: 0; flex: 1; }
  .buff-legend { margin: 7px 0 0; font-size: 9px; line-height: 1.7; }
  .buff-legend .lg {
    display: inline-block; padding: 0 5px; border-radius: var(--r-pill);
    font-size: 8.5px; font-weight: 700; border: 1px solid;
  }
  .buff-legend .lg.always { background: #CCF7FF; border-color: var(--sel-bd); color: var(--sel-fg); }
  .buff-legend .lg.extra { background: var(--state-temp-bg); border-color: var(--sim); color: var(--sim-fg); }
  .buff-note { margin: 8px 0 0; font-size: 9px; line-height: 1.6; }
  /* 値の調整。チップに重ねて出すので、ON にした数だけペインが伸びることがない。
     置き場所は app.css の .popover(「設定」の右端に揃えて下に開く) */
  :global(.buff-editor) { min-width: 210px; gap: 7px; }
  /* 値の行は「名前 + 欄」。名前は行が持つ(部品は読み上げ名だけを持つ・§07) */
  .stat-value-row { display: flex; align-items: center; gap: 3px; min-width: 0; }
  .stat-value-label {
    flex: none; min-width: 46px; margin-right: 5px; white-space: nowrap;
    font-size: 10.5px; font-weight: 700; color: var(--fg-muted);
  }
  /* 行の押せる面(.face)とは別の的。extra に置くので行そのものは押しても動かない */
  :global(.chip-config) {
    flex: none; margin: 0 2px 0 0; padding: 0 0 0 6px;
    border: 0; border-left: 1px solid var(--border-soft); background: none;
    color: var(--fg-muted); font: inherit; font-size: 8.5px; text-decoration: underline;
    text-underline-offset: 2px; cursor: pointer; opacity: .8;
  }
  :global(.chip-config:hover) { opacity: 1; color: var(--accent); }

</style>
