<script lang="ts">
  // 「lumina」補正源のペイン。ルミナの回廊の回廊効果(wiki: ミニゲーム/ルミナの回廊)。
  //
  // 回廊ポイントを払って習得する恒常バフで、**テイルズID 内の全キャラに効く**。キャラごとに
  // 違う値ではないが、このツールはキャラ単位でしか保存先を持たないので補正源として持つ
  // (1 キャラで入れたら他のキャラにも同じ値を入れる必要がある点は画面で言う)。
  //
  // 効くのは「最終ダメージ」(カテゴリL)と「全属性増加」の 2 つだけ。残りは記録のみ。
  // 経済の効果(週間 SEED / ELSO 上限・獲得量・回廊ポイント獲得量)は火力にも能力値にも
  // 効かないので収録しない — 0 で埋めず、扱わないことを文で言う(§00「要らないものを見せない」)。
  import type { StatPreview } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { fmtSigned, fmtSignedPct } from "../../../format";
  import { limits } from "../../../limits.svelte";
  import Choose from "../../../ui/Choose.svelte";
  import Value from "../../../ui/Value.svelte";
  import { t } from "../../../i18n";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
  }
  let { draft, preview }: Props = $props();

  const corridor = $derived(draft.statSources.lumina_corridor);

  /** 0..max の段階選択(§07 形態 2)。研究段階と同じ見せ方に揃える */
  const levelOptions = (max: number) =>
    Array.from({ length: max + 1 }, (_, i) => ({ value: String(i), label: String(i) }));

  type Row = {
    key: "final_damage_level" | "all_element_level" | "damage_reduction_level" | "hp_mp_sp_level";
    name: string;
    max: number;
    /** Lv からいまの効き(表示用の文)。計算に入らない行は recordOnly を立てる */
    effect: (level: number) => string;
    note: string;
    recordOnly?: boolean;
  };

  const rows = $derived<Row[]>([
    {
      key: "final_damage_level",
      name: t("最終ダメージ"),
      max: limits.corridor_final_damage_level_max,
      effect: (level) => fmtSignedPct(level / 100),
      note: t("与ダメージのカテゴリL(最終ダメージ)に乗ります"),
    },
    {
      key: "all_element_level",
      name: t("全属性増加"),
      max: limits.corridor_element_level_max,
      effect: (level) => fmtSigned(level),
      note: t("8 属性それぞれに Lv ぶん乗ります"),
    },
    {
      key: "damage_reduction_level",
      name: t("ダメージ減少"),
      max: limits.corridor_damage_reduction_level_max,
      effect: (level) => fmtSignedPct(level / 100),
      note: t("被ダメージ側はまだ計算に入れていません"),
      recordOnly: true,
    },
    {
      key: "hp_mp_sp_level",
      name: t("HP MP SP 増加"),
      max: limits.corridor_hp_mp_sp_level_max,
      effect: (level) => fmtSignedPct(level * 0.005),
      note: t("HP / MP / SP はこのツールの能力値 7 種に無いので記録だけです"),
      recordOnly: true,
    },
  ]);

  /** 全属性増加で実際に乗っている量(計算は Rust 側。preview を読むだけ) */
  const elementBonus = $derived(preview?.elements.corridor.fire ?? 0);
</script>

<div class="card">
  <div class="card-title">{t("回廊効果")}</div>
  <!-- 名前の列を全行で同じ幅にして、Lv0 の位置(押し始め)を縦に揃える(§00 01 視線を動かさない) -->
  <div class="corridor-rows">
    {#each rows as row (row.key)}
      <span class="corridor-name">
        {row.name}
        {#if row.recordOnly}<span class="badge">{t("記録のみ")}</span>{/if}
      </span>
      <Choose
        label={t("{name}のレベル", { name: row.name })}
        options={levelOptions(row.max)}
        cols={row.max + 1}
        cell={34}
        bind:value={
          () => String(corridor[row.key]),
          (v) => (draft.statSources.lumina_corridor[row.key] = Number(v))
        }
      />
      <Value
        class="corridor-value"
        motion={() => corridor[row.key]}
        value={corridor[row.key] === 0 ? "—" : row.effect(corridor[row.key])}
      />
      <p class="hint dim corridor-note">{row.note}</p>
    {/each}
  </div>
</div>

<p class="hint dim">
  {t("回廊効果は")}<b>{t("テイルズID 内の全キャラ")}</b>{t("に効く恒常バフで、2 週間のリセットでも消えません。 ほかのキャラにも同じ Lv を入れてください。")}
  {#if elementBonus > 0}
    {t("いまは全属性に")} <b><Value motion={() => elementBonus} value={fmtSigned(elementBonus)} /></b> {t("乗っています(属性の補正源で内訳を見られます)。")}
  {/if}
  <br />
  {t("週間 SEED / ELSO の上限・獲得量・回廊ポイント獲得量は、火力にも能力値にも効かないのでここでは扱いません。")}
</p>

<style>
  /* 共有の `.field`(ラベルを上に積むグリッド)とは別物なので名前を分ける。
     名前の列は `max-content` = 一番長い名前(記録のみバッジ込み)の幅で全行そろう。
     min-width で近い幅に寄せるのではなく、列で決めるので Lv0 の位置がぴったり一致する */
  .corridor-rows {
    display: grid;
    grid-template-columns: max-content max-content auto;
    align-items: center;
    column-gap: 10px;
  }
  .corridor-name {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 0;
    font-size: 12px;
  }
  /* 注記は 3 列ぶん使って次の行へ。段の列は名前と段階だけで決める */
  .corridor-note {
    grid-column: 1 / -1;
    margin: 0 0 6px;
  }
</style>
