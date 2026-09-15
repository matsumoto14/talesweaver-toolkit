<script lang="ts">
  // 行チップ(design-system §07「行チップ」)。オン / オフはアプリ全体でこれ 1 つ。
  // 素のチェックボックス・チェック印付きチップ・aria-pressed の自作ボタンは使わない。
  //
  // - 印(チェック)は置かない。オン / オフは面の色だけで言う:
  //   オン = 水色 + 上端 1px ハイライト(保存される)/ 一時 = ラベンダー / オフ = 白(押せる面)
  // - オフでも値は消さない。選べば何が乗るかを読ませ、色だけ --fg-off に落とす
  // - 値は右端の固定幅(min-width + tabular-nums)。条件は値の手前。30 行並んでも縦に読める
  // - 行の中に別の操作(Lv の段・設定・ポップオーバー)を持つ行は `extra` に置く。
  //   押せる面は名前側(`.face`)だけになり、段を押した瞬間にオフになることがない
  import type { Snippet } from "svelte";
  import { flash } from "./motion.svelte";

  interface Props {
    name: string;
    /** 右端の値。オフでも出す */
    value?: string;
    /** 発動条件など。値の手前に小さく */
    cond?: string;
    on: boolean;
    /** saved = キャラ・セットに保存される(水色)/ temp = この画面だけ(ラベンダー) */
    tone?: "saved" | "temp";
    disabled?: boolean;
    title?: string;
    onToggle: () => void;
    /** 名前の左のアイコン(§08: 名前は必ず併記するので、アイコン単独にはならない) */
    icon?: Snippet;
    /** 押せる面の外に置く別操作(設定ボタン・ポップオーバーなど) */
    extra?: Snippet;
  }
  let { name, value, cond, on, tone = "saved", disabled = false, title, onToggle, icon, extra }: Props = $props();
</script>

<div class="togrow" class:on class:temp={tone === "temp"} class:disabled>
  <button type="button" class="face" {disabled} {title} aria-pressed={on} onclick={onToggle}>
    {#if icon}<span class="ico">{@render icon()}</span>{/if}
    <span class="nm">{name}</span>
    {#if cond !== undefined}<span class="cond">{cond}</span>{/if}
    {#if value !== undefined}<span class="val num" use:flash={() => value ?? ""}>{value}</span>{/if}
  </button>
  {#if extra}{@render extra()}{/if}
</div>

<style>
  .togrow {
    /* 重なりもの(ポップオーバー)の位置の基準。行そのものは動かさない */
    position: relative;
    display: flex; align-items: center; gap: 6px; min-height: 28px;
    padding: 0 9px 0 10px; border-radius: var(--r-panel);
    border: 1px solid var(--border-soft); background: var(--bg-field);
    font-size: 12px; color: var(--fg-sub);
    transition: border-color 0.15s ease, background 0.15s ease;
  }
  .togrow:has(.face:hover:not(:disabled)) { border-color: var(--accent); }
  .togrow:has(.face:focus-visible) { outline: 1px solid var(--accent); outline-offset: 2px; }
  .face {
    flex: 1; min-width: 0; min-height: 26px; display: flex; align-items: center; gap: 8px;
    margin: 0; padding: 0; border: 0; background: none; color: inherit; font: inherit;
    text-align: left; cursor: pointer;
  }
  .face:disabled { cursor: default; opacity: 0.5; }
  .ico { flex: none; display: inline-flex; align-items: center; }
  /* 名前は先に幅を取る。値が長いときは値側を省略し、名前を数文字に潰さない(実機: 極限スキル) */
  .nm { flex: 1 1 auto; min-width: 40%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 500; }
  /* 条件は 1 行に収める。長い注記は title で読ませ、行の高さは変えない */
  .cond {
    margin-left: auto; flex: 0 1 auto; max-width: 45%; min-width: 70px; text-align: right;
    font-size: 9.5px; color: var(--fg-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .val {
    flex: 0 1 auto; margin-left: auto; min-width: 52px; max-width: 60%; text-align: right;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    font-variant-numeric: tabular-nums; font-weight: 700; color: var(--fg-off);
  }
  .cond + .val { margin-left: 0; }
  .on { background: var(--sel); border-color: var(--sel-bd); box-shadow: inset 0 1px 0 #fff; }
  .on .nm { font-weight: 700; color: var(--sel-fg); }
  .on .cond { color: var(--sel-fg); opacity: 0.75; }
  .on .val { color: var(--sel-fg); }
  .on.temp { background: var(--state-temp-bg); border-color: var(--sim); box-shadow: none; }
  .on.temp .nm, .on.temp .val { color: var(--sim-fg); }
  .on.temp .cond { color: var(--sim-fg); }
</style>
