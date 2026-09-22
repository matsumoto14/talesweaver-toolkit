<script lang="ts">
  // 手順 1 つ(見出し + 引用 or 選ばれた列だけの小表 + 出典)。design-system §08。
  // 値は wiki の色のまま(数値書体)、訂正があれば重ねて描く(4 色目 `--fix`)。
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { Correction, RowUnit, Step, Unit } from "../../ask";
  import { errorMessage } from "../../api/commands";
  import { reportError } from "../../toast.svelte";
  import Value from "../../ui/Value.svelte";
  import { CORRECTION_INTRO_APPARENT, CORRECTION_INTRO_CONFIRMED } from "./lines";
  import ValueWrong from "./ValueWrong.svelte";

  interface Props {
    step: Step;
    corrections: Correction[];
    /** 選択中キャラの Lv(登録キャラから)。未選択・未取得は null */
    level: number | null;
    /** 根拠になったユニット ID(段階 2)。行/段落に「根拠」バッジを付ける */
    basis: string[];
    /** 「この値は違う」を送るための answer_id・答えに出ていたユニット ID・元の質問文 */
    answerId: string;
    unitIds: string[];
    question: string;
  }
  let { step, corrections, level, basis, answerId, unitIds, question }: Props = $props();

  const MAX_ROWS = 8;
  const LEVEL_COLUMNS = new Set(["Lv", "レベル"]);

  const paragraphs = $derived(step.units.filter((u): u is Extract<Unit, { kind: "paragraph" }> => u.kind === "paragraph"));
  const rows = $derived(step.units.filter((u): u is RowUnit => u.kind === "row"));
  const shownRows = $derived(rows.slice(0, MAX_ROWS));
  const truncatedRows = $derived(rows.length > MAX_ROWS);
  const columns = $derived(step.columns && step.columns.length > 0
    ? step.columns
    : Array.from(new Set(rows.flatMap((r) => Object.keys(r.cells)))));

  function correctionFor(unitId: string, col: string): Correction | undefined {
    return corrections.find((c) => c.unit === unitId && c.col === col);
  }

  async function open(url: string) {
    try {
      await openUrl(url);
    } catch (e) {
      reportError(errorMessage(e));
    }
  }

  /** 同じ出典は 1 回だけ並べる */
  function uniqueSources(list: Correction[]): { title: string; url: string | null }[] {
    const seen = new Map<string, string | null>();
    for (const c of list) if (!seen.has(c.source.title)) seen.set(c.source.title, c.source.url);
    return [...seen].map(([title, url]) => ({ title, url }));
  }
</script>

<div class="step">
  <div class="step-head">
    <span class="step-title">{step.title}</span>
    <span class="step-source">{step.source.page}{#if step.source.section} › {step.source.section}{/if}</span>
  </div>

  {#each paragraphs as p (p.id)}
    <p class="quote">
      {p.text}
      {#if basis.includes(p.id)}<span class="badge basis">根拠</span>{/if}
    </p>
  {/each}

  {#if shownRows.length > 0}
    {#if step.filtered_by && Object.keys(step.filtered_by).length > 0}
      <p class="filtered">
        {#each Object.entries(step.filtered_by) as [k, v] (k)}{k} {v} の行だけ表示{/each}
        <span class="badge code">コード</span>
      </p>
    {/if}
    <div class="table-wrap">
      <table class="rows">
        <thead>
          <tr>{#each columns as col (col)}<th>{col}</th>{/each}</tr>
        </thead>
        <tbody>
          {#each shownRows as row (row.id)}
            <tr>
              {#each columns as col, colIndex (col)}
                {@const correction = correctionFor(row.id, col)}
                <td>
                  {#if correction}
                    <s class="wiki-value">{correction.wiki}</s>
                    <ValueWrong {answerId} unitId={row.id} {col} {unitIds} {question} value={correction.value} class="fix-value" />
                    <span class="badge fix" title={correction.source.title}>{correction.grade === "confirmed" ? "公式お知らせで訂正" : "アプリのデータで訂正"}</span>
                  {:else}
                    <ValueWrong {answerId} unitId={row.id} {col} {unitIds} {question} value={row.cells[col] ?? null} />
                  {/if}
                  {#if colIndex === 0 && basis.includes(row.id)}<span class="badge basis">根拠</span>{/if}
                  {#if LEVEL_COLUMNS.has(col) && level !== null}
                    <span class="badge you">あなたは Lv {level}</span>
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  <p class="source">
    出典:
    {#if step.source.url}
      <button type="button" class="link" onclick={() => open(step.source.url!)}>{step.source.page}#{step.source.section}</button>
    {:else}
      <span>{step.source.page}({step.source.section})</span>
    {/if}
    {#if truncatedRows && step.table}
      · <button type="button" class="link" onclick={() => open(step.table!.url)}>表の全{step.table.rows}行を wiki で見る</button>
    {:else if paragraphs.some((p) => p.truncated) && step.source.url}
      · <button type="button" class="link" onclick={() => open(step.source.url!)}>続きを wiki で読む</button>
    {/if}
  </p>

  {#if corrections.some((c) => shownRows.some((r) => r.id === c.unit))}
    <p class="correction-intro">
      {corrections.some((c) => c.grade === "confirmed") ? CORRECTION_INTRO_CONFIRMED : CORRECTION_INTRO_APPARENT}
    </p>
    <!-- 訂正の出典は必ず示す(ADR-021)。公式お知らせは URL を持つ。静的データは URL が無いので題名だけ -->
    <p class="source">
      訂正の出典:
      {#each uniqueSources(corrections.filter((c) => shownRows.some((r) => r.id === c.unit))) as src (src.title)}
        {#if src.url}
          <button type="button" class="link" onclick={() => open(src.url!)}>{src.title}</button>
        {:else}
          <span>{src.title}</span>
        {/if}
      {/each}
    </p>
  {/if}
</div>

<style>
  .step { display: flex; flex-direction: column; gap: 6px; padding: 8px 10px; border-radius: var(--r-inset); background: var(--bg-panel); border: 1px solid var(--border-soft); }
  .step-head { display: flex; align-items: baseline; gap: 6px; flex-wrap: wrap; }
  .step-title { font-size: 10.5px; font-weight: 700; color: var(--fg-head); }
  .step-source { font-size: 9px; color: var(--fg-dim); }
  .quote { margin: 0; font-size: 10.5px; line-height: 1.6; color: var(--fg-sub); border-left: 2px solid var(--border-soft); padding-left: 8px; }
  .filtered { margin: 0; font-size: 9px; color: var(--fg-muted); display: flex; align-items: center; gap: 5px; }
  .table-wrap { overflow-x: auto; }
  table.rows { border-collapse: collapse; font-size: 10px; min-width: 100%; }
  table.rows th { text-align: right; font-weight: 700; color: var(--fg-muted); padding: 2px 8px; white-space: nowrap; border-bottom: 1px solid var(--border-soft); }
  table.rows th:first-child { text-align: left; }
  table.rows td { text-align: right; padding: 2px 8px; white-space: nowrap; }
  table.rows td:first-child { text-align: left; }
  table.rows :global(.num) { min-width: 40px; }
  .wiki-value { color: var(--fg-off); margin-right: 4px; }
  table.rows :global(.fix-value) { color: var(--fix-fg); font-weight: 700; }
  .badge { font-size: 8.5px; font-weight: 700; border-radius: var(--r-inset); padding: 1px 6px; margin-left: 5px; white-space: nowrap; }
  .badge.code { background: var(--state-met-bg); color: var(--state-met-fg); }
  .badge.you { background: var(--state-met-bg); color: var(--state-met-fg); }
  .badge.basis { background: var(--state-met-bg); color: var(--state-met-fg); }
  .badge.fix { background: var(--fix-bg); color: var(--fix-fg); border: 1px solid var(--fix); }
  .source { margin: 0; font-size: 9px; color: var(--fg-dim); }
  .correction-intro { margin: 0; font-size: 9.5px; font-weight: 700; color: var(--fix-fg); }
  .link { background: none; border: none; padding: 0; font: inherit; color: var(--accent); cursor: pointer; text-decoration: underline; }
</style>
