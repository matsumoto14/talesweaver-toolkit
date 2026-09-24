<script lang="ts">
  // キャラタブの外枠: 登録ペイン or 選択キャラのワークスペース(補正源ドリルダウン)。
  import { app, selectedCharacter } from "../../state.svelte";
  import RegisterPane from "./RegisterPane.svelte";
  import Workspace from "./Workspace.svelte";
  import { t } from "../../i18n";

  const character = $derived(selectedCharacter());
  const showRegister = $derived(app.registerOpen || !character);
</script>

<div class="page">
  <div class="head-bar">
    <span class="title">{t("キャラの登録と補正源")}</span>
    <span class="note">{showRegister ? t("名前とキャラだけで登録できます") : t("編集すると自動で保存されます")}</span>
  </div>
  {#if showRegister}
    <div class="register-wrap">
      <RegisterPane />
    </div>
  {:else if character}
    {#key character.id}
      <Workspace {character} />
    {/key}
  {/if}
</div>

<style>
  .page { flex: 1; min-height: 0; display: flex; flex-direction: column; background: var(--bg-mid); }
  .register-wrap { flex: 1; min-height: 0; overflow: auto; scrollbar-gutter: stable; padding: 13px 16px 18px; }
</style>
