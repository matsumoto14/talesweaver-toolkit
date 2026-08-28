//! 「全コンテンツ×スキル」の評価(ホームの到達一覧・キャラレールのクリア数)を domain 側で
//! まとめる。単発計算(`DamageInput::new` を直接呼ぶ計算タブ)と共通の材料
//! (`DamageMaterial`)を経由することで、両者の数値がズレない構造にする。
//!
//! gamedata のカタログ解決(スキル依存種別ごとの係数・装備アイテムの装着時効果・
//! 属性値の供給源など)は呼び出し側(commands.rs)が行い、ここへは解決済みの値
//! (`SkillEvaluationInput` 1 件 = 1 スキルぶん)として渡す。domain はそれを使って
//! コンテンツ×スキルのループと最大火力スキルの選定だけを行う。

use serde::{Deserialize, Serialize};

use crate::actual_delay::{ActualDelayContribution, SkillUsesTable};
use crate::attack_power::AttackCoefficients;
use crate::awakening::Awakening;
use crate::calculate_damage;
use crate::category::DamageCategory;
use crate::common_skill::CommonSkills;
use crate::content::{evaluate_content, BestSkillDamage, Content, ContentArea, ContentEvaluation};
use crate::critical_rate::CriticalRateSources;
use crate::defense::AccuracyCorrection;
use crate::damage::DamageInput;
use crate::enemy::Enemy;
use crate::equipment::{wrist_base_bonus, Equipment, EquipmentCoefficients, EquipmentValues, WristBonusRule};
use crate::random_option::RandomOptionTotals;
use crate::skill::{Skill, SkillDependency};
use crate::stat_sources::{Adjustments, StatContribution};
use crate::stats::{BaseStats, StatModifierSet};
use crate::thesis_core::CoreRegion;
use crate::title::{title_added_damage_rate, title_attack_damage_rate, TitleDef};

/// スキル依存種別ごとに変わらない攻撃力/装備攻撃力/命中Pの係数(wiki: カテゴリA・
/// 計算式まとめ)。実データは gamedata が持つので、呼び出し側が解決して渡す。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DependencyCoefficients {
    pub attack: AttackCoefficients,
    pub equipment: EquipmentCoefficients,
    pub accuracy: AccuracyCorrection,
}

/// 与ダメージ計算のうち、スキル・敵・コンテンツによらない共通材料。
/// 計算タブの単発計算とホームの全コンテンツ評価が同じインスタンスの組み立て方
/// (呼び出し側で 1 回だけ構築)を通ることで、両者の数値がズレない。
#[derive(Debug, Clone)]
pub struct DamageMaterial {
    pub base_stats: BaseStats,
    pub stat_modifiers: StatModifierSet,
    pub stat_contributions: Vec<StatContribution>,
    pub equipment: Equipment,
    pub common_skills: CommonSkills,
    pub random_options: RandomOptionTotals,
    pub weapon_added_damage: i64,
    pub awakening_rate: f64,
    pub damage_cap: i64,
    pub stat_cap: i64,
    pub pins: Adjustments,
    pub actual_delay_skills: Vec<ActualDelayContribution>,
    pub critical_rate_sources: CriticalRateSources,
    pub skill_uses: SkillUsesTable,
}

impl DamageMaterial {
    /// スキル・敵ごとに変わる残りの値を渡して `DamageInput` を組み立てる。
    #[allow(clippy::too_many_arguments)]
    pub fn build_input(
        &self,
        skill: Skill,
        enemy: Enemy,
        combo_count: u32,
        temporary_pins: Option<Adjustments>,
        coefficients: DependencyCoefficients,
        equipment_base_totals: EquipmentValues,
        equipment_enhanced_totals: EquipmentValues,
        title_damage_rate: f64,
        title_added_damage_rate: f64,
        damage_contributions: Vec<(DamageCategory, f64)>,
        element_value: i64,
    ) -> DamageInput {
        DamageInput::new(
            self.base_stats.clone(),
            self.stat_modifiers.clone(),
            self.stat_contributions.clone(),
            coefficients.attack,
            self.equipment.clone(),
            self.common_skills,
            equipment_base_totals,
            equipment_enhanced_totals,
            coefficients.equipment,
            coefficients.accuracy,
            self.random_options,
            title_damage_rate,
            title_added_damage_rate,
            damage_contributions,
            self.weapon_added_damage,
            self.awakening_rate,
            self.damage_cap,
            self.stat_cap,
            skill,
            enemy,
            combo_count,
            element_value,
            self.pins.clone(),
            temporary_pins,
            self.actual_delay_skills.clone(),
            self.critical_rate_sources,
            self.skill_uses.clone(),
        )
    }
}

/// 「全コンテンツ×スキル」評価ループの中で、スキル固有だがコンテンツには依存しない
/// 入力(依存種別の係数・カテゴリ寄与・属性値)。腕装備パッシブ込みの装備基本能力値は
/// `WristBonusMaterial` から評価関数が依存種別ごとに導くのでここには含めない。
/// 呼び出し側(commands.rs)が gamedata のカタログを解決して、キャラのスキル数ぶんだけ
/// 1 回作る(コンテンツの数だけ繰り返し計算しない)。
#[derive(Debug, Clone)]
pub struct SkillEvaluationInput {
    pub skill: Skill,
    pub coefficients: DependencyCoefficients,
    pub damage_contributions: Vec<(DamageCategory, f64)>,
    pub element_value: i64,
}

/// 腕装備パッシブ(`WristBonusRule`)を適用するための材料。カタログ解決
/// (`WristType` がバンドかどうか)は呼び出し側(gamedata)が行う。
#[derive(Debug, Clone, Copy, Default)]
pub struct WristBonusMaterial {
    pub rule: Option<WristBonusRule>,
    pub is_band: bool,
    pub wrist_totals: EquipmentValues,
    pub siena_thrust: i64,
    /// キャラの主軸スキル(`main_skill_id`)の依存種別。`Some` なら、振り先がスキル依存で
    /// 変わるルール(ナヤトレイ・イサック)は評価中のスキルの依存種別によらず**常にこちら**を
    /// 使う(装備条件の判定は「主軸で戦う前提」のため)。`None` なら評価中の依存種別を使う。
    pub style_dependency_override: Option<SkillDependency>,
}

impl WristBonusMaterial {
    /// 依存種別ごとの装備基本能力値(腕装備パッシブ込み)をあらかじめ全 6 種ぶん組み立てる。
    fn base_totals_by_dependency(
        &self,
        base_stats: &BaseStats,
        equipment_base_totals_raw: EquipmentValues,
    ) -> [(SkillDependency, EquipmentValues); 6] {
        SkillDependency::ALL.map(|dependency| {
            let style_dependency = self.style_dependency_override.unwrap_or(dependency);
            let bonus = wrist_base_bonus(
                self.rule,
                self.is_band,
                base_stats,
                style_dependency,
                self.wrist_totals,
                self.siena_thrust,
            );
            (dependency, equipment_base_totals_raw.add(bonus))
        })
    }
}

/// 全コンテンツ×スキルを評価し、コンテンツごとに最大火力スキルと判定結果を返す
/// (`evaluate_contents` コマンドの本体)。
///
/// - 敵データが無いコンテンツは火力を判定せず、装備条件だけを `skills` の先頭スキルの
///   依存種別(`fixed_dependency` があればそちらを優先)で判定する。
/// - 敵データがあるコンテンツは `skills` の中から 1 ヒット(最大)与ダメージが最大の
///   スキルを選び、そのスキルの依存種別で装備条件を判定する。
/// - `fixed_dependency` は呼び出し側が指定したスキルの依存種別(計算タブのように
///   「今このスキルで戦う」文脈)。`None` ならコンテンツごとに上記の規則で決める。
///
/// ループ不変値(地域ごとの強化能力値・依存種別ごとの装備基本能力値)は呼び出し前に
/// 1 回だけ構築し、コンテンツの数だけ再計算しない(最重量パスでの無駄な再計算を避ける)。
#[allow(clippy::too_many_arguments)]
pub fn evaluate_contents_for_character(
    material: &DamageMaterial,
    content_areas: &[ContentArea],
    enemies: &[Enemy],
    skills: &[SkillEvaluationInput],
    equipment_base_totals_raw: EquipmentValues,
    wrist_bonus: WristBonusMaterial,
    titles: &[TitleDef],
    awakening: Awakening,
    fixed_dependency: Option<SkillDependency>,
) -> Vec<ContentEvaluation> {
    let equipment_base_totals_by_dependency =
        wrist_bonus.base_totals_by_dependency(&material.base_stats, equipment_base_totals_raw);
    let equipment_base_totals_for = |dependency: SkillDependency| {
        equipment_base_totals_by_dependency
            .iter()
            .find(|(d, _)| *d == dependency)
            .map(|(_, v)| *v)
            .unwrap_or(equipment_base_totals_raw)
    };

    let enhanced_by_region: Vec<(Option<CoreRegion>, EquipmentValues)> = std::iter::once(None)
        .chain(CoreRegion::ALL.into_iter().map(Some))
        .map(|region| (region, material.equipment.enhanced_totals(region)))
        .collect();
    let enhanced_for = |region: Option<CoreRegion>| {
        enhanced_by_region
            .iter()
            .find(|(r, _)| *r == region)
            .map(|(_, v)| *v)
            .unwrap_or_default()
    };

    let title = material.equipment.title.as_deref();

    let mut evaluations = Vec::new();
    for area in content_areas {
        for content in &area.contents {
            evaluations.push(evaluate_one_content(
                content,
                material,
                enemies,
                skills,
                equipment_base_totals_raw,
                &equipment_base_totals_for,
                &enhanced_for,
                title,
                titles,
                awakening,
                fixed_dependency,
            ));
        }
    }
    evaluations
}

#[allow(clippy::too_many_arguments)]
fn evaluate_one_content(
    content: &Content,
    material: &DamageMaterial,
    enemies: &[Enemy],
    skills: &[SkillEvaluationInput],
    equipment_base_totals_raw: EquipmentValues,
    equipment_base_totals_for: &impl Fn(SkillDependency) -> EquipmentValues,
    enhanced_for: &impl Fn(Option<CoreRegion>) -> EquipmentValues,
    title: Option<&str>,
    titles: &[TitleDef],
    awakening: Awakening,
    fixed_dependency: Option<SkillDependency>,
) -> ContentEvaluation {
    let thesis_core_total = material.equipment.thesis_cores.total_bonus(content.core_region);

    // 敵データが無いコンテンツ(入場条件のみ判定)は火力計算をしない。装備条件の
    // 比較先はキャラの代表スキル(一覧の先頭)の依存種別で決める。
    let Some(enemy_id) = content.enemy_id.as_deref() else {
        let dependency = fixed_dependency.or_else(|| skills.first().map(|s| s.skill.dependency));
        let equipment_base_totals =
            dependency.map(equipment_base_totals_for).unwrap_or(equipment_base_totals_raw);
        return evaluate_content(content, None, &equipment_base_totals, awakening, dependency, thesis_core_total);
    };

    let Some(enemy) = enemies.iter().find(|e| e.id == enemy_id) else {
        // データ整合性上、通常は発生しない(コンテンツの enemy_id は敵カタログに必ず
        // 存在する。gamedata のテストで担保)。万一のズレでも panic せず未判定として返す。
        let dependency = fixed_dependency.or_else(|| skills.first().map(|s| s.skill.dependency));
        let equipment_base_totals =
            dependency.map(equipment_base_totals_for).unwrap_or(equipment_base_totals_raw);
        return evaluate_content(content, None, &equipment_base_totals, awakening, dependency, thesis_core_total);
    };

    let equipment_enhanced_totals = enhanced_for(content.core_region);
    let title_damage_rate = title_attack_damage_rate(title, titles);
    let title_added_damage_rate =
        title_added_damage_rate(title, titles, content.game_region, content.enemy_id.as_deref());

    let mut best: Option<BestSkillDamage> = None;
    let mut best_dependency: Option<SkillDependency> = None;
    for entry in skills {
        let input = material.build_input(
            entry.skill.clone(),
            enemy.clone(),
            0,
            None,
            entry.coefficients,
            equipment_base_totals_for(entry.skill.dependency),
            equipment_enhanced_totals,
            title_damage_rate,
            title_added_damage_rate,
            entry.damage_contributions.clone(),
            entry.element_value,
        );
        let result = calculate_damage(&input);
        if best.as_ref().is_none_or(|b| result.per_hit.max > b.per_hit_max) {
            best = Some(BestSkillDamage {
                skill_id: entry.skill.id.clone(),
                per_hit_max: result.per_hit.max,
                total_max: result.total.max,
            });
            // 装備条件の比較先は「判定に使ったスキル」の依存種別で決める
            best_dependency = Some(entry.skill.dependency);
        }
    }

    let requirement_dependency = fixed_dependency.or(best_dependency);
    let equipment_base_totals =
        requirement_dependency.map(equipment_base_totals_for).unwrap_or(equipment_base_totals_raw);
    evaluate_content(
        content,
        best,
        &equipment_base_totals,
        awakening,
        requirement_dependency,
        thesis_core_total,
    )
}
