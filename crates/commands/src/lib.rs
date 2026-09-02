//! 画面から呼ばれるコマンドのうち、保存(SQLite)に触らないものの中身。
//!
//! デスクトップ固有の箱(Tauri コマンド属性・rusqlite の保存層)に依存しないので
//! wasm32 でビルドできる。
//! Web 版でも同じ計算を動かすために、デスクトップの箱から出してここに置く。
//! desktop 側(`apps/desktop` のコマンド定義)はコマンド属性を付けた薄いラッパになる。

use domain::{
    evaluate_contents_for_character, AttackPowerCoefficients, BuffDefinition, BuffSelection,
    CommonSkills, Content, ContentArea, ContentEvaluation, DamageMaterial, DamageTarget,
    DamageResult, DefenseProfile, DependencyCoefficients, Enemy, EquipmentAbilityDef,
    EquipmentPart, NewCharacter, RandomOptionDef, Skill, SkillEvaluationInput, TitleDef,
    WristBonusMaterial,
};
use gamedata::{EquipmentItem, GameCharacter};

pub type CommandResult<T> = Result<T, CommandError>;

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
/// 保存層(character_repository の validate)と同じ検証内容だが、こちらは永続化を経由しないので
/// domain の検証(`Equipment::validate_against_catalog` を含む)を直接呼ぶ
/// (保存層の validate は登録・更新の保存直前チェック用)。
fn validate_character_draft(character: &NewCharacter, buffs: &BuffSelection) -> CommandResult<()> {
    if character.name.trim().is_empty() {
        return Err("名前が空です".into());
    }
    character.base_stats.validate().map_err(|e| e.to_string())?;
    character.awakening.validate().map_err(|e| e.to_string())?;
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

pub fn list_game_characters() -> Vec<GameCharacter> {
    gamedata::characters().to_vec()
}

/// キャラ種のスキル一覧。主軸候補の順(単体優先 → 継続火力順。`Skill::main_skill_order`)で返す。
pub fn list_skills(game_character_id: String) -> Vec<Skill> {
    let mut skills = gamedata::skills_for(&game_character_id);
    skills.sort_by(Skill::main_skill_order);
    skills
}

pub fn list_enemies() -> Vec<Enemy> {
    gamedata::enemies()
}

/// バフカタログ 1 件 + 画面が要る派生値(ON 時の初期選択・火力グループ)。派生の規則は domain。
#[derive(Debug, Clone, serde::Serialize)]
pub struct BuffView {
    #[serde(flatten)]
    pub def: BuffDefinition,
    /// ON にしたときの初期選択(対象ステは画面が入れる)
    pub default_choice: domain::BuffChoice,
    pub damage_groups: Vec<domain::BuffDamageGroup>,
}

pub fn list_buff_catalog() -> Vec<BuffView> {
    gamedata::buff_catalog()
        .into_iter()
        .map(|def| BuffView {
            default_choice: def.default_choice(None),
            damage_groups: def.damage_groups(),
            def,
        })
        .collect()
}

/// 「付けたらいくつ効くか」の材料: 極限スキル 3 種すべての効果(いまのスーパー / ハイパー
/// リミット前提)と、ソウルリンクの効いている量。計算タブの地力の試し変更が使う
#[derive(Debug, Clone, serde::Serialize)]
pub struct PotentialEffects {
    pub ultimate: domain::UltimateSkillPreview,
    pub soul_link: domain::SoulLinkPreview,
}

pub fn preview_potential_effects(
    stat_sources: domain::StatSources,
    common_skills: CommonSkills,
) -> PotentialEffects {
    PotentialEffects {
        ultimate: domain::UltimateSkillPreview::potential(&common_skills.ultimate),
        soul_link: stat_sources.soul_link.preview(),
    }
}

/// 排他枠の衝突で選べないバフ(計算タブのバフチップ用。バフタブは summarize の応答に同じものが入る)。
pub fn list_blocked_buffs(buffs: BuffSelection) -> Vec<domain::BlockedBuff> {
    domain::blocked_buffs(&buffs, &gamedata::buff_catalog())
}

/// バフセット単体の与ダメージカテゴリ合計 + バフ別配賦。ゲームUIと同じカテゴリ名・上限を使う。
pub fn summarize_buff_selection(buffs: BuffSelection) -> CommandResult<domain::BuffDamageSummary> {
    domain::summarize_buff_selection(&buffs, &gamedata::buff_catalog())
        .map_err(|e| e.to_string().into())
}

/// 属性値の供給源カタログ(装備の属性強化以外。ペット / モンスターカード / ルーン /
/// 頭アビリティ / カフスアビリティ)。
pub fn list_element_sources() -> Vec<domain::ElementSourceDef> {
    gamedata::element_source_catalog().to_vec()
}

/// 装備の属性強化の合計(部位ごとに +9。対象属性は呼び出し側が決める — キャラ画面は
/// 「全部位の属性が一致しているか」を見た draft 編集中の選択を渡す)。
/// 計算は `Equipment::element_values` そのもの(フロントに再実装を持たせない)。
pub fn equipment_element_values(
    equipment: domain::Equipment,
    element: Option<domain::Element>,
) -> domain::ElementValues {
    equipment.element_values(element)
}

/// 属性値の内訳(キャラ基礎 / 装備の属性強化 / 装備以外の供給源 / 合計)。保存前のキャラデータで出す。
pub fn preview_elements(character: NewCharacter) -> CommandResult<domain::ElementPreview> {
    validate_character_draft(&character, &BuffSelection::default())?;
    Ok(gamedata::element_preview(
        &character.game_character_id,
        &character.equipment,
        &character.stat_sources,
    ))
}

pub fn list_contents() -> Vec<ContentArea> {
    gamedata::content_areas()
}

pub fn list_equipment_catalog() -> Vec<EquipmentItem> {
    gamedata::equipment_catalog()
}

/// 装備アビリティ 1 件と、それを受け付ける武器系統。画面は「含まれるか」だけで候補を絞る
/// (系統適合の規則は `WeaponSystem::accepts_ability` が唯一の正)。武器以外の部位は空。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EquipmentAbilityView {
    #[serde(flatten)]
    pub def: EquipmentAbilityDef,
    pub weapon_systems: Vec<domain::WeaponSystem>,
}

pub fn list_equipment_abilities() -> Vec<EquipmentAbilityView> {
    gamedata::equipment_abilities()
        .into_iter()
        .map(|def| EquipmentAbilityView {
            weapon_systems: if def.slot == domain::PartSlot::Weapon {
                domain::WeaponSystem::ALL
                    .into_iter()
                    .filter(|system| system.accepts_ability(def.family))
                    .collect()
            } else {
                Vec::new()
            },
            def,
        })
        .collect()
}

/// 装備候補 1 件(カタログ品 + このキャラ・主軸スキルから見た適合度)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EquipmentCandidate {
    #[serde(flatten)]
    pub item: EquipmentItem,
    pub fit: domain::ItemFit,
}

/// 部位の装備候補。`criterion` は何で絞ったか(帯の文言は画面が組む)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EquipmentCandidates {
    pub items: Vec<EquipmentCandidate>,
    pub criterion: Option<domain::FitCriterion>,
}

/// 部位の装備候補を、キャラの装備可能区分と主軸スキルで評価して並べて返す。
///
/// 並びは「値の大きい順(基本能力値上限 → エンチャント枠)→ 名前」。適合度での並べ替えは
/// しない(画面が推奨だけを出すか全部出すかを決める)。
pub fn list_equipment_candidates(
    game_character_id: Option<String>,
    main_skill_id: Option<String>,
    slot: domain::PartSlot,
) -> EquipmentCandidates {
    let character = game_character_id
        .as_deref()
        .and_then(gamedata::find_character);
    let main_skill = main_skill_id.as_deref().and_then(gamedata::find_skill);
    let rule = domain::EquipmentFitRule::new(
        slot,
        character.map(|c| domain::CharacterEquipmentClasses {
            weapon_classes: c.weapon_classes,
            armor_classes: c.armor_classes,
            wrist_types: c.wrist_types,
        }),
        main_skill.as_ref(),
    );
    let sum = |v: domain::EquipmentValues| -> i64 { v.fields().into_iter().map(|(_, x)| x).sum() };
    let mut items: Vec<EquipmentItem> = gamedata::equipment_catalog()
        .into_iter()
        .filter(|item| item.slot == slot)
        .collect();
    items.sort_by(|a, b| {
        sum(b.values_max)
            .cmp(&sum(a.values_max))
            .then_with(|| sum(b.enchant_caps).cmp(&sum(a.enchant_caps)))
            .then_with(|| a.name.cmp(b.name))
    });
    EquipmentCandidates {
        items: items
            .into_iter()
            .map(|item| EquipmentCandidate {
                fit: rule.fit(&domain::ItemClassification {
                    weapon_class: item.weapon_class,
                    armor_class: item.armor_class,
                    wrist_type: item.wrist_type,
                    recommended_dependency: item.recommended_dependency,
                }),
                item,
            })
            .collect(),
        criterion: rule.criterion().cloned(),
    }
}

/// この部位の武器系統(カタログ品の武器種 → カスタム装備の装備強化補正式の順に解決)。
/// 系統不明(カスタム武器で補正式も未選択)は `None`。
pub fn part_weapon_system(part: EquipmentPart) -> Option<domain::WeaponSystem> {
    part.weapon_system(&gamedata::equipment_catalog())
}

/// エンチャント案内 1 行(部位 × 装備補正)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnchantPlanRow {
    pub slot: domain::PartSlot,
    pub stat: domain::EquipmentStatKind,
    pub plan: domain::EnchantPlan,
}

/// 選択中の装備について、エンチャントを案内する補正と上限までのプランを返す。
/// カタログ外(カスタム)装備は案内しない(上限がカタログにないため)。
pub fn list_enchant_plans(character: NewCharacter) -> Vec<EnchantPlanRow> {
    let catalog = gamedata::equipment_catalog();
    let main_dependency = character
        .main_skill_id
        .as_deref()
        .and_then(gamedata::find_skill)
        .map(|skill| skill.dependency);
    let mut rows = Vec::new();
    for (slot, parts) in character.equipment.parts.iter_lists() {
        let Some(part) = parts.selected() else {
            continue;
        };
        let Some(item) = part
            .item_id
            .as_deref()
            .and_then(|id| catalog.iter().find(|i| i.id == id))
        else {
            continue;
        };
        let plan_item = domain::EnchantPlanItem {
            slot,
            weapon_class: item.weapon_class,
            weapon_system: item.weapon_system(),
            recommended_dependency: item.recommended_dependency,
            enchant_caps: item.enchant_caps,
        };
        for stat in domain::enchant_plan_stats(&plan_item, main_dependency) {
            rows.push(EnchantPlanRow {
                slot,
                stat,
                plan: domain::enchant_plan(item.enchant_caps.get(stat) - part.enchant.get(stat)),
            });
        }
    }
    rows
}

/// レリックの育成状況(段・上限・補正値の残り・段を動かせるか)。
pub fn relic_state(part: EquipmentPart) -> Option<domain::RelicState> {
    domain::relic_state(&part, &gamedata::equipment_catalog())
}

/// レリックの段を 1 つ動かした部位。動かせないときは `None`。
pub fn relic_step(
    part: EquipmentPart,
    direction: domain::RelicDirection,
) -> Option<EquipmentPart> {
    domain::relic_step(&part, &gamedata::equipment_catalog(), direction)
}

/// ランダムオプションのカタログ(wiki: ランダムオプション)。
pub fn list_random_options() -> Vec<RandomOptionDef> {
    gamedata::random_option_catalog()
}

/// マスタリーのカタログ(wiki: 各キャラの Skill ページ、スキル表の `P (M1)`〜`(M4)`)。
/// 段ごとに 1 つだけ選ぶ。キャラでの絞り込みは UI 側で `game_character_id` を見て行う。
pub fn list_masteries() -> Vec<domain::MasteryDef> {
    gamedata::mastery_catalog().to_vec()
}

/// シエナのオーラで選べる能力値・追加オプションのカタログ(wiki: 装備システム/シエナのオーラ)。
/// 中身は再抽選のランダム値なので、静的データとして持てるのは**種類と値域**だけ。
pub fn list_siena_kinds() -> domain::SienaCatalog {
    domain::siena_catalog()
}

/// キャラスキルのカタログ(パッシブ・自己バフ・味方バフ)。キャラでの絞り込みは
/// UI 側で `game_character_id` と `audience` を見て行う(味方スキルは誰でも ON にできる)。
pub fn list_character_skills() -> Vec<domain::CharacterSkillDef> {
    gamedata::character_skill_catalog().to_vec()
}

/// キャラ種を変えたときに残してよいキャラスキル id だけを返す(旧キャラ専用・未知の id を落とす)。
pub fn retain_character_skills(skill_ids: Vec<String>, game_character_id: String) -> Vec<String> {
    let mut skills = domain::CharacterSkills {
        skill_ids,
        ..Default::default()
    };
    skills.retain_applicable(gamedata::character_skill_catalog(), &game_character_id);
    skills.skill_ids
}

/// キャラスキル 1 件ぶんの、取っているマスタリーを踏まえた実際の効果。
#[derive(Debug, Clone, serde::Serialize)]
pub struct CharacterSkillEffectsView {
    pub id: String,
    pub effects: Vec<domain::SkillEffect>,
}

/// マスタリーによる効果差し替えを解決した、キャラスキル全件ぶんの効果。
///
/// `CharacterSkillDef::effects` はマスタリー未反映の素の値なので、選んでいるマスタリーで
/// 差し替わった後の値を見せる画面(キャラスキル選択・中ディレイ補正源)はここを呼ぶ
/// (`character_skill.rs` の `effects()` をそのまま呼ぶだけで、解決規則をフロントに持たせない)。
pub fn resolve_character_skill_effects(
    masteries: domain::Masteries,
) -> Vec<CharacterSkillEffectsView> {
    gamedata::character_skill_catalog()
        .iter()
        .map(|def| CharacterSkillEffectsView {
            id: def.id.to_string(),
            effects: def.effects(&masteries).to_vec(),
        })
        .collect()
}

/// 称号 1 件の表示用ビュー。`TitleDef` に、フロントで再計算させない事前計算値を添える。
#[derive(serde::Serialize)]
pub struct TitleView {
    #[serde(flatten)]
    pub def: TitleDef,
    /// 装備の基本能力値への加算 9 値の合計。正は `TitleDef::equipment_value_total`
    pub equipment_value_total: i64,
}

/// 称号のカタログ(wiki: 称号システム)。主要称号のみ。
pub fn list_titles() -> Vec<TitleView> {
    gamedata::title_catalog()
        .into_iter()
        .map(|def| TitleView {
            equipment_value_total: def.equipment_value_total(),
            def,
        })
        .collect()
}

/// 主軸スキル(攻撃力の依存種別を決める)はそのキャラのスキル一覧に含まれている必要がある。
/// キャラ種を変えたときに前キャラのスキルが残るのを防ぐ。未選択(`None`)は許す。
pub fn validate_main_skill(character: &NewCharacter) -> CommandResult<()> {
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

/// 保存する前の検証だけを行う(保存はしない)。ブラウザ版は保存先が IndexedDB(TS 側)なので、
/// 保存層を通らない。デスクトップ版の `create_character` / `update_character` が保存までに
/// 通すのと同じ順序・同じ文言にする(保存層のエラー変換が付ける「不正な値: 」もここで付ける)。
pub fn validate_character(character: NewCharacter) -> CommandResult<()> {
    if gamedata::find_character(&character.game_character_id).is_none() {
        return Err(format!(
            "ゲームキャラ '{}' は未登録です",
            character.game_character_id
        )
        .into());
    }
    validate_main_skill(&character)?;
    // 保存時はバフ選択を伴わないので、バフは既定(何も選んでいない)で見る
    validate_character_draft(&character, &BuffSelection::default()).map_err(|e| CommandError {
        message: format!("不正な値: {}", e.message),
        location: e.location,
    })
}

/// 同上のバフセット版(`buff_set_repository` の `validate_buff_set` と同じ内容)。
pub fn validate_buff_set(name: String, choices: BuffSelection) -> CommandResult<()> {
    if name.trim().is_empty() {
        return Err("不正な値: バフセット名が空です".into());
    }
    domain::stat_sources::build_modifiers(
        &domain::StatSources::default(),
        &choices,
        &gamedata::buff_catalog(),
    )
    .map_err(|e| CommandError::from(format!("不正な値: {e}")))?;
    Ok(())
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

pub fn preview_effective_stats(
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    buffs: BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    main_skill_id: Option<String>,
) -> CommandResult<StatPreviewPayload> {
    let coefficients = attack_coefficients_of(main_skill_id.as_deref())?;
    let part_enhance = part_enhance_previews(&equipment, stat_sources.soul_link);
    let base = domain::preview_effective_stats(
        &base_stats,
        &stat_sources,
        &buffs,
        &equipment,
        &common_skills,
        stat_catalogs(&gamedata::buff_catalog()),
        &gamedata::equipment_abilities(),
        &gamedata::title_catalog(),
        &gamedata::random_option_catalog(),
        coefficients,
        gamedata::awakening_caps(awakening).max_stat,
    )
    .map_err(|e| e.to_string())?;
    Ok(StatPreviewPayload {
        base,
        part_enhance,
    })
}

/// 「対象ステを選ぶ」バフの、ステごとの実際の効き(最終能力値が何点動くか)。
///
/// カタログの生値ではなく **このキャラでの効き** を返す — 素ステが上限に張り付いている
/// ステはバフを乗せても動かない(`gain = 0`)。並べ方・見せ方は呼び出し側の判断。
pub fn buff_target_stat_gains(
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    buffs: BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    buff_id: String,
) -> CommandResult<Vec<domain::BuffTargetStatGain>> {
    let catalog = gamedata::buff_catalog();
    let def = catalog
        .iter()
        .find(|d| d.id == buff_id)
        .ok_or_else(|| CommandError::from(format!("未知のバフです: {buff_id}")))?;
    domain::buff_target_stat_gains(
        &base_stats,
        &stat_sources,
        &buffs,
        &equipment,
        &common_skills,
        stat_catalogs(&catalog),
        def,
        gamedata::awakening_caps(awakening).max_stat,
    )
    .map_err(|e| e.to_string().into())
}

/// 防御側の戦闘能力値(docs/damage-formula.md §6〜7)。保存前のキャラデータで出す。
///
/// 与ダメージ式とは別経路なので対象コンテンツを取らない。装備補正 9 値は
/// 基本能力値 + 強化能力値(地域なし = テシスコアを含まない)の合計を渡す。
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
        stat_catalogs(&gamedata::buff_catalog()),
        &gamedata::equipment_abilities(),
        &gamedata::title_catalog(),
        &gamedata::random_option_catalog(),
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

/// 対人の命中率(wiki `#AccuracyPoint` / `#EvasionPoint` / `#HitRate`)。保存前のキャラデータで
/// 出す。攻撃側はスキルの命中Pまで、防御側は `preview_defense` と同じ防御プロファイルまで
/// それぞれ組み立て、突き合わせは `domain::versus_accuracy` に任せる(計算式は domain 側)。
pub fn preview_versus(
    attacker: NewCharacter,
    attacker_buffs: BuffSelection,
    skill_id: String,
    defender: NewCharacter,
    defender_buffs: BuffSelection,
) -> CommandResult<domain::VersusAccuracy> {
    validate_character_draft(&attacker, &attacker_buffs)?;
    validate_character_draft(&defender, &defender_buffs)?;
    let skill = find_skill(&skill_id)?;
    let skill_accuracy = skill
        .accuracy
        .ok_or_else(|| CommandError::from(format!("スキル '{skill_id}' の命中は未収録です")))?;

    let attacker_preview = domain::preview_effective_stats(
        &attacker.base_stats,
        &attacker.stat_sources,
        &attacker_buffs,
        &attacker.equipment,
        &attacker.common_skills,
        stat_catalogs(&gamedata::buff_catalog()),
        &gamedata::equipment_abilities(),
        &gamedata::title_catalog(),
        &gamedata::random_option_catalog(),
        None,
        gamedata::awakening_caps(attacker.awakening).max_stat,
    )
    .map_err(|e| e.to_string())?;
    let attacker_equipment_totals = attacker_preview
        .equipment_base_total
        .add(attacker.equipment.enhanced_totals(None));

    let defender_preview = domain::preview_effective_stats(
        &defender.base_stats,
        &defender.stat_sources,
        &defender_buffs,
        &defender.equipment,
        &defender.common_skills,
        stat_catalogs(&gamedata::buff_catalog()),
        &gamedata::equipment_abilities(),
        &gamedata::title_catalog(),
        &gamedata::random_option_catalog(),
        None,
        gamedata::awakening_caps(defender.awakening).max_stat,
    )
    .map_err(|e| e.to_string())?;
    let defender_equipment_totals = defender_preview
        .equipment_base_total
        .add(defender.equipment.enhanced_totals(None));
    let defender_profile = domain::defense_profile(
        &defender_preview.stats,
        &defender_equipment_totals,
        gamedata::awakening_caps(defender.awakening),
        &defender
            .equipment
            .random_option_totals(&gamedata::random_option_catalog()),
        // 対人は共通スキル + シエナのオーラどおりの倍率(preview_defense と同じ理由でコンテンツを取らない)
        defender
            .common_skills
            .defense_rates(defender.equipment.siena_defense_rate()),
    );

    let buff_catalog = gamedata::buff_catalog();
    let correction = gamedata::accuracy_correction(skill.dependency);
    let accuracy_boost = resolve_accuracy_boost(&attacker.stat_sources);
    let accuracy_random_option = attacker
        .equipment
        .random_option_totals(&gamedata::random_option_catalog())
        .accuracy_point;
    let defender_random_options = defender
        .equipment
        .random_option_totals(&gamedata::random_option_catalog());
    let evasion_random_option = defender_random_options.evasion_point;
    // 対象の最小回避率補正(wiki #HitRateCap)。ランダムオプション(固定回避・最大回避率)+
    // バフ(テイルズウィーバーのエネルギー)の合計。対人上限 10 のクランプは domain(`hit_rate`)
    // 側が行うのでここでは足し込むだけ
    let min_evasion_rate = defender_random_options.min_evasion_rate
        + domain::stat_sources::buff_min_evasion_rate_total(&defender_buffs, &buff_catalog);
    let attack_type = skill.dependency.attack_type();

    // 伸びしろ(§伸びしろの定義)の材料解決。エンチャント枠の実測上限はカタログ品だけ
    // 引ける(`resolve_enchant_caps` と同じ経路。list_enchant_gains も同じパターン)。
    let equipment_catalog = gamedata::equipment_catalog();
    let resolve_enchant_caps = |equipment: &domain::Equipment| -> Vec<(domain::PartSlot, domain::EquipmentValues)> {
        equipment
            .parts
            .iter()
            .into_iter()
            .filter_map(|(slot, part)| Some((slot, part.resolve_enchant_caps(&equipment_catalog)?)))
            .collect()
    };
    let attacker_enchant_caps = resolve_enchant_caps(&attacker.equipment);
    let defender_enchant_caps = resolve_enchant_caps(&defender.equipment);

    Ok(domain::versus_accuracy(
        &domain::VersusAttacker {
            learnable_accuracy_skill: learnable_accuracy_skill(&attacker.game_character_id),
            stats: &attacker_preview.stats,
            correction: &correction,
            equipment: &attacker.equipment,
            enchant_caps: &attacker_enchant_caps,
            stat_cap: gamedata::awakening_caps(attacker.awakening).max_stat,
            equipment_accuracy: attacker_equipment_totals.accuracy,
            skill_accuracy,
            // 最小命中率補正は今回まだ入力を持たない([仮] 中立値)
            accuracy_bonus: domain::stat_sources::buff_accuracy_point_total(
                &attacker_buffs,
                &buff_catalog,
                accuracy_boost,
            ),
            accuracy_boost,
            accuracy_random_option,
            accuracy_buff_catalog: &buff_catalog,
            accuracy_buff_selection: &attacker_buffs,
            // 最小命中率補正: プレイヤー側の供給源表が wiki に無い(載っているのはマップ側の
            // 値だけ)ため未収録。`VersusAccuracy::min_hit_rate_recorded` が `false` になる
            min_hit_rate: None,
        },
        &domain::VersusDefender {
            stats: &defender_preview.stats,
            profile: &defender_profile,
            equipment: &defender.equipment,
            enchant_caps: &defender_enchant_caps,
            stat_cap: gamedata::awakening_caps(defender.awakening).max_stat,
            evasion_random_option,
            min_evasion_rate: Some(min_evasion_rate),
        },
        attack_type,
    ))
}

/// スキル依存種別(`SkillDependency`)ごとに、エンチャントで見るべき装備値 2 種
/// (`domain::enchant_dependency_keys` = 装備攻撃力係数が非 0 の 2 種)。
/// フロントで「依存種別 → ステ 2 本」のルール表を持たないための静的テーブル
/// (StatLimits と同じく起動時に 1 回だけ取得する)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnchantDependencyKeys {
    pub dependency: domain::SkillDependency,
    pub keys: Vec<String>,
}

/// `domain::StatLimits` に `enchant_dependency_keys` を足したもの。gamedata(装備攻撃力係数)を
/// 要る値なので domain 側には置けず(domain は gamedata に依存できない)、ここで合成する。
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatLimitsPayload {
    #[serde(flatten)]
    pub base: domain::StatLimits,
    pub enchant_dependency_keys: Vec<EnchantDependencyKeys>,
}

pub fn get_stat_limits() -> StatLimitsPayload {
    let enchant_dependency_keys = domain::SkillDependency::ALL
        .into_iter()
        .map(|dependency| EnchantDependencyKeys {
            dependency,
            keys: domain::enchant_dependency_keys(&gamedata::equipment_coefficients(dependency))
                .into_iter()
                .map(str::to_string)
                .collect(),
        })
        .collect();
    StatLimitsPayload {
        base: domain::stat_sources::stat_limits(),
        enchant_dependency_keys,
    }
}

pub fn get_new_character_stat_sources() -> domain::StatSources {
    domain::StatSources::for_new_character()
}

/// 新規登録キャラの共通スキルの実用既定(`CommonSkills::practical_default`)。
pub fn get_new_character_common_skills() -> CommonSkills {
    CommonSkills::practical_default()
}

/// 武器の装備強化による追加固定ダメージ(wiki: 装備システム/装備強化、docs/damage-formula.md §5)。
///
/// `item_id` → カタログの `weapon_class` → 系統ごとの補正式、の順で解決する。
/// - 強化 Lv 0 は 0
/// - +1〜+11 は確定倍率で式から算出
/// - +12 以上は選択等級の確率区分上端で算出する
/// 強化 Lv・等級から引いた倍率で出した追加固定ダメージ。
/// 強化していない・装備種別や等級が決まっていないなら `None`。
fn weapon_enhance(weapon: &EquipmentPart) -> Option<i64> {
    if weapon.enhance_level == 0 {
        return None;
    }
    let enhance_type = weapon.enhance_type.or_else(|| {
        weapon
            .item_id
            .as_deref()
            .and_then(gamedata::equipment_enhance_type)
    });
    let rates = enhance_type.and_then(gamedata::enhance_rates_for_type)?;
    let multiplier = match gamedata::enhance_multiplier(weapon.enhance_level) {
        Some(multiplier) => multiplier,
        // +12 以上は等級ごとの確率区分。等級未選択なら倍率が決まらない
        None => gamedata::enhance_grade_multiplier(weapon.enhance_level, weapon.enhance_grade?)?,
    };
    let values = weapon.base.add(weapon.enchant);
    Some(domain::weapon_added_damage(&values, &rates, multiplier))
}

fn weapon_added_damage(weapon: &EquipmentPart) -> i64 {
    weapon_enhance(weapon).unwrap_or(0)
}

/// 鎧の強化による追加 HP。武器と違い等級を持たない段があるだけで式は同じ。
fn armor_enhance(armor: &EquipmentPart) -> Option<i64> {
    if armor.enhance_level == 0 {
        return None;
    }
    let enhance_type = armor.enhance_type.or_else(|| {
        armor
            .item_id
            .as_deref()
            .and_then(gamedata::equipment_enhance_type)
    });
    let class = enhance_type.and_then(gamedata::armor_class_for_type)?;
    let multiplier = gamedata::armor_enhance_multiplier(armor.enhance_level, armor.enhance_grade)?;
    let values = armor.base.add(armor.enchant);
    let rates = gamedata::armor_enhance_rates(class);
    Some(domain::armor_added_hp(
        &values,
        rates.physical_defense,
        rates.magic_defense,
        multiplier,
    ))
}

#[cfg(test)]
mod flatten_tests {
    use super::*;

    /// `#[serde(flatten)]` はカタログの手書き `Serialize` を通せないと実行時に落ちる。
    /// 画面はこの形をそのまま読むので、直列化できることをここで押さえる。
    #[test]
    fn 装備候補とアビリティ候補は平坦化して直列化できる() {
        let candidates = list_equipment_candidates(
            Some("boris".to_string()),
            Some("boris_continuous".to_string()),
            domain::PartSlot::Weapon,
        );
        let json = serde_json::to_value(&candidates).expect("装備候補を直列化できる");
        let first = &json["items"][0];
        assert!(first["id"].is_string());
        assert!(first["fit"].is_string());
        assert_eq!(json["criterion"]["kind"], "weapon_classes");

        let abilities = list_equipment_abilities();
        let json = serde_json::to_value(&abilities).expect("アビリティ候補を直列化できる");
        assert!(json[0]["id"].is_string());
        assert!(json[0]["weapon_systems"].is_array());
    }
}

#[cfg(test)]
fn armor_added_hp(armor: &EquipmentPart) -> i64 {
    armor_enhance(armor).unwrap_or(0)
}

/// 装備強化 1 部位ぶんの表示用内訳(キャラタブの「装備強化」カード)。
///
/// 追加効果は gamedata の系統別補正式と等級倍率が要るので domain 側では組み立てられない。
/// `StatLimitsPayload` と同じく、ここで gamedata と domain を合成して返す。
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct PartEnhancePreview {
    pub slot: domain::PartSlot,
    /// ソウルリンクを掛ける前の追加効果(武器 = 追加固定ダメージ、鎧 = 追加 HP)
    pub added: i64,
    /// ソウルリンク7(武器)/ 8(鎧)の倍率。Lv0 なら 1.0
    pub soul_link_multiplier: f64,
    /// ソウルリンクまで掛けた最終値
    pub total: i64,
}

/// 装備強化を持てる部位(武器・鎧)の内訳。強化していない部位は返さない。
fn part_enhance_previews(
    equipment: &domain::Equipment,
    soul_link: domain::SoulLinkStatus,
) -> Vec<PartEnhancePreview> {
    let mut previews = Vec::new();
    if let Some(added) = equipment
        .parts
        .get(domain::PartSlot::Weapon)
        .selected()
        .and_then(weapon_enhance)
    {
        previews.push(PartEnhancePreview {
            slot: domain::PartSlot::Weapon,
            added,
            soul_link_multiplier: soul_link.weapon_added_damage_multiplier(),
            total: soul_link.weapon_added_damage(added),
        });
    }
    if let Some(added) = equipment
        .parts
        .get(domain::PartSlot::Armor)
        .selected()
        .and_then(armor_enhance)
    {
        previews.push(PartEnhancePreview {
            slot: domain::PartSlot::Armor,
            added,
            soul_link_multiplier: 1.0 + soul_link.armor_added_hp_rate(),
            total: soul_link.armor_added_hp(added),
        });
    }
    previews
}

/// `domain::StatPreview` に装備強化の内訳を足したもの。`StatLimitsPayload` と同じ理由で
/// gamedata が要る値なので domain 側には置けず、ここで合成する。
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatPreviewPayload {
    #[serde(flatten)]
    pub base: domain::StatPreview,
    pub part_enhance: Vec<PartEnhancePreview>,
}

/// そのキャラが覚えられる命中P割合増加スキル(`SkillEffect::AccuracyRate`。極・的中剣は
/// マキシミン専用)。伸びしろの材料に「Lv7 まで」を出してよいかの判定にだけ使う
/// (覚えられないキャラに出すのは誤り。実機でイサックの伸びしろに出ていた)
fn learnable_accuracy_skill(game_character_id: &str) -> Option<&'static domain::CharacterSkillDef> {
    gamedata::character_skill_catalog().iter().find(|def| {
        def.applies_to(game_character_id)
            && def
                .effects
                .iter()
                .any(|e| matches!(e, domain::SkillEffect::AccuracyRate { .. }))
    })
}

/// `AccuracyBoost` をキャラスキルから解決する。ペット集中は今回まだ入力を持たないので
/// `Concentration` にはならない(`AccuracyBoost::resolve` の型だけ用意)。
fn resolve_accuracy_boost(stat_sources: &domain::StatSources) -> domain::AccuracyBoost {
    let skill = stat_sources.character_skills.accuracy_boost(
        gamedata::character_skill_catalog(),
        &stat_sources.masteries,
    );
    domain::AccuracyBoost::resolve(false, skill)
}

/// 能力値補正に要るカタログ一式。バフカタログだけ所有値なので呼び出し側が持つ。
fn stat_catalogs(buff_catalog: &[domain::BuffDefinition]) -> domain::StatCatalogs<'_> {
    domain::StatCatalogs {
        buffs: buff_catalog,
        masteries: gamedata::mastery_catalog(),
        character_skills: gamedata::character_skill_catalog(),
    }
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
    let buff_catalog = gamedata::buff_catalog();
    let (stat_modifiers, stat_contributions) = domain::build_stat_modifiers(
        stat_sources,
        buffs,
        equipment,
        &common_skills,
        stat_catalogs(&buff_catalog),
        temporary_adjustments,
    )
    .map_err(|e| e.to_string())?;
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
    let accuracy_boost = resolve_accuracy_boost(stat_sources);
    Ok(DamageMaterial {
        base_stats: base_stats.clone(),
        stat_modifiers,
        stat_contributions,
        common_skills,
        temporary_pins: temporary_adjustments.cloned(),
        siena_attack_rate: equipment.siena_attack_rate(),
        siena_critical_rate: equipment.siena_critical_rate(),
        siena_actual_delay_reduction: equipment.siena_actual_delay_reduction(),
        core_set_bonus: equipment.thesis_cores.set_bonus(),
        // 感電は今回まだ入力を持たない([仮] 中立値。goal 「命中Pの計算を wiki どおりに直す」の残タスク)
        accuracy_bonus: domain::stat_sources::buff_accuracy_point_total(
            buffs,
            &buff_catalog,
            accuracy_boost,
        ),
        accuracy_boost,
        accuracy_shocked: false,
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

/// ダメージ計算の入力(キャラ由来の材料 + 何を何に撃つか)を組み立てる
/// (calculate_damage / preview_damage 共通)。
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
) -> CommandResult<(DamageMaterial, DamageTarget)> {
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
    Ok((
        material,
        DamageTarget {
            skill,
            enemy,
            need_per_hit: content.need_per_hit,
            combo_count,
            coefficients,
            equipment_base_sources,
            equipment_enhanced_sources,
            title_attack_damage_rate: title_damage_rate,
            title_added_damage_rate,
            damage_contributions,
            element_value,
        },
    ))
}

/// コンボするなら「通常攻撃 → スキル」の 1 サイクルで、しないならスキル単体で計算する。
///
/// 通常攻撃を挟まないとコンボボーナスは成立しないので、コンボ扱いなのに通常攻撃が
/// 渡ってこないとき(そのキャラの通常攻撃が未収録)は、倍率だけ乗った単体計算になる。
fn damage_with_optional_combo(
    material: &DamageMaterial,
    target: &DamageTarget,
    combo_count: u32,
    normal_attack_id: Option<&str>,
) -> CommandResult<DamageResult> {
    let Some(id) = normal_attack_id.filter(|_| combo_count > 0) else {
        return Ok(domain::calculate_damage(material, target));
    };
    let normal = find_skill(id)?;
    Ok(domain::calculate_damage_with_combo(material, target, &normal))
}

/// 登録済みキャラ・draft のどちらでも通る、与ダメージ計算の本体。
///
/// desktop の `calculate_damage` は DB からキャラを引いたあとここを呼ぶ。draft 用の
/// `preview_damage` は検証を挟んでから同じ経路に入る(計算の重複を作らない)。
#[allow(clippy::too_many_arguments)]
pub fn damage_for_character(
    base_stats: &domain::BaseStats,
    game_character_id: &str,
    main_skill_id: Option<&str>,
    stat_sources: &domain::StatSources,
    buffs: &BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    skill_id: &str,
    content_id: &str,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    normal_attack_id: Option<&str>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<DamageResult> {
    let style_dependency = main_skill_id
        .map(find_skill)
        .transpose()?
        .map(|skill| skill.dependency);
    let content = find_content(content_id)?;
    let enemy = find_enemy(content.enemy_id.as_deref().unwrap_or_default())?;
    let (material, target) = build_damage_input(
        base_stats,
        game_character_id,
        style_dependency,
        stat_sources,
        buffs,
        equipment,
        common_skills,
        awakening,
        find_skill(skill_id)?,
        enemy,
        &content,
        combo_count,
        combo_skill_type,
        temporary_adjustments,
    )?;
    damage_with_optional_combo(&material, &target, combo_count, normal_attack_id)
}

/// 保存前のキャラデータ(編集中 draft・試し変更)でダメージ計算する。DB には書き込まない。
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
    validate_character_draft(&character, &buffs)?;
    damage_for_character(
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

/// 全コンテンツを判定する(ホームの到達一覧・キャラレールのクリア数)。
/// 火力はキャラのスキルのうち 1 ヒット(最大)が最大のもの、コンボ補正なしで評価する。
///
/// `dependency_skill_id` は装備条件(スキル依存で比較先が変わる)の判定に使うスキル。
/// 計算タブのように「今このスキルで戦う」文脈では選択中スキルを渡す。None ならコンテンツ
/// ごとの最大ダメージスキル(敵データなしコンテンツは一覧先頭)の依存で判定する。
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
        &character.equipment,
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

/// 「次に変えるなら / おすすめ強化」候補を列挙し、それぞれの試算結果を 1 回の IPC で返す。
/// 列挙・並び順は domain 側(`crates/domain/src/candidate.rs`)。ここは gamedata カタログの解決
/// (強化補正種別・エンチャント上限・武器の上位品探し)と試算(preview_damage と同じ経路)を担う。
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpgradeCandidate {
    pub id: String,
    pub label: String,
    pub cost: domain::CandidateCost,
    pub per_hit_primary: i64,
    pub total_primary: i64,
    /// 表記ダメージ(1 段)の伸び率。ユーザーがふだん見ている数字
    pub delta_pct: i32,
    /// 実際に敵へ入る総量の伸び率。表記が動かない候補(シャープネスビジョン等)はこちらにだけ出る
    pub delta_total_pct: i32,
    /// 必要 /hit 以上か。`need_per_hit` が無いコンテンツでは常に `false`。
    pub reaches: bool,
    /// この候補を適用したキャラ payload。UI はこれをそのまま whatif の sim に入れる。
    pub applied: NewCharacter,
}

/// 現武器と同じ `weapon_class` の上位カタログ品への更新候補(gamedata 固有の選定なので
/// ここで組み立てる。domain は `weapon_class` を知らない)。
///
/// カタログ最強 1 本ではなく**現武器に近い順に最大 3 本**を挙げる。最強品(改セイクリッド級)は
/// 大半のユーザーに入手困難で、一足飛びの提案は「次の一手」にならない。近い順なら
/// rank_candidates の「届く候補の増分最小を先頭」と噛み合い、手近な武器が自然に前へ出る。
/// 入手性(相場帯)をカタログに持たせるコスト軸は今後の課題(issue #14 相場共有と接続)。
const WEAPON_UPDATE_CANDIDATES: usize = 3;

fn weapon_update_changes(
    equipment: &domain::Equipment,
    common_skills: CommonSkills,
    catalog: &[EquipmentItem],
) -> Vec<domain::CandidateChange> {
    let Some(weapon) = equipment.parts.get(domain::PartSlot::Weapon).selected() else {
        return Vec::new();
    };
    let Some(item_id) = weapon.item_id.as_deref() else {
        return Vec::new();
    };
    let Some(current) = catalog.iter().find(|i| i.id == item_id) else {
        return Vec::new();
    };
    let Some(weapon_class) = current.weapon_class else {
        return Vec::new();
    };
    let sum = |v: domain::EquipmentValues| -> i64 { v.fields().into_iter().map(|(_, value)| value).sum() };
    let current_sum = sum(current.values_max);
    let mut upgrades: Vec<&EquipmentItem> = catalog
        .iter()
        .filter(|i| {
            i.slot == domain::PartSlot::Weapon
                && i.weapon_class == Some(weapon_class)
                && i.id != current.id
                && sum(i.values_max) > current_sum
        })
        .collect();
    upgrades.sort_by_key(|i| sum(i.values_max));
    upgrades
        .into_iter()
        .take(WEAPON_UPDATE_CANDIDATES)
        .filter_map(|upgrade| {
            let mut new_equipment = equipment.clone();
            let part = new_equipment
                .parts
                .get_mut(domain::PartSlot::Weapon)
                .selected_mut()?;
            part.item_id = Some(upgrade.id.to_string());
            part.custom_name = None;
            part.base = upgrade.values_max;
            part.enchant = part.enchant.clamp_to(upgrade.enchant_caps);
            Some(domain::CandidateChange {
                id: format!("weapon-upgrade-{}", upgrade.id),
                label: format!("武器を{}に更新", upgrade.name),
                cost: domain::CandidateCost::EquipmentUpdate,
                equipment: new_equipment,
                common_skills,
            })
        })
        .collect()
}

pub fn list_upgrade_candidates(
    character: NewCharacter,
    buffs: BuffSelection,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<Vec<UpgradeCandidate>> {
    validate_character_draft(&character, &buffs)?;
    let content = find_content(&content_id)?;
    let enemy = find_enemy(content.enemy_id.as_deref().unwrap_or_default())?;
    let skill = find_skill(&skill_id)?;
    let style_dependency = character
        .main_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(|s| s.dependency);

    // 表記ダメージ(到達判定の基準)と、実際に敵へ入る総量の 2 本を返す。シャープネスビジョンや
    // 武器強化のように**表記は動かさず総量だけ増やす**候補があるので、片方だけでは拾えない
    let damage =
        |equipment: domain::Equipment, common_skills: CommonSkills| -> CommandResult<(i64, i64)> {
            let (material, target) = build_damage_input(
                &character.base_stats,
                &character.game_character_id,
                style_dependency,
                &character.stat_sources,
                &buffs,
                equipment,
                common_skills,
                character.awakening,
                skill.clone(),
                enemy.clone(),
                &content,
                combo_count,
                combo_skill_type,
                temporary_adjustments.clone(),
            )?;
            let result = domain::calculate_damage(&material, &target);
            Ok((result.per_hit_primary, result.total_primary))
        };

    let (base_per_hit, base_total) = damage(character.equipment.clone(), character.common_skills)?;

    let equipment_catalog = gamedata::equipment_catalog();
    let resolved_enhance_type = |slot: domain::PartSlot| -> Option<domain::EquipmentEnhanceType> {
        let part = character.equipment.parts.get(slot).selected()?;
        part.enhance_type.or_else(|| {
            part.item_id
                .as_deref()
                .and_then(gamedata::equipment_enhance_type)
        })
    };
    let enchant_caps: Vec<(domain::PartSlot, domain::EquipmentValues)> = character
        .equipment
        .parts
        .iter()
        .into_iter()
        .filter_map(|(slot, part)| Some((slot, part.resolve_enchant_caps(&equipment_catalog)?)))
        .collect();
    // エンチャント候補はこのコンテンツで実際に振る主軸スキル(skill_id)の依存ステだけに絞る
    // (突き/斬り/魔攻/魔防の 4 種全部を出すと、主軸に効かない提案が混ざる)。
    let enchant_allowed_keys =
        domain::enchant_dependency_keys(&gamedata::equipment_coefficients(skill.dependency));

    let mut changes = domain::list_candidate_changes(
        &character.equipment,
        &character.common_skills,
        resolved_enhance_type(domain::PartSlot::Weapon),
        resolved_enhance_type(domain::PartSlot::Armor),
        &enchant_caps,
        &enchant_allowed_keys,
    );
    changes.extend(weapon_update_changes(
        &character.equipment,
        character.common_skills,
        &equipment_catalog,
    ));

    let mut outcomes = Vec::with_capacity(changes.len());
    for change in &changes {
        let (per_hit_primary, total_primary) =
            damage(change.equipment.clone(), change.common_skills)?;
        outcomes.push(domain::CandidateOutcome {
            id: change.id.clone(),
            per_hit_primary,
            total_primary,
        });
    }
    let ranked = domain::rank_candidates(outcomes, base_per_hit, base_total, content.need_per_hit);

    let mut by_id: std::collections::HashMap<String, domain::CandidateChange> =
        changes.into_iter().map(|c| (c.id.clone(), c)).collect();
    Ok(ranked
        .into_iter()
        .filter_map(|r| {
            let change = by_id.remove(&r.id)?;
            let mut applied = character.clone();
            applied.equipment = change.equipment;
            applied.common_skills = change.common_skills;
            Some(UpgradeCandidate {
                id: change.id,
                label: change.label,
                cost: change.cost,
                per_hit_primary: r.per_hit_primary,
                total_primary: r.total_primary,
                delta_pct: r.delta_pct,
                delta_total_pct: r.delta_total_pct,
                reaches: r.reaches,
                applied,
            })
        })
        .collect())
}

/// 「エンチャントの伸びしろ」1 行ぶん(部位 × ステ)。UI はこれを id ではなく
/// `slot`/`key` で直接引き当てる(装備の行・列と 1:1 対応させるため)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnchantGain {
    pub slot: domain::PartSlot,
    /// `EquipmentValues` のフィールド名("thrust"/"slash"/"magic_attack"/"magic_defense")
    pub key: String,
    /// 「MAX まで積むと base に対して何 % 伸びるか」(`rank_candidates` と同じ丸め)
    pub delta_pct: i32,
}

/// 選択中スキルの依存ステだけに絞った、部位・ステごとの「MAX まで積むと +x%」。
/// `list_upgrade_candidates` と同じ `enchant_candidates` + `rank_candidates` の経路を使う
/// (伸び率の式をフロントに持たせない。丸め方の食い違いを起こさない)。改善しない(0%以下)組は
/// `rank_candidates` が既に除外するので、返らない id はそのまま「伸びしろ無し」を意味する。
pub fn list_enchant_gains(
    character: NewCharacter,
    buffs: BuffSelection,
    skill_id: String,
    content_id: String,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<Vec<EnchantGain>> {
    validate_character_draft(&character, &buffs)?;
    let content = find_content(&content_id)?;
    let enemy = find_enemy(content.enemy_id.as_deref().unwrap_or_default())?;
    let skill = find_skill(&skill_id)?;
    let style_dependency = character
        .main_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(|s| s.dependency);

    let per_hit =
        |equipment: domain::Equipment, common_skills: CommonSkills| -> CommandResult<(i64, i64)> {
            let (material, target) = build_damage_input(
                &character.base_stats,
                &character.game_character_id,
                style_dependency,
                &character.stat_sources,
                &buffs,
                equipment,
                common_skills,
                character.awakening,
                skill.clone(),
                enemy.clone(),
                &content,
                combo_count,
                combo_skill_type,
                temporary_adjustments.clone(),
            )?;
            let result = domain::calculate_damage(&material, &target);
            Ok((result.per_hit_primary, result.total_primary))
        };

    let (base_per_hit, base_total) = per_hit(character.equipment.clone(), character.common_skills)?;

    let equipment_catalog = gamedata::equipment_catalog();
    let enchant_caps: Vec<(domain::PartSlot, domain::EquipmentValues)> = character
        .equipment
        .parts
        .iter()
        .into_iter()
        .filter_map(|(slot, part)| Some((slot, part.resolve_enchant_caps(&equipment_catalog)?)))
        .collect();
    let enchant_allowed_keys =
        domain::enchant_dependency_keys(&gamedata::equipment_coefficients(skill.dependency));

    let changes = domain::enchant_candidates(
        &character.equipment,
        &character.common_skills,
        &enchant_caps,
        &enchant_allowed_keys,
    );

    let mut outcomes = Vec::with_capacity(changes.len());
    for change in &changes {
        let (per_hit_primary, total_primary) =
            per_hit(change.equipment.clone(), change.common_skills)?;
        outcomes.push(domain::CandidateOutcome {
            id: change.id.clone(),
            per_hit_primary,
            total_primary,
        });
    }
    let ranked = domain::rank_candidates(outcomes, base_per_hit, base_total, None);

    // id は "enchant-{slot:?}-{key}"(小文字化)形式だが、`{:?}` は元の PartSlot の
    // 表記ゆれ(例: ShieldPlus → シリアライズは shield_plus だが Debug は shieldplus)を持つので
    // id をパースし直さず、変更を組み立てた `changes` から slot/key を直接引く
    let by_id: std::collections::HashMap<String, &domain::CandidateChange> =
        changes.iter().map(|c| (c.id.clone(), c)).collect();
    Ok(ranked
        .into_iter()
        .filter_map(|r| {
            let change = by_id.get(&r.id)?;
            let (slot, key) = enchant_id_slot_key(&character.equipment, change)?;
            Some(EnchantGain {
                slot,
                key,
                delta_pct: r.delta_pct,
            })
        })
        .collect())
}

/// `enchant_candidates` が作った 1 候補から、実際に変わった部位・ステを引き当てる
/// (id の文字列パースに頼らない。`domain::PartSlot::ALL` × 4 種の差分を見て特定する)。
fn enchant_id_slot_key(
    before: &domain::Equipment,
    change: &domain::CandidateChange,
) -> Option<(domain::PartSlot, String)> {
    const KEYS: [(&str, fn(&domain::EquipmentValues) -> i64); 4] = [
        ("thrust", |v| v.thrust),
        ("slash", |v| v.slash),
        ("magic_attack", |v| v.magic_attack),
        ("magic_defense", |v| v.magic_defense),
    ];
    for slot in domain::PartSlot::ALL {
        let Some(before_part) = before.parts.get(slot).selected() else {
            continue;
        };
        let Some(after_part) = change.equipment.parts.get(slot).selected() else {
            continue;
        };
        for (key, get) in KEYS {
            if get(&after_part.enchant) != get(&before_part.enchant) {
                return Some((slot, key.to_string()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        armor_added_hp, build_damage_input, resolve_accuracy_boost, resolve_combo_skill_type,
        weapon_added_damage,
    };
    use domain::{
        AccuracyBoost, AccuracyBoostSource, BaseStats, BuffSelection, ComboSkillType, CommonSkills,
        DamageCategory,
        EnhanceGrade, Equipment, EquipmentEnhanceType, EquipmentPart, EquipmentValues,
        SoulLinkStatus, StatSources,
    };

    #[test]
    fn 極的中剣を選んでいればboostはprecision_sword() {
        // Lv を明示していなければ既定 Lv7(Master)扱い(`CharacterSkills::level_of`)
        let mut stat_sources = StatSources::default();
        stat_sources.character_skills.skill_ids = vec!["maximin_hit_sword".to_string()];
        let boost = resolve_accuracy_boost(&stat_sources);
        assert!((boost.rate - 1.35).abs() < 1e-12);
        assert!(matches!(
            boost.source,
            AccuracyBoostSource::Skill { level: 7, .. }
        ));
    }

    #[test]
    fn 極的中剣のslvを明示すればそのlvが使われる() {
        let mut stat_sources = StatSources::default();
        stat_sources.character_skills.skill_ids = vec!["maximin_hit_sword".to_string()];
        stat_sources
            .character_skills
            .skill_levels
            .insert("maximin_hit_sword".to_string(), 3);
        assert!(matches!(
            resolve_accuracy_boost(&stat_sources).source,
            AccuracyBoostSource::Skill { level: 3, .. }
        ));
    }

    #[test]
    fn 極的中剣を選んでいなければboostはnone() {
        assert_eq!(
            resolve_accuracy_boost(&StatSources::default()),
            AccuracyBoost::NONE
        );
    }

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
        let (material, target) = build_damage_input(
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

        let soul_sources: Vec<_> = target
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
        let soul_damage: Vec<_> = target
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
        assert_eq!(material.weapon_added_damage, 84_711);
    }


}
