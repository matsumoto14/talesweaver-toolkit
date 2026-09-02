//! 「全コンテンツ×スキル」の評価(ホームの到達一覧・キャラレールのクリア数)を domain 側で
//! まとめる。単発計算(計算タブ)と同じキャラ由来の材料(`DamageMaterial`)を
//! 経由することで、両者の数値がズレない構造にする。
//!
//! gamedata のカタログ解決(スキル依存種別ごとの係数・装備アイテムの装着時効果・
//! 属性値の供給源など)は呼び出し側(commands.rs)が行い、ここへは解決済みの値
//! (`SkillEvaluationInput` 1 件 = 1 スキルぶん)として渡す。domain はそれを使って
//! コンテンツ×スキルのループと最大火力スキルの選定だけを行う。

use crate::awakening::Awakening;
use crate::content::{evaluate_content, BestSkillDamage, Content, ContentArea, ContentEvaluation};
use crate::damage::{
    calculate_damage, DamageContribution, DamageMaterial, DamageTarget, DependencyCoefficients,
};
use crate::enemy::Enemy;
use crate::equipment::{
    sum_equipment_value_sources, wrist_base_bonus, Equipment, EquipmentValueSource,
    EquipmentValues, WristBonusRule,
};
use crate::skill::{Skill, SkillDependency};
use crate::stats::BaseStats;
use crate::thesis_core::CoreRegion;
use crate::title::{title_added_damage_rate, title_attack_damage_rate, TitleDef};

/// 「全コンテンツ×スキル」評価ループの中で、スキル固有だがコンテンツには依存しない
/// 入力(依存種別の係数・カテゴリ寄与・属性値)。腕装備パッシブ込みの装備基本能力値は
/// `WristBonusMaterial` から評価関数が依存種別ごとに導くのでここには含めない。
/// 呼び出し側(commands.rs)が gamedata のカタログを解決して、キャラのスキル数ぶんだけ
/// 1 回作る(コンテンツの数だけ繰り返し計算しない)。
#[derive(Debug, Clone)]
pub struct SkillEvaluationInput {
    pub skill: Skill,
    pub coefficients: DependencyCoefficients,
    pub damage_contributions: Vec<DamageContribution>,
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
    /// 依存種別ごとの装備基本能力値の供給源(腕装備パッシブ込み)をあらかじめ全 6 種ぶん組み立てる。
    /// 腕装備パッシブは非 0 のときだけ「手首補正」という 1 供給源として追加する
    /// (「なぜこの数字?」パネルの装備攻撃力掘り下げに使う)。
    fn base_sources_by_dependency(
        &self,
        base_stats: &BaseStats,
        equipment_base_sources_raw: &[EquipmentValueSource],
    ) -> [(SkillDependency, Vec<EquipmentValueSource>); 6] {
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
            let mut sources = equipment_base_sources_raw.to_vec();
            if bonus != EquipmentValues::default() {
                sources.push(EquipmentValueSource {
                    source: "手首補正".to_string(),
                    values: bonus,
                });
            }
            (dependency, sources)
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
    equipment: &Equipment,
    content_areas: &[ContentArea],
    enemies: &[Enemy],
    skills: &[SkillEvaluationInput],
    equipment_base_sources_raw: Vec<EquipmentValueSource>,
    wrist_bonus: WristBonusMaterial,
    titles: &[TitleDef],
    awakening: Awakening,
    fixed_dependency: Option<SkillDependency>,
) -> Vec<ContentEvaluation> {
    let equipment_base_sources_by_dependency = wrist_bonus
        .base_sources_by_dependency(&material.base_stats, &equipment_base_sources_raw);
    let equipment_base_sources_for = |dependency: SkillDependency| {
        equipment_base_sources_by_dependency
            .iter()
            .find(|(d, _)| *d == dependency)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| equipment_base_sources_raw.clone())
    };

    let enhanced_by_region: Vec<(Option<CoreRegion>, Vec<EquipmentValueSource>)> =
        std::iter::once(None)
            .chain(CoreRegion::ALL.into_iter().map(Some))
            .map(|region| (region, equipment.enhanced_sources(region)))
            .collect();
    let enhanced_for = |region: Option<CoreRegion>| {
        enhanced_by_region
            .iter()
            .find(|(r, _)| *r == region)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    let title = equipment.title.as_deref();

    let mut evaluations = Vec::new();
    for area in content_areas {
        for content in &area.contents {
            evaluations.push(evaluate_one_content(
                content,
                material,
                equipment,
                enemies,
                skills,
                &equipment_base_sources_raw,
                &equipment_base_sources_for,
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

/// 入場条件の装備補正の判定値 = 基本能力値(依存別。腕変換込み)+強化能力値
/// (エンチャント・シエナのオーラ)。テシスコアの能力値増加は対象ダンジョン内
/// 限定のため含めない(`enhanced_for(None)` で除外。ユーザー確認 2026-08-29)。
fn entry_equipment_totals(
    dependency: Option<SkillDependency>,
    equipment_base_sources_raw: &[EquipmentValueSource],
    equipment_base_sources_for: &impl Fn(SkillDependency) -> Vec<EquipmentValueSource>,
    enhanced_for: &impl Fn(Option<CoreRegion>) -> Vec<EquipmentValueSource>,
) -> EquipmentValues {
    let base = dependency.map(equipment_base_sources_for).map_or_else(
        || sum_equipment_value_sources(equipment_base_sources_raw),
        |sources| sum_equipment_value_sources(&sources),
    );
    base.add(sum_equipment_value_sources(&enhanced_for(None)))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_one_content(
    content: &Content,
    material: &DamageMaterial,
    equipment: &Equipment,
    enemies: &[Enemy],
    skills: &[SkillEvaluationInput],
    equipment_base_sources_raw: &[EquipmentValueSource],
    equipment_base_sources_for: &impl Fn(SkillDependency) -> Vec<EquipmentValueSource>,
    enhanced_for: &impl Fn(Option<CoreRegion>) -> Vec<EquipmentValueSource>,
    title: Option<&str>,
    titles: &[TitleDef],
    awakening: Awakening,
    fixed_dependency: Option<SkillDependency>,
) -> ContentEvaluation {
    let thesis_core_total = equipment.thesis_cores.total_bonus(content.core_region);

    // 敵データが無いコンテンツ(入場条件のみ判定)は火力計算をしない。装備条件の
    // 比較先はキャラの代表スキル(一覧の先頭)の依存種別で決める。
    let Some(enemy_id) = content.enemy_id.as_deref() else {
        let dependency = fixed_dependency.or_else(|| skills.first().map(|s| s.skill.dependency));
        let equipment_entry_totals = entry_equipment_totals(
            dependency,
            equipment_base_sources_raw,
            equipment_base_sources_for,
            enhanced_for,
        );
        return evaluate_content(
            content,
            None,
            &equipment_entry_totals,
            awakening,
            dependency,
            thesis_core_total,
        );
    };

    let Some(enemy) = enemies.iter().find(|e| e.id == enemy_id) else {
        // データ整合性上、通常は発生しない(コンテンツの enemy_id は敵カタログに必ず
        // 存在する。gamedata のテストで担保)。万一のズレでも panic せず未判定として返す。
        let dependency = fixed_dependency.or_else(|| skills.first().map(|s| s.skill.dependency));
        let equipment_entry_totals = entry_equipment_totals(
            dependency,
            equipment_base_sources_raw,
            equipment_base_sources_for,
            enhanced_for,
        );
        return evaluate_content(
            content,
            None,
            &equipment_entry_totals,
            awakening,
            dependency,
            thesis_core_total,
        );
    };

    let equipment_enhanced_sources = enhanced_for(content.core_region);
    let title_damage_rate = title_attack_damage_rate(title, titles);
    let title_added_damage_rate = title_added_damage_rate(
        title,
        titles,
        content.game_region,
        content.enemy_id.as_deref(),
    );

    let mut best: Option<BestSkillDamage> = None;
    let mut best_dependency: Option<SkillDependency> = None;
    for entry in skills {
        let target = DamageTarget {
            skill: entry.skill.clone(),
            enemy: enemy.clone(),
            combo_count: 0,
            coefficients: entry.coefficients,
            equipment_base_sources: equipment_base_sources_for(entry.skill.dependency),
            equipment_enhanced_sources: equipment_enhanced_sources.clone(),
            title_attack_damage_rate: title_damage_rate,
            title_added_damage_rate,
            damage_contributions: entry.damage_contributions.clone(),
            element_value: entry.element_value,
        };
        let result = calculate_damage(material, &target);
        if best
            .as_ref()
            .is_none_or(|b| result.per_hit_primary > b.per_hit_primary)
        {
            best = Some(BestSkillDamage {
                skill_id: entry.skill.id.clone(),
                per_hit_primary: result.per_hit_primary,
                total_primary: result.total_primary,
            });
            // 装備条件の比較先は「判定に使ったスキル」の依存種別で決める
            best_dependency = Some(entry.skill.dependency);
        }
    }

    let requirement_dependency = fixed_dependency.or(best_dependency);
    let equipment_entry_totals = entry_equipment_totals(
        requirement_dependency,
        equipment_base_sources_raw,
        equipment_base_sources_for,
        enhanced_for,
    );
    evaluate_content(
        content,
        best,
        &equipment_entry_totals,
        awakening,
        requirement_dependency,
        thesis_core_total,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actual_delay::SkillUsesTable;
    use crate::attack_power::AttackCoefficients;
    use crate::character_skill::{CharacterSkillDef, CharacterSkills, SkillAudience, SkillEffect};
    use crate::common_skill::CommonSkills;
    use crate::critical_rate::CriticalRateSources;
    use crate::defense::{AccuracyBoost, AccuracyCorrection};
    use crate::element::Element;
    use crate::equipment::EquipmentCoefficients;
    use crate::mastery::Masteries;
    use crate::random_option::RandomOptionTotals;
    use crate::skill::{SkillDependency, SkillTarget};
    use crate::stat_sources::{build_modifiers, StatSources};
    use crate::stats::StatKind;
    use crate::thesis_core::CoreSetBonus;

    const STAB_DEF: &[StatKind] = &[StatKind::Stab, StatKind::Def];
    const ELITE_SWORDSMAN: &[SkillEffect] = &[SkillEffect::StatRate {
        stats: STAB_DEF,
        percent: 10.0,
        layer: crate::stat_sources::StatLayer::MultiplierB,
    }];

    /// マスタリーを取ってはじめて効果が出るキャラスキル(character_skill.rs のテストデータを
    /// 単純化したもの)。STAB/DEF +10%(倍率B)。
    const CATALOG: &[CharacterSkillDef] = &[CharacterSkillDef {
        id: "test_possession_swordsman",
        game_character_id: "test_char",
        name: "憑依【剣闘士】",
        audience: SkillAudience::SelfOnly,
        effects: &[],
        mastery_overrides: &[crate::character_skill::MasteryOverride {
            mastery_id: "test_m2_3",
            effects: ELITE_SWORDSMAN,
        }],
        source_url: "",
        note: "",
    }];

    fn skill() -> Skill {
        Skill {
            id: "s".into(),
            name: "テスト斬り".into(),
            dependency: SkillDependency::StabHack,
            multiplier: 0.99,
            hit_count: 1,
            critical_multiplier: 2.0,
            element: Element::Water,
            target: Some(SkillTarget::Single),
            accuracy: Some(92),
            critical_rate: Some(7),
            level: 1,
            single_target_channeling: false,
            base_actual_delay: Some(1.4),
            actual_delay_fixed: false,
            normal_attack: false,
            combo_interval: None,
            combo_variants: Vec::new(),
            power: Skill::compute_power(0.99, 1),
            power_per_second: Skill::compute_power_per_second(Skill::compute_power(0.99, 1), Some(1.4)),
        }
    }

    fn enemy() -> Enemy {
        Enemy {
            id: "e".into(),
            name: "テスト敵".into(),
            defense: 990,
            damage_reduction: 0,
            cut_rate_a: 1.0,
            element_threshold: 90,
            agi: None,
            critical_taken_rate: None,
        }
    }

    fn content_area() -> Vec<ContentArea> {
        vec![ContentArea {
            id: "area".into(),
            name: "テスト地域".into(),
            contents: vec![Content {
                id: "c".into(),
                name: "テスト".into(),
                series: None,
                enemy_id: Some("e".into()),
                need_per_hit: None,
                requirements: Vec::new(),
                core_region: None,
                game_region: None,
                entry_note: None,
                team_note: None,
            }],
        }]
    }

    fn coefficients() -> DependencyCoefficients {
        DependencyCoefficients {
            attack: AttackCoefficients {
                primary: (StatKind::Stab, 1.8),
                secondary: (StatKind::Hack, 1.8),
            },
            equipment: EquipmentCoefficients::default(),
            accuracy: AccuracyCorrection {
                bonus: None,
                penalty_primary: StatKind::Stab,
                penalty_secondary: Some(StatKind::Hack),
                penalty_divisor: 200.0,
            },
        }
    }

    fn material(apply_skill: bool) -> DamageMaterial {
        let stat_sources = StatSources::default();
        let (mut modifiers, mut contributions) =
            build_modifiers(&stat_sources, &crate::BuffSelection::default(), &[]).unwrap();
        if apply_skill {
            let skills = CharacterSkills {
                skill_ids: vec!["test_possession_swordsman".into()],
                skill_levels: Default::default(),
            };
            let masteries = Masteries {
                picked: vec!["test_m2_3".into()],
            };
            crate::stat_sources::apply_character_skills(
                &mut modifiers,
                &mut contributions,
                &skills,
                &masteries,
                CATALOG,
            );
        }
        DamageMaterial {
            base_stats: BaseStats {
                stab: 500,
                hack: 500,
                int: 0,
                def: 0,
                mr: 0,
                dex: 100,
                agi: 0,
            },
            stat_modifiers: modifiers,
            stat_contributions: contributions,
            common_skills: CommonSkills::default(),
            temporary_pins: None,
            siena_attack_rate: 0.0,
            siena_critical_rate: 0.0,
            siena_actual_delay_reduction: 0.0,
            core_set_bonus: CoreSetBonus::default(),
            accuracy_bonus: 0,
            accuracy_boost: AccuracyBoost::None,
            accuracy_shocked: false,
            random_options: RandomOptionTotals::default(),
            weapon_added_damage: 0,
            awakening_rate: 1.0,
            damage_cap: i64::MAX,
            stat_cap: i64::MAX,
            actual_delay_skills: Vec::new(),
            critical_rate_sources: CriticalRateSources::default(),
            skill_uses: SkillUsesTable {
                reduction_percents: Vec::new(),
                base_delays: Vec::new(),
                uses: Vec::new(),
            },
        }
    }

    fn evaluate(apply_skill: bool) -> Vec<ContentEvaluation> {
        let material = material(apply_skill);
        let skills = vec![SkillEvaluationInput {
            skill: skill(),
            coefficients: coefficients(),
            damage_contributions: Vec::new(),
            element_value: 0,
        }];
        evaluate_contents_for_character(
            &material,
            &Equipment::default(),
            &content_area(),
            &[enemy()],
            &skills,
            Vec::new(),
            WristBonusMaterial::default(),
            &[],
            Awakening::default(),
            None,
        )
    }

    /// ホームの全コンテンツ評価(`evaluate_contents_for_character`)は計算タブと同じ
    /// `DamageMaterial` を経由する。キャラスキルのステ補正(`apply_character_skills`)を
    /// 適用した material を渡せば、評価結果にもそのステ補正が反映されることを確認する
    /// (commands.rs 側で常に適用するようにした変更の回帰ガード)。
    #[test]
    fn キャラスキルのステ補正が全コンテンツ評価に反映される() {
        let without = evaluate(false);
        let with = evaluate(true);
        let dmg_without = without[0].damage.as_ref().unwrap().per_hit_primary;
        let dmg_with = with[0].damage.as_ref().unwrap().per_hit_primary;
        assert!(
            dmg_with > dmg_without,
            "ステ補正ありのほうが火力が高いはず: without={dmg_without}, with={dmg_with}"
        );
    }

    /// 入場条件の装備補正は基本+強化(エンチャント等)で判定し、テシスコアの
    /// 能力値増加(対象ダンジョン内限定)は含めない(ユーザー確認 2026-08-29)。
    /// コア除外は `enhanced_for(None)` を渡すことで表現される。
    #[test]
    fn 入場条件の判定値はエンチャントを含みテシスコアを含まない() {
        let base = vec![EquipmentValueSource {
            source: "基本".into(),
            values: EquipmentValues {
                slash: 100,
                ..Default::default()
            },
        }];
        let enhanced = |region: Option<CoreRegion>| match region {
            None => vec![EquipmentValueSource {
                source: "エンチャント".into(),
                values: EquipmentValues {
                    slash: 50,
                    ..Default::default()
                },
            }],
            Some(_) => vec![EquipmentValueSource {
                source: "コア込み(入場判定に使ってはいけない)".into(),
                values: EquipmentValues {
                    slash: 9_999,
                    ..Default::default()
                },
            }],
        };
        let totals = entry_equipment_totals(None, &base, &|_| base.clone(), &enhanced);
        assert_eq!(totals.slash, 150);
    }
}
