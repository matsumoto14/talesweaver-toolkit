<script lang="ts">
  // 「commonSkill」補正源のペイン。キャラ横断のパッシブ(オーグメントが Lv の前提)。
  import type { StatKind, StatPreview, UltimateSkill } from "../../../api/types";
  import type { Draft } from "../../../draft";
  import { STAT_KINDS, STAT_LABELS, ULTIMATE_SKILLS, ULTIMATE_SKILL_EFFECTS, ULTIMATE_SKILL_LABELS } from "../../../labels";
  import { limits } from "../../../limits.svelte";
  import { flash } from "../../../ui/motion.svelte";
  import StepSelect from "../../../ui/StepSelect.svelte";
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
      .join(" / ") || "未習得",
  );

  // アンリーシュ(能力解放)。効き先は能力値倍率B。Lv6 以降はレインフォース(Lv5 まで)が前提。
  // 正は crates/domain/src/common_skill.rs の UNLEASH。limits.unleash_rates(Σ% の小数表現)経由で引く
  const UNLEASH_RATES = $derived(limits.unleash_rates.map((r) => Math.round(r * 100)));
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
          ? `未習得(アンリーシュ Lv${limits.unleash_free_level_max} まで)`
          : `Lv${i}(アンリーシュ Lv${i + limits.unleash_free_level_max} まで)`,
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
      label: lv === 0 ? "未習得" : `Lv${lv}(+${UNLEASH_RATES[lv - 1]}%)`,
    })),
  );

  // オーグメントはストロングウェポン / プロテクトアーマー / ハイパーリミットの Lv2 以降の前提スキル。
  // 上限を超える Lv は保存時に Rust 側が弾くので、選択肢の側で先に絞る。
  const augmentGate = $derived(draft.commonSkills.augment_level + 1);
  /**
   * オーグメント Lv を下げたら、それに縛られる Lv(ストロングウェポン / プロテクトアーマー /
   * ハイパーリミット)も一緒に下げる。放置すると選択肢に無い値が残り、保存だけが失敗する。
   */
  function setAugmentLevel(level: number) {
    const c = draft.commonSkills;
    c.augment_level = level;
    const max = level + 1;
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
      label: lv === 0 ? "未習得" : label(lv),
      disabled: lv > augmentGate,
    })).filter((o) => !o.disabled);
  // 正は crates/domain/src/common_skill.rs の STRONG_WEAPON_RATE_PER_LEVEL
  const STRONG_WEAPON_RATE_PER_LEVEL = $derived(Math.round(limits.strong_weapon_rate_per_level * 100));
  // 正は crates/domain/src/common_skill.rs の PROTECT_ARMOR_PHYSICAL / _MAGIC
  const PROTECT_ARMOR_RATES = $derived(limits.protect_armor_physical_rates.map((r) => Math.round(r * 100)));
  const PROTECT_ARMOR_MAGIC = $derived(limits.protect_armor_magic_rates.map((r) => Math.round(r * 100)));
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
  const KAI_PROTECT_ARMOR_RATES = $derived(limits.kai_protect_armor_physical_rates.map((r) => Math.round(r * 100)));
  const KAI_PROTECT_ARMOR_MAGIC = $derived(limits.kai_protect_armor_magic_rates.map((r) => Math.round(r * 100)));
  // 正は crates/domain/src/common_skill.rs の SHARPNESS_VISION
  const SHARPNESS_RATES = $derived(limits.sharpness_vision_rates.map((r) => Math.round(r * 100)));
  // 段の名前は Lv だけ、効いている値は行の右に出す(段に「Lv6(+28%)」と書くと折り返す)。
  // **Lv5 まではほぼ全員が同じ**(そこで止まる)なので、ふだんは 5〜10 だけ出す
  const sharpnessVisionOptions = Array.from({ length: 10 }, (_, i) => ({
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
      out.push(`クリティカルダメージ +${Math.round((effects?.critical_damage_rate ?? 0) * 100)}%`);
    }
    if (u.slots.includes("full_throttle")) {
      out.push(`中ディレイ −${fullThrottlePercent}%`);
      out.push(`単体チャネリング段数 +${effects?.added_hit_count ?? 0}`);
    }
    if (u.slots.includes("wide_focus")) {
      out.push(`スキル範囲 +${effects?.skill_range_bonus ?? 0}`);
    }
    return out;
  });
  const ultimateEffectsText = $derived(ultimateEffects.join(" ・ "));
</script>

<!-- 効いている量(結果)。ペイン自体が既に「共通スキル」の名前を出しているので見出しは持たない -->
<div class="eq-summary num inset">
  <span><span class="dim">装備攻撃力強化</span> +{enhanceRatePercent}%</span>
  <span><span class="dim">装備防御力</span> 物 {defenseRatePercent.physical}% / 魔 {defenseRatePercent.magic}%</span>
  <span><span class="dim">割合追加ダメージ</span> +{sharpnessRatePercent}%</span>
  <span><span class="dim">アンリーシュ</span> {unleashSummary}</span>
</div>
<p class="dim tiny">オーグメント Lv{draft.commonSkills.augment_level} ・ 極限 {ultimatePicked}</p>

<div class="card">
  <div class="card-title inline">
    まず決める <span class="dim normal">人によって違うのはここ</span>
  </div>
  <div class="skill-fields">
    <SkillLevelField
      label="オーグメント"
      options={augmentOptions}
      cols={augmentOptions.length}
      cell={36}
      value={String(draft.commonSkills.augment_level)}
      onChange={(v) => setAugmentLevel(Number(v))}
      clearLabel="未習得"
      clearDisabled={draft.commonSkills.augment_level === 0}
      onClear={() => setAugmentLevel(0)}
    />
    <div class="skill-field">
      <span class="k">極限スキル</span>
      <div class="ultimate-row">
        {#each ULTIMATE_SKILLS as u (u)}
          {@const on = draft.commonSkills.ultimate.slots.includes(u)}
          <button
            type="button"
            class="chip"
            class:on
            disabled={!on && ultimatePickedCount >= 2}
            onclick={() => toggleUltimate(u)}
          >{ULTIMATE_SKILL_LABELS[u]}</button>
        {/each}
      </div>
      <span class="v num">{ultimatePickedCount} / 2</span>
    </div>
    <!-- 選んだ数で行数が変わると、下にあるものが上下する。**2 件ぶんの場所を先に取る**
         (§09 規則 4「あとから寸法が変わらない」) -->
    <div class="ultimate-notes">
      {#each [0, 1] as i (i)}
        {@const picked = draft.commonSkills.ultimate.slots[i]}
        <p class="hint dim skill-note">
          {#if picked !== null}{ULTIMATE_SKILL_LABELS[picked]}: {ULTIMATE_SKILL_EFFECTS[picked]}{/if}
        </p>
      {/each}
    </div>
  </div>
  <p class="hint dim">
    いまの効果:
    <b use:flash={() => ultimateEffectsText}>{ultimateEffectsText.length > 0 ? ultimateEffectsText : "—"}</b>
  </p>
  <p class="hint dim">
    wiki「Skill/共通」「Skill/極限」。<b>オーグメント</b>はストロングウェポン・プロテクトアーマー・
    ハイパーリミットを Lv2 以上にするための前提で、下げるとそれに縛られる Lv も一緒に下がります。
  </p>
</div>

<div class="card">
  <div class="card-title inline">アンリーシュ(能力解放)</div>
  <div class="skill-fields">
    {#each draft.commonSkills.unleash as slot, i (i)}
      <div class="skill-field">
        <span class="k">枠 {i + 1}</span>
        <StepSelect
          label=""
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
          >未使用</button>
        </span>
        <span class="v num" use:flash={() => (slot.stat === null ? "-" : `${UNLEASH_RATES[slot.level - 1]}`)}>
          {slot.stat === null ? "—" : `+${UNLEASH_RATES[slot.level - 1]}%`}
        </span>
      </div>
    {/each}
  </div>
  <p class="hint dim">
    選んだステが<b>能力値倍率B</b>で増えます(<b>バフ込みの基本能力値 × 倍率</b>なので、
    バフを盛るほど効きます)。<b>2 ステまで</b>で、同じステは 2 枠に入れられません。
    Lv は取れる上限(いまは <b>Lv{unleashCap}</b> = +{UNLEASH_RATES[unleashCap - 1]}%)で入ります。
  </p>
</div>

<div class="card">
  <div class="card-title inline">シャープネスビジョン</div>
  <div class="skill-fields">
    <SkillLevelField
      label="Lv"
      options={sharpnessOptionsNow}
      cols={sharpnessOptionsNow.length}
      cell={36}
      value={String(draft.commonSkills.sharpness_vision_level)}
      onChange={(v) => (draft.commonSkills.sharpness_vision_level = Number(v))}
      clearLabel="未習得"
      clearDisabled={draft.commonSkills.sharpness_vision_level === 0}
      onClear={() => (draft.commonSkills.sharpness_vision_level = 0)}
      valueText={draft.commonSkills.sharpness_vision_level === 0
        ? "—"
        : `+${SHARPNESS_RATES[draft.commonSkills.sharpness_vision_level - 1]}%`}
      valueMotion="bump"
      valueKey={draft.commonSkills.sharpness_vision_level === 0
        ? null
        : SHARPNESS_RATES[draft.commonSkills.sharpness_vision_level - 1]}
    >
      {#snippet extraAction()}
        {#if !sharpnessIsLow}
          <button
            type="button"
            class="chip quiet"
            class:on={sharpnessAllOpen}
            onclick={() => (sharpnessAllOpen = !sharpnessAllOpen)}
          >{sharpnessAllOpen ? "5 以上" : "1〜4"}</button>
        {/if}
      {/snippet}
    </SkillLevelField>
  </div>
  <p class="hint dim">
    割合追加ダメージは<b>合計ダメージ</b>に乗ります(1 発ごとではありません)。
    Lv6 以上は各 Lv の習得スクロールが要ります。
  </p>
</div>

<div class="card">
  <div class="card-title inline">
    ほぼ全員が同じ設定 <span class="dim normal">取り切っている前提で入れてあります</span>
  </div>
  <details class="fold">
    <summary>取っていない・Lv が違うときだけ開く(8 項目)</summary>
    <!-- 開いた先も上と同じ形。ラベル / 段 / 操作 / 効いている値の 4 列でそろえる -->
    <div class="fold-body skill-fields">
      <div class="skill-field">
        <span class="k">パワーウェポン</span>
        <span class="chip-row">
          <button
            type="button"
            class="chip"
            class:on={draft.commonSkills.power_weapon}
            onclick={() => (draft.commonSkills.power_weapon = !draft.commonSkills.power_weapon)}
          >取っている</button>
        </span>
        <span class="skill-actions"></span>
        <span class="v num">{draft.commonSkills.power_weapon ? `+${Math.round(limits.power_weapon_rate * 100)}%` : "—"}</span>
      </div>
      <SkillLevelField
        label="ストロングウェポン"
        options={strongWeaponLevels}
        cols={strongWeaponLevels.length}
        cell={36}
        disabledValues={strongWeaponLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
        value={String(draft.commonSkills.strong_weapon_level)}
        onChange={(v) => (draft.commonSkills.strong_weapon_level = Number(v))}
        clearLabel="未習得"
        clearDisabled={draft.commonSkills.strong_weapon_level === 0}
        onClear={() => (draft.commonSkills.strong_weapon_level = 0)}
        valueText={draft.commonSkills.strong_weapon_level === 0 ? "—" : `+${draft.commonSkills.strong_weapon_level * STRONG_WEAPON_RATE_PER_LEVEL}%`}
        valueMotion="bump"
        valueKey={draft.commonSkills.strong_weapon_level * STRONG_WEAPON_RATE_PER_LEVEL}
      />
      <div class="skill-field">
        <span class="k">コートアーマー</span>
        <span class="chip-row">
          <button
            type="button"
            class="chip"
            class:on={draft.commonSkills.coat_armor}
            onclick={() => (draft.commonSkills.coat_armor = !draft.commonSkills.coat_armor)}
          >取っている</button>
        </span>
        <span class="skill-actions"></span>
        <span class="v num">
          {draft.commonSkills.coat_armor
            ? `物${Math.round(limits.coat_armor_physical_rate * 100)} / 魔${Math.round(limits.coat_armor_magic_rate * 100)}%`
            : "—"}
        </span>
      </div>
      <SkillLevelField
        label="プロテクトアーマー"
        options={protectArmorLevels}
        cols={protectArmorLevels.length}
        cell={36}
        disabledValues={protectArmorLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
        value={String(draft.commonSkills.protect_armor_level)}
        onChange={(v) => (draft.commonSkills.protect_armor_level = Number(v))}
        clearLabel="未習得"
        clearDisabled={draft.commonSkills.protect_armor_level === 0}
        onClear={() => (draft.commonSkills.protect_armor_level = 0)}
        valueText={draft.commonSkills.protect_armor_level === 0
          ? "—"
          : `物${PROTECT_ARMOR_RATES[draft.commonSkills.protect_armor_level - 1]} / 魔${PROTECT_ARMOR_MAGIC[draft.commonSkills.protect_armor_level - 1]}%`}
        valueMotion="bump"
        valueKey={draft.commonSkills.protect_armor_level}
      />
      <SkillLevelField
        label="改・プロテクト"
        options={kaiProtectArmorLevels}
        cols={kaiProtectArmorLevels.length}
        cell={36}
        value={String(draft.commonSkills.kai_protect_armor_level)}
        onChange={(v) => (draft.commonSkills.kai_protect_armor_level = Number(v))}
        clearLabel="未習得"
        clearDisabled={draft.commonSkills.kai_protect_armor_level === 0}
        onClear={() => (draft.commonSkills.kai_protect_armor_level = 0)}
        valueText={draft.commonSkills.kai_protect_armor_level === 0
          ? "—"
          : `物${KAI_PROTECT_ARMOR_RATES[draft.commonSkills.kai_protect_armor_level - 1]} / 魔${KAI_PROTECT_ARMOR_MAGIC[draft.commonSkills.kai_protect_armor_level - 1]}%`}
        valueMotion="bump"
        valueKey={draft.commonSkills.kai_protect_armor_level}
      />
      <div class="skill-field">
        <span class="k">スーパーリミット</span>
        <span class="chip-row">
          <button
            type="button"
            class="chip"
            class:on={draft.commonSkills.ultimate.super_limit}
            onclick={() => (draft.commonSkills.ultimate.super_limit = !draft.commonSkills.ultimate.super_limit)}
          >取っている</button>
        </span>
        <span class="skill-actions"></span>
        <span class="v num">{draft.commonSkills.ultimate.super_limit ? "極限に加算" : "—"}</span>
      </div>
      <SkillLevelField
        label="ハイパーリミット"
        options={hyperLimitLevels}
        cols={hyperLimitLevels.length}
        cell={36}
        disabledValues={hyperLimitLevels.filter((o) => Number(o.value) > augmentGate).map((o) => o.value)}
        value={String(draft.commonSkills.ultimate.hyper_limit_level)}
        onChange={(v) => (draft.commonSkills.ultimate.hyper_limit_level = Number(v))}
        clearLabel="未習得"
        clearDisabled={draft.commonSkills.ultimate.hyper_limit_level === 0}
        onClear={() => (draft.commonSkills.ultimate.hyper_limit_level = 0)}
        valueText={draft.commonSkills.ultimate.hyper_limit_level === 0 ? "—" : `Lv${draft.commonSkills.ultimate.hyper_limit_level}`}
      />
      <SkillLevelField
        label="レインフォース"
        options={reinforceLevels}
        cols={reinforceLevels.length}
        cell={36}
        value={String(draft.commonSkills.reinforce_level)}
        onChange={(v) => setReinforceLevel(Number(v))}
        clearLabel="未習得"
        clearDisabled={draft.commonSkills.reinforce_level === 0}
        onClear={() => setReinforceLevel(0)}
        valueText={`Lv${unleashCap} まで`}
      />
      {#each draft.commonSkills.unleash as slot, i (i)}
        {#if slot.stat !== null}
          <div class="skill-field">
            <span class="k">解放 {i + 1} の Lv</span>
            <StepSelect
              label=""
              options={unleashLevelChoices}
              cols={unleashLevelChoices.length}
          cell={36}
              disabledValues={unleashLevelChoices.filter((o) => Number(o.value) > unleashCap).map((o) => o.value)}
              bind:value={() => String(slot.level), (v) => (slot.level = Number(v))}
            />
            <span class="skill-actions"></span>
            <span class="v num">{STAT_LABELS[slot.stat]} +{UNLEASH_RATES[slot.level - 1]}%</span>
          </div>
        {/if}
      {/each}
      <p class="hint dim">
        オーグメントで解放されていない段は押せません。
        {#if sienaDefenseRate > 0}装備防御力にはシエナのオーラの +{sienaDefenseRate}% を含みます。{/if}
        <b>リンゴの島・ベリネンルミでは装備防御力は常に 100%</b>(wiki 計算式まとめ §防御力)。
      </p>
    </div>
  </details>
</div>
