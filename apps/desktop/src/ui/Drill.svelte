<script lang="ts">
  /**
   * ドリルダウンの行(design-system §09 規則 2)。**行と、行の直下に開く面の両方**を持つ。
   *
   * 呼ぶ側が決めるのは「行に何を出すか」「面に何を出すか」だけ。
   * **進行方向の印(`›`)/ 押した行がその場に残ること(`.on`)/ 開いた面の入場**は部品が持つ。
   * 排他(1 つだけ開く)は呼ぶ側の `open` 1 つが決める — 行が持つ事実ではないため。
   *
   * **面を渡さなければ行だけを出す。**装備・シエナは面が 980px あって一覧の幅に入らないので
   * `<Modal>` に出している。「どこに開くか」を選ばせているのではなく、
   * 直下に面があるかどうか(渡したものが何か)で決まる — 段階 5 の `<Chip on>` と同じ線。
   *
   * 押した行の上には何も差し込まない(面は必ず行の**下**)。開いた面は `.open-in` で
   * 上端から伸びる(§10 型 6)。動きを消す設定では 1 フレームで終わる(app.css)。
   */
  import type { Snippet } from "svelte";

  interface Props {
    /** 開いているか */
    open: boolean;
    /** 行を押したとき。閉じる / 別の行へ移るの判断は呼ぶ側の state が持つ */
    onOpen: () => void;
    /** 行の中身(絵・名前・要約・バッジ)。印は部品が置くので書かない */
    line: Snippet;
    /** 行の直下に開く面。渡さなければ行だけ */
    detail?: Snippet;
    /** 面の見た目(`avatar-editor inset` など、その画面での事実) */
    detailClass?: string;
    /** 面の読み上げ名。行の名前だけでは何の面か決まらないとき */
    detailLabel?: string;
  }
  let { open, onOpen, line, detail, detailClass = "", detailLabel }: Props = $props();
</script>

<button type="button" class="part-row" class:on={open} onclick={onOpen}>
  {@render line()}
  <span class="chev dim" aria-hidden="true">›</span>
</button>
{#if open && detail}
  <div class="open-in {detailClass}" aria-label={detailLabel}>{@render detail()}</div>
{/if}

<style>
  /* 行。押しても動かず、開いているあいだは面で残る(§00 ③) */
  .part-row {
    display: flex; align-items: center; gap: 10px; padding: 9px 11px; border-radius: var(--r-panel);
    background: var(--bg-field); border: 1px solid var(--border-soft); text-align: left;
  }
  .part-row:hover { border-color: var(--accent); }
  /* いま開いている部位。押した行がその場に残っていることを面で示す */
  .part-row.on { background: var(--bg-active); border-color: var(--accent); }
  /* 進行方向の印は 1 つだけ。開閉で文字を差し替えると動きが出ず、開いたのかが一瞬わからない(§10) */
  .chev { flex-shrink: 0; font-size: 11px; }
</style>
