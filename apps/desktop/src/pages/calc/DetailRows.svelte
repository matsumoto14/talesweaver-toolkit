<script lang="ts">
  /**
   * 押した数値の内訳(design-system §14 決定 1 の鎖・§00 03「押した場所は動かない」)。
   * 鎖の節・帯の行・次の候補の一覧は、どれも「ラベル / 倍率 / 実数 / 補足」の同じ段なので
   * 行の文法はここ 1 つが持つ。中身の組み立ては calc/damageDetail.ts、開閉は calc/detailStore。
   *
   * `boxed` はインセットの面ごと出す形(読み取り専用。§02)。`open` を渡した面は、トリガが
   * 別の入れ物にいて <details> に載らない場所(鎖の節・帯の見出し)なので、`.open-in` + hidden で
   * 出し入れする。**閉じていても DOM に置いたまま隠す** — {#if} で外すと、閉じている間に称号などを
   * 切り替えた ↑↓・追加/削除 が、開いたときには消えている(差分は要素が前回値を覚えている。§00 04)。
   */
  import { fmtInt, fmtSigned } from "../../format";
  import { t } from "../../i18n";
  import Disclosure from "../../ui/Disclosure.svelte";
  import Value from "../../ui/Value.svelte";
  import { changed } from "../../ui/motion.svelte";
  import type { Detail, DetailStore } from "./detailStore.svelte";

  interface Props {
    d: Detail;
    store: DetailStore;
    /** インセットの面ごと出す(鎖の節の直下・帯の行の中) */
    boxed?: boolean;
    /** 開いているか。渡した面だけが .open-in + hidden で出し入れされる */
    open?: boolean;
  }
  let { d, store, boxed = false, open }: Props = $props();
</script>

{#snippet body()}
  <div class="detail-body">
    {#if d.head !== false}
      <div class="dt-head">
        <span class="dt-hk dim">{t("倍率")}</span>
        <!-- 倍率は書式済みの文字(「×87.7」「—」)で数が来ないので、変わったら光る(<Value> が決める) -->
        <Value class="dt-hv" value={d.mult} />
        <span class="dt-hk dim">{t("実数")}</span>
        <Value
          class={`dt-hv ${(d.delta ?? 0) < 0 ? "bad" : ""}`}
          motion={() => (d.delta === null ? null : Math.round(d.delta))}
          value={d.delta === null ? "—" : fmtSigned(d.delta)}
          delta={{}}
        />
        <span class="dt-hk dim">{t("結果")}</span>
        <Value class="dt-hv big" motion={() => d.to} value={d.to === null ? "—" : fmtInt(Math.round(d.to))} delta={{}} />
      </div>
    {/if}
    {#each d.mats as m, i (i)}
      {#if m.key}
        {@const key = m.key}
        <!-- 押すと要因の一覧が直下に開く(押した行は動かない)。段は内訳の grid のままなので
             <details> は段に溶かす(display: contents) -->
        <Disclosure
          class="dt-fold" summaryClass="dt-row dt-row-btn"
          bind:open={() => store.isOpen(key), (v) => store.setOpen(key, v)}
        >
          {#snippet summary()}
          <span class="dt-label">{#if m.prefix}<span class="dim">{m.prefix}</span> {/if}{m.label}{#if m.note}<span class="dt-swap dim" use:changed={() => m.note ?? ""}>{m.note}</span>{/if}</span>
          <Value class="dt-mult dim" value={m.mult ?? ""} />
          <Value
            class="dt-val"
            motion={() => m.n ?? null}
            value={m.value}
            delta={{ unit: m.unit, digits: m.digits }}
            deltaClass={m.changed ? "follow" : ""}
            onDelta={(e) => { e.stopPropagation(); store.follow(key); }}
          />
          <Value class="dt-sub dim" value={m.sub ?? ""} />
          {/snippet}
          <div class="dt-subs">
            <!-- 出典名でキーにする。入れ替わった出典は「抜けた行(取り消し線)+ 入った行」で残る -->
            {#each m.subs ?? [] as sm, j (sm.id ?? j)}
              <div class="dt-row" class:gone={sm.state === "gone"}>
                <span class="dt-label">{sm.label}</span>
                <Value class="dt-mult dim" value={sm.mult ?? ""} />
                <Value
                  class={`dt-val ${(sm.n ?? 0) < 0 ? "bad" : ""}`}
                  motion={() => sm.n ?? null}
                  value={sm.value}
                  delta={sm.state === "gone" || sm.state === "added" ? null : { unit: sm.unit }}
                />
                {#if sm.state === "gone"}<span class="delta num down delta-in">{t("削除")}</span>{:else if sm.state === "added"}<span class="delta num up delta-in">{t("追加")}</span>{/if}
                <Value class="dt-sub dim" value={sm.sub ?? ""} />
              </div>
            {/each}
          </div>
        </Disclosure>
      {:else}
        <div class="dt-row">
          <span class="dt-label">{#if m.prefix}<span class="dim">{m.prefix}</span> {/if}{m.label}</span>
          <Value class="dt-mult dim" value={m.mult ?? ""} />
          <Value class="dt-val" motion={() => m.n ?? null} value={m.value} delta={{ unit: m.unit, digits: m.digits }} />
          <Value class="dt-sub dim" value={m.sub ?? ""} />
        </div>
      {/if}
    {/each}
    {#if d.idle > 0}
      <p class="dt-note dim">{t("他 {n} 枠は中立(±0)なので、この段では効いていません。", { n: d.idle })}</p>
    {/if}
    {#if d.note}<p class="dt-note dim">{d.note}</p>{/if}
    {#if d.expr}<p class="dt-expr dim">{d.expr}</p>{/if}
  </div>
{/snippet}

{#if boxed}
  <div class="detail inset" class:open-in={open !== undefined} hidden={open === false}>
    {@render body()}
  </div>
{:else}
  {@render body()}
{/if}

<style>
  /* 読み取り専用なのでインセット面、列は band-row と同じ段にそろえる。
     display を上書きしているので、隠れているあいだは自分で消す */
  .detail[hidden] { display: none; }
  .detail { margin: 6px 0 2px; padding: 7px 9px; }
  /* 内訳の行の中の開閉(ui/Disclosure)。段は .detail の縦並びのままにする */
  .detail-body :global(details.dt-fold) { display: contents; }
  /* 中身は <Disclosure> の子としても使う(トリガと面が並ぶ band-row)ので、段は中身側が持つ */
  .detail-body { display: flex; flex-direction: column; gap: 4px; }
  .dt-head { display: flex; align-items: baseline; gap: 6px 7px; flex-wrap: wrap; }
  .dt-hk { font-size: 9px; letter-spacing: 0.06em; }
  /* dt-hv は ui/Value.svelte が描く子要素 */
  .dt-head :global(.dt-hv) { min-width: 62px; font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .dt-head :global(.dt-hv.big) { font-size: 13px; color: var(--fg); }
  .dt-head :global(.dt-hv.bad) { color: var(--danger); }
  .dt-row { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .dt-label { min-width: 0; flex: 1; font-size: 10px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  :global(.dt-mult) { flex-shrink: 0; width: 48px; text-align: right; font-size: 9.5px; }
  /* dt-val も ui/Value.svelte が描く子要素 */
  .dt-row :global(.dt-val) { flex-shrink: 0; width: 64px; text-align: right; font-size: 10px; font-weight: 700; color: var(--fg-sub); }
  :global(.dt-sub) { flex-shrink: 0; width: 112px; text-align: right; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dt-row :global(.dt-val.bad) { color: var(--danger); }
  /* 入れ替わった供給源はラベルの隣に残す(次の変化で書き換わる) */
  .dt-swap { margin-left: 8px; font-size: 9px; }
  /* 抜けた供給源の行。次に集合が変わるまで取り消し線で残す(消すと「どこが変わったか」が消える) */
  .dt-row.gone .dt-label, .dt-row.gone :global(.dt-val) { text-decoration: line-through; color: var(--fg-dim); }
  /* 差分枠は実数と同じ書体サイズ・同じ幅の列にして、行ごとに 到達 の位置がずれないようにする */
  .dt-row :global(.delta), .dt-head :global(.delta) { flex-shrink: 0; min-width: 64px; font-size: 10px; }
  .dt-head :global(.delta) { font-size: 11px; }
  /* ステの行は押すと要因が直下に開く。行の位置・高さは変わらない */
  .detail-body :global(summary.dt-row-btn) {
    width: 100%; text-align: left; padding: 1px 3px; margin: 0 -3px;
    border: 0; background: none; color: inherit; font: inherit; border-radius: var(--r-inset);
  }
  .detail :global(summary.dt-row-btn:hover) { background: var(--bg-active); }
  .detail :global(summary.dt-row-btn:focus-visible) { outline: 2px solid var(--accent); outline-offset: 1px; }
  .dt-subs {
    display: flex; flex-direction: column; gap: 3px;
    margin: 2px 0 3px 8px; padding-left: 8px; border-left: 1px solid var(--border-soft);
  }
  .dt-note, .dt-expr { margin: 2px 0 0; font-size: 9px; line-height: 1.6; }
  .dt-expr { font-family: var(--font-num); font-variant-numeric: tabular-nums; word-break: break-all; }
</style>
