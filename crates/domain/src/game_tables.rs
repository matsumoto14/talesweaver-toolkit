//! 画面が配列リテラルで持たないためのカタログ(並び・ラベル・部位ルール・段階表)。
//!
//! `StatLimits` は「数値の上限・刻み・既定値・係数」だけを持つ。ここはそれ以外
//! ——`xxx_labels` / 部位ごとの枠数ルール / enum の並び / 段階 → 値の表——をまとめ、
//! `get_game_tables` で起動時に 1 回だけ配る。

use serde::{Deserialize, Serialize};

use crate::category::DamageCategory;
use crate::equipment::{EquipmentStatKind, PartSlot, PartSlotRule};
use crate::stat_sources::{
    pet_skill_tier_bonuses, sacred_relic_value, PetSkillTierBonus, StatSourceGroup,
    SACRED_RELIC_STAGE_MAX,
};
use crate::stats::StatKind;
use crate::thesis_core::{POWER_BONUS, SUPPORT_BONUS};

/// `DamageCategory` 1 件の表示名。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageCategoryLabel {
    pub category: DamageCategory,
    pub label: String,
}

/// 装備補正 1 値の表示名。`kind` は serde のフィールド名(`EquipmentStatKind::key`)と一致する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentStatLabel {
    pub kind: String,
    pub label: String,
}

/// 並び・ラベル・段階表のカタログ(起動時に 1 回取得する想定)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameTables {
    /// 部位ごとの枠数ルール(装着アビリティ・ランダムオプション)。13 部位ぶん
    pub part_slot_rules: Vec<PartSlotRule>,
    // --- enum の並びと分類。画面は配列リテラルを持たず、ここを読んで並べる ---
    /// ステの並び(`StatKind::ALL`)
    pub stat_kinds: Vec<StatKind>,
    /// 補正の出どころの並び(`StatSourceGroup::ALL`)
    pub stat_source_groups: Vec<StatSourceGroup>,
    /// 属性の並び(`Element::ALL`)
    pub elements: Vec<crate::element::Element>,
    /// 装備に付与できる属性(`Element::can_enchant_equipment`)
    pub equipment_elements: Vec<crate::element::Element>,
    /// 装着アビリティの系統の並び(`EquipmentAbilityFamily::ALL`)
    pub ability_families: Vec<crate::equipment::EquipmentAbilityFamily>,
    /// ランダムオプションのランクの並び(`RandomOptionRank::ALL`。左ほど下位)
    pub random_option_ranks: Vec<crate::random_option::RandomOptionRank>,
    /// スキル依存種別の並び(`SkillDependency::ALL`)
    pub skill_dependencies: Vec<crate::skill::SkillDependency>,
    /// 極限スキルの並び(`UltimateSkill::ALL`)
    pub ultimate_skills: Vec<crate::ultimate_skill::UltimateSkill>,
    /// テシスコアの地域の並び(`CoreRegion::ALL`)
    pub core_regions: Vec<crate::thesis_core::CoreRegion>,
    /// テシスコアの火力タイプ(`CoreType::is_power`)。強化能力値に入る
    pub core_power_types: Vec<crate::thesis_core::CoreType>,
    /// テシスコアの補助タイプ。記録と入場条件の合計にだけ効く
    pub core_support_types: Vec<crate::thesis_core::CoreType>,
    /// 装備強化 Lv の選択肢(0 = 強化なし、実用は +10 以上。+12 以上は等級つき)
    pub enhance_level_candidates: Vec<u8>,
    // --- 段階(Lv)→ 値の表。画面は換算式を写経しない ---
    /// プロテクトアーマー Lv1〜6 の装備防御力倍率(物理 / 魔法)。Σ% の小数表現
    pub protect_armor_physical_rates: Vec<f64>,
    pub protect_armor_magic_rates: Vec<f64>,
    /// 改・プロテクトアーマー Lv1〜5 の装備防御力倍率(物理 / 魔法)。Σ% の小数表現
    pub kai_protect_armor_physical_rates: Vec<f64>,
    pub kai_protect_armor_magic_rates: Vec<f64>,
    /// シャープネスビジョン Lv1〜10 の割合追加ダメージ。Σ% の小数表現
    pub sharpness_vision_rates: Vec<f64>,
    /// アンリーシュ Lv1〜10 の能力値倍率B。Σ% の小数表現
    pub unleash_rates: Vec<f64>,
    /// ペット S スキルの段階ごとの固定値ボーナス(wiki: PET)
    pub pet_skill_tier_bonus: Vec<PetSkillTierBonus>,
    /// 神鳥の聖物の段階 → 最終固定値(添字が段階。0..=`sacred_relic_stage_max`)
    pub sacred_relic_stage_values: Vec<i64>,
    /// テシスコア・火力タイプの補正値テーブル(wiki: 進化強化表「火力」列)。添字は [進化段階][強化段階]
    pub core_power_bonus_table: Vec<Vec<i64>>,
    /// テシスコア・補助タイプの補正値テーブル(wiki: 進化強化表「補助」列)
    pub core_support_bonus_table: Vec<Vec<i64>>,
    // --- 表示名 ---
    /// 与ダメージ式カテゴリ(`DamageCategory`)の日本語名。`DamageCategory::ALL` の順
    pub damage_category_labels: Vec<DamageCategoryLabel>,
    /// 装備補正 9 値(`EquipmentValues`)の表示名。`EquipmentStatKind::ALL` の順。
    /// `CoreType`(テシスコア)の表示名もここと同じ(8 種が重なる。critical は含まない)
    pub equipment_stat_labels: Vec<EquipmentStatLabel>,
}

pub fn game_tables() -> GameTables {
    GameTables {
        part_slot_rules: PartSlot::ALL
            .into_iter()
            .map(|slot| PartSlotRule {
                slot,
                label: slot.label().to_string(),
                ability_slots: slot.ability_slots(),
                allows_ability: slot.allows_abilities(),
                allows_enhance: slot.allows_enhance(),
                allows_enchant: slot.allows_enchant(),
                allows_siena: slot.allows_siena(),
                siena_counts_as_equipment: slot.siena_values_are_equipment(),
                allows_random_option: slot.allows_random_option(),
                random_option_slots: slot.random_option_slots(),
                allows_element: slot.allows_element(),
            })
            .collect(),
        stat_kinds: StatKind::ALL.to_vec(),
        stat_source_groups: StatSourceGroup::ALL.to_vec(),
        elements: crate::element::Element::ALL.to_vec(),
        equipment_elements: crate::element::Element::ALL
            .into_iter()
            .filter(|e| e.can_enchant_equipment())
            .collect(),
        ability_families: crate::equipment::EquipmentAbilityFamily::ALL.to_vec(),
        random_option_ranks: crate::random_option::RandomOptionRank::ALL.to_vec(),
        skill_dependencies: crate::skill::SkillDependency::ALL.to_vec(),
        ultimate_skills: crate::ultimate_skill::UltimateSkill::ALL.to_vec(),
        core_regions: crate::thesis_core::CoreRegion::ALL.to_vec(),
        core_power_types: crate::thesis_core::CoreType::ALL
            .into_iter()
            .filter(|t| t.is_power())
            .collect(),
        core_support_types: crate::thesis_core::CoreType::ALL
            .into_iter()
            .filter(|t| !t.is_power())
            .collect(),
        enhance_level_candidates: crate::equipment::ENHANCE_LEVEL_CANDIDATES.to_vec(),
        protect_armor_physical_rates: crate::common_skill::PROTECT_ARMOR_PHYSICAL.to_vec(),
        protect_armor_magic_rates: crate::common_skill::PROTECT_ARMOR_MAGIC.to_vec(),
        kai_protect_armor_physical_rates: crate::common_skill::KAI_PROTECT_ARMOR_PHYSICAL.to_vec(),
        kai_protect_armor_magic_rates: crate::common_skill::KAI_PROTECT_ARMOR_MAGIC.to_vec(),
        sharpness_vision_rates: crate::common_skill::SHARPNESS_VISION.to_vec(),
        unleash_rates: crate::common_skill::UNLEASH.to_vec(),
        pet_skill_tier_bonus: pet_skill_tier_bonuses(),
        sacred_relic_stage_values: (0..=SACRED_RELIC_STAGE_MAX)
            .map(sacred_relic_value)
            .collect(),
        core_power_bonus_table: POWER_BONUS.iter().map(|row| row.to_vec()).collect(),
        core_support_bonus_table: SUPPORT_BONUS.iter().map(|row| row.to_vec()).collect(),
        damage_category_labels: DamageCategory::ALL
            .into_iter()
            .map(|category| DamageCategoryLabel {
                category,
                label: category.label().to_string(),
            })
            .collect(),
        equipment_stat_labels: EquipmentStatKind::ALL
            .into_iter()
            .map(|kind| EquipmentStatLabel {
                kind: kind.key().to_string(),
                label: kind.label().to_string(),
            })
            .collect(),
    }
}
