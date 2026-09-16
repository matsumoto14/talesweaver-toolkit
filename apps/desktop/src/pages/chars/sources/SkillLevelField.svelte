<script lang="ts">
  // 共通スキルペインで繰り返される「ラベル / 段階選択 / 外す(・見え方スイッチ) / 効いている値」の
  // 1 行(`.skill-field`)。段階制のスキルはこの形にほぼそろう(チップのオン/オフ切り替えは対象外)。
  import type { Snippet } from "svelte";
  import Num from "../../../ui/Num.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";

  interface Option { value: string; label: string }
  interface Props {
    label: string;
    options: Option[];
    cols?: number;
    cell?: number;
    disabledValues?: string[];
    value: string;
    onChange: (v: string) => void;
    /** ラベルの左に置くスキルの絵(§08: 名前と併記) */
    icon?: Snippet;
    /** 見え方を変える・段を絞るなどの追加アクション(「5 以上 / 1〜4」など)。clear より前に出す */
    extraAction?: Snippet;
    clearLabel?: string;
    clearDisabled?: boolean;
    onClear?: () => void;
    /** 効いている値。未指定なら空欄(オーグメントのように値欄を持たない行) */
    valueText?: string;
    valueMotion?: "bump" | "flash";
    valueKey?: unknown;
  }
  let {
    label, options, cols, cell, disabledValues = [], value, onChange,
    icon, extraAction, clearLabel, clearDisabled = false, onClear,
    valueText, valueMotion, valueKey,
  }: Props = $props();
</script>

<div class="skill-field">
  <span class="k">{#if icon}{@render icon()}{/if}{label}</span>
  <StepSelect
    label=""
    {options}
    cols={cols ?? options.length}
    {cell}
    {disabledValues}
    bind:value={() => value, onChange}
  />
  <span class="skill-actions">
    {#if extraAction}{@render extraAction()}{/if}
    {#if clearLabel}
      <button type="button" class="clear" disabled={clearDisabled} onclick={onClear}>{clearLabel}</button>
    {/if}
  </span>
  {#if valueText === undefined}
    <span></span>
  {:else if valueMotion === "bump"}
    <Num class="v" motion={() => (valueKey as number | null)} value={valueText} />
  {:else if valueMotion === "flash"}
    <Num class="v" value={String(valueKey)}>{#snippet children()}{valueText}{/snippet}</Num>
  {:else}
    <span class="v num">{valueText}</span>
  {/if}
</div>
