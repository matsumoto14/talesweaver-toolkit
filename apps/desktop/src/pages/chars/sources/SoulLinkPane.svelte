<script lang="ts">
  import type { SoulLinkStatus, StatPreview } from "../../../api/types";
  import { fmtRate, fmtSigned, fmtSignedPct } from "../../../format";
  import type { Draft } from "../../../draft";
  import { limits } from "../../../limits.svelte";
  import Value from "../../../ui/Value.svelte";
  import NumberField from "../../../ui/NumberField.svelte";
  import { t } from "../../../i18n";

  interface Props {
    draft: Draft;
    preview: StatPreview | null;
  }

  let { draft, preview }: Props = $props();
  type LevelField = keyof SoulLinkStatus;
  type Row = {
    field: LevelField;
    label: string;
    max: number;
    value: () => string;
    motion: () => number | undefined;
  };
  const signed = (value: number | undefined) => value === undefined ? "—" : fmtSigned(value);
  const percent = (value: number | undefined) =>
    value === undefined ? "—" : fmtSignedPct(value, { max: 1 });

  const EQUIPMENT_ROWS: Row[] = [
    { field: "thrust_level", label: t("突き"), max: limits.soul_link_equipment_level_max, value: () => signed(preview?.soul_link.equipment_values.thrust), motion: () => preview?.soul_link.equipment_values.thrust },
    { field: "slash_level", label: t("斬り"), max: limits.soul_link_equipment_level_max, value: () => signed(preview?.soul_link.equipment_values.slash), motion: () => preview?.soul_link.equipment_values.slash },
    { field: "magic_attack_level", label: t("魔法攻撃"), max: limits.soul_link_equipment_level_max, value: () => signed(preview?.soul_link.equipment_values.magic_attack), motion: () => preview?.soul_link.equipment_values.magic_attack },
    { field: "magic_defense_level", label: t("魔法防御"), max: limits.soul_link_equipment_level_max, value: () => signed(preview?.soul_link.equipment_values.magic_defense), motion: () => preview?.soul_link.equipment_values.magic_defense },
  ];
  const DAMAGE_ROWS: Row[] = [
    {
      field: "critical_damage_level",
      label: t("クリダメ"),
      max: limits.soul_link_critical_damage_level_max,
      value: () => percent(preview?.soul_link.critical_damage_rate),
      motion: () => preview?.soul_link.critical_damage_rate,
    },
    {
      field: "final_damage_level",
      label: t("最終ダメ"),
      max: limits.soul_link_final_damage_level_max,
      value: () => percent(preview?.soul_link.final_damage_rate),
      motion: () => preview?.soul_link.final_damage_rate,
    },
    {
      field: "weapon_enhance_level",
      label: t("武器強化"),
      max: limits.soul_link_weapon_enhance_level_max,
      value: () => preview ? fmtRate(preview.soul_link.weapon_added_damage_multiplier, 1) : "—",
      motion: () => preview?.soul_link.weapon_added_damage_multiplier,
    },
  ];
  const HP_ROWS: Row[] = [
    {
      field: "armor_enhance_level",
      label: t("鎧強化HP"),
      max: limits.soul_link_armor_enhance_level_max,
      value: () => percent(preview?.soul_link.armor_added_hp_rate),
      motion: () => preview?.soul_link.armor_added_hp_rate,
    },
  ];
</script>

<div class="card">
  <p class="hint dim soul-note">
    {t("リンク枠は扱わず、習得条件は満たしている前提です。")}
  </p>
</div>

<div class="card">
  <div class="card-title">{t("装備値に加算")}</div>
  <div class="stat-rows two">
    {#each EQUIPMENT_ROWS as row (row.field)}
      <div class="stat-row">
        <span class="k">{row.label}</span>
        <NumberField
          label={t("{label}リンクステータス Lv", { label: row.label })}
          max={row.max}
          bind:value={draft.statSources.soul_link[row.field]}
        />
        <Value class="v" motion={() => row.motion() ?? null} value={row.value()} />
      </div>
    {/each}
  </div>
  <p class="hint dim">
    {t("エンチャントではなく装備の基本能力値へ直接加算します。")}
  </p>
</div>

<div class="card">
  <div class="card-title">{t("ダメージ式に反映")}</div>
  <div class="stat-rows two">
    {#each DAMAGE_ROWS as row (row.field)}
      <div class="stat-row">
        <span class="k">{row.label}</span>
        <NumberField
          label={t("{label}リンクステータス Lv", { label: row.label })}
          max={row.max}
          bind:value={draft.statSources.soul_link[row.field]}
        />
        <Value class="v" motion={() => row.motion() ?? null} value={row.value()} />
      </div>
    {/each}
  </div>
  <p class="hint dim">
    {t("クリダメはクリティカル時だけ、最終ダメージはカテゴリL上限45%まで反映。武器強化は算出済み追加固定ダメージへ掛けてから既存のヒット分割を行います。")}
  </p>
</div>

<div class="card">
  <div class="card-title">{t("HPに反映")}</div>
  <div class="stat-rows two">
    {#each HP_ROWS as row (row.field)}
      <div class="stat-row">
        <span class="k">{row.label}</span>
        <NumberField
          label={t("{label}リンクステータス Lv", { label: row.label })}
          max={row.max}
          bind:value={draft.statSources.soul_link[row.field]}
        />
        <Value class="v" motion={() => row.motion() ?? null} value={row.value()} />
      </div>
    {/each}
  </div>
  <p class="hint dim">
    {t("鎧強化は追加HPです。")}
  </p>
</div>

<style>
  .soul-note { margin-top: 0; }
  .stat-row .k { min-width: 76px; }
  .stat-row :global(.v) { min-width: 62px; }
</style>
