<script lang="ts">
  // ゼリッピの答えの吹き出し。結論文(LLM · 検証済) → 状態チップ → 手順 → missing の定型文 →
  // リアクション、の順(design-system §00・s6.txt「画面」)。該当なし(kind:"none")は
  // 定型文 + 検索結果一覧(段階 0 と同じ形)。
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { errorMessage } from "../../api/commands";
  import type { AskAnswer, AskNone, NextQuestion, PrevTurn, Unit } from "../../ask";
  import { reportError } from "../../toast.svelte";
  import { collectUnits } from "./leadLine";
  import LeadLine from "./LeadLine.svelte";
  import { followupLine, missingLine, NEXT_LABEL, noLeadLine } from "./lines";
  import Playbook from "./Playbook.svelte";
  import Reaction from "./Reaction.svelte";
  import StepBlock from "./StepBlock.svelte";

  interface Props {
    question: string;
    response: AskAnswer | AskNone;
    /** 選択中キャラの Lv(登録キャラから)。未選択は null */
    level: number | null;
    /** 続きバッジの × を押したとき: 同じ質問を prev: null で聞き直す(新しいターンとして下に積む) */
    onRestartFresh: (question: string) => void;
    /** 次の一手のチップを押したとき: 今の答えのページを prev にして聞く */
    onAskNext: (next: NextQuestion, prev: PrevTurn | null) => void;
  }
  let { question, response, level, onRestartFresh, onAskNext }: Props = $props();

  const unitsById = $derived(response.kind === "answer" ? collectUnits(response.steps) : new Map<string, Unit>());
  const allUnitIds = $derived(response.kind === "answer" ? [...unitsById.keys()] : []);
  /** 今の答えの先頭手順のページ(次の一手を聞くときの prev.page) */
  const currentPage = $derived(response.kind === "answer" ? (response.steps[0]?.source.page ?? null) : null);
  const followup = $derived(response.kind === "answer" ? (response.followup ?? null) : null);
  const basis = $derived(response.kind === "answer" ? (response.basis ?? []) : []);

  function askNext(nq: NextQuestion) {
    onAskNext(nq, currentPage ? { question, page: currentPage } : null);
  }

  async function open(url: string) {
    try {
      await openUrl(url);
    } catch (e) {
      reportError(errorMessage(e));
    }
  }
</script>

{#if response.kind === "answer"}
  <div class="answer">
    {#if followup}
      <div class="followup badge-in">
        <span class="followup-label">{followupLine(followup.page)}</span>
        <button type="button" class="followup-close" aria-label="続きの判定をやり直す" onclick={() => onRestartFresh(question)}>×</button>
      </div>
    {/if}

    {#if response.lead.length > 0}
      <p class="lead pop-in">
        <LeadLine segments={response.lead} {unitsById} corrections={response.corrections} />
        <span class="tag llm">LLM · 検証済</span>
      </p>
    {:else if response.steps.length > 0}
      <p class="lead pop-in">{noLeadLine()}</p>
    {/if}

    {#if level !== null}
      <div class="state-chip">
        <span class="k">あなたの状態(登録キャラから)</span>
        <span class="v num">Lv {level}</span>
      </div>
    {/if}

    {#each response.steps as step, i (`${step.source.page}:${i}`)}
      <div class="pane-in">
        <StepBlock {step} corrections={response.corrections} {level} {basis} answerId={response.answer_id} unitIds={allUnitIds} {question} />
      </div>
    {/each}

    {#if response.missing && response.missing.length > 0}
      <ul class="missing">
        {#each response.missing as aspect (aspect)}
          <li>{missingLine(aspect)}</li>
        {/each}
      </ul>
    {/if}

    {#if response.next.length > 0}
      <div class="next pop-in">
        <span class="next-label">{NEXT_LABEL}</span>
        {#each response.next as nq (nq.page)}
          <button type="button" class="chip quiet" onclick={() => askNext(nq)}>{nq.question}</button>
        {/each}
      </div>
    {/if}

    {#if response.playbook === "cant_win"}
      <Playbook />
    {/if}

    <Reaction answerId={response.answer_id} unitIds={allUnitIds} {question} />
  </div>
{:else}
  <div class="answer">
    {#if response.search.length > 0}
      <ul class="hits pop-in">
        {#each response.search as hit (hit.id)}
          <li class="hit">
            <div class="hit-head">
              <span class="hit-page">{hit.page}</span>
              {#if hit.section}<span class="hit-sep" aria-hidden="true">›</span><span class="hit-section">{hit.section}</span>{/if}
            </div>
            <p class="hit-snippet">{hit.snippet}</p>
            <button type="button" class="link" onclick={() => open(hit.url)}>
              {hit.truncated ? "続きを wiki で読む" : "wiki で見る"}
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    {#if response.playbook === "cant_win"}
      <Playbook />
    {/if}
  </div>
{/if}

<style>
  .answer { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .lead { margin: 0; font-size: 11.5px; font-weight: 700; color: var(--sim-fg); line-height: 1.6; }
  .lead :global(.lead-value) { color: var(--fg); }
  .tag { margin-left: 6px; font-size: 8px; font-weight: 700; color: var(--sim); border: 1px solid var(--sim); border-radius: var(--r-inset); padding: 0 5px; vertical-align: middle; }
  .state-chip { display: flex; align-items: baseline; gap: 6px; font-size: 9.5px; align-self: flex-start; background: var(--bg-panel); border: 1px solid var(--border-soft); border-radius: var(--r-inset); padding: 2px 8px; }
  .state-chip .k { color: var(--fg-muted); }
  .state-chip .v { font-weight: 700; color: var(--fg); }
  .missing { margin: 0; padding: 0 0 0 14px; font-size: 10px; color: var(--fg-muted); display: flex; flex-direction: column; gap: 2px; }

  /* 続きバッジ。押した場所(答えの吹き出し)は動かさない — 上に載せるだけ */
  .followup {
    align-self: flex-start; display: flex; align-items: center; gap: 5px;
    background: var(--state-met-bg); color: var(--state-met-fg); border-radius: var(--r-inset);
    padding: 2px 4px 2px 8px; font-size: 9px; font-weight: 700;
  }
  .followup-close { background: none; border: none; padding: 0 2px; font: inherit; color: inherit; cursor: pointer; opacity: 0.7; }
  .followup-close:hover { opacity: 1; }

  .next { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .next-label { font-size: 9.5px; color: var(--fg-dim); }

  .hits { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .hit { padding: 7px 10px; border-radius: var(--r-inset); background: var(--bg-panel); border: 1px solid var(--border-soft); }
  .hit-head { display: flex; align-items: baseline; gap: 5px; min-width: 0; margin-bottom: 2px; }
  .hit-page { font-size: 10.5px; font-weight: 700; color: var(--fg-head); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .hit-sep { flex: none; color: var(--fg-dim); font-size: 10px; }
  .hit-section { min-width: 0; font-size: 10px; color: var(--fg-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .hit-snippet { margin: 0; font-size: 10.5px; color: var(--fg-sub); line-height: 1.6; }
  .link { margin-top: 4px; background: none; border: none; padding: 0; font-size: 9.5px; color: var(--accent); cursor: pointer; text-decoration: underline; }
</style>
