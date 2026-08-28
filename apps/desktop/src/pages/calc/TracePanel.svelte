<script lang="ts">
  // 詳細トレース(能力値・カテゴリ・式の各段)。「なぜこの数字？」の最深部。
  import type { CategoryTrace, DamageTrace, StatContribution, StatTrace } from "../../api/types";
  import { fmtInt, fmtNum, formatLayerValue } from "../../format";
  import { STAT_KINDS, STAT_LABELS, STAT_LAYER_LABELS } from "../../labels";
  import { bump } from "../../ui/motion.svelte";

  let { trace }: { trace: DamageTrace } = $props();

  // pin(能力値の固定)は計算タブの一時調整だけから来る。
  function pinnedBeforeLabel(s: StatTrace): string {
    if (s.pinned_from === null) return "";
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

  /** カテゴリ供給源内訳。カテゴリの並び(trace.categories = 式に現れる順)ごとにまとめる */
  const categoryContributions = $derived(
    trace.categories.flatMap((c) =>
      trace.category_contributions
        .filter((x) => x.category === c.category)
        .map((x) => ({ ...x, symbol: c.symbol, label: c.label, kind: c.kind })),
    ),
  );
  const fmtContributionValue = (kind: CategoryTrace["kind"], v: number) =>
    kind === "rate" ? `${v >= 0 ? "+" : ""}${fmtNum(v * 100)}%` : fmtNum(v);
</script>

<details class="trace">
  <summary>
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5L10.5 8 6 12.5"/></svg>
    <span>詳細トレース</span>
    <span class="dim">能力値 {trace.stats.length} / カテゴリ {trace.categories.length} / 式 {trace.steps_max.length} 段</span>
  </summary>

  <div class="section-label"><span>(a) 能力値計算</span><span class="rule"></span></div>
  <div class="tbl">
    <table class="grid ro">
      <thead><tr>
        <th>ステ</th><th class="n">最終</th><th class="n">素</th><th class="n">Σ割合</th><th class="n">固定</th><th class="n">Π倍率A</th>
        <th class="n">基本</th><th class="n">倍率B</th><th class="n">[基本×B]</th><th class="n">最終固定</th>
        <th class="n">上限</th><th class="n">上限で捨てた分</th>
      </tr></thead>
      <tbody>
        {#each trace.stats as s (s.kind)}
          <tr>
            <td>{STAT_LABELS[s.kind]}</td>
            <td class="n strong final">
              <span use:bump={() => s.effective}>{fmtInt(s.effective)}</span>
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
            <td class="n dim">{fmtInt(s.stat_cap)}</td>
            <td class="n" class:capped={s.capped_loss > 0}>{s.capped_loss > 0 ? fmtInt(s.capped_loss) : "—"}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if trace.stats.some((s) => s.capped_loss > 0)}
    <p class="cap-note">
      最終能力値が上限({fmtInt(trace.stats[0].stat_cap)})で頭打ちになっています。上限は覚醒段階とエタの意志 Lv で上がります(wiki: Quest/覚醒クエスト・エタの意志)。
    </p>
  {/if}

  <div class="section-label"><span>(a-1) 補正源内訳</span><span class="rule"></span></div>
  <div class="tbl">
    {#if contributions.length === 0}
      <p class="empty dim">補正源なし(素ステのみ)</p>
    {:else}
      <table class="grid ro">
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
    <table class="grid ro">
      <thead><tr>
        <th>記号</th><th>カテゴリ</th><th>種別</th><th class="n">集計値</th><th class="n">係数</th><th class="n">キャップ</th>
      </tr></thead>
      <tbody>
        {#each trace.categories as c (c.category)}
          <tr class:active={isActive(c)}>
            <td class="sym">{c.symbol}</td>
            <td>{c.label}</td>
            <td class="muted">{KIND_LABEL[c.kind]}</td>
            <td class="n" use:bump={() => c.value}>{fmtValue(c)}</td>
            <td class="n strong" use:bump={() => c.factor}>{fmtNum(c.factor)}</td>
            <td class="n muted">{fmtCap(c)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  <div class="section-label"><span>(b-1) カテゴリ供給源内訳</span><span class="rule"></span></div>
  <div class="tbl">
    {#if categoryContributions.length === 0}
      <p class="empty dim">供給源なし</p>
    {:else}
      <table class="grid ro">
        <thead><tr><th>記号</th><th>カテゴリ</th><th>出典</th><th class="n">値</th></tr></thead>
        <tbody>
          {#each categoryContributions as c, i (i)}
            <tr>
              <td class="sym">{c.symbol}</td>
              <td>{c.label}</td>
              <td class="muted">{c.source}</td>
              <td class="n" use:bump={() => c.value}>{fmtContributionValue(c.kind, c.value)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
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
    <table class="grid ro">
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
  .trace { border-top: 1px dashed var(--border-soft); margin-top: 12px; }
  summary {
    display: flex; align-items: center; gap: 8px; padding: 11px 2px;
    font-size: 10px; letter-spacing: 0.14em; color: var(--fg-muted); cursor: pointer; list-style: none;
    user-select: none;
  }
  summary::-webkit-details-marker { display: none; }
  summary svg { transition: transform 0.15s; }
  details[open] summary svg { transform: rotate(90deg); }
  summary:hover { color: var(--fg); }
  .section-label { padding: 10px 2px 8px; }
  /* .tbl は app.css(この画面だけ margin-bottom を持つ) */
  .tbl { margin: 0 0 8px; }
  .empty { padding: 10px 12px; font-size: 11px; }
  td.sym { font-weight: 700; color: var(--accent); }
  td.strong { font-weight: 500; }
  td.final { display: flex; align-items: center; gap: 6px; white-space: nowrap; }
  .capped { color: #B5443A; font-weight: 700; }
  .cap-note { margin: 4px 0 0; font-size: 9px; color: #B5443A; }
  /* .pin-badge は app.css */
  td.expr { white-space: normal; color: var(--fg-muted); font-size: 11px; min-width: 260px; }
  tr.active td { background: var(--bg-active); }
  tr.active td.sym { color: var(--warm); }
  .tabs { display: flex; border: 1px solid var(--border); border-radius: var(--r-inset); overflow: hidden; letter-spacing: 0; }
  .tabs button { padding: 3px 10px; background: var(--bg-field); color: var(--fg-muted); font-size: 11px; }
  .tabs button.on { background: var(--bg-active); color: var(--accent-hover); font-weight: 700; }
</style>
