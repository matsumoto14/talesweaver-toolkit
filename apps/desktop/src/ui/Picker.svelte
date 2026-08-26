<script lang="ts" module>
  // 多数から 1 つ選ぶ場所(design-system §07 の 5 形態に入らないもの)。
  //
  // 素の `<select>` は 5 形態のどれでもない。**並べられる数(≤12)は段階選択**に降ろすが、
  // スキル 20 種のように並べきれないものはここ。ドロップダウンとの違いは 3 つで、
  //
  //   1. **押した場所が動かない**。候補は重なって出る(§09 規則 3)
  //   2. **1 件が 1 行**で、名前だけでなく**選ぶのに要る値**(単 / 範・段数・属性)を持てる
  //   3. アイコンを併記できる(§06。名前と一緒に出す)
  //
  // ネイティブのドロップダウンでは 2 と 3 ができない — だからこの部品を作った。
  export interface PickerOption {
    value: string;
    /** 行の主となる名前 */
    name: string;
    /** 名前の後ろに小さく出す値(「単 ・ 11 段 ・ 水」など) */
    meta?: string;
    /** アイコンの id(gamedata の id)。無ければアイコンを出さない */
    iconId?: string | null;
    iconKind?: IconKind;
  }
</script>

<script lang="ts">
  import Icon, { type IconKind } from "./Icon.svelte";

  interface Props {
    label?: string;
    value: string;
    options: PickerOption[];
    /** 候補の面の上に出す一言(「火力の高い順」など) */
    note?: string;
    placeholder?: string;
    disabled?: boolean;
  }
  let {
    label, value = $bindable(), options, note, placeholder = "選択してください", disabled = false,
  }: Props = $props();

  let open = $state(false);
  const picked = $derived(options.find((o) => o.value === value) ?? null);
</script>

<div class="picker">
  {#if label}<span class="label">{label}</span>{/if}
  <button
    type="button"
    class="picker-trigger"
    class:open
    {disabled}
    onclick={() => (open = !open)}
  >
    {#if picked?.iconId !== undefined}
      <Icon kind={picked?.iconKind ?? "skill"} id={picked?.iconId ?? null} size={20} label={picked?.name ?? ""} />
    {/if}
    <span class="picker-name">{picked?.name ?? placeholder}</span>
    {#if picked?.meta}<span class="picker-meta num">{picked.meta}</span>{/if}
    <span class="picker-chev" class:rot={open}>▼</span>
  </button>
  {#if open}
    <!-- 候補は重なって出る。押した場所も下の行も動かない(§09 規則 3) -->
    <button type="button" class="picker-overlay" aria-label="閉じる" onclick={() => (open = false)}></button>
    <div class="picker-pop pop-in">
      {#if note}<div class="picker-pop-head">{note}</div>{/if}
      {#each options as o (o.value)}
        <button
          type="button"
          class="picker-row"
          class:on={o.value === value}
          onclick={() => { value = o.value; open = false; }}
        >
          {#if o.iconId !== undefined}
            <Icon kind={o.iconKind ?? "skill"} id={o.iconId} size={20} label={o.name} />
          {/if}
          <span class="picker-name">{o.name}</span>
          {#if o.meta}<span class="picker-meta num">{o.meta}</span>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker { position: relative; display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
</style>
