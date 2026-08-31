<script lang="ts">
  // ダメージ計算: v4 の縦フロー「相手を選ぶ → この一発 → もし〜だったら → なぜこの数字？」。
  // 右カラムは「計算の材料」(試し変更・バフ・入場条件)。計算はすべて Rust 側(preview_damage)。
  import { untrack } from "svelte";
  import {
    errorMessage, evaluateContents, listEnchantGains, listSkills, listUpgradeCandidates, previewDamage,
    previewDefense, previewEffectiveStats,
  } from "../../api/commands";
  import type {
    Adjustments, BuffChoice, BuffDefinition, BuffPurpose, CategoryTrace, ComboSkillType, ContentEvaluation, DamageCategory,
    DamageResult, DefenseProfile, EquipmentValues, FormulaStep, NewCharacter, PartSlot, Skill, StatKind,
    UltimateSkill, UpgradeCandidate,
  } from "../../api/types";
  import {
    BUFF_PURPOSES, isBlocked, isChoiceValue, isMultiTarget, isPercentLayer, isUserSelectedTarget,
    matchesPurpose,
    pickedStats, toggleBuff, toggleBuffStat, userInputRange,
  } from "../../buffs";
  import {
    enchantCap, enchantDepKeysFor, enchantRows as enchantRowsOf, ENCHANT_SLOT_LABELS, setEnchantValue,
    type EnchantDepKey,
  } from "../../enchant";
  import { selectedEquipmentPartOrNeutral } from "../../equipment";
  import { fmtInt, fmtNum, formatLayerValue, topRowsText } from "../../format";
  import {
    ELEMENT_LABELS, EQUIPMENT_STAT_LABELS, EQUIPMENT_STAT_SHORT, PART_SLOTS, STAT_KINDS, STAT_LABELS,
    STAT_LAYER_LABELS, ULTIMATE_SKILLS, ULTIMATE_SKILL_LABELS,
  } from "../../labels";
  import { limits } from "../../limits.svelte";
  import {
    app, enqueueCharacterSave, flatContents, focusCharacterSource, payloadOf, selectedCharacter, simIsDirty,
    upsertCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import CheckChip from "../../ui/CheckChip.svelte";
  import Icon from "../../ui/Icon.svelte";
  import DefensePanel from "./DefensePanel.svelte";
  import Select from "../../ui/Select.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import StepToggle from "../../ui/StepToggle.svelte";
  import { positionPopover } from "../../ui/popover";
  import SplitPage from "../../ui/SplitPage.svelte";
  import { latest } from "../../ui/latest.svelte";
  import { bump, flash } from "../../ui/motion.svelte";
  import { critChanceStage } from "../../ui/critChance";
  import { badgeStyle, REACH_BADGES, STATE, type Badge } from "../../ui/states";
  import StatInput from "../../ui/StatInput.svelte";
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
  let targetOpen = $state(false);
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
        );
        const saved = simActive
          ? await previewDamage(
              sp, sid, t.content.id, comboCount, JSON.parse(tempJson), comboType, JSON.parse(buffsJson),
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
  const need = $derived(target?.content.need_per_hit ?? 0);
  const ratio = $derived(perHit !== null && need > 0 ? perHit / need : 0);
  const hasReqs = $derived((target?.content.requirements.length ?? 0) > 0);
  // 評価が未取得の間は入場条件を「不明」として扱い、未達コンテンツに「通る/余裕」を
  // 出さない(ダメージ 120ms・評価 200ms のデバウンス差で毎回この窓が開く。PR レビュー指摘)
  const entryKnown = $derived(!hasReqs || targetEval !== null);
  const entryOk = $derived(!hasReqs || (targetEval?.entry_ok ?? false));
  const badgeState = $derived.by(() => {
    if (perHit === null || !entryKnown) return 6;
    if (hasReqs && !entryOk) return ratio >= 1 ? 5 : 4;
    return ratio >= 1.3 ? 0 : ratio >= 1 ? 1 : ratio >= 0.8 ? 2 : 3;
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
        pct: `${Math.max(1.5, (x.v / total) * 100).toFixed(2)}%`,
        share: `${Math.round((x.v / total) * 100)}%`,
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
  const FACTOR_STEPS = new Set(["スキル倍率", "クリティカル", "コンボ・属性・カット率・オーラ"]);
  const RUNNING_STEPS = new Set([
    "最終ダメージ固定値(下限)",
    "最終ダメージ・カット率A・被害減少",
    "各種ダメージ増減",
    "攻撃ダメージ・PVP補正",
  ]);
  const flowRows = $derived.by<FlowRow[]>(() => {
    if (pierced === null) return [];
    let running = pierced;
    const rows: FlowRow[] = [
      { k: "抜けた分(素通り)", add: pierced, mult: "—", factor: 1, c: "var(--fg-dim)", to: pierced, step: "攻撃力−防御力" },
    ];
    for (const s of steps) {
      if (!FACTOR_STEPS.has(s.name) && !RUNNING_STEPS.has(s.name)) continue;
      // 到達値は Rust の FormulaStep.reached。倍率列は倍率の段はその値、到達値で返る段は前段との比(表示用)
      const factor = FACTOR_STEPS.has(s.name) ? s.value : running > 0 ? s.reached / running : 1;
      const mult = FACTOR_STEPS.has(s.name) || running > 0 ? `×${factor.toFixed(2)}` : "—";
      rows.push({ k: s.name, add: s.reached - running, mult, factor, c: FLOW_COLORS[s.name] ?? "var(--fg-dim)", to: s.reached, step: s.name });
      running = s.reached;
    }
    return rows;
  });
  const flowTotal = $derived(flowRows.reduce((a, r) => a + Math.max(0, r.add), 0) || 1);
  const flowMultLabel = $derived(
    pierced !== null && pierced > 0 && perHit !== null ? `×${(perHit / pierced).toFixed(1)}` : "—",
  );
  let flowOpen = $state(false);

  // 倍率の材料(非中立カテゴリ)
  const activeCategories = $derived(
    (result?.trace.categories ?? []).filter((c) => c.kind !== "assigned" && c.value !== 0),
  );
  const catAtCap = (c: (typeof activeCategories)[number]) =>
    !!c.cap && c.cap.max !== null && c.value >= c.cap.max - 1e-9;
  const fmtCatValue = (c: (typeof activeCategories)[number]) =>
    c.kind === "rate" ? `${c.value >= 0 ? "+" : ""}${fmtNum(c.value * 100)}%` : fmtNum(c.value);
  /** 上限で捨てられた分(生の合算値 − 上限適用後)。0 なら捨てていない */
  const catLoss = (c: (typeof activeCategories)[number]) => c.raw - c.value;
  const fmtCatRaw = (c: (typeof activeCategories)[number]) =>
    c.kind === "rate" ? `${c.raw >= 0 ? "+" : ""}${fmtNum(c.raw * 100)}%` : fmtNum(c.raw);
  const fmtCatLoss = (c: (typeof activeCategories)[number]) => {
    const loss = catLoss(c);
    return c.kind === "rate" ? `${fmtNum(loss * 100)}%` : fmtNum(loss);
  };
  const cappedCategories = $derived(activeCategories.filter((c) => catLoss(c) > 1e-9));
  /**
   * 「一番効いている」は**プレイヤーが積み上げられるカテゴリ**の中から倍率で選ぶ。
   * - 段(スキル倍率・クリティカル)で比べると、スキル固有の値(D/F)が常勝して努力の範疇外になる
   * - 足した実数で比べると後段ほど大きな値に掛かり、最後の段が構造的に常勝する
   * (ユーザー指摘 2026-08-29)。代入(A〜D/F)・敵側(C/M/V1/Q/R/S/U/New2/V2)・PVP(Y)・
   * 子を持つ親(X)は候補から外す
   */
  const NOT_EFFORT = new Set<string>([
    "target_defense", "damage_reduction", "cut_rate_a", "damage_absorb", "taken_damage_rate",
    "taken_damage_reduction", "damage_resistance", "damage_mitigation", "cut_rate_b",
    "attack_damage_legacy", "attack_damage_rate", "pvp_correction",
  ]);
  const topLever = $derived(
    activeCategories
      .filter((c) => !NOT_EFFORT.has(c.category) && c.factor > 1)
      .sort((a, b) => b.factor - a.factor)[0] ?? null,
  );
  /**
   * 「ここを伸ばすのが効率いい」= 割合カテゴリに +1% 足したときの最終ダメージの伸び。
   * 同一カテゴリ内は加算なので伸びは `1 / factor`(いま積んでいる量が少ないほど大きい)。
   * 上限に達したカテゴリは伸びないので外す。候補はすでに積んでいる(供給源がある)ものだけ
   * (中立のカテゴリは全部 ×1.00 で並ぶので順位が付けられない)
   */
  const bestLevers = $derived(
    activeCategories
      .filter((c) => c.kind === "rate" && !NOT_EFFORT.has(c.category) && !catAtCap(c) && c.factor > 0)
      .sort((a, b) => a.factor - b.factor),
  );
  const bestLever = $derived(bestLevers[0] ?? null);
  /** 次の候補(2 位以降)。押した行の下に開く */
  const nextLevers = $derived(bestLevers.slice(1));
  let nextLeversOpen = $state(false);
  /** +1% 足したときの最終ダメージの伸び(%) */
  const leverGain = (c: CategoryTrace) => 1 / c.factor;
  const bestLeverGain = $derived(bestLever ? leverGain(bestLever) : 0);
  const fmtHeadroom = (c: CategoryTrace) =>
    c.cap && c.cap.max !== null ? `上限まで あと ${fmtNum((c.cap.max - c.value) * 100)}%` : "上限なし";
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
    /** `value` の数値。変わったら動かす(§00 04)ためだけに使う */
    n?: number;
    /** 押すと直下に `subs` が開く行。省略なら開かない行 */
    key?: string;
    subs?: Mat[];
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
  function toggleDetail(key: string) {
    openDetails = isDetailOpen(key) ? openDetails.filter((k) => k !== key) : [...openDetails, key];
  }
  const stepOf = (name: string): FormulaStep | null => steps.find((s) => s.name === name) ?? null;
  const categoryOf = (c: DamageCategory): CategoryTrace | null =>
    result?.trace.categories.find((x) => x.category === c) ?? null;
  /** カテゴリに実際に値を足した供給源(トレースの category_contributions から)。 */
  const catContributions = (c: string) =>
    (result?.trace.category_contributions ?? []).filter((x) => x.category === c);
  const fmtContributionValue = (kind: CategoryTrace["kind"], v: number) =>
    kind === "rate" ? `${v >= 0 ? "+" : ""}${fmtNum(v * 100)}%` : fmtNum(v);
  const catMat = (c: CategoryTrace): Mat => {
    // A 攻撃力は ① と同じ構成(ステ攻撃力 / 装備攻撃力 / 強化倍率)で開く。供給源 1 行では読めない
    if (c.category === "attack_power" && atkRows.length > 0) {
      return {
        label: `${c.symbol} ${c.label}`,
        value: fmtCatValue(c),
        key: "cat:attack_power",
        subs: atkRows.map((a) => ({
          label: a.k,
          value: fmtInt(Math.round(a.v)),
          sub: a.note,
          n: Math.round(a.v),
        })),
      };
    }
    const contributions = catContributions(c.category);
    return {
      label: `${c.symbol} ${c.label}`,
      mult: c.kind === "rate" ? `×${fmtNum(c.factor)}` : undefined,
      value: fmtCatValue(c),
      sub: catLoss(c) > 1e-9 ? `上限で −${fmtCatLoss(c)}` : undefined,
      key: contributions.length > 0 ? `cat:${c.category}` : undefined,
      subs:
        contributions.length > 0
          ? contributions.map((x) => ({
              label: x.source,
              value: fmtContributionValue(c.kind, x.value),
              n: x.value,
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
   * 実数(何ポイント動かしたか)は Rust の `StatContribution.effect`。UI で再計算しない。
   * 素ステ + Σ実数 − 捨てた分 = 最終能力値。
   */
  function statFactorMats(kind: StatKind): Mat[] {
    const st = result?.trace.stats.find((s) => s.kind === kind);
    if (!st) return [];
    const mats: Mat[] = [
      { label: "素ステ(振り分け)", value: fmtInt(st.base), n: st.base },
    ];
    for (const c of result?.trace.stat_contributions ?? []) {
      if (c.kind !== kind) continue;
      mats.push({
        label: c.source,
        value: `${c.effect < 0 ? "−" : "+"}${fmtInt(Math.abs(c.effect))}`,
        sub: `${STAT_LAYER_LABELS[c.layer]} ${formatLayerValue(c.layer, c.value)}`,
        n: c.effect,
      });
    }
    if (st.capped_loss > 0) {
      mats.push({
        label: "上限で捨てた分",
        value: `−${fmtInt(st.capped_loss)}`,
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
          mult: `+${fmtNum(s.value * 100)}%`,
          value: `+${fmtNum(s.value * 100)}%`,
          n: s.value,
        });
      }
    }
    return {
      mult: a.k === "装備攻撃力強化倍率" && atk ? `+${fmtNum(atk.enhance_rate * 100)}%` : "—",
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
      value: `${f.add < 0 ? "−" : "+"}${fmtInt(Math.round(Math.abs(f.add)))}`,
      sub: `到達 ${fmtInt(Math.round(f.to))}`,
    }));
    if (r.capped_loss.max > 0) {
      mats.push({
        label: "ダメージ上限(1 段ごと)",
        value: fmtInt(r.damage_cap),
        sub: `上限で −${fmtInt(r.capped_loss.max)}`,
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
      },
    ];
    if (r.weapon_added_per_hit !== 0) {
      mats.push({
        label: `武器強化(追加固定) ${fmtInt(r.weapon_added_per_hit)} × ${r.hit_count} 段`,
        mult: `×${r.hit_count}`,
        value: fmtInt(r.weapon_added_total),
        sub: "上限なし・表記ダメージとは別枠",
      });
    }
    if (added !== 0) {
      mats.push({
        label: "割合追加ダメージ(合計に乗る)",
        mult: `+${fmtNum(r.added_damage_rate * 100)}%`,
        value: fmtInt(added),
        sub: "シャープネスビジョン・ランダムOP・称号",
      });
    }
    if (!critMode) {
      mats.push({
        label: "クリティカルなら",
        mult: skill ? `×${fmtNum(skill.critical_multiplier)}` : undefined,
        value: fmtInt(r.total.critical),
      });
    }
    mats.push({ label: "乱数が最小のとき", value: fmtInt(r.total.min) });
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
      { label: "合計ダメージ", value: fmtInt(totalValue) },
      {
        label: "基本中ディレイ",
        value: `${d.base.toFixed(2)}s`,
        sub: d.fixed ? "固定(減少が効かない)" : undefined,
      },
    ];
    for (const c of d.contributions) {
      mats.push({ label: `↳ ${c.source}`, value: `−${(c.rate * 100).toFixed(0)}%` });
    }
    mats.push({
      label: `中ディレイ減少(上限 ${Math.round(limits.actual_delay_reduction_max * 100)}%)`,
      value: `${(d.reduction * 100).toFixed(0)}%`,
      sub: d.reduction_raw > d.reduction ? `選択中は ${(d.reduction_raw * 100).toFixed(0)}%` : undefined,
    });
    if (d.combo_rate < 1) {
      mats.push({ label: `コンボ(倍率A・${limits.combo_bonus_threshold} コンボ以上)`, mult: `×${fmtNum(d.combo_rate)}`, value: "" });
    }
    mats.push({
      label: "中ディレイ",
      value: `${d.value.toFixed(2)}s`,
      sub: d.floored ? `下限 ${limits.actual_delay_min.toFixed(1)}s で頭打ち` : undefined,
    });
    mats.push({
      label: "スキル回数",
      value: `${Math.round(d.uses_per_minute)} 回/分`,
      sub: d.uses_measured ? "実測表から" : "式 60 ÷ 中ディレイ",
    });
    if (r.expected_dps !== null && r.critical_chance > 0 && r.critical_chance < 1) {
      mats.push({
        label: "期待値(クリ率で按分)",
        value: fmtInt(Math.round(r.expected_dps)),
        sub: `合計(非クリ) × ${((1 - r.critical_chance) * 100).toFixed(1)}% + 合計(クリ) × ${(r.critical_chance * 100).toFixed(1)}%`,
      });
    }
    return {
      mult: `÷ ${d.value.toFixed(2)}s`,
      delta: null,
      to: Math.round(dpsValue),
      mats,
      idle: 0,
      expr: "1 秒あたり = 合計 × スキル回数(回/分) ÷ 60",
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
          k: `中ディレイ減少の上限(${Math.round(limits.actual_delay_reduction_max * 100)}%)`,
          raw: `${(ad.reduction_raw * 100).toFixed(0)}%`,
          val: `${(ad.reduction * 100).toFixed(0)}%`,
          loss: `${((ad.reduction_raw - ad.reduction) * 100).toFixed(0)}%`,
          kept: ad.reduction_raw > 0 ? ad.reduction / ad.reduction_raw : 1,
        });
      }
      if (ad.floored) {
        const want = ad.raw;
        out.push({
          k: `中ディレイの下限(${limits.actual_delay_min.toFixed(1)}s)`,
          raw: `${want.toFixed(2)}s`,
          val: `${ad.value.toFixed(2)}s`,
          loss: `${(ad.value - want).toFixed(2)}s ぶん遅い`,
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
      id: "enchant",
      label: () => "エンチャント",
      get: (p) => JSON.stringify(PART_SLOTS.map((s) => selectedEquipmentPartOrNeutral(p.equipment.parts[s]).enchant)),
      set: (p, v) => {
        const values = JSON.parse(v) as EquipmentValues[];
        PART_SLOTS.forEach((s, i) => {
          const list = p.equipment.parts[s];
          const part = list.registered.find((x) => x.id === list.selected_id);
          if (part) part.enchant = values[i];
        });
      },
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
      // 「次に変えるなら」の武器更新。基本値まで一緒に替わる 1 操作なので 1 チップで戻す。
      id: "weapon_item",
      label: (p) => {
        const weapon = weaponOf(p);
        const name = app.equipmentCatalog.find((i) => i.id === weapon.item_id)?.name
          ?? weapon.custom_name
          ?? "未装着";
        return `武器 ${name}`;
      },
      get: (p) => {
        const weapon = weaponOf(p);
        return JSON.stringify([weapon.item_id, weapon.custom_name, weapon.base]);
      },
      set: (p, v) => {
        const [itemId, customName, base] = JSON.parse(v);
        const weapon = weaponOf(p);
        weapon.item_id = itemId;
        weapon.custom_name = customName;
        weapon.base = base;
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
      id: "awakening",
      label: (p) => `覚醒 ${p.awakening.stage} / エタ Lv${p.awakening.eternal_level}`,
      get: (p) => JSON.stringify(p.awakening),
      set: (p, v) => (p.awakening = JSON.parse(v)),
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
  const BUFF_STATE_LABEL = { always: "常", extra: "追", off: "" } as const;
  const alwaysBuffCount = $derived(consumableBuffs.filter((d) => buffState(d) === "always").length);
  const extraBuffCount = $derived(consumableBuffs.filter((d) => buffState(d) === "extra").length);
  function chooseCalcBuffSet(value: string) {
    const id = value === "" ? null : Number(value);
    app.calcBuffSetId = id;
    const set = app.buffSets.find((item) => item.id === id);
    app.calcBuffs = JSON.parse(JSON.stringify(set?.choices ?? { choices: [] }));
  }
  /** 値の調整を開いているバフの id。高々 1 件で、重ねて出す(下に積むとペインが伸びる) */
  let buffEditorId = $state<string | null>(null);
  /** いま開いている目的グループ。null = 全部畳んである(既定)。同時に開くのは 1 つ */
  let openBuffPurpose = $state<BuffPurpose | null>(null);
  /** 「計算の材料」で開いているまとまり。null = 全部畳んである(既定)。
   *  実プレイでは一度に 1 つしか触らないので、開くのも 1 つに絞る — 全部開くと
   *  3512px(表示域の 4.6 画面ぶん)になり、目的のものまでスクロールで探すことになる */
  let openMaterial = $state<"ultimate" | "enchant" | "buffs" | null>(null);
  function toggleMaterial(id: "ultimate" | "enchant" | "buffs") {
    openMaterial = openMaterial === id ? null : id;
    if (openMaterial !== "buffs") openBuffPurpose = null;
  }
  function openBuffEditor(def: BuffDefinition) {
    buffEditorId = buffEditorId === def.id ? null : def.id;
  }
  /** 重なりものは外を押したときと Esc で閉じる。トリガ自身と中身は対象外 */
  function closeBuffEditor(event: MouseEvent) {
    if ((event.target as HTMLElement | null)?.closest(".popover, .chip-config")) return;
    buffEditorId = null;
  }

  function toggleBuffChip(def: BuffDefinition) {
    if (buffEditorId === def.id) buffEditorId = null;
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
  /** チップに併記する効果値。写経しない — Rust 側 preview_effective_stats(common_skill.ultimate)
   *  から引く。2 回の呼び出しで 3 種すべての効果値を取れる(枠は 2 つしかないので)。 */
  let ultimateEffects = $state<{
    critical_damage_rate: number; actual_delay_reduction: number; added_hit_count: number; skill_range_bonus: number;
  } | null>(null);
  const ultimateLatest = latest({ debounce: 150 });
  $effect(() => {
    const p = payload;
    if (!p) {
      ultimateLatest.cancel();
      ultimateEffects = null;
      return;
    }
    const pJson = JSON.stringify(p);
    const buffs = JSON.parse(JSON.stringify(app.calcBuffs));
    ultimateLatest.run(async (isCurrent) => {
      try {
        const withCombat = JSON.parse(pJson) as NewCharacter;
        withCombat.common_skills.ultimate.slots = ["scope_eye", "full_throttle"];
        const withRange = JSON.parse(pJson) as NewCharacter;
        withRange.common_skills.ultimate.slots = ["wide_focus", null];
        const [a, b] = await Promise.all([
          previewEffectiveStats(
            withCombat.base_stats, withCombat.stat_sources, withCombat.equipment, withCombat.common_skills,
            withCombat.awakening, withCombat.main_skill_id, buffs,
          ),
          previewEffectiveStats(
            withRange.base_stats, withRange.stat_sources, withRange.equipment, withRange.common_skills,
            withRange.awakening, withRange.main_skill_id, buffs,
          ),
        ]);
        if (isCurrent()) {
          ultimateEffects = {
            critical_damage_rate: a.common_skill.ultimate.critical_damage_rate,
            actual_delay_reduction: a.common_skill.ultimate.actual_delay_reduction,
            added_hit_count: a.common_skill.ultimate.added_hit_count,
            skill_range_bonus: b.common_skill.ultimate.skill_range_bonus,
          };
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
    if (skillId === "scope_eye") return `クリダメ +${Math.round(e.critical_damage_rate * 100)}%`;
    if (skillId === "full_throttle") {
      return `中ディレイ −${Math.round(e.actual_delay_reduction * 100)}% ・段数 +${e.added_hit_count}`;
    }
    return `範囲 +${Math.round(e.skill_range_bonus)}(火力には効きません)`;
  }

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
      .map((c) => ({ label: `${STAT_LABELS[c.kind]} ${c.effect > 0 ? "+" : ""}${fmtInt(c.effect)}`, value: c.effect }));
    const parts: string[] = [];
    if (statRows.length > 0) parts.push(topRowsText(statRows));
    for (const c of result?.trace.category_contributions ?? []) {
      if (c.source === def.name && c.value !== 0) {
        const cat = result?.trace.categories.find((x) => x.category === c.category);
        const label = cat ? `${cat.symbol}` : c.category;
        parts.push(`${label} ${c.value > 0 ? "+" : ""}${fmtNum(c.value * 100)}%`);
      }
    }
    return parts.join(" ・ ");
  }

</script>

<!-- 押した数値の内訳。押した行の直下にだけ開く(§00 03)。
     列は band-row と同じ段にそろえ、面はインセット = 読み取り専用(§02)。 -->
{#snippet detailBox(d: Detail)}
  <div class="detail open-in">
    <div class="dt-head">
      <span class="dt-hk dim">倍率</span>
      <span class="num dt-hv">{d.mult}</span>
      <span class="dt-hk dim">実数</span>
      <span class="num dt-hv" class:bad={(d.delta ?? 0) < 0}
      >{d.delta === null ? "—" : `${d.delta < 0 ? "−" : "+"}${fmtInt(Math.round(Math.abs(d.delta)))}`}</span>
      <span class="dt-hk dim">到達値</span>
      <span class="num dt-hv big" use:bump={() => d.to}
      >{d.to === null ? "—" : fmtInt(Math.round(d.to))}</span>
    </div>
    {#each d.mats as m, i (i)}
      {#if m.key}
        {@const key = m.key}
        <!-- 押すと要因の一覧が直下に開く(押した行は動かない) -->
        <button
          type="button" class="dt-row dt-row-btn"
          aria-expanded={isDetailOpen(key)} onclick={() => toggleDetail(key)}
        >
          <span class="dt-label">{m.label}</span>
          <span class="num dt-mult dim">{m.mult ?? ""}</span>
          <span class="num dt-val" use:bump={() => m.n ?? null}>{m.value}</span>
          <span class="num dt-sub dim">{m.sub ?? ""}</span>
        </button>
        {#if isDetailOpen(key)}
          <div class="dt-subs open-in">
            {#each m.subs ?? [] as sm, j (j)}
              <div class="dt-row">
                <span class="dt-label">{sm.label}</span>
                <span class="num dt-mult dim">{sm.mult ?? ""}</span>
                <span class="num dt-val" class:bad={(sm.n ?? 0) < 0} use:bump={() => sm.n ?? null}>{sm.value}</span>
                <span class="num dt-sub dim">{sm.sub ?? ""}</span>
              </div>
            {/each}
          </div>
        {/if}
      {:else}
        <div class="dt-row">
          <span class="dt-label">{m.label}</span>
          <span class="num dt-mult dim">{m.mult ?? ""}</span>
          <span class="num dt-val" use:bump={() => m.n ?? null}>{m.value}</span>
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
                {@const blocked = state === "off" && isBlocked(app.calcBuffs.choices, app.catalog, def)}
                {@const detail = state !== "off" && hasDetail(def)}
                <!-- 「設定」を独立した的にするため、チップ本体はネイティブ button ではなく
                     role="button" の div にする(button の中に button は入れられない)。
                     値の調整は**チップに重ねて**開く(§09 規則 3)— 下に積むと、ON にした数だけ
                     ペインが伸びる(実測: 詳細 11 件で 1330px)。 -->
                <div
                  class="buff-chip"
                  class:on={state !== "off"}
                  class:extra={state === "extra"}
                  class:disabled={blocked}
                  role="button"
                  tabindex={blocked ? -1 : 0}
                  aria-disabled={blocked}
                  aria-pressed={state !== "off"}
                  title={blocked ? "同枠の他バフと排他です" : def.note || undefined}
                  onclick={() => { if (!blocked) toggleBuffChip(def); }}
                  onkeydown={(e) => {
                    if ((e.key === "Enter" || e.key === " ") && !blocked) { e.preventDefault(); toggleBuffChip(def); }
                  }}
                >
                  <!-- アイコンは行内サイズ(20)。名前は必ず併記する(§08: アイコン単独表示は禁止)。
                       未収録の id は破線 + ? になり、その場でも幅は変わらない -->
                  <Icon kind="buff" id={def.id} size={20} label={def.name} />
                  <span class="buff-chip-copy">
                    <span>{def.name}</span>
                    <!-- ON にしたチップの実際の寄与(供給源ごとの実数)。写経しない -->
                    {#if state !== "off"}
                      {@const note = buffContributionText(def)}
                      {#if note}<span class="buff-chip-note dim" use:flash={() => note}>{note}</span>{/if}
                    {/if}
                  </span>
                  {#if detail}
                    <button
                      type="button"
                      class="chip-config"
                      onclick={(e) => { e.stopPropagation(); openBuffEditor(def); }}
                      aria-expanded={buffEditorId === def.id}
                      aria-label={`${def.name} の設定`}
                    >設定</button>
                  {/if}
                  <!-- 状態バッジの枠は常に確保する。付いた瞬間にチップが伸びると、
                       隣のチップが折り返して並びが動く(§09 規則 4) -->
                  <span class="chip-state" class:on={state !== "off"}
                  >{state !== "off" ? BUFF_STATE_LABEL[state] : ""}</span>
                  {#if detail && buffEditorId === def.id}
                    {@const choice = buffChoiceOf(def.id)}
                    {#if choice}
                      <div
                        class="popover buff-editor"
                        role="dialog"
                        tabindex="-1"
                        aria-label={`${def.name} の設定`}
                        use:positionPopover
                        onclick={(e) => e.stopPropagation()}
                        onkeydown={(e) => e.stopPropagation()}
                      >
                        {#if isMultiTarget(def.target)}
                          <!-- クラブエフェクトはステごとに 1 つずつ併用できる。ここでは対象ステの
                               出し入れだけを試せるようにし、値はバフタブ側の設定を引き継ぐ -->
                          <StepToggle
                            label="対象ステ"
                            options={statOptions}
                            max={STAT_KINDS.length}
                            values={pickedStats(app.calcBuffs.choices, def)}
                            onToggle={(v, next) => toggleBuffStatChip(def, v as StatKind, next)}
                          />
                        {:else if isUserSelectedTarget(def.target)}
                          <StepSelect
                            label="対象ステ"
                            options={statOptions}
                            bind:value={
                              () => choice.stat ?? STAT_KINDS[0],
                              (v) => editBuffChoice(def.id, (c) => (c.stat = v as StatKind))
                            }
                          />
                        {/if}
                        {#if isChoiceValue(def.value)}
                          {@const options = def.value.choice.map((v, i) => ({ value: String(i), label: formatLayerValue(def.layer, v) }))}
                          <Select
                            label="値"
                            {options}
                            bind:value={
                              () => String(choice.choice_index ?? 0),
                              (v) => editBuffChoice(def.id, (c) => (c.choice_index = Number(v)))
                            }
                          />
                        {/if}
                        {#if userInputRange(def.value)}
                          {@const range = userInputRange(def.value)!}
                          {@const scale = isPercentLayer(def.layer) ? 100 : 1}
                          {#if isMultiTarget(def.target)}
                            {#each pickedStats(app.calcBuffs.choices, def) as stat (stat)}
                              <StatInput
                                label={STAT_LABELS[stat]}
                                min={range.min * scale}
                                max={range.max * scale}
                                bind:value={
                                  () => (buffChoiceOfStat(def.id, stat)?.value ?? def.default_value ?? range.min) * scale,
                                  (v) => editBuffChoice(def.id, (c) => (c.value = v / scale), stat)
                                }
                              />
                            {/each}
                          {:else}
                            <StatInput
                              label={isPercentLayer(def.layer) ? "値 (%)" : "値"}
                              min={range.min * scale}
                              max={range.max * scale}
                              bind:value={
                                () => (choice.value ?? 0) * scale,
                                (v) => editBuffChoice(def.id, (c) => (c.value = v / scale))
                              }
                            />
                          {/if}
                        {/if}
                        <button type="button" class="popover-close" onclick={(e) => { e.stopPropagation(); buffEditorId = null; }}>閉じる</button>
                      </div>
                    {/if}
                  {/if}
                </div>
  {/snippet}

<svelte:window
  onclick={closeBuffEditor}
  onkeydown={(e) => { if (e.key === "Escape") buffEditorId = null; }}
/>

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
        <div class="side-tabs" role="tablist">
          <button
            type="button" class="side-tab" class:on={side === "attack"}
            role="tab" aria-selected={side === "attack"} onclick={() => (side = "attack")}
          >攻撃</button>
          <button
            type="button" class="side-tab" class:on={side === "defense"}
            role="tab" aria-selected={side === "defense"} onclick={() => (side = "defense")}
          >防御</button>
        </div>
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
            <button type="button" class="target-trigger" class:open={targetOpen} onclick={() => (targetOpen = !targetOpen)}>
              <span class="t-line1">
                <span class="t-name">{target.content.name}</span>
                <span class="t-chev" class:rot={targetOpen}>▼</span>
                <span class="t-index num dim">{targetIndex + 1} / {contents.length}</span>
              </span>
              <span class="t-line2">
                <span class="t-area dim">{target.areaName}</span>
                <span class="t-def num">防御 {defenseValue !== null ? fmtInt(defenseValue) : "—"}</span>
                <span class="t-need num">目安 {fmtInt(need)}</span>
              </span>
            </button>
            <button type="button" class="step" onclick={() => stepTarget(1)}>▶</button>
          </div>
          {#if targetOpen}
            <button type="button" class="overlay" aria-label="閉じる" onclick={() => (targetOpen = false)}></button>
            <div class="pop pop-in">
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
                      targetOpen = false;
                    }}
                  >
                    <span class="dot" style="background: {ev?.clear ? STATE.met.bd : ev?.entry_ok === false ? STATE.short.bd : STATE.unknown.bd};"></span>
                    {#if cov !== null}<span class="coverage">{cov}</span>{/if}
                    <span class="pop-name">{c.name}</span>
                    <span class="num dim">{ev?.damage ? fmtInt(ev.damage.per_hit_primary) : "—"}</span>
                  </button>
                {/each}
              {/each}
            </div>
          {/if}

          <!-- スキル行 -->
          <div class="skill-row">
            {#if skills.length === 0}
              <span class="dim">このキャラのスキルデータは未収録です(仮スキルはありません)。</span>
            {:else}
              <button type="button" class="skill-trigger" onclick={() => (skillOpen = !skillOpen)}>
                <span class="sk-line1">
                  <Icon kind="skill" id={skill?.id ?? null} size={20} label={skill?.name ?? "スキル"} />
                  <span class="sk-name">{skill?.name ?? ""}</span>
                  {#if skills.length > 1}<span class="t-chev" class:rot={skillOpen}>▼</span>{/if}
                  <!-- 主軸(キャラタブ)と違うスキルで計算している例外状態。保存されないので
                       ラベンダー(--sim)。行の高さは変えない -->
                  {#if skillOverridden}
                    <span class="sk-override badge-in" use:flash={() => skillId}>ここで上書き中</span>
                  {/if}
                </span>
                <span class="sk-meta num dim">
                  ×<span use:flash={() => result ? fmtNum(result.effective_skill_multiplier) : "—"}>{result ? fmtNum(result.effective_skill_multiplier) : "—"}</span>
                  ・ <span use:flash={() => result ? String(result.hit_count) : "—"}>{result?.hit_count ?? "—"}</span>段
                  ・ 中 <span use:flash={() => result?.effective_base_actual_delay != null ? `${fmtNum(result.effective_base_actual_delay)}s` : "?"}>{result?.effective_base_actual_delay != null ? `${fmtNum(result.effective_base_actual_delay)}s` : "?"}</span>
                  ・ Cri×{skill ? fmtNum(skill.critical_multiplier) : "—"}
                  {#if skill}・ {ELEMENT_LABELS[skill.element]}属性{/if}
                  {#if result?.accuracy_point != null}・ 命中P {fmtInt(result.accuracy_point)}{/if}
                </span>
              </button>
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
          {#if skillOpen && skills.length > 1}
            <button type="button" class="overlay" aria-label="閉じる" onclick={() => (skillOpen = false)}></button>
            <div class="pop gold pop-in">
              <div class="pop-head gold"><span>スキル {skills.length} 種 ／ この対象への合計ダメージ順</span></div>
              {#each pickerSkills as s (s.id)}
                {@const d = skillTotals[s.id]}
                <button
                  type="button"
                  class="pop-row"
                  class:on={s.id === skillId}
                  onclick={() => {
                    skillOverride = s.id;
                    skillOpen = false;
                  }}
                >
                  <Icon kind="skill" id={s.id} size={20} label={s.name} />
                  <span class="pop-name">{s.name}</span>
                  <span class="num dim">×{fmtNum(s.multiplier)} / {s.hit_count}段</span>
                  <span class="num strong">{d ? fmtInt(d.total) : "…"}</span>
                </button>
              {/each}
            </div>
          {/if}

          {#if skill && skill.combo_variants.length > 0}
            <div class="combo-type-row">
              <StepSelect
                label="コンボタイプ"
                options={COMBO_SKILL_TYPE_OPTIONS}
                full
                bind:value={
                  () => comboSkillType,
                  (v) => (comboSkillType = v as ComboSkillType)
                }
              />
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
                <span class="nl">1 発</span>
                <span class="hero-num num nv" use:bump={() => perHit}>{perHit !== null ? fmtInt(perHit) : "—"}</span>
                {#if simDirty}
                  <span class="nsub num">
                    <span class:up={deltaPct > 0} class:down={deltaPct < 0} use:bump={() => deltaPct}>
                      {deltaPct === 0 ? "±0%" : `${deltaPct > 0 ? "+" : ""}${deltaPct}%`}
                    </span>
                    ・ キャラ登録どおりなら {savedPerHit !== null ? fmtInt(savedPerHit) : "—"}
                  </span>
                {/if}
              </button>
              {#key badgeState}
                <span class="badge badge-in gatebadge" style={badgeStyle(BADGE[badgeState])}>{BADGE[badgeState].label}</span>
              {/key}
              <span class="op num">×<span use:bump={() => result?.hit_count ?? null}>{result?.hit_count ?? 1}</span> 段</span>
              <button
                type="button" class="node mid"
                aria-expanded={isDetailOpen("total")} onclick={() => toggleDetail("total")}
              >
                <span class="nl">合計</span>
                <span class="num nv" use:bump={() => totalValue}>{totalValue !== null ? fmtInt(totalValue) : "—"}</span>
                <span class="nsub num">
                  {#if result}
                    {#if result.critical_rate === null}
                      {#key "unrecorded"}
                        <span class="badge crit-badge" style={badgeStyle({ label: "", state: "unknown" })}>クリ率 未記載 → 確定扱い</span>
                      {/key}
                    {:else}
                      {@const stage = critChanceStage(result.critical_chance * 100)}
                      {#key stage.label}
                        <span class="badge crit-badge" style={badgeStyle({ label: "", state: stage.state })}
                        >{stage.label} {result.critical_rate.value.toFixed(1)}%</span>
                      {/key}
                    {/if}
                    {#if !critMode}・ クリなら ×{skill ? fmtNum(skill.critical_multiplier) : "—"} {fmtInt(result.total.critical)}{/if}
                  {/if}
                </span>
              </button>
              <span class="op num">÷ <span use:bump={() => result?.actual_delay?.value ?? null}>{result?.actual_delay ? result.actual_delay.value.toFixed(2) : "—"}</span>s</span>
              <button
                type="button" class="node rate"
                aria-expanded={isDetailOpen("dps")} onclick={() => toggleDetail("dps")}
              >
                <span class="nl">1 秒あたり</span>
                <span class="num nv" use:bump={() => dpsValue}>{dpsValue !== null ? fmtInt(Math.round(dpsValue)) : "—"}</span>
                <span class="nsub dim">
                  {#if result?.actual_delay}{Math.round(result.actual_delay.uses_per_minute)} 回/分 ・ {/if}{critMode ? "クリ確定" : "非クリ"}
                  {#if result && result.expected_dps !== null && result.critical_chance > 0 && result.critical_chance < 1}
                    ・ 期待値 <span class="num" use:bump={() => result?.expected_dps ?? null}>{fmtInt(Math.round(result.expected_dps))}</span>(クリ率 {(result.critical_chance * 100).toFixed(1)}%)
                  {/if}
                </span>
              </button>
              <!-- 討伐時間は敵 HP が gamedata に未収録(この画面に限らず全体で未実装)なので
                   ノードごと出さない(§00 02。0 や「—」で埋めると画面が嘘をつく) -->
            </div>
            <!-- 鎖の各数値の内訳。押した節は動かず、鎖の直下に増える(§00 03) -->
            {#if isDetailOpen("perHit") && perHitDetail}{@render detailBox(perHitDetail)}{/if}
            {#if isDetailOpen("total") && totalDetail}{@render detailBox(totalDetail)}{/if}
            {#if isDetailOpen("dps") && dpsDetail}{@render detailBox(dpsDetail)}{/if}
            <div class="meter big"><div class="fill" style="width: {Math.min(100, ratio * 100).toFixed(1)}%; background: {STATE[BADGE[badgeState].state].bar};"></div></div>
            <div class="hero-sentence">
              <span class="sentence" class:ok={ratio >= 1} class:ng={ratio < 1}>
                {#if perHit === null}
                  計算中…
                {:else if noPierce}
                  防御力を抜けていません(攻撃力 {atkA !== null ? fmtInt(atkA) : "—"} ≤ 防御力 {defenseValue !== null ? fmtInt(defenseValue) : "—"})
                {:else if ratio >= 1}
                  <!-- 桁が離れた倍率をそのまま出すと意味を成さない(ユーザー指摘)。一定倍率を
                       超えたら倍率を出さず「大きく超えている」とだけ伝える -->
                  {ratio >= 10 ? "目安を大きく超えています。" : `目安の ${ratio.toFixed(2)} 倍。火力は足りています。`}
                {:else}
                  目安まで あと {fmtInt(need - perHit)}(+{Math.max(1, Math.round((need / Math.max(perHit, 1) - 1) * 100))}% 必要)
                {/if}
              </span>
              <span class="num dim">目安 {fmtInt(need)}</span>
            </div>
            <!-- 足りない分をどう埋める? を 1 行に(旧: 紫のパネル)。候補が無い・すでに目安に
                 届いているときは行ごと消す(§00 02) -->
            {#if perHit !== null && ratio < 1 && whatIf.length > 0}
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
                  <span class="num fill-pct">({top.delta_pct > 0 ? "+" : ""}{top.delta_pct}%)</span>
                </button>
                {#if whatIf.length > 1}
                  <button
                    type="button" class="fill-more-toggle"
                    aria-expanded={fillMoreOpen} onclick={() => (fillMoreOpen = !fillMoreOpen)}
                  >{fillMoreOpen ? "▲" : `他 ${whatIf.length - 1} 件 ›`}</button>
                {/if}
              </div>
              {#if fillMoreOpen && whatIf.length > 1}
                <div class="fill-list open-in">
                  {#each whatIf.slice(1) as w (w.id)}
                    <button
                      type="button" class="fill-more-row"
                      class:whatif-leaving={leavingWhatIfId === w.id}
                      disabled={leavingWhatIfId === w.id}
                      onclick={() => applyWhatIf(w)}
                    >
                      <span class="dt-label">{w.label}</span>
                      <span class="num dt-val">{w.delta_pct > 0 ? "+" : ""}{w.delta_pct}%</span>
                    </button>
                  {/each}
                </div>
              {/if}
            {/if}
            {#if result?.actual_delay}
              {@const d = result.actual_delay}
              <div class="delay-note dim">
                中ディレイ {d.base.toFixed(2)}s
                {#if d.fixed}
                  ×(固定・減少が効かない)
                {:else if d.reduction > 0}
                  × (1 − {(d.reduction * 100).toFixed(0)}%){#if d.reduction_raw > d.reduction}<span class="warn"> ※減少値は上限 {Math.round(limits.actual_delay_reduction_max * 100)}%({(d.reduction_raw * 100).toFixed(0)}% ぶん選択中)</span>{/if}
                {/if}
                {#if d.combo_rate < 1}× {fmtNum(d.combo_rate)}(中ディレイ減少 {limits.combo_delay_threshold} コンボ以上){/if}
                = {d.value.toFixed(2)}s{#if d.floored}<span class="warn"> ※下限 {limits.actual_delay_min.toFixed(1)}s</span>{/if}
                {#if d.contributions.length > 0}
                  ／ 減少源: {d.contributions.map((c) => `${c.source} ${(c.rate * 100).toFixed(0)}%`).join(" ・ ")}
                {/if}
                <br />
                1 秒あたり = 合計 × {Math.round(d.uses_per_minute)} 回/分 ÷ 60
                {#if d.uses_measured}
                  (<b>実測表</b>: 総減少 {(d.reduction * 100).toFixed(0)}% × 基本 {d.base.toFixed(1)}s)
                {:else}
                  (実測表の範囲外なので 60 ÷ 中ディレイ の式で算出)
                {/if}
              </div>
            {/if}
            {#if result?.critical_rate}
              {@const c = result.critical_rate}
              <div class="delay-note dim">
                クリティカル率 (装備クリ補正 {fmtInt(c.equipment_critical)} + 1) × 2 × (AGI {fmtInt(c.agi)} / (AGI + 対象AGI {fmtInt(c.target_agi)}))
                {#if c.siena_rate > 0}× シエナのオーラ {(1 + c.siena_rate).toFixed(2)}{/if}
                = {c.from_agi.toFixed(1)}%
                ＋ スキル Cri値 {fmtInt(c.skill)}%{#if c.bonus > 0} ＋ 増加 {fmtInt(c.bonus)}%{/if}
                − 対象のクリティカル被撃率 {fmtInt(-c.target_taken_rate)}%
                = <b>{c.value.toFixed(1)}%</b>{#if c.raw < 0}<span class="warn"> ※下限 0%</span>{:else if c.raw > 100}<span class="warn"> ※上限 100%</span>{/if}
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
          <button type="button" class="panel-head blue" onclick={() => (flowOpen = !flowOpen)}>
            <span class="panel-title dark">なぜこの数字？</span>
            <span class="panel-note dark">{flowOpen ? "閉じる" : "内訳をひらく"}</span>
            <span class="t-chev" class:rot={flowOpen}>▼</span>
          </button>
          <div class="panel-body">
            <div class="flow-line">
              <span class="dim">抜けた分</span>
              <span class="num strong" use:bump={() => (pierced === null ? null : Math.max(0, Math.trunc(pierced)))}>{pierced !== null ? fmtInt(Math.max(0, Math.trunc(pierced))) : "—"}</span>
              <span class="arrow num dim">→</span>
              <span class="dim">倍率</span>
              <span class="num good strong">{flowMultLabel}</span>
              <span class="arrow num dim">→</span>
              <span class="num final" use:bump={() => perHit}>{perHit !== null ? fmtInt(perHit) : "—"}</span>
            </div>
            <div class="lever-note">
              {#if noPierce}
                攻撃力が相手の防御力に届いていないので、倍率は何もかかりません。まず攻撃力を上げる必要があります。
              {:else if topLever}
                いま一番効いている積み上げは「{topLever.symbol} {topLever.label}」の {fmtCatValue(topLever)}(×{fmtNum(topLever.factor)}){catAtCap(topLever) ? "。上限に達しています" : ""}。
                {#if bestLever}
                  <br />伸ばすなら「{bestLever.symbol} {bestLever.label}」。+1% ごとに最終ダメージが <span class="num" use:bump={() => bestLeverGain}>+{bestLeverGain.toFixed(2)}%</span> 伸びます({fmtHeadroom(bestLever)})。
                  {#if nextLevers.length > 0}
                    <button type="button" class="chip quiet" class:on={nextLeversOpen} aria-expanded={nextLeversOpen} onclick={() => (nextLeversOpen = !nextLeversOpen)}>
                      次の候補 {nextLevers.length}
                    </button>
                  {/if}
                {/if}
                {#if bestLever && nextLeversOpen && nextLevers.length > 0}
                  <!-- 次の候補。押した行は動かず、直下に増える(§00 03)。列は内訳と同じ段 -->
                  <div class="lever-list open-in">
                    {#each nextLevers as c, i (c.category)}
                      <div class="dt-row">
                        <span class="dt-label"><span class="dim">{i + 2}.</span> {c.symbol} {c.label}</span>
                        <span class="num dt-mult dim">{fmtCatValue(c)}</span>
                        <span class="num dt-val" use:bump={() => leverGain(c)}>+{leverGain(c).toFixed(2)}%</span>
                        <span class="num dt-sub dim">{fmtHeadroom(c)}</span>
                      </div>
                    {/each}
                    <p class="dt-note dim">+1% 足したときの最終ダメージの伸び。いま積んでいる量が少ないカテゴリほど 1% の価値が高い。</p>
                  </div>
                {/if}
              {:else}
                倍率はまだ何もかかっていません。
              {/if}
            </div>

            {#if flowOpen}
              <div class="open-in">
              <!-- ① 攻撃力をつくる -->
              <div class="stage">
                <span class="stage-no" style="background: var(--flow-1);">1</span>
                <span class="stage-title">攻撃力をつくる</span>
                <span class="num strong stage-val" use:bump={() => atkA}>{atkA !== null ? fmtInt(atkA) : "—"}</span>
              </div>
              <div class="band">
                {#each atkRows as a (a.k)}
                  <div style="width: {a.pct}; background: {a.c};"></div>
                {/each}
              </div>
              <div class="band-rows">
                {#each atkRows as a (a.k)}
                  <button
                    type="button" class="band-row"
                    aria-expanded={isDetailOpen(`atk:${a.k}`)} onclick={() => toggleDetail(`atk:${a.k}`)}
                  >
                    <span class="swatch" style="background: {a.c};"></span>
                    <span class="br-label">{a.k}</span>
                    <span class="br-note dim">{a.note}</span>
                    <span class="num br-val" use:bump={() => Math.round(a.v)}>{fmtInt(Math.round(a.v))}</span>
                    <span class="num br-share dim" use:bump={() => parseFloat(a.share)}>{a.share}</span>
                  </button>
                  {#if isDetailOpen(`atk:${a.k}`)}{@render detailBox(atkDetail(a))}{/if}
                {/each}
              </div>

              <!-- ② 防御力を抜く -->
              <div class="stage">
                <span class="stage-no" style="background: var(--danger);">2</span>
                <span class="stage-title">相手の防御力を抜く</span>
                <span class="num strong stage-val" use:bump={() => (pierced === null ? null : Math.max(0, Math.trunc(pierced)))}>{pierced !== null ? fmtInt(Math.max(0, Math.trunc(pierced))) : "—"}</span>
              </div>
              <div class="band">
                <div style="width: {(100 - defShare).toFixed(2)}%; background: var(--flow-pierce);"></div>
                <div style="width: {defShare.toFixed(2)}%; background: var(--hatch-lost);"></div>
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
                <span class="num strong stage-val" use:bump={() => perHit}>{perHit !== null ? fmtInt(perHit) : "—"}</span>
              </div>
              <div class="band">
                {#each flowRows.filter((r) => r.add > 0) as f (f.k)}
                  <div style="width: {((Math.max(0, f.add) / flowTotal) * 100).toFixed(2)}%; background: {f.c};"></div>
                {/each}
              </div>
              <div class="band-rows">
                {#each flowRows as f (f.k)}
                  <button
                    type="button" class="band-row"
                    aria-expanded={isDetailOpen(`flow:${f.k}`)} onclick={() => toggleDetail(`flow:${f.k}`)}
                  >
                    <span class="swatch" style="background: {f.c};"></span>
                    <span class="br-label" class:strong={topLeverStep === f.k} class:bad={f.add < 0}>{f.k}</span>
                    <span class="num br-mult dim">{f.mult}</span>
                    <span class="num br-val" class:bad={f.add < 0} use:bump={() => Math.round(Math.abs(f.add))}>{f.add < 0 ? "−" : "+"}{fmtInt(Math.round(Math.abs(f.add)))}</span>
                    <span class="num br-share dim" use:bump={() => Math.round((Math.abs(f.add) / flowTotal) * 100)}>{Math.round((Math.abs(f.add) / flowTotal) * 100)}%</span>
                  </button>
                  {#if isDetailOpen(`flow:${f.k}`)}{@render detailBox(stepDetail(f.step, f.mult, f.add, f.to))}{/if}
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
                      <div class="lost-row">
                        <span class="lost-label">{r.k}</span>
                        <span class="num lost-raw" use:flash={() => r.raw}>{r.raw}</span>
                        <span class="lost-arrow dim">→ 上限</span>
                        <span class="num lost-val" use:flash={() => r.val}>{r.val}</span>
                        <span class="lost-bar" aria-hidden="true">
                          <i style="width: {(r.kept * 100).toFixed(1)}%"></i>
                          <i class="cut" style="width: {(100 - r.kept * 100).toFixed(1)}%"></i>
                        </span>
                        <span class="num lost-loss" use:flash={() => r.loss}>{r.loss} は無効</span>
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
                    <span class="mat-chip" class:cap={catAtCap(c)} use:flash={() => (catAtCap(c) ? "cap" : "open")}>
                      <span class="dim">{c.label}</span>
                      <span class="num strong" use:flash={() => fmtCatValue(c)}>{fmtCatValue(c)}</span>
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
            {/if}
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
            >{simDirty ? (deltaPct === 0 ? "±0%" : `${deltaPct > 0 ? "+" : ""}${deltaPct}%`) : ""}</span>
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

        <!-- 差分チップも同じ理由で高さを固定する。3 件までなので 1 行に収め、
             溢れたら横にスクロールさせる(行が増えて下をずらさない) -->
        <div class="chips">
          {#each changedKnobs as k (k.id)}
            <span class="chip-diff badge-in" use:flash={() => k.get(app.sim!)}>
              <span>{k.label(app.sim!)}</span>
              <button type="button" class="chip-x" title="この変更だけ戻す" onclick={() => revertKnob(k)}>✕</button>
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
        <div class="card">
          <button type="button" class="card-head toggle" aria-expanded={openMaterial === "ultimate"} onclick={() => toggleMaterial("ultimate")}>
            <span class="bg-caret" aria-hidden="true">{openMaterial === "ultimate" ? "▾" : "▸"}</span>
            <!-- 見出しの顔。中身を代表する 1 つを置く(名前と併記なので §08 の単独表示にあたらない)。
                 画像が未収録なら破線 + ? になるだけで、幅は変わらない -->
            <Icon kind="skill" id="scope_eye" size={20} label="極限スキル" />
            <span class="card-title">極限スキル</span>
            <span class="dim small num" use:bump={() => ultimatePickedCount}>{ultimatePickedCount} / {ultimateSlotCount}</span>
          </button>
          {#if openMaterial === "ultimate"}
          <div class="ultimate-chips">
            {#each ULTIMATE_SKILLS as u (u)}
              {@const on = payload.common_skills.ultimate.slots.includes(u)}
              <button
                type="button" class="ultimate-chip" class:on
                disabled={!on && ultimateFull}
                title={!on && ultimateFull ? `${ultimateSlotCount} 枠まで選べます。ほかを外してから選んでください。` : undefined}
                onclick={() => toggleUltimate(u)}
              >
                <Icon kind="skill" id={u} size={20} label={ULTIMATE_SKILL_LABELS[u]} />
                <span class="uc-name">{ULTIMATE_SKILL_LABELS[u]}</span>
                <span class="uc-note dim" use:flash={() => ultimateChipNote(u)}>{ultimateChipNote(u)}</span>
              </button>
            {/each}
          </div>
          {#if ultimateFull}
            <p class="eq-note dim badge-in">{ultimateSlotCount} 枠まで選べます。ほかを外してから選んでください。</p>
          {/if}
          <p class="eq-note dim">スーパーリミット・ハイパーリミットの Lv は<b>キャラ</b>タブ(共通スキル)の設定を使います。</p>
          {/if}
        </div>

        <!-- エンチャントの伸びしろ(試し変更)。選択中スキルの依存ステだけを部位横断で見る -->
        <div class="card">
          <button type="button" class="card-head toggle" aria-expanded={openMaterial === "enchant"} onclick={() => toggleMaterial("enchant")}>
            <span class="bg-caret" aria-hidden="true">{openMaterial === "enchant" ? "▾" : "▸"}</span>
            <!-- †エクリプスウィング(体)。エンチャントは装備を伸ばす話なので、その顔として置く -->
            <Icon kind="equipment" id="wiki-af444f9bf21d" size={20} label="エンチャントの伸びしろ" />
            <span class="card-title">エンチャントの伸びしろ</span>
            <!-- 見出しの注記は件数 1 つだけ。アイコンぶん幅が減っていて、主軸を並べると
                 右端の件数が切れる(§00 05: 読めない文字は出さない)。主軸は中に出す -->
            <span class="dim small num" use:bump={() => visibleEnchantRows.length}>{visibleEnchantRows.length} 件</span>
          </button>
          {#if openMaterial === "enchant"}
            <p class="enchant-dep dim">主軸: {enchantDepKeys.map((k) => EQUIPMENT_STAT_SHORT[k]).join("・") || "—"}</p>
          {#if visibleEnchantRows.length === 0}
            <p class="eq-note dim">主軸スキルの依存ステを盛れる部位がないか、すでに上限です。</p>
          {:else}
            <div class="enchant-rows">
              {#each visibleEnchantRows as { row, keys } (row.slot)}
                <div class="enchant-row">
                  <span class="enchant-row-label">{ENCHANT_SLOT_LABELS[row.slot]}</span>
                  {#if row.capUnknown}
                    <button type="button" class="enchant-cap-unknown" onclick={() => focusCharacterSource("equipment", row.slot)}>
                      <span class="coverage" title="カタログ外(カスタム名)装備でエンチャント上限が未入力です">上限未入力</span>
                      <span class="chev dim">›</span>
                    </button>
                  {:else}
                  <div class="enchant-row-cols">
                    {#each keys as k (k)}
                      {@const cap = enchantCap(row.part, k, app.equipmentCatalog) ?? 0}
                      {@const gain = enchantGains[`${row.slot}:${k}`]}
                      <div class="enchant-stat">
                        <span class="enchant-stat-label">{EQUIPMENT_STAT_SHORT[k]}</span>
                        <StatInput
                          label=""
                          min={0}
                          max={cap}
                          strictMax
                          bind:value={
                            () => row.part.enchant[k],
                            (v) => editSim((p) => setEnchantValue(p.equipment, row.slot, k, v))
                          }
                        />
                        <span class="num enchant-gain" class:up={(gain ?? 0) > 0} use:bump={() => gain ?? null}
                        >{gain !== undefined ? `MAX で +${gain}%` : ""}</span>
                      </div>
                    {/each}
                  </div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
          {/if}
        </div>

        <!-- バフ -->
        <div class="card">
          <button type="button" class="card-head toggle" aria-expanded={openMaterial === "buffs"} onclick={() => toggleMaterial("buffs")}>
            <span class="bg-caret" aria-hidden="true">{openMaterial === "buffs" ? "▾" : "▸"}</span>
            <Icon kind="buff" id="illumination_drink" size={20} label="バフ" />
            <span class="card-title">バフ</span>
            <span class="dim small num" use:bump={() => alwaysBuffCount + extraBuffCount}>{alwaysBuffCount + extraBuffCount} 件</span>
          </button>
          {#if openMaterial === "buffs"}
          <label class="calc-buff-set">
            <span>使うセット</span>
            <select value={app.calcBuffSetId ?? ""} onchange={(e) => chooseCalcBuffSet(e.currentTarget.value)}>
              <option value="">なし</option>
              {#each app.buffSets as set (set.id)}<option value={set.id}>{set.name}</option>{/each}
            </select>
          </label>
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
              <button
                type="button"
                class="buff-group-head"
                class:open={openBuffPurpose === purpose.id}
                aria-expanded={openBuffPurpose === purpose.id}
                onclick={() => (openBuffPurpose = openBuffPurpose === purpose.id ? null : purpose.id)}
              >
                <span class="bg-caret" aria-hidden="true">{openBuffPurpose === purpose.id ? "▾" : "▸"}</span>
                <span class="bg-label">{purpose.label}</span>
                <span class="bg-count num" use:bump={() => picked}>{picked}/{defs.length}</span>
              </button>
              {#if openBuffPurpose === purpose.id}
                <div class="buff-chips">
                  {#each defs as def (def.id)}{@render buffChip(def)}{/each}
                </div>
              {/if}
            {/if}
          {/each}
          <p class="buff-note dim">変更はこの計算だけに反映され、バフセットやキャラには保存されません。</p>
          {/if}
        </div>

        <!-- コンボ -->
        <div class="combo">
          <CheckChip checked={combo} onCheckedChange={(v) => (combo = v)}>
            <span>{limits.combo_bonus_threshold} コンボ以上(ダメージ +{Math.round(limits.combo_bonus_rate * 100)}%)</span>
          </CheckChip>
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
  .target-trigger { min-width: 0; flex: 1; padding: 3px 8px; border-radius: var(--r-panel); border: 1px solid transparent; text-align: left; }
  .target-trigger:hover, .target-trigger.open { background: var(--bg-rail); border-color: #9FB4D0; }
  .t-line1 { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  .t-name { min-width: 0; font-size: 15px; font-weight: 800; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .t-chev { flex-shrink: 0; font-size: 8.5px; color: var(--fg-muted); transition: transform 0.18s; }
  .t-chev.rot { transform: rotate(180deg); }
  .t-index { flex-shrink: 0; margin-left: auto; font-size: 8.5px; }
  .t-line2 { margin-top: 1px; display: flex; align-items: baseline; gap: 9px; min-width: 0; }
  .t-area { min-width: 0; font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .t-def { flex-shrink: 0; font-size: 8.5px; color: var(--danger); }
  .t-need { flex-shrink: 0; font-size: 8.5px; color: var(--fg-sub); }

  .overlay { position: fixed; inset: 0; z-index: 40; cursor: default; }
  .pop {
    position: absolute; left: 10px; right: 10px; top: 88px; z-index: 41;
    max-height: 262px; overflow-y: auto; overscroll-behavior: contain;
    border-radius: var(--r-window); background: var(--bg-field); border: 1px solid var(--sel-bd);
    box-shadow: 0 10px 24px rgba(30, 44, 74, 0.3), inset 0 0 0 1px #fff;
  }
  .pop.gold { border-color: #A9821F; box-shadow: 0 10px 24px rgba(74, 60, 18, 0.28), inset 0 0 0 1px #fff; }
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
  /* .coverage は app.css(§14 決定 5)。この画面だけ詰めた padding にする */
  .coverage { padding: 0 6px; }
  .pop-name { min-width: 0; flex: 1; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pop-row.on .pop-name { font-weight: 700; }
  .pop-row .strong { font-weight: 700; }

  .skill-row { position: relative; z-index: 2; padding: 8px 11px 0; display: flex; align-items: center; gap: 7px; }
  .skill-trigger {
    min-width: 0; flex: 1; display: flex; flex-direction: column; align-items: stretch; gap: 1px;
    padding: 5px 9px; border-radius: var(--r-panel); background: #F4F9FE; border: 1px solid #D6E2F0; text-align: left;
  }
  .skill-trigger:hover { background: var(--bg-rail); }
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
    background: var(--surface-inset); border: 1px solid var(--border-strong); border-radius: var(--r-inset);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.65);
  }
  .combo-type-note { min-width: 0; padding-bottom: 3px; font-size: 9px; line-height: 1.45; }
  @media (max-width: 720px) {
    .combo-type-row { grid-template-columns: minmax(0, 1fr); align-items: stretch; }
  }

  .hero { padding: 11px 13px 12px; }
  .hero-num { font-size: var(--t-result); line-height: 1; font-weight: var(--w-strong); }
  .chain .nsub .up { color: var(--good); font-weight: 700; }
  .chain .nsub .down { color: var(--danger); font-weight: 700; }
  .meter.big { margin-top: 10px; height: 12px; border-radius: var(--r-inset); }
  .hero-sentence { margin-top: 7px; display: flex; align-items: baseline; gap: 9px; min-width: 0; }
  .sentence { min-width: 0; flex: 1; font-size: 11px; font-weight: 700; text-wrap: pretty; }
  .sentence.ok { color: var(--good); }
  .sentence.ng { color: var(--danger); }
  .hero-sentence .num { flex-shrink: 0; font-size: 9.5px; }
  /* 鎖(§14 決定 1)。44px の主役数値は増やさない — 金の帯 = 答えは 1 つ(§02)を
     壊さず、鎖が右に伸びるだけ。狭いときは折り返す(桁で隣が動かないよう各段に min-width) */
  .chain { display: flex; align-items: flex-end; gap: 10px 11px; flex-wrap: wrap; }
  .chain .node { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .chain .nl { font-size: 9px; letter-spacing: 0.06em; color: var(--fg-muted); white-space: nowrap; }
  .chain .nv { font-weight: 700; color: var(--fg); white-space: nowrap; }
  .chain .node.gate .nv { min-width: 120px; }
  .chain .node.mid .nv { font-size: 15px; min-width: 68px; }
  .chain .node.rate .nv { font-size: 19px; min-width: 68px; }
  .chain .nsub { font-size: 9px; color: var(--fg-dim); white-space: nowrap; }
  .chain .op { font-size: 12px; color: var(--fg-dim); padding-bottom: 3px; white-space: nowrap; }
  .chain .gatebadge { align-self: flex-end; margin-bottom: 3px; }
  /* 押すと内訳が鎖の直下に開く。hover は塗りだけで、余白を足して隣を動かさない(§00 03) */
  .chain button.node { text-align: left; border-radius: var(--r-inset); }
  .chain button.node:hover { background: var(--bg-active); box-shadow: 0 0 0 4px var(--bg-active); }
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
  .fill-line { margin-top: 8px; display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .fill-btn {
    min-width: 0; flex: 1; display: flex; align-items: baseline; gap: 6px; padding: 3px 6px;
    border-radius: var(--r-inset); text-align: left; font-size: 10.5px;
  }
  .fill-btn:hover:not(:disabled) { background: var(--state-temp-bg); }
  .fill-label { min-width: 0; flex-shrink: 1; font-weight: 700; color: var(--sim-fg); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .fill-pct { flex-shrink: 0; font-weight: 700; color: var(--good); }
  .fill-more-toggle { flex-shrink: 0; padding: 2px 6px; border-radius: var(--r-inset); font-size: 9px; color: var(--fg-muted); }
  .fill-more-toggle:hover { color: var(--fg); background: var(--bg-active); }
  .fill-list {
    margin-top: 4px; padding: 5px 7px; display: flex; flex-direction: column; gap: 2px;
    border-radius: var(--r-inset); background: var(--surface-inset); border: 1px solid var(--border-strong);
  }
  .fill-more-row { width: 100%; display: flex; align-items: center; gap: 8px; padding: 2px 3px; border-radius: var(--r-inset); text-align: left; }
  .fill-more-row:hover:not(:disabled) { background: var(--bg-active); }

  .flow-line { display: flex; align-items: center; gap: 7px; flex-wrap: wrap; font-size: 9px; }
  .flow-line .strong { font-size: 13px; font-weight: 700; color: var(--fg-sub); }
  .flow-line .good.strong { color: var(--flow-3); }
  .flow-line .final { font-size: 15px; font-weight: 700; color: var(--fg); }
  .lever-note .chip { margin-left: 6px; vertical-align: middle; }
  .lever-list {
    margin-top: 6px; padding: 6px 8px; display: flex; flex-direction: column; gap: 3px;
    border-radius: var(--r-inset); background: var(--surface-inset); border: 1px solid var(--border-strong);
    box-shadow: inset 0 1px 0 rgba(255,255,255,0.6);
  }
  .lever-note {
    margin-top: 9px; padding: 8px 10px; border-radius: var(--r-panel);
    background: #F4F9FE; border: 1px solid var(--border-soft);
    font-size: var(--t-label); font-weight: 500; line-height: 1.6; color: var(--fg-sub); text-wrap: pretty;
  }

  .stage { margin-top: 14px; padding-top: 12px; border-top: 1px dashed var(--border-soft); display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .stage-no { flex-shrink: 0; width: 15px; height: 15px; border-radius: 50%; color: #fff; font-size: 9px; line-height: 16px; text-align: center; font-family: var(--font-num); font-variant-numeric: tabular-nums; font-weight: 700; }
  .stage-title { font-size: 11px; font-weight: 700; white-space: nowrap; }
  .stage-note { min-width: 0; flex: 1; font-size: 9px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .stage-val { margin-left: auto; font-size: 15px; font-weight: 700; }
  .band { margin-top: 7px; display: flex; height: 11px; border-radius: var(--r-inset); overflow: hidden; border: 1px solid var(--border-soft); background: #EDF2F9; }
  .band > div { flex-shrink: 0; transition: width 0.5s ease; }
  .band-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 5px; }
  .band-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .swatch { flex-shrink: 0; width: 8px; height: 8px; border-radius: var(--r-inset); }
  .br-label { min-width: 0; flex: 1; font-size: var(--t-label); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .br-label.strong { font-weight: 700; }
  .br-label.bad { color: var(--danger); }
  .br-note { min-width: 0; flex: 1.2; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .br-mult { flex-shrink: 0; width: 48px; text-align: right; font-size: 10px; }
  .br-val { flex-shrink: 0; width: 64px; text-align: right; font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .br-val.bad { color: var(--danger); }
  .br-share { flex-shrink: 0; width: 32px; text-align: right; font-size: 9.5px; }
  /* 構成行・段フローの行は押すと内訳が直下に開く。行の位置・高さは変わらない */
  button.band-row { width: 100%; text-align: left; border-radius: var(--r-inset); }
  button.band-row:hover { background: var(--bg-active); box-shadow: 0 0 0 3px var(--bg-active); }
  button.band-row:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  /* 押した数値の内訳。読み取り専用なのでインセット面、列は band-row と同じ段にそろえる */
  .detail {
    margin: 6px 0 2px; padding: 7px 9px; display: flex; flex-direction: column; gap: 4px;
    border-radius: var(--r-inset); background: var(--surface-inset); border: 1px solid var(--border-strong);
    box-shadow: inset 0 1px 0 rgba(255,255,255,0.6);
  }
  .dt-head { display: flex; align-items: baseline; gap: 6px 7px; flex-wrap: wrap; }
  .dt-hk { font-size: 9px; letter-spacing: 0.06em; }
  .dt-hv { min-width: 62px; font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .dt-hv.big { font-size: 13px; color: var(--fg); }
  .dt-hv.bad { color: var(--danger); }
  .dt-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .dt-label { min-width: 0; flex: 1; font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dt-mult { flex-shrink: 0; width: 48px; text-align: right; font-size: 9.5px; }
  .dt-val { flex-shrink: 0; width: 64px; text-align: right; font-size: 10px; font-weight: 700; color: var(--fg-sub); }
  .dt-sub { flex-shrink: 0; width: 112px; text-align: right; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dt-val.bad { color: var(--danger); }
  /* ステの行は押すと要因が直下に開く。行の位置・高さは変わらない */
  button.dt-row-btn {
    width: 100%; text-align: left; padding: 1px 3px; margin: 0 -3px;
    border: 0; background: none; color: inherit; font: inherit; border-radius: var(--r-inset);
  }
  button.dt-row-btn:hover { background: var(--bg-active); }
  button.dt-row-btn:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
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
    padding: 4px 9px; border-radius: var(--r-inset);
    background: var(--surface-inset); border: 1px solid var(--border-soft);
  }
  .lost-label { min-width: 0; flex: 1; font-size: 10px; font-weight: 700; color: var(--fg-head); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .lost-raw { flex-shrink: 0; min-width: 62px; text-align: right; font-size: 10px; color: var(--fg-muted); text-decoration: line-through; }
  .lost-arrow { flex-shrink: 0; font-size: 9px; }
  .lost-val { flex-shrink: 0; min-width: 62px; text-align: right; font-size: 10px; font-weight: 700; }
  .lost-bar { flex-shrink: 0; width: 96px; height: 7px; display: flex; border-radius: var(--r-inset); overflow: hidden; border: 1px solid var(--border-soft); }
  .lost-bar > i { display: block; background: var(--flow-1); }
  .lost-bar > i.cut { background: var(--hatch-lost); }
  .lost-loss { flex-shrink: 0; min-width: 104px; text-align: right; font-size: 10px; color: var(--danger); }
  .lost-none, .lost-note { margin: 4px 0 0; font-size: 10px; line-height: 1.6; }
  .mat-chip .strong { font-size: 10px; font-weight: 700; }
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
  .chips {
    display: flex; flex-wrap: nowrap; gap: 5px; min-height: 24px;
    overflow-x: auto; overscroll-behavior-x: contain;
  }
  .chip-diff {
    display: inline-flex; align-items: center; gap: 7px; padding: 3px 4px 3px 9px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--sim); box-shadow: 0 1px 0 rgba(109, 106, 168, 0.25);
    font-size: 10px; font-weight: 500;
  }
  .chip-x {
    width: 16px; height: 16px; display: flex; align-items: center; justify-content: center;
    border-radius: 50%; background: var(--state-temp-bg); font-size: 9px; color: var(--sim);
  }
  .chip-x:hover { background: var(--sim); color: #fff; }
  /* 上限の注記。色ではなく文言で伝える(ラベンダーに 2 つ目の意味を持たせない。§14 決定 6) */
  .sim-limit { min-height: 15px; padding: 0 11px 7px; font-size: 9.5px; color: var(--fg-dim); }
  .sim-limit.hit { color: var(--fg); font-weight: 700; }

  /* .card-head / .small は app.css */

  .eq-note { margin: 8px 0 0; font-size: 9.5px; line-height: 1.6; }

  /* 極限スキル(2 枠の選択チップ)。§07 形態 3: 押して入れる / 押して外す */
  .ultimate-chips { margin-top: 8px; display: flex; flex-direction: column; gap: 6px; }
  .ultimate-chip {
    width: 100%; display: flex; align-items: center; gap: 8px; padding: 6px 10px;
    border-radius: var(--r-panel); background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .ultimate-chip:hover:not(:disabled) { border-color: var(--accent); }
  .ultimate-chip:disabled { opacity: 0.5; }
  .ultimate-chip.on { background: var(--sel); border-color: var(--sel-bd); box-shadow: inset 0 0 0 1px var(--sel-bd); }
  .uc-name { flex-shrink: 0; font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .ultimate-chip.on .uc-name { color: var(--sel-fg); }
  .uc-note { min-width: 0; flex: 1; text-align: right; font-size: 9.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* エンチャントの伸びしろ。部位ごとに 現在/上限/MAX での伸び幅を横並びで見せる */
  .enchant-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 6px; }
  .enchant-row { display: flex; flex-direction: column; gap: 4px; }
  .enchant-row-label { font-size: 10px; font-weight: 700; color: var(--fg-muted); }
  .enchant-row-cols { display: flex; flex-direction: column; gap: 5px; }
  .enchant-stat { display: flex; align-items: center; gap: 7px; }
  .enchant-stat-label { flex-shrink: 0; width: 30px; font-size: 9.5px; color: var(--fg-muted); }
  .enchant-gain { flex-shrink: 0; min-width: 84px; text-align: right; font-size: 9.5px; color: var(--fg-dim); }
  .enchant-gain.up { color: var(--good); font-weight: 700; }
  /* 上限が未収録(カタログ外で実測上限も未入力)の行。押すとキャラタブの装備編集へ */
  .enchant-cap-unknown {
    display: flex; align-items: center; gap: 8px; padding: 2px 0; background: none; border: none;
    cursor: pointer; text-align: left; width: 100%;
  }

  .side-tabs { display: flex; gap: 6px; margin-bottom: 9px; }
  .side-tab {
    padding: 6px 18px; border-radius: var(--r-panel);
    background: linear-gradient(180deg, #fff, #E9F1FB); border: 1px solid var(--border-strong);
    font-size: 11.5px; font-weight: 700; color: #2B3C57;
  }
  .side-tab:hover:not(.on) { border-color: var(--accent); }
  .side-tab.on {
    background: var(--sel-card); border-color: var(--accent); color: var(--sel-fg);
    box-shadow: inset 0 1px 0 #fff;
  }

  .mat-note { margin: 7px 0 0; font-size: 9px; line-height: 1.6; }

  /* 35 個を pill で流すと幅がまちまちで、251px の器では 31 行中 26 行が 1 個だけだった
     (実測)。横に並ぶ利点が出ないうえ、名前の頭が縦に揃わず探しにくい。**1 列の行**にして、
     頭を揃える。丸(--r-pill)は「小さな状態の印」に使う形なので、行にはインセットの角丸 */
  .buff-chips {
    /* 1 行 = アイコン 20 と、名前 + 寄与の 2 行(13 + 1 + 12)のうち高いほう + 余白と枠線 */
    --buff-text-h: 26px;
    --buff-row-h: calc(var(--buff-text-h) + 8px + 2px);
    margin-top: 5px; margin-bottom: 3px; display: flex; flex-direction: column; gap: 3px;
  }
  .enchant-dep { margin: 6px 0 0; font-size: 9px; }
  /* カード見出しの開閉。押しても見出し自身は動かず、中身がその下に生えるだけ */
  .card-head.toggle {
    width: 100%; padding: 0; border: 0; background: none; text-align: left; cursor: pointer;
  }
  .card-head.toggle:hover .card-title { color: var(--accent); }

  /* 目的グループの見出し。押しても見出し自身は動かず、中身がその下に生えるだけ */
  .buff-group-head {
    width: 100%; margin-top: 5px; padding: 5px 8px;
    display: flex; align-items: center; gap: 7px;
    border: 1px solid var(--border-soft); border-radius: var(--r-inset);
    background: var(--surface-inset); color: var(--fg-sub);
    font-size: 10px; font-weight: 700; text-align: left; cursor: pointer;
  }
  .buff-group-head:hover { border-color: var(--accent); }
  .buff-group-head.open { background: var(--sel-card); border-color: var(--sel-bd); color: var(--sel-fg); }
  /* 開閉の印は幅を固定する。▸ と ▾ で幅が変わると隣のラベルが動く(§09 規則 1) */
  .bg-caret { flex: none; width: 10px; text-align: center; font-size: 9px; }
  .bg-label { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .bg-count { flex: none; min-width: 5ch; text-align: right; font-size: 9px; font-weight: 500; }
  .calc-buff-set { margin-top: 8px; display: flex; align-items: center; gap: 8px; font-size: 10px; color: var(--fg-muted); }
  .calc-buff-set select { min-width: 160px; height: 28px; border: 1px solid var(--border); border-radius: var(--r-inset); background: var(--bg-field); color: var(--fg); }
  .buff-chip {
    /* 重なりもの(.buff-editor)の位置の基準。チップ自身は動かさない */
    position: relative;
    /* 行の高さは中身より先に決める(§09 規則 4)。ON になると寄与の行が増えるが、
       枠は 2 行ぶん取ってあるので下の行は動かない */
    width: 100%; height: var(--buff-row-h); display: flex; align-items: center; gap: 7px;
    padding: 4px 8px; border-radius: var(--r-inset);
    background: var(--bg-field); border: 1px solid var(--border-soft);
    font-size: 10px; font-weight: 500; color: var(--fg-muted); text-align: left; cursor: pointer;
  }
  .buff-chip:hover:not(.disabled) { border-color: var(--accent); }
  .buff-chip-copy { flex: 1; min-width: 0; height: var(--buff-text-h); display: flex; flex-direction: column; justify-content: center; gap: 1px; overflow: hidden; }
  .buff-chip-copy > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* ON にしたチップの実際の寄与。写経しない(previewDamage のトレースから引く) */
  .buff-chip-note { font-size: 8.5px; font-weight: 700; }
  .buff-chip.on {
    background: var(--sel);
    border-color: var(--sel-bd); color: var(--sel-fg); font-weight: 700;
  }
  /* 追加枠は「保存されない」ので、その専用色(--sim)にそろえる */
  .buff-chip.on.extra {
    background: linear-gradient(180deg, #fff, var(--state-temp-bg));
    border-color: var(--sim); color: var(--sim-fg);
  }
  .buff-chip .chip-state {
    display: inline-block; min-width: 15px; text-align: center;
    margin-left: 5px; padding: 0 5px; border-radius: var(--r-pill);
    background: transparent; border: 1px solid transparent;
    font-size: 8.5px; font-weight: 700; color: transparent;
  }
  .buff-chip .chip-state.on {
    background: rgba(255, 255, 255, 0.75); border-color: currentColor; color: inherit;
  }
  .buff-legend { margin: 7px 0 0; font-size: 9px; line-height: 1.7; }
  .buff-legend .lg {
    display: inline-block; padding: 0 5px; border-radius: var(--r-pill);
    font-size: 8.5px; font-weight: 700; border: 1px solid;
  }
  .buff-legend .lg.always { background: #CCF7FF; border-color: var(--sel-bd); color: var(--sel-fg); }
  .buff-legend .lg.extra { background: var(--state-temp-bg); border-color: var(--sim); color: var(--sim-fg); }
  .buff-note { margin: 8px 0 0; font-size: 9px; line-height: 1.6; }
  /* 値の調整。チップに重ねて出すので、ON にした数だけペインが伸びることがない */
  .buff-editor { top: calc(100% + 4px); left: 0; min-width: 210px; gap: 7px; }
  /* チップ本体(押すと ON/OFF)とは別の的。縦の区切りで「ここだけ別」と分かるようにする */
  .chip-config {
    flex: none; margin-left: 6px; padding: 0 0 0 6px;
    border: 0; border-left: 1px solid currentColor; background: none;
    color: inherit; font: inherit; font-size: 8.5px; text-decoration: underline;
    text-underline-offset: 2px; cursor: pointer; opacity: .8;
  }
  .chip-config:hover { opacity: 1; }
  .buff-chip.disabled { opacity: .45; cursor: default; }

</style>
