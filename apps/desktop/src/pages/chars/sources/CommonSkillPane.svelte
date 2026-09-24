<script lang="ts">
  import { t } from "../../../i18n";
  // 「commonSkill」補正源のペイン。キャラ横断のパッシブ(オーグメントが Lv の前提)。
  import type { StatKind, StatPreview, UltimateSkill } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { STAT_KINDS, STAT_LABELS, ULTIMATE_SKILLS, ULTIMATE_SKILL_EFFECTS, ULTIMATE_SKILL_LABELS } from "../../../labels";
  import { fmtPct, fmtSigned, fmtSignedPct } from "../../../format";
  import { limits } from "../../../limits.svelte";
  import { tables } from "../../../tables.svelte";
  import Chip from "../../../ui/Chip.svelte";
  import Disclosure from "../../../ui/Disclosure.svelte";
  import Icon from "../../../ui/Icon.svelte";
  import Value from "../../../ui/Value.svelte";
  import Choose from "../../../ui/Choose.svelte";
  import ToggleRow from "../../../ui/ToggleRow.svelte";
  import SkillLevelField from "./SkillLevelField.svelte";
  import {
    defenseRatePercent as defenseRatePercentOf,
    equipmentAttackRatePercent,
    sharpnessRatePercent as sharpnessRatePercentOf,
    unleashSummary as unleashSummaryOf,
  } from "../summaries";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
  }
  let { draft, preview }: Props = $props();

  // --- 効いている量(結果) --------------------------------------------------
  // 行サブタイトルと共有するものは summaries.ts の共有関数(計算は Rust 側 preview / limits)
  const enhanceRatePercent = $derived(equipmentAttackRatePercent(preview));
  const defenseRatePercent = $derived(defenseRatePercentOf(preview));
  const sharpnessRatePercent = $derived(sharpnessRatePercentOf(draft));
  const unleashSummary = $derived(unleashSummaryOf(draft));
  const ultimatePicked = $derived(
    draft.commonSkills.ultimate.slots
      .filter((u) => u !== null)
      .map((u) => ULTIMATE_SKILL_LABELS[u])
      .join(" / ") || t("未習得"),
  );

  // アンリーシュ(能力解放)。効き先は能力値倍率B。Lv6 以降はレインフォース(Lv5 まで)が前提。
  // 正は crates/domain/src/common_skill.rs の UNLEASH。tables.unleash_rates(Σ% の小数表現)経由で引く
  const UNLEASH_RATES = $derived(tables.unleash_rates.map((r) => Math.round(r * 100)));
  const reinforceGate = $derived(draft.commonSkills.reinforce_level + limits.unleash_free_level_max);
  /** レインフォース Lv を下げたら、それに縛られるアンリーシュの Lv も一緒に下げる */
  function setReinforceLevel(level: number) {
    const c = draft.commonSkills;
    c.reinforce_level = level;
    for (const slot of c.unleash) slot.level = Math.min(slot.level, level + limits.unleash_free_level_max);
  }
  const reinforceOptions = $derived(
    Array.from({ length: limits.reinforce_level_max + 1 }, (_, i) => ({
      value: String(i),
      label:
        i === 0
          ? t("未習得(アンリーシュ Lv{n} まで)", { n: limits.unleash_free_level_max })
          : t("Lv{lv}(アンリーシュ Lv{n} まで)", { lv: i, n: i + limits.unleash_free_level_max }),
    })),
  );
  const unleashStatOptions = STAT_KINDS.map((k) => ({ value: k, label: STAT_LABELS[k] }));
  const unleashDisabled = (slotIndex: number) => {
    const other = draft.commonSkills.unleash[1 - slotIndex].stat;
    return other === null ? [] : [other];
  };
  /** いま取れるアンリーシュの上限(レインフォース Lv + 5、最大 10) */
  const unleashCap = $derived(Math.min(limits.unleash_level_max, reinforceGate));
  /** ステを選んだら Lv は上限で入れる。ここは「どのステに乗せるか」だけを決める場所 */
  function setUnleashStat(slotIndex: number, value: string) {
    const slot = draft.commonSkills.unleash[slotIndex];
    slot.stat = value === "" ? null : (value as StatKind);
    slot.level = slot.stat === null ? 0 : unleashCap;
  }
  const unleashLevelOptions = $derived(
    Array.from({ length: Math.min(limits.unleash_level_max, reinforceGate) + 1 }, (_, lv) => ({
      value: String(lv),
      label: lv === 0 ? t("未習得") : t("Lv{lv}({rate})", { lv, rate: fmtSigned(UNLEASH_RATES[lv - 1], { max: 2 }, "%") }),
    })),
  );

  // オーグメントはストロングウェポン / プロテクトアーマー / ハイパーリミットの Lv2 以降の前提スキル。
  // 上限を超える Lv は保存時に Rust 側が弾くので、選択肢の側で先に絞る。
  const augmentGate = $derived(draft.commonSkills.augment_level + limits.augment_gate_offset);
  /**
   * オーグメント Lv を下げたら、それに縛られる Lv(ストロングウェポン / プロテクトアーマー /
   * ハイパーリミット)も一緒に下げる。放置すると選択肢に無い値が残り、保存だけが失敗する。
   */
  function setAugmentLevel(level: number) {
    const c = draft.commonSkills;
    c.augment_level = level;
    const max = level + limits.augment_gate_offset;
    c.strong_weapon_level = Math.min(c.strong_weapon_level, max);
    c.protect_armor_level = Math.min(c.protect_armor_level, max);
    c.ultimate.hyper_limit_level = Math.min(c.ultimate.hyper_limit_level, max);
  }
  // 「未習得」は段に入れない。ほぼ選ばれないものに 1 列を渡さず、外すのは行末の小さな 1 押しで足りる
  const augmentOptions = $derived(
    Array.from({ length: limits.augment_level_max }, (_, i) => ({
      value: String(i + 1),
      label: `Lv${i + 1}`,
    })),
  );
  /** オーグメントで解放されている Lv までを選択肢にする */
  const gatedLevelOptions = (max: number, label: (lv: number) => string) =>
    Array.from({ length: max + 1 }, (_, lv) => ({
      value: String(lv),
      label: lv === 0 ? t("未習得") : label(lv),
      disabled: lv > augmentGate,
    })).filter((o) => !o.disabled);
  // 正は crates/domain/src/common_skill.rs の STRONG_WEAPON_RATE_PER_LEVEL
  const STRONG_WEAPON_RATE_PER_LEVEL = $derived(Math.round(limits.strong_weapon_rate_per_level * 100));
  // 正は crates/domain/src/common_skill.rs の PROTECT_ARMOR_PHYSICAL / _MAGIC
  const PROTECT_ARMOR_RATES = $derived(tables.protect_armor_physical_rates.map((r) => Math.round(r * 100)));
  const PROTECT_ARMOR_MAGIC = $derived(tables.protect_armor_magic_rates.map((r) => Math.round(r * 100)));
  // 畳んだ中の段も上と同じ形にする。名前は Lv の数字だけ、効いている値は行の右
  const levelChoices = (max: number) =>
    Array.from({ length: max }, (_, i) => ({ value: String(i + 1), label: String(i + 1) }));
  const strongWeaponLevels = $derived(levelChoices(limits.strong_weapon_level_max));
  const protectArmorLevels = $derived(levelChoices(limits.protect_armor_level_max));
  const kaiProtectArmorLevels = $derived(levelChoices(limits.kai_protect_armor_level_max));
  const hyperLimitLevels = $derived(levelChoices(limits.hyper_limit_level_max));
  const reinforceLevels = $derived(levelChoices(limits.reinforce_level_max));
  const unleashLevelChoices = $derived(levelChoices(limits.unleash_level_max));
  // 正は crates/domain/src/common_skill.rs の KAI_PROTECT_ARMOR_PHYSICAL / _MAGIC
  const KAI_PROTECT_ARMOR_RATES = $derived(tables.kai_protect_armor_physical_rates.map((r) => Math.round(r * 100)));
  const KAI_PROTECT_ARMOR_MAGIC = $derived(tables.kai_protect_armor_magic_rates.map((r) => Math.round(r * 100)));
  // 正は crates/domain/src/common_skill.rs の SHARPNESS_VISION
  const SHARPNESS_RATES = $derived(tables.sharpness_vision_rates.map((r) => Math.round(r * 100)));
  // 段の名前は Lv だけ、効いている値は行の右に出す(段に「Lv6(+28%)」と書くと折り返す)。
  // **Lv5 まではほぼ全員が同じ**(そこで止まる)なので、ふだんは 5〜10 だけ出す
  const sharpnessVisionOptions = Array.from({ length: limits.sharpness_vision_level_max }, (_, i) => ({
    value: String(i + 1),
    label: String(i + 1),
  }));
  const sharpnessMainOptions = sharpnessVisionOptions.slice(4);
  let sharpnessAllOpen = $state(false);
  const sharpnessIsLow = $derived(
    draft.commonSkills.sharpness_vision_level > 0 && draft.commonSkills.sharpness_vision_level < 5,
  );
  const sharpnessOptionsNow = $derived(
    sharpnessAllOpen || sharpnessIsLow ? sharpnessVisionOptions : sharpnessMainOptions,
  );
  /** 装備防御力倍率(共通スキル + シエナのオーラの防御力増加)のうちシエナぶん。表示用。
   *  正は Equipment::siena_defense_rate(preview.siena_defense_rate。Σ% の小数表現) */
  const sienaDefenseRate = $derived(Math.round((preview?.siena_defense_rate ?? 0) * 100));

  // --- 極限スキル(wiki: Skill/極限)---------------------------------------
  // 3 択から 2 つ。効果値は 基本 + スーパーリミット + ハイパーリミット Lv の加算。
  /** その枠で選べる極限スキル(もう片方の枠で選ばれているものは出さない) */
  /** 極限は「3 つのうち 2 つ」。枠に分けず、押して入れる / 押して外す(§07 形態 3) */
  const ultimatePickedCount = $derived(
    draft.commonSkills.ultimate.slots.filter((u) => u !== null).length,
  );
  function toggleUltimate(skill: UltimateSkill) {
    const slots = draft.commonSkills.ultimate.slots;
    const at = slots.indexOf(skill);
    if (at !== -1) {
      slots[at] = null;
      return;
    }
    const empty = slots.indexOf(null);
    if (empty !== -1) slots[empty] = skill;
  }
  /** フルスロットル(共通スキル)の中ディレイ減少 %。0 = 未装着(計算は Rust 側) */
  const fullThrottlePercent = $derived(
    Math.round((preview?.common_skill.ultimate.actual_delay_reduction ?? 0) * 100),
  );
  /** 選択中の極限スキルの効果値(表示用。計算は Rust 側 = preview.common_skill.ultimate) */
  const ultimateEffects = $derived.by(() => {
    const u = draft.commonSkills.ultimate;
    const effects = preview?.common_skill.ultimate;
    const out: string[] = [];
    if (u.slots.includes("scope_eye")) {
      out.push(t("クリティカルダメージ {v}", { v: fmtSignedPct(effects?.critical_damage_rate ?? 0) }));
    }
    if (u.slots.includes("full_throttle")) {
      out.push(t("中ディレイ {v}", { v: fmtSigned(-fullThrottlePercent, { max: 2 }, "%") }));
    }
    if (u.slots.includes("wide_focus")) {
      out.push(t("スキル範囲 {v}", { v: fmtSigned(effects?.skill_range_bonus ?? 0) }));
    }
    return out;
  });
  const ultimateEffectsText = $derived(ultimateEffects.join(" ・ "));
</script>

<!-- 効いている量(結果)。ペイン自体が既に「共通スキル」の名前を出しているので見出しは持たない -->
<div class="eq-summary num inset">
  <span><span class="dim">{t("装備攻撃力強化")}</span> {fmtSigned(enhanceRatePercent, { max: 2 }, "%")}</span>
  <span><span class="dim">{t("装備防御力")}</span> {t("物")} {defenseRatePercent.physical}% / {t("魔")} {defenseRatePercent.magic}%</span>
  <span><span class="dim">{t("割合追加ダメージ")}</span> {fmtSigned(sharpnessRatePercent, { max: 2 }, "%")}</span>
  <span><span class="dim">{t("アンリーシュ")}</span> {unleashSummary}</span>
</div>
<p class="dim tiny">{t("オーグメント Lv{lv} ・ 極限 {picked}", { lv: draft.commonSkills.augment_level, picked: ultimatePicked })}</p>

<div class="card">
  <div class="card-title inline">
    {t("まず決める")} <span class="dim normal">{t("人によって違うのはここ")}</span>
  </div>
  <div class="skill-fields">
    <SkillLevelField
      label={t("オーグメント")}
      options={augmentOptions}
      cols={augmentOptions.length}
      cell={36}
      value={String(draft.commonSkills.augment_level)}
      onChange={(v) => setAugmentLevel(Number(v))}
      clearLabel={t("未習得")}
      clearDisabled={draft.commonSkills.augment_level === 0}
      onClear={() => setAugmentLevel(0)}
    >
      {#snippet icon()}<Icon kind="skill" id="common_augment" size={20} label={t("オーグメント")} />{/snippet}
    </SkillLevelField>
    <div class="skill-field">
      <span class="k">{t("極限スキル")}</span>
      <div class="ultimate-row">
        {#each ULTIMATE_SKILLS as u (u)}
          {@const on = draft.commonSkills.ultimate.slots.includes(u)}
          <ToggleRow
            name={ULTIMATE_SKILL_LABELS[u]}
            cond={ULTIMATE_SKILL_EFFECTS[u]}
            title={ULTIMATE_SKILL_EFFECTS[u]}
            {on}
            disabled={!on && ultimatePickedCount >= 2}
            onToggle={() => toggleUltimate(u)}
          >
            {#snippet icon()}<Icon kind="skill" id={u} size={20} label={ULTIMATE_SKILL_LABELS[u]} />{/snippet}
          </ToggleRow>
        {/each}
      </div>
      <span class="v num">{ultimatePickedCount} / 2</span>
    </div>
  </div>
  <p class="hint dim">
    {t("いまの効果:")}
    <b><Value value={ultimateEffectsText}>{#snippet children()}{ultimateEffectsText.length > 0 ? ultimateEffectsText : "—"}{/snippet}</Value></b>
  </p>
  <p class="hint dim">
    {t("wiki「Skill/共通」「Skill/極限」。")}<b>{t("オーグメント")}</b>{t("はストロングウェポン・プロテクトアーマー・ ハイパーリミットを Lv2 以上にするための前提で、下げるとそれに縛られる Lv も一緒に下がります。")}
  </p>
</div>

<div class="card">
  <div class="card-title inline">{t("アンリーシュ(能力解放)")}</div>
  <div class="skill-fields">
    {#each draft.commonSkills.unleash as slot, i (i)}
      <div class="skill-field">
        <span class="k"><Icon kind="skill" id="common_unleash" size={20} label={t("アンリーシュ")} />{t("枠 {n}", { n: i + 1 })}</span>
        <Choose
          label={t("アンリーシュ枠{name}のステ", { name: i + 1 })}
          options={unleashStatOptions}
          cols={unleashStatOptions.length}
          disabledValues={unleashDisabled(i)}
          bind:value={() => slot.stat ?? "", (v) => setUnleashStat(i, v)}
        />
        <span class="skill-actions">
          <button
            type="button"
            class="clear"
            disabled={slot.stat === null}
            onclick={() => setUnleashStat(i, "")}
          >{t("未使用")}</button>
        </span>
        <Value class="v" value={slot.stat === null ? "-" : `${UNLEASH_RATES[slot.level - 1]}`}
          >{#snippet children()}{slot.stat === null ? "—" : fmtSigned(UNLEASH_RATES[slot.level - 1], { max: 2 }, "%")}{/snippet}</Value
        >
      </div>
    {/each}
  </div>
  <p class="hint dim">
    {t("選んだステが")}<b>{t("能力値倍率B")}</b>{t("で増えます(")}<b>{t("バフ込みの基本能力値 × 倍率")}</b>{t("なので、バフを盛るほど効きます)。")}<b>{t("2 ステまで")}</b>{t("で、同じステは 2 枠に入れられません。Lv は取れる上限(いまは")} <b>Lv{unleashCap}</b> {t("= {rate})で入ります。", { rate: fmtSigned(UNLEASH_RATES[unleashCap - 1], { max: 2 }, "%") })}
  </p>
</div>

<div class="card">
  <div class="card-title inline">{t("シャープネスビジョン")}</div>
  <div class="skill-fields">
    <SkillLevelField
      label="Lv"
      options={sharpnessOptionsNow}
      cols={sharpnessOptionsNow.length}
      cell={36}
      value={String(draft.commonSkills.sharpness_vision_level)}
      onChange={(v) => (draft.commonSkills.sharpness_vision_level = Number(v))}
      clearLabel={t("未習得")}
      clearDisabled={draft.commonSkills.sharpness_vision_level === 0}
      onClear={() => (draft.commonSkills.sharpness_vision_level = 0)}
      valueText={draft.commonSkills.sharpness_vision_level === 0
        ? "—"
        : fmtSigned(SHARPNESS_RATES[draft.commonSkills.sharpness_vision_level - 1], { max: 2 }, "%")}
      motion={() =>
        draft.commonSkills.sharpness_vision_level === 0
          ? null
          : SHARPNESS_RATES[draft.commonSkills.sharpness_vision_level - 1]}
    >
      {#snippet icon()}<Icon kind="skill" id="common_sharpness_vision" size={20} label={t("シャープネスビジョン")} />{/snippet}
      {#snippet extraAction()}
        {#if !sharpnessIsLow}
          <Chip class="quiet"
 on={sharpnessAllOpen}
 onToggle={() => (sharpnessAllOpen = !sharpnessAllOpen)}
          >{sharpnessAllOpen ? t("5 以上") : t("1〜4")}</Chip>
        {/if}
      {/snippet}
    </SkillLevelField>
  </div>
  <p class="hint dim">
    {t("割合追加ダメージは")}<b>{t("合計ダメージ")}</b>{t("に乗ります(1 発ごとではありません)。 Lv6 以上は各 Lv の習得スクロールが要ります。")}
  </p>
</div>

<div class="card">
  <div class="card-title inline">
    {t("ほぼ全員が同じ設定")} <span class="dim normal">{t("取り切っている前提で入れてあります")}</span>
  </div>
  <Disclosure class="fold">
    {#snippet summary()}{t("取っていない・Lv が違うときだけ開く(8 項目)")}{/snippet}
    <!-- 開いた先も上と同じ形。ラベル / 段 / 操作 / 効いている値の 4 列でそろえる -->
    <div class="fold-body skill-fields">
      <div class="skill-field">
        <span class="k"><Icon kind="skill" id="common_power_weapon" size={20} label={t("パワーウェポン")} />{t("パワーウェポン")}</span>
        <span class="toggle-cell">
          <ToggleRow
            name={t("取っている")}
            value={draft.commonSkills.power_weapon ? fmtSignedPct(limits.power_weapon_rate) : "—"}
            on={draft.commonSkills.power_weapon}
            onToggle={() => (draft.commonSkills.power_weapon = !draft.commonSkills.power_weapon)}
          />
        </span>
      </div>
      <SkillLevelField
        label={t("ストロングウェポン")}
        options={strongWeaponLevels}
        cols={strongWeaponLevels.length}
        cell={36}
        disabledValues={strongWeaponLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
        value={String(draft.commonSkills.strong_weapon_level)}
        onChange={(v) => (draft.commonSkills.strong_weapon_level = Number(v))}
        clearLabel={t("未習得")}
        clearDisabled={draft.commonSkills.strong_weapon_level === 0}
        onClear={() => (draft.commonSkills.strong_weapon_level = 0)}
        valueText={draft.commonSkills.strong_weapon_level === 0 ? "—" : fmtSigned(draft.commonSkills.strong_weapon_level * STRONG_WEAPON_RATE_PER_LEVEL, { max: 2 }, "%")}
        motion={() => draft.commonSkills.strong_weapon_level * STRONG_WEAPON_RATE_PER_LEVEL}
      >
        {#snippet icon()}<Icon kind="skill" id="common_strong_weapon" size={20} label={t("ストロングウェポン")} />{/snippet}
      </SkillLevelField>
      <div class="skill-field">
        <span class="k"><Icon kind="skill" id="common_coat_armor" size={20} label={t("コートアーマー")} />{t("コートアーマー")}</span>
        <span class="toggle-cell">
          <ToggleRow
            name={t("取っている")}
            value={draft.commonSkills.coat_armor
              ? t("物{physical} / 魔{magic}", { physical: fmtPct(limits.coat_armor_physical_rate), magic: fmtPct(limits.coat_armor_magic_rate) })
              : "—"}
            on={draft.commonSkills.coat_armor}
            onToggle={() => (draft.commonSkills.coat_armor = !draft.commonSkills.coat_armor)}
          />
        </span>
      </div>
      <SkillLevelField
        label={t("プロテクトアーマー")}
        options={protectArmorLevels}
        cols={protectArmorLevels.length}
        cell={36}
        disabledValues={protectArmorLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
        value={String(draft.commonSkills.protect_armor_level)}
        onChange={(v) => (draft.commonSkills.protect_armor_level = Number(v))}
        clearLabel={t("未習得")}
        clearDisabled={draft.commonSkills.protect_armor_level === 0}
        onClear={() => (draft.commonSkills.protect_armor_level = 0)}
        valueText={draft.commonSkills.protect_armor_level === 0
          ? "—"
          : t("物{physical} / 魔{magic}%", { physical: PROTECT_ARMOR_RATES[draft.commonSkills.protect_armor_level - 1], magic: PROTECT_ARMOR_MAGIC[draft.commonSkills.protect_armor_level - 1] })}
        motion={() => draft.commonSkills.protect_armor_level}
      >
        {#snippet icon()}<Icon kind="skill" id="common_protect_armor" size={20} label={t("プロテクトアーマー")} />{/snippet}
      </SkillLevelField>
      <SkillLevelField
        label={t("改・プロテクト")}
        options={kaiProtectArmorLevels}
        cols={kaiProtectArmorLevels.length}
        cell={36}
        value={String(draft.commonSkills.kai_protect_armor_level)}
        onChange={(v) => (draft.commonSkills.kai_protect_armor_level = Number(v))}
        clearLabel={t("未習得")}
        clearDisabled={draft.commonSkills.kai_protect_armor_level === 0}
        onClear={() => (draft.commonSkills.kai_protect_armor_level = 0)}
        valueText={draft.commonSkills.kai_protect_armor_level === 0
          ? "—"
          : t("物{physical} / 魔{magic}%", { physical: KAI_PROTECT_ARMOR_RATES[draft.commonSkills.kai_protect_armor_level - 1], magic: KAI_PROTECT_ARMOR_MAGIC[draft.commonSkills.kai_protect_armor_level - 1] })}
        motion={() => draft.commonSkills.kai_protect_armor_level}
      >
        {#snippet icon()}<Icon kind="skill" id="common_kai_protect_armor" size={20} label={t("改・プロテクト")} />{/snippet}
      </SkillLevelField>
      <div class="skill-field">
        <span class="k"><Icon kind="skill" id="common_super_limit" size={20} label={t("スーパーリミット")} />{t("スーパーリミット")}</span>
        <span class="toggle-cell">
          <ToggleRow
            name={t("取っている")}
            value={draft.commonSkills.ultimate.super_limit ? t("極限に加算") : "—"}
            on={draft.commonSkills.ultimate.super_limit}
            onToggle={() => (draft.commonSkills.ultimate.super_limit = !draft.commonSkills.ultimate.super_limit)}
          />
        </span>
      </div>
      <SkillLevelField
        label={t("ハイパーリミット")}
        options={hyperLimitLevels}
        cols={hyperLimitLevels.length}
        cell={36}
        disabledValues={hyperLimitLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
        value={String(draft.commonSkills.ultimate.hyper_limit_level)}
        onChange={(v) => (draft.commonSkills.ultimate.hyper_limit_level = Number(v))}
        clearLabel={t("未習得")}
        clearDisabled={draft.commonSkills.ultimate.hyper_limit_level === 0}
        onClear={() => (draft.commonSkills.ultimate.hyper_limit_level = 0)}
        valueText={draft.commonSkills.ultimate.hyper_limit_level === 0 ? "—" : `Lv${draft.commonSkills.ultimate.hyper_limit_level}`}
      >
        {#snippet icon()}<Icon kind="skill" id="common_hyper_limit" size={20} label={t("ハイパーリミット")} />{/snippet}
      </SkillLevelField>
      <SkillLevelField
        label={t("レインフォース")}
        options={reinforceLevels}
        cols={reinforceLevels.length}
        cell={36}
        value={String(draft.commonSkills.reinforce_level)}
        onChange={(v) => setReinforceLevel(Number(v))}
        clearLabel={t("未習得")}
        clearDisabled={draft.commonSkills.reinforce_level === 0}
        onClear={() => setReinforceLevel(0)}
        valueText={t("Lv{lv} まで", { lv: unleashCap })}
      >
        {#snippet icon()}<Icon kind="skill" id="common_reinforce" size={20} label={t("レインフォース")} />{/snippet}
      </SkillLevelField>
      {#each draft.commonSkills.unleash as slot, i (i)}
        {#if slot.stat !== null}
          <div class="skill-field">
            <span class="k"><Icon kind="skill" id="common_unleash" size={20} label={t("アンリーシュ")} />{t("解放 {n} の Lv", { n: i + 1 })}</span>
            <Choose
              label={t("アンリーシュ解放{name}の Lv", { name: i + 1 })}
              options={unleashLevelChoices}
              cols={unleashLevelChoices.length}
          cell={36}
              disabledValues={unleashLevelChoices.filter((o) => Number(o.value) > unleashCap).map((o) => o.value)}
              bind:value={() => String(slot.level), (v) => (slot.level = Number(v))}
            />
            <span class="skill-actions"></span>
            <span class="v num">{STAT_LABELS[slot.stat]} {fmtSigned(UNLEASH_RATES[slot.level - 1], { max: 2 }, "%")}</span>
          </div>
        {/if}
      {/each}
      <p class="hint dim">
        {t("オーグメントで解放されていない段は押せません。")}
        {#if sienaDefenseRate > 0}{t("装備防御力にはシエナのオーラの {v} を含みます。", { v: fmtSigned(sienaDefenseRate, { max: 2 }, "%") })}{/if}
        <b>{t("リンゴの島・ベリネンルミでは装備防御力は常に 100%")}</b>{t("(wiki 計算式まとめ §防御力)。")}
      </p>
    </div>

  </Disclosure>
</div>
