<script lang="ts">
  import {
    createBuffSet, deleteBuffSet, duplicateBuffSet, errorMessage, previewDefense,
    previewEffectiveStats, summarizeBuffSelection, updateBuffSet,
  } from "../../api/commands";
  import type {
    BuffChoice, BuffDamageEffect, BuffDefinition, BuffOrigin, BuffPurpose, BuffSet, BuffTarget,
    DamageCategory, CategoryTrace, DefenseProfile, EffectiveStats, StatKind, StatSourceEffect,
  } from "../../api/types";
  import {
    isBlocked, isChoiceValue, isFixedValue, isPercentLayer, isRecordOnly, isUserSelectedTarget,
    toggleBuff, userInputRange,
  } from "../../buffs";
  import { fmtInt, formatLayerValue, topRows, topRowsText, type TopRows } from "../../format";
  import { singleEffectLabel } from "../../characterSkills";
  import { STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS } from "../../labels";
  import { app, focusCharacterSource, payloadOf, refreshEvaluation, syncCalcBuffs, selectedCharacter } from "../../state.svelte";
  import { reportError } from "../../toast.svelte";
  import { bump, flash } from "../../ui/motion.svelte";
  import StatInput from "../../ui/StatInput.svelte";
  import StepSelect from "../../ui/StepSelect.svelte";
  import Icon from "../../ui/Icon.svelte";

  const PURPOSES: { id: BuffPurpose; label: string; description: string }[] = [
    { id: "stats", label: "ステータスを上げたい", description: "能力値が伸びる効果" },
    { id: "damage", label: "火力を上げたい", description: "攻撃ダメージ効果を持つバフ" },
    { id: "durability", label: "耐久を上げたい", description: "受けるダメージや生存力に関わる効果" },
  ];
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
  let summarySeq = 0;
  const selected = $derived(app.buffSets.find((set) => set.id === selectedId) ?? app.buffSets[0] ?? null);
  const activePurposeMeta = $derived(PURPOSES.find((purpose) => purpose.id === activePurpose) ?? PURPOSES[0]);
  const matchesPurpose = (def: BuffDefinition, purpose: BuffPurpose) =>
    purpose === "damage" ? def.damage_effects.length > 0 : def.purposes.includes(purpose);
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
   *  値の調整が要るバフは ON のあいだチップがその場で編集フォームに変わる(下の .expanded) */
  async function toggle(def: BuffDefinition) {
    if (!selected || saving) return;
    if (openInfoId === def.id) openInfoId = null;
    const next: BuffSet = JSON.parse(JSON.stringify(selected));
    next.choices.choices = toggleBuff(next.choices.choices, def, !next.choices.choices.some((c) => c.buff_id === def.id));
    await persist(next);
  }

  /** ON 中のバフの、いまの保存済み選択(値の調整フォームが直接読み書きする) */
  const liveChoice = (def: BuffDefinition): BuffChoice | null =>
    selected?.choices.choices.find((c) => c.buff_id === def.id) ?? null;

  /** 値の調整フォームの入力。触った瞬間に確定する(§07: 適用ボタンを挟まない) */
  async function updateChoice(def: BuffDefinition, edit: (choice: BuffChoice) => void) {
    if (!selected || saving) return;
    const next: BuffSet = JSON.parse(JSON.stringify(selected));
    const index = next.choices.choices.findIndex((c) => c.buff_id === def.id);
    if (index < 0) return;
    edit(next.choices.choices[index]);
    await persist(next);
  }

  async function duplicate() {
    if (!selected || saving) return;
    saving = true;
    try { replaceSet(await duplicateBuffSet(selected.id)); }
    catch (e) { reportError(errorMessage(e)); }
    finally { saving = false; }
  }

  function requestRemove() {
    if (!selected || saving) return;
    confirmDeleteId = selected.id;
    if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = setTimeout(() => (confirmDeleteId = null), 4000);
  }

  async function remove() {
    if (!selected || saving || confirmDeleteId !== selected.id) return;
    if (confirmDeleteTimer !== null) clearTimeout(confirmDeleteTimer);
    confirmDeleteTimer = null;
    confirmDeleteId = null;
    saving = true;
    try {
      const deletedId = selected.id;
      await deleteBuffSet(deletedId);
      app.buffSets = app.buffSets.filter((set) => set.id !== deletedId);
      const affectedIds = new Set(
        app.characters.filter((character) => character.default_buff_set_id === deletedId).map((character) => character.id),
      );
      app.characters = app.characters.map((character) =>
        affectedIds.has(character.id) ? { ...character, default_buff_set_id: null } : character,
      );
      selectedId = app.buffSets[0]?.id ?? null;
      if (app.calcBuffSetId === deletedId) syncCalcBuffs(selectedCharacter());
      await Promise.all(app.characters.filter((character) => affectedIds.has(character.id)).map(refreshEvaluation));
    } catch (e) { reportError(errorMessage(e)); }
    finally { saving = false; }
  }

  const on = (def: BuffDefinition) => selected?.choices.choices.some((choice) => choice.buff_id === def.id) ?? false;
  const needsInput = (def: BuffDefinition) =>
    isUserSelectedTarget(def.target) || isChoiceValue(def.value) || userInputRange(def.value) !== null;
  const exclusive = (def: BuffDefinition) => def.exclusive_slots.length > 0 ? def.exclusive_slots.join(" / ") : "独立";
  const statOptions = STAT_KINDS.map((kind) => ({ value: kind, label: STAT_LABELS[kind] }));

  function targetLabel(target: BuffTarget): string {
    if (target === "all_stats") return "全ステータス";
    if (target === "user_selected") return "選択したステータス";
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

  function buffTooltip(def: BuffDefinition, blocked: boolean): string {
    const purposes = def.purposes.map((purpose) => PURPOSES.find((item) => item.id === purpose)?.label ?? purpose).join(" / ");
    const lines = [def.name, `目的: ${purposes}`, `種類: ${ORIGIN_LABELS[def.origin]}`, `主効果: ${effectSummary(def)}`, `対象: ${targetLabel(def.target)}`];
    const damage = def.damage_effects.map(singleEffectLabel).filter((label): label is string => label !== null);
    if (damage.length > 0 && !isRecordOnly(def.value)) lines.push(`追加効果: ${damage.join(" ・ ")}`);
    lines.push(`重複: ${exclusive(def)}`);
    if (def.note) lines.push(`補足: ${def.note}`);
    if (blocked) lines.unshift("選択不可: 同じ重複枠のバフが選ばれています", "");
    return lines.join("\n");
  }

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

  /** 「ほか n」ポップオーバー(.rest-popover)を、開いた場所の直下に置いたまま画面外へ
   *  はみ出さないよう位置決めする Svelte action(§09 規則 1: 押した場所=トリガ自身は動かさない、
   *  規則 3: 重なるものはレイアウトを押さない — どちらも絶対配置のまま完結させる)。
   *  下に入らなければ上に開く(フリップ)。上にも入り切らなければ、収まる高さに
   *  クランプして中身だけスクロールにする(画面外に開いたまま放置しない)。 */
  function positionRestPopover(node: HTMLElement) {
    const margin = 8;
    const rect = node.getBoundingClientRect();
    if (rect.bottom > window.innerHeight - margin) {
      node.classList.add("flip-up");
      const flipped = node.getBoundingClientRect();
      if (flipped.top < margin) {
        const available = Math.max(flipped.bottom - margin, 60);
        node.style.maxHeight = `${available}px`;
        node.style.overflowY = "auto";
      }
    }
    return {
      destroy() {
        node.classList.remove("flip-up");
        node.style.maxHeight = "";
        node.style.overflowY = "";
      },
    };
  }

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
  </aside>

  <section class="catalog">
    <div class="bar">セットに入れるバフ</div>
    {#if selected}
      <div class="set-tools">
        <input value={selected.name} disabled={saving} aria-label="バフセット名" onchange={(e) => persist({ ...selected, name: e.currentTarget.value })} />
        <button class="btn" onclick={duplicate}>複製</button>
        <button class="btn danger delete-set" disabled={saving} onclick={requestRemove}>削除</button>
        {#if confirmDeleteId === selected.id}
          <div class="delete-confirm" role="alert">
            <span>このセットを削除します</span>
            <button class="btn danger" disabled={saving} onclick={remove}>削除する</button>
            <button class="btn" disabled={saving} onclick={() => (confirmDeleteId = null)}>やめる</button>
          </div>
        {/if}
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
        <section class="buff-group" use:flash={() => `${activePurpose}:${activeDamageGroup}`}>
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
              {@const expanded = on(def) && needsInput(def)}
              <div
                class="buff-option" class:on={on(def)} class:expanded
                class:info-open={openInfoId === def.id}
                use:flash={() => (on(def) ? "on" : "off")}
              >
                <span class="buff-icon"><Icon kind="buff" id={def.id} size={28} label={def.name} /></span>
                {#if expanded}
                  <!-- ON のあいだ、チップそのものが値の調整フォームに変わる。適用ボタンは無く、
                       触った瞬間に確定する(§07)。外すはここに常設の 1 つだけ -->
                  <div class="buff-config">
                    <div class="config-head">
                      <strong>{def.name}</strong>
                      <span class="config-effect num" use:flash={() => statDeltaText(def) ?? effectSummary(def)}>{statDeltaText(def) ?? effectSummary(def)}</span>
                      <button type="button" class="clear" disabled={saving} onclick={() => toggle(def)}>外す</button>
                    </div>
                    <div class="choice-editor">
                      {#if isUserSelectedTarget(def.target)}
                        <StepSelect label="対象ステ" options={statOptions} cols={4} bind:value={() => liveChoice(def)?.stat ?? STAT_KINDS[0], (value) => updateChoice(def, (c) => (c.stat = value as StatKind))} />
                      {/if}
                      {#if isChoiceValue(def.value)}
                        {@const options = def.value.choice.map((value, index) => ({ value: String(index), label: formatLayerValue(def.layer, value) }))}
                        <StepSelect label="段階" {options} bind:value={() => String(liveChoice(def)?.choice_index ?? 0), (value) => updateChoice(def, (c) => (c.choice_index = Number(value)))} />
                      {/if}
                      {#if userInputRange(def.value)}
                        {@const range = userInputRange(def.value)!}
                        {@const scale = isPercentLayer(def.layer) ? 100 : 1}
                        <StatInput label={def.id === "club_effect" ? "クラブ効果" : (isPercentLayer(def.layer) ? "値 (%)" : "値")} min={range.min * scale} max={range.max * scale} bind:value={() => (liveChoice(def)?.value ?? def.default_value ?? range.min) * scale, (value) => updateChoice(def, (c) => (c.value = value / scale))} />
                      {/if}
                    </div>
                  </div>
                {:else}
                  {@const isOn = on(def)}
                  {@const top = isOn ? statTop(def) : null}
                  {@const dmg = isOn ? damageText(def) : null}
                  <!-- 「ほか n」を独立したボタンにするため、チップ本体はネイティブ button ではなく
                       role="button" の div にする(button の中に button は入れられない)。
                       クリック・キー操作の意味は button と同じに保つ。 -->
                  <div
                    class="buff-toggle"
                    class:disabled={blocked || saving}
                    role="button"
                    tabindex={blocked || saving ? -1 : 0}
                    aria-disabled={blocked || saving}
                    onclick={() => { if (!blocked && !saving) toggle(def); }}
                    onkeydown={(e) => {
                      if ((e.key === "Enter" || e.key === " ") && !blocked && !saving) { e.preventDefault(); toggle(def); }
                    }}
                    title={buffTooltip(def, blocked)}
                    aria-label={`${def.name}。${effectLine(def)}${needsInput(def) ? "。クリックして設定" : ""}`}
                  >
                    <span class="chip-copy">
                      <strong>{def.name}</strong>
                      {#if isOn && top}
                        <!-- ON: このキャラで実際に何点伸びたかを行ごとに出す(§00 05)。ステ増分と
                             ダメージ効果は別行 — 1 行に連結すると長い名前のダメージ効果で溢れる
                             (5周目 実機指摘)。「ほか n」は押せる要素にして中身を辿れるようにする
                             (title だけに情報を追いやらない)。チップ本体のトグルとは
                             stopPropagation で切り離す(§00 03: 押した場所は動かさない)。 -->
                        <span class="chip-effect" use:flash={() => effectLine(def)}>
                          {#if top.shown.length > 0}
                            <small>
                              {top.shown.join(" / ")}
                              {#if top.restCount > 0}
                                <button
                                  type="button"
                                  class="rest-link"
                                  onclick={(e) => { e.stopPropagation(); openInfoId = openInfoId === def.id ? null : def.id; }}
                                  aria-expanded={openInfoId === def.id}
                                >ほか {top.restCount}</button>
                              {/if}
                            </small>
                          {/if}
                          {#if dmg}<small>{dmg}</small>{/if}
                          {#if top.shown.length === 0 && !dmg}<small>{effectSummary(def)}</small>{/if}
                        </span>
                      {:else}
                        <span class="chip-effect"><small use:flash={() => effectLine(def)}>{effectLine(def)}</small></span>
                      {/if}
                      {#if needsInput(def)}<small class="op-hint">クリックして設定</small>{/if}
                      <span class="origin-badge">{ORIGIN_LABELS[def.origin]}</span>
                    </span>
                    {#if isOn && top && openInfoId === def.id}
                      {@const restRows = statRows(def)}
                      <!-- 「ほか n」の中身。押した場所(チップ)の直下に出し、レイアウトは押さない
                           (絶対配置なので下のチップを動かさない)。 -->
                      <div
                        class="rest-popover"
                        role="dialog"
                        tabindex="-1"
                        aria-label={`${def.name} の全ステ増分`}
                        use:positionRestPopover
                        onclick={(e) => e.stopPropagation()}
                        onkeydown={(e) => e.stopPropagation()}
                      >
                        {#each restRows as row (row.label)}
                          <div class="num">{row.label}</div>
                        {/each}
                        <button type="button" class="rest-close" onclick={(e) => { e.stopPropagation(); openInfoId = null; }}>閉じる</button>
                      </div>
                    {/if}
                  </div>
                {/if}
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
    <div class="bar">現在の効果</div>
    {#if selected}
      <div class="count num" use:bump={() => selected.choices.choices.length}>{selected.choices.choices.length}<small>件 ON</small></div>
      {#if summaryLoading}<p class="summary-note">集計しています…</p>{/if}
      <div class="summary-block" use:flash={() => JSON.stringify(statAfter)}>
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
      <div class="summary-block" use:flash={() => JSON.stringify(damageSummary)}>
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
      <div class="summary-block" use:flash={() => JSON.stringify(defenseAfter)}>
        <strong>耐久</strong>
        {#if defenseBefore && defenseAfter}
          <div class="summary-grid">
            <span>物理防御力</span><span class="num">{formatDelta(defenseAfter.physical_defense - defenseBefore.physical_defense)}</span>
            <span>魔法防御力</span><span class="num">{formatDelta(defenseAfter.magic_defense - defenseBefore.magic_defense)}</span>
            <span>複合防御力</span><span class="num">{formatDelta(defenseAfter.composite_defense - defenseBefore.composite_defense)}</span>
            <span>コンボ回避</span><span class="num">{formatDelta((defenseAfter.combo_evasion - defenseBefore.combo_evasion) * 100, 1)}%</span>
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
  .chips { flex: 1; min-height: 0; padding: 7px; display: grid; grid-template-columns: repeat(auto-fit, minmax(210px, 1fr)); grid-auto-rows: max-content; gap: 6px; border-top: 1px solid var(--border-soft); align-content: start; align-items: start; overflow-y: auto; scrollbar-gutter: stable; }
  .buff-option { position: relative; min-width: 0; border: 1px solid var(--border); border-radius: var(--r-panel); background: var(--bg-field); overflow: hidden; }
  .buff-option.on { border-color: var(--sel-bd); background: var(--sel-card); box-shadow: inset 0 0 0 1px var(--sel-bd); }
  /* 値の調整フォームに変わっているあいだは、グリッドの 1 列に収まらない中身があっても
     隣のチップを押し出さない — 行いっぱいに広がる(§00 03「押した場所は動かない」) */
  .buff-option.expanded { grid-column: 1 / -1; border-color: var(--sim); background: var(--state-temp-bg); box-shadow: inset 0 0 0 1px var(--sim); }
  /* 「ほか n」のポップオーバーはチップの外(下)へはみ出すので、開いている間だけ
     overflow:hidden を外して見えるようにする(§00 03: 押した場所は動かさない = 隣のチップは押し出さない)。 */
  .buff-option.info-open { overflow: visible; z-index: 7; }
  .buff-icon { position: absolute; z-index: 1; top: 16px; left: 7px; width: 28px; height: 28px; transition: top .2s ease; }
  .buff-option.expanded .buff-icon { top: 9px; }
  .buff-toggle { position: relative; width: 100%; min-height: 60px; padding: 6px 7px 6px 48px; display: flex; align-items: center; gap: 7px; border: 0; background: transparent; color: var(--fg); text-align: left; cursor: pointer; }
  .buff-toggle.disabled { opacity: .45; cursor: default; }
  /* ステ増分・ダメージ効果の行を積む場所。件数が 0〜2 行のどれでも同じ高さを確保し、
     チップの高さが行内で揃うようにする(5周目 実機指摘: チップの高さは全チップで揃える)。 */
  .chip-effect { display: flex; flex-direction: column; justify-content: center; gap: 2px; min-height: 22px; }
  /* 「ほか n」= 割愛した増分の中身を辿るボタン。チップ本体のトグルとは別の押せる要素だと
     分かるよう下線を付け、チップの色そのものは変えない(§00 03 と衝突させない)。 */
  .rest-link { display: inline; padding: 0; border: 0; background: none; color: var(--accent); font: inherit; text-decoration: underline; text-underline-offset: 2px; cursor: pointer; }
  .rest-link:hover { color: var(--accent-hover); }
  .rest-popover {
    position: absolute; z-index: 6; top: calc(100% + 4px); left: 7px; right: 7px;
    display: flex; flex-direction: column; gap: 3px; padding: 8px 9px;
    border: 1px solid var(--sel-bd); border-radius: var(--r-inset);
    background: var(--bg-field); box-shadow: var(--shadow-pop);
    color: var(--fg); font-size: 9px; cursor: default;
  }
  /* .chips のスクロール領域の下端付近で開くと下に入り切らない場合のフリップ先。
     押した「ほか n」自身の位置(トリガ)は動かさず、ポップオーバーだけが上に開く
     (positionRestPopover が付ける。§09 規則 1・3)。 */
  .rest-popover:global(.flip-up) { top: auto; bottom: calc(100% + 4px); }
  .rest-close { align-self: flex-end; margin-top: 2px; padding: 2px 8px; border: 1px solid var(--border-soft); border-radius: var(--r-pill); background: var(--surface-inset); color: var(--fg-muted); font-size: 8.5px; }
  .rest-close:hover { color: var(--fg); }
  .buff-config { min-height: 60px; padding: 7px 7px 7px 41px; display: flex; flex-direction: column; gap: 6px; }
  .config-head { display: flex; align-items: center; gap: 8px; }
  .config-head strong { flex: 1; min-width: 0; font-size: 10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .config-effect { flex-shrink: 0; font-size: 10.5px; font-weight: 700; color: var(--sim-fg); }
  .config-head .clear {
    flex-shrink: 0; padding: 3px 4px; border-radius: var(--r-inset);
    background: none; border: 0; color: var(--fg-dim); font-size: 9.5px; line-height: 1;
  }
  .config-head .clear:hover:not(:disabled) { background: var(--state-short-bg); color: var(--danger); }
  .choice-editor { padding: 7px; display: flex; flex-direction: column; gap: 7px; border: 1px solid var(--border-soft); border-radius: var(--r-inset); background: var(--bg-field); box-shadow: inset 0 1px #fff; }
  .chip-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; }
  .chip-copy strong, .chip-copy small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* チップ本体の要約行(.chip-effect 内)だけは、省略記号で切らず自然に折り返す — 幅で
     症状を消すのではなく、行を分けて出す量そのものを収める(5周目 実機指摘)。 */
  .chip-effect small { overflow: visible; text-overflow: clip; white-space: normal; }
  .chip-copy strong { font-size: 10px; }
  .chips small { color: var(--fg-muted); font-size: 9px; }
  /* 効果値とは別行の操作ヒント。同じ行に混ぜない(§00 05 考えさせない) */
  .op-hint { color: var(--sim-fg) !important; }
  .origin-badge { align-self: flex-start; margin-top: 3px; padding: 1px 6px; border: 1px solid var(--border-soft); border-radius: var(--r-pill); background: var(--surface-inset); color: var(--fg-muted); font-size: 8.5px; line-height: 1.4; white-space: nowrap; }
  .summary { background: var(--bg-raised); }
  .count { margin: 12px; padding: 13px; border: 1px solid var(--border); border-radius: var(--r-inset); background: var(--surface-inset); box-shadow: inset 0 1px #fff; font-size: 27px; font-weight: 700; }
  .count small { margin-left: 6px; font-family: var(--font); font-size: 10px; font-weight: 500; }
  .summary-note { margin: 0 12px 8px; color: var(--fg-muted); font-size: 9px; }
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
  .delete-confirm {
    position: absolute; z-index: 5; top: 43px; right: 10px;
    display: flex; align-items: center; gap: 7px; padding: 8px;
    border: 1px solid var(--danger); border-radius: var(--r-panel);
    background: var(--bg-field); box-shadow: var(--shadow-pop);
    color: var(--fg); font-size: 10px; white-space: nowrap;
  }
  @media (max-width: 1100px) { .category-switch { grid-template-columns: 1fr; } }
  @media (max-width: 950px) { .buff-page { grid-template-columns: 220px minmax(320px, 1fr); } .summary { grid-column: 1 / -1; } }
</style>
