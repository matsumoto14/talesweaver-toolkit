<script lang="ts">
  // インクリ: ゲームの装備システムウィンドウ(インクリタブ)をそのまま再現して試せる画面。
  //
  // 左がゲームの画面(InkriWindow、見た目はゲームに合わせる)、右がこのツールの面(装備を選ぶ・
  // まとめて試す・積み上げ)。判定と乱数と費用はすべて Rust 側(domain::inkri / gamedata::inkri)が持ち、
  // ここは結果を演出と音に変え、「n 回目の成功までに何回・いくら」を段として積むだけ。
  // 合成回数は追わない(ユーザー判断 2026-09-18)。仕様の出典は wiki「装備システム/インクリ」。
  import { onDestroy, onMount } from "svelte";
  import { errorMessage, etaScrollPrice, inkriSuccessRate, listInkriTargets, runInkriAttempts } from "../../api/commands";
  import type {
    EquipmentInkriState, EtaScrollPrice, InkriBatchMode, InkriKind, InkriStep, InkriTarget,
  } from "../../api/types";
  import { fmtInt } from "../../format";
  import { PART_SLOT_LABELS } from "../../labels";
  import { reportError } from "../../toast.svelte";
  import Choose from "../../ui/Choose.svelte";
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
  const saved = persisted("tw-inkri", { itemId: 0, kind: "vianu" as InkriKind, sound: true, startCount: 0 });
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

  async function run(mode: InkriBatchMode) {
    if (!target || asking) return;
    if (itemState.destroyed) {
      showNotice("インクリを進行する装備がありません。");
      return;
    }
    const batch = !("fixed" in mode && mode.fixed.attempts === 1);
    asking = true;
    if (batch) busy = true;
    try {
      const result = await runInkriAttempts({
        client_item_id: target.client_item_id,
        state: itemState,
        kind: saved.value.kind,
        mode,
        seed: randomSeed(),
      });
      if (result.attempts_made === 0) {
        showNotice("これ以上インクリを進行できません。");
        return;
      }
      itemState = result.final_state;
      totals.attempts += result.attempts_made;
      totals.successes += result.successes;
      totals.seed = totals.seed === null || result.consumed_seed === null ? null : totals.seed + result.consumed_seed;
      pushSteps(result.steps);
      // 押した瞬間に結果が出る。連打すると演出は出だしからやり直す(録画 2026-09-17)
      if (result.last_outcome === "success") {
        failPlay = 0;
        successPlay++;
        play(seSuccess);
      } else {
        successPlay = 0;
        failPlay++;
        play(seFail);
        if (result.last_outcome === "failure_destroyed") showNotice("インクリに失敗しました。アイテムが破壊されました。");
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
    void run({ fixed: { attempts: 1 } });
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
    const units = ["", "万", "億", "兆", "京"];
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

  const cost = $derived(
    saved.value.kind === "vianu" ? (target?.bianu_seed_cost ?? null)
    : saved.value.kind === "eta" ? (target?.eta_seed_cost ?? null)
    : null,
  );
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


  const itemOptions = $derived(
    targets
      .filter((t) => t.series === series)
      .map((t) => ({
        value: String(t.client_item_id),
        name: t.name,
        meta: `${PART_SLOT_LABELS[t.part]} · ${t.bianu_seed_cost === null ? "費用 ?" : seedText(t.bianu_seed_cost)}`,
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
      onrun={() => run({ fixed: { attempts: 1 } })}
      onfxend={fxEnd}
      onpickitem={() => picker?.scrollIntoView({ block: "nearest" })}
      onbutton={() => play(seButton)}
      {held}
    />
  </div>

  <div class="side" bind:this={picker}>
    <div class="section">
      <div class="area-head"><span class="area-name">装備</span><span class="area-rule"></span></div>
      <Choose label="系列" class="chiprow" options={seriesList.map((s) => ({ value: s, label: s }))} bind:value={series} />
      <div class="field">
        <span class="field-label">装備</span>
        <Picker
          label="装備"
          bind:value={() => (target && target.series === series ? String(target.client_item_id) : ""), (v) => pickItem(Number(v))}
          options={target && target.series === series ? itemOptions : [{ value: "", name: "選んでください", meta: series }, ...itemOptions]}
        />
      </div>
      <div class="field">
        <span class="field-label">今のインクリ回数</span>
        <NumberField label="今のインクリ回数" max={START_MAX} bind:value={() => saved.value.startCount, setStartCount} />
      </div>
    </div>

    <div class="section">
      <div class="area-head"><span class="area-name">まとめて試す</span><span class="area-rule"></span></div>
      <div class="batch">
        <button type="button" class="btn" disabled={busy} onclick={() => run({ fixed: { attempts: 10 } })}>10回</button>
        <button type="button" class="btn" disabled={busy} onclick={() => run({ fixed: { attempts: 100 } })}>100回</button>
        <button type="button" class="btn primary" disabled={busy} onclick={() => run({ until_success: { max_attempts: 1_000_000 } })}>
          次の成功まで
        </button>
      </div>
      <p class="note dim">
        <kbd>←</kbd> キーを押している間、ゲームと同じようにインクリし続けます。
        まとめて試すボタンは演出を省きます。
      </p>
    </div>

    <div class="section">
      <ToggleRow name="音を出す" on={saved.value.sound} tone="saved" onToggle={() => (saved.value = { ...saved.value, sound: !saved.value.sound })} />
    </div>

    <!-- 最後の面。一覧(.ladder)だけが残りの高さを使ってスクロールし、上の数字とボタンは動かない -->
    <div class="section stack">
      <div class="area-head">
        <span class="area-name">積み上げ</span><span class="area-rule"></span>
        <button type="button" class="btn" disabled={busy || totals.attempts === 0} onclick={reset}>最初から</button>
      </div>
      <div class="rows">
        <ReadRow label="インクリ回数" value="{fmtInt(itemState.inkri_count)}回" motion={() => itemState.inkri_count} />
        <ReadRow label="今の成功率" value={rateText(rate)} motion={() => rate}>
          {#snippet note()}{#if rate !== null}平均 {fmtInt(Math.round(100_000 / rate))}回{/if}{/snippet}
        </ReadRow>
        <ReadRow label="試行" value="{fmtInt(totals.attempts)}回" motion={() => totals.attempts} />
        <ReadRow label="消費 SEED" value={seedText(totals.seed)} motion={() => totals.seed} />
        {#if saved.value.kind === "eta"}
          <ReadRow label="呪文書" value="{fmtInt(totals.attempts)}枚" motion={() => totals.attempts} />
          <!-- 呪文書代。フォレスト(SEED)を主にし、トードー(ELSO)・ルイノの袋(TP)を注記に -->
          <ReadRow label="呪文書代" value={scrollPrice ? seedText(scrollPrice.seed * totals.attempts) : null} motion={() => totals.attempts}>
            {#snippet note()}{#if scrollPrice}または {fmtInt(scrollPrice.elso * totals.attempts)} ELSO / {fmtInt(scrollPrice.tp * totals.attempts)} TP{/if}{/snippet}
          </ReadRow>
        {/if}
      </div>
      {#if steps.length > 0}
        <!-- 新しい段ほど上。段が閉じる(成功する)と次の段がその上に積まれる -->
        <div class="ladder readrows inset">
          {#each steps as step (step.from_count)}
            <div class="step swap-in" class:open={!step.succeeded}>
              <ReadRow label="{fmtInt(step.from_count + 1)}回目" value="{fmtInt(step.attempts)}回" motion={() => step.attempts}>
                {#snippet note()}{step.succeeded ? (seedText(step.seed) ?? "?") : "試行中"}{/snippet}
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
  .field { display: flex; flex-direction: column; gap: 3px; }
  .rows { display: flex; flex-direction: column; gap: 4px; }
  .batch { display: flex; gap: 8px; flex-wrap: wrap; }
  .note { margin: 0; font-size: 10px; line-height: 1.6; }
  kbd { padding: 0 4px; border: 1px solid var(--border); border-radius: 3px; background: var(--bg-field); font: inherit; }
  .stack { flex: 1; min-height: 0; }
  .ladder { flex: 1; min-height: 0; overflow-y: auto; }
  .step.open { font-weight: 700; }
</style>
