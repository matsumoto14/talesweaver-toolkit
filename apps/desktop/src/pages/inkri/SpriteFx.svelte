<script lang="ts" module>
  // ゲームのアニメ(.d2a)をそのまま再生する。コマの並び・表示 tick・位置・フェードは
  // tools/gamedata/import_inkri_ui.py が書き出した JSON が持ち、ここは描くだけ。
  //
  // 動きの規格(design-system §10、0.5s 以内)の外にある。インクリ画面は「ゲームの画面そのまま」が
  // 要件(ユーザー決定 2026-09-17)で、この演出はゲームの演出の再現だから。動きを消す設定のときは再生しない。
  export interface FxFrame { x: number; y: number; w: number; h: number; ox: number; oy: number }
  export interface FxCue { track: number; frame: number; start: number; hold: number; dx: number; dy: number }
  export interface FxFade { track: number; start: number; ticks: number }
  export interface FxData { frames: FxFrame[]; timeline: FxCue[]; fades: FxFade[] }

  /** 1 tick の長さ(ms)。録画(2026-09-17)で FAIL の出だしから文字が出るまで(54 tick)が約 1.35 秒 → 40 tick/秒 [仮] */
  export const TICK_MS = 25;

  /**
   * AlphaFade の読み替え。アニメの値をそのまま tick にすると録画(2026-09-17)より早く消える。
   * 録画の FAIL は 爆発が約 1.5 秒で薄れ始め 2.3 秒で消え、文字が約 2.7 秒で薄れ始め 3.2 秒で消える。
   * (爆発: 開始 54・長さ 13、文字: 開始 89・長さ 13)に合う倍率を置いた。値の意味は未解明 [仮]
   */
  const FADE_START_SCALE = 1.15;
  const FADE_SCALE = 2;
  const fadeStart = (f: FxFade) => f.start * FADE_START_SCALE;
  const fadeEnd = (f: FxFade) => fadeStart(f) + f.ticks * FADE_SCALE;

  /** トラックごとの見え方。フェードがあるトラックは、最後のコマがフェードし終わるまで残る(録画の見え方) */
  function trackEnd(data: FxData, track: number): number {
    const cues = data.timeline.filter((c) => c.track === track);
    const last = Math.max(...cues.map((c) => c.start + c.hold));
    const fade = data.fades.find((f) => f.track === track);
    return fade ? Math.max(last, fadeEnd(fade)) : last;
  }

  export function fxLength(data: FxData): number {
    return Math.max(...[...new Set(data.timeline.map((c) => c.track))].map((t) => trackEnd(data, t)));
  }
</script>

<script lang="ts">
  import { onDestroy } from "svelte";

  interface Props {
    sheet: string;
    data: FxData;
    /** 基準点(この部品の置かれた入れ物の中の座標) */
    x: number;
    y: number;
    /** 値が変わるたびに頭から再生する。0 のあいだは何も出さない */
    play: number;
    onend?: () => void;
  }
  let { sheet, data, x, y, play, onend }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();
  const image = new Image();
  let raf = 0;

  // 描く範囲 = 全コマの外接矩形(基準点から見た相対)
  const bounds = $derived.by(() => {
    let l = Infinity, t = Infinity, r = -Infinity, b = -Infinity;
    for (const c of data.timeline) {
      const f = data.frames[c.frame];
      l = Math.min(l, f.ox + c.dx);
      t = Math.min(t, f.oy + c.dy);
      r = Math.max(r, f.ox + c.dx + f.w);
      b = Math.max(b, f.oy + c.dy + f.h);
    }
    return { l, t, w: r - l, h: b - t };
  });

  const lastCue = $derived(new Map(data.timeline.map((c) => [c.track, c] as const)));
  const ends = $derived(new Map(data.timeline.map((c) => [c.track, trackEnd(data, c.track)] as const)));

  function alphaOf(track: number, tick: number): number {
    const fade = data.fades.find((f) => f.track === track);
    if (!fade || tick < fadeStart(fade)) return 1;
    return Math.max(0, 1 - (tick - fadeStart(fade)) / (fadeEnd(fade) - fadeStart(fade)));
  }

  function draw(tick: number) {
    const ctx = canvas?.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, bounds.w, bounds.h);
    for (const c of data.timeline) {
      // トラックの最後のコマはフェードし終わるまで出し続ける
      const end = c === lastCue.get(c.track) ? ends.get(c.track)! : c.start + c.hold;
      if (tick < c.start || tick >= end) continue;
      const f = data.frames[c.frame];
      ctx.globalAlpha = alphaOf(c.track, tick);
      ctx.drawImage(image, f.x, f.y, f.w, f.h, f.ox + c.dx - bounds.l, f.oy + c.dy - bounds.t, f.w, f.h);
    }
    ctx.globalAlpha = 1;
  }

  function stop() {
    cancelAnimationFrame(raf);
    raf = 0;
    const ctx = canvas?.getContext("2d");
    ctx?.clearRect(0, 0, bounds.w, bounds.h);
  }

  function start() {
    stop();
    if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
      onend?.();
      return;
    }
    const length = fxLength(data);
    const t0 = performance.now();
    const step = (now: number) => {
      const tick = Math.floor((now - t0) / TICK_MS);
      if (tick >= length) {
        stop();
        onend?.();
        return;
      }
      draw(tick);
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
  }

  $effect(() => {
    image.src = sheet;
    // 初めて押したときにデコード待ちで最初のコマが詰まらないよう、先にデコードしておく
    void image.decode().catch(() => {});
  });

  $effect(() => {
    if (play === 0) {
      stop();
      return;
    }
    if (image.complete) start();
    else image.onload = () => start();
  });

  onDestroy(stop);
</script>

<canvas
  bind:this={canvas}
  class="fx"
  width={bounds.w}
  height={bounds.h}
  style:left="{x + bounds.l}px"
  style:top="{y + bounds.t}px"
  aria-hidden="true"
></canvas>

<style>
  .fx { position: absolute; pointer-events: none; z-index: 5; }
</style>
