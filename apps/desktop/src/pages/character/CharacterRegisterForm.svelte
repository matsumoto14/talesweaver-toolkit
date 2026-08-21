<script lang="ts">
  // 登録の入口は「名前 + キャラ種」のみ(docs/ux-guidelines.md 原則3)。
  // 素ステ・覚醒・恒常補正・常用バフ・調整値は中立値で組み立て、登録後の CharacterWorkspace で編集する。
  import { createCharacter, errorMessage } from "../../api/commands";
  import { STAT_KINDS } from "../../labels";
  import type { BaseStats, GameCharacter, NewCharacter, RegisteredCharacter } from "../../api/types";
  import { reportError } from "../../toast.svelte";
  import Select from "../../ui/Select.svelte";
  import { neutralStatSources } from "./draft";

  interface Props {
    gameCharacters: GameCharacter[];
    onCreated: (c: RegisteredCharacter) => void;
  }
  let { gameCharacters, onCreated }: Props = $props();

  const defaultStats = (): BaseStats =>
    Object.fromEntries(STAT_KINDS.map((k) => [k, 1])) as BaseStats;

  let name = $state("");
  let gameCharacterId = $state("");
  let saving = $state(false);

  const characterOptions = $derived(gameCharacters.map((c) => ({ value: c.id, label: c.name })));
  const canSubmit = $derived(name.trim().length > 0 && gameCharacterId !== "" && !saving);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!canSubmit) return;
    saving = true;
    try {
      const payload: NewCharacter = {
        name: name.trim(),
        game_character_id: gameCharacterId,
        base_stats: defaultStats(),
        awakening: { stage: 0, eternal_level: 0 },
        stat_sources: neutralStatSources(),
      };
      const saved = await createCharacter(payload);
      onCreated(saved);
      name = "";
      gameCharacterId = "";
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
  <div class="actions">
    <button type="submit" class="btn primary" disabled={!canSubmit}>{saving ? "登録中…" : "登録"}</button>
  </div>
</form>

<style>
  form { display: flex; flex-direction: column; }
  .block { display: flex; flex-direction: column; gap: 10px; padding: 12px 14px; }
  .text { display: flex; flex-direction: column; gap: 6px; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  input[type="text"] {
    padding: 8px 10px; background: var(--bg-field); border: 1px solid var(--border); color: var(--fg);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); }
  .actions { padding: 4px 14px 14px; display: flex; justify-content: flex-end; }
</style>
