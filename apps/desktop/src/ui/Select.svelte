<script lang="ts">
  interface Option { value: string; label: string }
  interface Props {
    label?: string;
    value: string;
    options: Option[];
    placeholder?: string;
    disabled?: boolean;
  }
  let { label, value = $bindable(), options, placeholder = "選択してください", disabled = false }: Props = $props();
</script>

<label class="select">
  {#if label}<span class="label">{label}</span>{/if}
  <span class="box">
    <select bind:value {disabled}>
      <option value="" disabled>{placeholder}</option>
      {#each options as o (o.value)}
        <option value={o.value}>{o.label}</option>
      {/each}
    </select>
    <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6.5L8 10.5l4-4"/></svg>
  </span>
</label>

<style>
  .select { display: flex; flex-direction: column; gap: 6px; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  .box { position: relative; display: flex; color: var(--fg-muted); }
  select {
    appearance: none; width: 100%;
    padding: 8px 30px 8px 10px;
    background: var(--bg-field); border: 1px solid var(--border);
    color: var(--fg); font-size: 13px;
  }
  select:focus { outline: none; border-color: var(--accent); }
  select:disabled { color: var(--fg-dim); }
  svg { position: absolute; right: 10px; top: 50%; transform: translateY(-50%); pointer-events: none; }
</style>
