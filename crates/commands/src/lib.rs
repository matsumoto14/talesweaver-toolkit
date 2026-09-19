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
    EquipmentInkriState, EquipmentPart, InkriBatchMode, InkriBatchResult, InkriKind, InkriRng,
    NewCharacter, RandomOptionDef, Skill, SkillEvaluationInput, TitleDef, WristBonusMaterial,
};
use gamedata::{EquipmentItem, GameCharacter, InkriTarget};

mod damage_inputs;

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
    let equipment_catalog = gamedata::equipment_catalog();
    let abilities = gamedata::equipment_abilities();
    let random_options = gamedata::random_option_catalog();
    let titles = gamedata::title_catalog();
    character.validate(&domain::CharacterCatalogs {
        equipment: &equipment_catalog,
        abilities: &abilities,
        random_options: &random_options,
        titles: &titles,
        character_skills: gamedata::character_skill_catalog(),
    })?;
    validate_summon_skill(character)?;
    // バフはキャラに保存しないので、キャラ本体の検証(domain)とは別にここで見る
    domain::stat_sources::build_modifiers(
        &character.stat_sources,
        buffs,
        &gamedata::buff_catalog(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 計算対象のコンテンツと敵を引く(敵データが無いコンテンツはダメージ計算の対象にできない)。
fn find_content_with_enemy(content_id: &str) -> CommandResult<(Content, Enemy)> {
    let content = find_content(content_id)?;
    let enemy_id = content.enemy_id.as_deref().unwrap_or_default();
    let enemy = find_enemy(enemy_id)?;
    Ok((content, enemy))
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

pub fn list_contents() -> Vec<ContentArea> {
    gamedata::content_areas()
}

pub fn list_equipment_catalog() -> Vec<EquipmentItem> {
    gamedata::equipment_catalog()
}

/// ビアヌのインクリ対象装備一覧(部位・ビアヌ費用は gamedata が持つ)。
pub fn list_inkri_targets() -> Vec<InkriTarget> {
    gamedata::inkri_targets().to_vec()
}

/// インクリの試行をまとめて実行する。`request.kind` がビアヌ / エタインクリのときだけ、対象装備の
/// その費用(SEED、未収録なら `None`)を消費 SEED の計算に使う(他 4 種は費用資料が無い)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InkriAttemptRequest {
    /// クライアント DB の ItemId(`InkriTarget::client_item_id`)
    pub client_item_id: u32,
    pub state: EquipmentInkriState,
    pub kind: InkriKind,
    pub mode: InkriBatchMode,
    /// 決定的 PRNG のシード(同じ値なら同じ結果になる)
    pub seed: u64,
}

/// エタインクリ呪文書 1 枚の値段(SEED / ELSO / TP)。
pub fn eta_scroll_price() -> domain::EtaScrollPrice {
    domain::ETA_SCROLL_PRICE
}

/// いまの成功率(10万分率)。画面が「今の確率」を出すために引く(表は domain が唯一の正)。
pub fn inkri_success_rate(kind: InkriKind, inkri_count: i64) -> i64 {
    kind.success_rate(inkri_count)
}

pub fn run_inkri_attempts(request: InkriAttemptRequest) -> CommandResult<InkriBatchResult> {
    let target = gamedata::find_inkri_target(request.client_item_id).ok_or_else(|| {
        format!(
            "インクリ対象 '{}' が見つかりません",
            request.client_item_id
        )
    })?;
    if request.kind == InkriKind::Eta && target.eta_seed_cost.is_none() {
        return Err(format!("{} はエタインクリの対象ではありません", target.name).into());
    }
    let seed_cost_per_attempt = match request.kind {
        InkriKind::Vianu => target.bianu_seed_cost,
        InkriKind::Eta => target.eta_seed_cost,
        _ => None,
    };
    let mut rng = InkriRng::new(request.seed);
    Ok(domain::run_batch(
        request.state,
        request.kind,
        request.mode,
        &mut rng,
        seed_cost_per_attempt,
    ))
}

/// 装備アビリティ 1 件と、それを受け付ける武器系統。画面は「含まれるか」だけで候補を絞る
/// (系統適合の規則は `WeaponSystem::accepts_ability` が唯一の正)。武器以外の部位は空。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EquipmentAbilityView {
    #[serde(flatten)]
    pub def: EquipmentAbilityDef,
    pub weapon_systems: Vec<domain::WeaponSystem>,
}

/// その部位で選べない追加候補(武器の HP/MP 自然回復力)を落とす。規則は
/// `EquipmentAbilityAdditionalKind::allowed_on` が唯一の正で、画面は返った候補を並べるだけ。
fn ability_view(mut def: EquipmentAbilityDef) -> EquipmentAbilityView {
    let slot = def.slot;
    def.additional_options
        .retain(|option| option.kind.allowed_on(slot));
    EquipmentAbilityView {
        weapon_systems: if slot == domain::PartSlot::Weapon {
            domain::WeaponSystem::ALL
                .into_iter()
                .filter(|system| system.accepts_ability(def.family))
                .collect()
        } else {
            Vec::new()
        },
        def,
    }
}

pub fn list_equipment_abilities() -> Vec<EquipmentAbilityView> {
    gamedata::equipment_abilities()
        .into_iter()
        .map(ability_view)
        .collect()
}

/// 装着アビリティの候補 1 件。`default_shown` が false のものは「ほかの等級」として畳む。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EquipmentAbilityCandidate {
    #[serde(flatten)]
    pub ability: EquipmentAbilityView,
    pub default_shown: bool,
}

/// この部位のアビリティ系統適合を解決する: `(この部位そのものの系統, 武器アビリティも合算する
/// def-slot)`。武器はそのまま武器系統。双剣Subの盾は武器・盾アビリティを合算するので
/// `also_from = Some(PartSlot::Weapon)`(系統適合は武器 def にだけ掛かる)。それ以外の部位は
/// 系統適合を持たない(`None, None`)。
fn ability_slot_fit(
    part: &EquipmentPart,
    slot: domain::PartSlot,
) -> (Option<domain::WeaponSystem>, Option<domain::PartSlot>) {
    let catalog = gamedata::equipment_catalog();
    if slot == domain::PartSlot::Weapon {
        (part.weapon_system(&catalog), None)
    } else {
        let dual_blade = part.dual_blade_weapon_ability_system(&catalog);
        (dual_blade, dual_blade.map(|_| domain::PartSlot::Weapon))
    }
}

/// この部位(武器はカテゴリー枠)に装着できるアビリティを、画面に出す順で返す。
/// 並び・等級での畳み方・武器系統の適合はすべて domain(`ability_candidates`)が決める。
pub fn list_equipment_ability_candidates(
    part: EquipmentPart,
    slot: domain::PartSlot,
    category: Option<u8>,
) -> Vec<EquipmentAbilityCandidate> {
    let (weapon_system, also_from) = ability_slot_fit(&part, slot);
    domain::ability_candidates(
        &gamedata::equipment_abilities(),
        slot,
        category,
        weapon_system,
        also_from,
        &part.abilities,
    )
    .into_iter()
    .map(|candidate| EquipmentAbilityCandidate {
        ability: ability_view(candidate.def),
        default_shown: candidate.default_shown,
    })
    .collect()
}

/// カタログ品をこの部位に当てた結果を返す(未知の id は `None`)。規則は domain の
/// `EquipmentPart::apply_catalog_item` 1 本だけ。
pub fn apply_catalog_item(part: EquipmentPart, item_id: String) -> Option<EquipmentPart> {
    let catalog = gamedata::equipment_catalog();
    let entry = catalog.iter().find(|i| i.id == item_id)?;
    let mut next = part;
    next.apply_catalog_item(entry);
    Some(next)
}

/// 装備強化 Lv を等級ごと書き換えた部位を返す(+12 以上は等級必須)。
pub fn set_enhance_level(part: EquipmentPart, level: u8) -> EquipmentPart {
    let mut next = part;
    next.set_enhance_level(level);
    next
}

/// 武器(または双剣Subの盾)の 1 カテゴリー枠のアビリティを入れ替えた部位を返す(`None` = 装着しない)。
pub fn set_ability_for_category(
    part: EquipmentPart,
    slot: domain::PartSlot,
    category: u8,
    ability_id: Option<String>,
) -> EquipmentPart {
    let (weapon_system, also_from) = ability_slot_fit(&part, slot);
    let mut next = part;
    next.set_ability_for_category(
        &gamedata::equipment_abilities(),
        slot,
        category,
        ability_id.as_deref(),
        weapon_system,
        also_from,
    );
    next
}

/// 武器以外の部位でアビリティを付け外しした結果を返す。枠数はカタログ品が正、
/// カタログ外は部位の既定枠数。
pub fn toggle_ability(
    part: EquipmentPart,
    slot: domain::PartSlot,
    ability_id: String,
) -> EquipmentPart {
    let catalog = gamedata::equipment_catalog();
    let ability_slots = part
        .item_id
        .as_deref()
        .and_then(|id| catalog.iter().find(|i| i.id == id))
        .map_or_else(|| slot.ability_slots(), |item| item.ability_slots);
    let mut next = part;
    next.toggle_ability(&gamedata::equipment_abilities(), &ability_id, ability_slots);
    next
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
            id: c.id,
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
                    usable_by: item.usable_by,
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

/// ランダムオプション 1 件と、実測の上書きが無いときの既定(ランク・効果値)。
/// 「どのランクが既定か」「上書きが無いときいくつか」の規則は domain が持つ。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RandomOptionView {
    #[serde(flatten)]
    pub def: RandomOptionDef,
    /// 付けたときの既定ランク(一覧のいちばん上位)
    pub default_rank: Option<domain::RandomOptionRank>,
    /// ランクごとの既定の効果値
    pub default_values: Vec<RandomOptionDefaultValue>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RandomOptionDefaultValue {
    pub rank: domain::RandomOptionRank,
    pub value: f64,
}

fn random_option_view(def: RandomOptionDef) -> RandomOptionView {
    RandomOptionView {
        default_rank: def.default_rank(),
        default_values: def
            .tiers
            .iter()
            .map(|tier| RandomOptionDefaultValue {
                rank: tier.rank,
                value: def.default_value(tier.rank),
            })
            .collect(),
        def,
    }
}

/// ランダムオプションのカタログ(wiki: ランダムオプション)。
pub fn list_random_options() -> Vec<RandomOptionView> {
    gamedata::random_option_catalog()
        .into_iter()
        .map(random_option_view)
        .collect()
}

/// この部位にまだ足せるランダムオプション 1 件。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RandomOptionCandidate {
    #[serde(flatten)]
    pub option: RandomOptionView,
    /// チップで先に出す候補(よく付けるもので、主軸スキルの依存種別で発動するもの)
    pub common_choice: bool,
}

/// この部位に足せるランダムオプションを、画面に出す順で返す。
///
/// 同じカテゴリーは 1 部位に 1 つまで(`addable_random_options`)、主軸スキルの依存種別で
/// 発動するか(`RandomOptionEffect::triggers_for`)はどちらも domain の規則。
/// 並びは「よく付けるもののうち主軸の依存に合うもの → 残りのよく付けるもの →
/// そのほか(カタログ順)」(ユーザー確認 2026-08-26)。
pub fn list_random_option_candidates(
    part: EquipmentPart,
    slot: domain::PartSlot,
    main_skill_id: Option<String>,
) -> Vec<RandomOptionCandidate> {
    let defs = gamedata::random_option_catalog();
    let dependency = main_skill_id
        .as_deref()
        .and_then(gamedata::find_skill)
        .map(|skill| skill.dependency);
    let addable = domain::addable_random_options(&part, &defs, slot);
    let triggers = |def: &RandomOptionDef| def.effect.triggers_for(dependency);
    let order = |def: &RandomOptionDef| match def.effect {
        domain::RandomOptionEffect::DependencyDamageRate(d) if Some(d) == dependency => 0,
        domain::RandomOptionEffect::DependencyDamageRate(_) => 1,
        _ => 2,
    };
    let mut common: Vec<&RandomOptionDef> = addable
        .iter()
        .copied()
        .filter(|def| def.common && triggers(def))
        .collect();
    common.sort_by_key(|def| order(def));
    let others = addable
        .iter()
        .copied()
        .filter(|def| !def.common || !triggers(def));
    common
        .into_iter()
        .map(|def| (def, true))
        .chain(others.map(|def| (def, false)))
        .map(|(def, common_choice)| RandomOptionCandidate {
            option: random_option_view(def.clone()),
            common_choice,
        })
        .collect()
}

/// 実測から敵の防御力とカット率を分けて逆算できるか(要る点の数は domain)。
pub fn can_separate_measurement(attacks: Vec<Option<i64>>) -> bool {
    domain::can_separate_defense_and_cut_rate(&attacks)
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

/// `main_skill_id` / `summon_skill_id` の正規化結果(`gamedata::normalize_summon_skill_selection`
/// のコマンド版)。SQLite の v17 移行は gamedata を直接呼べるのでこちらを経由しないが、
/// IndexedDB の v7 移行・書き出し JSON の読み込み(`transfer.ts`)はここを呼ぶ(2026-09-18 追記)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct NormalizedSkillSelection {
    pub main_skill_id: Option<String>,
    pub summon_skill_id: Option<String>,
}

/// 主軸に召喚スキルが紛れていたら召喚欄へ移す(`gamedata::normalize_summon_skill_selection` 参照)。
pub fn normalize_summon_skill_selection(
    main_skill_id: Option<String>,
    summon_skill_id: Option<String>,
) -> NormalizedSkillSelection {
    let (main_skill_id, summon_skill_id) =
        gamedata::normalize_summon_skill_selection(main_skill_id, summon_skill_id);
    NormalizedSkillSelection {
        main_skill_id,
        summon_skill_id,
    }
}

/// 主軸スキル(攻撃力の依存種別を決める)はそのキャラのスキル一覧に含まれている必要がある。
/// キャラ種を変えたときに前キャラのスキルが残るのを防ぐ。未選択(`None`)は許す。
/// 召喚獣(熊・精霊)が撃つスキルは本体の主軸にはできない(wiki 計算式まとめ `STAB(熊)` 行)。
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
    if gamedata::attacker_of(skill_id) != domain::Attacker::Player {
        return Err(CommandError::from(format!(
            "召喚獣が撃つスキル '{skill_id}' は主軸に選べません"
        )));
    }
    Ok(())
}

/// 召喚スキル(アナイスの熊・破壊精霊に撃たせるスキル)はそのキャラのスキル一覧に含まれ、
/// かつ本体以外(召喚獣)が撃つスキルである必要がある。未選択(`None`)は許す。
pub fn validate_summon_skill(character: &NewCharacter) -> CommandResult<()> {
    let Some(skill_id) = &character.summon_skill_id else {
        return Ok(());
    };
    if !gamedata::skills_for(&character.game_character_id)
        .iter()
        .any(|s| &s.id == skill_id)
    {
        return Err(CommandError::from(format!(
            "召喚スキル '{skill_id}' は '{}' のスキルではありません",
            character.game_character_id
        )));
    }
    if gamedata::attacker_of(skill_id) == domain::Attacker::Player {
        return Err(CommandError::from(format!(
            "本体が撃つスキル '{skill_id}' は召喚スキルに選べません"
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
    validate_summon_skill(&character)?;
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

/// スキル依存種別ごとに変わらない攻撃力/装備攻撃力/命中Pの係数を、攻撃者(本体 / 魔法人形)込みで
/// gamedata から解決する唯一の口。`attack_coefficients_of` / 旧 `dependency_coefficients` は
/// ここに統合済み(build_damage_input・evaluate_contents の skill_inputs・候補コンテキストは
/// 全部これ経由)。
fn coefficients_for(
    attacker: domain::Attacker,
    dependency: domain::SkillDependency,
) -> DependencyCoefficients {
    DependencyCoefficients {
        attack: gamedata::attack_coefficients_for(attacker, dependency),
        equipment: gamedata::equipment_coefficients_for(attacker, dependency),
        accuracy: gamedata::accuracy_correction_for(attacker, dependency),
    }
}

/// 指定スキルから攻撃力(A)の係数一式を引く。未選択なら `None`(攻撃力を出さない)。
/// 攻撃者はスキル自身の `attacker` から取るので、本体の主軸(`main_skill_id`)・熊の召喚
/// スキル(`summon_skill_id`)のどちらを渡しても正しい係数が返る(「いまの実力」帯の
/// 熊 / 本体チップは同じ関数を skill_id だけ替えて呼ぶ。二重実装しない)。
fn attack_coefficients_of(
    skill_id: Option<&str>,
) -> CommandResult<Option<AttackPowerCoefficients>> {
    let Some(skill_id) = skill_id else {
        return Ok(None);
    };
    let skill = find_skill(skill_id)?;
    let c = coefficients_for(skill.attacker, skill.dependency);
    Ok(Some(AttackPowerCoefficients {
        stat: c.attack,
        equipment: c.equipment,
    }))
}

#[allow(clippy::too_many_arguments)]
pub fn preview_effective_stats(
    base_stats: domain::BaseStats,
    stat_sources: domain::StatSources,
    buffs: BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    game_character_id: String,
    main_skill_id: Option<String>,
) -> CommandResult<StatPreviewPayload> {
    let coefficients = attack_coefficients_of(main_skill_id.as_deref())?;
    let part_enhance = part_enhance_previews(&equipment, stat_sources.soul_link);
    // 手首補正(腕装備パッシブ)の振り先は主軸スキルの依存種別で決まる。計算タブと同じ
    // 文脈を通すので、キャラ画面の装備合計・装備攻撃力も計算タブと一致する
    let inputs = EquipmentBaseInputs::new(&game_character_id);
    let equipment_base = inputs.context(
        &base_stats,
        stat_sources.soul_link,
        &buffs,
        character_style_dependency(main_skill_id.as_deref())?,
    );
    let base = stat_preview_of(
        &base_stats,
        &stat_sources,
        &buffs,
        &equipment,
        &common_skills,
        awakening,
        &game_character_id,
        equipment_base,
        coefficients,
    )?;
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
    let (stats, base_total) = combat_stats_of(&character, &buffs)?;
    let equipment_totals = base_total.add(character.equipment.enhanced_totals(None));
    Ok(domain::defense_profile(
        &stats,
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
///
/// `attacker_tries` / `defender_tries` は「次にできること」(`GrowthAction`)のうち画面で
/// ON にした手。**押した場所は動かない**ので、伸びしろ 6 フィールド(`accuracy_growth` /
/// `accuracy_max` / `accuracy_max_hit_rate` / `evasion_growth` / `evasion_max` /
/// `evasion_max_hit_rate`)は試す前(base)の材料で固定し、率と内訳(`accuracy_point` /
/// `evasion_point` / `hit_rate` とその内訳)は試した後(tried)の材料で出す
/// (`VersusAccuracy::before_tries` に試す前の値を残す)。
pub fn preview_versus(
    attacker: NewCharacter,
    attacker_buffs: BuffSelection,
    skill_id: String,
    defender: NewCharacter,
    defender_buffs: BuffSelection,
    attacker_tries: Vec<domain::GrowthAction>,
    defender_tries: Vec<domain::GrowthAction>,
) -> CommandResult<domain::VersusAccuracy> {
    validate_character_draft(&attacker, &attacker_buffs)?;
    validate_character_draft(&defender, &defender_buffs)?;
    let skill = find_skill(&skill_id)?;
    let skill_accuracy = skill
        .accuracy
        .ok_or_else(|| CommandError::from(format!("スキル '{skill_id}' の命中は未収録です")))?;

    let buff_catalog = gamedata::buff_catalog();
    let correction = gamedata::accuracy_correction(skill.dependency);
    let attack_type = skill.dependency.attack_type();

    // 伸びしろ(§伸びしろの定義)の材料解決に要るカタログ。装着アビリティ・ランダム OP・
    // 装備品は domain が持てない(gamedata 依存)ので、ここで解決して渡す。
    let equipment_catalog = gamedata::equipment_catalog();
    let abilities = gamedata::equipment_abilities();
    let random_option_catalog = gamedata::random_option_catalog();
    let resolve_enchant_caps = |equipment: &domain::Equipment| -> Vec<(domain::PartSlot, domain::EquipmentValues)> {
        equipment
            .parts
            .iter()
            .into_iter()
            .filter_map(|(slot, part)| Some((slot, part.resolve_enchant_caps(&equipment_catalog)?)))
            .collect()
    };
    let weapon_system_of = |equipment: &domain::Equipment| {
        equipment
            .parts
            .weapon
            .selected()
            .and_then(|part| part.weapon_system(&equipment_catalog))
    };
    // ステ増加バフの伸びしろは全カタログを通した再計算が要る(`stat_buff_rooms`)。
    let stat_buff_rooms_of = |character: &NewCharacter,
                              buffs: &BuffSelection,
                              kind: domain::StatKind|
     -> CommandResult<Vec<domain::BuffRoom>> {
        domain::stat_sources::stat_buff_rooms(
            &character.base_stats,
            &character.stat_sources,
            buffs,
            &character.equipment,
            &character.common_skills,
            stat_catalogs(&buff_catalog),
            kind,
            gamedata::awakening_caps(character.awakening).max_stat,
        )
        .map_err(|e| e.to_string().into())
    };

    // 攻撃側 / 防御側の材料一式から `versus_accuracy` を 1 回組み立てる。
    // `attacker` / `defender` はここでは試した後(tried)・試す前(base)を差し替えて渡す。
    let run = |attacker: &NewCharacter,
               attacker_buffs: &BuffSelection,
               defender: &NewCharacter,
               defender_buffs: &BuffSelection|
     -> CommandResult<domain::VersusAccuracy> {
        let (attacker_stats, attacker_base_total) = combat_stats_of(attacker, attacker_buffs)?;
        let attacker_equipment_totals =
            attacker_base_total.add(attacker.equipment.enhanced_totals(None));

        let (defender_stats, defender_base_total) = combat_stats_of(defender, defender_buffs)?;
        let defender_equipment_totals =
            defender_base_total.add(defender.equipment.enhanced_totals(None));
        let defender_profile = domain::defense_profile(
            &defender_stats,
            &defender_equipment_totals,
            gamedata::awakening_caps(defender.awakening),
            &defender
                .equipment
                .random_option_totals(&random_option_catalog),
            // 対人は共通スキル + シエナのオーラどおりの倍率(preview_defense と同じ理由でコンテンツを取らない)
            defender
                .common_skills
                .defense_rates(defender.equipment.siena_defense_rate()),
        );

        let accuracy_boost = resolve_accuracy_boost(&attacker.stat_sources);
        let accuracy_random_option = attacker
            .equipment
            .random_option_totals(&random_option_catalog)
            .accuracy_point;
        let defender_random_options = defender.equipment.random_option_totals(&random_option_catalog);
        let evasion_random_option = defender_random_options.evasion_point;
        // 対象の最小回避率補正(wiki #HitRateCap)。ランダムオプション(固定回避・最大回避率)+
        // バフ(テイルズウィーバーのエネルギー)の合計。対人上限 10 のクランプは domain(`hit_rate`)
        // 側が行うのでここでは足し込むだけ
        let min_evasion_rate = defender_random_options.min_evasion_rate
            + domain::stat_sources::buff_min_evasion_rate_total(defender_buffs, &buff_catalog);

        let attacker_enchant_caps = resolve_enchant_caps(&attacker.equipment);
        let defender_enchant_caps = resolve_enchant_caps(&defender.equipment);
        let attacker_stat_buff_rooms =
            stat_buff_rooms_of(attacker, attacker_buffs, domain::StatKind::Dex)?;
        let defender_stat_buff_rooms =
            stat_buff_rooms_of(defender, defender_buffs, domain::StatKind::Agi)?;

        Ok(domain::versus_accuracy(
            &domain::VersusAttacker {
                learnable_accuracy_skill: learnable_accuracy_skill(&attacker.game_character_id),
                stats: &attacker_stats,
                correction: &correction,
                equipment: &attacker.equipment,
                enchant_caps: &attacker_enchant_caps,
                stat_cap: gamedata::awakening_caps(attacker.awakening).max_stat,
                equipment_accuracy: attacker_equipment_totals.accuracy,
                skill_accuracy,
                // 最小命中率補正は今回まだ入力を持たない([仮] 中立値)
                accuracy_bonus: domain::stat_sources::buff_accuracy_point_total(
                    attacker_buffs,
                    &buff_catalog,
                    accuracy_boost,
                ),
                accuracy_boost,
                accuracy_random_option,
                accuracy_buff_catalog: &buff_catalog,
                accuracy_buff_selection: attacker_buffs,
                stat_sources: &attacker.stat_sources,
                abilities: &abilities,
                random_option_catalog: &random_option_catalog,
                weapon_system: weapon_system_of(&attacker.equipment),
                stat_buff_rooms: &attacker_stat_buff_rooms,
                // 最小命中率補正: プレイヤー側の供給源表が wiki に無い(載っているのはマップ側の
                // 値だけ)ため未収録。`VersusAccuracy::min_hit_rate_recorded` が `false` になる
                min_hit_rate: None,
            },
            &domain::VersusDefender {
                stats: &defender_stats,
                profile: &defender_profile,
                equipment: &defender.equipment,
                enchant_caps: &defender_enchant_caps,
                stat_cap: gamedata::awakening_caps(defender.awakening).max_stat,
                evasion_random_option,
                stat_sources: &defender.stat_sources,
                abilities: &abilities,
                random_option_catalog: &random_option_catalog,
                weapon_system: weapon_system_of(&defender.equipment),
                stat_buff_rooms: &defender_stat_buff_rooms,
                min_evasion_rate: Some(min_evasion_rate),
            },
            attack_type,
        ))
    };

    if attacker_tries.is_empty() && defender_tries.is_empty() {
        return run(&attacker, &attacker_buffs, &defender, &defender_buffs);
    }

    // 試す前(base)。伸びしろ 6 フィールドと `before_tries` の元にする。
    let base = run(&attacker, &attacker_buffs, &defender, &defender_buffs)?;

    // 試した後(tried)。攻撃側の手は攻撃側の材料に、防御側の手は防御側の材料に、順に当てる
    // (的中剣チップは呼び出し側が `character_skills.skill_ids` に反映して送ってくるので、
    // tries はその上に当たる)。
    let mut tried_attacker = attacker.clone();
    let mut tried_attacker_buffs = attacker_buffs.clone();
    let attacker_ctx = domain::GrowthApplyContext {
        buff_catalog: &buff_catalog,
        abilities: &abilities,
        enchant_caps: &resolve_enchant_caps(&attacker.equipment),
        weapon_system: weapon_system_of(&attacker.equipment),
    };
    for action in &attacker_tries {
        domain::apply_growth_action(
            &mut tried_attacker.stat_sources,
            &mut tried_attacker.equipment,
            &mut tried_attacker_buffs,
            action,
            &attacker_ctx,
        );
    }

    let mut tried_defender = defender.clone();
    let mut tried_defender_buffs = defender_buffs.clone();
    let defender_ctx = domain::GrowthApplyContext {
        buff_catalog: &buff_catalog,
        abilities: &abilities,
        enchant_caps: &resolve_enchant_caps(&defender.equipment),
        weapon_system: weapon_system_of(&defender.equipment),
    };
    for action in &defender_tries {
        domain::apply_growth_action(
            &mut tried_defender.stat_sources,
            &mut tried_defender.equipment,
            &mut tried_defender_buffs,
            action,
            &defender_ctx,
        );
    }

    let tried = run(
        &tried_attacker,
        &tried_attacker_buffs,
        &tried_defender,
        &tried_defender_buffs,
    )?;

    Ok(domain::VersusAccuracy {
        accuracy_growth: base.accuracy_growth,
        accuracy_max: base.accuracy_max,
        accuracy_max_hit_rate: base.accuracy_max_hit_rate,
        accuracy_max_hit_rate_gain: base.accuracy_max_hit_rate_gain,
        evasion_growth: base.evasion_growth,
        evasion_max: base.evasion_max,
        evasion_max_hit_rate: base.evasion_max_hit_rate,
        evasion_max_hit_rate_gain: base.evasion_max_hit_rate_gain,
        before_tries: Some(domain::VersusBeforeTries {
            accuracy_point: base.accuracy_point,
            evasion_point: base.evasion_point,
            hit_rate: base.hit_rate,
        }),
        ..tried
    })
}

/// スキル依存種別(`SkillDependency`)ごとに、エンチャントで見るべき装備値 2 種
/// (`domain::enchant_dependency_keys` = 装備攻撃力係数が非 0 の 2 種)。
/// フロントで「依存種別 → ステ 2 本」のルール表を持たないための静的テーブル
/// (`GameTables` と同じく起動時に 1 回だけ取得する)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnchantDependencyKeys {
    pub dependency: domain::SkillDependency,
    pub keys: Vec<domain::EquipmentStatKind>,
}

/// 数値の上限・刻み・既定値・係数だけ(並び・ラベル・段階表は `get_game_tables`)。
pub fn get_stat_limits() -> domain::StatLimits {
    domain::stat_sources::stat_limits()
}

/// `domain::GameTables` に `enchant_dependency_keys` を足したもの。gamedata(装備攻撃力係数)が
/// 要る値なので domain 側には置けず(domain は gamedata に依存できない)、ここで合成する。
#[derive(Debug, Clone, serde::Serialize)]
pub struct GameTablesPayload {
    #[serde(flatten)]
    pub base: domain::GameTables,
    pub enchant_dependency_keys: Vec<EnchantDependencyKeys>,
    /// 熊(魔法人形)の装備係数が非 0 の値種(斬り・魔攻・魔防。wiki `STAB(熊)` 行)。
    /// 熊は依存種別を持たない固定係数なので、`enchant_dependency_keys` のような依存種別ごとの
    /// 表ではなく単一のリスト。「いまの実力」帯の熊チップが装備列を絞るのに使う
    /// (TS に係数を書き写さない。docs/adr/016-summon-model.md)。
    pub magic_doll_enchant_keys: Vec<domain::EquipmentStatKind>,
}

/// 並び・ラベル・部位ルール・段階表のカタログ(起動時に 1 回だけ取得する)。キャラ文脈が無い
/// グローバル表なので、ここは `Player` 固定のまま(熊は関与しない)。ただし
/// `magic_doll_enchant_keys` だけは熊の固定係数(dependency に依らない)なので例外的にここへ足す。
pub fn get_game_tables() -> GameTablesPayload {
    let enchant_dependency_keys = domain::SkillDependency::ALL
        .into_iter()
        .map(|dependency| EnchantDependencyKeys {
            dependency,
            keys: domain::enchant_dependency_keys(&gamedata::equipment_coefficients(dependency)),
        })
        .collect();
    // 熊の装備係数は dependency を無視する固定値なので、SkillDependency::ALL のどれを渡しても
    // 結果は同じ(gamedata::equipment_coefficients_for の実装参照)。
    let magic_doll_enchant_keys = domain::enchant_dependency_keys(&gamedata::equipment_coefficients_for(
        domain::Attacker::MagicDoll,
        domain::SkillDependency::ALL[0],
    ));
    GameTablesPayload {
        base: domain::game_tables(),
        enchant_dependency_keys,
        magic_doll_enchant_keys,
    }
}

pub fn get_new_character_stat_sources() -> domain::StatSources {
    domain::StatSources::default()
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
    let enhance_type = weapon.resolved_enhance_type(gamedata::equipment_enhance_type);
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
    let enhance_type = armor.resolved_enhance_type(gamedata::equipment_enhance_type);
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
/// `GameTablesPayload` と同じく、ここで gamedata と domain を合成して返す。
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

/// `domain::StatPreview` に装備強化の内訳を足したもの。`GameTablesPayload` と同じ理由で
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

/// 防御・対人が要る 2 値(最終能力値と装備の基本能力値の合計)だけを出す。部位ごとの寄与まで
/// 組み立てるフルプレビュー(`stat_preview_of`)は通さない
fn combat_stats_of(
    character: &NewCharacter,
    buffs: &BuffSelection,
) -> CommandResult<(domain::EffectiveStats, domain::EquipmentValues)> {
    let stats = domain::effective_stats_of(
        &character.base_stats,
        &character.stat_sources,
        buffs,
        &character.equipment,
        &character.common_skills,
        stat_catalogs(&gamedata::buff_catalog()),
        gamedata::awakening_caps(character.awakening).max_stat,
    )
    .map_err(|e| e.to_string())?;
    let inputs = EquipmentBaseInputs::new(&character.game_character_id);
    // 主軸スキルは手首補正の振り先を決めるためだけに引く。カタログに無い id(スキルの改名を
    // またいだ古いキャラ)でも防御・対人は出せるべきなので、ここでは解けなくても止めない
    let style_dependency = character
        .main_skill_id
        .as_deref()
        .and_then(gamedata::find_skill)
        .map(|skill| skill.dependency);
    let base_total = inputs
        .context(
            &character.base_stats,
            character.stat_sources.soul_link,
            buffs,
            style_dependency,
        )
        .total(&character.equipment);
    Ok((stats, base_total))
}

/// 装備の基本能力値の文脈(`domain::EquipmentBaseContext`)を作るのに要る gamedata 側の材料。
///
/// 手首補正(腕装備パッシブ)は「その装備」から装備カタログを引いて解くので閉包で持つ。
/// **装備の基本合計を使う経路(キャラ画面・防御・対人・ダメージ計算・コンテンツ評価)は
/// すべてここから文脈を作る**。以前は経路ごとにソウルリンクと手首補正を継ぎ足していて、
/// キャラ画面・防御・対人だけ手首補正が落ちていた(2026-09-19)。
struct EquipmentBaseInputs {
    abilities: Vec<EquipmentAbilityDef>,
    titles: Vec<TitleDef>,
    wrist: Box<dyn Fn(&domain::Equipment) -> WristBonusMaterial>,
}

impl EquipmentBaseInputs {
    fn new(game_character_id: &str) -> Self {
        let catalog = gamedata::equipment_catalog();
        let id = game_character_id.to_string();
        Self {
            abilities: gamedata::equipment_abilities(),
            titles: gamedata::title_catalog(),
            wrist: Box::new(move |equipment| {
                gamedata::character_wrist_bonus_material(&id, equipment, &catalog)
            }),
        }
    }

    /// `style_dependency` は手首補正の振り先を決める依存種別(主軸スキル、無ければ計算中のスキル)。
    fn context<'a>(
        &'a self,
        base_stats: &domain::BaseStats,
        soul_link: domain::SoulLinkStatus,
        buffs: &BuffSelection,
        style_dependency: Option<domain::SkillDependency>,
    ) -> domain::EquipmentBaseContext<'a> {
        domain::EquipmentBaseContext {
            abilities: &self.abilities,
            titles: &self.titles,
            polish_active: domain::equipment_polish_active(buffs),
            soul_link,
            base_stats: *base_stats,
            wrist: Some(&*self.wrist),
            style_dependency,
        }
    }
}

/// キャラの主軸スキルの依存種別(未選択なら `None`)。手首補正の振り先に使う。
fn character_style_dependency(
    main_skill_id: Option<&str>,
) -> CommandResult<Option<domain::SkillDependency>> {
    main_skill_id
        .map(find_skill)
        .transpose()
        .map(|skill| skill.map(|s| s.dependency))
}

/// 能力値プレビュー(`domain::preview_effective_stats`)をカタログ込みで呼ぶ。
/// キャラ画面・防御・対人が同じ経路を通る
#[allow(clippy::too_many_arguments)]
fn stat_preview_of(
    base_stats: &domain::BaseStats,
    stat_sources: &domain::StatSources,
    buffs: &BuffSelection,
    equipment: &domain::Equipment,
    common_skills: &CommonSkills,
    awakening: domain::Awakening,
    game_character_id: &str,
    equipment_base: domain::EquipmentBaseContext<'_>,
    coefficients: Option<AttackPowerCoefficients>,
) -> CommandResult<domain::StatPreview> {
    domain::preview_effective_stats(
        base_stats,
        stat_sources,
        buffs,
        equipment,
        common_skills,
        stat_catalogs(&gamedata::buff_catalog()),
        equipment_base,
        &gamedata::random_option_catalog(),
        damage_inputs::element_preview(game_character_id, equipment, stat_sources, buffs),
        coefficients,
        gamedata::awakening_caps(awakening).max_stat,
    )
    .map_err(|e| e.to_string().into())
}

/// 能力値補正に要るカタログ一式。バフカタログだけ所有値なので呼び出し側が持つ。
fn stat_catalogs(buff_catalog: &[domain::BuffDefinition]) -> domain::StatCatalogs<'_> {
    domain::StatCatalogs {
        buffs: buff_catalog,
        masteries: gamedata::mastery_catalog(),
        character_skills: gamedata::character_skill_catalog(),
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
        actual_delay_skills: damage_inputs::actual_delay_contributions(
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
    // 装備の基本能力値(ソウルリンク・手首補正込み)はキャラ画面・防御・対人と同じ文脈から出す
    let inputs = EquipmentBaseInputs::new(game_character_id);
    let equipment_base_sources = inputs
        .context(
            base_stats,
            stat_sources.soul_link,
            buffs,
            Some(character_style_dependency.unwrap_or(skill.dependency)),
        )
        .sources(&equipment);
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
        damage_inputs::damage_contributions_of(stat_sources, buffs, &equipment, skill.dependency);
    let element_value =
        damage_inputs::element_value_for(game_character_id, &equipment, stat_sources, buffs, &skill);
    let coefficients = coefficients_for(skill.attacker, skill.dependency);
    Ok((
        material,
        DamageTarget {
            skill,
            enemy,
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

/// 召喚獣(熊・破壊精霊)ぶんのダメージ計算結果。`skill` は召喚スキル、`result` は召喚獣の
/// 係数・コンボ無し(`combo_count = 0`)で計算した `DamageResult` だが、DPS 由来の値
/// (`actual_delay.uses_per_minute` / `dps` / `expected_dps` / `defeat_seconds` / `reach`)は
/// 召喚獣の攻撃間隔式(`summon_uses_per_minute`。本体の実測回数表は使わない)で作り直したもの。
#[derive(Debug, Clone, serde::Serialize)]
pub struct SummonDamage {
    pub skill_id: String,
    pub result: DamageResult,
    /// 攻撃間隔(秒)。中ディレイ未収録なら `None`(0 で埋めない)
    pub interval_seconds: Option<f64>,
}

/// 本体 + 召喚獣の合計(合計 DPS = 本体 DPS + 召喚獣 DPS の単純和。本体は召喚中も自由に撃てるため)。
/// 召喚獣を持たないキャラ・召喚スキル未選択なら本体単独の値と同じ。
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct CombinedDamage {
    pub expected_dps: Option<f64>,
    pub defeat_seconds: Option<f64>,
    /// 合計の討伐時間から決まる到達段(`domain::ReachTier`)。討伐時間が出せないなら `None`。
    /// 召喚獣がいるキャラの「行ける?」判定は本体単独の `body.reach` ではなくこちらを見る
    /// (メーター・討伐時間の文言・ホームのスポットライトが共有する。ADR-016 決定 9・10)
    pub reach: Option<domain::ReachTier>,
}

/// `damage_for_character` / `preview_damage` の戻り。既存の `DamageResult` 全フィールドは
/// `body` にそのまま残し、召喚獣ぶん(`summon`)と合計(`combined`)を足したもの。
#[derive(Debug, Clone, serde::Serialize)]
pub struct CharacterDamageResult {
    pub body: DamageResult,
    /// キャラに `summon_skill_id` があるときだけ `Some`
    pub summon: Option<SummonDamage>,
    pub combined: CombinedDamage,
}

/// 召喚獣(熊・破壊精霊)のスキル 1 件ぶんを計算する。本体と同じ材料(能力値・装備・バフ)で
/// `coefficients` だけ召喚獣の係数(`build_damage_input` が `skill.attacker` を見て分岐)・
/// `combo_count = 0` になる(ここでは召喚獣のスキルを渡すだけでよい)。
#[allow(clippy::too_many_arguments)]
fn build_summon_damage(
    base_stats: &domain::BaseStats,
    game_character_id: &str,
    style_dependency: Option<domain::SkillDependency>,
    stat_sources: &domain::StatSources,
    buffs: &BuffSelection,
    equipment: domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    summon_skill_id: &str,
    enemy: Enemy,
    content: &domain::Content,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<SummonDamage> {
    let skill = find_skill(summon_skill_id)?;
    let (material, target) = build_damage_input(
        base_stats,
        game_character_id,
        style_dependency,
        stat_sources,
        buffs,
        equipment,
        common_skills,
        awakening,
        skill,
        enemy,
        content,
        0,
        None,
        temporary_adjustments,
    )?;
    let mut result = domain::calculate_damage(&material, &target);
    // 召喚獣の攻撃間隔は本体の実測回数表を使わず式で出す(domain::apply_summon_interval。
    // 中ディレイ・回数・DPS・討伐時間をまとめて作り直し、本体式の値を残さない)
    let interval_seconds = domain::apply_summon_interval(&mut result, target.enemy.hp);
    Ok(SummonDamage {
        skill_id: summon_skill_id.to_string(),
        result,
        interval_seconds,
    })
}

/// 本体 DPS + 召喚獣 DPS の単純和(本体は召喚中も自由に撃てるため)。召喚獣が無ければ本体の値のまま。
fn combine_damage(body: &DamageResult, summon: Option<&SummonDamage>) -> CombinedDamage {
    let Some(summon) = summon else {
        return CombinedDamage {
            expected_dps: body.expected_dps,
            defeat_seconds: body.defeat_seconds,
            reach: body.reach,
        };
    };
    let expected_dps = domain::combine_expected_dps(body.expected_dps, summon.result.expected_dps);
    let defeat_seconds = domain::defeat_seconds(body.enemy_hp, expected_dps);
    CombinedDamage {
        expected_dps,
        defeat_seconds,
        reach: domain::ReachTier::of_defeat_seconds(defeat_seconds),
    }
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
    summon_skill_id: Option<&str>,
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
) -> CommandResult<CharacterDamageResult> {
    let style_dependency = main_skill_id
        .map(find_skill)
        .transpose()?
        .map(|skill| skill.dependency);
    let (content, enemy) = find_content_with_enemy(content_id)?;
    let (material, target) = build_damage_input(
        base_stats,
        game_character_id,
        style_dependency,
        stat_sources,
        buffs,
        equipment.clone(),
        common_skills,
        awakening,
        find_skill(skill_id)?,
        enemy.clone(),
        &content,
        combo_count,
        combo_skill_type,
        temporary_adjustments.clone(),
    )?;
    let body = damage_with_optional_combo(&material, &target, combo_count, normal_attack_id)?;
    let summon = summon_skill_id
        .map(|id| {
            build_summon_damage(
                base_stats,
                game_character_id,
                style_dependency,
                stat_sources,
                buffs,
                equipment,
                common_skills,
                awakening,
                id,
                enemy,
                &content,
                temporary_adjustments,
            )
        })
        .transpose()?;
    let combined = combine_damage(&body, summon.as_ref());
    Ok(CharacterDamageResult {
        body,
        summon,
        combined,
    })
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
) -> CommandResult<CharacterDamageResult> {
    validate_character_draft(&character, &buffs)?;
    damage_for_character(
        &character.base_stats,
        &character.game_character_id,
        character.main_skill_id.as_deref(),
        character.summon_skill_id.as_deref(),
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
    // 装備の基本能力値は計算タブ・キャラ画面と同じ文脈から出す(手首補正の振り先は
    // 主軸スキル。主軸が未選択なら評価中のスキルの依存種別を domain 側が使う)
    let inputs = EquipmentBaseInputs::new(&character.game_character_id);
    let equipment_base = inputs.context(
        &character.base_stats,
        character.stat_sources.soul_link,
        &buffs,
        character_style_dependency(character.main_skill_id.as_deref())?,
    );
    // スキルごとに変わるがコンテンツには依存しない値(依存種別の係数・カテゴリ寄与・
    // 属性値)は、コンテンツの数だけ繰り返さずキャラのスキル数ぶんだけ 1 回作る。
    // 召喚獣(熊・破壊精霊)が撃つスキルは本体の最良スキル判定には含めない(本体は召喚獣の
    // スキルを自分で振れないため)。召喚獣ぶんの期待 DPS は別に集計して後段で合算する
    let skill_inputs: Vec<SkillEvaluationInput> = skills
        .iter()
        .filter(|skill| skill.attacker == domain::Attacker::Player)
        .map(|skill| SkillEvaluationInput {
            skill: skill.clone(),
            coefficients: coefficients_for(skill.attacker, skill.dependency),
            damage_contributions: damage_inputs::damage_contributions_of(
                &character.stat_sources,
                &buffs,
                &character.equipment,
                skill.dependency,
            ),
            element_value: damage_inputs::element_value_for(
                &character.game_character_id,
                &character.equipment,
                &character.stat_sources,
                &buffs,
                skill,
            ),
        })
        .collect();
    // 召喚獣(熊・破壊精霊)ぶんの入力(召喚スキル未選択・召喚獣を持たないキャラは None)。
    let summon_input = character
        .summon_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(|skill| SkillEvaluationInput {
            coefficients: coefficients_for(skill.attacker, skill.dependency),
            damage_contributions: damage_inputs::damage_contributions_of(
                &character.stat_sources,
                &buffs,
                &character.equipment,
                skill.dependency,
            ),
            element_value: damage_inputs::element_value_for(
                &character.game_character_id,
                &character.equipment,
                &character.stat_sources,
                &buffs,
                &skill,
            ),
            skill,
        });
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
        summon_input.as_ref(),
        equipment_base,
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
    /// 討伐時間が `REACHED_SECONDS`(5 分)以内か。討伐時間が出せない候補では常に `false`。
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
            part.apply_catalog_item(upgrade);
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

/// 「候補を試算する」コマンド(list_upgrade_candidates / list_enchant_gains)が共有する、
/// キャラ・スキル・コンテンツから決まる材料。
struct CandidateContext {
    content: Content,
    enemy: Enemy,
    skill: Skill,
    style_dependency: Option<domain::SkillDependency>,
    equipment_catalog: Vec<EquipmentItem>,
    /// 部位ごとのエンチャント実測上限(カタログ品が付いている部位だけ)
    enchant_caps: Vec<(domain::PartSlot, domain::EquipmentValues)>,
    /// エンチャント候補にするステ。このコンテンツで実際に振る主軸スキルの依存ステだけに絞る
    /// (突き/斬り/魔攻/魔防の 4 種全部を出すと、主軸に効かない提案が混ざる)
    enchant_allowed_keys: Vec<domain::EquipmentStatKind>,
}

fn candidate_context(
    character: &NewCharacter,
    buffs: &BuffSelection,
    skill_id: &str,
    content_id: &str,
) -> CommandResult<CandidateContext> {
    validate_character_draft(character, buffs)?;
    let (content, enemy) = find_content_with_enemy(content_id)?;
    let skill = find_skill(skill_id)?;
    let style_dependency = character
        .main_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(|s| s.dependency);
    let equipment_catalog = gamedata::equipment_catalog();
    let enchant_caps = character
        .equipment
        .parts
        .iter()
        .into_iter()
        .filter_map(|(slot, part)| Some((slot, part.resolve_enchant_caps(&equipment_catalog)?)))
        .collect();
    // 召喚獣(熊・破壊精霊)のスキルを直接プレビューしているときも、装備は本体のもの
    // (召喚獣は自分の装備を持たない)なので、係数だけ攻撃者で分岐させる
    let mut enchant_allowed_keys = domain::enchant_dependency_keys(
        &gamedata::equipment_coefficients_for(skill.attacker, skill.dependency),
    );
    // キャラに召喚獣が撃つスキルがあれば、エンチャント案内は本体 ∪ 召喚獣の和集合にする
    // (ADR-016 決定 8)。本体は召喚中も自由に撃てるので、召喚獣にしか効かない値種を除外すると
    // 本体側の伸びしろを見逃す。逆に本体だけに効く値種を除外すると召喚獣側を見逃す。
    if let Some(summon_skill_id) = character.summon_skill_id.as_deref() {
        let summon_skill = find_skill(summon_skill_id)?;
        for key in domain::enchant_dependency_keys(&gamedata::equipment_coefficients_for(
            summon_skill.attacker,
            summon_skill.dependency,
        )) {
            if !enchant_allowed_keys.contains(&key) {
                enchant_allowed_keys.push(key);
            }
        }
    }
    Ok(CandidateContext {
        content,
        enemy,
        skill,
        style_dependency,
        equipment_catalog,
        enchant_caps,
        enchant_allowed_keys,
    })
}

/// 候補 1 件を「その装備・共通スキルで撃ったら」で試算する。
struct CandidateTrial<'a> {
    ctx: &'a CandidateContext,
    character: &'a NewCharacter,
    buffs: &'a BuffSelection,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    temporary_adjustments: Option<&'a domain::Adjustments>,
}

impl CandidateTrial<'_> {
    /// 表記ダメージ(到達判定の基準)と、実際に敵へ入る総量の 2 本。シャープネスビジョンや
    /// 武器強化のように**表記は動かさず総量だけ増やす**候補があるので、片方だけでは拾えない
    fn damage(
        &self,
        equipment: domain::Equipment,
        common_skills: CommonSkills,
    ) -> CommandResult<(i64, i64, Option<f64>)> {
        let c = self.character;
        let (material, target) = build_damage_input(
            &c.base_stats,
            &c.game_character_id,
            self.ctx.style_dependency,
            &c.stat_sources,
            self.buffs,
            equipment,
            common_skills,
            c.awakening,
            self.ctx.skill.clone(),
            self.ctx.enemy.clone(),
            &self.ctx.content,
            self.combo_count,
            self.combo_skill_type,
            self.temporary_adjustments.cloned(),
        )?;
        let result = domain::calculate_damage(&material, &target);
        Ok((result.per_hit_primary, result.total_primary, result.defeat_seconds))
    }

    fn outcomes(
        &self,
        changes: &[domain::CandidateChange],
    ) -> CommandResult<Vec<domain::CandidateOutcome>> {
        changes
            .iter()
            .map(|change| {
                let (per_hit_primary, total_primary, defeat_seconds) =
                    self.damage(change.equipment.clone(), change.common_skills)?;
                Ok(domain::CandidateOutcome {
                    id: change.id.clone(),
                    per_hit_primary,
                    total_primary,
                    defeat_seconds,
                })
            })
            .collect()
    }
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
    let ctx = candidate_context(&character, &buffs, &skill_id, &content_id)?;
    let trial = CandidateTrial {
        ctx: &ctx,
        character: &character,
        buffs: &buffs,
        combo_count,
        combo_skill_type,
        temporary_adjustments: temporary_adjustments.as_ref(),
    };
    let (base_per_hit, base_total, _) =
        trial.damage(character.equipment.clone(), character.common_skills)?;

    let resolved_enhance_type = |slot: domain::PartSlot| -> Option<domain::EquipmentEnhanceType> {
        character
            .equipment
            .parts
            .get(slot)
            .selected()?
            .resolved_enhance_type(gamedata::equipment_enhance_type)
    };
    let mut changes = domain::list_candidate_changes(
        &character.equipment,
        &character.common_skills,
        resolved_enhance_type(domain::PartSlot::Weapon),
        resolved_enhance_type(domain::PartSlot::Armor),
        &ctx.enchant_caps,
        &ctx.enchant_allowed_keys,
    );
    changes.extend(weapon_update_changes(
        &character.equipment,
        character.common_skills,
        &ctx.equipment_catalog,
    ));
    let outcomes = trial.outcomes(&changes)?;
    let ranked = domain::rank_candidates(outcomes, base_per_hit, base_total);

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
    /// 積むステ(`EquipmentStatKind`。JSON は `EquipmentValues` のフィールド名と同じ)
    pub key: domain::EquipmentStatKind,
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
    let ctx = candidate_context(&character, &buffs, &skill_id, &content_id)?;
    let trial = CandidateTrial {
        ctx: &ctx,
        character: &character,
        buffs: &buffs,
        combo_count,
        combo_skill_type,
        temporary_adjustments: temporary_adjustments.as_ref(),
    };
    let (base_per_hit, base_total, _) =
        trial.damage(character.equipment.clone(), character.common_skills)?;

    let changes = domain::enchant_candidates(
        &character.equipment,
        &character.common_skills,
        &ctx.enchant_caps,
        &ctx.enchant_allowed_keys,
    );
    let outcomes = trial.outcomes(&changes)?;
    let ranked = domain::rank_candidates(outcomes, base_per_hit, base_total);

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
) -> Option<(domain::PartSlot, domain::EquipmentStatKind)> {
    for slot in domain::PartSlot::ALL {
        let Some(before_part) = before.parts.get(slot).selected() else {
            continue;
        };
        let Some(after_part) = change.equipment.parts.get(slot).selected() else {
            continue;
        };
        for kind in domain::ENCHANT_CANDIDATE_STATS {
            if after_part.enchant.get(kind) != before_part.enchant.get(kind) {
                return Some((slot, kind));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        armor_added_hp, build_damage_input, combat_stats_of, preview_effective_stats,
        preview_versus, resolve_accuracy_boost, resolve_combo_skill_type, weapon_added_damage,
    };
    use domain::{
        AccuracyBoost, AccuracyBoostSource, Awakening, BaseStats, BuffSelection, ComboSkillType,
        CommonSkills, DamageCategory,
        EnhanceGrade, Equipment, EquipmentEnhanceType, EquipmentPart, EquipmentValues,
        GrowthAction, NewCharacter, PetSkillTier, SoulLinkStatus, StatFixedSource, StatKind,
        StatSources,
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

    /// 検証用の最小 `NewCharacter`(アナイス)。
    fn anais() -> NewCharacter {
        NewCharacter {
            name: "アナイス".to_string(),
            game_character_id: "anais".to_string(),
            base_stats: BaseStats {
                stab: 1,
                hack: 1,
                int: 100,
                def: 1,
                mr: 1,
                dex: 1,
                agi: 1,
            },
            awakening: Awakening::default(),
            stat_sources: StatSources::default(),
            equipment: Equipment::default(),
            common_skills: CommonSkills::default(),
            main_skill_id: None,
            summon_skill_id: None,
            goal_content_id: None,
            default_buff_set_id: None,
        }
    }

    #[test]
    fn 熊が撃つスキルを主軸に選ぶと拒否される() {
        let mut c = anais();
        c.main_skill_id = Some("anais_mica_even_bear".to_string());
        let error = super::validate_main_skill(&c).unwrap_err();
        assert!(error.message.contains("主軸に選べません"));
    }

    #[test]
    fn 本体が撃つスキルを召喚スキルに選ぶと拒否される() {
        let mut c = anais();
        c.summon_skill_id = Some("anais_angry_pixie".to_string());
        let error = super::validate_summon_skill(&c).unwrap_err();
        assert!(error.message.contains("召喚スキルに選べません"));
    }

    #[test]
    fn 主軸と召喚の正しい組み合わせは通る() {
        let mut c = anais();
        c.main_skill_id = Some("anais_angry_pixie".to_string());
        c.summon_skill_id = Some("anais_mica_even_bear".to_string());
        assert!(super::validate_main_skill(&c).is_ok());
        assert!(super::validate_summon_skill(&c).is_ok());
    }

    #[test]
    fn 精霊が撃つスキルを主軸に選ぶと拒否される() {
        let mut c = anais();
        c.main_skill_id = Some("anais_lightning_attack".to_string());
        let error = super::validate_main_skill(&c).unwrap_err();
        assert!(error.message.contains("主軸に選べません"));
    }

    #[test]
    fn 精霊が撃つスキルは召喚スキルに選べる() {
        let mut c = anais();
        c.summon_skill_id = Some("anais_lightning_attack".to_string());
        assert!(super::validate_summon_skill(&c).is_ok());
    }

    #[test]
    fn api境界は未対応スキルへのコンボタイプ指定を拒否する() {
        let skill = gamedata::find_skill("maximin_moonlight_sword").unwrap();
        let error =
            resolve_combo_skill_type(skill, &Equipment::default(), Some(ComboSkillType::General))
                .unwrap_err();
        assert!(error.message.contains("対応していません"));
    }

    #[test]
    fn preview_versusはtriesを当てた後の値と当てる前の伸びしろを返す() {
        let attacker = NewCharacter {
            name: "atk".to_string(),
            game_character_id: "lucian".to_string(),
            base_stats: BaseStats {
                stab: 1,
                hack: 1,
                int: 1,
                def: 1,
                mr: 1,
                dex: 100,
                agi: 1,
            },
            awakening: Awakening::default(),
            stat_sources: StatSources::default(),
            equipment: Equipment::default(),
            common_skills: CommonSkills::default(),
            main_skill_id: None,
            summon_skill_id: None,
            goal_content_id: None,
            default_buff_set_id: None,
        };
        let defender = NewCharacter {
            name: "def".to_string(),
            base_stats: BaseStats {
                stab: 1,
                hack: 1,
                int: 1,
                def: 1,
                mr: 1,
                dex: 1,
                agi: 100,
            },
            ..attacker.clone()
        };
        let buffs = BuffSelection::default();

        let without_tries = preview_versus(
            attacker.clone(),
            buffs.clone(),
            "lucian_butt".to_string(),
            defender.clone(),
            buffs.clone(),
            vec![],
            vec![],
        )
        .unwrap();
        assert!(without_tries.before_tries.is_none());

        // DEX の固定上昇(ペット S)を攻撃側だけ試す
        let attacker_tries = vec![GrowthAction::StatFixed {
            stat: StatKind::Dex,
            source: StatFixedSource::PetSkill {
                target: PetSkillTier::TrueLv4,
            },
        }];
        let (attacker_for_both, defender_for_both, buffs_for_both, attacker_tries_for_both) =
            (attacker.clone(), defender.clone(), buffs.clone(), attacker_tries.clone());
        let with_tries = preview_versus(
            attacker,
            buffs.clone(),
            "lucian_butt".to_string(),
            defender,
            buffs,
            attacker_tries,
            vec![],
        )
        .unwrap();

        // 伸びしろ 6 フィールドは試す前(base)のまま(押した場所は動かない)
        assert_eq!(with_tries.accuracy_growth, without_tries.accuracy_growth);
        assert_eq!(with_tries.accuracy_max, without_tries.accuracy_max);
        assert_eq!(
            with_tries.accuracy_max_hit_rate,
            without_tries.accuracy_max_hit_rate
        );
        assert_eq!(
            with_tries.accuracy_max_hit_rate_gain,
            without_tries.accuracy_max_hit_rate_gain,
            "accuracy_max_hit_rate_gain も base 由来(押した場所は動かない)"
        );
        assert_eq!(with_tries.evasion_growth, without_tries.evasion_growth);
        assert_eq!(with_tries.evasion_max, without_tries.evasion_max);
        assert_eq!(
            with_tries.evasion_max_hit_rate,
            without_tries.evasion_max_hit_rate
        );

        // before_tries は試す前(base)の値
        let before = with_tries.before_tries.expect("tries が空でないので Some");
        assert_eq!(before.accuracy_point, without_tries.accuracy_point);
        assert_eq!(before.evasion_point, without_tries.evasion_point);
        assert_eq!(before.hit_rate, without_tries.hit_rate);

        // accuracy_point は試した後(DEX が伸びた分)。防御側は試していないので回避Pは動かない
        assert!(with_tries.accuracy_point > without_tries.accuracy_point);
        assert_eq!(with_tries.evasion_point, without_tries.evasion_point);

        // 防御側の手(AGI の固定上昇)は防御側の材料に当たり、回避Pだけが動く
        let defender_tries = vec![GrowthAction::StatFixed {
            stat: StatKind::Agi,
            source: StatFixedSource::PetSkill {
                target: PetSkillTier::TrueLv4,
            },
        }];
        let both = preview_versus(
            attacker_for_both,
            buffs_for_both.clone(),
            "lucian_butt".to_string(),
            defender_for_both,
            buffs_for_both,
            attacker_tries_for_both,
            defender_tries,
        )
        .unwrap();
        assert_eq!(both.accuracy_point, with_tries.accuracy_point);
        assert!(both.evasion_point > without_tries.evasion_point);
        assert_eq!(both.evasion_growth, without_tries.evasion_growth);
        assert_eq!(
            both.before_tries.expect("Some").evasion_point,
            without_tries.evasion_point
        );
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

    /// 熊(魔法人形)のスキルで組み立てると、突き係数 0・魔攻係数 23.75(基本)の熊固定係数になる
    /// (wiki 計算式まとめ `STAB(熊)` 行)。本体のスキルでは従来どおり依存種別の係数のまま。
    #[test]
    fn 熊のスキルは本体と別の係数で組み立たる() {
        let content = gamedata::content_areas()
            .into_iter()
            .flat_map(|area| area.contents)
            .find(|content| content.id == "ringo")
            .unwrap();
        let base_stats = BaseStats {
            stab: 100,
            hack: 100,
            int: 100,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let (_, bear_target) = build_damage_input(
            &base_stats,
            "anais",
            None,
            &StatSources::default(),
            &BuffSelection::default(),
            Equipment::default(),
            CommonSkills::default(),
            domain::Awakening::default(),
            gamedata::find_skill("anais_mica_even_bear").unwrap(),
            gamedata::find_enemy("ringo_boss").unwrap(),
            &content,
            0,
            None,
            None,
        )
        .unwrap();
        assert_eq!(bear_target.coefficients.equipment.base.thrust, 0.0);
        assert_eq!(bear_target.coefficients.equipment.base.magic_attack, 23.75);
        assert_eq!(bear_target.coefficients.attack.primary, (StatKind::Int, 2.1));

        let (_, body_target) = build_damage_input(
            &base_stats,
            "anais",
            None,
            &StatSources::default(),
            &BuffSelection::default(),
            Equipment::default(),
            CommonSkills::default(),
            domain::Awakening::default(),
            gamedata::find_skill("anais_angry_pixie").unwrap(),
            gamedata::find_enemy("ringo_boss").unwrap(),
            &content,
            0,
            None,
            None,
        )
        .unwrap();
        // anais_angry_pixie は Int 依存(本体スキル)。攻撃力係数は従来どおり Int×2.4(熊は 2.1)。
        assert_eq!(body_target.coefficients.attack.primary, (StatKind::Int, 2.4));
    }

    /// summon_skill_id があるキャラは summon が Some になり、合計 DPS は本体+熊、
    /// 討伐時間は本体単独より短くなる(本体は召喚中も自由に撃てるので単純和)。
    #[test]
    fn 召喚スキルがあるキャラは本体と熊の合計dpsになる() {
        let base_stats = BaseStats {
            stab: 300,
            hack: 300,
            int: 300,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let with_summon = super::damage_for_character(
            &base_stats,
            "anais",
            Some("anais_angry_pixie"),
            Some("anais_mica_even_bear"),
            &StatSources::default(),
            &BuffSelection::default(),
            Equipment::default(),
            CommonSkills::default(),
            domain::Awakening::default(),
            "anais_angry_pixie",
            "ringo",
            0,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(with_summon.summon.is_some());
        let summon_dps = with_summon.summon.as_ref().unwrap().result.expected_dps.unwrap();
        let body_dps = with_summon.body.expected_dps.unwrap();
        assert!((with_summon.combined.expected_dps.unwrap() - (body_dps + summon_dps)).abs() < 1e-6);
        assert!(with_summon.combined.defeat_seconds.unwrap() < with_summon.body.defeat_seconds.unwrap());

        // summon_skill_id が無いキャラ(他キャラ)は summon が None で combined = 本体(回帰)。
        let without_summon = super::damage_for_character(
            &BaseStats {
                stab: 300,
                hack: 300,
                int: 1,
                def: 1,
                mr: 1,
                dex: 1,
                agi: 1,
            },
            "boris",
            None,
            None,
            &StatSources::default(),
            &BuffSelection::default(),
            Equipment::default(),
            CommonSkills::default(),
            domain::Awakening::default(),
            "boris_horizontal_sword",
            "ringo",
            0,
            None,
            None,
            None,
        )
        .unwrap();
        assert!(without_summon.summon.is_none());
        assert_eq!(without_summon.combined.expected_dps, without_summon.body.expected_dps);
        assert_eq!(without_summon.combined.defeat_seconds, without_summon.body.defeat_seconds);
    }

    /// 召喚スキルがあるキャラのエンチャント案内は、本体(anais_angry_pixie。Int 依存 →
    /// 魔攻・魔防)∪ 熊(斬り・魔攻・魔防)の和集合になる(ADR-016 決定 8)。
    /// 斬りは熊にしか無い成分なので、和集合になっているかの目印にする。
    #[test]
    fn 召喚ありのエンチャント案内は本体と熊の和集合() {
        let mut c = anais();
        c.main_skill_id = Some("anais_angry_pixie".to_string());
        c.summon_skill_id = Some("anais_mica_even_bear".to_string());
        let ctx = super::candidate_context(&c, &BuffSelection::default(), "anais_angry_pixie", "ringo").unwrap();
        assert!(ctx.enchant_allowed_keys.contains(&domain::EquipmentStatKind::Slash));
        assert!(ctx.enchant_allowed_keys.contains(&domain::EquipmentStatKind::MagicAttack));
        assert!(ctx.enchant_allowed_keys.contains(&domain::EquipmentStatKind::MagicDefense));

        // 召喚スキル未選択なら本体(Int 依存)だけ。斬りは本体側に無い成分なので含まれない(回帰)。
        let mut without_summon = anais();
        without_summon.main_skill_id = Some("anais_angry_pixie".to_string());
        let ctx_without = super::candidate_context(&without_summon, &BuffSelection::default(), "anais_angry_pixie", "ringo").unwrap();
        assert!(!ctx_without.enchant_allowed_keys.contains(&domain::EquipmentStatKind::Slash));
    }

    #[test]
    fn 手首補正はキャラ画面と計算タブで同じ装備基本合計になる() {
        // ボリスは腕(盾)の突き(基本+エンチャント)が魔攻の基本補正になる
        // (`WristBonusRule::ThrustToMagicAttack`)。以前はこの変換を計算タブと
        // コンテンツ評価だけが継ぎ足していて、キャラ画面・防御・対人は落としていた。
        // 装備の基本合計は `EquipmentBaseContext` 1 本に集約したので両者は必ず一致する。
        let base_stats = BaseStats {
            stab: 100,
            hack: 100,
            int: 1,
            def: 1,
            mr: 1,
            dex: 1,
            agi: 1,
        };
        let mut equipment = Equipment::default();
        equipment.parts.shield = EquipmentPart {
            base: EquipmentValues {
                thrust: 120,
                ..Default::default()
            },
            enchant: EquipmentValues {
                thrust: 15,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();
        let content = gamedata::content_areas()
            .into_iter()
            .flat_map(|area| area.contents)
            .find(|content| content.id == "ringo")
            .unwrap();
        let (_, target) = build_damage_input(
            &base_stats,
            "boris",
            None,
            &StatSources::default(),
            &BuffSelection::default(),
            equipment.clone(),
            CommonSkills::default(),
            Awakening::default(),
            gamedata::find_skill("boris_horizontal_sword").unwrap(),
            gamedata::find_enemy("ringo_boss").unwrap(),
            &content,
            0,
            None,
            None,
        )
        .unwrap();
        let preview = preview_effective_stats(
            base_stats,
            StatSources::default(),
            BuffSelection::default(),
            equipment,
            CommonSkills::default(),
            Awakening::default(),
            "boris".to_string(),
            Some("boris_horizontal_sword".to_string()),
        )
        .unwrap();

        assert_eq!(
            preview.base.equipment_base_total,
            target.equipment_base_totals(),
            "キャラ画面と計算タブの装備基本合計は一致する"
        );
        assert_eq!(
            preview.base.equipment_base_total.magic_attack, 135,
            "腕の突き(基本 120 + エンチャント 15)が魔攻へ乗る"
        );
    }


    #[test]
    fn 防御と対人が通る経路にも手首補正が入る() {
        // ベンヤはバンドの敏捷 × 0.7 が HACK と MR の大小で 斬り / 魔防 へ行く
        // (`WristBonusRule::BandAgilityByStatComparison`)。魔防は防御プロファイルの入力なので、
        // 防御・対人が使う `combat_stats_of` の装備基本合計に入っていないと防御力が下振れする。
        let catalog = gamedata::equipment_catalog();
        let band = catalog
            .iter()
            .find(|item| item.wrist_type == Some(domain::WristType::Band))
            .expect("バンドの腕装備がカタログにある");
        let mut character = anais();
        character.game_character_id = "benya".to_string();
        // HACK < MR なので振り先は魔防
        character.base_stats.hack = 1;
        character.base_stats.mr = 100;
        character.equipment.parts.shield = EquipmentPart {
            item_id: Some(band.id.to_string()),
            base: EquipmentValues {
                agility: 100,
                ..Default::default()
            },
            ..Default::default()
        }
        .into();

        let (_, with_band) = combat_stats_of(&character, &BuffSelection::default()).unwrap();
        // 100 × 0.7 = 70(小数点以下切り捨て)
        assert_eq!(with_band.magic_defense, 70);

        // 腕を外せば手首補正も消える
        character.equipment.parts.shield.selected_id = None;
        let (_, without_band) = combat_stats_of(&character, &BuffSelection::default()).unwrap();
        assert_eq!(without_band.magic_defense, 0);
    }

    #[test]
    fn 主軸スキルがカタログに無くても防御と対人は出せる() {
        // スキルの改名をまたいだ古いキャラ。手首補正の振り先が決まらないだけで、
        // 防御・対人まで開けなくなってはいけない
        let mut character = anais();
        character.main_skill_id = Some("no-such-skill".to_string());
        assert!(combat_stats_of(&character, &BuffSelection::default()).is_ok());
    }


    #[test]
    fn 全属性バフと回廊は属性値の内訳に出る() {
        // 迅速の秘薬(全属性 +15)を ON、回廊の全属性増加を Lv10(各属性 +10)にすると、
        // キャラ画面の属性内訳にそれぞれ別の行として出て、合計にも乗る
        let mut character = anais();
        character.stat_sources.lumina_corridor = domain::LuminaCorridor {
            all_element_level: 10,
            ..Default::default()
        };
        let buffs = BuffSelection {
            choices: vec![domain::BuffChoice {
                buff_id: "swift_elixir".to_string(),
                stat: None,
                choice_index: None,
                value: None,
            }],
        };
        let preview = preview_effective_stats(
            character.base_stats,
            character.stat_sources.clone(),
            buffs,
            character.equipment.clone(),
            character.common_skills,
            character.awakening,
            character.game_character_id.clone(),
            None,
        )
        .unwrap();

        let elements = preview.base.elements;
        assert_eq!(elements.corridor.fire, 10, "回廊は 8 属性それぞれに +10");
        assert_eq!(elements.buff.fire, 15, "全属性バフは 8 属性それぞれに +15");
        // アナイスの基礎属性値(火 10)も足した合計
        assert_eq!(elements.total.fire, elements.base.fire + 25);
    }

}
