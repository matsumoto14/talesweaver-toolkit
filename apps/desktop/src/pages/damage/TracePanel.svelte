<script lang="ts">
  import type { DamageTrace, CategoryTrace, RegisteredCharacter, StatContribution, StatTrace } from "../../api/types";
  import { STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS } from "../../labels";
  import { fmtInt, fmtNum, formatLayerValue } from "../../format";

  let { trace, character = null }: { trace: DamageTrace; character?: RegisteredCharacter | null } = $props();

  // 「固定前」の表示値: pin の出所(pin_source、サーバ側 apply_pins が決定)が temporary
  // (計算リクエストの一時調整による上書き)で、かつキャラに保存済みの固定(pin)があるときは、
  // 「自分が普段固定している値」を基準にするほうが伝わりやすいのでそちらを見せる。
  // それ以外(pin_source が saved、または保存済み固定が無い)は
  // pinned_from(この計算で pin される直前の計算値)をそのまま使う。
  function pinnedBeforeLabel(s: StatTrace): string {
    if (s.pinned_from === null) return "";
    if (s.pin_source === "temporary") {
      const savedPin = character?.stat_sources.adjustments[s.kind].pin ?? null;
      if (savedPin !== null) return `保存済みの固定 ${fmtInt(savedPin)} を一時的に上書き`;
    }
    return `固定前: ${fmtInt(s.pinned_from)}`;
  }

  const KIND_LABEL = { assigned: "代入", fixed: "固定値", rate: "割合" } as const;
  type StepTab = "min" | "max" | "critical";
  const STEP_TABS: { id: StepTab; label: string }[] = [
    { id: "min", label: "最小" },
    { id: "max", label: "最大" },
    { id: "critical", label: "クリティカル" },
  ];
  let stepTab = $state<StepTab>("max");
  const steps = $derived(
    stepTab === "min" ? trace.steps_min : stepTab === "max" ? trace.steps_max : trace.steps_critical,
  );

  /** 中立値(割合 0%・固定値 0)でない行は式に効いているのでハイライトする */
  const isActive = (c: CategoryTrace) => (c.kind === "assigned" ? true : c.value !== 0);
  const fmtCap = (c: CategoryTrace) => {
    if (!c.cap) return "—";
    const f = (v: number | null) => (v === null ? "" : c.kind === "rate" ? `${fmtNum(v * 100)}%` : fmtNum(v));
    return `${f(c.cap.min)} … ${f(c.cap.max)}`;
  };
  const fmtValue = (c: CategoryTrace) =>
    c.kind === "rate" ? `${c.value >= 0 ? "+" : ""}${fmtNum(c.value * 100)}%` : fmtNum(c.value);

  /** ステ補正源の寄与内訳。STAT_KINDS の順、同じステ内は元の配列順を保つ */
  const contributions = $derived<StatContribution[]>(
    STAT_KINDS.flatMap((k) => trace.stat_contributions.filter((c) => c.kind === k)),
  );
</script>

<details class="trace">
  <summary>
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5L10.5 8 6 12.5"/></svg>
    <span>TRACE — 計算トレース</span>
    <span class="dim">能力値 {trace.stats.length} / カテゴリ {trace.categories.length} / 式 {trace.steps_max.length} 段</span>
  </summary>

  <div class="section-label"><span>(a) 能力値計算</span><span class="rule"></span></div>
  <div class="tbl">
    <table class="grid">
      <thead><tr>
        <th>ステ</th><th class="n">最終</th><th class="n">素</th><th class="n">Σ割合</th><th class="n">固定</th><th class="n">Π倍率A</th>
        <th class="n">基本</th><th class="n">倍率B</th><th class="n">[基本×B]</th><th class="n">最終固定</th>
      </tr></thead>
      <tbody>
        {#each trace.stats as s (s.kind)}
          <tr>
            <td>{STAT_LABELS[s.kind]}</td>
            <td class="n strong final">
              <span>{fmtInt(s.effective)}</span>
              {#if s.pinned_from !== null}
                <span class="pin-badge" title={pinnedBeforeLabel(s)}>固定</span>
              {/if}
            </td>
            <td class="n">{fmtInt(s.base)}</td>
            <td class="n">{fmtInt(s.percent_of_base_total)}</td>
            <td class="n">{fmtInt(s.fixed)}</td>
            <td class="n">{fmtNum(s.multiplier_a)}</td>
            <td class="n">{fmtInt(s.basic)}</td>
            <td class="n">{fmtNum(s.multiplier_b)}</td>
            <td class="n">{fmtInt(s.multiplier_b_bonus)}</td>
            <td class="n">{fmtInt(s.final_fixed)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <div class="section-label"><span>(a-1) 補正源内訳</span><span class="rule"></span></div>
  <div class="tbl">
    {#if contributions.length === 0}
      <p class="empty dim">補正源なし(素ステのみ)</p>
    {:else}
      <table class="grid">
        <thead><tr><th>ステ</th><th>出典</th><th>層</th><th class="n">値</th></tr></thead>
        <tbody>
          {#each contributions as c, i (i)}
            <tr>
              <td>{STAT_LABELS[c.kind]}</td>
              <td class="muted">{c.source}</td>
              <td class="muted">{STAT_LAYER_LABELS[c.layer]}</td>
              <td class="n">{formatLayerValue(c.layer, c.value)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>

  <div class="section-label"><span>(b) カテゴリ集計</span><span class="rule"></span><span class="dim">ハイライト = 非中立値</span></div>
  <div class="tbl">
    <table class="grid">
      <thead><tr>
        <th>記号</th><th>カテゴリ</th><th>種別</th><th class="n">集計値</th><th class="n">係数</th><th class="n">キャップ</th>
      </tr></thead>
      <tbody>
        {#each trace.categories as c (c.category)}
          <tr class:active={isActive(c)}>
            <td class="sym">{c.symbol}</td>
            <td>{c.label}</td>
            <td class="muted">{KIND_LABEL[c.kind]}</td>
            <td class="n">{fmtValue(c)}</td>
            <td class="n strong">{fmtNum(c.factor)}</td>
            <td class="n muted">{fmtCap(c)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <div class="section-label">
    <span>(c) 式の各段</span><span class="rule"></span>
    <span class="tabs">
      {#each STEP_TABS as t (t.id)}
        <button type="button" class:on={stepTab === t.id} onclick={() => (stepTab = t.id)}>{t.label}</button>
      {/each}
    </span>
  </div>
  <div class="tbl">
    <table class="grid">
      <thead><tr><th>#</th><th>段</th><th>式</th><th class="n">値</th></tr></thead>
      <tbody>
        {#each steps as s, i (i)}
          <tr>
            <td class="dim">{String(i + 1).padStart(2, "0")}</td>
            <td>{s.name}</td>
            <td class="expr">{s.expression}</td>
            <td class="n strong">{fmtNum(s.value)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
</details>

<style>
  .trace { border-top: 1px solid var(--border); }
  summary {
    display: flex; align-items: center; gap: 8px; padding: 11px 16px;
    font-size: 10px; letter-spacing: 0.14em; color: var(--fg-muted); cursor: pointer; list-style: none;
    user-select: none;
  }
  summary::-webkit-details-marker { display: none; }
  summary svg { transition: transform 0.15s; }
  details[open] summary svg { transform: rotate(90deg); }
  summary:hover { color: var(--fg); }
  .tbl { overflow-x: auto; margin: 0 16px 8px; border: 1px solid var(--border-soft); }
  .empty { padding: 10px 12px; font-size: 11px; }
  td.sym { font-weight: 700; color: var(--accent); }
  td.strong { font-weight: 500; }
  td.final { display: flex; align-items: center; gap: 6px; white-space: nowrap; }
  .pin-badge {
    font-size: 9px; letter-spacing: 0.05em; color: var(--accent); border: 1px solid var(--accent);
    padding: 1px 4px; cursor: default;
  }
  td.expr { white-space: normal; color: var(--fg-muted); font-size: 11px; min-width: 260px; }
  tr.active td { background: oklch(0.23 0.025 200); }
  tr.active td.sym { color: var(--warm); }
  .tabs { display: flex; border: 1px solid var(--border); letter-spacing: 0; }
  .tabs button {
    padding: 3px 10px; border: 0; background: var(--bg-field); color: var(--fg-muted); font-size: 11px;
  }
  .tabs button.on { background: var(--bg-active); color: var(--accent); }
</style>
