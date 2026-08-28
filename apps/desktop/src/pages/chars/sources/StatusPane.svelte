<script lang="ts">
  // 「status」補正源のペイン。キャラ選択・覚醒・エタの意志・主軸スキル・主属性・能力値の一覧。
  import type { Element, ElementPreview, Skill, StatKind, StatPreview } from "../../../api/types";
  import { previewElements } from "../../../api/commands";
  import { draftToPayload, ETERNAL_MILESTONES, type Draft } from "../../../draft";
  import { fmtInt, formatLayerValue } from "../../../format";
  import { ELEMENT_LABELS, ELEMENTS, STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { app } from "../../../state.svelte";
  import Icon from "../../../ui/Icon.svelte";
  import Picker, { type PickerOption } from "../../../ui/Picker.svelte";
  import StatInput from "../../../ui/StatInput.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
    skills: Skill[];
  }
  let { draft, preview, skills }: Props = $props();

  const STAT_MIN = 1;

  // キャラは名前で探すより顔で選ぶほうが速い(ゲーム内も顔で選ぶ)。§06 の 40px。
  // 名前は必ず併記する(アイコン単独表示は禁止)
  const gameCharacterName = $derived(
    app.gameCharacters.find((c) => c.id === draft.gameCharacterId)?.name ?? "未選択",
  );
  /** キャラは登録時に決めるもの。ふだんは畳んでおく */
  let charPickOpen = $state(false);
  function setGameCharacterId(id: string) {
    if (id === draft.gameCharacterId) return;
    draft.gameCharacterId = id;
    draft.mainSkillId = "";
  }

  // エタの意志 Lv は 0〜100 の**数値**。101 個を並べても段階にならないので、
  // 節目(20 / 40 / 60 / 80 / 90)を選べる形 + 数値の微調整にする。
  // 節目はそこを超えると上限の増え方が一段上がる地点で、育成の目標地点そのもの。
  const eternalMilestoneOptions = $derived(
    ETERNAL_MILESTONES.filter((lv) => lv <= limits.eternal_level_max).map((lv) => ({
      value: String(lv),
      label: String(lv),
    })),
  );
  /** エタの意志は覚醒 5 の先にあるものなので、触った時点で覚醒は 5 で確定する */
  function setEternalLevel(level: string) {
    draft.eternalLevel = level;
    if (Number(level) > 0) draft.stage = "5";
  }
  // 覚醒段階は 4 と 5 しか使わない(このツールの対象)。それ以外は開いたときだけ出す
  const stageOptions = Array.from({ length: 6 }, (_, i) => ({ value: String(i), label: String(i) }));
  const stageMainOptions = [4, 5].map((i) => ({ value: String(i), label: String(i) }));
  let stageAllOpen = $state(false);
  const stageIsLow = $derived(Number(draft.stage) < 4);

  // 主軸スキル。未収録のキャラがあるので未選択("")を許す。
  /** 火力の高い順。主軸に選ばれるのはほぼこの上位なので、候補として先に出す */
  const skillPower = (s: Skill) => s.multiplier * Math.max(1, s.hit_count);
  /** 一覧でも名前だけにしない。単 / 範・段数・属性を名前の後ろに付ける */
  const skillMeta = (s: Skill) =>
    `${s.target === null ? "?" : s.target === "single" ? "単" : "範"} ・ ${s.hit_count} 段 ・ ${ELEMENT_LABELS[s.element]}`;
  const mainSkillOptions = $derived<PickerOption[]>([
    { value: "", name: "未選択(攻撃力を出さない)", iconId: null },
    ...[...skills]
      .sort((a, b) => skillPower(b) - skillPower(a))
      .map((s) => ({ value: s.id, name: s.name, meta: skillMeta(s), iconId: s.id, iconKind: "skill" as const })),
  ]);
  const topSkills = $derived([...skills].sort((a, b) => skillPower(b) - skillPower(a)).slice(0, 3));
  const mainSkill = $derived(skills.find((s) => s.id === draft.mainSkillId) ?? null);
  /** 候補にない主軸を選んでいるとき、または自分で開いたときだけ全部出す */
  let skillListOpen = $state(false);
  const skillPickedOutside = $derived(
    draft.mainSkillId !== "" && !topSkills.some((s) => s.id === draft.mainSkillId),
  );

  // 属性は主軸スキルで決まる。無属性のスキルのときだけ、乗せる属性を選ばせる
  // (アンプルで属性を足す運用が多い)
  const skillElement = $derived(mainSkill?.element ?? null);
  const elementFromSkill = $derived(skillElement !== null && skillElement !== "neutral");
  let elementPickOpen = $state(false);

  // --- 主属性 -------------------------------------------------------------
  // 供給源(ペット / モンスターカード / ルーンスキル / 頭・カフスのアビリティ)は、
  // 実際には**全部同じ属性に振る**。だから供給源ごとに聞かず、主属性を 1 回選ばせて
  // まとめて乗せる(§00「要らないものを見せない」)
  const elementSourceDefs = $derived(app.elementSources);
  const elementOptions = [
    { value: "", label: "なし" },
    ...ELEMENTS.map((e) => ({ value: e, label: ELEMENT_LABELS[e] })),
  ];
  /** 供給源が全部同じ属性ならそれが主属性。ばらけていたら "" を返す */
  const mainElement = $derived.by(() => {
    const picked = elementSourceDefs.map((def) => draft.statSources.elements[def.id] ?? null);
    const first = picked[0] ?? null;
    return first !== null && picked.every((e) => e === first) ? first : "";
  });
  function setMainElement(value: string) {
    for (const def of elementSourceDefs) {
      draft.statSources.elements[def.id] = value === "" ? null : (value as Element);
    }
  }
  const elementSourceTotal = $derived(elementSourceDefs.reduce((n, def) => n + def.value, 0));
  /**
   * 属性は主軸スキルで決まるので、スキルを選んだら供給源もその属性に合わせる(自動値)。
   * 自分で「別の属性を乗せる」を開いたときは触らない — 例外操作を上書きしない
   */
  $effect(() => {
    if (!elementFromSkill || elementPickOpen) return;
    if (mainElement === skillElement) return;
    setMainElement(skillElement as string);
  });
  // 内訳は Rust 側で出す(キャラ基礎属性値は gamedata にしか無い)。開いている間だけ引く
  let elementPreview = $state<ElementPreview | null>(null);
  let elementSeq = 0;
  $effect(() => {
    const payload = draftToPayload(draft);
    const seq = ++elementSeq;
    previewElements(payload)
      .then((p) => {
        if (seq === elementSeq) elementPreview = p;
      })
      .catch(() => {
        if (seq === elementSeq) elementPreview = null;
      });
  });

  const traceFor = (k: StatKind) => preview?.traces.find((t) => t.kind === k) ?? null;
  const signed = (n: number) => `${n >= 0 ? "+" : ""}${fmtInt(n)}`;
</script>

<div class="card">
  <div class="fields">
    <label class="text">
      <span class="label">名前</span>
      <input type="text" bind:value={draft.name} maxlength="32" placeholder="表示名" />
    </label>
    <!-- キャラは登録のときに決めて、ふだんは変えない。いまのキャラだけ出して、
         変えるときに顔を並べる(§00 02)。名前はアイコンに必ず併記する -->
    <div class="wide">
      <span class="label">キャラ</span>
      <div class="char-now">
        <Icon kind="character" id={draft.gameCharacterId} size={40} label={gameCharacterName} />
        <span class="char-name">{gameCharacterName}</span>
        <button type="button" class="chip quiet" class:on={charPickOpen} onclick={() => (charPickOpen = !charPickOpen)}>
          {charPickOpen ? "閉じる" : "変更"}
        </button>
      </div>
      {#if charPickOpen}
        <div class="pick-grid open-in">
          {#each app.gameCharacters as c (c.id)}
            <button
              type="button"
              class="pick"
              class:on={c.id === draft.gameCharacterId}
              onclick={() => { setGameCharacterId(c.id); charPickOpen = false; }}
            >
              <Icon kind="character" id={c.id} size={40} label={c.name} />
              <span class="pick-name">{c.name}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
    <!-- エタの意志は覚醒 5 の先にあるもの。**選んだ時点で覚醒は 5 で確定する**ので、
         覚醒より先に置く(§00 01 決める順に並べる) -->
    <div class="wide">
      <span class="label">エタの意志 Lv</span>
      <div class="eternal-row">
        <StatInput
          label=""
          min={0}
          max={limits.eternal_level_max}
          bind:value={
            () => Number(draft.eternalLevel),
            (v) => setEternalLevel(String(v))
          }
        />
        <StepSelect
          label=""
          options={eternalMilestoneOptions}
          cols={eternalMilestoneOptions.length}
          bind:value={() => draft.eternalLevel, setEternalLevel}
        />
      </div>
      <p class="hint dim">節目(20 / 40 / 60 / 80 / 90)を超えると、ダメージ上限・防御力上限・能力値上限の伸びが一段上がります。Lv を入れると覚醒は 5 段階になります。</p>
    </div>
    <!-- 覚醒段階は 4 と 5 しか使わない。それ以外は開いたときだけ出す(§00 02) -->
    <div class="stage-field wide">
      <span class="label">覚醒段階</span>
      <div class="stage-row">
        <StepSelect
          label=""
          options={stageAllOpen || stageIsLow ? stageOptions : stageMainOptions}
          cols={stageAllOpen || stageIsLow ? stageOptions.length : stageMainOptions.length}
          bind:value={draft.stage}
        />
        {#if !stageIsLow}
          <button type="button" class="chip quiet" class:on={stageAllOpen} onclick={() => (stageAllOpen = !stageAllOpen)}>
            {stageAllOpen ? "4 / 5 だけ" : "それ以外"}
          </button>
        {/if}
      </div>
    </div>
    <!-- 主軸に選ばれるのはほぼ火力上位。3 つを候補に出し、それ以外は開いたときだけ -->
    <div class="wide">
      <span class="label">主軸スキル</span>
      <div class="skill-row">
        {#each topSkills as sk (sk.id)}
          <!-- スキルは名前だけでは選べない。単 / 範・段数・属性を名前の隣に出す。
               対象指定が wiki と突き合わせできていないものは `?`(0 や「単体」で埋めない) -->
          <button
            type="button"
            class="chip skill-chip"
            class:on={draft.mainSkillId === sk.id}
            onclick={() => (draft.mainSkillId = sk.id)}
          >
            <Icon kind="skill" id={sk.id} size={20} label={sk.name} />
            <span class="skill-name">{sk.name}</span>
            <span class="skill-meta num" class:unknown={sk.target === null}>
              {sk.target === null ? "?" : sk.target === "single" ? "単" : "範"}
            </span>
            <span class="skill-meta num">{sk.hit_count} 段</span>
            <span class="skill-meta num elem-{sk.element}">{ELEMENT_LABELS[sk.element]}</span>
          </button>
        {/each}
        {#if skills.length > topSkills.length}
          <button
            type="button"
            class="chip quiet"
            class:on={skillListOpen || skillPickedOutside}
            onclick={() => (skillListOpen = !skillListOpen)}
          >ほかのスキル</button>
        {/if}
      </div>
      {#if skillListOpen || skillPickedOutside}
        <!-- open-in は overflow: hidden なので、重ねて出す候補が切れる。
             ここは面が現れるだけなので swap-in(§10 型 3b) -->
        <div class="skill-all swap-in">
          <Picker
            options={mainSkillOptions}
            note="火力の高い順(倍率 × 段数)"
            placeholder="スキルを選ぶ"
            bind:value={draft.mainSkillId}
          />
        </div>
      {/if}
    </div>
    <!-- 属性はふつう主軸スキルで決まる。無属性のときだけ「何を乗せるか」を選ばせる -->
    <div class="wide">
      <span class="label">属性</span>
      {#if elementFromSkill && !elementPickOpen}
        <p class="element-auto">
          <b>{ELEMENT_LABELS[skillElement!]}</b>
          <span class="dim">— 主軸スキル「{mainSkill?.name}」で決まります</span>
          <button type="button" class="chip quiet" onclick={() => (elementPickOpen = true)}>別の属性を乗せる</button>
        </p>
      {:else}
        {#if skillElement === "neutral"}
          <p class="hint dim">主軸スキルが無属性なので、アンプルなどで乗せる属性を選びます。</p>
        {/if}
        <StepSelect
          label=""
          options={elementOptions}
          cols={elementOptions.length}
          tone={(v) => (v === "" ? undefined : `elem-${v}`)}
          bind:value={() => mainElement, setMainElement}
        />
        <p class="hint dim">ペット・カード・ルーン・アビリティの +{elementSourceTotal} をまとめて乗せます。</p>
      {/if}
    </div>
  </div>
  {#if elementPreview}
    <p class="hint dim">
      属性値
      {#each ELEMENTS.filter((e) => elementPreview!.total[e] > 0) as e (e)}
        <b>{ELEMENT_LABELS[e]} {fmtInt(elementPreview.total[e])}</b>
        <span class="dim">(キャラ {fmtInt(elementPreview.base[e])} + 装備 {fmtInt(elementPreview.equipment[e])} + 主属性 {fmtInt(elementPreview.sources[e])})</span>
      {:else}
        まだどの属性も乗っていません
      {/each}
      。与ダメージに効くのは<b>攻撃側 − 敵</b>の差で、差 +1 ごとに +0.625%、+80 で上限 +50%(敵は 120 / 125)。
    </p>
  {/if}
  <p class="hint dim">
    {#if skills.length === 0}
      このキャラのスキルはまだ未収録です。収録されるまで攻撃力は出せません。
    {:else if draft.mainSkillId === ""}
      主軸スキルを選ぶと攻撃力が出ます。スキルの依存種別(突き / 斬り / 魔攻 / 魔防 / 複合)で装備の係数が変わるためです。
    {:else}
      攻撃力はこのスキルの依存種別で計算します。ダメージ計算タブは選んだスキルごとに計算します。
    {/if}
  </p>
</div>
<div class="card">
  <div class="card-title">能力値 <span class="dim normal">設定を触ると即時更新</span></div>
  <div class="tbl">
    <table class="grid">
      <thead><tr><th>ステ</th><th class="n">素</th><th class="n">補正</th><th>素ステ → 最終</th><th class="n">最終</th></tr></thead>
      <tbody>
        {#each STAT_KINDS as k (k)}
          {@const trace = traceFor(k)}
          {@const diff = preview ? preview.stats[k] - draft.baseStats[k] : null}
          {@const cap = trace?.stat_cap ?? 0}
          {@const basePct = cap > 0 ? Math.min(100, (draft.baseStats[k] / cap) * 100) : 0}
          {@const addPct = cap > 0 && diff !== null ? Math.max(0, Math.min(100 - basePct, (diff / cap) * 100)) : 0}
          <tr>
            <td>{STAT_LABELS[k]}</td>
            <td class="n stat-cell">
              <StatInput label="" min={STAT_MIN} max={limits.base_stat_max} bind:value={draft.baseStats[k]} />
            </td>
            <td class="n muted ro">{diff === null ? "—" : signed(diff)}</td>
            <!-- 素ステ → 最終を 1 本のバーで(§11)。数字の羅列ではなく「どれだけ伸びたか」を見せる。
                 灰が素ステ(振り分け)、青が補正で乗った分。長さは最終能力値の上限に対する割合 -->
            <td class="ro">
              <span
                class="grow"
                title={cap > 0 ? `上限 ${fmtInt(cap)}(覚醒段階 + エタの意志 Lv)` : "上限は計算中"}
              >
                <i class="base" style="width: {basePct.toFixed(1)}%"></i>
                <i class="add" style="width: {addPct.toFixed(1)}%"></i>
              </span>
            </td>
            <td class="n final ro">
              <span class="strong">{preview ? fmtInt(preview.stats[k]) : "—"}</span>
              {#if trace?.pinned_from !== null && trace?.pinned_from !== undefined}
                <span class="pin-badge" title={`固定前: ${fmtInt(trace.pinned_from)}`}>固定</span>
              {/if}
              <!-- 「満」の枠は常に確保する。出たときに行がずれない(§09 規則 4 / §11) -->
              <span
                class="cap-badge"
                class:on={trace !== null && trace !== undefined && trace.capped_loss > 0}
                title={trace && trace.capped_loss > 0
                  ? `上限 ${fmtInt(trace.stat_cap)} で ${fmtInt(trace.capped_loss)} 捨てています。上限は覚醒段階とエタの意志 Lv で上がります`
                  : ""}
              >{trace && trace.capped_loss > 0 ? "満" : ""}</span>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <details class="contrib">
    <summary>補正の内訳 <span class="dim">{preview ? preview.contributions.length : 0} 件</span></summary>
    {#if !preview || preview.contributions.length === 0}
      <p class="empty dim">補正源なし(素ステのみ)</p>
    {:else}
      <div class="tbl">
        <table class="grid ro">
          <thead><tr><th>ステ</th><th>出典</th><th>層</th><th class="n">値</th></tr></thead>
          <tbody>
            {#each STAT_KINDS.flatMap((k) => preview!.contributions.filter((c) => c.kind === k)) as c, i (i)}
              <tr>
                <td>{STAT_LABELS[c.kind]}</td>
                <td class="muted">{c.source}</td>
                <td class="muted">{STAT_LAYER_LABELS[c.layer]}</td>
                <td class="n">{formatLayerValue(c.layer, c.value)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </details>
</div>
