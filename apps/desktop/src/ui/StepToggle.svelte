<script lang="ts">
  // 複数選択のチップ(design-system §07 形態 3)。StepSelect と並びは同じで、違うのは
  // 「1 つだけ残る」か「押したぶんだけ残る」かだけなので、見た目は同じ `.seg` を使う。
  // チェックボックス(CheckChip)は 1 件ずつの ON/OFF 用。**同じ軸の値をいくつか選ぶ**
  // ときはこちら — 選べる全体が一度に見えて、いくつ選んだかが数えられる。
  //
  // 押した瞬間に結果が動く(「適用」を挟まない)。押した段は同じ位置に残る(§09 規則 1)。
  // 上限は値の隣に常設する(§07)。上限に達したら未選択の段を押せなくする — 段は消さない。
  interface Option {
    value: string;
    label: string;
  }
  interface Props {
    label?: string;
    /** 選択中の値。並びは呼び出し側が決める(押した順で入れ替わらないように) */
    values: string[];
    options: Option[];
    onToggle: (value: string, next: boolean) => void;
    disabled?: boolean;
    /** 列を固定して並べる(§08 `.seg.cols`)。折り返しても行をまたいで幅が揃う */
    cols?: number;
    /** 同時に選べる数の上限 */
    max?: number;
    /**
     * 押せない段。**段そのものは消さない** — 消すと段の数が変わって幅が動く
     * (§09 規則 4)。理由は `titleFor` で読ませる
     */
    disabledValues?: string[];
    /** 段ごとの説明(hover)。押せない段の理由を出すのに使う */
    titleFor?: (value: string) => string | undefined;
  }
  let {
    label, values, options, onToggle, disabled = false, cols, max,
    disabledValues = [], titleFor,
  }: Props = $props();
  const full = $derived(max !== undefined && values.length >= max);
</script>

<div class="step-toggle">
  {#if label}
    <span class="head">
      <span class="label">{label}</span>
      {#if max !== undefined}<span class="count num">{values.length}/{max}</span>{/if}
    </span>
  {/if}
  <div
    class="seg"
    class:cols={cols !== undefined}
    style={cols === undefined ? undefined : `--seg-cols: ${cols}`}
    role="group"
    aria-label={label ?? ""}
  >
    {#each options as o (o.value)}
      {@const on = values.includes(o.value)}
      <button
        type="button"
        class="step"
        class:on
        aria-pressed={on}
        disabled={disabled || disabledValues.includes(o.value) || (full && !on)}
        title={titleFor?.(o.value)}
        onclick={() => onToggle(o.value, !on)}
      >{o.label}</button>
    {/each}
  </div>
</div>

<style>
  .step-toggle { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .head { display: flex; align-items: baseline; gap: 8px; }
  .label { font-size: 10px; letter-spacing: 0.1em; color: var(--fg-dim); }
  .count { font-size: 10px; color: var(--fg-dim); }
</style>
