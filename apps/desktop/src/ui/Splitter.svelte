<script lang="ts">
  // グリッドの列境界に置く、縦のドラッグ可能な区切り線。
  // `controls: "prev"` はこのスプリッターの左の列を制御する(右にドラッグすると増える)。
  // `controls: "next"` は右の列を制御する(右にドラッグすると減る。列が右側にあるレイアウト用)。
  interface Props {
    value: number;
    min: number;
    max?: number;
    defaultValue: number;
    controls: "prev" | "next";
    label: string;
  }
  let { value = $bindable(), min, max, defaultValue, controls, label }: Props = $props();

  const clamp = (n: number) => {
    // 外部(localStorage 経由)から不正値(NaN・undefined 由来・キー欠落)が来ても
    // 壊れた grid-template-columns を作らないよう、非有限値は defaultValue にフォールバックする。
    const base = Number.isFinite(n) ? n : defaultValue;
    return Math.max(min, Math.min(max ?? Infinity, base));
  };

  // マウント時、および外部(persisted の初期値等)から value が不正値に変わったときに
  // min/max 範囲へ自動的に補正する。ドラッグ中の内部更新は既に clamp 済みの値を
  // 書き戻すだけなので実質無害。
  $effect(() => {
    const clamped = clamp(value);
    if (clamped !== value) value = clamped;
  });

  let dragging = $state(false);
  let startX = 0;
  let startValue = 0;

  function endDrag(e: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    const el = e.currentTarget as HTMLElement;
    if (el.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
    document.body.classList.remove("tw-no-select");
  }

  function handlePointerDown(e: PointerEvent) {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragging = true;
    startX = e.clientX;
    startValue = value;
    document.body.classList.add("tw-no-select");
    e.preventDefault();
  }

  function handlePointerMove(e: PointerEvent) {
    if (!dragging) return;
    const delta = (e.clientX - startX) * (controls === "prev" ? 1 : -1);
    value = clamp(startValue + delta);
  }

  // pointerup に加え、タッチのパームリジェクションやシステムジェスチャ割込みなどで
  // pointerup が来ずに pointercancel だけ発生するケースがあるため、同じ後始末を行う
  // (放置すると dragging が固着し、tw-no-select が残ってテキスト選択不能になる)。
  function handlePointerUp(e: PointerEvent) {
    endDrag(e);
  }

  function handlePointerCancel(e: PointerEvent) {
    endDrag(e);
  }

  function handleDblClick() {
    value = clamp(defaultValue);
  }

  function handleKeydown(e: KeyboardEvent) {
    const sign = controls === "prev" ? 1 : -1;
    const step = e.shiftKey ? 32 : 8;
    if (e.key === "ArrowLeft") {
      value = clamp(value - sign * step);
      e.preventDefault();
    } else if (e.key === "ArrowRight") {
      value = clamp(value + sign * step);
      e.preventDefault();
    } else if (e.key === "Enter") {
      value = clamp(defaultValue);
      e.preventDefault();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  tabindex="0"
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max ?? 9999}
  aria-label={label}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointercancel={handlePointerCancel}
  ondblclick={handleDblClick}
  onkeydown={handleKeydown}
>
  <span class="rule"></span>
</div>

<style>
  .splitter {
    width: 6px; height: 100%; flex-shrink: 0; cursor: col-resize;
    display: flex; align-items: center; justify-content: center;
    background: var(--bg); touch-action: none;
  }
  .splitter .rule { width: 1px; height: 100%; background: var(--border); }
  .splitter:hover, .splitter.dragging { background: var(--accent); }
  .splitter:hover .rule, .splitter.dragging .rule { background: var(--accent); }
  .splitter:focus-visible { outline: 1px solid var(--accent); outline-offset: -1px; }
</style>
