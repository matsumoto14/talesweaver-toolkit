<script lang="ts">
  // 「siena」補正源のペイン。部位ごとに登録し、装着中の 1 件だけを計算へ反映する。
  import type { SienaAuraList, SienaExtraKind, SienaValueKind, StatPreview } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import {
    neutralSienaAura, selectedSienaAura, selectedSienaAuraRegistration,
    sienaExtraCapacity, sienaExtraValue, sienaStage, zeroValues,
  } from "../../../equipment";
  import { fmtInt } from "../../../format";
  import {
    EQUIPMENT_STAT_KINDS, EQUIPMENT_STAT_SHORT, PART_SLOT_LABELS,
    SIENA_ALLOWED_SLOTS, SIENA_EQUIPMENT_VALUE_SLOTS,
  } from "../../../labels";
  import type { SienaPartSlot } from "../../../labels";
  import { app } from "../../../state.svelte";
  import { bump, flash } from "../../../ui/motion.svelte";
  import StatInput from "../../../ui/StatInput.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
  }
  let { draft, preview }: Props = $props();

  // --- シエナのオーラ(部位ごと) ------------------------------------------
  // **段階は入力しない。**能力値スロットを 1 個ずつ足した数がそのまま段階になる
  // (wiki: 段階ごとに能力値スロットが 1 個解放)。追加オプションの枠も段階から出る。
  let openSienaPart = $state<SienaPartSlot | null>(null);
  const sienaList = (slot: SienaPartSlot): SienaAuraList => draft.equipment.siena[slot];
  const sienaRegistration = (slot: SienaPartSlot) => selectedSienaAuraRegistration(sienaList(slot));
  const sienaForDisplay = (slot: SienaPartSlot) => selectedSienaAura(sienaList(slot)) ?? neutralSienaAura();
  const closeSienaOnEscape = (event: KeyboardEvent) => {
    if (event.key !== "Escape") return;
    if (openSienaPart !== null) openSienaPart = null;
  };
  const createSienaRegistration = (slot: SienaPartSlot) => {
    const list = sienaList(slot);
    const id = Math.max(0, ...list.registered.map((entry) => entry.id)) + 1;
    list.registered.push({ id, label: `オーラ ${list.registered.length + 1}`, aura: neutralSienaAura() });
    list.selected_id = id;
  };
  const removeSelectedSienaRegistration = (slot: SienaPartSlot) => {
    const list = sienaList(slot);
    const index = list.registered.findIndex((entry) => entry.id === list.selected_id);
    if (index < 0) return;
    list.registered.splice(index, 1);
    list.selected_id = list.registered[0]?.id ?? null;
  };
  const sienaIsEquipmentValues = (slot: SienaPartSlot) => SIENA_EQUIPMENT_VALUE_SLOTS.includes(slot);
  /** その部位に出る能力値の種類。武器/盾とその他の部位で一覧が丸ごと違う */
  const sienaValueDefs = (slot: SienaPartSlot) =>
    app.siena.values
      .filter((d) => d.is_equipment_value === sienaIsEquipmentValues(slot))
      // 記録するだけのものは後ろへ。ふだん選ぶのは計算に入るほう(§00 02)
      .sort((a, b) => Number(b.is_modeled) - Number(a.is_modeled));
  const sienaValueDef = (kind: SienaValueKind) => app.siena.values.find((d) => d.kind === kind);
  const sienaExtraDef = (kind: SienaExtraKind) => app.siena.extras.find((d) => d.kind === kind);
  const sienaCapacity = (slot: SienaPartSlot) =>
    sienaExtraCapacity(sienaForDisplay(slot), app.siena.extra_unlock_stages);
  /** まだ付いていない追加オプション(wiki: 同じ種類は同じ装備の別スロットには出ない) */
  const sienaAddableExtras = (slot: SienaPartSlot) => {
    const used = new Set(sienaForDisplay(slot).extras.map((e) => e.kind));
    return app.siena.extras
      .filter((d) => !used.has(d.kind))
      .sort((a, b) => Number(b.is_modeled) - Number(a.is_modeled));
  };
  /** 取りうる値が連番かどうか。連番ならステッパー、飛び飛び(中ディレイ)は段階選択 */
  const sienaChoicesAreRun = (choices: number[]) =>
    choices.every((c, i) => c === choices[0] + i);
  /** 足した直後の値はレンジ上限(再抽選で振り直せるので想定値は最上値。ランダムOP と同じ) */
  function addSienaSlot(slot: SienaPartSlot, kind: SienaValueKind) {
    const def = sienaValueDef(kind);
    const siena = selectedSienaAura(sienaList(slot));
    if (!def || !siena) return;
    siena.slots.push({ kind, value: def.max });
  }
  function removeSienaSlot(slot: SienaPartSlot, index: number) {
    const siena = selectedSienaAura(sienaList(slot));
    if (!siena) return;
    siena.slots.splice(index, 1);
    // 段階が下がって枠が閉じたら、はみ出た追加オプションも落とす(値だけ残る幽霊状態を作らない)
    const capacity = sienaExtraCapacity(siena, app.siena.extra_unlock_stages);
    if (siena.extras.length > capacity) siena.extras.length = capacity;
  }
  function addSienaExtra(slot: SienaPartSlot, kind: SienaExtraKind) {
    const def = sienaExtraDef(kind);
    const siena = selectedSienaAura(sienaList(slot));
    if (!def || def.choices.length === 0 || !siena) return;
    siena.extras.push({
      kind,
      value: def.choices[def.choices.length - 1],
    });
  }
  const removeSienaExtra = (slot: SienaPartSlot, index: number) =>
    selectedSienaAura(sienaList(slot))?.extras.splice(index, 1);

  /** 能力値スロットの装備補正合計(武器/盾)。計算は Rust 側(preview) */
  const sienaPartValues = (slot: SienaPartSlot) =>
    preview?.siena_part_values.find((p) => p.slot === slot)?.values ?? zeroValues();
  /** 部位の行に出す要約。段階はバッジで出しているので、ここでは効き先の合計だけ */
  const sienaSummary = (slot: SienaPartSlot): string => {
    const siena = sienaForDisplay(slot);
    if (sienaList(slot).selected_id === null) return "未装着";
    if (sienaStage(siena) === 0) return "未発現";
    const parts: string[] = [];
    if (sienaIsEquipmentValues(slot)) {
      const v = sienaPartValues(slot);
      const top = EQUIPMENT_STAT_KINDS.filter((k) => v[k] > 0)
        .map((k) => `${EQUIPMENT_STAT_SHORT[k]}${fmtInt(v[k])}`);
      if (top.length > 0) parts.push(top.join(" / "));
    }
    // 正は SienaAura::stat_bonus().total()(preview.siena_part_stat_totals。部位別の内訳)
    const statTotal = preview?.siena_part_stat_totals.find((p) => p.slot === slot)?.value ?? 0;
    if (statTotal > 0) parts.push(`ステ +${fmtInt(statTotal)}`);
    const attack = sienaExtraValue(siena, "attack_rate");
    if (attack > 0) parts.push(`攻撃力 +${attack}%`);
    return parts.length > 0 ? parts.join(" ・ ") : "—";
  };
  /** 行に出すバッジ。段階 10 だと 13 個になるので上位だけ出し、残りは「+N」で畳む(§00 01) */
  const SIENA_BADGE_MAX = 4;
  const sienaBadges = (slot: SienaPartSlot) => {
    const siena = sienaForDisplay(slot);
    const rows: { key: string; text: string; title: string; modeled: boolean }[] = [];
    siena.slots.forEach((s, i) => {
      const def = sienaValueDef(s.kind);
      if (def) rows.push({
        key: `v${i}`, text: `${def.short}${s.value}${def.unit}`,
        title: `${def.label} +${s.value}${def.unit}`, modeled: def.is_modeled,
      });
    });
    siena.extras.forEach((e, i) => {
      const def = sienaExtraDef(e.kind);
      if (def) rows.push({
        key: `e${i}`, text: `${def.short}${e.value}${def.unit}`,
        title: `${def.label} +${e.value}${def.unit}`, modeled: def.is_modeled,
      });
    });
    return rows;
  };
</script>

<svelte:window onkeydown={closeSienaOnEscape} />

<div class="card">
  <p class="hint dim">
    wiki「装備システム/シエナのオーラ」。オーラは装備から抽出して、同じ部位の別装備へ注入できます。
    そのため装備とは別に登録し、<b>部位ごとに装着中の 1 件だけ</b>を計算へ反映します。
    中身は再抽選のランダム値なので、<b>スロットに出ているものを 1 個ずつ選んで足します</b>。
    <b>増幅段階は足したスロットの数</b>で、段階 3/7/10 で追加オプションの枠が 1/2/3 個開きます。
    効果値は触らなければレンジ上限で計算します(再抽選で振り直せるため)。
    グレーの枠は<b>記録するだけ</b>(防御側・HP/MP/SP など未収録の概念)で計算には入りません。
  </p>
</div>
<div class="part-list">
  {#each SIENA_ALLOWED_SLOTS as slot (slot)}
    {@const list = sienaList(slot)}
    {@const current = sienaRegistration(slot)}
    {@const siena = sienaForDisplay(slot)}
    {@const stage = sienaStage(siena)}
    {@const badges = sienaBadges(slot)}
    <button type="button" class="part-row" class:on={openSienaPart === slot} onclick={() => (openSienaPart = slot)}>
      <span class="siena-mark" class:off={current === null} aria-hidden="true">◆</span>
      <span class="part-main">
        <span class="part-name">{PART_SLOT_LABELS[slot]}</span>
        <span class="part-item">{current?.label || (current ? `オーラ ${current.id}` : "未装着")}</span>
        <span class="part-abi" use:bump={() => list.registered.length}>登録 {list.registered.length}</span>
        <span class="part-plus wide" class:on={stage > 0}>{stage > 0 ? `${stage} 段階` : ""}</span>
      </span>
      <span class="ro-badges">
        {#each badges.slice(0, SIENA_BADGE_MAX) as b (b.key)}
          <span class="ro-badge" class:record-only={!b.modeled} title={b.title}>{b.text}</span>
        {/each}
        {#if badges.length > SIENA_BADGE_MAX}
          <span class="ro-badge more">+{badges.length - SIENA_BADGE_MAX}</span>
        {/if}
      </span>
      <span class="part-vals num dim" use:flash={() => `${list.selected_id}:${sienaSummary(slot)}`}>{sienaSummary(slot)}</span>
      <span class="chev dim">›</span>
    </button>
    {#if list.registered.length > 0}
      <div class="part-switches siena-quick-switches" aria-label={`${PART_SLOT_LABELS[slot]}のオーラ切替`}>
        <button type="button" class:on={list.selected_id === null} onclick={() => (list.selected_id = null)}>未装着</button>
        {#each list.registered as entry (entry.id)}
          <button type="button" class:on={entry.id === list.selected_id} onclick={() => (list.selected_id = entry.id)}>
            <span class="siena-mini-mark" aria-hidden="true">◆</span>{entry.label || `オーラ ${entry.id}`}
          </button>
        {/each}
      </div>
    {/if}
  {/each}
</div>
{#if openSienaPart !== null}
  {@const slot = openSienaPart}
  {@const list = sienaList(slot)}
  {@const registration = sienaRegistration(slot)}
  {@const siena = sienaForDisplay(slot)}
  {@const stage = sienaStage(siena)}
  {@const capacity = sienaCapacity(slot)}
  <div class="equipment-overlay modal-overlay" role="presentation">
    <div class="part-detail modal-surface pane-in" role="dialog" aria-modal="true" aria-label={`${PART_SLOT_LABELS[slot]}のシエナのオーラ`}>
      <div class="part-detail-header">
        <b>{PART_SLOT_LABELS[slot]}のシエナのオーラ</b>
        <button type="button" class="btn close-equipment" onclick={() => (openSienaPart = null)}>閉じる <span aria-hidden="true">×</span></button>
      </div>
      <div class="part-actions siena-registration-actions">
        <div class="part-switches" aria-label="装着するオーラ">
          <button type="button" class:on={list.selected_id === null} onclick={() => (list.selected_id = null)}>未装着</button>
          {#each list.registered as entry (entry.id)}
            <button type="button" class:on={entry.id === list.selected_id} onclick={() => (list.selected_id = entry.id)}>
              <span class="siena-mini-mark" aria-hidden="true">◆</span>{entry.label || `オーラ ${entry.id}`}
            </button>
          {/each}
        </div>
        <button type="button" class="btn primary" onclick={() => createSienaRegistration(slot)}>＋ 新しいオーラを登録</button>
      </div>
      {#if registration === null}
        <div class="card empty siena-unattached">
          <p class="hint dim">この部位は未装着です。登録済みのオーラを選ぶか、新しく登録してください。</p>
        </div>
      {:else}
        <div class="card registration-name-card">
          <label class="text custom-name">
            <span class="label">登録名 <span class="dim">同じ部位のオーラを見分ける名前</span></span>
            <input type="text" bind:value={registration.label} maxlength="40" placeholder="例: 火力用" />
          </label>
          <button type="button" class="chip quiet siena-delete" onclick={() => removeSelectedSienaRegistration(slot)}>この登録を削除</button>
        </div>
        <div class="card">
          <div class="card-title inline">
            {PART_SLOT_LABELS[slot]}: 能力値スロット
            <span class="dim normal num" use:bump={() => stage}>{stage} / {app.siena.stage_max} 段階</span>
        </div>
        <!-- 足す場所は**行より上**。下に置くと、1 個足すたびに押したチップが
             行の高さぶん下へ逃げる(§09 規則 1)。足したものは真下に増える -->
        {#if stage < app.siena.stage_max}
          <div class="ro-add-row">
            {#each sienaValueDefs(slot) as def (def.kind)}
              <button
                type="button"
                class="chip add"
                class:record-only={!def.is_modeled}
                title={def.note}
                onclick={() => addSienaSlot(slot, def.kind)}
              >＋ {def.label}</button>
            {/each}
          </div>
        {:else}
          <p class="hint dim">段階 {app.siena.stage_max} まで埋まりました。変えるときは外してから足します。</p>
        {/if}
        {#each siena.slots as s, index (index)}
          {@const def = sienaValueDef(s.kind)}
          {#if def}
            <!-- 1 スロット 1 行。種類 / 値 / 外す を列でそろえる(§00 01)。
                 効き先の但し書きは title に入れ、行は増やさない -->
            <div class="siena-row swap-in" class:record-only={!def.is_modeled}>
              <span class="ro-name" title="{def.label}{def.note ? ` — ${def.note}` : ''}">{def.label}</span>
              <StatInput
                label=""
                min={def.min}
                max={def.max}
                format={def.min > 1 ? () => `wiki ${def.min}–${def.max}${def.unit}` : undefined}
                bind:value={() => s.value, (v) => (s.value = v)}
                stepper
              />
              <button type="button" class="clear" onclick={() => removeSienaSlot(slot, index)}>外す</button>
            </div>
          {/if}
        {/each}
        </div>
        <div class="card">
        <div class="card-title inline">
          追加オプション
          <span class="dim normal num" use:bump={() => capacity}>
            {siena.extras.length} / {capacity} 枠
          </span>
        </div>
        {#if capacity === 0}
          <p class="hint dim">
            段階 {app.siena.extra_unlock_stages[0]} で 1 枠目が開きます(いま段階 {stage})。
          </p>
        {:else}
          {#if siena.extras.length < capacity}
            <div class="ro-add-row">
              {#each sienaAddableExtras(slot) as def (def.kind)}
                <button
                  type="button"
                  class="chip add"
                  class:record-only={!def.is_modeled}
                  title={def.note}
                  onclick={() => addSienaExtra(slot, def.kind)}
                >＋ {def.label}</button>
              {/each}
            </div>
          {:else}
            <p class="hint dim">
              いまの段階で開いている {capacity} 枠は埋まりました。次は段階
              {app.siena.extra_unlock_stages[capacity] ?? app.siena.stage_max} で開きます。
            </p>
          {/if}
          {#each siena.extras as e, index (index)}
            {@const def = sienaExtraDef(e.kind)}
            {#if def}
              <div class="siena-row swap-in" class:record-only={!def.is_modeled}>
                <span class="ro-name" title="{def.label} — {def.note}">
                  {def.label}
                  <span class="siena-to">{def.note}</span>
                </span>
                {#if sienaChoicesAreRun(def.choices)}
                  <StatInput
                    label=""
                    min={def.choices[0]}
                    max={def.choices[def.choices.length - 1]}
                    format={def.choices[0] > 1
                      ? () => `wiki ${def.choices[0]}–${def.choices[def.choices.length - 1]}${def.unit}`
                      : undefined}
                    bind:value={() => e.value, (v) => (e.value = v)}
                    stepper
                  />
                {:else}
                  <!-- 飛び飛びの値(中ディレイ 0.5 / 1 / 2%)はステッパーだと無い値を作れてしまう -->
                  <StepSelect
                    label=""
                    options={def.choices.map((c) => ({ value: String(c), label: `${c}${def.unit}` }))}
                    bind:value={() => String(e.value), (v) => (e.value = Number(v))}
                  />
                {/if}
                <button type="button" class="clear" onclick={() => removeSienaExtra(slot, index)}>外す</button>
              </div>
            {/if}
          {/each}
        {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}
