//! フロントエンドから呼ばれるコマンド。ロジックは書かない。エラーは String に変換して返す。

use domain::{
    evaluate_content, BestSkillDamage, BuffDefinition, ContentArea, ContentEvaluation, DamageInput,
    DamageResult, Enemy, Skill,
};
use gamedata::GameCharacter;
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
pub fn list_characters(state: State<'_, AppState>) -> CommandResult<Vec<RegisteredCharacter>> {
    with_repo(&state, |repo| repo.list())
}

#[tauri::command]
pub fn create_character(
    state: State<'_, AppState>,
    character: NewCharacter,
) -> CommandResult<RegisteredCharacter> {
    if gamedata::find_character(&character.game_character_id).is_none() {
        return Err(format!("ゲームキャラ '{}' は未登録です", character.game_character_id));
    }
    with_repo(&state, |repo| repo.create(&character, &gamedata::buff_catalog()))
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
    with_repo(&state, |repo| repo.update(id, &character, &gamedata::buff_catalog()))
}

#[tauri::command]
pub fn delete_character(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    with_repo(&state, |repo| repo.delete(id))
}

#[tauri::command]
pub fn preview_effective_stats(
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    game_character_id: String,
) -> CommandResult<domain::StatPreview> {
    domain::preview_effective_stats(&base_stats, &stat_sources, &gamedata::buff_catalog(), &game_character_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_stat_limits() -> domain::StatLimits {
    domain::stat_sources::stat_limits()
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
    combo_count: u32,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageInput> {
    let coefficients = gamedata::attack_coefficients(skill.dependency);
    let equipment_coefficients = gamedata::equipment_coefficients(skill.dependency);
    let awakening_rate = gamedata::awakening_rate(awakening);
    let (mut stat_modifiers, mut stat_contributions) =
        domain::stat_sources::build_modifiers(stat_sources, &gamedata::buff_catalog(), game_character_id)
            .map_err(|e| e.to_string())?;
    if let Some(temp) = &temporary_adjustments {
        temp.validate().map_err(|e| e.to_string())?;
        domain::stat_sources::apply_temporary_adjustments(&mut stat_modifiers, &mut stat_contributions, temp);
    }
    Ok(DamageInput::new(
        base_stats.clone(),
        stat_modifiers,
        stat_contributions,
        coefficients,
        equipment,
        equipment_coefficients,
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
    enemy_id: String,
    combo_count: u32,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageResult> {
    let character = with_repo(&state, |repo| repo.get(character_id))?;
    let input = build_damage_input(
        &character.base_stats,
        &character.game_character_id,
        &character.stat_sources,
        character.equipment,
        character.awakening,
        find_skill(&skill_id)?,
        find_enemy(&enemy_id)?,
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
    enemy_id: String,
    combo_count: u32,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageResult> {
    storage::validate_new_character(&character, &gamedata::buff_catalog()).map_err(|e| e.to_string())?;
    let input = build_damage_input(
        &character.base_stats,
        &character.game_character_id,
        &character.stat_sources,
        character.equipment,
        character.awakening,
        find_skill(&skill_id)?,
        find_enemy(&enemy_id)?,
        combo_count,
        temporary_adjustments,
    )?;
    Ok(domain::calculate_damage(&input))
}

/// 全コンテンツを判定する(ホームの到達一覧・キャラレールのクリア数)。
/// 火力はキャラのスキルのうち 1 ヒット(最大)が最大のもの、コンボ補正なしで評価する。
#[tauri::command]
pub fn evaluate_contents(character: NewCharacter) -> CommandResult<Vec<ContentEvaluation>> {
    let catalog = gamedata::buff_catalog();
    storage::validate_new_character(&character, &catalog).map_err(|e| e.to_string())?;
    let skills = gamedata::skills_for(&character.game_character_id);
    // ループ不変値(キャラのみ依存)は 1 回だけ構築する。コンテンツ×スキルごとに
    // カタログとステ補正を再構築すると、この最重量パスで無駄な再計算になる(PR レビュー指摘)。
    let (stat_modifiers, stat_contributions) = domain::stat_sources::build_modifiers(
        &character.stat_sources,
        &catalog,
        &character.game_character_id,
    )
    .map_err(|e| e.to_string())?;
    let awakening_rate = gamedata::awakening_rate(character.awakening);
    let mut evaluations = Vec::new();
    for area in gamedata::content_areas() {
        for content in &area.contents {
            let enemy = find_enemy(&content.enemy_id)?;
            let mut best: Option<BestSkillDamage> = None;
            for skill in &skills {
                let input = DamageInput::new(
                    character.base_stats.clone(),
                    stat_modifiers.clone(),
                    stat_contributions.clone(),
                    gamedata::attack_coefficients(skill.dependency),
                    character.equipment,
                    gamedata::equipment_coefficients(skill.dependency),
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
                }
            }
            evaluations.push(evaluate_content(content, best, &character.equipment, character.awakening));
        }
    }
    Ok(evaluations)
}
