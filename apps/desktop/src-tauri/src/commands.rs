//! フロントエンドから呼ばれるコマンド。ロジックは書かない。エラーは String に変換して返す。

use domain::{
    evaluate_content, AttackPowerCoefficients, BestSkillDamage, BuffDefinition, Content,
    CommonSkills, ContentArea, ContentEvaluation, CoreRegion, DamageInput, DamageResult, DefenseProfile,
    Enemy, EquipmentAbilityDef, EquipmentPart, RandomOptionDef, Skill, TitleDef,
};
use gamedata::{EquipmentItem, GameCharacter};
use storage::{CharacterRepository, NewCharacter, RegisteredCharacter};
use tauri::State;

use crate::AppState;

type CommandResult<T> = Result<T, String>;

fn with_repo<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&CharacterRepository) -> storage::Result<T>,
) -> CommandResult<T> {
    let repo = state.repo.lock().map_err(|e| format!("リポジトリのロックに失敗: {e}"))?;
    f(&repo).map_err(|e| e.to_string())
}

fn find_skill(skill_id: &str) -> CommandResult<Skill> {
    gamedata::find_skill(skill_id).ok_or_else(|| format!("スキル '{skill_id}' が見つかりません"))
}

fn find_enemy(enemy_id: &str) -> CommandResult<Enemy> {
    gamedata::find_enemy(enemy_id).ok_or_else(|| format!("敵 '{enemy_id}' が見つかりません"))
}

/// 計算対象のコンテンツを引く。敵データが無いコンテンツはダメージ計算の対象にできない。
fn find_content(content_id: &str) -> CommandResult<Content> {
    let content = gamedata::content_areas()
        .into_iter()
        .flat_map(|area| area.contents)
        .find(|c| c.id == content_id)
        .ok_or_else(|| format!("コンテンツ '{content_id}' が見つかりません"))?;
    if content.enemy_id.is_none() {
        return Err(format!("コンテンツ '{content_id}' には敵データがありません"));
    }
    Ok(content)
}

#[tauri::command]
pub fn list_game_characters() -> Vec<GameCharacter> {
    gamedata::characters().to_vec()
}

#[tauri::command]
pub fn list_skills(game_character_id: String) -> Vec<Skill> {
    gamedata::skills_for(&game_character_id)
}

#[tauri::command]
pub fn list_enemies() -> Vec<Enemy> {
    gamedata::enemies()
}

#[tauri::command]
pub fn list_buff_catalog() -> Vec<BuffDefinition> {
    gamedata::buff_catalog()
}

/// 属性値の供給源カタログ(装備の属性強化以外。ペット / モンスターカード / ルーン /
/// 頭アビリティ / カフスアビリティ)。
#[tauri::command]
pub fn list_element_sources() -> Vec<domain::ElementSourceDef> {
    gamedata::element_source_catalog().to_vec()
}

/// 属性値の内訳(キャラ基礎 / 装備の属性強化 / 装備以外の供給源 / 合計)。保存前のキャラデータで出す。
#[tauri::command]
pub fn preview_elements(character: NewCharacter) -> CommandResult<domain::ElementPreview> {
    storage::validate_new_character(
        &character,
        &gamedata::buff_catalog(),
        &gamedata::equipment_catalog(),
        &gamedata::equipment_abilities(),
        &gamedata::random_option_catalog(),
        &gamedata::title_catalog(),
        gamedata::actual_delay_skill_catalog(),
    )
    .map_err(|e| e.to_string())?;
    Ok(element_preview(&character.game_character_id, &character.equipment, &character.stat_sources))
}

#[tauri::command]
pub fn list_contents() -> Vec<ContentArea> {
    gamedata::content_areas()
}

#[tauri::command]
pub fn list_equipment_catalog() -> Vec<EquipmentItem> {
    gamedata::equipment_catalog()
}

#[tauri::command]
pub fn list_equipment_abilities() -> Vec<EquipmentAbilityDef> {
    gamedata::equipment_abilities()
}

/// ランダムオプションのカタログ(wiki: ランダムオプション)。
#[tauri::command]
pub fn list_random_options() -> Vec<RandomOptionDef> {
    gamedata::random_option_catalog()
}

/// 中ディレイ減少スキルのカタログ(wiki: ステータス「中ディレイ倍率B」)。キャラ固有のパッシブのみ。
/// 9 件しかないので全件返し、キャラでの絞り込みは UI 側で `game_character_id` を見て行う。
#[tauri::command]
pub fn list_actual_delay_skills() -> Vec<domain::ActualDelaySkillDef> {
    gamedata::actual_delay_skill_catalog().to_vec()
}

/// 称号のカタログ(wiki: 称号システム)。主要称号のみ。
#[tauri::command]
pub fn list_titles() -> Vec<TitleDef> {
    gamedata::title_catalog()
}

#[tauri::command]
pub fn list_characters(state: State<'_, AppState>) -> CommandResult<Vec<RegisteredCharacter>> {
    with_repo(&state, |repo| repo.list())
}

/// 主軸スキル(攻撃力の依存種別を決める)はそのキャラのスキル一覧に含まれている必要がある。
/// キャラ種を変えたときに前キャラのスキルが残るのを防ぐ。未選択(`None`)は許す。
fn validate_main_skill(character: &NewCharacter) -> CommandResult<()> {
    let Some(skill_id) = &character.main_skill_id else {
        return Ok(());
    };
    if !gamedata::skills_for(&character.game_character_id).iter().any(|s| &s.id == skill_id) {
        return Err(format!(
            "主軸スキル '{skill_id}' は '{}' のスキルではありません",
            character.game_character_id
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn create_character(
    state: State<'_, AppState>,
    character: NewCharacter,
) -> CommandResult<RegisteredCharacter> {
    if gamedata::find_character(&character.game_character_id).is_none() {
        return Err(format!("ゲームキャラ '{}' は未登録です", character.game_character_id));
    }
    validate_main_skill(&character)?;
    with_repo(&state, |repo| {
        repo.create(
            &character,
            &gamedata::buff_catalog(),
            &gamedata::equipment_catalog(),
            &gamedata::equipment_abilities(),
            &gamedata::random_option_catalog(),
            &gamedata::title_catalog(),
            gamedata::actual_delay_skill_catalog(),
        )
    })
}

#[tauri::command]
pub fn update_character(
    state: State<'_, AppState>,
    id: i64,
    character: NewCharacter,
) -> CommandResult<RegisteredCharacter> {
    if gamedata::find_character(&character.game_character_id).is_none() {
        return Err(format!("ゲームキャラ '{}' は未登録です", character.game_character_id));
    }
    validate_main_skill(&character)?;
    with_repo(&state, |repo| {
        repo.update(
            id,
            &character,
            &gamedata::buff_catalog(),
            &gamedata::equipment_catalog(),
            &gamedata::equipment_abilities(),
            &gamedata::random_option_catalog(),
            &gamedata::title_catalog(),
            gamedata::actual_delay_skill_catalog(),
        )
    })
}

#[tauri::command]
pub fn delete_character(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    with_repo(&state, |repo| repo.delete(id))
}

/// キャラの主軸スキルから攻撃力(A)の係数一式を引く。未選択なら `None`(攻撃力を出さない)。
fn attack_coefficients_of(main_skill_id: Option<&str>) -> CommandResult<Option<AttackPowerCoefficients>> {
    let Some(skill_id) = main_skill_id else {
        return Ok(None);
    };
    let dependency = find_skill(skill_id)?.dependency;
    Ok(Some(AttackPowerCoefficients {
        stat: gamedata::attack_coefficients(dependency),
        equipment: gamedata::equipment_coefficients(dependency),
    }))
}

#[tauri::command]
pub fn preview_effective_stats(
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    game_character_id: String,
    main_skill_id: Option<String>,
) -> CommandResult<domain::StatPreview> {
    let coefficients = attack_coefficients_of(main_skill_id.as_deref())?;
    domain::preview_effective_stats(
        &base_stats,
        &stat_sources,
        &equipment,
        &common_skills,
        &gamedata::buff_catalog(),
        &gamedata::equipment_abilities(),
        &gamedata::title_catalog(),
        &game_character_id,
        coefficients,
        gamedata::awakening_caps(awakening).max_stat,
    )
    .map_err(|e| e.to_string())
}

/// 防御側の戦闘能力値(docs/damage-formula.md §6〜7)。保存前のキャラデータで出す。
///
/// 与ダメージ式とは別経路なので対象コンテンツを取らない。装備補正 9 値は
/// 基本能力値 + 強化能力値(地域なし = テシスコアを含まない)の合計を渡す。
#[tauri::command]
pub fn preview_defense(character: NewCharacter) -> CommandResult<DefenseProfile> {
    storage::validate_new_character(
        &character,
        &gamedata::buff_catalog(),
        &gamedata::equipment_catalog(),
        &gamedata::equipment_abilities(),
        &gamedata::random_option_catalog(),
        &gamedata::title_catalog(),
        gamedata::actual_delay_skill_catalog(),
    )
    .map_err(|e| e.to_string())?;
    let preview = domain::preview_effective_stats(
        &character.base_stats,
        &character.stat_sources,
        &character.equipment,
        &character.common_skills,
        &gamedata::buff_catalog(),
        &gamedata::equipment_abilities(),
        &gamedata::title_catalog(),
        &character.game_character_id,
        None,
        gamedata::awakening_caps(character.awakening).max_stat,
    )
    .map_err(|e| e.to_string())?;
    let equipment_totals = character
        .equipment
        .base_totals(&gamedata::equipment_abilities(), &gamedata::title_catalog())
        .add(character.equipment.enhanced_totals(None));
    Ok(domain::defense_profile(
        &preview.stats,
        &equipment_totals,
        gamedata::awakening_caps(character.awakening),
        &character.equipment.random_option_totals(&gamedata::random_option_catalog()),
        // 装備防御力倍率(共通スキル + シエナのオーラの防御力増加)。
        // リンゴの島・ベリネンルミは常に 100% だが、防御タブは対象コンテンツを取らないので
        // ここでは習得どおりの倍率で出す(その注記は UI 側で出す)
        character.common_skills.defense_rates(character.equipment.siena_defense_rate()),
    ))
}

#[tauri::command]
pub fn get_stat_limits() -> domain::StatLimits {
    domain::stat_sources::stat_limits()
}

/// 武器の装備強化による追加固定ダメージ(wiki: 装備システム/装備強化、docs/damage-formula.md §5)。
///
/// `item_id` → カタログの `weapon_class` → 系統ごとの補正式、の順で解決する。
/// - 強化 Lv 0 は 0
/// - +1〜+11 は確定倍率で式から算出
/// - +12 以上は `enhance_added_damage`(実測上書き)があればそれ、無ければ**レンジ上限**の倍率で算出
///   (wiki 装備システム/装備強化「強化数値の再設定」: 再設定呪文書で振り直せるので、
///   実用上の想定値はレンジの最上値。ユーザー決定 2026-08-25)
/// - カスタム武器(カタログ外・`weapon_class` 不明)は式で算出できないため `enhance_added_damage ?? 0`
fn weapon_added_damage(weapon: &EquipmentPart) -> i64 {
    if weapon.enhance_level == 0 {
        return 0;
    }
    let weapon_class = weapon
        .item_id
        .as_deref()
        .and_then(gamedata::find_equipment_item)
        .and_then(|item| item.weapon_class);
    let Some(weapon_class) = weapon_class else {
        return weapon.enhance_added_damage.unwrap_or(0);
    };
    let rates = gamedata::enhance_rates(weapon_class);
    if let Some(multiplier) = gamedata::enhance_multiplier(weapon.enhance_level) {
        return domain::weapon_added_damage(&weapon.base, &rates, multiplier);
    }
    if let Some(added) = weapon.enhance_added_damage {
        return added;
    }
    let (_min_multiplier, max_multiplier) =
        gamedata::enhance_multiplier_range(weapon.enhance_level).unwrap_or((0.0, 0.0));
    domain::weapon_added_damage(&weapon.base, &rates, max_multiplier)
}

/// 属性値の内訳。キャラの基礎属性値(gamedata)+ 装備の属性強化(部位ごとに 0〜9)+
/// 装備以外の供給源(ペット / モンスターカード / ルーン / 頭アビ / カフスアビ)。合計は上限 255。
fn element_preview(
    game_character_id: &str,
    equipment: &domain::Equipment,
    stat_sources: &domain::StatSources,
) -> domain::ElementPreview {
    domain::ElementPreview::new(
        gamedata::element_base(game_character_id),
        equipment.element_values(),
        stat_sources.elements.values(gamedata::element_source_catalog()),
    )
}

/// スキルの属性に対応するキャラの属性値(wiki: カテゴリI の起点)。
fn element_value_for(
    game_character_id: &str,
    equipment: &domain::Equipment,
    stat_sources: &domain::StatSources,
    skill: &Skill,
) -> i64 {
    element_preview(game_character_id, equipment, stat_sources).total.get(skill.element)
}

/// ダメージ計算の入力を組み立てる(calculate_damage / preview_damage / evaluate_contents 共通)。
#[allow(clippy::too_many_arguments)]
fn build_damage_input(
    base_stats: &domain::BaseStats,
    game_character_id: &str,
    stat_sources: &domain::StatSources,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    skill: Skill,
    enemy: Enemy,
    core_region: Option<CoreRegion>,
    combo_count: u32,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageInput> {
    let coefficients = gamedata::attack_coefficients(skill.dependency);
    let equipment_coefficients = gamedata::equipment_coefficients(skill.dependency);
    let awakening_rate = gamedata::awakening_rate(awakening);
    let (mut stat_modifiers, mut stat_contributions) =
        domain::stat_sources::build_modifiers(stat_sources, &gamedata::buff_catalog(), game_character_id)
            .map_err(|e| e.to_string())?;
    domain::stat_sources::apply_siena_stats(&mut stat_modifiers, &mut stat_contributions, &equipment);
    domain::stat_sources::apply_unleash(&mut stat_modifiers, &mut stat_contributions, &common_skills);
    if let Some(temp) = &temporary_adjustments {
        temp.validate().map_err(|e| e.to_string())?;
        domain::stat_sources::apply_temporary_adjustments(&mut stat_modifiers, &mut stat_contributions, temp);
    }
    let equipment_base_totals = equipment.base_totals(&gamedata::equipment_abilities(), &gamedata::title_catalog());
    let equipment_enhanced_totals = equipment.enhanced_totals(core_region);
    let random_options = equipment.random_option_totals(&gamedata::random_option_catalog());
    let added_damage = weapon_added_damage(&equipment.parts.weapon);
    let element_value = element_value_for(game_character_id, &equipment, stat_sources, &skill);
    Ok(DamageInput::new(
        base_stats.clone(),
        stat_modifiers,
        stat_contributions,
        coefficients,
        equipment,
        common_skills,
        equipment_base_totals,
        equipment_enhanced_totals,
        equipment_coefficients,
        gamedata::accuracy_correction(skill.dependency),
        random_options,
        added_damage,
        awakening_rate,
        gamedata::awakening_caps(awakening).max_damage,
        gamedata::awakening_caps(awakening).max_stat,
        skill,
        enemy,
        combo_count,
        element_value,
        stat_sources.adjustments.clone(),
        temporary_adjustments,
        stat_sources.actual_delay_skills.contributions(gamedata::actual_delay_skill_catalog()),
        stat_sources.critical_rate,
        gamedata::skill_uses_table(),
    ))
}

#[tauri::command]
pub fn calculate_damage(
    state: State<'_, AppState>,
    character_id: i64,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageResult> {
    let character = with_repo(&state, |repo| repo.get(character_id))?;
    let content = find_content(&content_id)?;
    let enemy = find_enemy(content.enemy_id.as_deref().unwrap_or_default())?;
    let input = build_damage_input(
        &character.base_stats,
        &character.game_character_id,
        &character.stat_sources,
        character.equipment,
        character.common_skills,
        character.awakening,
        find_skill(&skill_id)?,
        enemy,
        content.core_region,
        combo_count,
        temporary_adjustments,
    )?;
    Ok(domain::calculate_damage(&input))
}

/// 保存前のキャラデータ(編集中 draft・試し変更)でダメージ計算する。DB には書き込まない。
#[tauri::command]
pub fn preview_damage(
    character: NewCharacter,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageResult> {
    storage::validate_new_character(
        &character,
        &gamedata::buff_catalog(),
        &gamedata::equipment_catalog(),
        &gamedata::equipment_abilities(),
        &gamedata::random_option_catalog(),
        &gamedata::title_catalog(),
        gamedata::actual_delay_skill_catalog(),
    )
    .map_err(|e| e.to_string())?;
    let content = find_content(&content_id)?;
    let enemy = find_enemy(content.enemy_id.as_deref().unwrap_or_default())?;
    let input = build_damage_input(
        &character.base_stats,
        &character.game_character_id,
        &character.stat_sources,
        character.equipment,
        character.common_skills,
        character.awakening,
        find_skill(&skill_id)?,
        enemy,
        content.core_region,
        combo_count,
        temporary_adjustments,
    )?;
    Ok(domain::calculate_damage(&input))
}

/// 全コンテンツを判定する(ホームの到達一覧・キャラレールのクリア数)。
/// 火力はキャラのスキルのうち 1 ヒット(最大)が最大のもの、コンボ補正なしで評価する。
///
/// `dependency_skill_id` は装備条件(スキル依存で比較先が変わる)の判定に使うスキル。
/// 計算タブのように「今このスキルで戦う」文脈では選択中スキルを渡す。None ならコンテンツ
/// ごとの最大ダメージスキル(敵データなしコンテンツは一覧先頭)の依存で判定する。
#[tauri::command]
pub fn evaluate_contents(
    character: NewCharacter,
    dependency_skill_id: Option<String>,
) -> CommandResult<Vec<ContentEvaluation>> {
    let catalog = gamedata::buff_catalog();
    let equipment_catalog = gamedata::equipment_catalog();
    let equipment_abilities = gamedata::equipment_abilities();
    let random_options = gamedata::random_option_catalog();
    let titles = gamedata::title_catalog();
    storage::validate_new_character(
        &character,
        &catalog,
        &equipment_catalog,
        &equipment_abilities,
        &random_options,
        &titles,
        gamedata::actual_delay_skill_catalog(),
    )
    .map_err(|e| e.to_string())?;
    let skills = gamedata::skills_for(&character.game_character_id);
    // ループ不変値(キャラのみ依存)は 1 回だけ構築する。コンテンツ×スキルごとに
    // カタログとステ補正を再構築すると、この最重量パスで無駄な再計算になる(PR レビュー指摘)。
    let (mut stat_modifiers, mut stat_contributions) = domain::stat_sources::build_modifiers(
        &character.stat_sources,
        &catalog,
        &character.game_character_id,
    )
    .map_err(|e| e.to_string())?;
    domain::stat_sources::apply_siena_stats(
        &mut stat_modifiers,
        &mut stat_contributions,
        &character.equipment,
    );
    domain::stat_sources::apply_unleash(
        &mut stat_modifiers,
        &mut stat_contributions,
        &character.common_skills,
    );
    let awakening_rate = gamedata::awakening_rate(character.awakening);
    let actual_delay_skills = character
        .stat_sources
        .actual_delay_skills
        .contributions(gamedata::actual_delay_skill_catalog());
    let skill_uses = gamedata::skill_uses_table();
    // 装備集計(基本能力値・武器追加固定ダメージ)はキャラのみ依存なのでループの外で 1 回だけ計算する。
    let equipment_base_totals = character.equipment.base_totals(&equipment_abilities, &titles);
    let random_option_totals = character.equipment.random_option_totals(&random_options);
    let added_damage = weapon_added_damage(&character.equipment.parts.weapon);
    // 強化能力値はテシスコアの地域で変わるので、地域ごとに 1 回だけ集計してループ内で使い回す
    // (地域は 4 つ + 地域なし。コンテンツごとに再集計すると最重量パスで無駄な再計算になる)。
    let enhanced_totals_of = |region: Option<CoreRegion>| character.equipment.enhanced_totals(region);
    let enhanced_by_region: Vec<(Option<CoreRegion>, domain::EquipmentValues)> =
        std::iter::once(None)
            .chain(CoreRegion::ALL.into_iter().map(Some))
            .map(|region| (region, enhanced_totals_of(region)))
            .collect();
    let enhanced_for = |region: Option<CoreRegion>| {
        enhanced_by_region.iter().find(|(r, _)| *r == region).map(|(_, v)| *v).unwrap_or_default()
    };
    // 呼び出し側がスキルを指定したら、装備条件の比較先はそのスキルの依存で固定する。
    let fixed_dependency = match dependency_skill_id {
        None => None,
        Some(id) => Some(find_skill(&id)?.dependency),
    };
    let mut evaluations = Vec::new();
    for area in gamedata::content_areas() {
        for content in &area.contents {
            // 敵データが無いコンテンツ(入場条件のみ判定)は火力計算をしない。装備条件の
            // 比較先はキャラの代表スキル(一覧の先頭)の依存種別で決める。
            let thesis_core_total =
                character.equipment.thesis_cores.total_bonus(content.core_region);
            let Some(enemy_id) = content.enemy_id.as_deref() else {
                let dependency = fixed_dependency.or_else(|| skills.first().map(|s| s.dependency));
                evaluations.push(evaluate_content(
                    content,
                    None,
                    &equipment_base_totals,
                    character.awakening,
                    dependency,
                    thesis_core_total,
                ));
                continue;
            };
            let equipment_enhanced_totals = enhanced_for(content.core_region);
            let enemy = find_enemy(enemy_id)?;
            let mut best: Option<BestSkillDamage> = None;
            let mut best_dependency: Option<domain::SkillDependency> = None;
            for skill in &skills {
                let input = DamageInput::new(
                    character.base_stats.clone(),
                    stat_modifiers.clone(),
                    stat_contributions.clone(),
                    gamedata::attack_coefficients(skill.dependency),
                    character.equipment.clone(),
                    character.common_skills,
                    equipment_base_totals,
                    equipment_enhanced_totals,
                    gamedata::equipment_coefficients(skill.dependency),
                    gamedata::accuracy_correction(skill.dependency),
                    random_option_totals,
                    added_damage,
                    awakening_rate,
                    gamedata::awakening_caps(character.awakening).max_damage,
                    gamedata::awakening_caps(character.awakening).max_stat,
                    skill.clone(),
                    enemy.clone(),
                    0,
                    element_value_for(
                        &character.game_character_id,
                        &character.equipment,
                        &character.stat_sources,
                        skill,
                    ),
                    character.stat_sources.adjustments.clone(),
                    None,
                    actual_delay_skills.clone(),
                    character.stat_sources.critical_rate,
                    skill_uses.clone(),
                );
                let result = domain::calculate_damage(&input);
                if best.as_ref().is_none_or(|b| result.per_hit.max > b.per_hit_max) {
                    best = Some(BestSkillDamage {
                        skill_id: skill.id.clone(),
                        per_hit_max: result.per_hit.max,
                        total_max: result.total.max,
                    });
                    // 装備条件の比較先は「判定に使ったスキル」の依存種別で決める
                    best_dependency = Some(skill.dependency);
                }
            }
            evaluations.push(evaluate_content(
                content,
                best,
                &equipment_base_totals,
                character.awakening,
                fixed_dependency.or(best_dependency),
                thesis_core_total,
            ));
        }
    }
    Ok(evaluations)
}

#[cfg(test)]
mod tests {
    use super::weapon_added_damage;
    use domain::{EquipmentPart, EquipmentValues};

    // 刀(HACK系: 斬×6.67 + 突×1.00)・突100/斬300 → INT(300×6.67+100) = 2101
    fn weapon(item_id: Option<&str>, level: u8, added: Option<i64>) -> EquipmentPart {
        EquipmentPart {
            item_id: item_id.map(String::from),
            enhance_level: level,
            enhance_added_damage: added,
            base: EquipmentValues { thrust: 100, slash: 300, ..Default::default() },
            ..Default::default()
        }
    }

    #[test]
    fn 強化なしは追加固定ダメージ0() {
        assert_eq!(weapon_added_damage(&weapon(Some("abyss-scimitar"), 0, None)), 0);
    }

    #[test]
    fn カタログ外武器は上書きが無ければ0_あればその値() {
        assert_eq!(weapon_added_damage(&weapon(None, 5, None)), 0);
        assert_eq!(weapon_added_damage(&weapon(None, 12, Some(12345))), 12345);
    }

    #[test]
    fn 確定倍率帯は系統式から算出する() {
        // +10 倍率 28.8 → INT(2101×28.8) = 60508(偶数なのでそのまま)
        assert_eq!(weapon_added_damage(&weapon(Some("abyss-scimitar"), 10, None)), 60508);
    }

    #[test]
    fn レンジ倍率帯は上書き優先_無ければレンジ上限() {
        assert_eq!(weapon_added_damage(&weapon(Some("abyss-scimitar"), 12, Some(311220))), 311220);
        // +12 レンジ上限 280 → INT(2101×280) = 588280(偶数)
        assert_eq!(weapon_added_damage(&weapon(Some("abyss-scimitar"), 12, None)), 588280);
        // +15 レンジ上限 880 → INT(2101×880) = 1848880(偶数)
        assert_eq!(weapon_added_damage(&weapon(Some("abyss-scimitar"), 15, None)), 1848880);
    }
}
