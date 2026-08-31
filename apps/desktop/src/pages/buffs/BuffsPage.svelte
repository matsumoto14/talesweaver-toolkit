<script lang="ts">
  import {
    buffTargetStatGains, createBuffSet, deleteBuffSet, duplicateBuffSet, errorMessage, previewDefense,
    previewEffectiveStats, setDefaultBuffSet, summarizeBuffSelection, updateBuffSet,
  } from "../../api/commands";
  import type {
    BuffChoice, BuffDamageEffect, BuffDefinition, BuffOrigin, BuffPurpose, BuffSet, BuffTarget,
    BuffTargetStatGain,
    DamageCategory, CategoryTrace, DefenseProfile, EffectiveStats, StatKind, StatSourceEffect,
  } from "../../api/types";
  import {
    BUFF_PURPOSES, isBlocked, isChoiceValue, isFixedValue, isMultiTarget, isPercentLayer, isRecordOnly,
    matchesPurpose,
    isUserSelectedTarget, pickedStats, toggleBuff, toggleBuffStat, userInputRange,
  } from "../../buffs";
  import { fmtInt, formatLayerValue, topRows, topRowsText, type TopRows } from "../../format";
  import { singleEffectLabel } from "../../characterSkills";
  import { STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS } from "../../labels";
  import { app, focusCharacterSource, payloadOf, refreshEvaluation, syncCalcBuffs, selectedCharacter, upsertCharacter } from "../../state.svelte";
  import { reportError, reportUndo } from "../../toast.svelte";
  import { bump, flash, swap } from "../../ui/motion.svelte";
  import StatInput from "../../ui/StatInput.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import StepToggle from "../../ui/StepToggle.svelte";
  import Spinner from "../../ui/Spinner.svelte";
  import { positionPopover } from "../../ui/popover";
  import Icon from "../../ui/Icon.svelte";

  const PURPOSES = BUFF_PURPOSES;
  const ORIGIN_LABELS: Record<BuffOrigin, string> = {
    item: "アイテム", event: "イベント", club: "クラブ", skill: "スキル",
    rune: "ルーン", soul_link: "ソウルリンク", battle_state: "戦闘中", minigame: "ミニゲーム",
  };
  type DamageGroup = "general" | "isabel" | "japan" | "other";
  const DAMAGE_GROUPS: { id: DamageGroup; label: string }[] = [
    { id: "general", label: "一般" },
    { id: "isabel", label: "イザベル" },
    { id: "japan", label: "日本独自" },
    { id: "other", label: "その他" },
  ];
  /** バフ未選択・キャラ未選択のときの `buff_stat_amplification` 初期値(全ステ 0) */
  const ZERO_STATS: EffectiveStats = Object.fromEntries(STAT_KINDS.map((kind) => [kind, 0])) as EffectiveStats;

  let selectedId = $state<number | null>(null);
  let newName = $state("");
  let saving = $state(false);
  let persisting = false;
  let pendingPersist: BuffSet | null = null;
  let confirmDeleteId = $state<number | null>(null);
  let confirmDeleteTimer: ReturnType<typeof setTimeout> | null = null;
  /** チップの「ほか n」を開いて中身(割愛したステ増分)を見ている ON バフの id。
   *  常に高々 1 件(§00 03: 押した場所は動かさない = 場所を固定した使い捨てのポップオーバー)。 */
  let openInfoId = $state<string | null>(null);
  /** 値の調整を開いている ON バフの id。「ほか n」と同じく高々 1 件で、同時には開かない
   *  (§00 03: 押した場所は動かさない = 場所を固定した使い捨ての重なりもの) */
  let openEditorId = $state<string | null>(null);
  let activePurpose = $state<BuffPurpose>("stats");
  let activeDamageGroup = $state<DamageGroup>("general");
  let damageSummary = $state<CategoryTrace[]>([]);
  let statAfter = $state<EffectiveStats | null>(null);
  let defenseBefore = $state<DefenseProfile | null>(null);
  let defenseAfter = $state<DefenseProfile | null>(null);
  let summaryLoading = $state(false);
  /** 選択中バフ 1 件ずつの実効果(チップの増分表示に使う)。previewEffectiveStats が返す
   *  source_effects(cap 込みで Σ が最終能力値 − 素ステに一致する帰属)の副産物なので、
   *  ここだけの別計算は作らない */
  let buffSourceEffects = $state<StatSourceEffect[]>([]);
  /** 選択中バフの固定値/割合増加が、倍率を持つ補正源(マスタリー等)に増幅されて最終能力値へ
   *  乗った分(ステ別)。バフ行の source_effects だけ足しても最終能力値の増分に届かない
   *  差の実体 — ドメイン(preview_effective_stats)が確定させて返す(フロントで引き算しない)。 */
  let buffStatAmplification = $state<EffectiveStats>(ZERO_STATS);
  /** ON かつ攻撃ダメージ効果を持つバフの、その 1 件ぶんの与ダメージカテゴリ配賦。
   *  能力値には出ない層(ダメージ n%・倍率)なので stat_contributions からは引けない。
   *  ドメイン(summarize_buff_selection)がカテゴリ上限適用後の値をバフへ配賦した結果を
   *  そのまま返すので、フロントで単独計算を組み立て直さない(Σ per-buff == カテゴリ合計)。 */
  let buffDamageEffects = $state<BuffDamageEffect[]>([]);
  /** 「対象ステを選ぶ」バフの、このキャラでの効き(ドメインが返す最終能力値の増分)。
   *  バフ id → ステごとの gain。並べ替えはこの値を見てフロントで行う。 */
  let targetStatGains = $state<Record<string, BuffTargetStatGain[]>>({});
  let gainsSeq = 0;
  let summarySeq = 0;
  const selected = $derived(app.buffSets.find((set) => set.id === selectedId) ?? app.buffSets[0] ?? null);
  const activePurposeMeta = $derived(PURPOSES.find((purpose) => purpose.id === activePurpose) ?? PURPOSES[0]);
  const damageCategories = (def: BuffDefinition): DamageCategory[] =>
    def.damage_effects.flatMap((effect) => effect !== "record_only" && "damage" in effect ? [effect.damage.category] : []);
  const matchesDamageGroup = (def: BuffDefinition, group: DamageGroup) => {
    const categories = damageCategories(def);
    if (group === "general") return categories.includes("attack_damage_general");
    if (group === "isabel") return categories.includes("attack_damage_isabel");
    if (group === "japan") return categories.includes("attack_damage_japan");
    return categories.some((category) => ![
      "attack_damage_general", "attack_damage_isabel", "attack_damage_japan",
    ].includes(category));
  };
  const activeDefinitions = $derived(app.catalog.filter((def) =>
    matchesPurpose(def, activePurpose) && (activePurpose !== "damage" || matchesDamageGroup(def, activeDamageGroup))
  ));

  $effect(() => {
    if (selectedId === null && app.buffSets.length > 0) selectedId = app.buffSets[0].id;
  });

  $effect(() => {
    const choices = selected ? JSON.parse(JSON.stringify(selected.choices)) : null;
    const character = selectedCharacter();
    const signature = `${selected?.id ?? "none"}:${JSON.stringify(choices)}:${character?.id ?? "none"}`;
    void signature;
    const seq = ++summarySeq;
    summaryLoading = true;
    void (async () => {
      if (!choices) {
        damageSummary = []; statAfter = null; defenseBefore = null; defenseAfter = null;
        buffSourceEffects = []; buffDamageEffects = []; buffStatAmplification = ZERO_STATS;
        summaryLoading = false;
        return;
      }
      try {
        const { categories, buff_effects } = await summarizeBuffSelection(choices);
        let afterStats: EffectiveStats | null = null;
        let beforeDefense: DefenseProfile | null = null;
        let afterDefense: DefenseProfile | null = null;
        let afterSourceEffects: StatSourceEffect[] = [];
        let afterAmplification: EffectiveStats = ZERO_STATS;
        if (character) {
          const draft = payloadOf(character);
          const [buffPreview, baseDefense, buffDefense] = await Promise.all([
            previewEffectiveStats(draft.base_stats, draft.stat_sources, draft.equipment, draft.common_skills, draft.awakening, draft.main_skill_id, choices),
            previewDefense(draft), previewDefense(draft, choices),
          ]);
          afterStats = buffPreview.stats;
          afterSourceEffects = buffPreview.source_effects;
          afterAmplification = buffPreview.buff_stat_amplification;
          beforeDefense = baseDefense; afterDefense = buffDefense;
        }
        if (seq !== summarySeq) return;
        damageSummary = categories; statAfter = afterStats;
        defenseBefore = beforeDefense; defenseAfter = afterDefense;
        buffSourceEffects = afterSourceEffects;
        buffStatAmplification = afterAmplification;
        buffDamageEffects = buff_effects;
      } catch (e) {
        if (seq === summarySeq) reportError(errorMessage(e));
      } finally {
        if (seq === summarySeq) summaryLoading = false;
      }
    })();
  });

  $effect(() => {
    const character = selectedCharacter();
    const choices = selected ? JSON.parse(JSON.stringify(selected.choices)) : null;
    const targets = app.catalog.filter((def) => isUserSelectedTarget(def.target));
    const seq = ++gainsSeq;
    if (!character || !choices || targets.length === 0) {
      targetStatGains = {};
      return;
    }
    void (async () => {
      try {
        const draft = payloadOf(character);
        const rows = await Promise.all(
          targets.map(async (def) => [def.id, await buffTargetStatGains(draft, choices, def.id)] as const),
        );
        if (seq !== gainsSeq) return;
        targetStatGains = Object.fromEntries(rows);
      } catch (e) {
        if (seq === gainsSeq) reportError(errorMessage(e));
      }
    })();
  });

  /** 対象ステの段を「このキャラで効く順」に並べる。効きが同じなら STAT_KINDS の順で安定させる。
   *  効きの計算はドメイン(buff_target_stat_gains)。ここでやるのは並べ替えと見せ方だけ。
   *
   *  ただし**複数ステを選ぶバフ(クラブ効果)は常に STAT_KINDS の順**で置く。効き順にすると、
   *  1 つ選んだ瞬間に残りの効きが変わって段が並び替わり、次に押したい段が別の場所へ動く
   *  (§00 03: 押した場所は動かない)。値の行(.per-stat)も STAT_KINDS 順なので、
   *  段と行の並びがそのまま対応する。 */
  function statOptionsFor(def: BuffDefinition) {
    if (isMultiTarget(def.target)) {
      return STAT_KINDS.map((kind) => ({ value: kind, label: STAT_LABELS[kind] }));
    }
    const gains = targetStatGains[def.id];
    const gainOf = (kind: StatKind) => gains?.find((g) => g.kind === kind)?.gain ?? null;
    const order = [...STAT_KINDS].sort((a, b) => {
      const ga = gainOf(a), gb = gainOf(b);
      if (ga === null || gb === null || ga === gb) return STAT_KINDS.indexOf(a) - STAT_KINDS.indexOf(b);
      return gb - ga;
    });
    return order.map((kind) => ({ value: kind, label: STAT_LABELS[kind] }));
  }
  /** 選んでも最終能力値が 1 も動かないステ(素ステが上限に張り付いている)。
   *  段は消さずに押せなくする — 消すと段の数が変わって幅が動く(§09 規則 4)。
   *  育成が進んで上限に余裕ができれば、そのまま押せるようになる。 */
  const cappedStats = (def: BuffDefinition): StatKind[] =>
    (targetStatGains[def.id] ?? []).filter((g) => g.gain === 0).map((g) => g.kind);
  const cappedTitle = (def: BuffDefinition) => (value: string) =>
    cappedStats(def).includes(value as StatKind)
      ? `${STAT_LABELS[value as StatKind]} は上限に達しているので、選んでも最終能力値は動きません`
      : undefined;
  /** ON にしたときの既定の対象ステ = いちばん効くもの(初期値は実用値、ux-guidelines) */
  const bestStatFor = (def: BuffDefinition): StatKind | undefined => {
    const gains = targetStatGains[def.id];
    if (!gains || gains.length === 0) return undefined;
    return [...gains].sort((a, b) => b.gain - a.gain)[0].kind;
  };

  function openEditor(def: BuffDefinition) {
    openInfoId = null;
    openEditorId = openEditorId === def.id ? null : def.id;
  }
  /** 重なりものは、外を押したときと Esc で閉じる。開いたままにすると「まだ開いている」ことを
   *  覚えておく必要が出る(§00 05)。トリガ自身とポップオーバーの中は対象外 */
  function closeOverlays(event: MouseEvent) {
    const target = event.target as HTMLElement | null;
    if (target?.closest(".rest-popover, .rest-link")) return;
    openInfoId = null;
    openEditorId = null;
  }

  function replaceSet(set: BuffSet) {
    const index = app.buffSets.findIndex((item) => item.id === set.id);
    if (index >= 0) app.buffSets[index] = set;
    else app.buffSets.push(set);
    selectedId = set.id;
  }

  async function create() {
    if (!newName.trim() || saving) return;
    saving = true;
    try {
      replaceSet(await createBuffSet(newName, { choices: [] }));
      newName = "";
    } catch (e) { reportError(errorMessage(e)); }
    finally { saving = false; }
  }

  async function persist(set: BuffSet) {
    // 数値欄は入力のたびに値が届く。通信中の入力を捨てず、最新のセット全体だけを
    // 続けて保存する。応答が古い内容で画面を巻き戻さないよう、先にローカルへ反映する。
    if (saving && !persisting) return;
    replaceSet(set);
    pendingPersist = JSON.parse(JSON.stringify(set));
    if (persisting) return;
    persisting = true;
    saving = true;
    try {
      while (pendingPersist) {
        const next = pendingPersist;
        pendingPersist = null;
        const saved = await updateBuffSet(next.id, next.name, next.choices);
        if (pendingPersist === null) replaceSet(saved);
        const affected = app.characters.filter((character) => character.default_buff_set_id === saved.id);
        await Promise.all(affected.map(refreshEvaluation));
      }
    }
    catch (e) { pendingPersist = null; reportError(errorMessage(e)); }
    finally { persisting = false; saving = false; }
  }

  /** バフの ON/OFF。ON にする瞬間から実用値(defaultChoice)で確定する — 「初期値は
   *  常に埋まっている」の原則(ux-guidelines)通り、適用ボタンは挟まない。
   *  値の調整が要るバフは、チップの下に最初から確保してある調整欄(.editor-slot)が
   *  ON のあいだだけ中身を見せる */
  async function toggle(def: BuffDefinition) {
    if (!selected || saving) return;
    if (openInfoId === def.id) openInfoId = null;
    if (openEditorId === def.id) openEditorId = null;
    const next: BuffSet = JSON.parse(JSON.stringify(selected));
    next.choices.choices = toggleBuff(next.choices.choices, def, !next.choices.choices.some((c) => c.buff_id === def.id), bestStatFor(def));
    await persist(next);
  }

  /** ON 中のバフの、いまの保存済み選択(値の調整フォームが直接読み書きする)。
   *  複数ステ対象(クラブ効果)は 1 ステ = 1 choice なので `stat` で引き当てる */
  const liveChoice = (def: BuffDefinition, stat?: StatKind): BuffChoice | null =>
    selected?.choices.choices.find(
      (c) => c.buff_id === def.id && (stat === undefined || c.stat === stat),
    ) ?? null;

  /** 値の調整フォームの入力。触った瞬間に確定する(§07: 適用ボタンを挟まない) */
  async function updateChoice(
    def: BuffDefinition,
    edit: (choice: BuffChoice) => void,
    stat?: StatKind,
  ) {
    if (!selected || saving) return;
    const next: BuffSet = JSON.parse(JSON.stringify(selected));
    const index = next.choices.choices.findIndex(
      (c) => c.buff_id === def.id && (stat === undefined || c.stat === stat),
    );
    if (index < 0) return;
    edit(next.choices.choices[index]);
    await persist(next);
  }

  /** 複数ステ対象バフの、1 ステぶんの ON/OFF。最後の 1 つを外すとバフ自体が OFF になる
   *  (「外す」と同じ結果になるので、外し方を 2 通り覚えさせない) */
  async function toggleStat(def: BuffDefinition, stat: StatKind, next: boolean) {
    if (!selected || saving) return;
    if (openInfoId === def.id) openInfoId = null;
    const draft: BuffSet = JSON.parse(JSON.stringify(selected));
    draft.choices.choices = toggleBuffStat(draft.choices.choices, def, stat, next);
    await persist(draft);
  }

  /** 複数ステ対象バフで、いま選ばれているステ */
  const chosenStats = (def: BuffDefinition): StatKind[] =>
    pickedStats(selected?.choices.choices ?? [], def);

  async function duplicate() {
    if (!selected || saving) return;
    saving = true;
    try { replaceSet(await duplicateBuffSet(selected.id)); }
    catch (e) { reportError(errorMessage(e)); }
    finally { saving = false; }
  }

  /** 確認中のセット。`confirmDeleteId` を直に読まず、いま消そうとしている実体で持つ —
   *  選択が動いても確認の対象は動かない(押した時点の対象を最後まで指す) */
  const pendingDelete = $derived(app.buffSets.find((set) => set.id === confirmDeleteId) ?? null);
  /** そのセットを「いつものバフ」にしているキャラ。削除で紐付けが外れる先 */
  const charactersUsing = (setId: number) =>
    app.characters.filter((character) => character.default_buff_set_id === setId);

  function requestRemove() {
    if (!selected || saving) return;
    confirmDeleteId = selected.id;
    if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
    // 対象名と影響を読んでから決める時間を取る(4 秒だと読み終わる前に消えていた)
    confirmDeleteTimer = setTimeout(() => (confirmDeleteId = null), 10000);
  }

  function cancelRemove() {
    if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = null;
    confirmDeleteId = null;
  }

  async function remove() {
    // 消すのは**確認したセット**。「いま選ばれているセット」で消すと、削除のあいだに
    // 選択が動いたときに別のものが消える(実機で本番セットを失った)
    const deleted = pendingDelete;
    if (!deleted || saving) return;
    if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = null;
    confirmDeleteId = null;
    saving = true;
    try {
      const deletedId = deleted.id;
      // 消える前に、戻すのに要るものを控える(名前・中身・「いつものバフ」にしていたキャラ)。
      // DB 側は ON DELETE SET NULL なので、キャラの紐付けは黙って外れる — 戻すときに
      // ここから付け直す
      const snapshot = { name: deleted.name, choices: JSON.parse(JSON.stringify(deleted.choices)) };
      const affectedIds = app.characters
        .filter((character) => character.default_buff_set_id === deletedId)
        .map((character) => character.id);
      // 削除後の選択は**消したものの隣**へ。先頭に飛ばすと、続けて「削除」を押したときに
      // 見ていたものと違うセットが消える(実機で本番セットを失った)
      const removedIndex = app.buffSets.findIndex((set) => set.id === deletedId);

      await deleteBuffSet(deletedId);
      app.buffSets = app.buffSets.filter((set) => set.id !== deletedId);
      const affected = new Set(affectedIds);
      app.characters = app.characters.map((character) =>
        affected.has(character.id) ? { ...character, default_buff_set_id: null } : character,
      );
      // 選択を動かすのは、消えたのがまさに見ていたセットだったときだけ
      if (selectedId === deletedId || selectedId === null) {
        const neighbour = app.buffSets[Math.min(removedIndex, app.buffSets.length - 1)];
        selectedId = neighbour?.id ?? null;
      }
      if (app.calcBuffSetId === deletedId) syncCalcBuffs(selectedCharacter());
      await Promise.all(app.characters.filter((character) => affected.has(character.id)).map(refreshEvaluation));
      reportUndo(`「${snapshot.name}」を削除しました`, () => restore(snapshot, affectedIds));
    } catch (e) { reportError(errorMessage(e)); }
    finally { saving = false; }
  }

  /** 削除したセットを作り直し、「いつものバフ」にしていたキャラへ付け直す。
   *  作り直しなので id は変わる — 参照はここで張り替える */
  async function restore(
    snapshot: { name: string; choices: BuffSet["choices"] },
    characterIds: number[],
  ) {
    try {
      const created = await createBuffSet(snapshot.name, snapshot.choices);
      replaceSet(created);
      const updated = await Promise.all(characterIds.map((id) => setDefaultBuffSet(id, created.id)));
      for (const character of updated) upsertCharacter(character);
      if (app.calcBuffSetId === null) syncCalcBuffs(selectedCharacter());
    } catch (e) { reportError(errorMessage(e)); }
  }

  const on = (def: BuffDefinition) => selected?.choices.choices.some((choice) => choice.buff_id === def.id) ?? false;
  const needsInput = (def: BuffDefinition) =>
    isUserSelectedTarget(def.target) || isChoiceValue(def.value) || userInputRange(def.value) !== null;
  const exclusive = (def: BuffDefinition) => def.exclusive_slots.length > 0 ? def.exclusive_slots.join(" / ") : "独立";

  function targetLabel(target: BuffTarget): string {
    if (target === "all_stats") return "全ステータス";
    if (target === "user_selected") return "選択したステータス";
    if (target === "user_selected_multi") return "選択したステータス(複数可)";
    if ("stat" in target) return STAT_LABELS[target.stat];
    return target.stats.map((stat) => STAT_LABELS[stat]).join(" / ");
  }

  function effectSummary(def: BuffDefinition): string {
    if (isFixedValue(def.value)) return `${STAT_LAYER_LABELS[def.layer]} ${formatLayerValue(def.layer, def.value.fixed)}`;
    if (isChoiceValue(def.value)) return def.value.choice.map((value) => formatLayerValue(def.layer, value)).join(" / ");
    const range = userInputRange(def.value);
    if (range) return `${STAT_LAYER_LABELS[def.layer]} ${formatLayerValue(def.layer, range.min)}〜${formatLayerValue(def.layer, range.max)}`;
    const damage = def.damage_effects.map(singleEffectLabel).filter((label): label is string => label !== null);
    return damage.join(" ・ ") || "効果を記録";
  }

  /** 選べない理由(同じ重複枠を占めている、いまセットに入っているバフの名前)。
   *  title を hover しなくても画面(チップ本体)から読めるように、ここを唯一の
   *  文面の出どころにして buffTooltip とチップの両方から使う。 */
  function blockingBuffNames(def: BuffDefinition): string {
    if (!selected || def.exclusive_slots.length === 0) return "";
    const slots = new Set(def.exclusive_slots);
    return selected.choices.choices
      .map((choice) => app.catalog.find((d) => d.id === choice.buff_id))
      .filter((d): d is BuffDefinition => d !== undefined && d.exclusive_slots.some((s) => slots.has(s)))
      .map((d) => d.name)
      .join(" / ");
  }
  function blockReason(def: BuffDefinition): string {
    const names = blockingBuffNames(def);
    return names ? `${names} と選べません` : "選択不可";
  }

  function buffTooltip(def: BuffDefinition, blocked: boolean): string {
    const purposes = def.purposes.map((purpose) => PURPOSES.find((item) => item.id === purpose)?.label ?? purpose).join(" / ");
    const lines = [def.name, `目的: ${purposes}`, `種類: ${ORIGIN_LABELS[def.origin]}`, `主効果: ${effectSummary(def)}`, `対象: ${targetLabel(def.target)}`];
    const damage = def.damage_effects.map(singleEffectLabel).filter((label): label is string => label !== null);
    if (damage.length > 0 && !isRecordOnly(def.value)) lines.push(`追加効果: ${damage.join(" ・ ")}`);
    lines.push(`重複: ${exclusive(def)}`);
    if (def.note) lines.push(`補足: ${def.note}`);
    if (blocked) lines.unshift(blockReason(def), "");
    return lines.join("\n");
  }

  // 目的タブの分子(purposeSelectedCount)は matchesPurpose が「複数の目的を持つバフ」を
  // 許すぶん重複して数えている(例: ステ+火力どちらの目的も持つバフは両方の分子に入る)。
  // そのため 3 タブの分子を足しても右列の summary の「n 件 ON」には一致しない。
  // これは意図的な仕様(ユーザー判断: 直さない) — 次に「合計が合わない」と気付いたときの
  // ためにここへ書いておく。
  const purposeSelectedCount = (purpose: BuffPurpose) =>
    app.catalog.filter((def) => matchesPurpose(def, purpose) && on(def)).length;
  const damageGroupSelectedCount = (group: DamageGroup) =>
    app.catalog.filter((def) => matchesPurpose(def, "damage") && matchesDamageGroup(def, group) && on(def)).length;

  function choosePurpose(purpose: BuffPurpose) {
    activePurpose = purpose;
  }

  function chooseDamageGroup(group: DamageGroup) {
    activeDamageGroup = group;
  }

  /** カテゴリ → 表示ラベル(damageSummary の CategoryTrace から引く)。buffDamageEffects 自体は
   *  カテゴリ ID しか持たないので、表示用のラベルだけこちらで引き当てる */
  const categoryLabel = $derived(new Map(damageSummary.map((c) => [c.category, c.label])));

  /** ON にしているバフの表示名の集合(source_effects の行がバフ由来かどうかの判定に使う。
   *  ペット/クラウン/マスタリー等の恒常補正の行と区別する) */
  const activeBuffNames = $derived(new Set(
    (selected?.choices.choices ?? [])
      .map((choice) => app.catalog.find((def) => def.id === choice.buff_id)?.name)
      .filter((name): name is string => name !== undefined),
  ));

  /** ステごとの「現在の効果」= チップに出しているのと同じバフ行(source_effects)の合計。
   *  倍率で増幅された分は別行(buffStatAmplification、ドメインが確定させた値)で出すので
   *  ここでは足さない。 */
  const statBuffTotal = (kind: StatKind) => buffSourceEffects
    .filter((c) => c.kind === kind && activeBuffNames.has(c.source))
    .reduce((sum, c) => sum + c.effect, 0);
  /** ステ別増幅のどれか 1 つでも動いていれば、見出しに但し書きを一度だけ添える(§00 05) */
  const hasAmplification = $derived(STAT_KINDS.some((kind) => buffStatAmplification[kind] !== 0));

  /** ON バフ 1 件ぶんの実効果。能力値に出る分は previewEffectiveStats の source_effects(cap 込みで
   *  Σ が最終能力値 − 素ステに一致する帰属)から引く — 上限反映後の実際の値なので、カタログの
   *  生値を手で再計算しない(写経しない)。攻撃ダメージ層にしか出ない分は
   *  buffDamageEffects(ドメインがカテゴリ上限適用後の値をバフへ配賦した結果)を足す。
   *  どちらも無ければ null(キャラ未選択、または本当に記録のみの層)。
   *  全ステに乗るバフは 7 件そのまま並べると欠けるので、効きの大きい上位 2 件 + ほか n に絞る
   *  (計算タブの buffContributionText と同じ topRowsText を使い、二重管理にしない)。 */
  /** ON バフ 1 件のステ増分行(ラベル・値のペア)。「ほか n」の中身を辿るポップオーバーと
   *  チップの要約行の両方がここから作る(値の出どころを 1 本にする)。 */
  function statRows(def: BuffDefinition): { label: string; value: number }[] {
    return buffSourceEffects
      .filter((c) => c.source === def.name && c.effect !== 0)
      .map((c) => ({ label: `${STAT_LABELS[c.kind]} ${c.effect >= 0 ? "+" : ""}${fmtInt(c.effect)}`, value: c.effect }));
  }
  /** ステ増分行を上位 2 件 + ほか n(の件数)に絞ったもの。「ほか n」は別枠のボタンとして
   *  出すので topRowsText(1 行テキスト化)ではなく構造化された topRows を使う。 */
  const statTop = (def: BuffDefinition): TopRows => topRows(statRows(def), 2);
  /** ON バフ 1 件のダメージ効果行。攻撃ダメージ効果は多くても 1〜2 件なので、そのまま並べる。 */
  function damageText(def: BuffDefinition): string | null {
    const damageRows = buffDamageEffects.filter((e) => e.buff_name === def.name && e.effect !== 0);
    if (damageRows.length === 0) return null;
    return damageRows
      .map((e) => `${categoryLabel.get(e.category) ?? e.category} ${e.effect >= 0 ? "+" : ""}${(e.effect * 100).toFixed(0)}%`)
      .join(" ・ ");
  }
  /** aria-label や値調整フォーム(.config-effect)向けの 1 行版。こちらは視覚的な幅制約が
   *  無い場所専用なので、ステ増分とダメージ効果を連結してよい(チップ本体はここを使わない —
   *  1 行に押し込むと溢れるため、テンプレート側で行を分けて出す)。 */
  function statDeltaText(def: BuffDefinition): string | null {
    const parts: string[] = [];
    const rows = statRows(def);
    if (rows.length > 0) parts.push(topRowsText(rows));
    const damage = damageText(def);
    if (damage) parts.push(damage);
    return parts.length > 0 ? parts.join(" ・ ") : null;
  }

  const formatDelta = (value: number, digits = 0) => `${value >= 0 ? "+" : ""}${value.toFixed(digits)}`;

  /** チップに出す効果の説明。「クリックして…」の操作ヒントとは別行にする(§00 05)。
   *  ON のチップは静的な定義文ではなく、このキャラで実際に何点(何%)伸びたかを出す
   *  (ユーザー合意: ON にしたチップに増分を出す)。OFF のチップは従来どおりカタログの説明 */
  function effectLine(def: BuffDefinition): string {
    if (on(def)) return statDeltaText(def) ?? effectSummary(def);
    const damage = def.damage_effects.map(singleEffectLabel).filter((label): label is string => label !== null);
    if (activePurpose === "damage" && damage.length > 0) return damage.join(" ・ ");
    return effectSummary(def);
  }
</script>

<svelte:window
  onclick={closeOverlays}
  onkeydown={(e) => { if (e.key === "Escape") { openInfoId = null; openEditorId = null; } }}
/>

<div class="buff-page">
  <aside class="sets">
    <div class="bar">バフセット <span use:bump={() => app.buffSets.length}>{app.buffSets.length}</span></div>
    <div class="create-row">
      <input bind:value={newName} disabled={saving} placeholder="セット名" aria-label="新しいバフセット名" onkeydown={(e) => e.key === "Enter" && create()} />
      <button class="btn primary" disabled={!newName.trim() || saving} onclick={create}>作成</button>
    </div>
    <div class="set-list">
      {#each app.buffSets as set (set.id)}
        <button class:on={selected?.id === set.id} disabled={saving} onclick={() => (selectedId = set.id)}>
          <span>{set.name}</span><small class="num">{set.choices.choices.length}</small>
        </button>
      {/each}
      {#if app.buffSets.length === 0}<p>セットを作ると、キャラや計算で使えます。</p>{/if}
    </div>
    <!-- 削除の確認は**消える対象(セット一覧)の隣**に出す。削除ボタンの真下だと目的タブに
         重なって押せなくなるうえ、何が消えるのかは対象から離れたところで読むことになる。
         リストの下は空きなので、出しても既にある行を動かさない(§09 規則 2) -->
    {#if pendingDelete}
      {@const users = charactersUsing(pendingDelete.id)}
      <div class="delete-confirm" role="alert">
        <strong>「{pendingDelete.name}」を削除します</strong>
        {#if users.length > 0}
          <small>{users.map((character) => character.name).join(" / ")} の「いつものバフ」が外れます</small>
        {/if}
        <div class="confirm-actions">
          <button class="btn danger" disabled={saving} onclick={remove}>削除する</button>
          <button class="btn" disabled={saving} onclick={cancelRemove}>やめる</button>
        </div>
      </div>
    {/if}
  </aside>

  <section class="catalog">
    <div class="bar">セットに入れるバフ</div>
    {#if selected}
      <div class="set-tools">
        <input value={selected.name} disabled={saving} aria-label="バフセット名" onchange={(e) => persist({ ...selected, name: e.currentTarget.value })} />
        <button class="btn" onclick={duplicate}>複製</button>
        <button class="btn danger delete-set" disabled={saving} onclick={requestRemove}>削除</button>
      </div>
      <div class="groups">
        <div class="category-switch" role="tablist" aria-label="伸ばしたい効果">
          {#each PURPOSES as purpose (purpose.id)}
            {@const definitions = app.catalog.filter((def) => matchesPurpose(def, purpose.id))}
            {@const picked = purposeSelectedCount(purpose.id)}
            <button
              class="chip category-tab"
              class:on={activePurpose === purpose.id}
              role="tab"
              aria-selected={activePurpose === purpose.id}
              onclick={() => choosePurpose(purpose.id)}
            >
              <span>{purpose.label}</span>
              <span class="group-count num" use:bump={() => picked}>{picked}/{definitions.length}</span>
            </button>
          {/each}
        </div>
        <!-- 面の入れ替えは型 3b(swap-in = 上から短く入る)。型 5 の badge-in(flash)を
             ここに使うと、面ぜんたいが中心から膨らんで他タブの切り替えと動きが揃わない -->
        <section class="buff-group" use:swap={() => `${activePurpose}:${activeDamageGroup}`}>
          <div class="group-summary">
            <span class="group-copy"><strong>{activePurposeMeta.label}</strong><small>{activePurposeMeta.description}</small></span>
            {#if activePurpose === "stats"}
              <button class="guide-link" type="button" onclick={() => focusCharacterSource("commonSkill")}>
                アンリーシュはキャラ設定 <span aria-hidden="true">›</span>
              </button>
            {/if}
          </div>
          {#if activePurpose === "damage"}
            <div class="damage-switch" role="tablist" aria-label="攻撃ダメージの種類">
              {#each DAMAGE_GROUPS as group (group.id)}
                {@const definitions = app.catalog.filter((def) => matchesPurpose(def, "damage") && matchesDamageGroup(def, group.id))}
                {@const picked = damageGroupSelectedCount(group.id)}
                <button
                  class="chip damage-tab"
                  class:on={activeDamageGroup === group.id}
                  role="tab"
                  aria-selected={activeDamageGroup === group.id}
                  onclick={() => chooseDamageGroup(group.id)}
                >
                  <span>{group.label}</span>
                  <span class="group-count num" use:bump={() => picked}>{picked}/{definitions.length}</span>
                </button>
              {/each}
            </div>
          {/if}
          <div class="chips">
            {#each activeDefinitions as def (def.id)}
              {@const blocked = !on(def) && isBlocked(selected.choices.choices, app.catalog, def)}
              {@const isOn = on(def)}
              {@const hasEditor = needsInput(def)}
              {@const top = isOn ? statTop(def) : null}
              {@const dmg = isOn ? damageText(def) : null}
              {@const openHere = openInfoId === def.id || openEditorId === def.id}
              <!-- チップの大きさは ON/OFF で変えない。値の調整も増分の内訳も**重ねて**出す
                   (§09 規則 3: 重なるものはレイアウトを押さない / 閉じたときに何も動かない)。
                   チップ自体も動かさない — ON/OFF が変わったことは中の状態バッジが弾んで伝える
                   (§10 型 5「行そのものは動かさない」) -->
              <div class="buff-option" class:on={isOn} class:info-open={openHere}>
                <span class="buff-icon"><Icon kind="buff" id={def.id} size={28} label={def.name} /></span>
                <!-- 「ほか n」「設定」を独立したボタンにするため、チップ本体はネイティブ button
                     ではなく role="button" の div にする(button の中に button は入れられない)。
                     クリック・キー操作の意味は button と同じに保つ。 -->
                <div
                  class="buff-toggle"
                  class:disabled={blocked || saving}
                  role="button"
                  tabindex={blocked || saving ? -1 : 0}
                  aria-disabled={blocked || saving}
                  aria-pressed={isOn}
                  onclick={() => { if (!blocked && !saving) toggle(def); }}
                  onkeydown={(e) => {
                    if ((e.key === "Enter" || e.key === " ") && !blocked && !saving) { e.preventDefault(); toggle(def); }
                  }}
                  title={buffTooltip(def, blocked)}
                  aria-label={`${def.name}。${effectLine(def)}`}
                >
                  <span class="chip-copy">
                    <span class="chip-head">
                      <strong>{def.name}</strong>
                      <!-- ON / 選べない の状態バッジ。枠は 3 状態すべてで常に確保し(空のときは
                           透明)、ここだけ見れば ON・OFF・選択不可の判別が付くようにする
                           (§00 05・03: バッジが出た瞬間に幅が変わって隣が動くのを防ぐ) -->
                      <span
                        class="chip-state"
                        class:on={isOn}
                        class:blocked
                        use:flash={() => (isOn ? "on" : blocked ? "blocked" : "off")}
                      >{isOn ? "選択中" : blocked ? "選択不可" : ""}</span>
                    </span>
                    {#if isOn && top}
                      <!-- ON: このキャラで実際に何点伸びたかを行ごとに出す(§00 05)。ステ増分と
                           ダメージ効果は別行 — 1 行に連結すると長い名前のダメージ効果で溢れる
                           (5周目 実機指摘)。行の右端に開く的を 1 つだけ置く:
                           値の調整が要るバフは「設定」(調整と内訳を兼ねる)、
                           それ以外で割愛した増分があるときだけ「ほか n」。
                           2 つ並べると 288px に収まらないうえ、押し分けを迫ることになる。 -->
                      <span class="chip-effect" use:flash={() => effectLine(def)}>
                        {#if top.shown.length > 0}
                          <small class="chip-effect-row">
                            <span class="chip-effect-values">{top.shown.join(" / ")}</span>
                            {#if hasEditor}
                              <!-- トグル面の中央に押し分けの要る的を置かない — 行の右端に、縦の区切りで
                                   「ここだけ別の的」と分かるようにする(実機で誤タップ報告あり)。 -->
                              <button
                                type="button"
                                class="rest-link"
                                onclick={(e) => { e.stopPropagation(); openEditor(def); }}
                                aria-expanded={openEditorId === def.id}
                              >設定</button>
                            {:else if top.restCount > 0}
                              <button
                                type="button"
                                class="rest-link"
                                onclick={(e) => { e.stopPropagation(); openInfoId = openInfoId === def.id ? null : def.id; openEditorId = null; }}
                                aria-expanded={openInfoId === def.id}
                              >ほか {top.restCount}</button>
                            {/if}
                          </small>
                        {/if}
                        {#if dmg}<small>{dmg}</small>{/if}
                        {#if top.shown.length === 0 && !dmg}<small>{effectSummary(def)}</small>{/if}
                      </span>
                    {:else if blocked}
                      <!-- 選べない理由をここに出す(title 無しで読めるように)。同じ
                           chip-effect の枠を使い、OFF 単独のときの説明文と入れ替える形にして
                           新しい行を増やさない(チップの高さを崩さない) -->
                      <span class="chip-effect"><small class="block-reason" use:flash={() => blockReason(def)}>{blockReason(def)}</small></span>
                    {:else}
                      <span class="chip-effect"><small use:flash={() => effectLine(def)}>{effectLine(def)}</small></span>
                    {/if}
                    <span class="origin-badge">{ORIGIN_LABELS[def.origin]}</span>
                  </span>
                  {#if isOn && top && openInfoId === def.id}
                    {@const restRows = statRows(def)}
                    <!-- 「ほか n」の中身。押した場所(チップ)の直下に出し、レイアウトは押さない
                         (絶対配置なので下のチップを動かさない)。 -->
                    <div
                      class="popover rest-popover"
                      role="dialog"
                      tabindex="-1"
                      aria-label={`${def.name} の全ステ増分`}
                      use:positionPopover
                      onclick={(e) => e.stopPropagation()}
                      onkeydown={(e) => e.stopPropagation()}
                    >
                      {#each restRows as row (row.label)}
                        <div class="num">{row.label}</div>
                      {/each}
                      <button type="button" class="popover-close" onclick={(e) => { e.stopPropagation(); openInfoId = null; }}>閉じる</button>
                    </div>
                  {/if}
                  {#if isOn && hasEditor && openEditorId === def.id}
                    {@const restRows = statRows(def)}
                    <!-- 値の調整。**チップに重ねて**出すのでレイアウトを押さない(§09 規則 3)。
                         適用ボタンは無く、触った瞬間に確定する(§07)。割愛した増分の内訳も
                         ここに入れて、押す的を 1 つに保つ。 -->
                    <div
                      class="popover rest-popover editor-popover"
                      role="dialog"
                      tabindex="-1"
                      aria-label={`${def.name} の設定`}
                      use:positionPopover
                      onclick={(e) => e.stopPropagation()}
                      onkeydown={(e) => e.stopPropagation()}
                    >
                      <div class="choice-editor">
                        {#if isMultiTarget(def.target)}
                          <!-- クラブエフェクトはステごとに 1 つずつ、別々の段で併用できる
                               (wiki: クラブ)。段の並びと押せる段はドメインの「効き」から決める -->
                          {@const picked = chosenStats(def)}
                          {@const range = userInputRange(def.value)}
                          <StepToggle
                            label="対象ステ"
                            options={statOptionsFor(def)}
                            cols={STAT_KINDS.length}
                            max={STAT_KINDS.length}
                            values={picked}
                            disabled={saving}
                            disabledValues={cappedStats(def)}
                            titleFor={cappedTitle(def)}
                            onToggle={(value, next) => toggleStat(def, value as StatKind, next)}
                          />
                          {#if range}
                            {@const scale = isPercentLayer(def.layer) ? 100 : 1}
                            <div class="per-stat">
                              {#each picked as stat (stat)}
                                <StatInput
                                  label={STAT_LABELS[stat]}
                                  min={range.min * scale}
                                  max={range.max * scale}
                                  bind:value={() => (liveChoice(def, stat)?.value ?? def.default_value ?? range.min) * scale,
                                    (value) => updateChoice(def, (c) => (c.value = value / scale), stat)}
                                />
                              {/each}
                            </div>
                          {/if}
                        {:else}
                          {#if isUserSelectedTarget(def.target)}
                            <StepSelect label="対象ステ" options={statOptionsFor(def)} cols={STAT_KINDS.length} disabledValues={cappedStats(def)} bind:value={() => liveChoice(def)?.stat ?? STAT_KINDS[0], (value) => updateChoice(def, (c) => (c.stat = value as StatKind))} />
                          {/if}
                          {#if isChoiceValue(def.value)}
                            {@const options = def.value.choice.map((value, index) => ({ value: String(index), label: formatLayerValue(def.layer, value) }))}
                            <StepSelect label="段階" {options} bind:value={() => String(liveChoice(def)?.choice_index ?? 0), (value) => updateChoice(def, (c) => (c.choice_index = Number(value)))} />
                          {/if}
                          {#if userInputRange(def.value)}
                            {@const range = userInputRange(def.value)!}
                            {@const scale = isPercentLayer(def.layer) ? 100 : 1}
                            <StatInput label={isPercentLayer(def.layer) ? "値 (%)" : "値"} min={range.min * scale} max={range.max * scale} bind:value={() => (liveChoice(def)?.value ?? def.default_value ?? range.min) * scale, (value) => updateChoice(def, (c) => (c.value = value / scale))} />
                          {/if}
                        {/if}
                      </div>
                      {#if top && restRows.length > top.shown.length}
                        <!-- チップに出し切れなかった増分。「ほか n」を別の的にせず、ここに畳む -->
                        <div class="editor-rows">
                          {#each restRows as row (row.label)}
                            <div class="num">{row.label}</div>
                          {/each}
                        </div>
                      {/if}
                      <button type="button" class="popover-close" onclick={(e) => { e.stopPropagation(); openEditorId = null; }}>閉じる</button>
                    </div>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </section>
      </div>
    {:else}
      <div class="empty">左でバフセットを作ってください。</div>
    {/if}
  </section>

  <aside class="summary">
    <div class="bar">現在の効果<Spinner active={summaryLoading} label="バフの効果を集計しています" /></div>
    {#if selected}
      <!-- 跳ねるのは**変わった数字だけ**(§10 型 1)。カードに use:bump を付けると
           scale(1.07) が面ごと掛かり、カードの幅が 264 → 282px に膨らんで戻る。
           数字側は 2 桁ぶんの幅を先に取ってあるので、桁が増えても「件 ON」は動かない -->
      <div class="count">
        <span class="count-value num" use:bump={() => selected.choices.choices.length}>{selected.choices.choices.length}</span><small>件 ON</small>
      </div>
      <div class="summary-block">
        <div class="summary-head">
          <strong>ステータス</strong>
          {#if hasAmplification}<small class="amp-caption">( )は他の補正と重なって増えた分</small>{/if}
        </div>
        {#if statAfter}
          <div class="summary-grid stats-grid">
            {#each STAT_KINDS as kind}
              {@const delta = statBuffTotal(kind)}
              {@const amp = buffStatAmplification[kind]}
              {@const total = delta + amp}
              <span>{STAT_LABELS[kind]}</span>
              <span class:positive={total > 0} class="num" use:bump={() => total}
              >{formatDelta(delta)}{#if amp !== 0}<span class="amp-value"> ({formatDelta(amp)})</span>{/if}</span>
            {/each}
          </div>
        {:else}<p>キャラを選ぶと、実際に何点伸びるか表示します。</p>{/if}
      </div>
      <div class="summary-block">
        <strong>攻撃ダメージ</strong>
        {#if damageSummary.length > 0}
          <div class="damage-list">
            {#each damageSummary as row (row.category)}
              <div><span>{row.label}</span><span class="num positive" use:bump={() => row.value}>+{(row.value * 100).toFixed(0)}%</span></div>
              {#if row.raw > row.value}<small>上限で {((row.raw - row.value) * 100).toFixed(0)}% は未反映</small>{/if}
            {/each}
          </div>
        {:else}<p>選択中のバフによる攻撃ダメージ増加はありません。</p>{/if}
      </div>
      <div class="summary-block">
        <strong>耐久</strong>
        {#if defenseBefore && defenseAfter}
          {@const physical = defenseAfter.physical_defense - defenseBefore.physical_defense}
          {@const magic = defenseAfter.magic_defense - defenseBefore.magic_defense}
          {@const composite = defenseAfter.composite_defense - defenseBefore.composite_defense}
          {@const evasion = (defenseAfter.combo_evasion - defenseBefore.combo_evasion) * 100}
          <div class="summary-grid">
            <!-- 変わったのは数値なので、動かすのは数値だけ(§10 型 1)。ブロックごと
                 badge-in で膨らませると、どの行が動いたのか読めなくなる -->
            <span>物理防御力</span><span class="num" use:bump={() => physical}>{formatDelta(physical)}</span>
            <span>魔法防御力</span><span class="num" use:bump={() => magic}>{formatDelta(magic)}</span>
            <span>複合防御力</span><span class="num" use:bump={() => composite}>{formatDelta(composite)}</span>
            <span>コンボ回避</span><span class="num" use:bump={() => evasion}>{formatDelta(evasion, 1)}%</span>
          </div>
        {:else}<p>キャラを選ぶと、防御力などの変化を表示します。</p>{/if}
        {#if selected.choices.choices.some((choice) => choice.buff_id === "boiled_mimic")}
          <div class="unmodeled">被ダメージ -30%（記録のみ・耐久計算には未反映）</div>
        {/if}
      </div>
    {/if}
  </aside>
</div>

<style>
  .buff-page { flex: 1; min-height: 0; display: grid; grid-template-columns: 250px minmax(360px, 1fr) 290px; gap: 10px; padding: 12px; overflow: auto; }
  .sets, .catalog, .summary { min-height: 0; background: var(--bg-panel); border: 1px solid var(--border); border-radius: var(--r-panel); overflow: hidden; box-shadow: inset 0 1px #fff; }
  .catalog { display: flex; flex-direction: column; }
  .bar { height: 32px; padding: 0 12px; display: flex; align-items: center; justify-content: space-between; background: var(--head-bar); color: #fff; font-size: 11px; font-weight: 700; letter-spacing: .08em; }
  .create-row, .set-tools { display: flex; gap: 7px; padding: 10px; border-bottom: 1px solid var(--border-soft); }
  .set-tools { position: relative; }
  input { min-width: 0; height: 30px; flex: 1; border: 1px solid var(--border); border-radius: var(--r-inset); padding: 0 9px; background: var(--bg-field); color: var(--fg); }
  .set-list { padding: 8px; display: flex; flex-direction: column; gap: 4px; }
  .set-list > button { display: flex; align-items: center; gap: 8px; width: 100%; padding: 8px 9px; border: 1px solid transparent; border-radius: var(--r-inset); text-align: left; }
  .set-list > button.on { background: var(--sel-card); border-color: var(--sel-bd); }
  .set-list small { margin-left: auto; min-width: 2ch; text-align: right; }
  .set-list p, .empty { margin: 12px; color: var(--fg-muted); font-size: 11px; }
  .groups { flex: 1; min-height: 0; padding: 8px; display: flex; flex-direction: column; gap: 7px; overflow: hidden; }
  .category-switch { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 5px; }
  .category-tab { min-width: 0; width: 100%; justify-content: flex-start; border-radius: var(--r-inset); }
  .category-tab > span:first-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .buff-group { flex: 1; min-height: 0; display: flex; flex-direction: column; border: 1px solid var(--border-soft); border-radius: var(--r-panel); background: var(--surface-inset); box-shadow: inset 0 1px #fff; overflow: hidden; }
  .group-summary { min-height: 41px; padding: 6px 9px; display: flex; align-items: center; gap: 10px; }
  .group-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; }
  .group-summary small { color: var(--fg-muted); font-size: 9px; }
  .group-count { margin-left: auto; min-width: 5ch; color: inherit; text-align: right; font-size: 9px; }
  .damage-switch { padding: 0 7px 7px; display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 5px; }
  .damage-tab { min-width: 0; width: 100%; justify-content: flex-start; border-radius: var(--r-inset); }
  .guide-link { flex: none; padding: 2px 4px; color: var(--fg-muted); font-size: 9px; text-decoration: underline; text-underline-offset: 2px; }
  .guide-link:hover { color: var(--accent-hover); }
  /* チップ 1 枚の高さは**中身より先に**ここで決める(§09 規則 4)。中身の積み上げに任せると、
     効果が 1 行のバフと 2 行のバフ、バッジが出るチップと出ないチップで数 px ずつ食い違い、
     ON/OFF のたびに下がずれる。高さは「何が入るか」から式で置く — 数字を実測に合わせて
     いじると、行が 1 本増えたときに同じことを繰り返す。 */
  .chips {
    /* 効果は最大 2 行(ステ増分 + ダメージ効果)。1 行 13px + 行間 2px */
    --chip-effect-h: 28px;
    /* 名前の行 + 効果 + 種類バッジ(margin 込み) + トグルの上下余白と枠線 */
    --chip-h: calc(16px + var(--chip-effect-h) + 19px + 14px);
    flex: 1; min-height: 0; padding: 7px; display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    grid-auto-rows: var(--chip-h); gap: 6px;
    border-top: 1px solid var(--border-soft);
    align-content: start; overflow-y: auto; scrollbar-gutter: stable;
  }
  .buff-option { position: relative; min-width: 0; height: 100%; border: 1px solid var(--border); border-radius: var(--r-panel); background: var(--bg-field); overflow: hidden; }
  .buff-option.on { border-color: var(--sel-bd); background: var(--sel-card); box-shadow: inset 0 0 0 1px var(--sel-bd); }
  /* 重なりものはチップの外(下)へはみ出すので、開いている間だけ overflow:hidden を外す。
     チップ一覧そのもの(.chips)のスクロール枠を越える分は ui/popover.ts が fixed に
     置き換えて逃がす(§00 03: 押した場所は動かさない = 隣のチップは押し出さない)。 */
  .buff-option.info-open { overflow: visible; z-index: 7; }
  .buff-icon { position: absolute; z-index: 1; top: 16px; left: 7px; width: 28px; height: 28px; transition: top .2s ease; }
  .buff-toggle { position: relative; width: 100%; height: 100%; padding: 6px 7px 6px 48px; display: flex; align-items: center; gap: 7px; border: 0; background: transparent; color: var(--fg); text-align: left; cursor: pointer; }
  .buff-toggle.disabled { opacity: .45; cursor: default; }
  /* ステ増分・ダメージ効果の行を積む場所。**2 行ぶんの高さを常に取る** — ON にした瞬間に
     ダメージ効果の行が増えて 9px 伸びると、下のチップが動く(§09 規則 4: サイズはデータが
     来る前に決まっている)。OFF・1 行・2 行のどれでも同じ高さで、中身だけ変わる。 */
  /* 効果の行。枠は先に決まっていて、中身が 0 行でも 2 行でもここが伸び縮みしない */
  .chip-effect { flex: none; height: var(--chip-effect-h); display: flex; flex-direction: column; justify-content: center; gap: 2px; overflow: hidden; }
  /* ステ増分の値と「ほか n」を同じ行の左右に離す。値はトグル本体の一部(押すと ON/OFF)、
     「ほか n」は別の的(押すとポップオーバー)なので、中央で押し分けさせない — 右端に寄せ、
     縦の区切り線で「ここだけ別」と分かるようにする(実機で中央を押して誤爆した報告あり)。 */
  .chip-effect-row { display: flex; align-items: baseline; gap: 6px; }
  .chip-effect-values { flex: 1; min-width: 0; }
  /* 「ほか n」= 割愛した増分の中身を辿るボタン。チップ本体のトグルとは別の押せる要素だと
     分かるよう下線を付け、チップの色そのものは変えない(§00 03 と衝突させない)。
     margin-left: auto で行の右端に固定し、border-left の区切りと左パディングで
     独立した的として十分な広さを確保する(高さ・幅はチップ側を変えない)。 */
  .rest-link {
    flex-shrink: 0; align-self: stretch; display: inline-flex; align-items: center;
    margin-left: auto; padding: 0 0 0 8px; border: 0; border-left: 1px solid var(--border-soft);
    background: none; color: var(--accent); font: inherit; text-decoration: underline; text-underline-offset: 2px;
    white-space: nowrap; cursor: pointer;
  }
  .rest-link:hover { color: var(--accent-hover); }
  /* チップの直下・チップ幅に合わせて出す(面そのものは app.css の .popover) */
  .rest-popover { top: calc(100% + 4px); left: 7px; right: 7px; }
  /* 値の調整。「ほか n」と同じ重なりもの(.rest-popover)に乗せる — 幅はチップに合わせ、
     中身だけ差し替える。レイアウトは押さないので、開いても閉じても何も動かない */
  .editor-popover { gap: 7px; }
  .editor-rows { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 3px 10px; padding-top: 6px; border-top: 1px solid var(--border-soft); }
  .choice-editor { padding: 7px; display: flex; flex-direction: column; gap: 7px; border: 1px solid var(--border-soft); border-radius: var(--r-inset); background: var(--bg-field); box-shadow: inset 0 1px #fff; }
  /* 選んだステごとの値。**1 ステ 1 行**で積む — 2 列に畳むとチップ幅(272px)では
     ラベル・数値欄・MAX が重なって読めなかった(実機報告)。行が増えて伸びた分は
     ui/popover.ts が置き直す(上に開く / 収まる高さでスクロール)ので、下のチップは動かない */
  .per-stat { display: grid; grid-template-columns: minmax(0, 1fr); gap: 6px; }
  .chip-copy { min-width: 0; flex: 1; height: 100%; display: flex; flex-direction: column; justify-content: center; }
  .chip-copy strong, .chip-copy small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chip-head { flex: none; height: 16px; display: flex; align-items: center; gap: 6px; }
  .chip-head strong { min-width: 0; flex: 1; }
  /* ON / 選べない のバッジ。3 状態(ON・OFF・選べない)のどれでも同じ場所を占めるよう、
     OFF のときも要素自体は出したまま透明にする(枠だけ確保) — CalcPage の「常/追」バッジ
     (chip-state)と同じ手当て。ON は「セットに保存される」の水色(--sel、他の ON チップと
     同系色)、選べないは新しい色を作らず §03 の状態 6 系統の unknown(対象外・判定不能)を使う。 */
  .chip-state {
    /* 高さは chip-head(16px 固定)が決める。文字が入るかどうかで動かない */
    flex-shrink: 0; display: flex; align-items: center; justify-content: center;
    min-width: 46px; height: 100%; padding: 0 6px; border-radius: var(--r-pill);
    border: 1px solid transparent; background: transparent; color: transparent;
    font-size: 8.5px; font-weight: 700; text-align: center; white-space: nowrap;
  }
  .chip-state.on { background: var(--sel); border-color: var(--sel-bd); color: var(--sel-fg); }
  .chip-state.blocked { background: var(--state-unknown-bg); border-color: var(--state-unknown-bd); color: var(--state-unknown-fg); }
  /* チップ本体の要約行(.chip-effect 内)だけは、省略記号で切らず自然に折り返す — 幅で
     症状を消すのではなく、行を分けて出す量そのものを収める(5周目 実機指摘)。 */
  .chip-effect small { overflow: visible; text-overflow: clip; white-space: normal; }
  .block-reason { color: var(--state-unknown-fg) !important; }
  .chip-copy strong { font-size: 10px; }
  .chips small { color: var(--fg-muted); font-size: 9px; }
  /* 効果値とは別行の操作ヒント。同じ行に混ぜない(§00 05 考えさせない) */
  .origin-badge { flex: none; align-self: flex-start; margin-top: 3px; padding: 1px 6px; border: 1px solid var(--border-soft); border-radius: var(--r-pill); background: var(--surface-inset); color: var(--fg-muted); font-size: 8.5px; line-height: 1.4; white-space: nowrap; }
  .summary { background: var(--bg-raised); }
  .count { margin: 12px; padding: 13px; display: flex; align-items: baseline; border: 1px solid var(--border); border-radius: var(--r-inset); background: var(--surface-inset); box-shadow: inset 0 1px #fff; font-size: 27px; font-weight: 700; }
  .count-value { min-width: 2ch; text-align: right; }
  .count small { margin-left: 6px; font-family: var(--font); font-size: 10px; font-weight: 500; }

  .summary-block { margin: 0 12px 8px; padding: 9px; border: 1px solid var(--border-soft); border-radius: var(--r-inset); background: var(--surface-inset); box-shadow: inset 0 1px #fff; }
  .summary-block > strong { display: block; margin-bottom: 6px; font-size: 10px; }
  .summary-block p { margin: 0; color: var(--fg-muted); font-size: 9px; }
  .summary-head { display: flex; align-items: baseline; gap: 6px; margin-bottom: 6px; }
  .summary-head > strong { font-size: 10px; }
  .amp-caption { color: var(--fg-muted); font-size: 8.5px; }
  .summary-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(64px, auto); gap: 3px 8px; font-size: 9px; }
  .summary-grid > span:nth-child(even) { min-width: 64px; text-align: right; font-variant-numeric: tabular-nums; }
  .amp-value { color: var(--fg-muted); font-weight: 400; }
  .positive { color: var(--good); font-weight: 700; }
  .damage-list { display: flex; flex-direction: column; gap: 4px; }
  .damage-list div { display: flex; align-items: baseline; gap: 8px; font-size: 9px; }
  .damage-list div > span:last-child { margin-left: auto; min-width: 54px; text-align: right; }
  .damage-list small { color: var(--danger); font-size: 8.5px; text-align: right; }
  .unmodeled { margin-top: 7px; padding: 5px 6px; border: 1px dashed var(--border); border-radius: var(--r-inset); color: var(--fg-muted); font-size: 8.5px; }
  .danger { color: var(--danger); }
  .delete-set { width: 58px; flex: none; }
  /* 消える対象(セット一覧)の直下。何も覆わないので absolute にしない —
     出しても動くのは自分より下の空きだけ */
  .delete-confirm {
    margin: 0 8px; padding: 9px;
    display: flex; flex-direction: column; gap: 6px;
    border: 1px solid var(--danger); border-radius: var(--r-panel);
    background: var(--bg-field); box-shadow: var(--shadow-pop);
    color: var(--fg); font-size: 10px;
  }
  .delete-confirm strong { font-size: 11px; }
  .delete-confirm small { color: var(--fg-muted); }
  .confirm-actions { display: flex; gap: 7px; }
  .confirm-actions .btn { flex: 1; justify-content: center; }
  @media (max-width: 1100px) { .category-switch { grid-template-columns: 1fr; } }
  @media (max-width: 950px) { .buff-page { grid-template-columns: 220px minmax(320px, 1fr); } .summary { grid-column: 1 / -1; } }
</style>
