<script lang="ts" module>
  // ゲームの装備システムウィンドウ(exe の CImproveWin、766×586)の「インクリ」タブを再現する。
  // 部品の画像はクライアントの展開物(tools/gamedata/import_inkri_ui.py)。枠・左欄・費用行の寸法と色は
  // ユーザーが録画したゲーム画面(2026-09-17)から測った値。枠と左欄の画像はクライアント内で未特定なので CSS。
  //
  // 見た目はアプリのデザインシステムではなくゲームに合わせる(ユーザー要件 2026-09-17「画面ほぼ一緒」)。
  // このファイルは表示だけを持ち、判定・乱数・費用はページ(Rust 側のコマンド)が持つ。
  // 合成回数はゲームの画面にはあるがこのシミュレータは追わないので出さない(ユーザー判断 2026-09-18)
  export interface WindowItem {
    name: string;
    icon: string | null;
    inkriCount: number;
    destroyed: boolean;
  }
  export interface WindowKind {
    id: string;
    label: string;
    rateLabel: string;
    destroysOnFailure: boolean;
    /** 1 回ごとに呪文書も 1 枚使う(エタインクリ) */
    consumesScroll?: boolean;
  }
</script>

<script lang="ts">
  import panel from "../../assets/inkri/ui/panel.png";
  import radioOff from "../../assets/inkri/ui/radio_0.png";
  import radioOn from "../../assets/inkri/ui/radio_1.png";
  import buttonNormal from "../../assets/inkri/ui/button_inkri_0.png";
  import buttonHover from "../../assets/inkri/ui/button_inkri_1.png";
  import buttonDown from "../../assets/inkri/ui/button_inkri_2.png";
  import buttonDisabled from "../../assets/inkri/ui/button_inkri_3.png";
  import successSheet from "../../assets/inkri/fx/success.webp";
  import successData from "../../assets/inkri/fx/success.json";
  import failSheet from "../../assets/inkri/fx/fail.webp";
  import failData from "../../assets/inkri/fx/fail.json";
  import SpriteFx, { type FxData } from "./SpriteFx.svelte";
  import { t } from "../../i18n";

  const tabImages = import.meta.glob<string>("../../assets/inkri/ui/tab_*.png", {
    eager: true,
    import: "default",
  });
  const tab = (id: string, state: 0 | 1 | 2) => tabImages[`../../assets/inkri/ui/tab_${id}_${state}.png`];
  // ゲームのタブの並び。インクリ以外は押せない(このシミュレータはインクリだけ)
  const TABS = ["aura", "enchant", "inkri", "improve", "mr", "evolve", "element"] as const;

  interface Props {
    item: WindowItem | null;
    kinds: WindowKind[];
    kind: string;
    /** 書式済みの費用。null = 未収録 */
    cost: string | null;
    /** いまの装備で選べない種類の id(エタレベル装備でないときのエタインクリ)。押せない見た目にする */
    unavailable?: string[];
    /** 書式済みの所持 SEED 表示(シミュレータでは消費の累計) */
    seed: string;
    /** 下の一言を差し替える(「これ以上インクリを進行できません。」など)。null ならゲームの既定文 */
    notice: string | null;
    /** まとめて試している間(ボタンを止める) */
    busy: boolean;
    /** 結果演出の番号。値が変わるたびに頭から再生する(連打すると出だしからやり直すのはゲームと同じ) */
    successPlay: number;
    failPlay: number;
    onkind: (id: string) => void;
    onrun: () => void;
    onfxend: () => void;
    onpickitem: () => void;
    onbutton: () => void;
    /** ←キー長押しで連続インクリしている間。ボタンを押し込んだ見た目にする */
    held?: boolean;
  }
  let {
    item, kinds, kind, cost, seed, notice, busy, held = false, unavailable = [],
    successPlay, failPlay, onkind, onrun, onfxend, onpickitem, onbutton,
  }: Props = $props();

  const selected = $derived(kinds.find((k) => k.id === kind) ?? null);
  const shown = $derived(item && !item.destroyed ? item : null);
  const disabled = $derived(busy || shown === null);

  let hover = $state(false);
  let down = $state(false);
  const buttonImage = $derived(disabled ? buttonDisabled : down || held ? buttonDown : hover ? buttonHover : buttonNormal);

  // 結果演出の基準点(本体の中の座標)。録画では説明文の中央に出る
  const FX = { x: 248, y: 258 };
</script>

<div class="tw-window" role="group" aria-label={t("装備システム(インクリ)")}>
  <div class="title">Equipment System<span class="close" aria-hidden="true">✕</span></div>

  <!-- 左: 装備の説明(ゲームのアイテム説明) -->
  <aside class="detail">
    <div class="detail-head">
      <button type="button" class="detail-slot" onclick={onpickitem} title={t("装備を選ぶ")}>
        {#if shown?.icon}<img src={shown.icon} alt="" />{/if}
      </button>
      <div class="detail-name">{shown ? t(shown.name) : t("装備を選択してください。")}</div>
    </div>
    <div class="info-bar">{t("アイテム情報")}<span class="info-min" aria-hidden="true"></span></div>
    {#if shown}
      <div class="detail-body">
        <div>{t("インクリ")}</div>
        {#if shown.inkriCount > 0}<div class="ind blue">{t("インクリ回数 {n}", { n: shown.inkriCount })}</div>{/if}
      </div>
    {/if}
  </aside>

  <!-- 右: タブ + インクリの面 -->
  <div class="tabs">
    {#each TABS as tab_id (tab_id)}
      <img src={tab(tab_id, tab_id === "inkri" ? 1 : 0)} alt={tab_id === "inkri" ? t("インクリ") : ""} />
    {/each}
  </div>

  <section class="body">
    <div class="panel" style:background-image="url({panel})">
      <button type="button" class="slot" onclick={onpickitem} title={t("装備を選ぶ")}>
        {#if shown?.icon}<img src={shown.icon} alt="" />{/if}
      </button>
      {#if shown}
        <div class="name">{t(shown.name)}</div>
        <div class="row r2"><span>{t("エンチャント回数")}</span><b>{t("{n}回", { n: 0 })}</b></div>
        <div class="row r3"><span>{t("インクリ回数")}</span><b>{t("{n}回", { n: shown.inkriCount })}</b></div>
      {/if}
      <div class="list-head">{t("インクリ 選択")}</div>
      <div class="list" role="radiogroup" aria-label={t("インクリの種類")}>
        {#each kinds as k, i (k.id)}
          <button
            type="button"
            role="radio"
            aria-checked={k.id === kind}
            class="kind"
            class:on={k.id === kind}
            class:off={unavailable.includes(k.id)}
            style:top="{43 + i * 25.5}px"
            disabled={busy || unavailable.includes(k.id)}
            onclick={() => { onbutton(); onkind(k.id); }}
          >
            <img src={k.id === kind ? radioOn : radioOff} alt="" />{t(k.label)}
          </button>
        {/each}
      </div>
    </div>

    {#if selected && shown}
      <div class="desc">
        <p>{t("{rate} でインクリが成功します。", { rate: t(selected.rateLabel) })}</p>
        <p>{t("インクリ成功時")} <b>{t("インクリ回数が1増加")}</b> {t("します。")}</p>
        {#if selected.destroysOnFailure}
          <p>{t("インクリ失敗時")} <b>{t("アイテムが破壊")}</b> {t("されます。")}</p>
        {:else}
          <p>{t("インクリが失敗しても")} <b>{t("アイテムは破壊")}</b> {t("されません。")}</p>
        {/if}
        {#if selected.consumesScroll}
          <p>{t("1回ごとに")} <b>{t("エタインクリ呪文書")}</b> {t("を1枚消費します。")}</p>
        {/if}
      </div>
    {/if}

    <img class="cost-radio r1" src={radioOn} alt="" />
    <div class="cost c1">
      <span class="k">{t("費用")}</span><span class="v yellow">{shown ? (cost ?? "?") : "0"}</span>
      <span class="k sep">SEED</span><span class="v">{seed}</span>
    </div>
    <img class="cost-radio r2" src={radioOff} alt="" />
    <div class="cost c2">
      <span class="k">{t("費用")}</span><span class="v">-</span>
      <span class="k sep">ELSO</span><span class="v">-</span>
    </div>
    <p class="note">{notice ?? t("選択したインクリによって費用が異なります。")}</p>

    <div class="strip">
      <button
        type="button"
        class="run"
        {disabled}
        onpointerenter={() => (hover = true)}
        onpointerleave={() => { hover = false; down = false; }}
        onpointerdown={() => (down = true)}
        onpointerup={() => (down = false)}
        onclick={() => { onbutton(); onrun(); }}
      >
        <img src={buttonImage} alt={t("インクリ")} />
      </button>
    </div>

    <SpriteFx sheet={successSheet} data={successData as FxData} x={FX.x} y={FX.y} play={successPlay} onend={onfxend} />
    <SpriteFx sheet={failSheet} data={failData as FxData} x={FX.x} y={FX.y} play={failPlay} onend={onfxend} />
  </section>
</div>

<style>
  /* 書体はゲームの UI(12px のゴシック)。色は録画から拾った値 */
  .tw-window {
    position: relative; width: 766px; height: 586px; flex: none; box-sizing: border-box;
    font: 12px/1.25 "MS PGothic", "MS UI Gothic", "Meiryo UI", sans-serif; color: #3a3a3a;
    background: linear-gradient(#8ea4d6 0%, #7e95cb 60%, #6f84b8 100%);
    border: 1px solid #82878f; box-shadow: inset 0 0 0 3px #484e5f, 0 4px 14px rgb(0 0 0 / 0.35);
    user-select: none;
  }
  .title {
    position: absolute; left: 3px; right: 3px; top: 3px; height: 22px;
    font: bold 13px/22px "Tahoma", "Segoe UI", sans-serif; color: #1f2a3c; text-align: center;
    background: linear-gradient(#e2e9f8, #b3c8ec); border-bottom: 1px solid #6e82b5;
  }
  .close {
    position: absolute; right: 6px; top: 3px; width: 16px; height: 15px; line-height: 15px;
    font-size: 11px; color: #1f2a3c; background: linear-gradient(#fbfcff, #d9e2f3);
    border: 1px solid #6e82b5; border-radius: 2px;
  }

  .detail {
    position: absolute; left: 12px; top: 29px; width: 234px; height: 549px; box-sizing: border-box;
    background: #dbd5cc; border: 2px solid #6a7db2; border-radius: 2px; overflow: hidden;
  }
  .detail-head {
    position: relative; height: 110px;
    background:
      radial-gradient(circle at 50% 45%, rgb(255 255 255 / 0.55) 0 30%, transparent 31% 44%, rgb(255 255 255 / 0.35) 45% 46%, transparent 47%),
      linear-gradient(#e6efff, #c6dbff);
  }
  .detail-slot {
    position: absolute; left: 90px; top: 31px; width: 48px; height: 48px; padding: 0; box-sizing: border-box;
    background: #6b6b6b; border: 3px solid #b99b77; outline: 1px solid #5a4630; border-radius: 3px; cursor: pointer;
  }
  .detail-slot img { width: 36px; height: 36px; margin: 3px; image-rendering: pixelated; }
  .detail-name { position: absolute; left: 0; right: 0; top: 90px; text-align: center; font-weight: bold; font-size: 13px; color: #2a2a2a; }
  .info-bar {
    position: relative; height: 15px; line-height: 15px; text-align: center; color: #fff; font-size: 12px;
    background: linear-gradient(#9c9a98, #85827f); border-top: 1px solid #b4b1ad;
  }
  .info-min { position: absolute; right: 10px; top: 5px; width: 14px; height: 4px; background: #9fe7f5; border-radius: 1px; }
  .detail-body { padding: 8px 14px; line-height: 15px; }
  .ind { padding-left: 12px; }
  .blue { color: #44509a; }

  .tabs { position: absolute; left: 258px; top: 26px; display: flex; gap: 1px; }
  .tabs img { display: block; }

  .body {
    position: absolute; left: 248px; top: 51px; width: 512px; height: 527px; box-sizing: border-box;
    background: linear-gradient(#cfe0f6 0%, #b5d3f3 50%, #a8cfee 88%);
    border: 2px solid #7b92c8; border-radius: 2px; box-shadow: inset 0 0 0 1px #e3ecfb;
  }
  .panel { position: absolute; left: 19px; top: 13px; width: 458px; height: 204px; }
  .slot {
    position: absolute; left: 93px; top: 19px; width: 42px; height: 42px; padding: 0;
    background: transparent; border: 0; cursor: pointer;
  }
  .slot img { width: 34px; height: 34px; margin: 4px; image-rendering: pixelated; }
  .name { position: absolute; left: 0; width: 226px; top: 72px; text-align: center; font-weight: bold; font-size: 13px; color: #2a2a2a; }
  .row { position: absolute; left: 0; width: 226px; display: grid; grid-template-columns: 1fr 1fr; column-gap: 10px; }
  .row span { text-align: right; }
  .row b { font-variant-numeric: tabular-nums; color: #2a2a2a; }
  .r2 { top: 123px; } .r3 { top: 148px; }
  .list-head { position: absolute; left: 228px; width: 228px; top: 16px; text-align: center; font-weight: bold; color: #2a2a2a; }
  .kind {
    position: absolute; left: 277px; height: 22px; padding: 0; display: flex; align-items: center; gap: 13px;
    background: none; border: 0; font: inherit; color: #3a3a3a; cursor: pointer;
  }
  .kind.on { color: #2f6fe0; }
  .kind.off { color: #9a9a9a; cursor: default; }
  .kind img { display: block; }

  .desc { position: absolute; left: 0; right: 0; top: 229px; text-align: center; line-height: 21px; }
  .desc p { margin: 0; }
  .desc b { color: #2a2a2a; }

  .cost-radio { position: absolute; left: 20px; }
  .cost-radio.r1 { top: 390px; } .cost-radio.r2 { top: 412px; }
  .cost {
    position: absolute; left: 38px; width: 431px; height: 20px; box-sizing: border-box;
    display: grid; grid-template-columns: 58px 1fr 58px 1fr; align-items: center;
    background: #86a0ad; border: 1px solid #a9bcc6; border-radius: 2px; color: #fff;
  }
  .c1 { top: 388px; }
  .c2 { top: 410px; opacity: 0.55; }
  .cost .k { font-weight: bold; padding-left: 20px; }
  .cost .v { text-align: center; font-variant-numeric: tabular-nums; }
  .cost .sep { border-left: 1px solid #c9d6dc; }
  .cost .yellow { color: #ffff3c; }
  .note { position: absolute; left: 0; right: 0; top: 441px; margin: 0; text-align: center; color: #c0392b; }

  .strip {
    position: absolute; left: 0; right: 0; bottom: 0; height: 57px;
    background: linear-gradient(#e2eaee, #d8e2e8); border-top: 1px solid #c4d3dc;
  }
  .run { position: absolute; left: 176px; top: 5px; padding: 0; border: 0; background: none; cursor: pointer; }
  .run:disabled { cursor: default; }
  .run img { display: block; }
</style>
