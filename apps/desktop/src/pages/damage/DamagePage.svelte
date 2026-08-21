<script lang="ts">
  import { untrack } from "svelte";
  import {
    calculateDamage, errorMessage, listCharacters, listEnemies, listGameCharacters, listSkills,
  } from "../../api/commands";
  import { STAT_KINDS, STAT_LABELS } from "../../labels";
  import type { Adjustments, DamageResult, Enemy, GameCharacter, RegisteredCharacter, Skill } from "../../api/types";
  import { limits } from "../../limits.svelte";
  import { fmtInt, fmtNum } from "../../format";
  import { reportError } from "../../toast.svelte";
  import AdjustmentEditor from "../../ui/AdjustmentEditor.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Select from "../../ui/Select.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import TracePanel from "./TracePanel.svelte";

  const DEFAULT_INPUT_WIDTH = 336;
  const DEFAULT_TARGET_WIDTH = 296;
  const layoutWidths = persisted("tw-layout-damage", { input: DEFAULT_INPUT_WIDTH, target: DEFAULT_TARGET_WIDTH });
  const gridTemplateColumns = $derived(
    `minmax(200px, ${layoutWidths.value.input ?? DEFAULT_INPUT_WIDTH}px) 6px minmax(180px, ${layoutWidths.value.target ?? DEFAULT_TARGET_WIDTH}px) 6px minmax(240px, 1fr)`,
  );

  interface Props {
    /** ヘッダーの「計算」ボタンから再計算させるための呼び出し口 */
    registerRecalculate: (fn: () => void) => void;
  }
  let { registerRecalculate }: Props = $props();

  const COMBO_THRESHOLD = 3;

  let characters = $state<RegisteredCharacter[]>([]);
  let gameCharacters = $state<GameCharacter[]>([]);
  let enemies = $state<Enemy[]>([]);
  let skills = $state<Skill[]>([]);

  let characterId = $state("");
  let skillId = $state("");
  let enemyId = $state("");
  let combo = $state(false);

  let result = $state<DamageResult | null>(null);
  let calculating = $state(false);

  // 一時調整: キャラには保存せず、計算リクエストにのみ乗せる(docs/ux-guidelines.md 原則4)。
  const neutralAdjustments = (): Adjustments =>
    Object.fromEntries(STAT_KINDS.map((k) => [k, { add: 0, pin: null }])) as Adjustments;
  let temporaryAdjustments = $state<Adjustments>(neutralAdjustments());
  const hasTemporaryAdjustments = $derived(
    STAT_KINDS.some((k) => temporaryAdjustments[k].add !== 0 || temporaryAdjustments[k].pin !== null),
  );

  const character = $derived(characters.find((c) => String(c.id) === characterId) ?? null);
  const skill = $derived(skills.find((s) => s.id === skillId) ?? null);
  const enemy = $derived(enemies.find((e) => e.id === enemyId) ?? null);
  const gameName = (id: string) => gameCharacters.find((g) => g.id === id)?.name ?? id;

  $effect(() => {
    Promise.all([listCharacters(), listGameCharacters(), listEnemies()])
      .then(([cs, gs, es]) => {
        characters = cs;
        gameCharacters = gs;
        enemies = es;
        if (cs.length === 1) characterId = String(cs[0].id);
        if (es.length === 1) enemyId = es[0].id;
      })
      .catch((e) => reportError(errorMessage(e)));
  });

  // キャラが変わったら、そのゲームキャラのスキル一覧を引き直す
  $effect(() => {
    const gameCharacterId = character?.game_character_id;
    if (!gameCharacterId) {
      skills = [];
      skillId = "";
      return;
    }
    listSkills(gameCharacterId)
      .then((list) => {
        skills = list;
        if (!list.some((s) => s.id === skillId)) skillId = list[0]?.id ?? "";
      })
      .catch((e) => reportError(errorMessage(e)));
  });

  // キャラを切り替えたら一時調整・結果をリセットする(前のキャラの一時調整を引き継がない)
  let lastCharacterId = untrack(() => character?.id);
  $effect(() => {
    const id = character?.id;
    if (id === lastCharacterId) return;
    lastCharacterId = id;
    temporaryAdjustments = neutralAdjustments();
    result = null;
  });

  // 選択が揃ったら自動計算。古いリクエストの応答は捨てる
  let requestSeq = 0;
  function calculate() {
    if (!character || !skillId || !enemyId) {
      result = null;
      return;
    }
    const seq = ++requestSeq;
    calculating = true;
    calculateDamage(character.id, skillId, enemyId, combo ? COMBO_THRESHOLD : 0, temporaryAdjustments)
      .then((r) => { if (seq === requestSeq) result = r; })
      .catch((e) => { if (seq === requestSeq) { result = null; reportError(errorMessage(e)); } })
      .finally(() => { if (seq === requestSeq) calculating = false; });
  }
  let debounceHandle: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    // 依存を明示的に読む。$state はプロパティ単位で追跡されるため、一時調整は各ステの
    // fixed/final_fixed を明示的に読む必要がある。
    void [character?.id, skillId, enemyId, combo];
    for (const k of STAT_KINDS) void [temporaryAdjustments[k].add, temporaryAdjustments[k].pin];
    if (debounceHandle) clearTimeout(debounceHandle);
    debounceHandle = setTimeout(calculate, 100);
    return () => {
      if (debounceHandle) clearTimeout(debounceHandle);
    };
  });
  $effect(() => {
    registerRecalculate(calculate);
  });

  const characterOptions = $derived(characters.map((c) => ({ value: String(c.id), label: `${c.name} (${gameName(c.game_character_id)})` })));
  const skillOptions = $derived(skills.map((s) => ({ value: s.id, label: s.name })));
  const enemyOptions = $derived(enemies.map((e) => ({ value: e.id, label: e.name })));
</script>

<div class="layout" style="grid-template-columns: {gridTemplateColumns};">
  <!-- INPUT -->
  <section>
    <div class="panel-head"><span class="dot"></span><span class="title">INPUT — 入力</span></div>
    <div class="scroll">
      <div class="block">
        <Select label="キャラ" bind:value={characterId} options={characterOptions}
          placeholder={characters.length === 0 ? "登録キャラがありません" : "選択してください"}
          disabled={characters.length === 0} />
        <Select label="スキル" bind:value={skillId} options={skillOptions} disabled={!character} />
        {#if skill}
          <div class="meta dim">
            <span>依存 {skill.dependency.toUpperCase().replace("_", "+")}</span>
            <span>倍率 ×{fmtNum(skill.multiplier)}</span>
            <span>{skill.hit_count} 段</span>
            <span>Cri ×{fmtNum(skill.critical_multiplier)}</span>
          </div>
        {/if}
      </div>

      <div class="section-label"><span>素ステータス</span><span class="rule"></span><span class="note">自動反映</span></div>
      {#each STAT_KINDS as k (k)}
        <div class="kv"><span class="k">{STAT_LABELS[k]}</span><span class="v">{character ? fmtInt(character.base_stats[k]) : "—"}</span></div>
      {/each}

      <div class="section-label"><span>覚醒</span><span class="rule"></span><span class="note">自動反映</span></div>
      <div class="kv"><span class="k">覚醒段階</span><span class="v">{character ? character.awakening.stage : "—"}</span></div>
      <div class="kv"><span class="k">エタの意志 Lv</span><span class="v">{character ? character.awakening.eternal_level : "—"}</span></div>
    </div>
  </section>

  <Splitter
    bind:value={layoutWidths.value.input}
    min={200}
    defaultValue={DEFAULT_INPUT_WIDTH}
    controls="prev"
    label="INPUT と TARGET の境界"
  />

  <!-- TARGET -->
  <section>
    <div class="panel-head"><span class="dot"></span><span class="title">TARGET — 条件</span></div>
    <div class="scroll">
      <div class="block">
        <Select label="対象" bind:value={enemyId} options={enemyOptions} />
      </div>
      <div class="kv"><span class="k">防御力</span><span class="v">{enemy ? fmtInt(enemy.defense) : "—"}</span></div>
      <div class="kv"><span class="k">被害減少</span><span class="v">{enemy ? fmtInt(enemy.damage_reduction) : "—"}</span></div>
      <div class="kv"><span class="k">カット率A</span><span class="v">{enemy ? `×${fmtNum(enemy.cut_rate_a)}` : "—"}</span></div>
      <div class="kv"><span class="k">属性閾値</span><span class="v">{enemy ? fmtInt(enemy.element_threshold) : "—"}</span></div>

      <div class="section-label"><span>コンボ</span><span class="rule"></span></div>
      <label class="toggle">
        <input type="checkbox" bind:checked={combo} />
        <span class="check" aria-hidden="true">
          <svg width="9" height="9" viewBox="0 0 9 9" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M1.6 4.5l1.9 1.9L7.4 2.6"/></svg>
        </span>
        <span>{COMBO_THRESHOLD} コンボ以上</span>
        <span class="hint dim">コンボボーナス</span>
      </label>

      <details class="adjustments">
        <summary>
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M6 3.5L10.5 8 6 12.5"/></svg>
          <span>調整(一時) — 計算にのみ反映、キャラには保存されません</span>
        </summary>
        <div class="block">
          <AdjustmentEditor
            adjustments={temporaryAdjustments}
            addMin={limits.adjustment_add_min} addMax={limits.adjustment_add_max}
            pinMin={limits.adjustment_pin_min} pinMax={limits.adjustment_pin_max}
            pinDefault={(k) => result?.trace.stats.find((s) => s.kind === k)?.effective ?? character?.base_stats[k] ?? 0}
          />
        </div>
      </details>
    </div>
  </section>

  <Splitter
    bind:value={layoutWidths.value.target}
    min={180}
    defaultValue={DEFAULT_TARGET_WIDTH}
    controls="prev"
    label="TARGET と RESULT の境界"
  />

  <!-- RESULT -->
  <section class="result">
    <div class="panel-head">
      <span class="dot warm"></span><span class="title">RESULT — 結果</span>
      {#if hasTemporaryAdjustments}<span class="badge">調整あり</span>{/if}
      {#if calculating}<span class="dim status">計算中…</span>{/if}
    </div>
    <div class="scroll">
      {#if result}
        <div class="hero">
          <span class="cap">1 ヒットあたり(最大)</span>
          <span class="big num">{fmtInt(result.per_hit.max)}</span>
          <span class="dim">{character?.name} / {skill?.name} → {enemy?.name}</span>
        </div>
        <div class="triple">
          <div><span class="cap">最小</span><span class="num">{fmtInt(result.per_hit.min)}</span></div>
          <div><span class="cap">最大</span><span class="num">{fmtInt(result.per_hit.max)}</span></div>
          <div><span class="cap">クリティカル</span><span class="num warm">{fmtInt(result.per_hit.critical)}</span></div>
        </div>
        <div class="section-label"><span>合計 × {result.hit_count} 段</span><span class="rule"></span></div>
        <div class="triple total">
          <div><span class="cap">最小</span><span class="num">{fmtInt(result.total.min)}</span></div>
          <div><span class="cap">最大</span><span class="num accent">{fmtInt(result.total.max)}</span></div>
          <div><span class="cap">クリティカル</span><span class="num warm">{fmtInt(result.total.critical)}</span></div>
        </div>
        <TracePanel trace={result.trace} {character} />
      {:else}
        <p class="empty dim">キャラ・スキル・対象を選ぶと自動で計算します。</p>
      {/if}
    </div>
  </section>
</div>

<style>
  .layout {
    height: 100%; display: grid;
    background: var(--border); overflow-x: auto;
  }
  section { background: var(--bg); display: flex; flex-direction: column; min-height: 0; min-width: 0; }
  section.result { background: var(--bg-raised); }
  .scroll { overflow: auto; min-height: 0; }
  .block { padding: 12px 14px; display: flex; flex-direction: column; gap: 10px; border-bottom: 1px solid var(--border-soft); }
  .meta { display: flex; gap: 12px; font-size: 11px; }
  .status { margin-left: auto; font-size: 10px; }

  .toggle { display: flex; align-items: center; flex-wrap: wrap; gap: 9px; padding: 5px 14px; cursor: pointer; font-size: 12px; min-width: 0; }
  .toggle input { position: absolute; opacity: 0; width: 0; height: 0; }
  .check {
    width: 13px; height: 13px; flex-shrink: 0; border: 1px solid var(--border-strong);
    display: flex; align-items: center; justify-content: center; color: transparent;
  }
  .toggle input:checked + .check { background: var(--accent); border-color: var(--accent); color: var(--bg); }
  .toggle input:focus-visible + .check { outline: 1px solid var(--accent); outline-offset: 2px; }
  .hint { margin-left: auto; font-size: 11px; }

  details.adjustments { border-top: 1px solid var(--border); margin-top: 4px; }
  details.adjustments summary {
    display: flex; align-items: center; gap: 8px; padding: 11px 14px;
    font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); cursor: pointer; list-style: none;
    user-select: none;
  }
  details.adjustments summary::-webkit-details-marker { display: none; }
  details.adjustments summary svg { transition: transform 0.15s; }
  details.adjustments[open] summary svg { transform: rotate(90deg); }
  details.adjustments summary:hover { color: var(--fg); }

  .badge {
    font-size: 10px; letter-spacing: 0.08em; color: var(--warm); border: 1px solid var(--warm);
    padding: 1px 6px;
  }

  .hero { padding: 20px 16px 16px; display: flex; flex-direction: column; gap: 6px; border-bottom: 1px solid var(--border-soft); }
  .cap { font-size: 10px; letter-spacing: 0.14em; color: var(--fg-muted); }
  .big { font-size: 46px; font-weight: 700; line-height: 1; color: var(--accent); letter-spacing: -0.01em; }
  .triple { display: flex; border-bottom: 1px solid var(--border-soft); }
  .triple > div {
    flex: 1; padding: 11px 16px; display: flex; flex-direction: column; gap: 3px;
    border-right: 1px solid var(--border-soft);
  }
  .triple > div:last-child { border-right: 0; }
  .triple .cap { letter-spacing: 0; }
  .triple .num { font-size: 15px; font-weight: 500; }
  .triple.total .num { font-size: 17px; }
  .warm { color: var(--warm); }
  .accent { color: var(--accent); }
  .empty { padding: 20px 16px; font-size: 12px; }
</style>
