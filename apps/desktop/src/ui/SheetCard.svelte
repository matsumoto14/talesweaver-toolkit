<script lang="ts">
  // 「行ける?」(CalcPage・金)と「どれだけ耐える?」(DefensePanel・青)で構造が完全一致していた
  // シート枠(.sheet/.sheet-head/.gem/.sheet-title)を色だけ切り替えて共有する。
  import type { Snippet } from "svelte";
  import Spinner from "./Spinner.svelte";

  interface Props {
    tone: "gold" | "blue";
    title: string;
    /** ヘッダー右側の淡い説明文 */
    note?: string;
    /**
     * 中の数字がまだ確定していない。**見出しの帯に印を出すだけ**で、中身の文言は変えない —
     * 説明文に「・計算中…」を足すと帯の文字数が変わり、押した場所が動く(§09 規則 1)
     */
    busy?: boolean;
    children: Snippet;
  }
  let { tone, title, note = "", busy = false, children }: Props = $props();
</script>

<div class="sheet-card {tone}">
  <div class="sheet-head">
    <span class="gem"></span>
    <span class="sheet-title">{title}</span>
    <span class="sheet-char dim">{note}</span>
    <Spinner active={busy} />
  </div>
  {@render children()}
</div>

<style>
  .sheet-card {
    border-radius: var(--r-window); border: 1px solid var(--border-strong); background: var(--bg-field);
  }
  .sheet-head {
    display: flex; align-items: center; gap: 8px; padding: 8px 13px;
    border-radius: var(--r-window) var(--r-window) 0 0;
  }
  .gem { flex-shrink: 0; width: 9px; height: 9px; transform: rotate(45deg); }
  .sheet-title { font-size: 12px; font-weight: 800; white-space: nowrap; color: var(--fg-head); }
  .sheet-char { min-width: 0; flex: 1; font-size: 9.5px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

  /* 金 = 「行ける?」(CalcPage)。中に .pop の絶対配置ポップを持つので position: relative が要る */
  .sheet-card.gold { position: relative; border-color: var(--sel-bd); box-shadow: 0 1px 0 rgba(121, 140, 172, 0.4); }
  .sheet-card.gold .sheet-head { padding: 7px 13px; background: linear-gradient(180deg, #F2E3BD, #DCC27E); border-bottom: 1px solid #BFA155; }
  .sheet-card.gold .gem { background: linear-gradient(160deg, #fff, #C9A227); border: 1px solid #A9821F; }
  .sheet-card.gold .sheet-title { font-size: 11px; letter-spacing: 0.08em; color: #4A3C12; }
  .sheet-card.gold .sheet-char { font-size: 9px; color: #6B5A24; }

  /* 青 = 「どれだけ耐える?」(DefensePanel) */
  .sheet-card.blue { background: var(--bg-panel); overflow: hidden; }
  .sheet-card.blue .sheet-head { background: linear-gradient(180deg, #E9F1FB, #D8E6F6); border-bottom: 1px solid var(--border); }
  .sheet-card.blue .gem { background: var(--head-bar); border: 1px solid #4C6689; }
</style>
