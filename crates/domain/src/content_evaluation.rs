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
    apply_fixed_interval_dps, apply_summon_interval, calculate_damage, combine_expected_dps,
    defeat_seconds, DamageContribution, DamageMaterial, DamageResult, DamageTarget,
    DependencyCoefficients,
};
use crate::rotation::{
    choose_rotation, plan_rotation, rotation_dps, RotationCandidate, RotationDamage,
    RotationInsert, RotationRole,
};
use crate::enemy::Enemy;
use crate::equipment::{
    sum_equipment_value_sources, Equipment, EquipmentBaseContext, EquipmentValueSource,
    EquipmentValues,
};
use crate::skill::{Skill, SkillDependency};
use crate::thesis_core::CoreRegion;
use crate::title::{title_added_damage_rate, title_attack_damage_rate, TitleDef};

/// 「全コンテンツ×スキル」評価ループの中で、スキル固有だがコンテンツには依存しない
/// 入力(依存種別の係数・カテゴリ寄与・属性値)。腕装備パッシブ込みの装備基本能力値は
/// `EquipmentBaseContext` から評価関数が依存種別ごとに導くのでここには含めない。
/// 呼び出し側(commands.rs)が gamedata のカタログを解決して、キャラのスキル数ぶんだけ
/// 1 回作る(コンテンツの数だけ繰り返し計算しない)。
///
/// 熊(魔法人形)ぶんの入力(`evaluate_contents_for_character` の `summon` 引数)も同じ形
/// (1 件だけ)なので、この型をそのまま使う(二重実装しない)。
#[derive(Debug, Clone)]
pub struct SkillEvaluationInput {
    pub skill: Skill,
    pub coefficients: DependencyCoefficients,
    pub damage_contributions: Vec<DamageContribution>,
    /// キャラスキルの割合追加ダメージ(§5 新-割合)。形態で絞るスキルはこのスキルで
    /// 解決済み(`CharacterSkills::added_damage_rate`)
    pub skill_added_damage_rate: f64,
    pub element_value: i64,
    /// <フラグ>(イェフネンの、技とは別枠のダメージ)の持続ぶんの入力。
    /// **技ごとに付く** — <フラグ> の技データは積んだ技から命中・Cri値を引き継ぐため。
    /// 積んでいないキャラ・スタック 0 は `None`
    pub flag: Option<FlagEvaluationInput>,
    /// この技を主軸にしたときの回し(連打する技 + 差し込む CT 技)。
    /// 差し込む技が無い(CT 技を持たないキャラ)なら `None` で、DPS は技そのものの値
    pub rotation: Option<RotationEvaluationInput>,
}

/// <フラグ> の持続ぶんの入力(`SkillEvaluationInput` にぶら下がる)。
/// 1 秒ごとに入るので、回しとは無関係に周期で割った DPS を足す。
#[derive(Debug, Clone)]
pub struct FlagEvaluationInput {
    /// 持続 1 回ぶん(`flag` / `rotation` は常に `None`。入れ子にしない)
    pub duration: Box<SkillEvaluationInput>,
    /// 持続ダメージの周期(秒)
    pub tick_seconds: f64,
}

/// 回しぶんの入力。計算タブ(`commands::build_rotation`)と同じ材料を、カタログ解決済みで
/// 受け取る。どれを連打してどれを差し込むかは `rotation::choose_rotation` が決めるので、
/// ここでは**候補を並べるだけ**(役割を呼び出し側で決めない)。
#[derive(Debug, Clone)]
pub struct RotationEvaluationInput {
    /// 同じキャラ・同じ形態のプレイヤー攻撃技(主軸は含めない)
    pub candidates: Vec<RotationCandidateEvaluationInput>,
    /// <フラグ> の積み直しで連打技が決まっているならその技の id(同形態の 連 / 爆)
    pub pinned_filler_id: Option<String>,
    /// 主軸自身を差し込むときに付いてくるもの
    pub main: RotationInsertMaterial,
    /// キャラに保存された「差し込む CT 技」(`NewCharacter::rotation_skill_ids`)。
    /// `None` = 既定、`Some([])` = 差し込まない、`Some([id, …])` = 明示指定
    pub insert_skill_ids: Option<Vec<String>>,
}

/// 回しの候補 1 件。
#[derive(Debug, Clone)]
pub struct RotationCandidateEvaluationInput {
    pub skill: Box<SkillEvaluationInput>,
    /// 主軸を連打するときに差し込んでよいか(<フラグ> を爆発させる技は、主軸で積んだ
    /// ぶんを使う技だけ `true`)
    pub insertable: bool,
    pub material: RotationInsertMaterial,
}

/// 差し込んだときに付いてくるもの(<フラグ> の爆発と積み直しの回数)。
#[derive(Debug, Clone, Default)]
pub struct RotationInsertMaterial {
    /// この技が起こす <フラグ> の爆発 1 回ぶん
    pub burst: Option<Box<SkillEvaluationInput>>,
    /// 撃つ前に連打技を最低何回挟むか(<フラグ> の積み直し)
    pub minimum_filler_uses: u32,
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
    // 熊(魔法人形)ぶんの入力。魔法人形を持たない・召喚スキル未選択のキャラは `None`
    // (本体だけで判定する、従来どおりの動き)
    summon: Option<&SkillEvaluationInput>,
    equipment_base: EquipmentBaseContext<'_>,
    titles: &[TitleDef],
    awakening: Awakening,
    fixed_dependency: Option<SkillDependency>,
) -> Vec<ContentEvaluation> {
    // 依存種別ごとの供給源(手首補正の振り先が依存で変わるキャラがいる)を先に全 6 種ぶん
    // 組み立てる。主軸スキルが決まっているキャラ(`style_dependency`)は、評価中のスキルに
    // よらず常に主軸で振る(装備条件の判定は「主軸で戦う前提」のため)。
    let equipment_base_sources_by_dependency: [(SkillDependency, Vec<EquipmentValueSource>); 6] =
        SkillDependency::ALL.map(|dependency| {
            let style = equipment_base.style_dependency.or(Some(dependency));
            (
                dependency,
                equipment_base.for_dependency(style).sources(equipment),
            )
        });
    // 依存種別が決まらない場面(敵データが無く、スキルも無いコンテンツ)のぶん
    let equipment_base_sources_raw = equipment_base.sources(equipment);
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
    // 全地域(None 込み)ぶん組み立て済みなので必ず見つかる
    let enhanced_for = |region: Option<CoreRegion>| {
        enhanced_by_region
            .iter()
            .find(|(r, _)| *r == region)
            .map(|(_, v)| v.clone())
            .expect("地域ごとの強化能力値は全地域ぶん組み立て済み")
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
                summon,
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
    summon: Option<&SkillEvaluationInput>,
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
    // (コンテンツの enemy_id が敵カタログに無いのは gamedata のテストで弾いているので
    // 通常は起きないが、万一のズレでも panic せず同じ「未判定」で返す)
    let enemy = content
        .enemy_id
        .as_deref()
        .and_then(|enemy_id| enemies.iter().find(|e| e.id == enemy_id));
    let Some(enemy) = enemy else {
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

    // 1 件ぶんの計算対象。攻撃者(本体 / 召喚獣 / <フラグ>)で変わるのは入力だけなので、
    // 的(敵・称号・装備)の組み立てはここ 1 か所にまとめる
    let to_target = |entry: &SkillEvaluationInput| DamageTarget {
        skill: entry.skill.clone(),
        enemy: enemy.clone(),
        combo_count: 0,
        coefficients: entry.coefficients,
        equipment_base_sources: equipment_base_sources_for(entry.skill.dependency),
        equipment_enhanced_sources: equipment_enhanced_sources.clone(),
        title_attack_damage_rate: title_damage_rate,
        title_added_damage_rate,
        damage_contributions: entry.damage_contributions.clone(),
        skill_added_damage_rate: entry.skill_added_damage_rate,
        element_value: entry.element_value,
    };

    let mut best: Option<BestSkillDamage> = None;
    let mut best_dependency: Option<SkillDependency> = None;
    // 最良スキルの入力と結果(<フラグ> の 1 周を組むのに、主軸 1 回ぶんの火力と時間が要る)
    let mut best_entry: Option<(&SkillEvaluationInput, DamageResult)> = None;
    for entry in skills {
        let result = calculate_damage(material, &to_target(entry));
        if best
            .as_ref()
            .is_none_or(|b| result.per_hit_primary > b.per_hit_primary)
        {
            best = Some(BestSkillDamage {
                skill_id: entry.skill.id.clone(),
                per_hit_primary: result.per_hit_primary,
                total_primary: result.total_primary,
                defeat_seconds: result.defeat_seconds,
                expected_dps: result.expected_dps,
            });
            // 装備条件の比較先は「判定に使ったスキル」の依存種別で決める
            best_dependency = Some(entry.skill.dependency);
            best_entry = Some((entry, result));
        }
    }

    // 本体以外(召喚獣・<フラグ>)の期待 DPS を足し、討伐時間を出し直す。
    // 足す先は 1 か所にまとめる(2 つが同時に成立しても片方の合算が捨てられない)。
    if let Some(b) = best.as_mut() {
        let mut combined = b.expected_dps;

        // 回し(連打する技 + 差し込む CT 技)。計算タブ(`commands::combine_damage`)と
        // 同じ規則で技の期待 DPS を**置き換え**、<フラグ> の持続はそれに足す。
        // 技の DPS が出せないときは何も足さない
        if let Some((entry, main_result)) = best_entry.as_ref() {
            if combined.is_some() {
                if let Some(rotation) = entry.rotation.as_ref() {
                    match rotation_expected_dps(material, &to_target, rotation, entry, main_result)
                    {
                        Some(expected) => combined = Some(expected),
                        // 回しを組めないのに <フラグ> の爆発がある技は、爆発をどの間隔で
                        // 入れるか決まらない。爆発を無視した確定値を出さない
                        // (計算タブの `commands::combine_damage` と同じ規則)
                        None if rotation.main.burst.is_some() => combined = None,
                        None => {}
                    }
                }
                if let Some(flag) = entry.flag.as_ref() {
                    let mut duration = calculate_damage(material, &to_target(&flag.duration));
                    apply_fixed_interval_dps(&mut duration, flag.tick_seconds);
                    combined = combine_expected_dps(combined, duration.expected_dps);
                }
            }
        }

        // 召喚獣(熊・破壊精霊)。wiki 計算式まとめ `STAB(熊)` 行。本体は召喚中も自由に
        // 撃てるので単純和。コンボボーナスは乗らず(combo_count = 0)、実測回数表は本体
        // プレイヤーの実測なので使わず `summon_uses_per_minute` の式で回数を出す
        if let Some(summon_input) = summon {
            let mut summon_result = calculate_damage(material, &to_target(summon_input));
            apply_summon_interval(&mut summon_result, enemy.hp);
            combined = combine_expected_dps(combined, summon_result.expected_dps);
        }

        if combined != b.expected_dps {
            b.defeat_seconds = defeat_seconds(enemy.hp, combined);
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

/// 回しの期待 DPS(計算タブの `commands::build_rotation` + `combine_damage` と同じ規則)。
/// 候補も連打技も差し込む技も、主軸とまったく同じ的(`to_target`)で 1 回ぶんを計算し、
/// 役割は `rotation::choose_rotation`(実際の 1 回の所要時間が基準)で決める。
/// 回しを組めないなら `None`(呼び出し側は技そのものの DPS のまま)。
fn rotation_expected_dps(
    material: &DamageMaterial,
    to_target: &impl Fn(&SkillEvaluationInput) -> DamageTarget,
    rotation: &RotationEvaluationInput,
    main: &SkillEvaluationInput,
    main_result: &DamageResult,
) -> Option<f64> {
    // 役割の判定は実際の所要時間が基準なので、候補も 1 回ぶんを計算する
    let results: Vec<DamageResult> = rotation
        .candidates
        .iter()
        .map(|candidate| calculate_damage(material, &to_target(&candidate.skill)))
        .collect();
    // 1 回で同時に入るダメージ(技本体 + それが起こす <フラグ> の爆発)。
    // 差し込むと上がるかを `choose_rotation` がその場で確かめるので、候補ぶんも先に作る
    let candidate_damages: Vec<Vec<RotationDamage>> = rotation
        .candidates
        .iter()
        .zip(&results)
        .map(|(candidate, result)| {
            let mut part = vec![RotationDamage::of(result)];
            if let Some(burst) = candidate.material.burst.as_ref() {
                part.push(RotationDamage::of(&calculate_damage(material, &to_target(burst))));
            }
            part
        })
        .collect();
    let candidates: Vec<RotationCandidate<'_>> = rotation
        .candidates
        .iter()
        .zip(&results)
        .zip(&candidate_damages)
        .map(|((candidate, result), damage)| RotationCandidate {
            skill: &candidate.skill.skill,
            seconds: result.cycle_seconds(),
            insertable: candidate.insertable,
            damage,
            minimum_filler_uses: candidate.material.minimum_filler_uses,
        })
        .collect();
    let roles = choose_rotation(
        &main.skill,
        main_result.cycle_seconds(),
        Some(RotationDamage::of(main_result)),
        &candidates,
        rotation.pinned_filler_id.as_deref(),
    );
    // 主軸を**連打**しながら <フラグ> を爆発させる回しは、爆発をどの間隔で入れるか決まらない
    // (爆発は主軸 1 回につき 1 度だが、連打の回数は時間配分の結果として決まる)。
    // ADR-019 決定 8 と同じく、爆発を黙って落とした「確定値」を出さず DPS を不明にする
    // (計算タブの `commands::build_rotation` と同じ規則)
    if rotation.main.burst.is_some() && roles.filler == Some(RotationRole::Main) {
        return None;
    }
    // 画面から明示された差し込み(キャラに保存した「差し込む CT 技」)は計算タブと同じ 1 本で当てる
    let candidate_ids: Vec<&str> = rotation
        .candidates
        .iter()
        .map(|candidate| candidate.skill.skill.id.as_str())
        .collect();
    let insert_roles = crate::rotation::apply_explicit_inserts(
        &roles,
        rotation.insert_skill_ids.as_deref(),
        &main.skill.id,
        &candidate_ids,
    );
    let result_of = |role: RotationRole| match role {
        RotationRole::Main => main_result,
        RotationRole::Candidate(index) => &results[index],
    };
    let skill_of = |role: RotationRole| match role {
        RotationRole::Main => &main.skill,
        RotationRole::Candidate(index) => &rotation.candidates[index].skill.skill,
    };
    let material_of = |role: RotationRole| match role {
        RotationRole::Main => &rotation.main,
        RotationRole::Candidate(index) => &rotation.candidates[index].material,
    };
    let filler = roles
        .filler
        .map(result_of)
        .and_then(|result| Some((RotationDamage::of(result), result.cycle_seconds()?)));
    let inserts: Vec<RotationInsert> = insert_roles
        .iter()
        .map(|&role| RotationInsert {
            seconds: result_of(role).cycle_seconds(),
            cooldown_seconds: skill_of(role).cooldown_seconds.unwrap_or(0.0),
            minimum_filler_uses: material_of(role).minimum_filler_uses,
        })
        .collect();
    // 差し込むぶんの 1 回のダメージ。候補ぶんは上で作ってあるので作り直さない
    let parts: Vec<Vec<RotationDamage>> = insert_roles
        .iter()
        .map(|&role| match role {
            RotationRole::Candidate(index) => candidate_damages[index].clone(),
            RotationRole::Main => {
                let mut part = vec![RotationDamage::of(main_result)];
                if let Some(burst) = rotation.main.burst.as_ref() {
                    part.push(RotationDamage::of(&calculate_damage(material, &to_target(burst))));
                }
                part
            }
        })
        .collect();
    let parts: Vec<&[RotationDamage]> = parts.iter().map(Vec::as_slice).collect();
    let plan = plan_rotation(filler.map(|(_, seconds)| seconds), &inserts)?;
    rotation_dps(&plan, &parts, filler).map(|(_, expected)| expected)
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
    use crate::stats::{BaseStats, StatKind};
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
        max_level: 1,
        effects: &[],
        mastery_overrides: &[crate::character_skill::MasteryOverride {
            mastery_id: "test_m2_3",
            effects: ELITE_SWORDSMAN,
        }],
        exclusive_with: &[],
        requires: None,
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
            weapon_classes: Vec::new(),
            target: Some(SkillTarget::Single),
            accuracy: Some(92),
            critical_rate: Some(7),
            level: 1,
            channeling: None,
            field: None,
            base_actual_delay: Some(1.4),
            actual_delay_fixed: false,
            normal_attack: false,
            combo_interval: None,
            combo_variants: Vec::new(),
            power: Skill::compute_power(0.99, 1),
            power_per_second: Skill::compute_power_per_second(
                Skill::compute_power(0.99, 1),
                Some(1.4),
                None,
            ),
            attacker: crate::Attacker::Player,
            summon_form: None,
            form: None,
            swift_sword: None,
            full_charge: None,
            charge_seconds: 0.0,
            applies_flag: false,
            detonates_flag: false,
            cooldown_seconds: None,
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
            hp: None,
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
            accuracy_boost: AccuracyBoost::NONE,
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
            skill_added_damage_rate: 0.0,
            element_value: 0,
            flag: None,
            rotation: None,
        }];
        evaluate_contents_for_character(
            &material,
            &Equipment::default(),
            &content_area(),
            &[enemy()],
            &skills,
            None,
            EquipmentBaseContext::catalog_only(&[], &[]),
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

    /// ホームの討伐時間も回し(連打する技 × n → 差し込む CT 技 → <フラグ> の爆発)で出る。
    /// 計算タブ(`commands::combine_damage`)と同じ式なので、式そのものを手計算と突き合わせる。
    #[test]
    fn 回しで討伐時間を出す() {
        let material = material(false);
        let input = |s: Skill| SkillEvaluationInput {
            skill: s,
            coefficients: coefficients(),
            damage_contributions: Vec::new(),
            skill_added_damage_rate: 0.0,
            element_value: 0,
            flag: None,
            rotation: None,
        };
        // 主軸(CT のある、<フラグ> を爆発させる技)。連打技より 1 発が大きいので最良に選ばれる
        let main_skill = Skill { id: "main".into(), multiplier: 3.0, detonates_flag: true, cooldown_seconds: Some(10.0), ..skill() };
        let filler_skill = Skill { id: "filler".into(), multiplier: 1.0, applies_flag: true, base_actual_delay: Some(1.0), ..skill() };
        let burst_skill = Skill { id: "burst".into(), multiplier: 2.0, base_actual_delay: None, ..skill() };
        let duration_skill = Skill { id: "duration".into(), multiplier: 0.5, base_actual_delay: None, ..skill() };
        let mut main = input(main_skill);
        main.flag = Some(FlagEvaluationInput {
            duration: Box::new(input(duration_skill.clone())),
            tick_seconds: 1.0,
        });
        main.rotation = Some(RotationEvaluationInput {
            insert_skill_ids: None,
            // 候補は連打技 1 つだけ。主軸に CT があるので連打技として選ばれる
            candidates: vec![RotationCandidateEvaluationInput {
                skill: Box::new(input(filler_skill.clone())),
                insertable: false,
                material: RotationInsertMaterial::default(),
            }],
            pinned_filler_id: None,
            main: RotationInsertMaterial {
                burst: Some(Box::new(input(burst_skill.clone()))),
                // 積み直し 10 ÷ 2 = 5 回
                minimum_filler_uses: 5,
            },
        });
        let enemy_with_hp = Enemy { hp: Some(100_000), ..enemy() };
        let run = |skills: &[SkillEvaluationInput]| {
            evaluate_contents_for_character(
                &material,
                &Equipment::default(),
                &content_area(),
                &[enemy_with_hp.clone()],
                skills,
                None,
                EquipmentBaseContext::catalog_only(&[], &[]),
                &[],
                Awakening::default(),
                None,
            )[0]
                .damage
                .clone()
                .unwrap()
        };
        let with_flag = run(&[main.clone()]);

        // 手計算: 5 回では 5 × 1.0 + 1.4 = 6.4s で CT 10s に足りないので
        // ⌈(10 − 1.4) / 1.0⌉ = 9 回に増え、主軸の間隔は 10.4s
        let to_target_for = |i: &SkillEvaluationInput| DamageTarget {
            skill: i.skill.clone(),
            enemy: enemy_with_hp.clone(),
            combo_count: 0,
            coefficients: i.coefficients,
            equipment_base_sources: Vec::new(),
            equipment_enhanced_sources: Vec::new(),
            title_attack_damage_rate: 0.0,
            title_added_damage_rate: 0.0,
            damage_contributions: Vec::new(),
            skill_added_damage_rate: 0.0,
            element_value: 0,
        };
        let filler = calculate_damage(&material, &to_target_for(&input(filler_skill)));
        let burst = calculate_damage(&material, &to_target_for(&input(burst_skill)));
        let main_result = calculate_damage(&material, &to_target_for(&main));
        let mut duration = calculate_damage(&material, &to_target_for(&input(duration_skill)));
        apply_fixed_interval_dps(&mut duration, 1.0);
        let plan = plan_rotation(
            filler.cycle_seconds(),
            &[RotationInsert {
                seconds: main_result.cycle_seconds(),
                cooldown_seconds: 10.0,
                minimum_filler_uses: 5,
            }],
        )
        .unwrap();
        assert_eq!(plan.slots[0].filler_uses, 9);
        assert!((plan.slots[0].interval_seconds - 10.4).abs() < 1e-9);
        let (_, expected) = rotation_dps(
            &plan,
            &[&[RotationDamage::of(&main_result), RotationDamage::of(&burst)][..]],
            Some((RotationDamage::of(&filler), filler.cycle_seconds().unwrap())),
        )
        .unwrap();
        let combined = expected + duration.expected_dps.unwrap();
        assert!(
            (with_flag.defeat_seconds.unwrap() - 100_000.0 / combined).abs() < 1e-6,
            "{:?}",
            with_flag.defeat_seconds
        );

        // 回しが無ければ技単独のまま(回帰)。CT を見ない「技を連打する」前提なので、
        // 回し(CT 10 秒を守る)より速く出る — 比べる対象ではないので値だけ確認する
        let without = run(&[input(Skill { id: "main".into(), multiplier: 3.0, ..skill() })]);
        assert!(
            (without.defeat_seconds.unwrap() - 100_000.0 / main_result.expected_dps.unwrap()).abs()
                < 1e-6
        );
    }

    /// 主軸が連打技(CT なし)のときは、差し込む CT 技のぶんだけ期待 DPS が上がる。
    /// 差し込む技は CT ごとに 1 回しか撃てないので、その技を連打した DPS にはならない。
    #[test]
    fn 連打技に差し込むと期待dpsが上がる() {
        let material = material(false);
        let input = |s: Skill| SkillEvaluationInput {
            skill: s,
            coefficients: coefficients(),
            damage_contributions: Vec::new(),
            skill_added_damage_rate: 0.0,
            element_value: 0,
            flag: None,
            rotation: None,
        };
        let main_skill = Skill { id: "main".into(), multiplier: 1.0, base_actual_delay: Some(1.0), ..skill() };
        let insert_skill = Skill { id: "insert".into(), multiplier: 3.0, base_actual_delay: Some(1.0), cooldown_seconds: Some(10.0), ..skill() };
        let enemy_with_hp = Enemy { hp: Some(100_000), ..enemy() };
        let run = |skills: &[SkillEvaluationInput]| {
            evaluate_contents_for_character(
                &material,
                &Equipment::default(),
                &content_area(),
                &[enemy_with_hp.clone()],
                skills,
                None,
                EquipmentBaseContext::catalog_only(&[], &[]),
                &[],
                Awakening::default(),
                None,
            )[0]
                .damage
                .clone()
                .unwrap()
        };
        let alone = run(&[input(main_skill.clone())]);
        let mut with_insert = input(main_skill.clone());
        with_insert.rotation = Some(RotationEvaluationInput {
            insert_skill_ids: None,
            candidates: vec![RotationCandidateEvaluationInput {
                skill: Box::new(input(insert_skill.clone())),
                insertable: true,
                material: RotationInsertMaterial::default(),
            }],
            pinned_filler_id: None,
            main: RotationInsertMaterial::default(),
        });
        let with_insert = run(&[with_insert]);
        // 差し込むほうが速い(同じ時間に強い技が混ざる)
        assert!(with_insert.defeat_seconds.unwrap() < alone.defeat_seconds.unwrap());
        // ただし差し込む技を連打できるわけではない(CT 10 秒ぶんの間隔がある)
        let spam = run(&[input(Skill { cooldown_seconds: None, ..insert_skill })]);
        assert!(with_insert.defeat_seconds.unwrap() > spam.defeat_seconds.unwrap());
    }

    /// 熊(魔法人形)の入力を足すと期待 DPS が本体+熊になり、討伐時間が短くなる
    /// (wiki 計算式まとめ `STAB(熊)` 行)。入力なしなら既存テストのまま変わらない(回帰)。
    #[test]
    fn 熊の入力があると討伐時間が短くなる() {
        let material = material(false);
        let skills = vec![SkillEvaluationInput {
            skill: skill(),
            coefficients: coefficients(),
            damage_contributions: Vec::new(),
            skill_added_damage_rate: 0.0,
            element_value: 0,
            flag: None,
            rotation: None,
        }];
        let summon_skill = Skill {
            id: "bear".into(),
            base_actual_delay: Some(1.0),
            attacker: crate::Attacker::MagicDoll,
            ..skill()
        };
        let summon_input = SkillEvaluationInput {
            skill: summon_skill,
            coefficients: coefficients(),
            damage_contributions: Vec::new(),
            skill_added_damage_rate: 0.0,
            element_value: 0,
            flag: None,
            rotation: None,
        };
        let enemy_with_hp = Enemy {
            hp: Some(100_000),
            ..enemy()
        };

        let run = |summon: Option<&SkillEvaluationInput>| {
            evaluate_contents_for_character(
                &material,
                &Equipment::default(),
                &content_area(),
                &[enemy_with_hp.clone()],
                &skills,
                summon,
                EquipmentBaseContext::catalog_only(&[], &[]),
                &[],
                Awakening::default(),
                None,
            )
        };

        let without = run(None);
        let with = run(Some(&summon_input));
        let seconds_without = without[0].damage.as_ref().unwrap().defeat_seconds.unwrap();
        let seconds_with = with[0].damage.as_ref().unwrap().defeat_seconds.unwrap();
        assert!(
            seconds_with < seconds_without,
            "熊ぶんを足すと討伐時間が短くなるはず: without={seconds_without}, with={seconds_with}"
        );

        // 熊の入力が無いときは、既存(evaluate_contents_for_character に None を渡す)テストと
        // 同じ結果のまま変わらない(回帰)。
        let existing = evaluate(false);
        assert_eq!(
            existing[0].damage.as_ref().unwrap().per_hit_primary,
            without[0].damage.as_ref().unwrap().per_hit_primary
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
