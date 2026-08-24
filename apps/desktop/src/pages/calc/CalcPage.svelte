<script lang="ts">
  // ダメージ計算: v4 の縦フロー「相手を選ぶ → この一発 → もし〜だったら → なぜこの数字？」。
  // 右カラムは「計算の材料」(試し変更・バフ・入場条件)。計算はすべて Rust 側(preview_damage)。
  import { untrack } from "svelte";
  import {
    errorMessage, evaluateContents, listSkills, previewDamage, updateCharacter,
  } from "../../api/commands";
  import type {
    Adjustments, BuffDefinition, ContentEvaluation, DamageResult, NewCharacter, Skill, StatKind,
  } from "../../api/types";
  import {
    isBlocked, isChoiceValue, isConsumable, isFixedValue, isPercentLayer, isUserSelectedTarget,
    toggleBuff, userInputRange,
  } from "../../buffs";
  import { candidatesFor, COST_COLORS, type Candidate } from "../../candidates";
  import { fmtInt, fmtNum, formatLayerValue } from "../../format";
  import { EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_LABELS, STAT_KINDS, STAT_LABELS } from "../../labels";
  import { limits } from "../../limits.svelte";
  import {
    app, flatContents, payloadOf, selectedCharacter, upsertCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Select from "../../ui/Select.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import StatInput from "../../ui/StatInput.svelte";
  import TracePanel from "./TracePanel.svelte";

  const DEFAULT_RIGHT_WIDTH = 380;
  const layoutWidths = persisted("tw-v4-calc", { right: DEFAULT_RIGHT_WIDTH });
  const gridTemplateColumns = $derived(
    `minmax(320px, 1fr) 6px minmax(280px, ${layoutWidths.value.right ?? DEFAULT_RIGHT_WIDTH}px)`,
  );

  const COMBO_THRESHOLD = 3;

  const character = $derived(selectedCharacter());
  const savedPayload = $derived(character ? payloadOf(character) : null);
  const payload = $derived(app.sim ?? savedPayload);

  // --- 対象(コンテンツ) --------------------------------------------------
  const contents = $derived(flatContents());
  const targetIndex = $derived(
    Math.max(0, contents.findIndex((x) => x.content.id === app.calcTargetId)),
  );
  const target = $derived(contents[targetIndex] ?? null);
  let targetOpen = $state(false);
  function stepTarget(dir: number) {
    if (contents.length === 0) return;
    app.calcTargetId = contents[(targetIndex + dir + contents.length) % contents.length].content.id;
  }

  // --- スキル(キャラ種で引き直し) ----------------------------------------
  let skills = $state<Skill[]>([]);
  let skillId = $state("");
  // 取得済みスキルが属するキャラ種(非リアクティブ)。キャラ種が変わった瞬間に skillId を
  // 同期的に空へ戻す。残すと listSkills の応答まで「別キャラのステ × 前キャラのスキル」で
  // 計算・表示されてしまう(Rust 側はスキル所有チェックをしない。PR レビュー指摘)。
  let skillsGid: string | null = null;
  $effect(() => {
    const gid = character?.game_character_id ?? null;
    if (gid === skillsGid) return; // 保存等でキャラのオブジェクトだけ変わった場合は選択を保つ
    skillsGid = gid;
    skills = [];
    skillId = "";
    if (!gid) return;
    listSkills(gid)
      .then((list) => {
        if (skillsGid !== gid) return; // 切替済みの古い応答は捨てる
        skills = list;
        // ホームからの遷移はホームの判定に使ったスキル(最大ダメージ)を引き継ぐ。
        const handoff = app.calcSkillId;
        app.calcSkillId = null;
        if (handoff && list.some((s) => s.id === handoff)) skillId = handoff;
        else skillId = list[0]?.id ?? "";
      })
      .catch((e) => reportError(errorMessage(e)));
  });
  const skill = $derived(skills.find((s) => s.id === skillId) ?? null);
  let skillOpen = $state(false);

  // スキル一覧の対象別ダメージ(ドロップダウンを開いたときに計算)
  let skillTotals = $state<Record<string, { perHit: number; total: number }>>({});
  let skillSeq = 0;
  $effect(() => {
    // 対象・キャラ・試し変更が変わったら古い合計を出さない(PR レビュー指摘)
    skillTotals = {};
    if (!skillOpen || !payload || !target || skills.length === 0) return;
    const p = JSON.parse(JSON.stringify(payload)) as NewCharacter;
    const temp = JSON.parse(JSON.stringify(temporaryAdjustments)) as Adjustments;
    const enemyId = target.content.enemy_id;
    const comboCount = combo ? COMBO_THRESHOLD : 0;
    const seq = ++skillSeq;
    Promise.all(
      skills.map(async (s) => [s.id, await previewDamage(p, s.id, enemyId, comboCount, temp)] as const),
    )
      .then((rs) => {
        if (seq !== skillSeq) return;
        skillTotals = Object.fromEntries(
          rs.map(([id, r]) => [id, { perHit: r.per_hit.max, total: r.total.max }]),
        );
      })
      .catch((e) => reportError(errorMessage(e)));
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
  });

  // --- 計算(payload と saved の両方) -------------------------------------
  let result = $state<DamageResult | null>(null);
  let savedResult = $state<DamageResult | null>(null);
  let calculating = $state(false);
  let requestSeq = 0;
  let debounceHandle: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null; // sim のネスト変更も拾う
    const sp = savedPayload;
    const t = target;
    const sid = skillId;
    const comboCount = combo ? COMBO_THRESHOLD : 0;
    const simActive = app.sim !== null;
    const tempJson = JSON.stringify(temporaryAdjustments);
    if (debounceHandle) clearTimeout(debounceHandle);
    if (!pJson || !sp || !t || !sid) {
      result = null;
      savedResult = null;
      return;
    }
    const seq = ++requestSeq;
    calculating = true;
    debounceHandle = setTimeout(async () => {
      try {
        const main = await previewDamage(JSON.parse(pJson), sid, t.content.enemy_id, comboCount, JSON.parse(tempJson));
        const saved = simActive
          ? await previewDamage(sp, sid, t.content.enemy_id, comboCount, JSON.parse(tempJson))
          : main;
        if (seq === requestSeq) {
          result = main;
          savedResult = saved;
        }
      } catch (e) {
        if (seq === requestSeq) {
          result = null;
          reportError(errorMessage(e));
        }
      } finally {
        if (seq === requestSeq) calculating = false;
      }
    }, 120);
    return () => {
      if (debounceHandle) clearTimeout(debounceHandle);
    };
  });

  // --- 入場条件・通るのは(payload 基準、Rust 側で判定) --------------------
  let evals = $state<ContentEvaluation[]>([]);
  let evalSeq = 0;
  let evalHandle: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null;
    if (evalHandle) clearTimeout(evalHandle);
    if (!pJson) {
      evals = [];
      return;
    }
    const seq = ++evalSeq;
    evalHandle = setTimeout(() => {
      evaluateContents(JSON.parse(pJson))
        .then((rs) => {
          if (seq === evalSeq) evals = rs;
        })
        .catch((e) => reportError(errorMessage(e)));
    }, 200);
    return () => {
      if (evalHandle) clearTimeout(evalHandle);
    };
  });
  const targetEval = $derived(target ? (evals.find((e) => e.content_id === target.content.id) ?? null) : null);
  const clearCount = $derived(evals.filter((e) => e.clear).length);

  // --- 表示値 -------------------------------------------------------------
  const perHit = $derived(result?.per_hit.max ?? null);
  const savedPerHit = $derived(savedResult?.per_hit.max ?? null);
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
  const BADGE = ["余裕", "通る", "ぎりぎり", "届かない", "条件・火力とも未達", "条件だけ未達", "判定中"];
  const BADGE_BG = ["#DCEBFF", "#DFF3E6", "#FDF3DE", "#F6E8E5", "#ECEEF2", "#EFEEF8", "#ECEEF2"];
  const BADGE_BD = ["#426DD6", "#6FA98A", "#C2A057", "#B08480", "#A9B4C4", "#6D6AA8", "#A9B4C4"];
  const BADGE_FG = ["#2B4FA8", "#2E6B4C", "#7A6420", "#8C4A42", "#5E6E88", "#4A4780", "#5E6E88"];
  const BAR_BG = [
    "linear-gradient(90deg,#90D7FF,#426DD6)",
    "linear-gradient(90deg,#9FD9BC,#3E8C63)",
    "linear-gradient(90deg,#F0D79A,#C2A057)",
    "linear-gradient(90deg,#E8B3A2,#B0574A)",
    "linear-gradient(90deg,#CBD3DE,#9AA6B6)",
    "linear-gradient(90deg,#C3C1E4,#6D6AA8)",
    "linear-gradient(90deg,#CBD3DE,#9AA6B6)",
  ];

  // --- なぜこの数字?(トレースの式から組み立て) ---------------------------
  const stepsMax = $derived(result?.trace.steps_max ?? []);
  const stepValue = (name: string): number | null =>
    stepsMax.find((s) => s.name === name)?.value ?? null;
  const atkA = $derived(stepValue("攻撃力(A)"));
  const atkStat = $derived(stepValue("ステ攻撃力"));
  const atkEquip = $derived(stepValue("装備攻撃力"));
  const atkBonus = $derived(
    atkA !== null && atkStat !== null && atkEquip !== null
      ? Math.max(0, atkA - Math.floor(atkStat + atkEquip))
      : 0,
  );
  const atkRows = $derived.by(() => {
    if (atkA === null) return [];
    const raw = [
      { k: "ステ攻撃力", v: atkStat ?? 0, c: "#9AA6B6", note: "素ステ・補正源から" },
      { k: "装備攻撃力", v: atkEquip ?? 0, c: "#426DD6", note: "基本/強化 × 依存別係数" },
      { k: "装備攻撃力強化倍率", v: atkBonus, c: "#6D6AA8", note: "パワーW・ストロングW" },
    ].filter((x) => x.v > 0);
    const total = raw.reduce((a, x) => a + x.v, 0) || 1;
    return raw.map((x) => ({
      ...x,
      pct: `${Math.max(1.5, (x.v / total) * 100).toFixed(2)}%`,
      share: `${Math.round((x.v / total) * 100)}%`,
    }));
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
    c: string;
  }
  const FLOW_COLORS: Record<string, string> = {
    "スキル倍率": "#426DD6",
    "クリティカル": "#5B8FD6",
    "コンボ・属性・カット率・オーラ": "#3E8C63",
    "最終ダメージ固定値(下限)": "#6FA98A",
    "最終ダメージ・カット率A・被害減少": "#8FBFA6",
    "各種ダメージ増減": "#C2A057",
    "攻撃ダメージ・PVP補正": "#B0824A",
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
    const rows: FlowRow[] = [{ k: "抜けた分(素通り)", add: pierced, mult: "—", c: "#93A0B2" }];
    for (const s of stepsMax) {
      if (FACTOR_STEPS.has(s.name)) {
        const next = running * s.value;
        rows.push({ k: s.name, add: next - running, mult: `×${s.value.toFixed(2)}`, c: FLOW_COLORS[s.name] ?? "#93A0B2" });
        running = next;
      } else if (RUNNING_STEPS.has(s.name)) {
        rows.push({ k: s.name, add: s.value - running, mult: "—", c: FLOW_COLORS[s.name] ?? "#93A0B2" });
        running = s.value;
      }
    }
    return rows;
  });
  const flowTotal = $derived(flowRows.reduce((a, r) => a + Math.max(0, r.add), 0) || 1);
  const topLever = $derived(
    [...flowRows.slice(1)].filter((r) => r.add > 0).sort((a, b) => b.add - a.add)[0] ?? null,
  );
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

  // --- 試し変更(sim) ------------------------------------------------------
  function editSim(fn: (p: NewCharacter) => void) {
    if (!payload) return;
    const p = JSON.parse(JSON.stringify(payload)) as NewCharacter;
    fn(p);
    app.sim = p;
  }
  const simActive = $derived(app.sim !== null);
  const simDirty = $derived(
    app.sim !== null && savedPayload !== null && JSON.stringify(app.sim) !== JSON.stringify(savedPayload),
  );
  function resetSim() {
    app.sim = null;
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
      label: (p) => `パワーW ${p.equipment.power_weapon ? "ON" : "OFF"}`,
      get: (p) => String(p.equipment.power_weapon),
      set: (p, v) => (p.equipment.power_weapon = v === "true"),
    },
    {
      id: "sw",
      label: (p) => `ストロングW ${p.equipment.strong_weapon_level > 0 ? `Lv${p.equipment.strong_weapon_level}` : "なし"}`,
      get: (p) => String(p.equipment.strong_weapon_level),
      set: (p, v) => (p.equipment.strong_weapon_level = Number(v)),
    },
    ...EQUIPMENT_STAT_KINDS.flatMap((k): Knob[] => [
      {
        id: `base_${k}`,
        label: (p) => `基本 ${EQUIPMENT_STAT_LABELS[k]} ${fmtInt(p.equipment.base[k])}`,
        get: (p) => String(p.equipment.base[k]),
        set: (p, v) => (p.equipment.base[k] = Number(v)),
      },
      {
        id: `enh_${k}`,
        label: (p) => `強化 ${EQUIPMENT_STAT_LABELS[k]} ${fmtInt(p.equipment.enhanced[k])}`,
        get: (p) => String(p.equipment.enhanced[k]),
        set: (p, v) => (p.equipment.enhanced[k] = Number(v)),
      },
    ]),
    {
      id: "buffs",
      label: (p) => `バフ選択 ${p.stat_sources.buffs.choices.length}件`,
      get: (p) => JSON.stringify(p.stat_sources.buffs.choices),
      set: (p, v) => (p.stat_sources.buffs.choices = JSON.parse(v)),
    },
    {
      id: "adjust",
      label: () => "調整",
      get: (p) => JSON.stringify(p.stat_sources.adjustments),
      set: (p, v) => (p.stat_sources.adjustments = JSON.parse(v)),
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
  }

  // --- もし〜だったら ------------------------------------------------------
  interface WhatIf {
    candidate: Candidate;
    perHit: number;
    deltaPct: number;
  }
  let whatIf = $state<WhatIf[]>([]);
  let whatIfSeq = 0;
  let whatIfHandle: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const pJson = payload ? JSON.stringify(payload) : null;
    const tempJson = JSON.stringify(temporaryAdjustments);
    const t = target;
    const sid = skillId;
    const base = perHit;
    if (whatIfHandle) clearTimeout(whatIfHandle);
    if (!pJson || !t || !sid || base === null) {
      whatIf = [];
      return;
    }
    const seq = ++whatIfSeq;
    whatIfHandle = setTimeout(async () => {
      try {
        const current = JSON.parse(pJson) as NewCharacter;
        const list = candidatesFor(current);
        const rs = await Promise.all(
          list.map(async (candidate) => {
            const p = JSON.parse(pJson) as NewCharacter;
            candidate.apply(p);
            const r = await previewDamage(p, sid, t.content.enemy_id, combo ? COMBO_THRESHOLD : 0, JSON.parse(tempJson));
            return {
              candidate,
              perHit: r.per_hit.max,
              deltaPct: base > 0 ? Math.round((r.per_hit.max / base - 1) * 100) : 0,
            };
          }),
        );
        if (seq === whatIfSeq) {
          whatIf = rs.filter((w) => w.perHit > base).sort((a, b) => b.perHit - a.perHit);
        }
      } catch (e) {
        if (seq === whatIfSeq) reportError(errorMessage(e));
      }
    }, 250);
    return () => {
      if (whatIfHandle) clearTimeout(whatIfHandle);
    };
  });
  function applyWhatIf(w: WhatIf) {
    editSim((p) => w.candidate.apply(p));
  }

  // --- 右カラム: バフ・装備の編集(試し変更として) -------------------------
  const consumableBuffs = $derived(app.catalog.filter(isConsumable));
  const buffOn = (def: BuffDefinition) =>
    (payload?.stat_sources.buffs.choices ?? []).some((c) => c.buff_id === def.id);
  function toggleBuffChip(def: BuffDefinition) {
    editSim((p) => {
      p.stat_sources.buffs.choices = toggleBuff(p.stat_sources.buffs.choices, def, !buffOn(def));
    });
  }
  // ON のバフのうち、対象ステ・効果量の選択肢・手入力を持つものの詳細編集(試し変更として反映)
  const statOptions = STAT_KINDS.map((k) => ({ value: k, label: STAT_LABELS[k] }));
  const buffChoiceOf = (buffId: string) =>
    payload?.stat_sources.buffs.choices.find((c) => c.buff_id === buffId) ?? null;
  const hasDetail = (def: BuffDefinition) =>
    isUserSelectedTarget(def.target) || isChoiceValue(def.value) || userInputRange(def.value) !== null || isFixedValue(def.value);
  function editBuffChoice(buffId: string, fn: (c: NewCharacter["stat_sources"]["buffs"]["choices"][number]) => void) {
    editSim((p) => {
      const c = p.stat_sources.buffs.choices.find((x) => x.buff_id === buffId);
      if (c) fn(c);
    });
  }
  const strongWeaponOptions = [
    { value: "0", label: "なし" },
    ...Array.from({ length: limits.strong_weapon_level_max }, (_, i) => ({
      value: String(i + 1),
      label: `Lv${i + 1}(+${(i + 1) * 3}%)`,
    })),
  ];

  const totalContents = $derived(contents.length);
</script>

<div class="layout" style="grid-template-columns: {gridTemplateColumns};">
  <section class="mid">
    <div class="head-bar">
      <span class="title">行ける？</span>
      <span class="note">→ 足りない分をどう埋める？ → なぜこの数字？</span>
    </div>
    <div class="scroll">
      {#if !character}
        <p class="empty dim">キャラを登録するとダメージ計算ができます。</p>
      {:else if !target}
        <p class="empty dim">コンテンツデータがありません。</p>
      {:else}
        <!-- 行ける?カード -->
        <div class="sheet">
          <div class="sheet-head">
            <span class="gem"></span>
            <span class="sheet-title">行ける？</span>
            <span class="sheet-char dim">{character.name}{calculating ? " ・ 計算中…" : ""}</span>
            <span class="badge" style="background: {BADGE_BG[badgeState]}; border-color: {BADGE_BD[badgeState]}; color: {BADGE_FG[badgeState]};">{BADGE[badgeState]}</span>
          </div>

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
            <div class="pop">
              {#each app.areas as area (area.id)}
                <div class="pop-head"><span class="pop-diamond"></span><span>{area.name}</span><span class="num dim">{area.contents.length} 件</span></div>
                {#each area.contents as c (c.id)}
                  {@const ev = evals.find((e) => e.content_id === c.id)}
                  <button
                    type="button"
                    class="pop-row"
                    class:on={c.id === target.content.id}
                    onclick={() => {
                      app.calcTargetId = c.id;
                      targetOpen = false;
                    }}
                  >
                    <span class="dot" style="background: {ev?.clear ? '#3E8C63' : ev?.entry_ok === false ? '#B0574A' : '#C1D3E6'};"></span>
                    <span class="pop-name">{c.name}</span>
                    <span class="num dim">{ev?.damage ? fmtInt(ev.damage.per_hit_max) : "—"}</span>
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
                  <span class="sk-name">{skill?.name ?? ""}</span>
                  {#if skills.length > 1}<span class="t-chev" class:rot={skillOpen}>▼</span>{/if}
                </span>
                <span class="sk-meta num dim">
                  ×{skill ? fmtNum(skill.multiplier) : "—"} ・ {skill?.hit_count ?? "—"}段 ・ Cri×{skill ? fmtNum(skill.critical_multiplier) : "—"}
                </span>
              </button>
            {/if}
          </div>
          {#if skillOpen && skills.length > 1}
            <button type="button" class="overlay" aria-label="閉じる" onclick={() => (skillOpen = false)}></button>
            <div class="pop gold">
              <div class="pop-head gold"><span>スキル {skills.length} 種 ／ 対象への合計ダメージ順は仮なし・登録順</span></div>
              {#each skills as s (s.id)}
                {@const d = skillTotals[s.id]}
                <button
                  type="button"
                  class="pop-row"
                  class:on={s.id === skillId}
                  onclick={() => {
                    skillId = s.id;
                    skillOpen = false;
                  }}
                >
                  <span class="pop-name">{s.name}</span>
                  <span class="num dim">×{fmtNum(s.multiplier)} / {s.hit_count}段</span>
                  <span class="num strong">{d ? fmtInt(d.total) : "…"}</span>
                </button>
              {/each}
            </div>
          {/if}

          <!-- この一発 -->
          <div class="hero">
            <div class="hero-line">
              <span class="hero-num num">{perHit !== null ? fmtInt(perHit) : "—"}</span>
              {#if simActive}
                <span class="hero-delta num" class:up={deltaPct > 0} class:down={deltaPct < 0}>
                  {deltaPct === 0 ? "±0%" : `${deltaPct > 0 ? "+" : ""}${deltaPct}%`}
                </span>
                <span class="hero-saved num dim">登録どおりなら {savedPerHit !== null ? fmtInt(savedPerHit) : "—"}</span>
              {/if}
            </div>
            <div class="meter big"><div class="fill" style="width: {Math.min(100, ratio * 100).toFixed(1)}%; background: {BAR_BG[badgeState]};"></div></div>
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
            <div class="totals">
              <div class="total-box">
                <span class="cap dim">{skill && skill.hit_count > 1 ? `合計 ×${skill.hit_count}段` : "合計(1段)"}</span>
                <span class="num strong">{result ? fmtInt(result.total.max) : "—"}</span>
              </div>
              <div class="total-box crit">
                <span class="cap">クリティカル ×{skill ? fmtNum(skill.critical_multiplier) : "—"}</span>
                <span class="num strong">{result ? fmtInt(result.total.critical) : "—"}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- もし〜だったら -->
        {#if whatIf.length > 0}
          <div class="panel purple">
            <div class="panel-head purple">
              <span class="panel-title">足りない分をどう埋める？</span>
              <span class="panel-note">押すと試し変更に入ります(保存されません)</span>
            </div>
            <div class="panel-body">
              {#each whatIf as w (w.candidate.id)}
                <button type="button" class="whatif" onclick={() => applyWhatIf(w)}>
                  <span class="wi-main">
                    <span class="wi-label">{w.candidate.label}</span>
                    <span
                      class="cost"
                      style="background: {COST_COLORS[w.candidate.cost][0]}; border-color: {COST_COLORS[w.candidate.cost][1]}; color: {COST_COLORS[w.candidate.cost][2]};"
                    >{w.candidate.cost}</span>
                  </span>
                  <span class="wi-nums">
                    <span class="num wi-pct">+{w.deltaPct}%</span>
                    <span class="num dim">{fmtInt(w.perHit)}</span>
                  </span>
                </button>
              {/each}
            </div>
          </div>
        {/if}

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
              <span class="num strong">{pierced !== null ? fmtInt(Math.max(0, Math.trunc(pierced))) : "—"}</span>
              <span class="arrow num dim">→</span>
              <span class="dim">倍率</span>
              <span class="num good strong">{flowMultLabel}</span>
              <span class="arrow num dim">→</span>
              <span class="num final">{perHit !== null ? fmtInt(perHit) : "—"}</span>
            </div>
            <div class="lever-note">
              {#if noPierce}
                攻撃力が相手の防御力に届いていないので、倍率は何もかかりません。まず攻撃力を上げる必要があります。
              {:else if topLever}
                いま一番効いているのは「{topLever.k}」。ここが最終ダメージの {Math.round((topLever.add / flowTotal) * 100)}% を作っています。
              {:else}
                倍率はまだ何もかかっていません。
              {/if}
            </div>

            {#if flowOpen}
              <!-- ① 攻撃力をつくる -->
              <div class="stage">
                <span class="stage-no" style="background: #426DD6;">1</span>
                <span class="stage-title">攻撃力をつくる</span>
                <span class="num strong stage-val">{atkA !== null ? fmtInt(atkA) : "—"}</span>
              </div>
              <div class="band">
                {#each atkRows as a (a.k)}
                  <div style="width: {a.pct}; background: {a.c};"></div>
                {/each}
              </div>
              <div class="band-rows">
                {#each atkRows as a (a.k)}
                  <div class="band-row">
                    <span class="swatch" style="background: {a.c};"></span>
                    <span class="br-label">{a.k}</span>
                    <span class="br-note dim">{a.note}</span>
                    <span class="num br-val">{fmtInt(Math.round(a.v))}</span>
                    <span class="num br-share dim">{a.share}</span>
                  </div>
                {/each}
              </div>

              <!-- ② 防御力を抜く -->
              <div class="stage">
                <span class="stage-no" style="background: #8C4A42;">2</span>
                <span class="stage-title">相手の防御力を抜く</span>
                <span class="num strong stage-val">{pierced !== null ? fmtInt(Math.max(0, Math.trunc(pierced))) : "—"}</span>
              </div>
              <div class="band">
                <div style="width: {(100 - defShare).toFixed(2)}%; background: linear-gradient(90deg, #8FB2E8, #426DD6);"></div>
                <div style="width: {defShare.toFixed(2)}%; background: repeating-linear-gradient(135deg, #B08480 0 4px, #8C4A42 4px 8px);"></div>
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
                <span class="stage-no" style="background: #3E8C63;">3</span>
                <span class="stage-title">倍率で伸ばす</span>
                <span class="stage-note dim">帯の幅＝足した分(赤字は減る倍率)</span>
                <span class="num strong stage-val">{perHit !== null ? fmtInt(perHit) : "—"}</span>
              </div>
              <div class="band">
                {#each flowRows.filter((r) => r.add > 0) as f (f.k)}
                  <div style="width: {((Math.max(0, f.add) / flowTotal) * 100).toFixed(2)}%; background: {f.c};"></div>
                {/each}
              </div>
              <div class="band-rows">
                {#each flowRows as f (f.k)}
                  <div class="band-row">
                    <span class="swatch" style="background: {f.c};"></span>
                    <span class="br-label" class:strong={topLever?.k === f.k} class:bad={f.add < 0}>{f.k}</span>
                    <span class="num br-mult dim">{f.mult}</span>
                    <span class="num br-val" class:bad={f.add < 0}>{f.add < 0 ? "−" : "+"}{fmtInt(Math.round(Math.abs(f.add)))}</span>
                    <span class="num br-share dim">{Math.round((Math.abs(f.add) / flowTotal) * 100)}%</span>
                  </div>
                {/each}
              </div>

              <!-- 倍率の材料 -->
              <div class="materials">
                <div class="mat-head">
                  <span class="mat-title">倍率の材料</span>
                  <span class="dim">上限に届いた枠は「満」</span>
                </div>
                <div class="mat-chips">
                  {#each activeCategories as c (c.category)}
                    <span class="mat-chip" class:cap={catAtCap(c)}>
                      <span class="dim">{c.label}</span>
                      <span class="num strong">{fmtCatValue(c)}</span>
                      {#if catAtCap(c)}<span class="full">満</span>{/if}
                    </span>
                  {/each}
                  {#if activeCategories.length === 0}
                    <span class="dim">まだ倍率の材料がありません(バフ・称号などを設定すると増えます)。</span>
                  {/if}
                </div>
              </div>

              {#if result}
                <TracePanel trace={result.trace} {character} />
              {/if}
            {/if}
          </div>
        </div>
      {/if}
    </div>
  </section>

  <Splitter
    bind:value={layoutWidths.value.right}
    min={280}
    defaultValue={DEFAULT_RIGHT_WIDTH}
    controls="next"
    label="計算シートと材料の境界"
  />

  <section class="right">
    <div class="head-bar">
      <span class="title">計算の材料</span>
      <span class="note">{character?.name ?? ""}</span>
    </div>
    <div class="scroll pad">
      {#if character && payload}
        <!-- 試し変更バー -->
        <div class="sim-bar" class:active={simDirty}>
          <div class="sim-line">
            <span class="sim-dot" class:active={simDirty}></span>
            <span class="sim-title">{simDirty ? "試し変更中" : "登録どおり"}</span>
            {#if simActive}
              <span class="num sim-delta" class:up={deltaPct > 0} class:down={deltaPct < 0}>
                {deltaPct === 0 ? "±0%" : `${deltaPct > 0 ? "+" : ""}${deltaPct}%`}
              </span>
            {/if}
          </div>
          <div class="sim-note-text dim">
            {simDirty
              ? "保存されていません。チップの ✕ で1つずつ戻せます。"
              : "下の材料を変えると、その場で数字が動きます(保存されません)。"}
          </div>
          {#if simDirty}
            <div class="sim-actions">
              <button type="button" class="btn" onclick={resetSim}>ぜんぶ戻す</button>
              <button type="button" class="btn primary" disabled={saving} onclick={saveSim}>{saving ? "保存中…" : "キャラに保存"}</button>
            </div>
          {/if}
        </div>

        {#if changedKnobs.length > 0}
          <div class="chips">
            {#each changedKnobs as k (k.id)}
              <span class="chip-diff">
                <span>{k.label(app.sim!)}</span>
                <button type="button" class="chip-x" title="この変更だけ戻す" onclick={() => revertKnob(k)}>✕</button>
              </span>
            {/each}
          </div>
        {/if}

        <!-- 装備(試し変更) -->
        <div class="card">
          <div class="card-head">
            <span class="card-title">装備</span>
            <span class="dim small">変更は試し変更として反映</span>
          </div>
          <label class="pw">
            <input
              type="checkbox"
              checked={payload.equipment.power_weapon}
              onchange={(e) => {
                const v = e.currentTarget.checked;
                editSim((p) => (p.equipment.power_weapon = v));
              }}
            />
            <span>パワーウェポン(+2%)</span>
          </label>
          <div class="sw">
            <Select
              label="ストロングウェポン"
              options={strongWeaponOptions}
              bind:value={
                () => String(payload.equipment.strong_weapon_level),
                (v) => editSim((p) => (p.equipment.strong_weapon_level = Number(v)))
              }
            />
          </div>
          <details class="eq-details">
            <summary>装備補正 8 値(基本/強化)</summary>
            <div class="eq-grid">
              <span class="eq-cap dim">基本能力値</span>
              {#each EQUIPMENT_STAT_KINDS as k (k)}
                <StatInput
                  label={EQUIPMENT_STAT_LABELS[k]}
                  min={0}
                  max={limits.equipment_value_max}
                  bind:value={
                    () => payload.equipment.base[k],
                    (v) => editSim((p) => (p.equipment.base[k] = v))
                  }
                />
              {/each}
              <span class="eq-cap dim">強化能力値</span>
              {#each EQUIPMENT_STAT_KINDS as k (k)}
                <StatInput
                  label={EQUIPMENT_STAT_LABELS[k]}
                  min={0}
                  max={limits.equipment_value_max}
                  bind:value={
                    () => payload.equipment.enhanced[k],
                    (v) => editSim((p) => (p.equipment.enhanced[k] = v))
                  }
                />
              {/each}
            </div>
          </details>
        </div>

        <!-- バフ -->
        <div class="card">
          <div class="card-head">
            <span class="card-title">バフ</span>
            <span class="dim small">押した瞬間に数字が動きます</span>
          </div>
          <div class="buff-chips">
            {#each consumableBuffs as def (def.id)}
              {@const on = buffOn(def)}
              {@const blocked = !on && isBlocked(payload.stat_sources.buffs.choices, app.catalog, def)}
              <button
                type="button"
                class="buff-chip"
                class:on
                disabled={blocked}
                title={blocked ? "同枠の他バフと排他です" : def.note || undefined}
                onclick={() => toggleBuffChip(def)}
              >{def.name}</button>
            {/each}
          </div>
          {#each consumableBuffs.filter((d) => buffOn(d) && hasDetail(d)) as def (def.id)}
            {@const choice = buffChoiceOf(def.id)}
            {#if choice}
              <div class="buff-detail">
                <span class="bd-name">{def.name}</span>
                {#if isUserSelectedTarget(def.target)}
                  <Select
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
                  <span class="dim bd-fixed">値: {formatLayerValue(def.layer, def.value.fixed)}</span>
                {/if}
              </div>
            {/if}
          {/each}
          <p class="buff-note dim">変更は試し変更として反映されます。「キャラに保存」で常用セットとして残ります。</p>
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
        <label class="combo">
          <input type="checkbox" bind:checked={combo} />
          <span>{COMBO_THRESHOLD} コンボ以上(+15%)</span>
        </label>

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
              <div class="reqs">
                {#each targetEval.checks as c (c.label)}
                  <div class="req" class:ng={!c.ok}>
                    <span class="req-label">{c.label}</span>
                    <span class="num dim">{fmtInt(c.current)} / {fmtInt(c.required)}</span>
                    <span class="req-tag">{c.ok ? "OK" : `あと ${fmtInt(c.required - c.current)}`}</span>
                  </div>
                {/each}
              </div>
            {/if}
            {#if target.content.team_note}
              <p class="buff-note dim">チーム条件: {target.content.team_note}</p>
            {/if}
          </div>
        {/if}
      {:else}
        <p class="empty dim">キャラを選択してください。</p>
      {/if}
    </div>
  </section>
</div>

<style>
  .layout { flex: 1; min-height: 0; display: grid; }
  section { min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  section.mid { background: var(--bg-mid); }
  section.right { background: var(--bg-rail); border-left: 1px solid var(--border-strong); }
  .scroll { flex: 1; min-height: 0; overflow: auto; padding: 13px 16px 18px; }
  .scroll.pad { padding: 11px; display: flex; flex-direction: column; gap: 9px; }
  .empty { font-size: 12px; }

  /* 行ける?カード */
  .sheet { position: relative; border-radius: 13px; border: 1px solid #687287; box-shadow: 0 1px 0 rgba(121, 140, 172, 0.4); background: #fff; }
  .sheet-head {
    display: flex; align-items: center; gap: 8px; padding: 7px 13px;
    border-radius: 12px 12px 0 0;
    background: linear-gradient(180deg, #F2E3BD, #DCC27E); border-bottom: 1px solid #BFA155;
  }
  .gem { flex-shrink: 0; width: 9px; height: 9px; transform: rotate(45deg); background: linear-gradient(160deg, #fff, #C9A227); border: 1px solid #A9821F; }
  .sheet-title { font-size: 11px; font-weight: 800; letter-spacing: 0.08em; color: #4A3C12; white-space: nowrap; }
  .sheet-char { min-width: 0; flex: 1; font-size: 9px; color: #6B5A24; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .target-row { position: relative; z-index: 3; display: flex; align-items: center; gap: 8px; padding: 10px 11px 0; background: linear-gradient(180deg, #F4F9FE, #fff); }
  .step {
    flex-shrink: 0; width: 25px; height: 25px; display: flex; align-items: center; justify-content: center;
    border-radius: 7px; background: linear-gradient(180deg, #fff, #E9F1FB); border: 1px solid #9FB4D0;
    font-size: 9px; font-weight: 700; color: #3B4A63; font-family: var(--font-num);
  }
  .step:hover { background: var(--bg-active); }
  .target-trigger { min-width: 0; flex: 1; padding: 3px 8px; border-radius: 8px; border: 1px solid transparent; text-align: left; }
  .target-trigger:hover, .target-trigger.open { background: var(--bg-rail); border-color: #9FB4D0; }
  .t-line1 { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  .t-name { min-width: 0; font-size: 15px; font-weight: 800; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .t-chev { flex-shrink: 0; font-size: 8px; color: var(--fg-muted); font-family: var(--font-num); transition: transform 0.18s; }
  .t-chev.rot { transform: rotate(180deg); }
  .t-index { flex-shrink: 0; margin-left: auto; font-size: 8.5px; }
  .t-line2 { margin-top: 1px; display: flex; align-items: baseline; gap: 9px; min-width: 0; }
  .t-area { min-width: 0; font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .t-def { flex-shrink: 0; font-size: 8.5px; color: var(--danger); }
  .t-need { flex-shrink: 0; font-size: 8.5px; color: #3B4A63; }

  .overlay { position: fixed; inset: 0; z-index: 40; cursor: default; }
  .pop {
    position: absolute; left: 10px; right: 10px; top: 88px; z-index: 41;
    max-height: 262px; overflow-y: auto; overscroll-behavior: contain;
    border-radius: 12px; background: #fff; border: 1px solid #687287;
    box-shadow: 0 10px 24px rgba(30, 44, 74, 0.3), inset 0 0 0 1px #fff;
  }
  .pop.gold { border-color: #A9821F; box-shadow: 0 10px 24px rgba(74, 60, 18, 0.28), inset 0 0 0 1px #fff; }
  .pop-head {
    position: sticky; top: 0; z-index: 1; display: flex; align-items: center; gap: 7px;
    padding: 6px 13px 6px 11px;
    background: linear-gradient(180deg, #DBE6F8, #C6D8F0); border-bottom: 1px solid var(--border);
    font-size: 9.5px; font-weight: 800; letter-spacing: 0.1em; color: #26334A;
  }
  .pop-head.gold { background: linear-gradient(180deg, #F2E3BD, #DCC27E); border-bottom-color: #BFA155; color: #4A3C12; }
  .pop-head .num { margin-left: auto; font-weight: 400; }
  .pop-diamond { width: 6px; height: 6px; transform: rotate(45deg); background: var(--head-bar); }
  .pop-row {
    width: 100%; display: flex; align-items: center; gap: 9px; padding: 7px 13px 7px 11px;
    border-bottom: 1px solid #EDF2F9; text-align: left;
  }
  .pop-row:hover { background: #F1F7FE; }
  .pop-row.on { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); }
  .pop-row .dot { width: 7px; height: 7px; flex-shrink: 0; border-radius: 50%; }
  .pop-name { min-width: 0; flex: 1; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pop-row.on .pop-name { font-weight: 700; }
  .pop-row .strong { font-weight: 700; }

  .skill-row { position: relative; z-index: 2; padding: 8px 11px 0; }
  .skill-trigger {
    width: 100%; display: flex; flex-direction: column; align-items: stretch; gap: 1px;
    padding: 5px 9px; border-radius: 9px; background: #F4F9FE; border: 1px solid #D6E2F0; text-align: left;
  }
  .skill-trigger:hover { background: var(--bg-rail); }
  .sk-line1 { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  .sk-name { min-width: 0; flex: 1; font-size: 11.5px; font-weight: 700; color: #3E2B26; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sk-meta { font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .hero { padding: 11px 13px 12px; }
  .hero-line { display: flex; align-items: baseline; gap: 10px; min-width: 0; }
  .hero-num { font-size: 44px; line-height: 1; font-weight: 700; }
  .hero-delta { font-size: 13px; font-weight: 700; color: var(--fg-dim); }
  .hero-delta.up { color: var(--good); }
  .hero-delta.down { color: var(--danger); }
  .hero-saved { min-width: 0; flex: 1; text-align: right; font-size: 9.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meter.big { margin-top: 10px; height: 12px; border-radius: 7px; }
  .hero-sentence { margin-top: 7px; display: flex; align-items: baseline; gap: 9px; min-width: 0; }
  .sentence { min-width: 0; flex: 1; font-size: 11px; font-weight: 700; text-wrap: pretty; }
  .sentence.ok { color: var(--good); }
  .sentence.ng { color: var(--danger); }
  .hero-sentence .num { flex-shrink: 0; font-size: 9.5px; }
  .totals { margin-top: 11px; padding-top: 11px; border-top: 1px dashed var(--border-soft); display: flex; gap: 7px; }
  .total-box { flex: 1; min-width: 0; padding: 6px 10px; border-radius: 9px; background: var(--bg-panel); border: 1px solid var(--border-soft); display: flex; flex-direction: column; }
  .total-box .cap { font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .total-box .strong { font-size: 15px; font-weight: 700; }
  .total-box.crit { background: #FFFBF0; border-color: #E0C98A; }
  .total-box.crit .cap { color: #7A6420; }
  .total-box.crit .strong { color: #A97E1E; }

  /* パネル(もし〜/なぜ) */
  .panel { margin-top: 11px; border-radius: 12px; overflow: hidden; border: 1px solid var(--border-strong); background: #fff; }
  .panel.purple { border-color: #6D6AA8; background: #FBFAFE; }
  .panel-head { width: 100%; display: flex; align-items: center; gap: 8px; padding: 7px 12px; text-align: left; }
  .panel-head.purple { background: linear-gradient(180deg, #6D6AA8, #565394); border-bottom: 1px solid #565394; }
  .panel-head.blue { background: linear-gradient(180deg, #DBE6F8, #AEC7F0); border-bottom: 1px solid var(--border-strong); cursor: pointer; }
  .panel-title { font-size: 10.5px; font-weight: 800; letter-spacing: 0.08em; color: #fff; white-space: nowrap; }
  .panel-title.dark { color: var(--fg); }
  .panel-note { min-width: 0; flex: 1; text-align: right; font-size: 9px; color: #E4E3F4; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .panel-note.dark { color: #40536F; }
  .panel-body { padding: 11px 13px 12px; }

  .whatif {
    width: 100%; display: flex; align-items: center; gap: 10px; padding: 8px 10px; margin-bottom: 6px;
    border-radius: 9px; background: #fff; border: 1px solid var(--border-soft); text-align: left;
  }
  .whatif:last-child { margin-bottom: 0; }
  .whatif:hover { border-color: #6D6AA8; background: #F7F6FC; }
  .wi-main { min-width: 0; flex: 1; display: flex; align-items: center; gap: 8px; }
  .wi-label { min-width: 0; flex: 1; font-size: 11px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cost { flex-shrink: 0; padding: 1px 7px; border-radius: 999px; border: 1px solid; font-size: 8.5px; font-weight: 700; white-space: nowrap; }
  .wi-nums { flex-shrink: 0; text-align: right; display: flex; flex-direction: column; }
  .wi-pct { font-size: 12.5px; font-weight: 700; color: #4A4780; }

  .flow-line { display: flex; align-items: center; gap: 7px; flex-wrap: wrap; font-size: 9px; }
  .flow-line .strong { font-size: 13px; font-weight: 700; color: #3B4A63; }
  .flow-line .good.strong { color: #3E8C63; }
  .flow-line .final { font-size: 15px; font-weight: 700; color: var(--fg); }
  .lever-note {
    margin-top: 9px; padding: 8px 10px; border-radius: 9px;
    background: #F4F9FE; border: 1px solid var(--border-soft);
    font-size: 10.5px; font-weight: 500; line-height: 1.6; color: #3B4A63; text-wrap: pretty;
  }

  .stage { margin-top: 14px; padding-top: 12px; border-top: 1px dashed var(--border-soft); display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .stage-no { flex-shrink: 0; width: 15px; height: 15px; border-radius: 50%; color: #fff; font-size: 9px; line-height: 16px; text-align: center; font-family: var(--font-num); font-weight: 700; }
  .stage-title { font-size: 11px; font-weight: 700; white-space: nowrap; }
  .stage-note { min-width: 0; flex: 1; font-size: 9px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .stage-val { margin-left: auto; font-size: 15px; font-weight: 700; }
  .band { margin-top: 7px; display: flex; height: 11px; border-radius: 6px; overflow: hidden; border: 1px solid var(--border-soft); background: #EDF2F9; }
  .band > div { flex-shrink: 0; transition: width 0.5s ease; }
  .band-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 5px; }
  .band-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .swatch { flex-shrink: 0; width: 8px; height: 8px; border-radius: 2px; }
  .br-label { min-width: 0; flex: 1; font-size: 10.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .br-label.strong { font-weight: 700; }
  .br-label.bad { color: var(--danger); }
  .br-note { min-width: 0; flex: 1.2; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .br-mult { flex-shrink: 0; width: 48px; text-align: right; font-size: 10px; }
  .br-val { flex-shrink: 0; width: 64px; text-align: right; font-size: 11px; font-weight: 700; color: #3B4A63; }
  .br-val.bad { color: var(--danger); }
  .br-share { flex-shrink: 0; width: 32px; text-align: right; font-size: 9.5px; }
  .pierce-note { margin-top: 7px; display: flex; align-items: center; gap: 10px; font-size: 9.5px; color: var(--fg-muted); min-width: 0; }
  .def-warn { min-width: 0; flex: 1; text-align: right; font-family: var(--font); font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .def-warn.bad { color: var(--danger); }

  .materials { margin-top: 12px; padding-top: 10px; border-top: 1px dashed var(--border-soft); }
  .mat-head { display: flex; align-items: baseline; gap: 8px; }
  .mat-title { font-size: 10px; font-weight: 700; letter-spacing: 0.06em; color: var(--fg-muted); }
  .mat-head .dim { margin-left: auto; font-size: 9px; }
  .mat-chips { margin-top: 7px; display: flex; flex-wrap: wrap; gap: 5px; }
  .mat-chip {
    display: inline-flex; align-items: center; gap: 6px; padding: 4px 9px; border-radius: 8px;
    background: var(--bg-panel); border: 1px solid var(--border-soft); font-size: 9.5px;
  }
  .mat-chip.cap { background: #F6E8E5; border-color: #B08480; }
  .mat-chip .strong { font-size: 10px; font-weight: 700; }
  .mat-chip .full { font-size: 8.5px; font-weight: 700; color: var(--danger); }

  /* 右カラム */
  .sim-bar { padding: 10px 11px; border-radius: 11px; background: var(--bg-panel); border: 1px solid var(--border-soft); }
  .sim-bar.active { background: #F7F6FC; border-color: #6D6AA8; }
  .sim-line { display: flex; align-items: center; gap: 8px; }
  .sim-dot { flex-shrink: 0; width: 7px; height: 7px; border-radius: 50%; background: #9FB4D0; }
  .sim-dot.active { background: #6D6AA8; }
  .sim-title { font-size: 10.5px; font-weight: 700; color: #3B4A63; }
  .sim-bar.active .sim-title { color: #4A4780; }
  .sim-delta { margin-left: auto; font-size: 13px; font-weight: 700; color: var(--fg-dim); }
  .sim-delta.up { color: var(--good); }
  .sim-delta.down { color: var(--danger); }
  .sim-note-text { margin-top: 3px; font-size: 9px; line-height: 1.6; text-wrap: pretty; }
  .sim-actions { margin-top: 8px; display: flex; gap: 7px; }
  .sim-actions .btn { flex: 1; }

  .chips { display: flex; flex-wrap: wrap; gap: 5px; }
  .chip-diff {
    display: inline-flex; align-items: center; gap: 7px; padding: 3px 4px 3px 9px; border-radius: 999px;
    background: #fff; border: 1px solid #6D6AA8; box-shadow: 0 1px 0 rgba(109, 106, 168, 0.25);
    font-size: 10px; font-weight: 500;
  }
  .chip-x {
    width: 16px; height: 16px; display: flex; align-items: center; justify-content: center;
    border-radius: 50%; background: #EFEEF8; font-size: 9px; color: #6D6AA8;
  }
  .chip-x:hover { background: #6D6AA8; color: #fff; }

  .card-head { display: flex; align-items: center; gap: 8px; }
  .small { margin-left: auto; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .pw { margin-top: 8px; display: flex; align-items: center; gap: 8px; font-size: 11.5px; cursor: pointer; }
  .pw input { accent-color: var(--accent); }
  .sw { margin-top: 8px; }
  .eq-details { margin-top: 9px; border-top: 1px dashed var(--border-soft); }
  .eq-details summary { padding: 8px 0 0; font-size: 10.5px; color: var(--fg-muted); cursor: pointer; }
  .eq-details summary:hover { color: var(--fg); }
  .eq-grid { display: flex; flex-direction: column; gap: 7px; padding-top: 8px; }
  .eq-cap { font-size: 9.5px; letter-spacing: 0.08em; }

  .buff-chips { margin-top: 8px; display: flex; flex-wrap: wrap; gap: 5px; }
  .buff-chip {
    padding: 4px 9px; border-radius: 999px;
    background: #fff; border: 1px solid var(--border-soft);
    font-size: 10px; font-weight: 500; color: var(--fg-muted); white-space: nowrap;
  }
  .buff-chip:hover:not(:disabled) { border-color: var(--accent); }
  .buff-chip.on {
    background: linear-gradient(180deg, #CCF7FF, #90D7FF);
    border-color: #687287; color: #123047; font-weight: 700;
  }
  .buff-note { margin: 8px 0 0; font-size: 9px; line-height: 1.6; }
  .buff-detail {
    margin-top: 7px; padding: 7px 9px; border-radius: 9px;
    background: var(--bg-panel); border: 1px dashed var(--border-soft);
    display: flex; flex-direction: column; gap: 7px;
  }
  .bd-name { font-size: 10px; font-weight: 700; color: #3B4A63; }
  .bd-fixed { font-size: 10px; }

  .card.adj summary { cursor: pointer; font-size: 11px; }
  .adj-note { margin: 8px 0 0; font-size: 9px; line-height: 1.6; }

  .combo { display: flex; align-items: center; gap: 8px; padding: 2px 4px; font-size: 11.5px; cursor: pointer; }
  .combo input { accent-color: var(--accent); }

  .reqs { margin-top: 8px; display: flex; flex-direction: column; gap: 5px; }
  .req {
    display: flex; align-items: center; gap: 8px; padding: 6px 9px; border-radius: 8px;
    background: #F4F9FE; border: 1px solid var(--border-soft);
  }
  .req.ng { background: #F6E8E5; border-color: #B08480; }
  .req-label { min-width: 0; flex: 1; font-size: 10.5px; font-weight: 500; color: #3B4A63; white-space: nowrap; }
  .req.ng .req-label { color: var(--danger); }
  .req .num { font-size: 10px; white-space: nowrap; }
  .req-tag { flex-shrink: 0; font-size: 9.5px; font-weight: 700; color: #3B4A63; white-space: nowrap; }
  .req.ng .req-tag { color: var(--danger); }
</style>
