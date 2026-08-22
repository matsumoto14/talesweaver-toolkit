<script lang="ts">
  // キャラ詳細の中央・右カラムの外枠。draft(編集状態)を1つの $state にまとめて持ち、
  // CharacterData(中央)・CharacterSettings(右)の双方から書き換えられるようにする
  // (draft は $state のプロキシなので、子へは通常の prop で渡し、子はネストしたプロパティを
  // 直接書き換える。トップレベルの draft 自体を差し替えないので bind: は使わない)。
  // 親 CharacterPage が {#key character.id} で作り直す前提。docs/claude/decisions.md の
  // 「{#key} で作り直す」パターンを踏襲し、$effect による再同期は書かない。
  import { onMount, untrack } from "svelte";
  import { errorMessage, listBuffCatalog, previewEffectiveStats, updateCharacter } from "../../api/commands";
  import type { BuffDefinition, GameCharacter, NewCharacter, RegisteredCharacter, StatPreview, StatSources } from "../../api/types";
  import { reportError } from "../../toast.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import Splitter from "../../ui/Splitter.svelte";
  import CharacterData from "./CharacterData.svelte";
  import CharacterSettings from "./CharacterSettings.svelte";
  import { buildDraft, type Draft } from "./draft";

  interface Props {
    character: RegisteredCharacter;
    gameCharacters: GameCharacter[];
    onSaved: (c: RegisteredCharacter) => void;
  }
  let { character, gameCharacters, onSaved }: Props = $props();

  const DEFAULT_SETTINGS_WIDTH = 340;
  const layoutWidths = persisted("tw-layout-character-workspace", { settings: DEFAULT_SETTINGS_WIDTH });
  const gridTemplateColumns = $derived(`minmax(240px, 1fr) 6px minmax(220px, ${layoutWidths.value.settings ?? DEFAULT_SETTINGS_WIDTH}px)`);

  // 親が {#key} でこのコンポーネントを作り直す前提なので初期値だけ untrack で取る。
  const initial = untrack(() => character);
  // 保存成功のたびに更新する基準スナップショット。$state にしないと保存後も
  // 「未保存」表示・保存ボタンの有効状態が変わらないままになる。
  let initialSnapshot = $state(JSON.stringify(buildDraft(initial)));

  let draft = $state<Draft>(buildDraft(initial));
  const dirty = $derived(JSON.stringify(draft) !== initialSnapshot);

  let saving = $state(false);
  let catalog = $state<BuffDefinition[]>([]);

  onMount(() => {
    listBuffCatalog()
      .then((c) => (catalog = c))
      .catch((e) => reportError(errorMessage(e)));
  });

  // キャラ種(gameCharacterId)を切り替えたら、旧キャラ専用のキャラスキルバフ(BuffGroup::CharacterSkill)の
  // 選択を落とす。UI(CharacterSettings)は選択中キャラのスキルだけを表示するが、
  // draft.statSources.buffs.choices 自体は明示的に消さないと「幽霊バフ」として計算に残り続ける
  // (味方スキル AllySkill は誰のキャラでも有効なので残してよい)。catalog 読み込み前は判定できないため待つ。
  let lastGameCharacterId = draft.gameCharacterId;
  $effect(() => {
    const currentId = draft.gameCharacterId;
    if (currentId === lastGameCharacterId || catalog.length === 0) return;
    lastGameCharacterId = currentId;
    draft.statSources.buffs.choices = draft.statSources.buffs.choices.filter((choice) => {
      const def = catalog.find((d) => d.id === choice.buff_id);
      if (!def || typeof def.group !== "object" || !("character_skill" in def.group)) return true;
      return def.group.character_skill.game_character_id === currentId;
    });
  });

  // --- 即時プレビュー ---------------------------------------------------
  // draft の baseStats/statSources が変わるたびに(100ms debounce)preview_effective_stats を呼ぶ。
  // エラー時は previewError に入れて能力値表の近くに表示する(トーストは出さない。直前の
  // プレビューは保持する。全部 null にしない)。
  let preview = $state<StatPreview | null>(null);
  let previewError = $state<string | null>(null);
  let debounceHandle: ReturnType<typeof setTimeout> | undefined;
  let previewSeq = 0;

  $effect(() => {
    // 依存を明示的に読む(JSON 化で baseStats/statSources 全体の変更を拾う)。
    const baseStats = { ...draft.baseStats };
    const statSources = JSON.parse(JSON.stringify(draft.statSources)) as StatSources;

    if (debounceHandle) clearTimeout(debounceHandle);
    const seq = ++previewSeq;
    debounceHandle = setTimeout(() => {
      previewEffectiveStats(baseStats, statSources, draft.gameCharacterId)
        .then((p) => { if (seq === previewSeq) { preview = p; previewError = null; } })
        .catch((e) => { if (seq === previewSeq) { previewError = errorMessage(e); } });
    }, 100);

    return () => {
      if (debounceHandle) clearTimeout(debounceHandle);
    };
  });

  // 保存はキャラ単位で1ボタン、未保存変更があるときだけ有効(docs/claude/decisions.md)。
  const canSubmit = $derived(draft.name.trim().length > 0 && draft.gameCharacterId !== "" && !saving && dirty);

  async function save() {
    if (!canSubmit) return;
    saving = true;
    try {
      const payload: NewCharacter = {
        name: draft.name.trim(),
        game_character_id: draft.gameCharacterId,
        base_stats: { ...draft.baseStats },
        awakening: { stage: Number(draft.stage), eternal_level: Number(draft.eternalLevel) },
        stat_sources: $state.snapshot(draft.statSources),
        equipment: $state.snapshot(draft.equipment),
      };
      const saved = await updateCharacter(character.id, payload);
      // 保存成功: 基準スナップショットを現在の draft に合わせ、「未保存」表示・保存ボタンを消す。
      initialSnapshot = JSON.stringify(draft);
      onSaved(saved);
    } catch (err) {
      reportError(errorMessage(err));
    } finally {
      saving = false;
    }
  }
</script>

<div class="workspace" style="grid-template-columns: {gridTemplateColumns};">
  <CharacterData {draft} {preview} {previewError} {gameCharacters} {catalog} {save} {saving} {dirty} {canSubmit} />
  <Splitter
    bind:value={layoutWidths.value.settings}
    min={220}
    defaultValue={DEFAULT_SETTINGS_WIDTH}
    controls="next"
    label="キャラデータと設定の境界"
  />
  <CharacterSettings {draft} {preview} {catalog} />
</div>

<style>
  .workspace {
    height: 100%; display: grid;
    background: var(--border); overflow-x: auto;
  }
  .workspace > :global(section) { min-width: 0; }
</style>
