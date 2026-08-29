//! フロントエンドから呼ばれるコマンド。ロジックは書かない。エラーは `CommandError` に変換して返す。

use domain::{
    evaluate_contents_for_character, AttackPowerCoefficients, BuffDefinition, BuffSelection,
    CommonSkills, Content, ContentArea, ContentEvaluation, DamageInput, DamageMaterial,
    DamageResult, DefenseProfile, DependencyCoefficients, Enemy, EquipmentAbilityDef,
    EquipmentPart, RandomOptionDef, Skill, SkillEvaluationInput, TitleDef, WristBonusMaterial,
};
use gamedata::{EquipmentItem, GameCharacter};
use storage::{BuffSet, CharacterRepository, NewCharacter, RegisteredCharacter};
use tauri::{Manager, State};

use crate::{AppInfo, AppState};

type CommandResult<T> = Result<T, CommandError>;

/// フロントに返すエラー。文言だけでなく「どこの話か」(装備の部位・アビリティ)も運ぶ。
/// エラー帯はこの `location` を使って該当部位の詳細まで飛ぶ。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub message: String,
    pub location: Option<domain::ValidationLocation>,
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self {
            message,
            location: None,
        }
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_string(),
            location: None,
        }
    }
}

impl From<domain::ValidationError> for CommandError {
    fn from(error: domain::ValidationError) -> Self {
        Self {
            message: error.message,
            location: error.location,
        }
    }
}

impl From<storage::StorageError> for CommandError {
    fn from(error: storage::StorageError) -> Self {
        match error {
            storage::StorageError::InvalidValue(invalid) => Self {
                message: format!("不正な値: {}", invalid.message),
                location: invalid.location,
            },
            other => Self {
                message: other.to_string(),
                location: None,
            },
        }
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
    f(&repo).map_err(CommandError::from)
}

fn find_skill(skill_id: &str) -> CommandResult<Skill> {
    gamedata::find_skill(skill_id)
        .ok_or_else(|| format!("スキル '{skill_id}' が見つかりません").into())
}

fn find_enemy(enemy_id: &str) -> CommandResult<Enemy> {
    gamedata::find_enemy(enemy_id).ok_or_else(|| format!("敵 '{enemy_id}' が見つかりません").into())
}

/// 保存前のキャラデータ(draft)を検証する。DB には書き込まないプレビュー系コマンド
/// (preview_elements / preview_defense / preview_damage / evaluate_contents)専用。
///
/// `storage::character_repository::validate` と同じ検証内容だが、こちらは永続化を経由しないので
/// `storage` を呼ばず domain の検証(`Equipment::validate_against_catalog` を含む)を直接呼ぶ
/// (`storage::character_repository::validate` は登録・更新の保存直前チェック用)。
fn validate_character_draft(character: &NewCharacter, buffs: &BuffSelection) -> CommandResult<()> {
    if character.name.trim().is_empty() {
        return Err("名前が空です".into());
    }
    character.base_stats.validate().map_err(|e| e.to_string())?;
    if character.awakening.stage > domain::Awakening::MAX_STAGE {
        return Err(format!("覚醒段階は 0〜{} です", domain::Awakening::MAX_STAGE).into());
    }
    if character.awakening.eternal_level > domain::Awakening::MAX_ETERNAL_LEVEL {
        return Err(format!(
            "エタの意志 Lv は 0〜{} です",
            domain::Awakening::MAX_ETERNAL_LEVEL
        )
        .into());
    }
    character
        .stat_sources
        .validate()
        .map_err(|e| e.to_string())?;
    character
        .stat_sources
        .character_skills
        .validate(
            gamedata::character_skill_catalog(),
            &character.game_character_id,
        )
        .map_err(|e| e.to_string())?;
    domain::stat_sources::build_modifiers(
        &character.stat_sources,
        buffs,
        &gamedata::buff_catalog(),
    )
    .map_err(|e| e.to_string())?;
    character.equipment.validate().map_err(|e| e.to_string())?;
    character
        .common_skills
        .validate()
        .map_err(|e| e.to_string())?;
    character.equipment.validate_against_catalog(
        &gamedata::equipment_catalog(),
        &gamedata::equipment_abilities(),
        &gamedata::random_option_catalog(),
    )?;
    // 称号は装備部位ではないので部位ループの外で見る(1 枠・カタログ参照のみ)
    if let Some(id) = &character.equipment.title {
        if !gamedata::title_catalog()
            .iter()
            .any(|t| t.id == id.as_str())
        {
            return Err(format!("未知の称号 '{id}' です").into());
        }
    }
    Ok(())
}

/// 計算対象のコンテンツを引く。敵データが無いコンテンツはダメージ計算の対象にできない。
fn find_content(content_id: &str) -> CommandResult<Content> {
    let content = gamedata::content_areas()
        .into_iter()
        .flat_map(|area| area.contents)
        .find(|c| c.id == content_id)
        .ok_or_else(|| CommandError::from(format!("コンテンツ '{content_id}' が見つかりません")))?;
    if content.enemy_id.is_none() {
        return Err(format!("コンテンツ '{content_id}' には敵データがありません").into());
    }
    Ok(content)
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

/// バフセット単体の与ダメージカテゴリ合計。ゲームUIと同じカテゴリ名・上限を使う。
#[tauri::command]
pub fn summarize_buff_selection(buffs: BuffSelection) -> CommandResult<Vec<domain::CategoryTrace>> {
    domain::summarize_buff_selection(&buffs, &gamedata::buff_catalog())
        .map_err(|e| e.to_string().into())
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
    validate_character_draft(&character, &BuffSelection::default())?;
    Ok(gamedata::element_preview(
        &character.game_character_id,
        &character.equipment,
        &character.stat_sources,
    ))
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

/// マスタリーのカタログ(wiki: 各キャラの Skill ページ、スキル表の `P (M1)`〜`(M4)`)。
/// 段ごとに 1 つだけ選ぶ。キャラでの絞り込みは UI 側で `game_character_id` を見て行う。
#[tauri::command]
pub fn list_masteries() -> Vec<domain::MasteryDef> {
    gamedata::mastery_catalog().to_vec()
}

/// シエナのオーラで選べる能力値・追加オプションのカタログ(wiki: 装備システム/シエナのオーラ)。
/// 中身は再抽選のランダム値なので、静的データとして持てるのは**種類と値域**だけ。
#[tauri::command]
pub fn list_siena_kinds() -> domain::SienaCatalog {
    domain::siena_catalog()
}

/// キャラスキルのカタログ(パッシブ・自己バフ・味方バフ)。キャラでの絞り込みは
/// UI 側で `game_character_id` と `audience` を見て行う(味方スキルは誰でも ON にできる)。
#[tauri::command]
pub fn list_character_skills() -> Vec<domain::CharacterSkillDef> {
    gamedata::character_skill_catalog().to_vec()
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

/// 主軸スキル(攻撃力の依存種別を決める)はそのキャラのスキル一覧に含まれている必要がある。
/// キャラ種を変えたときに前キャラのスキルが残るのを防ぐ。未選択(`None`)は許す。
fn validate_main_skill(character: &NewCharacter) -> CommandResult<()> {
    let Some(skill_id) = &character.main_skill_id else {
        return Ok(());
    };
    if !gamedata::skills_for(&character.game_character_id)
        .iter()
        .any(|s| &s.id == skill_id)
    {
        return Err(CommandError::from(format!(
            "主軸スキル '{skill_id}' は '{}' のスキルではありません",
            character.game_character_id
        )));
    }
    Ok(())
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
    validate_main_skill(&character)?;
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
            gamedata::character_skill_catalog(),
        )
    })
}

#[tauri::command]
pub fn delete_character(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    with_repo(&state, |repo| repo.delete(id))
}

/// キャラの主軸スキルから攻撃力(A)の係数一式を引く。未選択なら `None`(攻撃力を出さない)。
fn attack_coefficients_of(
    main_skill_id: Option<&str>,
) -> CommandResult<Option<AttackPowerCoefficients>> {
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
    buffs: BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    main_skill_id: Option<String>,
) -> CommandResult<domain::StatPreview> {
    let coefficients = attack_coefficients_of(main_skill_id.as_deref())?;
    domain::preview_effective_stats(
        &base_stats,
        &stat_sources,
        &buffs,
        &equipment,
        &common_skills,
        &gamedata::buff_catalog(),
        gamedata::mastery_catalog(),
        gamedata::character_skill_catalog(),
        &gamedata::equipment_abilities(),
        &gamedata::title_catalog(),
        coefficients,
        gamedata::awakening_caps(awakening).max_stat,
    )
    .map_err(|e| e.to_string().into())
}

/// 防御側の戦闘能力値(docs/damage-formula.md §6〜7)。保存前のキャラデータで出す。
///
/// 与ダメージ式とは別経路なので対象コンテンツを取らない。装備補正 9 値は
/// 基本能力値 + 強化能力値(地域なし = テシスコアを含まない)の合計を渡す。
#[tauri::command]
pub fn preview_defense(
    character: NewCharacter,
    buffs: BuffSelection,
) -> CommandResult<DefenseProfile> {
    validate_character_draft(&character, &buffs)?;
    let preview = domain::preview_effective_stats(
        &character.base_stats,
        &character.stat_sources,
        &buffs,
        &character.equipment,
        &character.common_skills,
        &gamedata::buff_catalog(),
        gamedata::mastery_catalog(),
        gamedata::character_skill_catalog(),
        &gamedata::equipment_abilities(),
        &gamedata::title_catalog(),
        None,
        gamedata::awakening_caps(character.awakening).max_stat,
    )
    .map_err(|e| e.to_string())?;
    // preview と同じ基本能力値(装備 + アビリティ + 称号 + ソウルリンク)を使う。
    // ソウルリンクはエンチャントではなく基本能力値へ直接加算する。
    let equipment_totals = preview
        .equipment_base_total
        .add(character.equipment.enhanced_totals(None));
    Ok(domain::defense_profile(
        &preview.stats,
        &equipment_totals,
        gamedata::awakening_caps(character.awakening),
        &character
            .equipment
            .random_option_totals(&gamedata::random_option_catalog()),
        // 装備防御力倍率(共通スキル + シエナのオーラの防御力増加)。
        // リンゴの島・ベリネンルミは常に 100% だが、防御タブは対象コンテンツを取らないので
        // ここでは習得どおりの倍率で出す(その注記は UI 側で出す)
        character
            .common_skills
            .defense_rates(character.equipment.siena_defense_rate()),
    ))
}

#[tauri::command]
pub fn get_stat_limits() -> domain::StatLimits {
    domain::stat_sources::stat_limits()
}

#[tauri::command]
pub fn get_new_character_stat_sources() -> domain::StatSources {
    domain::StatSources::for_new_character()
}

/// 武器の装備強化による追加固定ダメージ(wiki: 装備システム/装備強化、docs/damage-formula.md §5)。
///
/// `item_id` → カタログの `weapon_class` → 系統ごとの補正式、の順で解決する。
/// - 強化 Lv 0 は 0
/// - +1〜+11 は確定倍率で式から算出
/// - +12 以上は選択等級の確率区分上端で算出する
fn weapon_added_damage(weapon: &EquipmentPart) -> i64 {
    if weapon.enhance_level == 0 {
        return 0;
    }
    let enhance_type = weapon.enhance_type.or_else(|| {
        weapon
            .item_id
            .as_deref()
            .and_then(gamedata::equipment_enhance_type)
    });
    let Some(rates) = enhance_type.and_then(gamedata::enhance_rates_for_type) else {
        return 0;
    };
    let values = weapon.base.add(weapon.enchant);
    if let Some(multiplier) = gamedata::enhance_multiplier(weapon.enhance_level) {
        return domain::weapon_added_damage(&values, &rates, multiplier);
    }
    let Some(grade) = weapon.enhance_grade else {
        return 0;
    };
    let multiplier = gamedata::enhance_grade_multiplier(weapon.enhance_level, grade).unwrap_or(0.0);
    domain::weapon_added_damage(&weapon.base.add(weapon.enchant), &rates, multiplier)
}

#[cfg(test)]
fn armor_added_hp(armor: &EquipmentPart) -> i64 {
    if armor.enhance_level == 0 {
        return 0;
    }
    let enhance_type = armor.enhance_type.or_else(|| {
        armor
            .item_id
            .as_deref()
            .and_then(gamedata::equipment_enhance_type)
    });
    let Some(class) = enhance_type.and_then(gamedata::armor_class_for_type) else {
        return 0;
    };
    let multiplier =
        gamedata::armor_enhance_multiplier(armor.enhance_level, armor.enhance_grade).unwrap_or(0.0);
    let values = armor.base.add(armor.enchant);
    let rates = gamedata::armor_enhance_rates(class);
    domain::armor_added_hp(
        &values,
        rates.physical_defense,
        rates.magic_defense,
        multiplier,
    )
}

/// スキル依存種別ごとに変わらない攻撃力/装備攻撃力/命中Pの係数を gamedata から解決する。
fn dependency_coefficients(dependency: domain::SkillDependency) -> DependencyCoefficients {
    DependencyCoefficients {
        attack: gamedata::attack_coefficients(dependency),
        equipment: gamedata::equipment_coefficients(dependency),
        accuracy: gamedata::accuracy_correction(dependency),
    }
}

fn resolve_combo_skill_type(
    skill: Skill,
    equipment: &domain::Equipment,
    combo_skill_type: Option<domain::ComboSkillType>,
) -> CommandResult<Skill> {
    match combo_skill_type {
        Some(combo_type) => skill
            .resolve_combo_variant(combo_type, equipment.siena_actual_delay_reduction())
            .map_err(|e| e.to_string().into()),
        None => Ok(skill),
    }
}

/// 与ダメージ計算のうち、スキル・敵・コンテンツによらない共通材料を組み立てる
/// (calculate_damage / preview_damage / evaluate_contents 共通。`domain::DamageMaterial` 参照)。
fn build_damage_material(
    base_stats: &domain::BaseStats,
    stat_sources: &domain::StatSources,
    buffs: &BuffSelection,
    equipment: &domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    temporary_adjustments: Option<&domain::Adjustments>,
) -> CommandResult<DamageMaterial> {
    let (mut stat_modifiers, mut stat_contributions) =
        domain::stat_sources::build_modifiers(stat_sources, buffs, &gamedata::buff_catalog())
            .map_err(|e| e.to_string())?;
    domain::stat_sources::apply_siena_stats(
        &mut stat_modifiers,
        &mut stat_contributions,
        equipment,
    );
    domain::stat_sources::apply_masteries(
        &mut stat_modifiers,
        &mut stat_contributions,
        &stat_sources.masteries,
        gamedata::mastery_catalog(),
    );
    domain::stat_sources::apply_character_skills(
        &mut stat_modifiers,
        &mut stat_contributions,
        &stat_sources.character_skills,
        &stat_sources.masteries,
        gamedata::character_skill_catalog(),
    );
    domain::stat_sources::apply_unleash(
        &mut stat_modifiers,
        &mut stat_contributions,
        &common_skills,
    );
    if let Some(temp) = temporary_adjustments {
        temp.validate().map_err(|e| e.to_string())?;
        domain::stat_sources::apply_temporary_adjustments(
            &mut stat_modifiers,
            &mut stat_contributions,
            temp,
        );
    }
    let weapon_added_damage = equipment
        .parts
        .weapon
        .selected()
        .map(weapon_added_damage)
        .unwrap_or(0);
    // リンクステータス7は、武器強化で丸め終えた追加固定ダメージへ倍率を掛けて再度切り捨てる。
    // リンクステータス8は鎧の追加HPであり、与ダメージには加えない。
    let added_damage = stat_sources
        .soul_link
        .weapon_added_damage(weapon_added_damage);
    Ok(DamageMaterial {
        base_stats: base_stats.clone(),
        stat_modifiers,
        stat_contributions,
        equipment: equipment.clone(),
        common_skills,
        random_options: equipment.random_option_totals(&gamedata::random_option_catalog()),
        weapon_added_damage: added_damage,
        awakening_rate: gamedata::awakening_rate(awakening),
        damage_cap: gamedata::awakening_caps(awakening).max_damage,
        stat_cap: gamedata::awakening_caps(awakening).max_stat,
        actual_delay_skills: gamedata::actual_delay_contributions(
            &stat_sources.character_skills,
            &stat_sources.masteries,
        ),
        critical_rate_sources: stat_sources.critical_rate,
        skill_uses: gamedata::skill_uses_table(),
    })
}

/// ダメージ計算の入力を組み立てる(calculate_damage / preview_damage 共通)。
#[allow(clippy::too_many_arguments)]
fn build_damage_input(
    base_stats: &domain::BaseStats,
    game_character_id: &str,
    character_style_dependency: Option<domain::SkillDependency>,
    stat_sources: &domain::StatSources,
    buffs: &BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    skill: Skill,
    enemy: Enemy,
    content: &domain::Content,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageInput> {
    let material = build_damage_material(
        base_stats,
        stat_sources,
        buffs,
        &equipment,
        common_skills,
        awakening,
        temporary_adjustments.as_ref(),
    )?;
    let skill = resolve_combo_skill_type(skill, &equipment, combo_skill_type)?;
    let equipment_catalog = gamedata::equipment_catalog();
    let mut equipment_base_sources =
        equipment.base_sources(&gamedata::equipment_abilities(), &gamedata::title_catalog());
    if let Some(source) = stat_sources.soul_link.equipment_source() {
        equipment_base_sources.push(source);
    }
    let wrist_bonus = gamedata::character_wrist_base_bonus(
        game_character_id,
        base_stats,
        character_style_dependency.unwrap_or(skill.dependency),
        &equipment,
        &equipment_catalog,
    );
    if wrist_bonus != domain::EquipmentValues::default() {
        equipment_base_sources.push(domain::EquipmentValueSource {
            source: "手首補正".to_string(),
            values: wrist_bonus,
        });
    }
    let equipment_enhanced_sources = equipment.enhanced_sources(content.core_region);
    let title_damage_rate =
        domain::title_attack_damage_rate(equipment.title.as_deref(), &gamedata::title_catalog());
    let title_added_damage_rate = domain::title_added_damage_rate(
        equipment.title.as_deref(),
        &gamedata::title_catalog(),
        content.game_region,
        content.enemy_id.as_deref(),
    );
    let damage_contributions =
        gamedata::damage_contributions_of(stat_sources, buffs, &equipment, skill.dependency);
    let element_value =
        gamedata::element_value_for(game_character_id, &equipment, stat_sources, &skill);
    let coefficients = dependency_coefficients(skill.dependency);
    Ok(material.build_input(
        skill,
        enemy,
        combo_count,
        temporary_adjustments,
        coefficients,
        equipment_base_sources,
        equipment_enhanced_sources,
        title_damage_rate,
        title_added_damage_rate,
        damage_contributions,
        element_value,
    ))
}

#[tauri::command]
pub fn calculate_damage(
    state: State<'_, AppState>,
    character_id: i64,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<domain::Adjustments>,
    buffs: BuffSelection,
) -> CommandResult<DamageResult> {
    let character = with_repo(&state, |repo| repo.get(character_id))?;
    let style_dependency = character
        .main_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(|skill| skill.dependency);
    let content = find_content(&content_id)?;
    let enemy = find_enemy(content.enemy_id.as_deref().unwrap_or_default())?;
    let input = build_damage_input(
        &character.base_stats,
        &character.game_character_id,
        style_dependency,
        &character.stat_sources,
        &buffs,
        character.equipment,
        character.common_skills,
        character.awakening,
        find_skill(&skill_id)?,
        enemy,
        &content,
        combo_count,
        combo_skill_type,
        temporary_adjustments,
    )?;
    Ok(domain::calculate_damage(&input))
}

/// 保存前のキャラデータ(編集中 draft・試し変更)でダメージ計算する。DB には書き込まない。
#[tauri::command]
pub fn preview_damage(
    character: NewCharacter,
    buffs: BuffSelection,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageResult> {
    validate_character_draft(&character, &buffs)?;
    let content = find_content(&content_id)?;
    let enemy = find_enemy(content.enemy_id.as_deref().unwrap_or_default())?;
    let style_dependency = character
        .main_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(|skill| skill.dependency);
    let input = build_damage_input(
        &character.base_stats,
        &character.game_character_id,
        style_dependency,
        &character.stat_sources,
        &buffs,
        character.equipment,
        character.common_skills,
        character.awakening,
        find_skill(&skill_id)?,
        enemy,
        &content,
        combo_count,
        combo_skill_type,
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
    buffs: BuffSelection,
    dependency_skill_id: Option<String>,
) -> CommandResult<Vec<ContentEvaluation>> {
    validate_character_draft(&character, &buffs)?;
    // 後段のループで繰り返し使う(下の「評価ループの不変値」コメント参照)。
    let equipment_catalog = gamedata::equipment_catalog();
    let equipment_abilities = gamedata::equipment_abilities();
    let titles = gamedata::title_catalog();
    let skills = gamedata::skills_for(&character.game_character_id);
    let enemies = gamedata::enemies();
    // コンテンツの enemy_id は敵カタログに必ず存在する(gamedata のテストで担保)。
    // ここで一括検証し、domain 側のループは検索失敗を気にしなくてよいようにする。
    for area in gamedata::content_areas() {
        for content in &area.contents {
            if let Some(enemy_id) = content.enemy_id.as_deref() {
                if !enemies.iter().any(|e| e.id == enemy_id) {
                    return Err(format!("敵 '{enemy_id}' が見つかりません").into());
                }
            }
        }
    }

    // 評価ループの不変値(キャラのみ依存)は 1 回だけ構築する。コンテンツ×スキルごとに
    // カタログとステ補正を再構築すると、この最重量パスで無駄な再計算になる(PR レビュー指摘)。
    // 計算タブ(build_damage_input)と同じ材料構築を通るため、キャラスキルのステ補正も適用する。
    let material = build_damage_material(
        &character.base_stats,
        &character.stat_sources,
        &buffs,
        &character.equipment,
        character.common_skills,
        character.awakening,
        None,
    )?;
    let mut equipment_base_sources_raw = character
        .equipment
        .base_sources(&equipment_abilities, &titles);
    if let Some(source) = character.stat_sources.soul_link.equipment_source() {
        equipment_base_sources_raw.push(source);
    }
    let character_style_dependency = character
        .main_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(|skill| skill.dependency);
    let wrist_bonus = WristBonusMaterial {
        style_dependency_override: character_style_dependency,
        ..gamedata::character_wrist_bonus_material(
            &character.game_character_id,
            &character.equipment,
            &equipment_catalog,
        )
    };
    // スキルごとに変わるがコンテンツには依存しない値(依存種別の係数・カテゴリ寄与・
    // 属性値)は、コンテンツの数だけ繰り返さずキャラのスキル数ぶんだけ 1 回作る。
    let skill_inputs: Vec<SkillEvaluationInput> = skills
        .iter()
        .map(|skill| SkillEvaluationInput {
            skill: skill.clone(),
            coefficients: dependency_coefficients(skill.dependency),
            damage_contributions: gamedata::damage_contributions_of(
                &character.stat_sources,
                &buffs,
                &character.equipment,
                skill.dependency,
            ),
            element_value: gamedata::element_value_for(
                &character.game_character_id,
                &character.equipment,
                &character.stat_sources,
                skill,
            ),
        })
        .collect();
    // 呼び出し側がスキルを指定したら、装備条件の比較先はそのスキルの依存で固定する。
    let fixed_dependency = match dependency_skill_id {
        None => None,
        Some(id) => Some(find_skill(&id)?.dependency),
    };
    Ok(evaluate_contents_for_character(
        &material,
        &gamedata::content_areas(),
        &enemies,
        &skill_inputs,
        equipment_base_sources_raw,
        wrist_bonus,
        &titles,
        character.awakening,
        fixed_dependency,
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        armor_added_hp, build_damage_input, resolve_combo_skill_type, weapon_added_damage,
    };
    use domain::{
        BaseStats, BuffSelection, ComboSkillType, CommonSkills, DamageCategory, EnhanceGrade,
        Equipment, EquipmentEnhanceType, EquipmentPart, EquipmentValues, SoulLinkStatus,
        StatSources,
    };

    #[test]
    fn api境界は未対応スキルへのコンボタイプ指定を拒否する() {
        let skill = gamedata::find_skill("maximin_moonlight_sword").unwrap();
        let error =
            resolve_combo_skill_type(skill, &Equipment::default(), Some(ComboSkillType::General))
                .unwrap_err();
        assert!(error.message.contains("対応していません"));
    }

    // 刀(HACK系: 斬×6.67 + 突×1.00)・突100/斬300 → INT(300×6.67+100) = 2101
    fn weapon(item_id: Option<&str>, level: u8, grade: Option<EnhanceGrade>) -> EquipmentPart {
        EquipmentPart {
            item_id: item_id.map(String::from),
            enhance_level: level,
            enhance_grade: grade,
            base: EquipmentValues {
                thrust: 100,
                slash: 300,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn 強化なしは追加固定ダメージ0() {
        assert_eq!(
            weapon_added_damage(&weapon(Some("abyss-scimitar"), 0, None)),
            0
        );
    }

    #[test]
    fn カタログ外武器は式を特定できないため0() {
        assert_eq!(weapon_added_damage(&weapon(None, 5, None)), 0);
        assert_eq!(
            weapon_added_damage(&weapon(None, 12, Some(EnhanceGrade::Highest))),
            0
        );
    }

    #[test]
    fn 確定倍率帯は系統式から算出する() {
        // +10 倍率 28.8 → INT(2101×28.8) = 60508(偶数なのでそのまま)
        assert_eq!(
            weapon_added_damage(&weapon(Some("abyss-scimitar"), 10, None)),
            60508
        );
    }

    #[test]
    fn 確定倍率帯もエンチャント込みで算出する() {
        let mut value = weapon(Some("abyss-scimitar"), 10, None);
        value.enchant = EquipmentValues {
            thrust: 10,
            slash: 20,
            ..Default::default()
        };
        // INT((110 + 320×6.67) × 28.8) = 64,627、奇数なので 64,626
        assert_eq!(weapon_added_damage(&value), 64_626);
    }

    #[test]
    fn レンジ倍率帯は等級上端を使う() {
        // +12 レンジ上限 280 → INT(2101×280) = 588280(偶数)
        assert_eq!(
            weapon_added_damage(&weapon(
                Some("abyss-scimitar"),
                12,
                Some(EnhanceGrade::Highest)
            )),
            588280
        );
        // +15 レンジ上限 880 → INT(2101×880) = 1848880(偶数)
        assert_eq!(
            weapon_added_damage(&weapon(
                Some("abyss-scimitar"),
                15,
                Some(EnhanceGrade::Highest)
            )),
            1848880
        );
    }

    #[test]
    fn 魔鎧15最上は画像の追加hpになる() {
        let armor = EquipmentPart {
            enhance_level: 15,
            enhance_grade: Some(EnhanceGrade::Highest),
            enhance_type: Some(EquipmentEnhanceType::ArmorMagic),
            base: EquipmentValues {
                physical_defense: 650,
                magic_defense: 510,
                ..Default::default()
            },
            ..Default::default()
        };
        // (650×3.8 + 510×4.0) × 440 = 1,984,400。与ダメージには接続しない。
        assert_eq!(armor_added_hp(&armor), 1_984_400);
    }

    #[test]
    fn 実入力組立てはソウルリンク1から7を一度だけ反映する() {
        let sources = StatSources {
            soul_link: SoulLinkStatus {
                thrust_level: 1,
                slash_level: 2,
                magic_attack_level: 3,
                magic_defense_level: 4,
                critical_damage_level: 1,
                final_damage_level: 1,
                weapon_enhance_level: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut equipment = Equipment::default();
        equipment.parts.weapon = weapon(Some("abyss-scimitar"), 10, None).into();
        let content = gamedata::content_areas()
            .into_iter()
            .flat_map(|area| area.contents)
            .find(|content| content.id == "ringo")
            .unwrap();
        let input = build_damage_input(
            &BaseStats {
                stab: 100,
                hack: 100,
                int: 1,
                def: 1,
                mr: 1,
                dex: 1,
                agi: 1,
            },
            "boris",
            None,
            &sources,
            &BuffSelection::default(),
            equipment,
            CommonSkills::default(),
            domain::Awakening::default(),
            gamedata::find_skill("boris_horizontal_sword").unwrap(),
            gamedata::find_enemy("ringo_boss").unwrap(),
            &content,
            0,
            None,
            None,
        )
        .unwrap();

        let soul_sources: Vec<_> = input
            .equipment_base_sources
            .iter()
            .filter(|source| source.source == "ソウルリンク")
            .collect();
        assert_eq!(soul_sources.len(), 1);
        assert_eq!(
            soul_sources[0].values,
            EquipmentValues {
                thrust: 2,
                slash: 4,
                magic_attack: 6,
                magic_defense: 8,
                ..Default::default()
            }
        );
        let soul_damage: Vec<_> = input
            .damage_contributions
            .iter()
            .filter(|source| source.source == "ソウルリンク")
            .collect();
        assert_eq!(soul_damage.len(), 2);
        assert!(soul_damage.iter().any(|source| {
            source.category == DamageCategory::CriticalDamageRate
                && (source.value - 0.015).abs() < f64::EPSILON
        }));
        assert!(soul_damage.iter().any(|source| {
            source.category == DamageCategory::FinalDamageRate
                && (source.value - 0.04).abs() < f64::EPSILON
        }));
        // 武器+10の丸め済み60,508へリンクLv4の+40%を掛けて切り捨てる。
        assert_eq!(input.weapon_added_damage, 84_711);
    }
}
