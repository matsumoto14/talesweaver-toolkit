<script lang="ts">
  // インクリ: ゲームの装備システムウィンドウ(インクリタブ)をそのまま再現して試せる画面。
  //
  // 左がゲームの画面(InkriWindow、見た目はゲームに合わせる)、右がこのツールの面(装備を選ぶ・
  // まとめて試す・積み上げ)。判定と乱数と費用はすべて Rust 側(domain::inkri / gamedata::inkri)が持ち、
  // ここは結果を演出と音に変え、「n 回目の成功までに何回・いくら」を段として積むだけ。
  // 合成回数は追わない(ユーザー判断 2026-09-18)。仕様の出典は wiki「装備システム/インクリ」。
  import { onDestroy, onMount } from "svelte";
  import { errorMessage, etaScrollPrice, inkriSeedCost, inkriSuccessRate, listInkriTargets, runInkriAttempts } from "../../api/commands";
  import type {
    EquipmentInkriState, EtaScrollPrice, InkriKind, InkriRunLimit, InkriStep, InkriTarget,
  } from "../../api/types";
  import { fmtInt } from "../../format";
  import { t } from "../../i18n";
  import { PART_SLOT_LABELS } from "../../labels";
  import { reportError } from "../../toast.svelte";
  import NumberField from "../../ui/NumberField.svelte";
  import Picker from "../../ui/Picker.svelte";
  import ReadRow from "../../ui/ReadRow.svelte";
  import ToggleRow from "../../ui/ToggleRow.svelte";
  import { persisted } from "../../ui/persistedState.svelte";
  import seSuccess from "../../assets/inkri/se/success.wav";
  import seFail from "../../assets/inkri/se/fail.wav";
  import seButton from "../../assets/inkri/se/button.wav";
  import InkriWindow, { type WindowKind } from "./InkriWindow.svelte";

  const itemIcons = import.meta.glob<string>("../../assets/inkri/items/*.png", { eager: true, import: "default" });
  const iconOf = (id: number) => itemIcons[`../../assets/inkri/items/${id}.png`] ?? null;

  // 画面の文言はクライアント DB(dm_00000_0092 / 0419)どおり。成功率・破壊の有無の正は domain::inkri
  const KINDS: (WindowKind & { id: InkriKind })[] = [
    { id: "lord", label: "ロードのインクリ", rateLabel: "低確率", destroysOnFailure: true },
    { id: "grace", label: "加護のインクリ", rateLabel: "低確率", destroysOnFailure: true },
    { id: "blessing", label: "祝福のインクリ", rateLabel: "中確率", destroysOnFailure: true },
    { id: "royal", label: "王室のインクリ", rateLabel: "中確率", destroysOnFailure: true },
    { id: "vianu", label: "ビアヌのインクリ", rateLabel: "極めて低確率", destroysOnFailure: false },
    { id: "eta", label: "エタインクリ", rateLabel: "低確率", destroysOnFailure: false, consumesScroll: true },
  ];

  let targets = $state<InkriTarget[]>([]);
  /** エタインクリ呪文書 1 枚の値段(起動時に 1 回引く) */
  let scrollPrice = $state<EtaScrollPrice | null>(null);
  const saved = persisted("tw-inkri", { itemId: 0, kind: "vianu" as InkriKind, sound: true, startCount: 0, happyHour: false, budgetOku: 100 });
  /** 「今のインクリ回数」の欄の上限。仕組み上の上限は無いので、入力欄の形を保つための値(ユーザー指定 1000、2026-09-18) */
  const START_MAX = 1000;

  const seriesList = $derived([...new Set(targets.map((t) => t.series))]);
  let series = $state("");
  const target = $derived(targets.find((t) => t.client_item_id === saved.value.itemId) ?? null);

  let itemState = $state<EquipmentInkriState>({ inkri_count: 0, destroyed: false });
  const totals = $state({ attempts: 0, successes: 0, seed: 0 as number | null });
  /** 積み上げ。先頭が今取り組んでいる段(まだ成功していない)、以降は成功した段を新しい順に */
  let steps = $state<InkriStep[]>([]);
  /** いまの成功率(10万分率)。種類かインクリ回数が変わるたびに Rust に聞く。null = 取得前 */
  let rate = $state<number | null>(null);
  $effect(() => {
    const kind = saved.value.kind;
    const count = itemState.inkri_count;
    inkriSuccessRate(kind, count).then((r) => {
      if (kind === saved.value.kind && count === itemState.inkri_count) rate = r;
    }).catch((e) => reportError(errorMessage(e)));
  });
  /** 10万分率 → 「0.07%」「21%」 */
  const rateText = (r: number | null) => (r === null ? null : `${(r / 1000).toFixed(3).replace(/\.?0+$/, "")}%`);

  /** まとめて試している間。1 回ずつのインクリは止めない(ゲームは連打できる) */
  let busy = $state(false);
  /** Rust 側に 1 回分を問い合わせている間。同じ状態で 2 回引かないように、その間の連打は捨てる */
  let asking = false;
  /** ウィンドウ下の一言をゲームのメッセージで差し替える */
  let notice = $state<string | null>(null);
  let successPlay = $state(0);
  let failPlay = $state(0);
  let noticeTimer = 0;

  onMount(async () => {
    try {
      [targets, scrollPrice] = await Promise.all([listInkriTargets(), etaScrollPrice()]);
      const first = targets.find((t) => t.client_item_id === saved.value.itemId) ?? targets.find((t) => t.series === "アクィルス") ?? targets[0];
      if (first) pickItem(first.client_item_id);
    } catch (e) {
      reportError(errorMessage(e));
    }
  });

  function pickItem(id: number) {
    const t = targets.find((x) => x.client_item_id === id);
    if (!t) return;
    // エタインクリはエタレベル装備だけ。対象外の装備に替えたらビアヌに戻す
    const kind = saved.value.kind === "eta" && t.eta_seed_cost === null ? "vianu" : saved.value.kind;
    saved.value = { ...saved.value, itemId: id, kind };
    series = t.series;
    reset();
  }

  /** 今のインクリ回数を変える。積み上げはその回数から始め直す */
  function setStartCount(n: number) {
    saved.value = { ...saved.value, startCount: n };
    reset();
  }

  function reset() {
    itemState = { inkri_count: saved.value.startCount, destroyed: false };
    totals.attempts = 0;
    totals.successes = 0;
    totals.seed = 0;
    steps = [];
    notice = null;
    successPlay = 0;
    failPlay = 0;
  }

  /** 今回の内訳を積み上げに足す。先頭の開いた段は続きなので合算する */
  function pushSteps(incoming: InkriStep[]) {
    let next = [...steps];
    for (const step of incoming) {
      const head = next[0];
      if (head && !head.succeeded && head.from_count === step.from_count) {
        next[0] = {
          ...head,
          attempts: head.attempts + step.attempts,
          succeeded: step.succeeded,
          seed: head.seed === null || step.seed === null ? null : head.seed + step.seed,
        };
      } else {
        next = [step, ...next];
      }
    }
    steps = next;
  }

  function play(src: string) {
    if (!saved.value.sound) return;
    const audio = new Audio(src);
    audio.volume = 0.6;
    void audio.play().catch(() => {});
  }

  function showNotice(text: string) {
    notice = text;
    clearTimeout(noticeTimer);
    noticeTimer = window.setTimeout(() => (notice = null), 3000);
  }

  function randomSeed(): number {
    const a = new Uint32Array(2);
    crypto.getRandomValues(a);
    return a[0] * 0x200000 + (a[1] & 0x1fffff); // 53bit に収める
  }

  const untilSuccess = (max_attempts: number): InkriRunLimit => ({ until_success: { max_attempts } });

  async function run(limit: InkriRunLimit) {
    if (!target || asking) return;
    if (itemState.destroyed) {
      showNotice(t("インクリを進行する装備がありません。"));
      return;
    }
    const batch = !("until_success" in limit && limit.until_success.max_attempts === 1);
    asking = true;
    if (batch) busy = true;
    try {
      const result = await runInkriAttempts({
        client_item_id: target.client_item_id,
        state: itemState,
        kind: saved.value.kind,
        limit,
        happy_hour: saved.value.happyHour,
        seed: randomSeed(),
      });
      if (result.attempts_made === 0) {
        showNotice(t("これ以上インクリを進行できません。"));
        return;
      }
      itemState = result.final_state;
      totals.attempts += result.attempts_made;
      totals.successes += result.successes;
      totals.seed = totals.seed === null || result.consumed_seed === null ? null : totals.seed + result.consumed_seed;
      pushSteps(result.steps);
      // 押した瞬間に結果が出る。連打すると演出は出だしからやり直す(録画 2026-09-17)。
      // 予算まで回すと最後の 1 回はたいてい失敗なので、途中で 1 回でも成功していれば成功の演出にする
      if (result.successes > 0) {
        failPlay = 0;
        successPlay++;
        play(seSuccess);
      } else {
        successPlay = 0;
        failPlay++;
        play(seFail);
        if (result.last_outcome === "failure_destroyed") showNotice(t("インクリに失敗しました。アイテムが破壊されました。"));
      }
    } catch (e) {
      reportError(errorMessage(e));
    } finally {
      asking = false;
      busy = false;
    }
  }

  // --- ←キー長押しで連続インクリ -------------------------------------------------
  // 押した瞬間に 1 回、押している間は REPEAT_MS ごとに 1 回。OS のキーリピート(毎秒 30 回前後)には任せない —
  // 演出が出だしのコマから進まず、音も潰れる。録画の連打(約 0.2〜0.3 秒ごと)から始め、
  // ユーザーの指定で倍の速さにした(2026-09-17)
  const REPEAT_MS = 125;
  let held = $state(false);
  let repeatTimer = 0;

  // ←キーはボタンを押していないので、ボタンのクリック音は鳴らさない(ユーザー指定 2026-09-17)
  function pressOnce() {
    void run(untilSuccess(1));
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.key !== "ArrowLeft" || held || busy) return;
    // 入力欄・選択肢の上では矢印キー本来の動き(カーソル移動・選択の移動)を優先する
    const el = e.target as HTMLElement | null;
    if (el?.closest("input, textarea, select, [role=radio], [contenteditable=true]")) return;
    e.preventDefault();
    held = true;
    pressOnce();
    repeatTimer = window.setInterval(() => {
      if (busy || itemState.destroyed) return stopHold();
      pressOnce();
    }, REPEAT_MS);
  }

  function stopHold() {
    held = false;
    clearInterval(repeatTimer);
  }

  function onKeyUp(e: KeyboardEvent) {
    if (e.key === "ArrowLeft") stopHold();
  }

  onDestroy(stopHold);

  function fxEnd() {
    successPlay = 0;
    failPlay = 0;
  }

  // ゲームの表記: 「1050万 0000」「1兆 9908億 3680万 4000」
  function seedText(n: number | null): string | null {
    if (n === null) return null;
    if (n === 0) return "0";
    const units = ["", t("万"), t("億"), t("兆"), t("京")];
    const parts: string[] = [];
    let rest = Math.floor(n);
    let i = 0;
    while (rest > 0) {
      const group = rest % 10000;
      rest = Math.floor(rest / 10000);
      parts.unshift(rest > 0 ? String(group).padStart(4, "0") + units[i] : String(group) + units[i]);
      i++;
    }
    return parts.join(" ");
  }

  /** 1 回あたりの SEED(ハッピーアワーの割引込み)。割引の正は domain::inkri なので Rust に聞く */
  let cost = $state<number | null>(null);
  $effect(() => {
    const id = target?.client_item_id;
    const { kind, happyHour } = saved.value;
    if (id === undefined) return;
    inkriSeedCost(id, kind, happyHour).then((c) => {
      if (id === target?.client_item_id && kind === saved.value.kind && happyHour === saved.value.happyHour) cost = c;
    }).catch((e) => reportError(errorMessage(e)));
  });
  /** 予算は億 SEED で入れる(手持ちの一部だけ回す、のように額は人それぞれ) */
  const OKU = 100_000_000;
  const budgetSeed = $derived(saved.value.budgetOku * OKU);
  /** その予算で回せる回数。費用のない種類・未収録は null */
  const budgetAttempts = $derived(cost === null || cost === 0 ? null : Math.floor(budgetSeed / cost));

  /** いまの装備で選べない種類(エタレベル装備でなければエタインクリ) */
  const unavailable = $derived(target && target.eta_seed_cost === null ? ["eta"] : []);

  const windowItem = $derived(
    target
      ? {
          name: target.name,
          icon: iconOf(target.client_item_id),
          inkriCount: itemState.inkri_count,
          destroyed: itemState.destroyed,
        }
      : null,
  );


  const seriesOptions = $derived(
    seriesList.map((s) => ({ value: s, name: s, meta: t("{n}件", { n: targets.filter((t) => t.series === s).length }) })),
  );

  const itemOptions = $derived(
    targets
      .filter((it) => it.series === series)
      .map((it) => ({
        value: String(it.client_item_id),
        name: it.name,
        iconId: String(it.client_item_id),
        iconKind: "equipment" as const,
        iconSource: iconOf(it.client_item_id),
        meta: `${PART_SLOT_LABELS[it.part]} · ${it.bianu_seed_cost === null ? t("費用 ?") : seedText(it.bianu_seed_cost)}`,
      })),
  );

  let picker: HTMLDivElement | undefined = $state();
</script>

<svelte:window onkeydown={onKeyDown} onkeyup={onKeyUp} onblur={stopHold} />

<div class="inkri-page">
  <div class="game">
    <InkriWindow
      item={windowItem}
      kinds={KINDS}
      kind={saved.value.kind}
      cost={seedText(cost)}
      {unavailable}
      seed={seedText(totals.seed) ?? "?"}
      {notice}
      {busy}
      {successPlay}
      {failPlay}
      onkind={(k) => (saved.value = { ...saved.value, kind: k as InkriKind })}
      onrun={() => run(untilSuccess(1))}
      onfxend={fxEnd}
      onpickitem={() => picker?.scrollIntoView({ block: "nearest" })}
      onbutton={() => play(seButton)}
      {held}
    />
  </div>

  <div class="side" bind:this={picker}>
    <div class="section">
      <div class="area-head"><span class="area-name">{t("装備")}</span><span class="area-rule"></span></div>
      <!-- 系列と今の回数は短いので横に並べ、装備は名前が長いので 1 行を使う -->
      <div class="equip-head">
        <div class="field">
          <span class="field-label">{t("系列")}</span>
          <Picker label={t("系列")} options={seriesOptions} bind:value={series} />
        </div>
        <div class="field">
          <span class="field-label">{t("今のインクリ回数")}</span>
          <NumberField label={t("今のインクリ回数")} max={START_MAX} bind:value={() => saved.value.startCount, setStartCount} />
        </div>
      </div>
      <div class="field">
        <span class="field-label">{t("装備")}</span>
        <Picker
          label={t("装備")}
          bind:value={() => (target && target.series === series ? String(target.client_item_id) : ""), (v) => pickItem(Number(v))}
          options={target && target.series === series ? itemOptions : [{ value: "", name: t("選んでください"), meta: series }, ...itemOptions]}
        />
      </div>
    </div>

    <div class="section">
      <div class="area-head"><span class="area-name">{t("まとめて試す")}</span><span class="area-rule"></span></div>
      <!-- 上の 3 つは成功したら止まる。予算は成功しても続けて、予算を使い切る手前まで回す -->
      <div class="batch">
        <button type="button" class="btn" disabled={busy} onclick={() => run(untilSuccess(10))}>{t("10回")}</button>
        <button type="button" class="btn" disabled={busy} onclick={() => run(untilSuccess(100))}>{t("100回")}</button>
        <button type="button" class="btn primary" disabled={busy} onclick={() => run(untilSuccess(1_000_000))}>
          {t("次の成功まで")}
        </button>
      </div>
      <div class="field">
        <span class="field-label">{t("予算(億 SEED)")}</span>
        <div class="budget">
          <NumberField
            label={t("予算(億 SEED)")}
            min={1}
            digits={5}
            bind:value={() => saved.value.budgetOku, (v) => (saved.value = { ...saved.value, budgetOku: v })}
            format={() => (budgetAttempts === null ? t("費用なし") : t("{n}回分", { n: fmtInt(budgetAttempts) }))}
            reason={t("手持ちに合わせて")}
          />
          <button type="button" class="btn" disabled={busy || !budgetAttempts} onclick={() => run({ budget: { seed: budgetSeed } })}>{t("予算まで回す")}</button>
        </div>
      </div>
      <p class="note dim">
        {t("10回・100回は成功した時点で止まり、予算は成功しても使い切るまで回します(演出は省略)。")}
        <kbd>←</kbd> {t("長押しでゲームと同じように連続インクリ。")}
      </p>
    </div>

    <div class="toggles">
      <ToggleRow name={t("ハッピーアワー")} cond={t("インクリ費用")} value="-20%" on={saved.value.happyHour} tone="saved" onToggle={() => (saved.value = { ...saved.value, happyHour: !saved.value.happyHour })} />
      <ToggleRow name={t("音を出す")} on={saved.value.sound} tone="saved" onToggle={() => (saved.value = { ...saved.value, sound: !saved.value.sound })} />
    </div>

    <!-- 最後の面。一覧(.ladder)だけが残りの高さを使ってスクロールし、上の数字とボタンは動かない -->
    <div class="section stack">
      <div class="area-head">
        <span class="area-name">{t("積み上げ")}</span><span class="area-rule"></span>
        <button type="button" class="btn" disabled={busy || totals.attempts === 0} onclick={reset}>{t("最初から")}</button>
      </div>
      <div class="rows">
        <ReadRow label={t("インクリ回数")} value={t("{n}回", { n: fmtInt(itemState.inkri_count) })} motion={() => itemState.inkri_count} />
        <ReadRow label={t("今の成功率")} value={rateText(rate)} motion={() => rate}>
          {#snippet note()}{#if rate !== null}{t("平均 {n}回", { n: fmtInt(Math.round(100_000 / rate)) })}{/if}{/snippet}
        </ReadRow>
        <ReadRow label={t("試行")} value={t("{n}回", { n: fmtInt(totals.attempts) })} motion={() => totals.attempts} />
        <ReadRow label={t("消費 SEED")} value={seedText(totals.seed)} motion={() => totals.seed} />
        {#if saved.value.kind === "eta"}
          <ReadRow label={t("呪文書")} value={t("{n}枚", { n: fmtInt(totals.attempts) })} motion={() => totals.attempts} />
          <!-- 呪文書代。フォレスト(SEED)を主にし、トードー(ELSO)・ルイノの袋(TP)を注記に -->
          <ReadRow label={t("呪文書代")} value={scrollPrice ? seedText(scrollPrice.seed * totals.attempts) : null} motion={() => totals.attempts}>
            {#snippet note()}{#if scrollPrice}{t("または {seed} ELSO / {tp} TP", { seed: fmtInt(scrollPrice.elso * totals.attempts), tp: fmtInt(scrollPrice.tp * totals.attempts) })}{/if}{/snippet}
          </ReadRow>
        {/if}
      </div>
      {#if steps.length > 0}
        <!-- 新しい段ほど上。段が閉じる(成功する)と次の段がその上に積まれる -->
        <div class="ladder readrows inset">
          {#each steps as step (step.from_count)}
            <div class="step swap-in" class:open={!step.succeeded}>
              <ReadRow label={t("{n}回目", { n: fmtInt(step.from_count + 1) })} value={t("{n}回", { n: fmtInt(step.attempts) })} motion={() => step.attempts}>
                {#snippet note()}{step.succeeded ? (seedText(step.seed) ?? "?") : t("試行中")}{/snippet}
              </ReadRow>
            </div>
          {/each}
        </div>
      {/if}
    </div>

  </div>
</div>

<style>
  .inkri-page { flex: 1; min-width: 0; min-height: 0; display: flex; gap: 18px; overflow: auto; scrollbar-gutter: stable; padding: 16px 22px 22px; background: var(--bg-mid); }
  .game { flex: none; }
  .side { flex: 1; min-width: 240px; max-width: 360px; display: flex; flex-direction: column; gap: 14px; }

  .section { display: flex; flex-direction: column; gap: 8px; }
  .area-head { display: flex; align-items: center; gap: 9px; min-width: 0; }
  .area-name { font-size: 11.5px; font-weight: 800; letter-spacing: 0.08em; color: var(--fg-head); text-shadow: 0 1px 0 rgba(255, 255, 255, 0.9); white-space: nowrap; }
  .area-rule { flex: 1; height: 2px; border-radius: var(--r-inset); background: linear-gradient(90deg, #B9CCE2, rgba(185, 204, 226, 0)); box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8); }
  .field { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .equip-head { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 10px; align-items: end; }
  .rows { display: flex; flex-direction: column; gap: 4px; }
  .batch { display: flex; gap: 8px; flex-wrap: wrap; }
  .budget { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .budget .btn { flex: none; white-space: nowrap; }
  /* オン / オフ 2 つは横に並べて 1 段に収め、残りの高さを積み上げの一覧に回す */
  .toggles { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; }
  .note { margin: 0; font-size: 10px; line-height: 1.6; }
  kbd { padding: 0 4px; border: 1px solid var(--border); border-radius: 3px; background: var(--bg-field); font: inherit; }
  /* 一覧は残りの高さを使うが、窓の下端には付けない(最後の行が縁に貼り付いて見えないように) */
  .stack { flex: 1; min-height: 0; margin-bottom: 16px; }
  .ladder { flex: 1; min-height: 0; overflow-y: auto; }
  .step.open { font-weight: 700; }
</style>
