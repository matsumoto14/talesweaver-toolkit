<script lang="ts">
  // 防御側パネル(規格シート 5c)。攻撃タブと同列で「自分がどれだけ耐えるか」を出す。
  // 計算は Rust 側(crates/domain/src/defense.rs)。ここは表示だけ。
  import type { DefenseProfile } from "../../api/types";
  import { fmtInt, fmtNum, fmtPct } from "../../format";
  import { t } from "../../i18n";
  import { limits } from "../../limits.svelte";
  import ReadRow from "../../ui/ReadRow.svelte";
  import SheetCard from "../../ui/SheetCard.svelte";
  import { setContext } from "svelte";
  import { DELTA_SCOPE } from "../../ui/motion.svelte";

  // この面の値は防御の計算(previewDefense)で来る。攻撃の計算が着いたときに
  // 「動かなかった」と判定されて差分が消えないよう、見る世代を宣言する
  setContext(DELTA_SCOPE, "defense");

  interface Props {
    profile: DefenseProfile | null;
    error: string | null;
  }
  let { profile, error }: Props = $props();

  const pct = (v: number) => `${fmtNum(Math.round(v * 1000) / 10)}%`;
  // 防御力の上限で捨てられた分(3 種のどれかが 0 超なら注記を出す)
  const capLoss = $derived(
    profile === null
      ? 0
      : profile.physical_defense_loss + profile.magic_defense_loss + profile.composite_defense_loss,
  );
</script>

<SheetCard tone="blue" title={t("どれだけ耐える？")} note={t("対象コンテンツに依らない自分の値です")} busy={!error && !profile}>
  {#if error}
    <p class="err">{error}</p>
  {:else if !profile}
    <!-- まだ値が来ていない。文言は出さず(待っていることは見出しの印が伝える)、
         行の高さだけ確保して、値が入った瞬間に下がずれないようにする -->
    <p class="empty dim" aria-hidden="true">&nbsp;</p>
  {:else}
    <div class="block">
      <div class="block-head">
        <span class="block-title">{t("防御力")}</span>
        <span class="formula num dim">
          {t("[ステ×{a} + 装備防御×倍率×{b}](複合は (DEF+MR)×{c} + 装備×{d})", {
            a: limits.defense_stat_multiplier, b: limits.defense_equipment_multiplier,
            c: limits.composite_defense_stat_multiplier, d: limits.composite_defense_equipment_multiplier,
          })}
        </span>
      </div>
      <div class="rows readrows inset">
        <ReadRow label={t("物理防御力")} value={fmtInt(profile.physical_defense)} motion={() => profile.physical_defense} delta={{}}>
          {#snippet note()}{t("DEF×{a} + 装備物防 {v}×{r}×{b}", {
            a: limits.defense_stat_multiplier, v: fmtInt(profile.equipment_physical_defense),
            r: fmtNum(profile.defense_rates.physical), b: limits.defense_equipment_multiplier,
          })}{/snippet}
        </ReadRow>
        <ReadRow label={t("魔法防御力")} value={fmtInt(profile.magic_defense)} motion={() => profile.magic_defense} delta={{}}>
          {#snippet note()}{t("MR×{a} + 装備魔防 {v}×{r}×{b}", {
            a: limits.defense_stat_multiplier, v: fmtInt(profile.equipment_magic_defense),
            r: fmtNum(profile.defense_rates.magic), b: limits.defense_equipment_multiplier,
          })}{/snippet}
        </ReadRow>
        <ReadRow label={t("複合防御力")} value={fmtInt(profile.composite_defense)} motion={() => profile.composite_defense} delta={{}}>
          {#snippet note()}{t("(DEF+MR)×{a} + 装備×{b}", {
            a: limits.composite_defense_stat_multiplier, b: limits.composite_defense_equipment_multiplier,
          })}{/snippet}
        </ReadRow>
        <ReadRow label={t("装備防御力倍率")} value={t("物 {v}", { v: fmtPct(profile.defense_rates.physical, { max: 2 }) })} motion={() => profile.defense_rates.physical * 100} delta={{ unit: "%" }}>
          {#snippet note()}{t("魔 {v}。共通スキル(コートアーマー / プロテクトアーマー)+ シエナのオーラの防御力増加。", { v: fmtPct(profile.defense_rates.magic, { max: 2 }) })}<b>{t("リンゴの島・ベリネンルミでは常に 100%")}</b>{/snippet}
        </ReadRow>
        <ReadRow label={t("防御力の上限")} value={fmtInt(profile.defense_cap)} motion={() => profile.defense_cap} delta={{}}>
          {#snippet note()}{t("覚醒段階 + エタの意志 Lv で開放(wiki: Quest/覚醒クエスト・エタの意志)")}{/snippet}
        </ReadRow>
      </div>
      {#if capLoss > 0}
        <p class="note dim">
          {t("上限で捨てられた分: 物理 {p} ・ 魔法 {m} ・ 複合 {c}。ここから先の軽減はカット率 J が担います。", {
            p: fmtInt(profile.physical_defense_loss), m: fmtInt(profile.magic_defense_loss), c: fmtInt(profile.composite_defense_loss),
          })}
        </p>
      {/if}
    </div>

    <div class="block">
      <div class="block-head">
        <span class="block-title">{t("カット率(与ダメージ式の J)")}</span>
        <span class="formula num dim">
          {t("r = 1 − a / (a + {denom})、a = {base} + [(防御ステ + 装備防御 − 1) / {div}]", {
            denom: limits.cut_rate_denominator, base: limits.cut_rate_a_base, div: limits.cut_rate_divisor,
          })}
        </span>
      </div>
      <div class="rows readrows inset">
        <ReadRow label={t("物理")} value={pct(profile.physical_cut_rate)} motion={() => Math.round(profile.physical_cut_rate * 1000) / 10} delta={{ unit: "%", digits: 1 }}>
          {#snippet note()}{t("DEF + 装備物防 から")}{/snippet}
        </ReadRow>
        <ReadRow label={t("魔法")} value={pct(profile.magic_cut_rate)} motion={() => Math.round(profile.magic_cut_rate * 1000) / 10} delta={{ unit: "%", digits: 1 }}>
          {#snippet note()}{t("MR + 装備魔防 から")}{/snippet}
        </ReadRow>
        <ReadRow label={t("複合")} value={pct(profile.composite_cut_rate)} motion={() => Math.round(profile.composite_cut_rate * 1000) / 10} delta={{ unit: "%", digits: 1 }}>
          {#snippet note()}{t("DEF + 装備物防 + MR + 装備魔防 から(除数 {v})", { v: limits.cut_rate_composite_divisor })}{/snippet}
        </ReadRow>
      </div>
      <p class="note dim">{t("防御力には上限があり、上限に届いたあとの軽減はこのカット率が担います。")}</p>
    </div>

    <div class="block">
      <div class="block-head">
        <span class="block-title">{t("回避")}</span>
        <span class="formula num dim">
          {t("回避P = [{base} + (AGI + 装備回避率)×{rate} + 装備敏捷度/{div} + 攻撃タイプ別増加]", {
            base: limits.evasion_point_base, rate: limits.evasion_point_agi_rate, div: limits.evasion_type_divisor,
          })}
        </span>
      </div>
      <div class="rows readrows inset">
        <ReadRow label={t("回避P(物理)")} value={fmtInt(profile.evasion_point.physical)} motion={() => profile.evasion_point.physical} delta={{}}>
          {#snippet note()}{t("+ (DEF×2 + [(突き+斬り)/{a}]) / {b}", { a: limits.evasion_physical_attack_divisor, b: limits.evasion_type_divisor })}{/snippet}
        </ReadRow>
        <ReadRow label={t("回避P(魔法)")} value={fmtInt(profile.evasion_point.magic)} motion={() => profile.evasion_point.magic} delta={{}}>
          {#snippet note()}{t("+ MR×2 / {v}", { v: limits.evasion_type_divisor })}{/snippet}
        </ReadRow>
        <ReadRow label={t("回避P(複合)")} value={fmtInt(profile.evasion_point.composite)} motion={() => profile.evasion_point.composite} delta={{}}>
          {#snippet note()}{t("+ (DEF+MR) / {div}。装備回避率 {evasion}×{rate} + 装備敏捷度 {agi}/{div2} を含む", {
            div: limits.evasion_type_divisor, evasion: fmtInt(profile.equipment_evasion), rate: limits.evasion_point_agi_rate,
            agi: fmtInt(profile.equipment_agility), div2: limits.evasion_type_divisor,
          })}{/snippet}
        </ReadRow>
        <ReadRow label={t("特殊回避(コンボ)")} value={pct(profile.combo_evasion)} motion={() => Math.round(profile.combo_evasion * 1000) / 10} delta={{ unit: "%", digits: 1 }}>
          {#snippet note()}{t("(10 + MR/15 + AGI/7.5)%、下限 20% / 上限 63%")}{/snippet}
        </ReadRow>
      </div>
      <p class="note dim">
        {t("通常回避「率」は敵の命中Pが要り、その入力(wiki 狩り場情報一覧「上限回避P」)が全行未記載なので出しません。回避Pを上げるほど当たりにくくなります(上限 85%)。特殊回避は成功すると多段攻撃の全段を回避します。")}
      </p>
    </div>
  {/if}
</SheetCard>

<style>
  /* .sheet-card/.sheet-head/.gem/.sheet-title/.sheet-char は ui/SheetCard.svelte */
  .empty, .err { margin: 0; padding: 16px 13px; font-size: 11px; }
  .err { color: var(--danger); }

  .block { padding: 11px 13px; border-bottom: 1px solid var(--border-soft); }
  .block:last-child { border-bottom: 0; }
  .block-head { display: flex; align-items: baseline; gap: 9px; flex-wrap: wrap; }
  .block-title { font-size: 11px; font-weight: 700; color: var(--fg-head); white-space: nowrap; }
  .formula { font-size: 9px; line-height: 1.5; }

  .rows { margin-top: 7px; }
  .note { margin: 7px 0 0; font-size: 9px; line-height: 1.6; }
</style>
