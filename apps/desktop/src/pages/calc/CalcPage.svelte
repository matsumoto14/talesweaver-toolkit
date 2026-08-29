<script lang="ts">
  // ダメージ計算: v4 の縦フロー「相手を選ぶ → この一発 → もし〜だったら → なぜこの数字？」。
  // 右カラムは「計算の材料」(試し変更・バフ・入場条件)。計算はすべて Rust 側(preview_damage)。
  import { untrack } from "svelte";
  import {
    errorMessage, evaluateContents, listSkills, previewDamage, previewDefense, updateCharacter,
  } from "../../api/commands";
  import type {
    Adjustments, BuffChoice, BuffDefinition, CategoryTrace, ComboSkillType, ContentEvaluation, DamageCategory,
    DamageResult, DefenseProfile, FormulaStep, NewCharacter, Skill, StatKind,
  } from "../../api/types";
  import {
    isBlocked, isChoiceValue, isFixedValue, isPercentLayer, isUserSelectedTarget,
    toggleBuff, userInputRange,
  } from "../../buffs";
  import { candidatesFor, COST_COLORS, COST_LABELS, tryCandidates, type Candidate } from "../../candidates";
  import { selectedEquipmentPartOrNeutral } from "../../equipment";
  import { fmtInt, fmtNum, formatLayerValue } from "../../format";
  import {
    ELEMENT_LABELS, EQUIPMENT_STAT_LABELS, STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS,
  } from "../../labels";
  import { limits } from "../../limits.svelte";
  import {
    app, flatContents, payloadOf, selectedCharacter, totalContents as totalContentsCount, upsertCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import CheckChip from "../../ui/CheckChip.svelte";
  import Icon from "../../ui/Icon.svelte";
  import DefensePanel from "./DefensePanel.svelte";
  import RequirementList from "../../ui/RequirementList.svelte";
  import Select from "../../ui/Select.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import SplitPage from "../../ui/SplitPage.svelte";
  import { latest } from "../../ui/latest.svelte";
  import { bump, flash } from "../../ui/motion.svelte";
  import { critChanceStage } from "../../ui/critChance";
  import { badgeStyle, REACH_BADGES, STATE, triadStyle, type Badge } from "../../ui/states";
  import StatInput from "../../ui/StatInput.svelte";
  import TracePanel from "./TracePanel.svelte";

  const DEFAULT_RIGHT_WIDTH = 380;

  const COMBO_THRESHOLD = 3;
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
    const temp = JSON.parse(JSON.stringify(temporaryAdjustments)) as Adjustments;
    const contentId = target.content.id;
    const comboCount = combo ? COMBO_THRESHOLD : 0;
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
  // 計算リクエストにのみ乗せ、「キャラに保存」の対象にしない(旧仕様を踏襲。
  // sim に混ぜると「もしステ+50なら」が保存で永続化されてしまう。PR レビュー指摘)。
  const neutralAdjustments = (): Adjustments =>
    Object.fromEntries(STAT_KINDS.map((k) => [k, { add: 0, pin: null }])) as Adjustments;
  let temporaryAdjustments = $state<Adjustments>(neutralAdjustments());
  const hasTemporaryAdjustments = $derived(
    STAT_KINDS.some((k) => temporaryAdjustments[k].add !== 0 || temporaryAdjustments[k].pin !== null),
  );
  // キャラを切り替えたら一時調整をリセット(前のキャラの調整を引き継がない)
  let lastCharacterId = untrack(() => character?.id);
  $effect(() => {
    const id = character?.id;
    if (id === lastCharacterId) return;
    lastCharacterId = id;
    temporaryAdjustments = neutralAdjustments();
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
    const comboCount = combo ? COMBO_THRESHOLD : 0;
    const comboType = selectedComboSkillType;
    const simActive = app.sim !== null;
    const tempJson = JSON.stringify(temporaryAdjustments);
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
  const clearCount = $derived(evals.filter((e) => e.clear).length);

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
  const hasEquipmentReq = $derived(
    target?.content.requirements.some((r) => "equipment_by_skill" in r) ?? false,
  );
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
      { k: "装備攻撃力", v: atk.equipment_base_attack + atk.equipment_enhanced_attack, c: "var(--flow-1)", note: "基本/強化 × 依存別係数" },
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
    return { mult: flowMultLabel, delta: perHit - pierced, to: perHit, mats, idle: 0, expr: null };
  });
  /** 鎖「合計」: 1 発 × 段数 ＋ 割合追加ダメージ。クリ率は段階表示で読む */
  const totalDetail = $derived.by<Detail | null>(() => {
    const r = result;
    if (r === null || perHit === null || totalValue === null) return null;
    const added = pick(r.added_damage) ?? 0;
    const mats: Mat[] = [
      {
        label: `1 発 ${fmtInt(perHit)} × ${r.hit_count} 段`,
        mult: `×${r.hit_count}`,
        value: fmtInt(totalValue - added),
      },
    ];
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
      label: "中ディレイ減少(上限 70%)",
      value: `${(d.reduction * 100).toFixed(0)}%`,
      sub: d.reduction_raw > d.reduction ? `選択中は ${(d.reduction_raw * 100).toFixed(0)}%` : undefined,
    });
    if (d.combo_rate < 1) {
      mats.push({ label: `コンボ(倍率A・${COMBO_THRESHOLD} コンボ以上)`, mult: `×${fmtNum(d.combo_rate)}`, value: "" });
    }
    mats.push({
      label: "中ディレイ",
      value: `${d.value.toFixed(2)}s`,
      sub: d.floored ? "下限 0.3s で頭打ち" : undefined,
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
    // 中ディレイ。減少値の上限(70%)と秒そのものの下限(0.3s)は別の捨て方なので分けて出す
    const ad = result?.actual_delay ?? null;
    if (ad !== null) {
      if (ad.reduction_raw > ad.reduction + 1e-9) {
        out.push({
          k: "中ディレイ減少の上限(70%)",
          raw: `${(ad.reduction_raw * 100).toFixed(0)}%`,
          val: `${(ad.reduction * 100).toFixed(0)}%`,
          loss: `${((ad.reduction_raw - ad.reduction) * 100).toFixed(0)}%`,
          kept: ad.reduction_raw > 0 ? ad.reduction / ad.reduction_raw : 1,
        });
      }
      if (ad.floored) {
        const want = ad.raw;
        out.push({
          k: "中ディレイの下限(0.3s)",
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
  const simActive = $derived(app.sim !== null);
  const simDirty = $derived(
    app.sim !== null && savedPayload !== null && JSON.stringify(app.sim) !== JSON.stringify(savedPayload),
  );
  function resetSim() {
    app.sim = null;
    simLimited = false;
  }
  let saving = $state(false);
  async function saveSim() {
    if (!character || !app.sim) return;
    saving = true;
    try {
      const saved = await updateCharacter(character.id, JSON.parse(JSON.stringify(app.sim)));
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
      id: "weapon_enchant_thrust",
      label: (p) => `武器エンチャント 突き ${fmtInt(weaponOf(p).enchant.thrust)}`,
      get: (p) => String(weaponOf(p).enchant.thrust),
      set: (p, v) => (weaponOf(p).enchant.thrust = Number(v)),
    },
    {
      id: "weapon_enchant_slash",
      label: (p) => `武器エンチャント 斬り ${fmtInt(weaponOf(p).enchant.slash)}`,
      get: (p) => String(weaponOf(p).enchant.slash),
      set: (p, v) => (weaponOf(p).enchant.slash = Number(v)),
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
  interface WhatIf {
    candidate: Candidate;
    perHit: number;
    deltaPct: number;
  }
  let whatIf = $state<WhatIf[]>([]);
  /** 押した候補は、移動先の差分チップと同時に短く退出させる(§10「移った」)。 */
  let leavingWhatIfId = $state<string | null>(null);
  /** 試した候補の数。0 件のときに「候補が無い」のか「超えるものが無い」のかを書き分ける */
  let whatIfTried = $state(0);
  const whatIfLatest = latest({ debounce: 250 });
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null;
    const tempJson = JSON.stringify(temporaryAdjustments);
    const t = target;
    const sid = skillId;
    const base = perHit;
    const comboCount = combo ? COMBO_THRESHOLD : 0;
    const comboType = selectedComboSkillType;
    const buffsJson = JSON.stringify(app.calcBuffs);
    if (!pJson || !t || !sid || base === null) {
      whatIfLatest.cancel();
      whatIf = [];
      whatIfTried = 0;
      return;
    }
    whatIfLatest.run(async (isCurrent) => {
      try {
        const current = JSON.parse(pJson) as NewCharacter;
        const list = candidatesFor(current, app.equipmentCatalog);
        const rs = await tryCandidates(
          list,
          () => JSON.parse(pJson) as NewCharacter,
          (p) => previewDamage(
            p, sid, t.content.id, comboCount, JSON.parse(tempJson), comboType, JSON.parse(buffsJson),
          ),
          base,
          (w) => w.perHit > base,
        );
        if (isCurrent()) {
          whatIf = rs;
          whatIfTried = list.length;
          leavingWhatIfId = null;
        }
      } catch (e) {
        if (isCurrent()) reportError(errorMessage(e));
      }
    });
    return () => whatIfLatest.cancel();
  });
  function applyWhatIf(w: WhatIf) {
    leavingWhatIfId = w.candidate.id;
    editSim((p) => w.candidate.apply(p));
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
  function toggleBuffChip(def: BuffDefinition) {
    app.calcBuffs = { choices: toggleBuff(app.calcBuffs.choices, def, !buffOn(def)) };
  }
  // ON のバフのうち、対象ステ・効果量の選択肢・手入力を持つものの詳細編集(試し変更として反映)
  const statOptions = STAT_KINDS.map((k) => ({ value: k, label: STAT_LABELS[k] }));
  const buffChoiceOf = (buffId: string) =>
    app.calcBuffs.choices.find((c) => c.buff_id === buffId) ?? null;
  const hasDetail = (def: BuffDefinition) =>
    isUserSelectedTarget(def.target) || isChoiceValue(def.value) || userInputRange(def.value) !== null || isFixedValue(def.value);
  function editBuffChoice(buffId: string, fn: (c: BuffChoice) => void) {
    const choices = app.calcBuffs.choices.map((choice) => ({ ...choice }));
    const choice = choices.find((item) => item.buff_id === buffId);
    if (choice) fn(choice);
    app.calcBuffs = { choices };
  }
  const strongWeaponOptions = $derived([
    { value: "0", label: "なし" },
    ...Array.from({ length: limits.strong_weapon_level_max }, (_, i) => {
      const percent = Math.round((i + 1) * limits.strong_weapon_rate_per_level * 100);
      return { value: String(i + 1), label: `Lv${i + 1}(+${percent}%)` };
    }),
  ]);
  // 武器固有の固定エンチャント枠。実物の装備本体補正には左右されない。
  const weaponEnchantCaps = $derived.by(() => {
    const weapon = payload ? weaponOf(payload) : null;
    const item = weapon?.item_id ? app.equipmentCatalog.find((i) => i.id === weapon.item_id) : null;
    return {
      thrust: item?.enchant_caps.thrust ?? limits.equipment_value_max,
      slash: item?.enchant_caps.slash ?? limits.equipment_value_max,
    };
  });

  // 「通るのは」の分母は入場条件だけを見る全コンテンツ数(他画面と統一)。
  // 上の contents(対象ピッカー用)は敵データを持つものだけに絞っているため数え方が違う
  const totalContents = $derived(totalContentsCount());
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

<SplitPage
  midTitle="行ける？"
  midNote="→ 足りない分をどう埋める？ → なぜこの数字？"
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
        <SheetCard tone="gold" title="行ける？" note={character.name + (calculating ? " ・ 計算中…" : "")}>
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
                {#if simActive}
                  <span class="nsub num">
                    <span class:up={deltaPct > 0} class:down={deltaPct < 0} use:bump={() => deltaPct}>
                      {deltaPct === 0 ? "±0%" : `${deltaPct > 0 ? "+" : ""}${deltaPct}%`}
                    </span>
                    ・ 登録どおりなら {savedPerHit !== null ? fmtInt(savedPerHit) : "—"}
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
                  ・ 判定は付けない
                </span>
              </button>
              <span class="op">→</span>
              <!-- 討伐時間。敵 HP を gamedata に持っていないので破線 +「—」で出す。
                   0 で埋めると画面が嘘をつく(§00 欠けを正常な状態として見せる) -->
              <div class="node pending">
                <span class="nl">討伐時間</span>
                <span class="num nv">— 秒</span>
                <span class="nsub dim">敵 HP が未収録</span>
              </div>
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
                  目安の {ratio.toFixed(2)} 倍。火力は足りています。
                {:else}
                  目安まで あと {fmtInt(need - perHit)}(+{Math.max(1, Math.round((need / Math.max(perHit, 1) - 1) * 100))}% 必要)
                {/if}
              </span>
              <span class="num dim">目安 {fmtInt(need)}</span>
            </div>
            {#if result?.actual_delay}
              {@const d = result.actual_delay}
              <div class="delay-note dim">
                中ディレイ {d.base.toFixed(2)}s
                {#if d.fixed}
                  ×(固定・減少が効かない)
                {:else if d.reduction > 0}
                  × (1 − {(d.reduction * 100).toFixed(0)}%){#if d.reduction_raw > d.reduction}<span class="warn"> ※減少値は上限 70%({(d.reduction_raw * 100).toFixed(0)}% ぶん選択中)</span>{/if}
                {/if}
                {#if d.combo_rate < 1}× 0.5(2 コンボ以上){/if}
                = {d.value.toFixed(2)}s{#if d.floored}<span class="warn"> ※下限 0.3s</span>{/if}
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

        <!-- もし〜だったら -->
        <div class="panel purple">
          <div class="panel-head purple">
            <span class="panel-title">足りない分をどう埋める？</span>
            <span class="panel-note">押すと試し変更に入ります(保存されません)</span>
          </div>
          <div class="panel-body">
            {#if whatIf.length === 0}
              <p class="wi-empty dim">
                {whatIfTried === 0
                  ? "いま変えられる場所がありません。共通スキル・エンチャントはすでに上限です。"
                  : `${whatIfTried} 件ためしましたが、どれもいまの数字を超えませんでした。`}
              </p>
            {/if}
            {#each whatIf as w (w.candidate.id)}
              <button
                type="button"
                class="whatif"
                class:whatif-leaving={leavingWhatIfId === w.candidate.id}
                disabled={leavingWhatIfId === w.candidate.id}
                onclick={() => applyWhatIf(w)}
              >
                <span class="wi-main">
                  <span class="wi-label">{w.candidate.label}</span>
                  <span class="cost" style={triadStyle(COST_COLORS[w.candidate.cost])}>{COST_LABELS[w.candidate.cost]}</span>
                </span>
                <span class="wi-nums">
                  <span class="num wi-pct">+{w.deltaPct}%</span>
                  <span class="num dim">{fmtInt(w.perHit)}</span>
                </span>
              </button>
            {/each}
          </div>
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
            <span class="sim-title">{simDirty ? "試し変更中" : "登録どおり"}</span>
            <!-- 差分の枠も常に確保する。出た瞬間に行が 1px 伸びて下がずれる(§09 規則 4) -->
            <span class="num sim-delta" class:on={simActive} class:up={deltaPct > 0} class:down={deltaPct < 0}
            >{simActive ? (deltaPct === 0 ? "±0%" : `${deltaPct > 0 ? "+" : ""}${deltaPct}%`) : ""}</span>
          </div>
          <div class="sim-note-text dim">
            {simDirty
              ? "保存されていません。チップの ✕ で1つずつ戻せます。"
              : "下の材料を変えると、その場で数字が動きます(保存されません)。"}
          </div>
          <!-- 試し変更に入っても下がずれないよう、操作の枠は常に確保する(§09 規則 1・4)。
               ここは押した材料(装備・バフ)より**上**にあるので、差し込むと押したものが流れる -->
          <div class="sim-actions">
            <button type="button" class="btn" disabled={!simDirty} onclick={resetSim}>ぜんぶ戻す</button>
            <button type="button" class="btn primary" disabled={!simDirty || saving} onclick={saveSim}>{saving ? "保存中…" : "キャラに保存"}</button>
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
        <!-- 上限の注記は固定領域。行を差し込んで下をずらさない(§07) -->
        <div class="sim-limit" class:hit={simLimited}>
          {#if simLimited}
            試し変更は同時 {SIM_LIMIT} 件までです。どれかを ✕ で戻すか、「キャラに保存」で確定してください。
          {:else}
            同時に試せるのは {changedKnobs.length} / {SIM_LIMIT} 件です。
          {/if}
        </div>

        <!-- 装備(試し変更) -->
        <div class="card">
          <div class="card-head">
            <span class="card-title">装備</span>
            <span class="dim small">変更は試し変更として反映</span>
          </div>
          <div class="pw">
            <CheckChip
              checked={payload.common_skills.power_weapon}
              onCheckedChange={(v) => editSim((p) => (p.common_skills.power_weapon = v))}
            >
              <span>パワーウェポン(+2%)</span>
            </CheckChip>
          </div>
          <div class="sw">
            <StepSelect
              label="ストロングウェポン"
              options={strongWeaponOptions}
              bind:value={
                () => String(payload.common_skills.strong_weapon_level),
                (v) => editSim((p) => (p.common_skills.strong_weapon_level = Number(v)))
              }
            />
          </div>
          <details class="eq-details">
            <summary>武器のエンチャント(突き・斬り)</summary>
            <div class="eq-grid">
              <StatInput
                label={EQUIPMENT_STAT_LABELS.thrust}
                min={0}
                max={weaponEnchantCaps.thrust}
                strictMax
                bind:value={
                  () => weaponOf(payload).enchant.thrust,
                  (v) => editSim((p) => (weaponOf(p).enchant.thrust = v))
                }
              />
              <StatInput
                label={EQUIPMENT_STAT_LABELS.slash}
                min={0}
                max={weaponEnchantCaps.slash}
                strictMax
                bind:value={
                  () => weaponOf(payload).enchant.slash,
                  (v) => editSim((p) => (weaponOf(p).enchant.slash = v))
                }
              />
            </div>
            <p class="eq-note dim">武器のアイテム変更・基本能力値・強化 Lv・アビリティ・他の部位は<b>キャラ</b>タブで編集します。</p>
          </details>
        </div>

        <!-- バフ -->
        <div class="card">
          <div class="card-head">
            <span class="card-title">バフ</span>
            <span class="dim small">押した瞬間に数字が動きます</span>
          </div>
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
          <div class="buff-chips">
            {#each consumableBuffs as def (def.id)}
              {@const state = buffState(def)}
              {@const blocked = state === "off" && isBlocked(app.calcBuffs.choices, app.catalog, def)}
              <button
                type="button"
                class="buff-chip"
                class:on={state !== "off"}
                class:extra={state === "extra"}
                disabled={blocked}
                title={blocked ? "同枠の他バフと排他です" : def.note || undefined}
                onclick={() => toggleBuffChip(def)}
              >
                <span>{def.name}</span>
                <!-- 状態バッジの枠は常に確保する。付いた瞬間にチップが伸びると、
                     隣のチップが折り返して並びが動く(§09 規則 4) -->
                <span class="chip-state" class:on={state !== "off"}
                >{state !== "off" ? BUFF_STATE_LABEL[state] : ""}</span>
              </button>
            {/each}
          </div>
          {#each consumableBuffs.filter((d) => buffOn(d) && hasDetail(d)) as def (def.id)}
            {@const choice = buffChoiceOf(def.id)}
            {#if choice}
              <div class="buff-detail">
                <Icon kind="buff" id={def.id} size={20} label={def.name} />
                <span class="bd-name">{def.name}</span>
                {#if isUserSelectedTarget(def.target)}
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
                {#if isFixedValue(def.value)}
                  {@const fixedLabel = formatLayerValue(def.layer, def.value.fixed)}
                  <span class="dim bd-fixed" use:flash={() => fixedLabel}>値: {fixedLabel}</span>
                {/if}
              </div>
            {/if}
          {/each}
          <p class="buff-note dim">変更はこの計算だけに反映され、バフセットやキャラには保存されません。</p>
        </div>

        <!-- 調整(一時) -->
        <details class="card adj">
          <summary class="card-title">
            調整(一時) — 「もしステが +50 なら」{hasTemporaryAdjustments ? " ・ 調整あり" : ""}
          </summary>
          <p class="adj-note dim">計算にのみ反映されます。「キャラに保存」には含まれません(保存する調整はキャラタブで)。</p>
          <AdjustmentEditor
            adjustments={temporaryAdjustments}
            addMin={limits.adjustment_add_min}
            addMax={limits.adjustment_add_max}
            pinMin={limits.adjustment_pin_min}
            pinMax={limits.adjustment_pin_max}
            pinDefault={(k) => result?.trace.stats.find((s) => s.kind === k)?.effective ?? payload.base_stats[k]}
          />
        </details>

        <!-- コンボ -->
        <div class="combo">
          <CheckChip checked={combo} onCheckedChange={(v) => (combo = v)}>
            <span>{COMBO_THRESHOLD} コンボ以上(+15%)</span>
          </CheckChip>
        </div>

        <!-- 入場条件 -->
        {#if target}
          <div class="card">
            <div class="card-head">
              <span class="card-title">入場条件</span>
              <span class="dim small">通るのは {clearCount} / {totalContents}</span>
            </div>
            {#if target.content.requirements.length === 0}
              <p class="buff-note dim">{target.content.name} に入場条件はありません。</p>
            {:else if targetEval}
              <RequirementList checks={targetEval.checks} />
              {#if hasEquipmentReq}
                <!-- 装備条件は突き/斬り/魔攻/魔防/複合の別条件で、使うスキルの依存で比較先が決まる -->
                <p class="buff-note dim">
                  装備条件は選択中のスキル{skill ? `「${skill.name}」` : ""}の依存で判定しています(突き / 斬り / 魔攻 / 魔防 / 複合のいずれか 1 つを満たせば OK)。
                </p>
              {/if}
            {/if}
            {#if target.content.entry_note}
              <p class="entry-note">{target.content.entry_note}</p>
            {/if}
            {#if target.content.team_note}
              <p class="buff-note dim">チーム条件: {target.content.team_note}</p>
            {/if}
          </div>
        {/if}
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
    border-radius: var(--r-window); background: var(--bg-field); border: 1px solid #687287;
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
  /* まだデータが来ていない段。0 で埋めず、破線で「無い」と見せる(§00) */
  /* まだデータが来ていない段は 1 行に畳む(§00 触らない場所は 1 行に)。
     破線 = 「無い」の記号。0 で埋めない */
  .chain .node.pending {
    flex-direction: row; align-items: baseline; gap: 6px;
    padding: 3px 9px; border-radius: var(--r-panel);
    border: 1px dashed var(--border); background: var(--bg-rail);
  }
  .chain .node.pending .nv { font-size: 13px; color: var(--fg-off); }

  .delay-note { margin-top: 6px; font-size: 9px; line-height: 1.5; }
  .delay-note .warn { color: var(--danger, #B5443A); }

  /* パネル(もし〜/なぜ) */
  .panel { margin-top: 11px; border-radius: var(--r-window); overflow: hidden; border: 1px solid var(--border-strong); background: var(--bg-field); }
  .panel.purple { border-color: var(--sim); background: var(--sim-bg); }
  .panel-head { width: 100%; display: flex; align-items: center; gap: 8px; padding: 7px 12px; text-align: left; }
  .panel-head.purple { background: linear-gradient(180deg, var(--sim), var(--sim-strong)); border-bottom: 1px solid var(--sim-strong); }
  .panel-head.blue { background: linear-gradient(180deg, #DBE6F8, #AEC7F0); border-bottom: 1px solid var(--border-strong); cursor: pointer; }
  .panel-title { font-size: var(--t-label); font-weight: 800; letter-spacing: 0.08em; color: #fff; white-space: nowrap; }
  .panel-title.dark { color: var(--fg); }
  .panel-note { min-width: 0; flex: 1; text-align: right; font-size: 9px; color: #E4E3F4; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .panel-note.dark { color: #40536F; }
  .panel-body { padding: 11px 13px 12px; }

  .whatif {
    width: 100%; display: flex; align-items: center; gap: 10px; padding: 8px 10px; margin-bottom: 6px;
    border-radius: var(--r-panel); background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .whatif:last-child { margin-bottom: 0; }
  .whatif:hover { border-color: var(--sim); background: #F7F6FC; }
  .wi-main { min-width: 0; flex: 1; display: flex; align-items: center; gap: 8px; }
  .wi-label { min-width: 0; flex: 1; font-size: 11px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cost { flex-shrink: 0; padding: 1px 7px; border-radius: var(--r-pill); border: 1px solid; font-size: 8.5px; font-weight: 700; white-space: nowrap; }
  .wi-nums { flex-shrink: 0; text-align: right; display: flex; flex-direction: column; }
  .wi-pct { font-size: 12.5px; font-weight: 700; color: var(--sim-fg); }
  .wi-empty { margin: 0; padding: 9px 11px; font-size: 11px; line-height: 1.6; }

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
  .dt-expr { font-family: var(--font-num); word-break: break-all; }
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
  .sim-title { font-size: var(--t-label); font-weight: 700; color: var(--fg-sub); }
  .sim-bar.active .sim-title { color: var(--sim-fg); }
  .sim-delta { min-width: 34px; text-align: right; font-size: 11px; font-weight: 700; color: transparent; }
  .sim-delta.on { color: var(--fg-dim); }
  .sim-delta.on.up { color: var(--good); }
  .sim-delta.on.down { color: var(--danger); }
  .sim-note-text { margin-top: 3px; font-size: 9px; line-height: 1.6; text-wrap: pretty; }
  .sim-actions { margin-top: 8px; display: flex; gap: 7px; min-height: 30px; }
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

  .pw { margin-top: 8px; }
  .sw { margin-top: 8px; }
  .eq-details { margin-top: 9px; border-top: 1px dashed var(--border-soft); }
  .eq-details summary { padding: 8px 0 0; font-size: var(--t-label); color: var(--fg-muted); cursor: pointer; }
  .eq-details summary:hover { color: var(--fg); }
  .eq-grid { display: flex; flex-direction: column; gap: 7px; padding-top: 8px; }
  .eq-note { margin: 8px 0 0; font-size: 9.5px; line-height: 1.6; }

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

  .buff-chips { margin-top: 8px; display: flex; flex-wrap: wrap; gap: 5px; }
  .calc-buff-set { margin-top: 8px; display: flex; align-items: center; gap: 8px; font-size: 10px; color: var(--fg-muted); }
  .calc-buff-set select { min-width: 160px; height: 28px; border: 1px solid var(--border); border-radius: var(--r-inset); background: var(--bg-field); color: var(--fg); }
  .buff-chip {
    padding: 4px 9px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--border-soft);
    font-size: 10px; font-weight: 500; color: var(--fg-muted); white-space: nowrap;
  }
  .buff-chip:hover:not(:disabled) { border-color: var(--accent); }
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
  .entry-note {
    margin: 8px 0 0; padding: 7px 9px; border-radius: var(--r-panel);
    background: #FDF9EE; border: 1px solid var(--gold);
    font-size: 9px; font-weight: 500; line-height: 1.6; color: var(--state-edge-fg);
  }
  .buff-detail {
    margin-top: 7px; padding: 7px 9px; border-radius: var(--r-panel);
    background: var(--bg-panel); border: 1px dashed var(--border-soft);
    display: flex; flex-direction: column; gap: 7px;
  }
  .bd-name { font-size: 10px; font-weight: 700; color: var(--fg-sub); }
  .bd-fixed { font-size: 10px; }

  .card.adj summary { cursor: pointer; font-size: 11px; }
  .adj-note { margin: 8px 0 0; font-size: 9px; line-height: 1.6; }

  /* .reqs/.req/.req-label/.req-tag は app.css(ui/RequirementList.svelte 経由) */
</style>
