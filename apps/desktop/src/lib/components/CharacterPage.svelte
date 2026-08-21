<script lang="ts">
  import { deleteCharacter, errorMessage, listCharacters, listGameCharacters, STAT_KINDS, STAT_LABELS } from "../api";
  import type { GameCharacter, RegisteredCharacter } from "../api";
  import { fmtInt } from "../format";
  import { reportError } from "../toast.svelte";
  import CharacterForm from "./CharacterForm.svelte";

  let characters = $state<RegisteredCharacter[]>([]);
  let gameCharacters = $state<GameCharacter[]>([]);
  let loading = $state(true);

  const gameName = (id: string) => gameCharacters.find((g) => g.id === id)?.name ?? id;

  $effect(() => {
    Promise.all([listCharacters(), listGameCharacters()])
      .then(([cs, gs]) => { characters = cs; gameCharacters = gs; })
      .catch((e) => reportError(errorMessage(e)))
      .finally(() => (loading = false));
  });

  async function remove(c: RegisteredCharacter) {
    try {
      await deleteCharacter(c.id);
      characters = characters.filter((x) => x.id !== c.id);
    } catch (e) {
      reportError(errorMessage(e));
    }
  }
</script>

<div class="layout">
  <section class="list">
    <div class="panel-head"><span class="dot"></span><span class="title">CHARACTERS — 登録一覧</span><span class="count dim">{characters.length} 件</span></div>
    {#if loading}
      <p class="empty dim">読み込み中…</p>
    {:else if characters.length === 0}
      <p class="empty dim">登録キャラはまだありません。右のフォームから登録してください。</p>
    {:else}
      <div class="scroll">
        <table class="grid">
          <thead>
            <tr>
              <th>名前</th>
              <th>キャラ</th>
              <th class="n">覚醒</th>
              <th class="n">エタLv</th>
              {#each STAT_KINDS as k (k)}<th class="n">{STAT_LABELS[k]}</th>{/each}
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each characters as c (c.id)}
              <tr>
                <td class="name">{c.name}</td>
                <td class="muted">{gameName(c.game_character_id)}</td>
                <td class="n">{c.awakening.stage}</td>
                <td class="n">{c.awakening.eternal_level}</td>
                {#each STAT_KINDS as k (k)}<td class="n">{fmtInt(c.base_stats[k])}</td>{/each}
                <td class="n"><button class="btn danger small" onclick={() => remove(c)}>削除</button></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </section>

  <section class="form">
    <div class="panel-head"><span class="dot warm"></span><span class="title">REGISTER — 新規登録</span></div>
    <div class="scroll">
      <CharacterForm {gameCharacters} onCreated={(c) => (characters = [...characters, c])} />
    </div>
  </section>
</div>

<style>
  .layout {
    height: 100%; display: grid; grid-template-columns: minmax(0, 1fr) 400px;
    gap: 1px; background: var(--border);
  }
  section { background: var(--bg); display: flex; flex-direction: column; min-height: 0; }
  section.form { background: var(--bg-raised); }
  .count { margin-left: auto; font-size: 10px; }
  .scroll { overflow: auto; min-height: 0; }
  .empty { padding: 20px 14px; font-size: 12px; }
  td.name { font-weight: 500; }
  .btn.small { padding: 3px 8px; font-size: 11px; }
</style>
