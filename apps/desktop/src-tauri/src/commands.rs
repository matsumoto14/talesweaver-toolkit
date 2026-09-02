//! フロントエンドから呼ばれるコマンド。ロジックは書かない。
//!
//! 保存に触らない計算系の中身は `commands` crate にある(Web 版でも同じ計算を動かすため)。
//! ここは `#[tauri::command]` を付けて呼ぶだけの薄いラッパと、保存層(rusqlite なので
//! wasm で動かない)を触るコマンドだけを持つ。

use base64::Engine;
use commands::{
    CharacterSkillEffectsView, CommandError, CommandResult, EnchantGain, EnchantPlanRow,
    EquipmentAbilityView, EquipmentCandidates, GameTablesPayload, TitleView, UpgradeCandidate,
};
use domain::{
    BuffSelection, CommonSkills, ContentArea, ContentEvaluation, DamageResult,
    DefenseProfile, Enemy, GrowthAction, NewCharacter, Skill, VersusAccuracy,
};
use gamedata::{EquipmentItem, GameCharacter};
use storage::{BuffSet, CharacterIcon, CharacterRepository, DamageSnapshot, RegisteredCharacter};
use tauri::{Manager, State};

use crate::{AppInfo, AppState};

/// 保存層のエラーをフロント向けに変換する。`CommandError` も `StorageError` も
/// このクレートの外の型なので `From` は実装できず(孤児則)、関数で変換する。
fn storage_error(error: storage::StorageError) -> CommandError {
    match error {
        storage::StorageError::InvalidValue(invalid) => CommandError {
            message: format!("不正な値: {}", invalid.message),
            location: invalid.location,
        },
        other => CommandError {
            message: other.to_string(),
            location: None,
        },
    }
}

fn with_repo<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&CharacterRepository) -> storage::Result<T>,
) -> CommandResult<T> {
    let repo = state
        .repo
        .lock()
        .map_err(|e| format!("リポジトリのロックに失敗: {e}"))?;
    f(&repo).map_err(storage_error)
}

/// 起動時に復元などが起きたことの通知。通常起動なら `None`。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupNoticePayload {
    pub message: String,
    /// `false` のとき、この起動で加えた変更は保存されない。
    pub persists_changes: bool,
}

#[tauri::command]
pub fn get_app_info(app: tauri::AppHandle) -> CommandResult<AppInfo> {
    let database_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("保存先を特定できません: {e}"))?
        .join(crate::DATABASE_FILE_NAME);
    Ok(AppInfo {
        version: crate::APP_VERSION.to_string(),
        database_path: database_path.display().to_string(),
    })
}

#[tauri::command]
pub fn get_startup_notice(state: State<'_, AppState>) -> Option<StartupNoticePayload> {
    state
        .startup_notice
        .as_ref()
        .map(|notice| StartupNoticePayload {
            message: notice.message(),
            persists_changes: notice.persists_changes(),
        })
}

#[tauri::command]
pub fn list_game_characters() -> Vec<GameCharacter> {
    commands::list_game_characters()
}

#[tauri::command]
pub fn list_skills(game_character_id: String) -> Vec<Skill> {
    commands::list_skills(game_character_id)
}

#[tauri::command]
pub fn list_enemies() -> Vec<Enemy> {
    commands::list_enemies()
}

#[tauri::command]
pub fn list_buff_catalog() -> Vec<commands::BuffView> {
    commands::list_buff_catalog()
}

#[tauri::command]
pub fn preview_potential_effects(
    stat_sources: domain::StatSources,
    common_skills: domain::CommonSkills,
) -> commands::PotentialEffects {
    commands::preview_potential_effects(stat_sources, common_skills)
}

#[tauri::command]
pub fn summarize_buff_selection(buffs: BuffSelection) -> CommandResult<domain::BuffDamageSummary> {
    commands::summarize_buff_selection(buffs)
}

#[tauri::command]
pub fn list_blocked_buffs(buffs: BuffSelection) -> Vec<domain::BlockedBuff> {
    commands::list_blocked_buffs(buffs)
}

#[tauri::command]
pub fn list_element_sources() -> Vec<domain::ElementSourceDef> {
    commands::list_element_sources()
}

#[tauri::command]
pub fn equipment_element_values(
    equipment: domain::Equipment,
    element: Option<domain::Element>,
) -> domain::ElementValues {
    commands::equipment_element_values(equipment, element)
}

#[tauri::command]
pub fn preview_elements(character: NewCharacter) -> CommandResult<domain::ElementPreview> {
    commands::preview_elements(character)
}

#[tauri::command]
pub fn list_contents() -> Vec<ContentArea> {
    commands::list_contents()
}

#[tauri::command]
pub fn list_equipment_catalog() -> Vec<EquipmentItem> {
    commands::list_equipment_catalog()
}

#[tauri::command]
pub fn list_equipment_abilities() -> Vec<EquipmentAbilityView> {
    commands::list_equipment_abilities()
}

#[tauri::command]
pub fn list_equipment_candidates(
    game_character_id: Option<String>,
    main_skill_id: Option<String>,
    slot: domain::PartSlot,
) -> EquipmentCandidates {
    commands::list_equipment_candidates(game_character_id, main_skill_id, slot)
}

#[tauri::command]
pub fn part_weapon_system(part: domain::EquipmentPart) -> Option<domain::WeaponSystem> {
    commands::part_weapon_system(part)
}

#[tauri::command]
pub fn list_enchant_plans(character: NewCharacter) -> Vec<EnchantPlanRow> {
    commands::list_enchant_plans(character)
}

#[tauri::command]
pub fn relic_state(part: domain::EquipmentPart) -> Option<domain::RelicState> {
    commands::relic_state(part)
}

#[tauri::command]
pub fn relic_step(
    part: domain::EquipmentPart,
    direction: domain::RelicDirection,
) -> Option<domain::EquipmentPart> {
    commands::relic_step(part, direction)
}

#[tauri::command]
pub fn list_equipment_ability_candidates(
    part: domain::EquipmentPart,
    slot: domain::PartSlot,
    category: Option<u8>,
) -> Vec<commands::EquipmentAbilityCandidate> {
    commands::list_equipment_ability_candidates(part, slot, category)
}

#[tauri::command]
pub fn apply_catalog_item(
    part: domain::EquipmentPart,
    item_id: String,
) -> Option<domain::EquipmentPart> {
    commands::apply_catalog_item(part, item_id)
}

#[tauri::command]
pub fn set_enhance_level(part: domain::EquipmentPart, level: u8) -> domain::EquipmentPart {
    commands::set_enhance_level(part, level)
}

#[tauri::command]
pub fn set_ability_for_category(
    part: domain::EquipmentPart,
    slot: domain::PartSlot,
    category: u8,
    ability_id: Option<String>,
) -> domain::EquipmentPart {
    commands::set_ability_for_category(part, slot, category, ability_id)
}

#[tauri::command]
pub fn toggle_ability(
    part: domain::EquipmentPart,
    slot: domain::PartSlot,
    ability_id: String,
) -> domain::EquipmentPart {
    commands::toggle_ability(part, slot, ability_id)
}

#[tauri::command]
pub fn list_random_options() -> Vec<commands::RandomOptionView> {
    commands::list_random_options()
}

#[tauri::command]
pub fn list_random_option_candidates(
    part: domain::EquipmentPart,
    slot: domain::PartSlot,
    main_skill_id: Option<String>,
) -> Vec<commands::RandomOptionCandidate> {
    commands::list_random_option_candidates(part, slot, main_skill_id)
}

#[tauri::command]
pub fn can_separate_measurement(attacks: Vec<Option<i64>>) -> bool {
    commands::can_separate_measurement(attacks)
}

#[tauri::command]
pub fn list_masteries() -> Vec<domain::MasteryDef> {
    commands::list_masteries()
}

#[tauri::command]
pub fn list_siena_kinds() -> domain::SienaCatalog {
    commands::list_siena_kinds()
}

#[tauri::command]
pub fn list_character_skills() -> Vec<domain::CharacterSkillDef> {
    commands::list_character_skills()
}

#[tauri::command]
pub fn resolve_character_skill_effects(
    masteries: domain::Masteries,
) -> Vec<CharacterSkillEffectsView> {
    commands::resolve_character_skill_effects(masteries)
}

#[tauri::command]
pub fn list_titles() -> Vec<TitleView> {
    commands::list_titles()
}

#[tauri::command]
pub fn list_characters(state: State<'_, AppState>) -> CommandResult<Vec<RegisteredCharacter>> {
    with_repo(&state, |repo| repo.list())
}

#[tauri::command]
pub fn list_buff_sets(state: State<'_, AppState>) -> CommandResult<Vec<BuffSet>> {
    with_repo(&state, |repo| repo.list_buff_sets())
}

#[tauri::command]
pub fn create_buff_set(
    state: State<'_, AppState>,
    name: String,
    choices: BuffSelection,
) -> CommandResult<BuffSet> {
    with_repo(&state, |repo| {
        repo.create_buff_set(&name, &choices, &gamedata::buff_catalog())
    })
}

#[tauri::command]
pub fn update_buff_set(
    state: State<'_, AppState>,
    id: i64,
    name: String,
    choices: BuffSelection,
) -> CommandResult<BuffSet> {
    with_repo(&state, |repo| {
        repo.update_buff_set(id, &name, &choices, &gamedata::buff_catalog())
    })
}

#[tauri::command]
pub fn duplicate_buff_set(state: State<'_, AppState>, id: i64) -> CommandResult<BuffSet> {
    with_repo(&state, |repo| repo.duplicate_buff_set(id))
}

#[tauri::command]
pub fn delete_buff_set(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    with_repo(&state, |repo| repo.delete_buff_set(id))
}

#[tauri::command]
pub fn set_default_buff_set(
    state: State<'_, AppState>,
    character_id: i64,
    buff_set_id: Option<i64>,
) -> CommandResult<RegisteredCharacter> {
    with_repo(&state, |repo| {
        repo.set_default_buff_set(character_id, buff_set_id)?;
        repo.get(character_id)
    })
}

#[tauri::command]
pub fn create_character(
    state: State<'_, AppState>,
    character: NewCharacter,
) -> CommandResult<RegisteredCharacter> {
    if gamedata::find_character(&character.game_character_id).is_none() {
        return Err(CommandError::from(format!(
            "ゲームキャラ '{}' は未登録です",
            character.game_character_id
        )));
    }
    commands::validate_main_skill(&character)?;
    with_repo(&state, |repo| {
        repo.create(
            &character,
            &gamedata::buff_catalog(),
            &gamedata::equipment_catalog(),
            &gamedata::equipment_abilities(),
            &gamedata::random_option_catalog(),
            &gamedata::title_catalog(),
            gamedata::character_skill_catalog(),
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
        return Err(CommandError::from(format!(
            "ゲームキャラ '{}' は未登録です",
            character.game_character_id
        )));
    }
    commands::validate_main_skill(&character)?;
    with_repo(&state, |repo| {
        repo.update(
            id,
            &character,
            &gamedata::buff_catalog(),
            &gamedata::equipment_catalog(),
            &gamedata::equipment_abilities(),
            &gamedata::random_option_catalog(),
            &gamedata::title_catalog(),
            gamedata::character_skill_catalog(),
        )
    })
}

#[tauri::command]
pub fn delete_character(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    with_repo(&state, |repo| repo.delete(id))
}

/// 登録キャラの表示画像。保存済み PNG をそのまま data URL にして返す(端末内だけで使う)。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterIconDto {
    pub character_id: i64,
    pub data_url: String,
}

impl From<CharacterIcon> for CharacterIconDto {
    fn from(icon: CharacterIcon) -> Self {
        Self {
            character_id: icon.character_id,
            data_url: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(icon.png)
            ),
        }
    }
}

#[tauri::command]
pub fn list_character_icons(state: State<'_, AppState>) -> CommandResult<Vec<CharacterIconDto>> {
    let icons = with_repo(&state, |repo| repo.list_character_icons())?;
    Ok(icons.into_iter().map(CharacterIconDto::from).collect())
}

#[tauri::command]
pub fn set_character_icon(
    state: State<'_, AppState>,
    character_id: i64,
    source: Vec<u8>,
) -> CommandResult<CharacterIconDto> {
    let icon = with_repo(&state, |repo| repo.set_character_icon(character_id, &source))?;
    Ok(CharacterIconDto::from(icon))
}

#[tauri::command]
pub fn reset_character_icon(state: State<'_, AppState>, character_id: i64) -> CommandResult<()> {
    with_repo(&state, |repo| repo.reset_character_icon(character_id))
}

#[tauri::command]
pub fn get_damage_snapshot(
    state: State<'_, AppState>,
    character_id: i64,
) -> CommandResult<Option<DamageSnapshot>> {
    with_repo(&state, |repo| repo.get_damage_snapshot(character_id))
}

#[tauri::command]
pub fn set_damage_snapshot(
    state: State<'_, AppState>,
    character_id: i64,
    skill_id: String,
    content_id: String,
    per_hit: i64,
) -> CommandResult<DamageSnapshot> {
    with_repo(&state, |repo| {
        repo.set_damage_snapshot(character_id, &skill_id, &content_id, per_hit)
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn preview_effective_stats(
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    buffs: BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    main_skill_id: Option<String>,
) -> CommandResult<commands::StatPreviewPayload> {
    commands::preview_effective_stats(
        base_stats,
        stat_sources,
        buffs,
        equipment,
        common_skills,
        awakening,
        main_skill_id,
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn buff_target_stat_gains(
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    buffs: BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    buff_id: String,
) -> CommandResult<Vec<domain::BuffTargetStatGain>> {
    commands::buff_target_stat_gains(
        base_stats,
        stat_sources,
        buffs,
        equipment,
        common_skills,
        awakening,
        buff_id,
    )
}

#[tauri::command]
pub fn preview_defense(
    character: NewCharacter,
    buffs: BuffSelection,
) -> CommandResult<DefenseProfile> {
    commands::preview_defense(character, buffs)
}

#[tauri::command]
pub fn preview_versus(
    attacker: NewCharacter,
    attacker_buffs: BuffSelection,
    skill_id: String,
    defender: NewCharacter,
    defender_buffs: BuffSelection,
    attacker_tries: Vec<GrowthAction>,
    defender_tries: Vec<GrowthAction>,
) -> CommandResult<VersusAccuracy> {
    commands::preview_versus(
        attacker, attacker_buffs, skill_id, defender, defender_buffs, attacker_tries, defender_tries,
    )
}

#[tauri::command]
pub fn get_stat_limits() -> domain::StatLimits {
    commands::get_stat_limits()
}

#[tauri::command]
pub fn get_game_tables() -> GameTablesPayload {
    commands::get_game_tables()
}

#[tauri::command]
pub fn get_new_character_stat_sources() -> domain::StatSources {
    commands::get_new_character_stat_sources()
}

#[tauri::command]
pub fn get_new_character_common_skills() -> domain::CommonSkills {
    commands::get_new_character_common_skills()
}

#[tauri::command]
pub fn retain_character_skills(skill_ids: Vec<String>, game_character_id: String) -> Vec<String> {
    commands::retain_character_skills(skill_ids, game_character_id)
}

/// 登録済みキャラでダメージ計算する。DB からキャラを引くのはここだけで、
/// 引いたあとの計算は `commands` crate(Web 版と共通)に任せる。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn calculate_damage(
    state: State<'_, AppState>,
    character_id: i64,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    normal_attack_id: Option<String>,
    temporary_adjustments: Option<domain::Adjustments>,
    buffs: BuffSelection,
) -> CommandResult<DamageResult> {
    let character = with_repo(&state, |repo| repo.get(character_id))?;
    commands::damage_for_character(
        &character.base_stats,
        &character.game_character_id,
        character.main_skill_id.as_deref(),
        &character.stat_sources,
        &buffs,
        character.equipment,
        character.common_skills,
        character.awakening,
        &skill_id,
        &content_id,
        combo_count,
        combo_skill_type,
        normal_attack_id.as_deref(),
        temporary_adjustments,
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn preview_damage(
    character: NewCharacter,
    buffs: BuffSelection,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    normal_attack_id: Option<String>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageResult> {
    commands::preview_damage(
        character,
        buffs,
        skill_id,
        content_id,
        combo_count,
        combo_skill_type,
        normal_attack_id,
        temporary_adjustments,
    )
}

#[tauri::command]
pub fn evaluate_contents(
    character: NewCharacter,
    buffs: BuffSelection,
    dependency_skill_id: Option<String>,
) -> CommandResult<Vec<ContentEvaluation>> {
    commands::evaluate_contents(character, buffs, dependency_skill_id)
}

#[tauri::command]
pub fn list_upgrade_candidates(
    character: NewCharacter,
    buffs: BuffSelection,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<Vec<UpgradeCandidate>> {
    commands::list_upgrade_candidates(
        character,
        buffs,
        skill_id,
        content_id,
        combo_count,
        combo_skill_type,
        temporary_adjustments,
    )
}

#[tauri::command]
pub fn list_enchant_gains(
    character: NewCharacter,
    buffs: BuffSelection,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<Vec<EnchantGain>> {
    commands::list_enchant_gains(
        character,
        buffs,
        skill_id,
        content_id,
        combo_count,
        combo_skill_type,
        temporary_adjustments,
    )
}
