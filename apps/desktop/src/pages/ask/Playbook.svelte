<script lang="ts">
  // 困りごとの型「勝てない」の打ち手(stage3-spec.md A)。装備の中身はサーバーに送らない —
  // サーバーは「困りごとは cant_win」という印だけを返し、この部品が端末のローカルの計算(登録キャラ)
  // から打ち手を描く。書き手はすべてコード(LLM の文は無い)。wiki の答えが none でも出す。
  import { previewDamage, previewDefense } from "../../api/commands";
  import type { DefenseProfile } from "../../api/types";
  import { fmtInt, fmtPct } from "../../format";
  import { app, buffSelectionFor, currentDamageTarget, payloadOf, selectedCharacter } from "../../state.svelte";
  import { latest } from "../../ui/latest.svelte";
  import Value from "../../ui/Value.svelte";
  import {
    CRITICAL_SOURCE_LABELS,
    PLAYBOOK_COST_ORDER_NOTE,
    PLAYBOOK_CRIT_TITLE,
    PLAYBOOK_DEFENSE_MOVES,
    PLAYBOOK_DEFENSE_TITLE,
    PLAYBOOK_GO_TO_CALC,
    PLAYBOOK_INTRO,
    PLAYBOOK_NO_CHARACTER,
    PLAYBOOK_TAG,
  } from "./lines";

  const character = $derived(selectedCharacter());

  let defense = $state<DefenseProfile | null>(null);
  /** 敵の AGI・被撃率・スキルの Cri 値が揃っているときだけ非 null(0..1) */
  let criticalChance = $state<number | null>(null);
  const requestLatest = latest();

  $effect(() => {
    const c = character;
    defense = null;
    criticalChance = null;
    if (!c) return;
    const buffs = buffSelectionFor(c);
    requestLatest.run(async (isCurrent) => {
      try {
        const d = await previewDefense(payloadOf(c), buffs);
        if (isCurrent()) defense = d;
      } catch {
        // 未収録・失敗は「?」のまま(打ち手そのものは静的なので致命的ではない)
      }
      const target = currentDamageTarget();
      const skillId = c.main_skill_id;
      if (!skillId || !target) return;
      try {
        const result = await previewDamage(payloadOf(c), skillId, target.id, 0, null, null, buffs);
        if (isCurrent()) criticalChance = result.body.critical_rate !== null ? result.body.critical_chance : null;
      } catch {
        // 同上
      }
    });
    return () => requestLatest.cancel();
  });

  /** クリティカル率の 4 供給源。費用の安い順に「使用中 / 未使用」(stage3-spec.md A-3) */
  const criticalSources = $derived.by(() => {
    const s = character?.stat_sources.critical_rate;
    if (!s) return [];
    return [
      { label: CRITICAL_SOURCE_LABELS.pet, used: s.pet },
      { label: CRITICAL_SOURCE_LABELS.deadly_blow, used: s.deadly_blow },
      { label: CRITICAL_SOURCE_LABELS.ultimate_rune, used: s.ultimate_rune },
      { label: CRITICAL_SOURCE_LABELS.architect_lab_stage, used: s.architect_lab_stage > 0 },
    ];
  });

  function goToCalc() {
    app.tab = "calc";
  }
</script>

<div class="playbook pop-in">
  <p class="intro"><span class="line">{PLAYBOOK_INTRO}</span><span class="tag code">{PLAYBOOK_TAG} コード</span></p>

  {#if !character}
    <p class="no-character">{PLAYBOOK_NO_CHARACTER}</p>
  {:else}
    <div class="causes">
      <div class="cause">
        <p class="cause-title">{PLAYBOOK_CRIT_TITLE}</p>
        <div class="crit-rate">
          <span class="k">クリティカル率</span>
          <Value value={criticalChance !== null ? fmtPct(criticalChance) : null} class="v" />
        </div>
        <ul class="moves">
          {#each criticalSources as source (source.label)}
            <li>
              <span class="move-label">{source.label}</span>
              <span class="badge" class:used={source.used}>{source.used ? "使用中" : "未使用"}</span>
            </li>
          {/each}
        </ul>
      </div>

      <div class="cause">
        <p class="cause-title">{PLAYBOOK_DEFENSE_TITLE}</p>
        <div class="defense-values">
          <span class="dv"><span class="k">防御</span><Value value={defense ? fmtInt(defense.physical_defense) : null} class="v" /></span>
          <span class="dv"><span class="k">カット率</span><Value value={defense ? fmtPct(defense.physical_cut_rate) : null} class="v" /></span>
          <span class="dv"><span class="k">回避</span><Value value={defense ? fmtInt(defense.evasion_point.physical) : null} class="v" /></span>
        </div>
        <ul class="moves plain">
          {#each PLAYBOOK_DEFENSE_MOVES as move (move)}
            <li>{move}</li>
          {/each}
        </ul>
      </div>
    </div>

    <p class="footer">
      {PLAYBOOK_COST_ORDER_NOTE}
      <button type="button" class="link" onclick={goToCalc}>{PLAYBOOK_GO_TO_CALC}</button>
    </p>
  {/if}
</div>

<style>
  .playbook {
    display: flex; flex-direction: column; gap: 8px; padding: 8px 10px; border-radius: var(--r-inset);
    background: var(--bg-panel); border: 1px solid var(--border-soft);
  }
  .intro { margin: 0; font-size: 10.5px; font-weight: 700; color: var(--fg-head); }
  .tag { margin-left: 6px; font-size: 8px; font-weight: 700; border-radius: var(--r-inset); padding: 0 5px; vertical-align: middle; }
  .tag.code { background: var(--state-met-bg); color: var(--state-met-fg); }
  .no-character { margin: 0; font-size: 10.5px; color: var(--fg-muted); }

  .causes { display: flex; flex-direction: column; gap: 10px; }
  .cause { display: flex; flex-direction: column; gap: 5px; }
  .cause-title { margin: 0; font-size: 10px; font-weight: 700; color: var(--fg-head); }

  .crit-rate { display: flex; align-items: baseline; gap: 6px; font-size: 9.5px; }
  .crit-rate .k { color: var(--fg-muted); }
  .crit-rate :global(.v) { font-weight: 700; }

  .defense-values { display: flex; flex-wrap: wrap; gap: 10px; }
  .dv { display: flex; align-items: baseline; gap: 4px; font-size: 9.5px; }
  .dv .k { color: var(--fg-muted); }
  .dv :global(.v) { font-weight: 700; min-width: 3em; text-align: right; }

  .moves { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 3px; }
  .moves li { display: flex; align-items: center; gap: 6px; font-size: 9.5px; color: var(--fg-sub); }
  .moves.plain li { color: var(--fg-sub); }
  .move-label { flex: 1; min-width: 0; }
  .badge { flex: none; font-size: 8.5px; font-weight: 700; border-radius: var(--r-inset); padding: 1px 6px; white-space: nowrap; background: var(--bg-field); border: 1px solid var(--border-soft); color: var(--fg-muted); }
  .badge.used { background: var(--state-met-bg); border-color: var(--state-met-bd); color: var(--state-met-fg); }

  .footer { margin: 0; font-size: 9px; color: var(--fg-dim); display: flex; align-items: center; gap: 6px; }
  .link { background: none; border: none; padding: 0; font: inherit; color: var(--accent); cursor: pointer; text-decoration: underline; }
</style>
