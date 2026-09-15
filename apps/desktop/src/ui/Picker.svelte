<script lang="ts" module>
  // 順序の無いものから 1 つ選ぶ(design-system §07「1 つ選ぶ」)。順序があるものは StepSelect。
  //
  // 素の `<select>` は 5 形態のどれでもないので使わない。ここは 5 形態の順どおりに、
  //
  //   1. **よく使う候補はチップ(形態 3)で手前に固定**する。開かずに 1 押しで決まる。
  //      「よく使う」はドメイン知識で固定し(`pinned`)、使用履歴では並べない —
  //      選んだ瞬間にチップが入れ替わると §00 ③「押した場所は動かない」に反する
  //   2. 残りは候補面(1 件 1 行)に送る。**押した場所が動かない**(候補は重なって出る、§09 規則 3)
  //   3. 候補が CHIPS_ONLY_MAX 件以下なら全部チップで出し、候補面を出さない
  //
  // 候補行は名前だけでなく**選ぶのに要る値**(`meta`)を必ず持つ。値の無い候補は並べる根拠が無く、
  // 素の select と同じ画面になる。「未選択」も候補の 1 行(value = "")にし、placeholder に頼らない。
  export interface PickerOption {
    value: string;
    /** 行の主となる名前 */
    name: string;
    /** 選ぶのに要る値(「単 ・ 11 段 ・ 水」「+18%」など)。空を許さない */
    meta: string;
    /** アイコンの id(gamedata の id)。無ければアイコンを出さない */
    iconId?: string | null;
    iconKind?: IconKind;
    /** 手前にチップで固定する(よく使う候補)。件数が CHIPS_ONLY_MAX 以下なら全部チップになる */
    pinned?: boolean;
    /** 段に足すクラス(`record-only` など) */
    tone?: string;
  }
  /** この件数以下なら全部チップで出し、候補面を出さない(§07「1 つ選ぶ」順序なし・少数) */
  export const CHIPS_ONLY_MAX = 5;
</script>

<script lang="ts">
  import Icon, { type IconKind } from "./Icon.svelte";
  import { positionPopover } from "./popover";

  interface Props {
    label?: string;
    value: string;
    options: PickerOption[];
    /** 候補の面の上に出す一言(「火力の高い順」など) */
    note?: string;
    disabled?: boolean;
    /**
     * チップを出さず候補面だけにする。文の中に埋める(「A が B に当てる」)ときと、
     * 選ぶ = 足す操作(値が残らない)のときだけ
     */
    menu?: boolean;
  }
  let {
    label, value = $bindable(), options, note, disabled = false, menu = false,
  }: Props = $props();

  let open = $state(false);
  const chips = $derived(
    menu ? [] : options.length <= CHIPS_ONLY_MAX ? options : options.filter((o) => o.pinned),
  );
  const rest = $derived(
    menu ? options : options.length <= CHIPS_ONLY_MAX ? [] : options.filter((o) => !o.pinned),
  );
  const picked = $derived(options.find((o) => o.value === value) ?? null);
  /** 候補面の中の 1 件を選んでいる。チップ側を選んでいるときは面の口は「ほか n 件」になる */
  const pickedInRest = $derived(picked !== null && rest.includes(picked));
  /** 「未選択」(value = "")を候補面から選んでいる状態は、選択色にしない — 何かを選んだように見えてしまう */
  const pickedNone = $derived(pickedInRest && picked?.value === "");
</script>

<div class="picker">
  {#if label}<span class="label">{label}</span>{/if}
  <div class="picker-line">
    {#each chips as o (o.value)}
      <button
        type="button"
        class="chip picker-chip {o.tone ?? ''}"
        class:on={o.value === value}
        class:with-icon={o.iconId !== undefined}
        aria-pressed={o.value === value}
        {disabled}
        onclick={() => (value = o.value)}
      >
        {#if o.iconId !== undefined}
          <Icon kind={o.iconKind ?? "skill"} id={o.iconId} size={20} label={o.name} />
        {/if}
        <span class="picker-chip-name">{o.name}</span>
        <span class="picker-chip-meta num">{o.meta}</span>
      </button>
    {/each}
    {#if rest.length > 0}
      <button
        type="button"
        class="picker-trigger"
        class:open
        class:compact={chips.length > 0}
        class:muted={!pickedInRest || pickedNone}
        {disabled}
        onclick={() => (open = !open)}
      >
        {#if pickedInRest && picked}
          {#if picked.iconId !== undefined}
            <Icon kind={picked.iconKind ?? "skill"} id={picked.iconId} size={20} label={picked.name} />
          {/if}
          <span class="picker-name">{picked.name}</span>
          <span class="picker-meta num">{picked.meta}</span>
        {:else}
          <span class="picker-name">{chips.length > 0 ? `ほか ${rest.length} 件` : `${rest.length} 件から選ぶ`}</span>
        {/if}
        <span class="picker-chev" class:rot={open}>▼</span>
      </button>
    {/if}
  </div>
  {#if open}
    <!-- 候補は重なって出る。押した場所も下の行も動かない(§09 規則 3) -->
    <button type="button" class="picker-overlay" aria-label="閉じる" onclick={() => (open = false)}></button>
    <div class="picker-pop pop-in" use:positionPopover>
      {#if note}<div class="picker-pop-head">{note}</div>{/if}
      {#each rest as o (o.value)}
        <button
          type="button"
          class="picker-row {o.tone ?? ''}"
          class:on={o.value === value}
          onclick={() => { value = o.value; open = false; }}
        >
          {#if o.iconId !== undefined}
            <Icon kind={o.iconKind ?? "skill"} id={o.iconId} size={20} label={o.name} />
          {/if}
          <span class="picker-name">{o.name}</span>
          <span class="picker-meta num">{o.meta}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker { position: relative; display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
</style>
