//! フロントエンドから呼ばれるコマンド。ロジックは書かない。エラーは String に変換して返す。

use domain::{
    evaluate_content, AttackPowerCoefficients, BestSkillDamage, BuffDefinition, Content,
    ContentArea, ContentEvaluation, CoreRegion, DamageInput, DamageResult, DefenseProfile,
    Enemy, EquipmentAbilityDef, EquipmentPart, Skill,
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
    game_character_id: String,
    main_skill_id: Option<String>,
) -> CommandResult<domain::StatPreview> {
    let coefficients = attack_coefficients_of(main_skill_id.as_deref())?;
    domain::preview_effective_stats(
        &base_stats,
        &stat_sources,
        &equipment,
        &gamedata::buff_catalog(),
        &gamedata::equipment_abilities(),
        &game_character_id,
        coefficients,
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
    )
    .map_err(|e| e.to_string())?;
    let preview = domain::preview_effective_stats(
        &character.base_stats,
        &character.stat_sources,
        &character.equipment,
        &gamedata::buff_catalog(),
        &gamedata::equipment_abilities(),
        &character.game_character_id,
        None,
    )
    .map_err(|e| e.to_string())?;
    let equipment_totals = character
        .equipment
        .base_totals(&gamedata::equipment_abilities())
        .add(character.equipment.enhanced_totals(None));
    Ok(domain::defense_profile(&preview.stats, &equipment_totals))
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
/// - +12 以上は `enhance_added_damage`(実測上書き)があればそれ、無ければレンジ下限の倍率で算出
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
    let (min_multiplier, _max_multiplier) =
        gamedata::enhance_multiplier_range(weapon.enhance_level).unwrap_or((0.0, 0.0));
    domain::weapon_added_damage(&weapon.base, &rates, min_multiplier)
}

/// ダメージ計算の入力を組み立てる(calculate_damage / preview_damage / evaluate_contents 共通)。
#[allow(clippy::too_many_arguments)]
fn build_damage_input(
    base_stats: &domain::BaseStats,
    game_character_id: &str,
    stat_sources: &domain::StatSources,
    equipment: domain::Equipment,
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
    if let Some(temp) = &temporary_adjustments {
        temp.validate().map_err(|e| e.to_string())?;
        domain::stat_sources::apply_temporary_adjustments(&mut stat_modifiers, &mut stat_contributions, temp);
    }
    let equipment_base_totals = equipment.base_totals(&gamedata::equipment_abilities());
    let equipment_enhanced_totals = equipment.enhanced_totals(core_region);
    let added_damage = weapon_added_damage(&equipment.parts.weapon);
    Ok(DamageInput::new(
        base_stats.clone(),
        stat_modifiers,
        stat_contributions,
        coefficients,
        equipment,
        equipment_base_totals,
        equipment_enhanced_totals,
        equipment_coefficients,
        added_damage,
        awakening_rate,
        skill,
        enemy,
        combo_count,
        stat_sources.adjustments.clone(),
        temporary_adjustments,
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
    )
    .map_err(|e| e.to_string())?;
    let content = find_content(&content_id)?;
    let enemy = find_enemy(content.enemy_id.as_deref().unwrap_or_default())?;
    let input = build_damage_input(
        &character.base_stats,
        &character.game_character_id,
        &character.stat_sources,
        character.equipment,
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
    storage::validate_new_character(&character, &catalog, &equipment_catalog, &equipment_abilities)
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
    let awakening_rate = gamedata::awakening_rate(character.awakening);
    // 装備集計(基本能力値・武器追加固定ダメージ)はキャラのみ依存なのでループの外で 1 回だけ計算する。
    let equipment_base_totals = character.equipment.base_totals(&equipment_abilities);
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
                    equipment_base_totals,
                    equipment_enhanced_totals,
                    gamedata::equipment_coefficients(skill.dependency),
                    added_damage,
                    awakening_rate,
                    skill.clone(),
                    enemy.clone(),
                    0,
                    character.stat_sources.adjustments.clone(),
                    None,
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
    fn レンジ倍率帯は上書き優先_無ければレンジ下限() {
        assert_eq!(weapon_added_damage(&weapon(Some("abyss-scimitar"), 12, Some(311220))), 311220);
        // +12 レンジ下限 140 → INT(2101×140) = 294140(偶数)
        assert_eq!(weapon_added_damage(&weapon(Some("abyss-scimitar"), 12, None)), 294140);
    }
}
