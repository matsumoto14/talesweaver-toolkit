<script lang="ts">
  // ダメージ計算: v4 の縦フロー「相手を選ぶ → この一発 → もし〜だったら → なぜこの数字？」。
  // 右カラムは「計算の材料」(試し変更・バフ・入場条件)。計算はすべて Rust 側(preview_damage)。
  import { untrack } from "svelte";
  import {
    errorMessage, evaluateContents, listSkills, previewDamage, previewDefense, updateCharacter,
  } from "../../api/commands";
  import type {
    Adjustments, BuffDefinition, ContentEvaluation, DamageResult, DefenseProfile, NewCharacter,
    Skill, StatKind,
  } from "../../api/types";
  import {
    isBlocked, isChoiceValue, isConsumable, isFixedValue, isPercentLayer, isUserSelectedTarget,
    toggleBuff, userInputRange,
  } from "../../buffs";
  import { candidatesFor, COST_COLORS, type Candidate } from "../../candidates";
  import { fmtInt, fmtNum, formatLayerValue } from "../../format";
  import { ELEMENT_LABELS, EQUIPMENT_STAT_LABELS, STAT_KINDS, STAT_LABELS } from "../../labels";
  import { limits } from "../../limits.svelte";
  import {
    app, flatContents, payloadOf, selectedCharacter, upsertCharacter,
  } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Icon from "../../ui/Icon.svelte";
  import DefensePanel from "./DefensePanel.svelte";
  import Select from "../../ui/Select.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import { bump } from "../../ui/motion.svelte";
  import { badgeStyle, STATE, type Badge } from "../../ui/states";
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
  /** ピッカーの並びは合計ダメージの降順(v4 指定)。合計が未取得のものは登録順で末尾 */
  const pickerSkills = $derived(
    [...skills].sort((a, b) => (skillTotals[b.id]?.total ?? -1) - (skillTotals[a.id]?.total ?? -1)),
  );

  // スキル一覧の対象別ダメージ(ドロップダウンを開いたときに計算)
  let skillTotals = $state<Record<string, { perHit: number; total: number }>>({});
  let skillSeq = 0;
  $effect(() => {
    // 対象・キャラ・試し変更が変わったら古い合計を出さない(PR レビュー指摘)
    skillTotals = {};
    if (!skillOpen || !payload || !target || skills.length === 0) return;
    const p = JSON.parse(JSON.stringify(payload)) as NewCharacter;
    const temp = JSON.parse(JSON.stringify(temporaryAdjustments)) as Adjustments;
    const contentId = target.content.id;
    const comboCount = combo ? COMBO_THRESHOLD : 0;
    const seq = ++skillSeq;
    Promise.all(
      skills.map(async (s) => [s.id, await previewDamage(p, s.id, contentId, comboCount, temp)] as const),
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
        const main = await previewDamage(JSON.parse(pJson), sid, t.content.id, comboCount, JSON.parse(tempJson));
        const saved = simActive
          ? await previewDamage(sp, sid, t.content.id, comboCount, JSON.parse(tempJson))
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
    // 計算タブは「今このスキルで戦う」文脈なので、装備条件も選択中スキルの依存で判定する
    // (ホームはコンテンツごとの最大ダメージスキルで判定する)。
    const sid = skillId;
    if (evalHandle) clearTimeout(evalHandle);
    if (!pJson) {
      evals = [];
      return;
    }
    const seq = ++evalSeq;
    evalHandle = setTimeout(() => {
      evaluateContents(JSON.parse(pJson), sid || undefined)
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
  // 言葉はこの画面のもの、色は 6 系統から選ぶ(design-system §03)
  const BADGE: Badge[] = [
    { label: "余裕", state: "goal" },
    { label: "通る", state: "met" },
    { label: "ぎりぎり", state: "edge" },
    { label: "届かない", state: "short" },
    { label: "条件・火力とも未達", state: "unknown" },
    { label: "条件だけ未達", state: "temp" },
    { label: "判定中", state: "unknown" },
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
      { k: "ステ攻撃力", v: atkStat ?? 0, c: "var(--flow-base)", note: "素ステ・補正源から" },
      { k: "装備攻撃力", v: atkEquip ?? 0, c: "var(--flow-1)", note: "基本/強化 × 依存別係数" },
      { k: "装備攻撃力強化倍率", v: atkBonus, c: "var(--flow-2)", note: "パワーW・ストロングW" },
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
    const rows: FlowRow[] = [{ k: "抜けた分(素通り)", add: pierced, mult: "—", c: "var(--fg-dim)" }];
    for (const s of stepsMax) {
      if (FACTOR_STEPS.has(s.name)) {
        const next = running * s.value;
        rows.push({ k: s.name, add: next - running, mult: `×${s.value.toFixed(2)}`, c: FLOW_COLORS[s.name] ?? "var(--fg-dim)" });
        running = next;
      } else if (RUNNING_STEPS.has(s.name)) {
        rows.push({ k: s.name, add: s.value - running, mult: "—", c: FLOW_COLORS[s.name] ?? "var(--fg-dim)" });
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
  /** 上限で捨てられた分(生の合算値 − 上限適用後)。0 なら捨てていない */
  const catLoss = (c: (typeof activeCategories)[number]) => c.raw - c.value;
  const fmtCatRaw = (c: (typeof activeCategories)[number]) =>
    c.kind === "rate" ? `${c.raw >= 0 ? "+" : ""}${fmtNum(c.raw * 100)}%` : fmtNum(c.raw);
  const fmtCatLoss = (c: (typeof activeCategories)[number]) => {
    const loss = catLoss(c);
    return c.kind === "rate" ? `${fmtNum(loss * 100)}%` : fmtNum(loss);
  };
  const cappedCategories = $derived(activeCategories.filter((c) => catLoss(c) > 1e-9));

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
    if (result !== null && result.capped_loss.max > 0) {
      const before = result.per_hit.max + result.capped_loss.max;
      out.push({
        k: "ダメージ上限(1 段ごと)",
        raw: fmtInt(before),
        val: fmtInt(result.damage_cap),
        loss: fmtInt(result.capped_loss.max),
        kept: before > 0 ? result.per_hit.max / before : 1,
      });
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
        const want = ad.base * (1 - ad.reduction) * ad.combo_rate;
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
  let defenseSeq = 0;
  $effect(() => {
    // 防御側は対象コンテンツに依らない。キャラ(試し変更込み)が変わったときだけ引き直す
    const p = payload;
    if (!p) {
      defense = null;
      return;
    }
    const seq = ++defenseSeq;
    previewDefense(p)
      .then((d) => {
        if (seq === defenseSeq) {
          defense = d;
          defenseError = null;
        }
      })
      .catch((e) => {
        if (seq === defenseSeq) defenseError = errorMessage(e);
      });
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
      label: (p) => `武器エンチャント 突き ${fmtInt(p.equipment.parts.weapon.enchant.thrust)}`,
      get: (p) => String(p.equipment.parts.weapon.enchant.thrust),
      set: (p, v) => (p.equipment.parts.weapon.enchant.thrust = Number(v)),
    },
    {
      id: "weapon_enchant_slash",
      label: (p) => `武器エンチャント 斬り ${fmtInt(p.equipment.parts.weapon.enchant.slash)}`,
      get: (p) => String(p.equipment.parts.weapon.enchant.slash),
      set: (p, v) => (p.equipment.parts.weapon.enchant.slash = Number(v)),
    },
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
    simLimited = false;
  }

  // --- もし〜だったら ------------------------------------------------------
  interface WhatIf {
    candidate: Candidate;
    perHit: number;
    deltaPct: number;
  }
  let whatIf = $state<WhatIf[]>([]);
  /** 試した候補の数。0 件のときに「候補が無い」のか「超えるものが無い」のかを書き分ける */
  let whatIfTried = $state(0);
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
      whatIfTried = 0;
      return;
    }
    const seq = ++whatIfSeq;
    whatIfHandle = setTimeout(async () => {
      try {
        const current = JSON.parse(pJson) as NewCharacter;
        const list = candidatesFor(current, app.equipmentCatalog);
        // 1 候補の失敗(装備検証エラー等)で他候補まで消さない(独立レビュー指摘)
        const settled = await Promise.allSettled(
          list.map(async (candidate) => {
            const p = JSON.parse(pJson) as NewCharacter;
            candidate.apply(p);
            const r = await previewDamage(p, sid, t.content.id, combo ? COMBO_THRESHOLD : 0, JSON.parse(tempJson));
            return {
              candidate,
              perHit: r.per_hit.max,
              deltaPct: base > 0 ? Math.round((r.per_hit.max / base - 1) * 100) : 0,
            };
          }),
        );
        const rs = settled.flatMap((s) => (s.status === "fulfilled" ? [s.value] : []));
        if (seq === whatIfSeq) {
          whatIf = rs.filter((w) => w.perHit > base).sort((a, b) => b.perHit - a.perHit);
          whatIfTried = list.length;
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
  /**
   * バフの 3 状態(v4)。保存済みかどうかで「常時(マイセット)」と「追加枠」を分ける。
   * - `always`: キャラに保存済み = 毎回のっている常用セット
   * - `extra`: この計算だけの追加(試し変更。保存されない)
   * - `off`: 使わない(保存済みバフを一時的に外した場合も含む)
   * 常時への昇格は保存操作(「試し変更を保存」)で行う。チップのクリックで DB を書かない。
   */
  const buffState = (def: BuffDefinition): "always" | "extra" | "off" => {
    const saved = (savedPayload?.stat_sources.buffs.choices ?? []).some((c) => c.buff_id === def.id);
    if (!buffOn(def)) return "off";
    return saved ? "always" : "extra";
  };
  const BUFF_STATE_LABEL = { always: "常時", extra: "追加", off: "" } as const;
  const alwaysBuffCount = $derived(consumableBuffs.filter((d) => buffState(d) === "always").length);
  const extraBuffCount = $derived(consumableBuffs.filter((d) => buffState(d) === "extra").length);
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
  // 武器のエンチャント上限(カタログ item ならその上限、カスタム・未装備は equipment_value_max)
  const weaponEnchantCaps = $derived.by(() => {
    const weapon = payload?.equipment.parts.weapon;
    const item = weapon?.item_id ? app.equipmentCatalog.find((i) => i.id === weapon.item_id) : null;
    return {
      thrust: item?.enchant_caps.thrust ?? limits.equipment_value_max,
      slash: item?.enchant_caps.slash ?? limits.equipment_value_max,
    };
  });

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
        <DefensePanel profile={defense} error={defenseError} />
      {:else}
        <!-- 行ける?カード -->
        <div class="sheet">
          <div class="sheet-head">
            <span class="gem"></span>
            <span class="sheet-title">行ける？</span>
            <span class="sheet-char dim">{character.name}{calculating ? " ・ 計算中…" : ""}</span>
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
                  <Icon kind="skill" id={skill?.id ?? null} size={20} label={skill?.name ?? "スキル"} />
                  <span class="sk-name">{skill?.name ?? ""}</span>
                  {#if skills.length > 1}<span class="t-chev" class:rot={skillOpen}>▼</span>{/if}
                </span>
                <span class="sk-meta num dim">
                  ×{skill ? fmtNum(skill.multiplier) : "—"} ・ {skill?.hit_count ?? "—"}段 ・ Cri×{skill ? fmtNum(skill.critical_multiplier) : "—"}
                  {#if skill}・ {ELEMENT_LABELS[skill.element]}属性{/if}
                  {#if result?.accuracy_point != null}・ 命中P {fmtInt(result.accuracy_point)}{/if}
                </span>
              </button>
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
                    skillId = s.id;
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

          <!-- この一発 -->
          <div class="hero">
            <!-- 鎖(§14 決定 1)。1 発は**ゲート**(防御を抜けるか・目安を超えるかの閾値判定)、
                 DPS は**レート**(どれくらいの速さで削れるか)で、種類の違う量。ゲートを通らな
                 ければレートに意味が無いので、軸を切り替えず因果の順に繋ぐ。
                 判定(バッジ)はゲートの位置だけに置き、レートには付けない — 「何秒までなら
                 合格」の基準がゲーム側に存在しないので、付けたら嘘になる。
                 44px の主役数値は増やさない(金の帯 = 答えは 1 つ。§02)。鎖が右に伸びるだけ。 -->
            <div class="chain">
              <div class="node gate">
                <span class="nl">1 発</span>
                <span class="hero-num num nv" use:bump={() => perHit}>{perHit !== null ? fmtInt(perHit) : "—"}</span>
                {#if simActive}
                  <span class="nsub num">
                    <span class:up={deltaPct > 0} class:down={deltaPct < 0}>
                      {deltaPct === 0 ? "±0%" : `${deltaPct > 0 ? "+" : ""}${deltaPct}%`}
                    </span>
                    ・ 登録どおりなら {savedPerHit !== null ? fmtInt(savedPerHit) : "—"}
                  </span>
                {/if}
              </div>
              {#key badgeState}
                <span class="badge badge-in gatebadge" style={badgeStyle(BADGE[badgeState])}>{BADGE[badgeState].label}</span>
              {/key}
              <span class="op num">×{skill?.hit_count ?? 1} 段</span>
              <div class="node mid">
                <span class="nl">合計</span>
                <span class="num nv" use:bump={() => result?.total.max ?? null}>{result ? fmtInt(result.total.max) : "—"}</span>
                <span class="nsub num">
                  クリティカル ×{skill ? fmtNum(skill.critical_multiplier) : "—"}
                  {result ? fmtInt(result.total.critical) : "—"}
                  {#if result?.critical_rate}・ 発生 {result.critical_rate.value.toFixed(1)}%{/if}
                </span>
              </div>
              <span class="op num">÷ {result?.actual_delay ? result.actual_delay.value.toFixed(2) : "—"}s</span>
              <div class="node rate">
                <span class="nl">1 秒あたり</span>
                <span class="num nv" use:bump={() => (result?.dps ? Math.round(result.dps.max) : null)}>{result?.dps ? fmtInt(Math.round(result.dps.max)) : "—"}</span>
                <span class="nsub dim">
                  {#if result?.actual_delay}{Math.round(result.actual_delay.uses_per_minute)} 回/分 ・ {/if}判定は付けない
                </span>
              </div>
              <span class="op">→</span>
              <!-- 討伐時間。敵 HP を gamedata に持っていないので破線 +「—」で出す。
                   0 で埋めると画面が嘘をつく(§00 欠けを正常な状態として見せる) -->
              <div class="node pending">
                <span class="nl">討伐時間</span>
                <span class="num nv">— 秒</span>
                <span class="nsub dim">敵 HP が未収録</span>
              </div>
            </div>
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
            {#if skill && skill.base_actual_delay === null}
              <div class="delay-note dim">このスキルは wiki に基本中ディレイ(「動作」列)が無いため、1 秒あたりの火力を出せません。</div>
            {/if}
          </div>
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
                  <div class="band-row">
                    <span class="swatch" style="background: {f.c};"></span>
                    <span class="br-label" class:strong={topLever?.k === f.k} class:bad={f.add < 0}>{f.k}</span>
                    <span class="num br-mult dim">{f.mult}</span>
                    <span class="num br-val" class:bad={f.add < 0}>{f.add < 0 ? "−" : "+"}{fmtInt(Math.round(Math.abs(f.add)))}</span>
                    <span class="num br-share dim">{Math.round((Math.abs(f.add) / flowTotal) * 100)}%</span>
                  </div>
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
                        <span class="num lost-raw">{r.raw}</span>
                        <span class="lost-arrow dim">→ 上限</span>
                        <span class="num lost-val">{r.val}</span>
                        <span class="lost-bar" aria-hidden="true">
                          <i style="width: {(r.kept * 100).toFixed(1)}%"></i>
                          <i class="cut" style="width: {(100 - r.kept * 100).toFixed(1)}%"></i>
                        </span>
                        <span class="num lost-loss">{r.loss} は無効</span>
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
                <TracePanel trace={result.trace} {character} />
              {/if}
              </div>
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
          <label class="pw">
            <input
              type="checkbox"
              checked={payload.common_skills.power_weapon}
              onchange={(e) => {
                const v = e.currentTarget.checked;
                editSim((p) => (p.common_skills.power_weapon = v));
              }}
            />
            <span>パワーウェポン(+2%)</span>
          </label>
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
                bind:value={
                  () => payload.equipment.parts.weapon.enchant.thrust,
                  (v) => editSim((p) => (p.equipment.parts.weapon.enchant.thrust = v))
                }
              />
              <StatInput
                label={EQUIPMENT_STAT_LABELS.slash}
                min={0}
                max={weaponEnchantCaps.slash}
                bind:value={
                  () => payload.equipment.parts.weapon.enchant.slash,
                  (v) => editSim((p) => (p.equipment.parts.weapon.enchant.slash = v))
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
          <p class="buff-legend dim">
            <span class="lg always">常時</span> マイセット(キャラに保存済み・{alwaysBuffCount} 件)
            ／ <span class="lg extra">追加</span> この計算だけ({extraBuffCount} 件・保存されません)
            ／ 無印 使わない。<b>常時にするには「試し変更を保存」</b>。
          </p>
          <div class="buff-chips">
            {#each consumableBuffs as def (def.id)}
              {@const state = buffState(def)}
              {@const blocked = state === "off" && isBlocked(payload.stat_sources.buffs.choices, app.catalog, def)}
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
                {#if state !== "off"}<span class="chip-state">{BUFF_STATE_LABEL[state]}</span>{/if}
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
  .sheet { position: relative; border-radius: var(--r-window); border: 1px solid #687287; box-shadow: 0 1px 0 rgba(121, 140, 172, 0.4); background: var(--bg-field); }
  .sheet-head {
    display: flex; align-items: center; gap: 8px; padding: 7px 13px;
    border-radius: var(--r-window) 12px 0 0;
    background: linear-gradient(180deg, #F2E3BD, #DCC27E); border-bottom: 1px solid #BFA155;
  }
  .gem { flex-shrink: 0; width: 9px; height: 9px; transform: rotate(45deg); background: linear-gradient(160deg, #fff, #C9A227); border: 1px solid #A9821F; }
  .sheet-title { font-size: 11px; font-weight: 800; letter-spacing: 0.08em; color: #4A3C12; white-space: nowrap; }
  .sheet-char { min-width: 0; flex: 1; font-size: 9px; color: #6B5A24; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

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
  .pop-row.on { background: linear-gradient(180deg, #D9ECFF, #C2E1FF); }
  .pop-row .dot { width: 7px; height: 7px; flex-shrink: 0; border-radius: 50%; }
  /* 収録度(§14 決定 5)。破線 = 「まだ無い」の記号 */
  .coverage {
    flex-shrink: 0; padding: 0 6px; border-radius: var(--r-pill);
    border: 1px dashed var(--border); background: var(--bg-rail);
    font-size: 8.5px; font-weight: 700; color: var(--fg-muted); white-space: nowrap;
  }
  .pop-name { min-width: 0; flex: 1; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .pop-row.on .pop-name { font-weight: 700; }
  .pop-row .strong { font-weight: 700; }

  .skill-row { position: relative; z-index: 2; padding: 8px 11px 0; }
  .skill-trigger {
    width: 100%; display: flex; flex-direction: column; align-items: stretch; gap: 1px;
    padding: 5px 9px; border-radius: var(--r-panel); background: #F4F9FE; border: 1px solid #D6E2F0; text-align: left;
  }
  .skill-trigger:hover { background: var(--bg-rail); }
  .sk-line1 { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  .sk-name { min-width: 0; flex: 1; font-size: 11.5px; font-weight: 700; color: #3E2B26; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sk-meta { font-size: 8.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

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
  .sim-delta { margin-left: auto; font-size: 13px; font-weight: 700; color: var(--fg-dim); }
  .sim-delta.up { color: var(--good); }
  .sim-delta.down { color: var(--danger); }
  .sim-note-text { margin-top: 3px; font-size: 9px; line-height: 1.6; text-wrap: pretty; }
  .sim-actions { margin-top: 8px; display: flex; gap: 7px; }
  .sim-actions .btn { flex: 1; }

  .chips { display: flex; flex-wrap: wrap; gap: 5px; }
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

  .card-head { display: flex; align-items: center; gap: 8px; }
  .small { margin-left: auto; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  .pw { margin-top: 8px; display: flex; align-items: center; gap: 8px; font-size: 11.5px; cursor: pointer; }
  .pw input { accent-color: var(--accent); }
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
    background: linear-gradient(180deg, #D9ECFF, #C2E1FF); border-color: var(--accent); color: #123047;
    box-shadow: inset 0 1px 0 #fff;
  }

  .mat-note { margin: 7px 0 0; font-size: 9px; line-height: 1.6; }

  .buff-chips { margin-top: 8px; display: flex; flex-wrap: wrap; gap: 5px; }
  .buff-chip {
    padding: 4px 9px; border-radius: var(--r-pill);
    background: var(--bg-field); border: 1px solid var(--border-soft);
    font-size: 10px; font-weight: 500; color: var(--fg-muted); white-space: nowrap;
  }
  .buff-chip:hover:not(:disabled) { border-color: var(--accent); }
  .buff-chip.on {
    background: linear-gradient(180deg, #CCF7FF, #90D7FF);
    border-color: #687287; color: #123047; font-weight: 700;
  }
  /* 追加枠は「保存されない」ので、その専用色(--sim)にそろえる */
  .buff-chip.on.extra {
    background: linear-gradient(180deg, #fff, var(--state-temp-bg));
    border-color: var(--sim); color: var(--sim-fg);
  }
  .buff-chip .chip-state {
    margin-left: 5px; padding: 0 5px; border-radius: var(--r-pill);
    background: rgba(255, 255, 255, 0.75); border: 1px solid currentColor;
    font-size: 8.5px; font-weight: 700;
  }
  .buff-legend { margin: 7px 0 0; font-size: 9px; line-height: 1.7; }
  .buff-legend .lg {
    display: inline-block; padding: 0 5px; border-radius: var(--r-pill);
    font-size: 8.5px; font-weight: 700; border: 1px solid;
  }
  .buff-legend .lg.always { background: #CCF7FF; border-color: #687287; color: #123047; }
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

  .combo { display: flex; align-items: center; gap: 8px; padding: 2px 4px; font-size: 11.5px; cursor: pointer; }
  .combo input { accent-color: var(--accent); }

  .reqs { margin-top: 8px; display: flex; flex-direction: column; gap: 5px; }
  .req {
    display: flex; align-items: center; gap: 8px; padding: 6px 9px; border-radius: var(--r-panel);
    background: #F4F9FE; border: 1px solid var(--border-soft);
  }
  .req.ng { background: var(--state-short-bg); border-color: var(--state-short-bd); }
  .req-label { min-width: 0; flex: 1; font-size: var(--t-label); font-weight: 500; color: var(--fg-sub); white-space: nowrap; }
  .req.ng .req-label { color: var(--danger); }
  .req .num { font-size: 10px; white-space: nowrap; }
  .req-tag { flex-shrink: 0; font-size: 9.5px; font-weight: 700; color: var(--fg-sub); white-space: nowrap; }
  .req.ng .req-tag { color: var(--danger); }
</style>
