<script lang="ts">
  /**
   * 開閉するブロック(design-system §10 型 6「開いた / 閉じた」)。土台は `<details>`。
   *
   * 呼ぶ側が決めるのは「トリガに何を出すか」「中に何を出すか」だけ。
   * **キーボード操作 / ページ内検索での自動展開 / 開閉の状態 / 排他(同じ `group` は 1 つだけ開く)/
   * 読み上げの開閉状態**はブラウザが持つ。`aria-expanded` は書かない —
   * `<summary>` の暗黙のロールが既に持っていて、手で書くと二重になる。
   *
   * 動きは app.css の `details::details-content`(`block-size` の補間)が持つ。
   * flex 列の子でも開閉どちらも動くことを実機で確かめてある(WebView2 / Chromium 153、
   * 2026-09-17: 開き 18→218px・閉じ 218→18px を 220ms。`prefers-reduced-motion` で 1 フレーム)。
   * そのため `svelte/transition` を当てる必要はなく、旧 `use:collapse` / `use:disclosurePane` は消えた。
   *
   * キャレットは**トリガの先頭に 1 つだけ**この部品が置いて回す。呼ぶ側は向きも位置も決めない
   * (`▸` / `▾` の文字差し替えは動きが出ないので使わない — §10)。
   *
   * 閉じても中身は DOM に残る(`<details>` の既定。`::details-content` が隠すだけで外さない)ので、
   * 前回値を覚えたまま次に開ける — CalcPage の内訳がこれに依存している。
   */
  import type { Snippet } from "svelte";

  interface Props {
    /** 開閉する面ぜんたいの見た目(`fold` / `contrib` など)。その画面固有の事実で、作法の選択肢ではない */
    class?: string;
    /** トリガの見た目(`chip quiet` など)。Popover の `triggerClass` と同じ役 */
    summaryClass?: string;
    /** 同じ文字列を渡した面は 1 つしか開かない(`<details name>`)。排他が要る面だけ */
    group?: string;
    /** 開いているか。初期値を与える / 外から開かせる面だけ渡す(`bind:open` 可) */
    open?: boolean;
    /** トリガの中身。キャレットは部品が置くので書かない */
    summary: Snippet<[boolean]>;
    /** 面の中身 */
    children: Snippet<[boolean]>;
  }
  let { class: klass = "", summaryClass = "", group, open = $bindable(false), summary, children }: Props = $props();
</script>

<details class={klass} name={group} bind:open>
  <summary class={summaryClass}><span class="caret" aria-hidden="true">▼</span>{@render summary(open)}</summary>
  {@render children(open)}
</details>

<style>
  /* UA の三角は消す(キャレットは自分で置く)。summary は行そのものなので、
     中身の並べ方(flex)は画面側の class が決める */
  summary { list-style: none; cursor: pointer; user-select: none; }
  summary::-webkit-details-marker { display: none; }
  /* 回す向きはここだけが持つ。呼ぶ側に class:rot を書かせない */
  details[open] > summary > .caret { transform: rotate(180deg); }
</style>
