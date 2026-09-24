<script lang="ts">
  /**
   * 「なぜこの数字？」— 表記ダメージに至る 3 段(① 攻撃力をつくる → ② 相手の防御力を抜く →
   * ③ 倍率で伸ばす)と、その材料・上限で捨てている量・式のトレースを 1 枚に出す。
   * 値はすべて Rust 由来(DamageTrace / DamageResult)。ここが作るのは並べ方だけで、
   * 段の組み立ては calc/damageDetail.ts、開いた面の状態は calc/detailStore が持つ。
   */
  import type { Attacker, DamageCategory, DamageResult, DefenseProfile } from "../../api/types";
  import { fmtInt, fmtNum, fmtPct, fmtRate, fmtSigned } from "../../format";
  import { t } from "../../i18n";
  import Chip from "../../ui/Chip.svelte";
  import Disclosure from "../../ui/Disclosure.svelte";
  import Value from "../../ui/Value.svelte";
  import { changed } from "../../ui/motion.svelte";
  import TracePanel from "./TracePanel.svelte";
  import DetailRows from "./DetailRows.svelte";
  import type { DetailStore, Mat } from "./detailStore.svelte";
  import {
    activeCategoriesOf, atkDetail, attackRows, catAtCap, changedFlowKeys, fmtCatValue, flowRowsOf,
    lostRowsOf, stepDetail, stepsOf, stepValue,
  } from "./damageDetail";

  interface Props {
    result: DamageResult | null;
    defense: DefenseProfile | null;
    /** 表記ダメージ(1 発)。③ の到達値 */
    perHit: number | null;
    critMode: boolean;
    store: DetailStore;
    /** 誰の数字を掘り下げているか。熊(magic_doll)のときは ① の掘り下げに「係数 STAB(熊)」の行が
     *  1 行増える(ADR-016 突き合わせ)。精霊(destruction_spirit)は本体と同じ行なので増えない。
     *  面の形は変えない */
    attacker: Attacker;
    /** キャラが持つ召喚獣の attacker(熊 = magic_doll / 精霊 = destruction_spirit)。召喚獣が
     *  いないキャラは null(2 択自体を出さない) */
    summonAttacker?: Attacker | null;
    /** 召喚獣がいるキャラだけ見出しに 本体 / 熊(または精霊)の 2 択を置く(ユーザー要望
     *  2026-09-18)。押すと親が attacker を替える */
    onAttacker?: (attacker: Attacker) => void;
  }
  let { result, defense, perHit, critMode, store, attacker, summonAttacker = null, onAttacker }: Props = $props();
  const summonAttackerLabel = $derived(summonAttacker === "destruction_spirit" ? t("精霊") : t("熊"));

  const steps = $derived(stepsOf(result, critMode));
  const atkRows = $derived(attackRows(result?.trace.attack ?? null));
  const atkA = $derived(result?.trace.attack?.value ?? null);
  const defenseValue = $derived(result?.trace.categories.find((c) => c.symbol === "C")?.value ?? null);
  const pierced = $derived(stepValue(steps, "攻撃力−防御力"));
  const noPierce = $derived(pierced !== null && pierced <= 0);
  const defShare = $derived(
    atkA !== null && defenseValue !== null && atkA > 0 ? Math.min(97, (defenseValue / atkA) * 100) : 0,
  );

  const flowRows = $derived(flowRowsOf(steps, pierced));
  const flowTotal = $derived(flowRows.reduce((a, r) => a + Math.max(0, r.add), 0) || 1);
  const flowMultLabel = $derived(
    pierced !== null && pierced > 0 && perHit !== null ? fmtRate(perHit / pierced, 1) : "—",
  );
  /** 直近の計算で変わった段(鎖の ↑ から辿る先)。描画のついでに親 → 子 を控える */
  const changedFlow = $derived.by(() => {
    const keys = changedFlowKeys(store, flowRows, result);
    store.setChildren("perHit", keys);
    return keys;
  });
  const changedAtkKeys = $derived.by(() => {
    const keys = atkRows.filter((a) => store.changes.touch(`atk:${a.k}`, Math.round(a.v), result)).map((a) => `atk:${a.k}`);
    store.setChildren("atkA", keys);
    return keys;
  });

  const activeCategories = $derived(activeCategoriesOf(result));
  const lostRows = $derived(lostRowsOf(result, defense, perHit, critMode));

  // 「一番効いている / 次に伸ばす」の規則は Rust 側(`damage_levers` / `DamageCategory::is_effort`)。
  // ここは結果をカテゴリ行に引き当てて出すだけ
  const categoryById = (id: DamageCategory | null) =>
    id === null ? null : (activeCategories.find((c) => c.category === id) ?? null);
  const topLever = $derived(categoryById(result?.levers.top ?? null));
  const bestLevers = $derived(
    (result?.levers.candidates ?? []).flatMap((lc) => {
      const c = categoryById(lc.category);
      return c ? [{ ...c, gain: lc.gain_percent, headroom: lc.headroom }] : [];
    }),
  );
  const bestLever = $derived(bestLevers[0] ?? null);
  /** 次の候補(2 位以降)。押した行の下に開く */
  const nextLevers = $derived(bestLevers.slice(1));
  let nextLeversOpen = $state(false);
  /** 積み上げの助言(いま効いている / 次に伸ばす)を開いているか。ふだんは畳む */
  let leverOpen = $state(false);
  type Lever = (typeof bestLevers)[number];
  /** +1% 足したときの最終ダメージの伸び(%)(Rust `LeverCandidate::gain_percent`) */
  const leverGain = (c: Lever) => c.gain;
  const bestLeverGain = $derived(bestLever ? leverGain(bestLever) : 0);
  const fmtHeadroom = (c: Lever) =>
    c.headroom !== null ? t("上限まで あと {v}%", { v: fmtNum(c.headroom * 100) }) : t("上限なし");
  /** topLever が乗っている段(帯の行を太字にするため) */
  const topLeverStep = $derived(
    topLever ? (steps.find((s) => s.categories.includes(topLever.category as DamageCategory))?.name ?? null) : null,
  );
  /** 次の候補は段を持たない一覧なので、見出し(倍率 / 実数 / 結果)を出さずに行だけ並べる */
  const nextLeverDetail = $derived({
    mult: "—", delta: null, to: null, idle: 0, expr: null, head: false,
    note: t("+1% 足したときの最終ダメージの伸び。いま積んでいる量が少ないカテゴリほど 1% の価値が高い。"),
    mats: nextLevers.map((c, i): Mat => ({
      prefix: `${i + 2}.`,
      label: `${c.symbol} ${t(c.label)}`,
      mult: fmtCatValue(c),
      value: fmtSigned(leverGain(c), 2, "%"),
      n: leverGain(c),
      unit: "%",
      digits: 2,
      sub: fmtHeadroom(c),
    })),
  });
</script>

<div class="panel">
  <!-- 見出しは全体が開閉のボタン。本体 / 召喚獣 の 2 択(§07 段階選択)はボタンの中に入れられない
       (button の入れ子は不正)ので、同じ帯の上に隣として重ねる。召喚獣がいないキャラでは何も置かない -->
  <div class="panel-head-wrap">
    <button type="button" class="panel-head blue" aria-expanded={store.flowOpen} onclick={() => (store.flowOpen = !store.flowOpen)}>
      <span class="panel-title dark">{t("なぜこの数字？")}</span>
      <span class="panel-note dark">{store.flowOpen ? t("閉じる") : t("内訳をひらく")}</span>
      <span class="caret" class:rot={store.flowOpen}>▼</span>
    </button>
    {#if onAttacker && summonAttacker}
      <div class="who-switch" role="group" aria-label={t("なぜこの数字? を本体 / {who}のどちらで見るか", { who: summonAttackerLabel })}>
        <Chip class="quiet" name="why-attacker" value="player" on={attacker === "player"} onToggle={() => onAttacker?.("player")}>{t("本体")}</Chip>
        <Chip class="quiet" name="why-attacker" value={summonAttacker} on={attacker === summonAttacker} onToggle={() => onAttacker?.(summonAttacker)}>{summonAttackerLabel}</Chip>
      </div>
    {/if}
  </div>
  <div class="panel-body">
    <div class="flow-line">
      <span class="dim">{t("防御を抜けた攻撃力")}</span>
      <Value
        class="strong"
        motion={() => (pierced === null ? null : Math.max(0, Math.trunc(pierced)))}
        value={pierced !== null ? fmtInt(Math.max(0, Math.trunc(pierced))) : "—"}
        delta={{}}
      />
      <span class="arrow num dim">→</span>
      <span class="dim">{t("倍率")}</span>
      <!-- 材料を変えると倍率も変わる。跳ねないと 1 つだけ古い値に見える(§00 04。
           実機の tools/design-audit/live/motion.js が検出した) -->
      <Value class="good strong" value={flowMultLabel} />
      <span class="arrow num dim">→</span>
      <Value class="final" motion={() => perHit} value={perHit !== null ? fmtInt(perHit) : "—"} delta={{}} />
    </div>
    <!-- 積み上げの助言はトグル(ユーザー指示 2026-08-31)。ふだんは畳んでおき、押したときだけ
         下に開く。押すボタンは上の行に居座るので、開いても押した場所は動かない(§00 03)。
         ただし「攻撃力が届いていない」「倍率ゼロ」は畳まない — 数字が伸びない理由そのもので、
         畳むと「なぜこの数字?」に答えないまま閉じることになる -->
    {#if noPierce}
      <div class="lever-note">
        {t("攻撃力が相手の防御力に届いていないので、倍率は何もかかりません。まず攻撃力を上げる必要があります。")}
      </div>
    {:else if topLever}
      <Disclosure class="lever-toggle" summaryClass="chip quiet" bind:open={leverOpen}>
        {#snippet summary(open)}{open ? t("閉じる") : t("どこが効いてる？")}{/snippet}
      <div class="lever-note">
        {t("いま一番効いている積み上げは「{cat}」の {val}(×{factor})。", {
          cat: `${topLever.symbol} ${t(topLever.label)}`, val: fmtCatValue(topLever), factor: fmtNum(topLever.factor),
        })}{catAtCap(topLever) ? t("上限に達しています。") : ""}
        {#if bestLever}
          <br />{t("伸ばすなら「{cat}」。+1% ごとに最終ダメージが", { cat: `${bestLever.symbol} ${t(bestLever.label)}` })} <Value motion={() => bestLeverGain} value={fmtSigned(bestLeverGain, 2, "%")} delta={{ unit: "%", digits: 2 }} /> {t("伸びます({room})。", { room: fmtHeadroom(bestLever) })}
          {#if nextLevers.length > 0}
            <!-- 次の候補。押した行は動かず、直下に増える(§00 03)。列は内訳と同じ段。
                 チップは文中に居るので <details> は段に溶かす(display: contents) -->
            <Disclosure class="next-levers" summaryClass="chip quiet" bind:open={nextLeversOpen}>
              {#snippet summary()}{t("次の候補 {n}", { n: nextLevers.length })}{/snippet}
              <div class="lever-list inset"><DetailRows d={nextLeverDetail} {store} /></div>
            </Disclosure>
          {/if}
        {/if}
      </div>
      </Disclosure>
    {:else}
      <div class="lever-note">{t("倍率はまだ何もかかっていません。")}</div>
    {/if}

    <!-- 閉じていても描画して隠す。閉じている間の変更でも材料の前回値が残り、開いたとき・↑ を辿るときに
         「何が変わったか」が出せる(DetailRows と同じ理由) -->
    <div class="flow-body open-in" hidden={!store.flowOpen}>
      <!-- ① 攻撃力をつくる -->
      <div class="stage">
        <span class="stage-no" style="background: var(--flow-1);">1</span>
        <span class="stage-title">{t("攻撃力をつくる")}</span>
        <Value
          class="strong stage-val" motion={() => atkA} value={atkA !== null ? fmtInt(atkA) : "—"}
          delta={{}} deltaClass={changedAtkKeys.length > 0 ? "follow" : ""}
          onDelta={() => store.follow("atkA")}
        />
      </div>
      <div class="band">
        {#each atkRows as a (a.k)}
          <div style="width: {a.pct}; background: {a.c};"></div>
        {/each}
      </div>
      <div class="band-rows">
        {#each atkRows as a (a.k)}
          <Disclosure
            class="band-fold" summaryClass="band-row"
            bind:open={() => store.isOpen(`atk:${a.k}`), (v) => store.setOpen(`atk:${a.k}`, v)}
          >
            {#snippet summary()}
            <span class="swatch" style="background: {a.c};"></span>
            <span class="br-label">{t(a.k)}</span>
            <span class="br-note dim">{a.note}</span>
            <Value class="br-val" motion={() => Math.round(a.v)} value={fmtInt(Math.round(a.v))} delta={{}} />
            <Value class="br-share dim" motion={() => parseFloat(a.share)} value={a.share} />
            {/snippet}
            <DetailRows boxed d={store.register(`atk:${a.k}`, atkDetail(result, steps, a, attacker))} {store} />
          </Disclosure>
        {/each}
      </div>

      <!-- ② 防御力を抜く -->
      <div class="stage">
        <span class="stage-no" style="background: var(--danger);">2</span>
        <span class="stage-title">{t("相手の防御力を抜く")}</span>
        <Value
          class="strong stage-val"
          motion={() => (pierced === null ? null : Math.max(0, Math.trunc(pierced)))}
          value={pierced !== null ? fmtInt(Math.max(0, Math.trunc(pierced))) : "—"}
          delta={{}}
        />
      </div>
      <div class="band">
        <div style="width: {100 - defShare}%; background: var(--flow-pierce);"></div>
        <div style="width: {defShare}%; background: var(--hatch-lost);"></div>
      </div>
      <div class="pierce-note num">
        <span>{t("攻撃力 {v}", { v: atkA !== null ? fmtInt(atkA) : "—" })}</span>
        <span class="bad">{t("− 防御 {v}", { v: defenseValue !== null ? fmtInt(defenseValue) : "—" })}</span>
        <span class="def-warn" class:bad={defShare >= 60}>
          {defShare >= 60
            ? t("攻撃力の {pct}% が防御力で消えています", { pct: Math.round(defShare) })
            : t("防御で消えるのは {pct}%", { pct: Math.round(defShare) })}
        </span>
      </div>

      <!-- ③ 倍率で伸ばす -->
      <div class="stage">
        <span class="stage-no" style="background: var(--flow-3);">3</span>
        <span class="stage-title">{t("倍率で伸ばす")}</span>
        <span class="stage-note dim">{t("帯の幅＝足した分(赤字は減る倍率)")}</span>
        <Value
          class="strong stage-val" motion={() => perHit} value={perHit !== null ? fmtInt(perHit) : "—"}
          delta={{}} deltaClass={changedFlow.length > 0 ? "follow" : ""}
          onDelta={() => store.follow("perHit")}
        />
      </div>
      <div class="band">
        {#each flowRows.filter((r) => r.add > 0) as f (f.k)}
          <div style="width: {(Math.max(0, f.add) / flowTotal) * 100}%; background: {f.c};"></div>
        {/each}
      </div>
      <div class="band-rows">
        {#each flowRows as f (f.k)}
          <Disclosure
            class="band-fold" summaryClass="band-row"
            bind:open={() => store.isOpen(`flow:${f.k}`), (v) => store.setOpen(`flow:${f.k}`, v)}
          >
            {#snippet summary()}
            <span class="swatch" style="background: {f.c};"></span>
            <span class="br-label" class:strong={topLeverStep === f.k} class:bad={f.add < 0}>{t(f.k)}</span>
            <span class="num br-mult dim">{f.mult}</span>
            <Value
              class={`br-val ${f.add < 0 ? "bad" : ""}`} motion={() => Math.round(f.add)} value={fmtSigned(f.add)}
              delta={{}} deltaClass={changedFlow.includes(`flow:${f.k}`) ? "follow" : ""}
              onDelta={(e) => { e.stopPropagation(); store.follow(`flow:${f.k}`); }}
            />
            <Value class="br-share dim" motion={() => Math.round((Math.abs(f.add) / flowTotal) * 100)} value={fmtPct(Math.abs(f.add) / flowTotal)} />
            {/snippet}
            <DetailRows
              boxed
              d={store.register(`flow:${f.k}`, stepDetail(store, result, steps, atkRows, f.step, f.mult, f.add, f.to))}
              {store}
            />
          </Disclosure>
        {/each}
      </div>

      <!-- 効いていない分(§14 決定 2)。5 階層に散っていた「捨てた量」をここに集める -->
      <div class="materials">
        <div class="mat-head">
          <span class="mat-title">{t("どこで頭打ち？")}</span>
          <span class="dim">{t("積んだのに上限で捨てている量")}</span>
        </div>
        {#if lostRows.length === 0}
          <p class="lost-none dim">{t("まだどの上限にも当たっていません。積んだ分はすべて効いています。")}</p>
        {:else}
          <div class="lost">
            {#each lostRows as r (r.k)}
              <div class="lost-row inset">
                <span class="lost-label">{r.k}</span>
                <Value class="lost-raw" value={r.raw} />
                <span class="lost-arrow dim">{t("→ 上限")}</span>
                <Value class="lost-val" value={r.val} />
                <span class="lost-bar" aria-hidden="true">
                  <i style="width: {r.kept * 100}%"></i>
                  <i class="cut" style="width: {100 - r.kept * 100}%"></i>
                </span>
                <Value class="lost-loss" value={t("{loss} は無効", { loss: r.loss })} />
              </div>
            {/each}
          </div>
          <p class="lost-note dim">{t("斜線が捨てている量です。ここが太い枠は、伸ばしても数字が動きません。")}</p>
        {/if}
      </div>

      <!-- 倍率の材料 -->
      <div class="materials">
        <div class="mat-head">
          <span class="mat-title">{t("倍率の材料")}</span>
          <span class="dim">{t("上限に届いた枠は「満」")}</span>
        </div>
        <div class="mat-chips">
          {#each activeCategories as c (c.category)}
            <span class="mat-chip" class:cap={catAtCap(c)} use:changed={() => (catAtCap(c) ? "cap" : "open")}>
              <span class="dim">{t(c.label)}</span>
              <Value class="strong" value={fmtCatValue(c)} />
              {#if catAtCap(c)}<span class="full">{t("満")}</span>{/if}
            </span>
          {/each}
          {#if activeCategories.length === 0}
            <span class="dim">{t("まだ倍率の材料がありません(バフ・称号などを設定すると増えます)。")}</span>
          {/if}
        </div>
        <!-- 計算に入らないものを明示する(黙って 0 にしない) -->
        <p class="mat-note dim">
          {t("称号・ランダムOP は")}<b>{t("キャラ")}</b>{t("タブで選んだものが入ります(発動条件付きのランダムOP と称号の条件付き効果は記録するだけで計算に入りません)。属性値はキャラの基礎値 + 装備の属性強化で計算しますが、wiki の一覧から属性を読み取れないスキルは属性差ボーナスなし(×1.00)で出ます。防御側(防御力・カット率・回避)は")}<b>{t("防御")}</b>{t("タブに出しています。")}
        </p>
      </div>

      {#if result}
        <TracePanel trace={result.trace} />
      {/if}
    </div>
  </div>
</div>

<style>
  .panel { margin-top: 11px; border-radius: var(--r-window); overflow: hidden; border: 1px solid var(--border-strong); background: var(--bg-field); }
  .panel-head { width: 100%; display: flex; align-items: center; gap: 8px; padding: 7px 12px; text-align: left; }
  .panel-head.blue { background: linear-gradient(180deg, #DBE6F8, #AEC7F0); border-bottom: 1px solid var(--border-strong); cursor: pointer; }
  .panel-title { font-size: var(--t-label); font-weight: 800; letter-spacing: 0.08em; color: #fff; white-space: nowrap; }
  .panel-title.dark { color: var(--fg); }
  .panel-note { min-width: 0; flex: 1; text-align: right; font-size: 9px; color: #E4E3F4; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .panel-note.dark { color: #40536F; }
  .panel-head-wrap { position: relative; }
  /* 見出し文字の右隣。見出しの文字幅は固定(7 文字)なので位置も固定。帯の高さは変えない */
  .who-switch { position: absolute; left: 112px; top: 50%; transform: translateY(-50%); display: flex; gap: 4px; }
  .panel-body { padding: 11px 13px 12px; }

  .flow-line { display: flex; align-items: center; gap: 7px; flex-wrap: wrap; font-size: 9px; }
  /* strong / good / final は ui/Value.svelte が描く子要素 */
  .flow-line :global(.strong) { font-size: 13px; font-weight: 700; color: var(--fg-sub); }
  .flow-line :global(.good.strong) { color: var(--flow-3); }
  .flow-line :global(.final) { font-size: 15px; font-weight: 700; color: var(--fg); }
  .lever-note :global(.chip) { margin-left: 6px; vertical-align: middle; }
  .lever-list {
    margin-top: 6px; padding: 6px 8px;
  }
  /* 一覧の行の文法は calc/DetailRows.svelte が持つ。ここは詰めの差だけ */
  .lever-list :global(.detail-body) { gap: 3px; }
  :global(details.lever-toggle) { margin-top: 9px; }
  /* 文中のチップなので段に溶かす。一覧は .lever-note の子として並ぶ */
  .lever-note :global(details.next-levers) { display: contents; }
  /* トグルの直下に開くので、上マージンは詰める(帯が二重に空かない) */
  :global(details.lever-toggle) .lever-note { margin-top: 6px; }
  .lever-note {
    margin-top: 9px; padding: 8px 10px; border-radius: var(--r-panel);
    background: #F4F9FE; border: 1px solid var(--border-soft);
    font-size: var(--t-label); font-weight: 500; line-height: 1.6; color: var(--fg-sub); text-wrap: pretty;
  }

  /* トリガ(帯の見出し)が別の入れ物にいて <details> に載らないので、面だけを
     §10 型 6 の .open-in + hidden で出し入れする。display を上書きしているので自分で消す */
  .flow-body[hidden] { display: none; }
  .stage { margin-top: 14px; padding-top: 12px; border-top: 1px dashed var(--border-soft); display: flex; align-items: baseline; gap: 8px; min-width: 0; }
  .stage-no { flex-shrink: 0; width: 15px; height: 15px; border-radius: 50%; color: #fff; font-size: 9px; line-height: 16px; text-align: center; font-family: var(--font-num); font-variant-numeric: tabular-nums; font-weight: 700; }
  .stage-title { font-size: 11px; font-weight: 700; white-space: nowrap; }
  .stage-note { min-width: 0; flex: 1; font-size: 9px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* .stage-val は ui/Value.svelte が描く */
  .stage :global(.stage-val) { margin-left: auto; font-size: 15px; font-weight: 700; }
  /* 段の数値と行の数値の右端をそろえる: 数値(64)+ 差分(64)+ 割合(32)の 3 列を段にも持たせる。
     差分枠の幅を固定しないと、文言の幅ぶん数値が左右にずれる(ユーザー指摘 2026-09-15) */
  .stage :global(.delta), .band-rows :global(summary.band-row .delta) { flex-shrink: 0; width: 64px; font-size: 10px; }
  .stage::after { content: ""; flex-shrink: 0; width: 32px; }
  .band { margin-top: 7px; display: flex; height: 11px; border-radius: var(--r-inset); overflow: hidden; border: 1px solid var(--border-soft); background: #EDF2F9; }
  .band > div { flex-shrink: 0; transition: width var(--dur-bar) var(--ease-in-out); }
  .band-rows { margin-top: 8px; display: flex; flex-direction: column; gap: 5px; }
  .band-rows :global(summary.band-row) { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .swatch { flex-shrink: 0; width: 8px; height: 8px; border-radius: var(--r-inset); }
  .br-label { min-width: 0; flex: 1; font-size: var(--t-label); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .br-label.strong { font-weight: 700; }
  .br-label.bad { color: var(--danger); }
  .br-note { min-width: 0; flex: 1.2; font-size: 9px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .br-mult { flex-shrink: 0; width: 48px; text-align: right; font-size: 10px; }
  /* br-val / br-share は ui/Value.svelte が描く子要素 */
  .band-rows :global(.br-val) { flex-shrink: 0; width: 64px; text-align: right; font-size: 11px; font-weight: 700; color: var(--fg-sub); }
  .band-rows :global(.br-val.bad) { color: var(--danger); }
  .band-rows :global(.br-share) { flex-shrink: 0; width: 32px; text-align: right; font-size: 9.5px; }
  /* 構成行・段フローの行は押すと内訳が直下に開く。行の位置・高さは変わらない */
  .band-rows :global(details.band-fold) { display: contents; }
  .band-rows :global(summary.band-row) { width: 100%; text-align: left; border-radius: var(--r-inset); }
  .band-rows :global(summary.band-row:hover) { background: var(--bg-active); box-shadow: 0 0 0 3px var(--bg-active); }
  .band-rows :global(summary.band-row:focus-visible) { outline: 2px solid var(--accent); outline-offset: 2px; }

  .pierce-note { margin-top: 7px; display: flex; align-items: center; gap: 10px; font-size: 9.5px; color: var(--fg-muted); min-width: 0; }
  .def-warn { min-width: 0; flex: 1; text-align: right; font-family: var(--font); font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .def-warn.bad { color: var(--danger); }

  .materials { margin-top: 12px; padding-top: 10px; border-top: 1px dashed var(--border-soft); }
  .mat-head { display: flex; align-items: baseline; gap: 8px; }
  .mat-title { font-size: 10px; font-weight: 700; letter-spacing: 0.06em; color: var(--fg-muted); }
  .mat-head .dim { margin-left: auto; font-size: 9px; }
  .mat-chips { margin-top: 7px; display: flex; flex-wrap: wrap; gap: 5px; }
  .mat-chip {
    display: inline-flex; align-items: center; gap: 6px; padding: 4px 9px; border-radius: var(--r-panel);
    background: var(--bg-panel); border: 1px solid var(--border-soft); font-size: 9.5px;
  }
  .mat-chip.cap { background: var(--state-short-bg); border-color: var(--state-short-bd); }
  /* .strong は ui/Value.svelte が描く子要素(倍率の材料チップの値) */
  .mat-chip :global(.strong) { font-size: 10px; font-weight: 700; }
  .mat-chip .full { font-size: 8.5px; font-weight: 700; color: var(--danger); }
  .mat-note { margin: 7px 0 0; font-size: 9px; line-height: 1.6; }

  /* 効いていない分の棚卸し(§14 決定 2)。塗り = 効いている量、斜線 = 捨てた量(§03) */
  .lost { margin-top: 7px; display: flex; flex-direction: column; gap: 4px; }
  .lost-row {
    display: flex; align-items: center; gap: 8px; min-width: 0;
    padding: 4px 9px;
  }
  .lost-label { min-width: 0; flex: 1; font-size: 10px; font-weight: 700; color: var(--fg-head); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* lost-raw / lost-val / lost-loss は ui/Value.svelte が描く子要素 */
  .lost-row :global(.lost-raw) { flex-shrink: 0; min-width: 62px; text-align: right; font-size: 10px; color: var(--fg-muted); text-decoration: line-through; }
  .lost-arrow { flex-shrink: 0; font-size: 9px; }
  .lost-row :global(.lost-val) { flex-shrink: 0; min-width: 62px; text-align: right; font-size: 10px; font-weight: 700; }
  .lost-bar { flex-shrink: 0; width: 96px; height: 7px; display: flex; border-radius: var(--r-inset); overflow: hidden; border: 1px solid var(--border-soft); }
  .lost-bar > i { display: block; background: var(--flow-1); }
  .lost-bar > i.cut { background: var(--hatch-lost); }
  .lost-row :global(.lost-loss) { flex-shrink: 0; min-width: 104px; text-align: right; font-size: 10px; color: var(--danger); }
  .lost-none, .lost-note { margin: 4px 0 0; font-size: 10px; line-height: 1.6; }
</style>
