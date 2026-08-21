//! フロントエンドから呼ばれるコマンド。ロジックは書かない。エラーは String に変換して返す。

use domain::{DamageInput, DamageResult, Enemy, Skill, StatModifierSet};
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
    with_repo(&state, |repo| repo.create(&character))
}

#[tauri::command]
pub fn delete_character(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    with_repo(&state, |repo| repo.delete(id))
}

#[tauri::command]
pub fn calculate_damage(
    state: State<'_, AppState>,
    character_id: i64,
    skill_id: String,
    enemy_id: String,
    combo_count: u32,
) -> CommandResult<DamageResult> {
    let character = with_repo(&state, |repo| repo.get(character_id))?;
    let skill = gamedata::find_skill(&skill_id).ok_or_else(|| format!("スキル '{skill_id}' が見つかりません"))?;
    let enemy = gamedata::find_enemy(&enemy_id).ok_or_else(|| format!("敵 '{enemy_id}' が見つかりません"))?;
    let input = DamageInput {
        base_stats: character.base_stats,
        stat_modifiers: StatModifierSet::neutral(),
        coefficients: gamedata::attack_coefficients(skill.dependency),
        awakening_rate: gamedata::awakening_rate(character.awakening),
        skill,
        enemy,
        combo_count,
        element_value: 0,
        equipment_attack: 0.0,
        equipment_enhance_rate: 0.0,
    };
    Ok(domain::calculate_damage(&input))
}
