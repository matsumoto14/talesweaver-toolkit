<script lang="ts">
  import { createCharacter, errorMessage, STAT_KINDS, STAT_LABELS } from "../api";
  import type { BaseStats, GameCharacter, RegisteredCharacter } from "../api";
  import { reportError } from "../toast.svelte";
  import Select from "./Select.svelte";
  import Stepper from "./Stepper.svelte";

  interface Props {
    gameCharacters: GameCharacter[];
    onCreated: (c: RegisteredCharacter) => void;
  }
  let { gameCharacters, onCreated }: Props = $props();

  const STAT_MIN = 1;
  const STAT_MAX = 1500;
  const defaultStats = (): BaseStats => ({ stab: 1, hack: 1, int: 1, def: 1, mr: 1, dex: 1, agi: 1 });

  let name = $state("");
  let gameCharacterId = $state("");
  let stats = $state<BaseStats>(defaultStats());
  let stage = $state("0");
  let eternalLevel = $state("0");
  let saving = $state(false);

  const stageOptions = Array.from({ length: 6 }, (_, i) => ({ value: String(i), label: `${i} 段階` }));
  const eternalOptions = Array.from({ length: 81 }, (_, i) => ({ value: String(i), label: `Lv ${i}` }));
  const characterOptions = $derived(gameCharacters.map((c) => ({ value: c.id, label: c.name })));
  const canSubmit = $derived(name.trim().length > 0 && gameCharacterId !== "" && !saving);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    saving = true;
    try {
      const created = await createCharacter({
        name: name.trim(),
        game_character_id: gameCharacterId,
        base_stats: { ...stats },
        awakening: { stage: Number(stage), eternal_level: Number(eternalLevel) },
      });
      onCreated(created);
      name = "";
      stats = defaultStats();
    } catch (err) {
      reportError(errorMessage(err));
    } finally {
      saving = false;
    }
  }
</script>

<form onsubmit={submit}>
  <div class="block">
    <label class="text">
      <span class="label">名前</span>
      <input type="text" bind:value={name} maxlength="32" placeholder="表示名" />
    </label>
    <Select label="キャラ" bind:value={gameCharacterId} options={characterOptions} />
  </div>

  <div class="section-label"><span>素ステータス</span><span class="rule"></span><span class="dim">{STAT_MIN}–{STAT_MAX}</span></div>
  <div class="block stats">
    {#each STAT_KINDS as k (k)}
      <Stepper label={STAT_LABELS[k]} bind:value={stats[k]} min={STAT_MIN} max={STAT_MAX} />
    {/each}
  </div>

  <div class="section-label"><span>覚醒</span><span class="rule"></span></div>
  <div class="block two">
    <Select label="覚醒段階" bind:value={stage} options={stageOptions} />
    <Select label="エタの意志 Lv" bind:value={eternalLevel} options={eternalOptions} />
  </div>

  <div class="actions">
    <button type="submit" class="btn primary" disabled={!canSubmit}>
      {saving ? "保存中…" : "登録"}
    </button>
  </div>
</form>

<style>
  form { display: flex; flex-direction: column; }
  .block { display: flex; flex-direction: column; gap: 10px; padding: 12px 14px; }
  .block.stats { gap: 8px; }
  .block.two { flex-direction: row; }
  .block.two > :global(*) { flex: 1; }
  .text { display: flex; flex-direction: column; gap: 6px; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  input[type="text"] {
    padding: 8px 10px; background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); }
  .actions { padding: 12px 14px 16px; display: flex; justify-content: flex-end; }
</style>
