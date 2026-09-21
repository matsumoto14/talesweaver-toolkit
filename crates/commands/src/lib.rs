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

/// 保存済みの「差し込む CT 技」から、選べなくなった id を落とす
/// (`gamedata::retain_rotation_skills` 参照)。SQLite の v19 移行は gamedata を直接呼ぶので
/// ここを経由しないが、IndexedDB の v11 正規化・書き出し JSON の読み込みはこのコマンドを呼ぶ。
pub fn retain_rotation_skills(
    rotation_skill_ids: Option<Vec<String>>,
    game_character_id: String,
) -> Option<Vec<String>> {
    let mut ids = rotation_skill_ids?;
    gamedata::retain_rotation_skills(&mut ids, &game_character_id);
    Some(ids)
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

/// 保存済みのキャラスキル選択から、カタログから消えた id を落とす
/// (`gamedata::normalize_character_skill_selection` 参照)。SQLite の v18 移行は gamedata を
/// 直接呼ぶのでここを経由しないが、IndexedDB の v10 移行・書き出し JSON の読み込みは
/// このコマンドを呼ぶ(TS に消えた id の一覧を書き写さないため)。
pub fn normalize_character_skills(character_skills: domain::CharacterSkills) -> domain::CharacterSkills {
    let mut skills = character_skills;
    gamedata::normalize_character_skill_selection(&mut skills);
    skills
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

/// 回しに差し込む CT 技(`rotation_skill_ids`)は、そのキャラが自分で撃つ攻撃技で、
/// クールタイムを持つものだけを指定できる。未設定(`None`)・空(`Some([])` = 差し込まない)は許す。
/// 主軸スキル・召喚スキルと同じ形の検証(`validate_main_skill` 参照)。
pub fn validate_rotation_skills(character: &NewCharacter) -> CommandResult<()> {
    let Some(ids) = &character.rotation_skill_ids else {
        return Ok(());
    };
    let skills = gamedata::skills_for(&character.game_character_id);
    for id in ids {
        let Some(skill) = skills.iter().find(|s| &s.id == id) else {
            return Err(CommandError::from(format!(
                "差し込む CT 技 '{id}' は '{}' のスキルではありません",
                character.game_character_id
            )));
        };
        if skill.attacker != domain::Attacker::Player {
            return Err(CommandError::from(format!(
                "召喚獣が撃つスキル '{id}' は差し込む CT 技に選べません"
            )));
        }
        if skill.cooldown_seconds.is_none() {
            return Err(CommandError::from(format!(
                "クールタイムの無いスキル '{id}' は差し込む CT 技に選べません"
            )));
        }
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
    validate_rotation_skills(&character)?;
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

/// キャラスキルの習得(速剣・最大までチャージ)で変わる技の性能を解決する。
/// 判定は `Skill` 側の印(`swift_sword` / `full_charge`)だけを見るので、形態の if は
/// ここにも呼び出し側にも無い(実体は `gamedata::resolve_skill_variants`)。
fn resolve_skill_variants(skill: Skill, stat_sources: &domain::StatSources) -> Skill {
    gamedata::resolve_skill_variants(
        skill,
        &stat_sources.character_skills,
        &stat_sources.masteries,
    )
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
    let skill = resolve_skill_variants(
        resolve_combo_skill_type(skill, &equipment, combo_skill_type)?,
        stat_sources,
    );
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
        damage_inputs::damage_contributions_of(stat_sources, buffs, &equipment, &skill);
    let skill_added_damage_rate = damage_inputs::added_damage_rate_of(stat_sources, &skill);
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
            skill_added_damage_rate,
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

/// <フラグ>(イェフネンの、技とは別枠のダメージ)ぶんの計算結果。
///
/// 持続(1 秒ごと)と爆発(スレイ / クラッシュのときだけ)を、技とまったく同じ材料
/// (能力値・装備・バフ・敵デバフ・割合追加ダメージ)で計算したもの。形態限定の効果
/// (速剣・後方攻撃・最大チャージ)とコンボボーナスは乗らない(`gamedata::flag_skill` /
/// `build_flag_damage` 参照)。DPS 由来の値は技データ側に中ディレイが無いので `None` のまま —
/// 合算は `combine_damage` が周期と技 1 回の所要時間で出す。
#[derive(Debug, Clone, serde::Serialize)]
pub struct FlagDamage {
    /// いま敵に付いているスタック数(1〜10)
    pub stacks: u8,
    /// 倍率(スタック数から引いた値。画面の説明用)
    pub multiplier: f64,
    /// 持続ダメージ 1 回ぶん(倍率 × 1 段、Cri倍率 2.0)
    pub duration: DamageResult,
    /// 爆発 1 回ぶん(倍率 × 5 段、Cri倍率 2.5)。主軸が爆発させる技のときだけ `Some`。
    /// `dps` は主軸を撃つ間隔(回しの `interval_seconds`)で割った値
    pub burst: Option<DamageResult>,
    /// 持続ダメージの周期(秒)
    pub tick_seconds: f64,
    /// <フラグ> の持続時間(秒)
    pub lasts_seconds: f64,
}

/// 回し(連打する技 1 つ + 差し込む CT 技 0〜数個)の内訳。
///
/// CT のある技は明けるまで撃てないので、その間は連打技を撃っている。回数と時間の規則は
/// `domain::plan_rotation`、DPS は `domain::rotation_dps`。差し込む技が 1 つも無い
/// (CT 技を持たないキャラ・主軸の CT が所要時間以下)なら回しを組まず `None`。
#[derive(Debug, Clone, serde::Serialize)]
pub struct Rotation {
    /// 連打する技。`None` = 連打できる技が無い(CT が明くのを待つだけ)
    pub filler: Option<RotationFiller>,
    /// 差し込む CT 技(主軸が CT 技ならその 1 つ目)
    pub inserts: Vec<RotationInsert>,
    /// 連打技に回る時間の割合(0〜1)
    pub filler_share: f64,
    /// 差し込む技だけで時間が埋まり、間隔を伸ばして詰めたか(全部は CT どおりに撃てない)
    pub crowded: bool,
    /// 回し全体の DPS(側ごと)と期待値
    pub dps: domain::DpsTriple,
    pub expected_dps: f64,
}

/// 回しの連打技(差し込みの合間に撃つ技)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RotationFiller {
    pub skill_id: String,
    pub skill_name: String,
    /// 連打しているのが主軸そのものか(画面は鎖に出ている結果をそのまま使う)
    pub is_main: bool,
    /// 1 回ぶんの結果。主軸そのものなら `None`(鎖の結果と同じ)
    pub result: Option<DamageResult>,
    /// 1 回撃つのにかかる時間(秒)。コンボ中は通常攻撃を挟んだ 1 サイクル
    pub seconds: f64,
    /// この技が出している期待 DPS(回しの中での取り分。`domain::rotation_shares`)
    pub expected_dps: f64,
    /// 回しの期待 DPS に占める割合(0〜1)。差し込みぶんと足すと 1
    pub dps_share: f64,
    /// 1 分あたりに撃つ回数(空いた時間ぶん)
    pub uses_per_minute: f64,
}

/// 回しに差し込む CT 技 1 つぶん。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RotationInsert {
    pub skill_id: String,
    pub skill_name: String,
    /// 差し込む技が主軸そのものか(画面は鎖に出ている結果をそのまま使う)
    pub is_main: bool,
    /// 1 回ぶんの結果。主軸そのものなら `None`(鎖の結果と同じ)
    pub result: Option<DamageResult>,
    /// この技が起こす <フラグ> の爆発 1 回ぶん。主軸そのものなら `None`
    /// (`FlagDamage::burst` にある)
    pub burst: Option<DamageResult>,
    /// 1 回撃つのにかかる時間(秒)
    pub seconds: f64,
    /// クールタイム(秒)
    pub cooldown_seconds: f64,
    /// この技を撃つ間隔(秒)= 1 回ぶん + 挟む連打技。CT より短くならない
    pub interval_seconds: f64,
    /// 1 回あたり挟む連打技の回数
    pub filler_uses: u32,
    /// CT を満たすために連打の回数を増やしたか(<フラグ> の積み直しより多く挟んでいる)
    pub cooldown_bound: bool,
    /// この技を差し込むことで増える期待 DPS(**負なら差し込むと下がる**)。
    /// 主軸自身は外す選択肢が無いので `None`
    pub expected_dps_gain: Option<f64>,
    /// この技が出している期待 DPS(回しの中での取り分。<フラグ> 爆発ぶんを含む)
    pub expected_dps: f64,
    /// 回しの期待 DPS に占める割合(0〜1)
    pub dps_share: f64,
    /// 1 分あたりに撃つ回数
    pub uses_per_minute: f64,
}

/// 本体 + 召喚獣 + <フラグ> の合計(合計 DPS は単純和。本体は召喚中も自由に撃てるため)。
/// 召喚獣を持たないキャラ・召喚スキル未選択・<フラグ> 無しなら本体単独の値と同じ。
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct CombinedDamage {
    /// 1 発の主役数字 = 技の合計ダメージ + <フラグ> 爆発の合計ダメージ。
    /// 召喚獣は別の攻撃者なのでここには足さない(DPS だけで合流する)
    pub total_primary: i64,
    /// 側(最小 / 最大 / クリ)ごとに足した 1 秒あたり。画面は側を選ぶだけでよい
    pub dps: Option<domain::DpsTriple>,
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
    /// <フラグ> を 1 スタック以上積んでいるときだけ `Some`(イェフネン)
    pub flag: Option<FlagDamage>,
    /// 回し(連打する技 + 差し込む CT 技)。差し込む CT 技が無いなら `None` で、
    /// DPS は技そのものの値
    pub rotation: Option<Rotation>,
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

/// <フラグ>(技とは別枠のダメージ)1 件ぶんを計算する。技とまったく同じ材料を通すので、
/// 自己バフ・敵デバフ(ブレンド)・割合追加ダメージは技と同じように効く。違うのは 3 つだけ:
///
/// - 技データは `gamedata::flag_skill`(HACK 依存・物理・無属性・<フラグ> の倍率 / 段数 / Cri倍率)
/// - マスタリー3(欠片)の ±% を E1 として**ここにだけ**足す(技には効かない)
/// - コンボボーナス(倍率A)は乗せない(技の一撃ではないため `combo_count = 0`)
///
/// スタック 0 なら `None`。
#[allow(clippy::too_many_arguments)]
fn build_flag_damage(
    base_stats: &domain::BaseStats,
    game_character_id: &str,
    style_dependency: Option<domain::SkillDependency>,
    stat_sources: &domain::StatSources,
    buffs: &BuffSelection,
    equipment: &domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    skill: &Skill,
    stacks: u8,
    part: gamedata::FlagPart,
    enemy: &Enemy,
    content: &domain::Content,
    temporary_adjustments: Option<&domain::Adjustments>,
) -> CommandResult<Option<DamageResult>> {
    let Some(flag_skill) = gamedata::flag_skill(skill, stacks, part) else {
        return Ok(None);
    };
    let (material, mut target) = build_damage_input(
        base_stats,
        game_character_id,
        style_dependency,
        stat_sources,
        buffs,
        equipment.clone(),
        common_skills,
        awakening,
        flag_skill,
        enemy.clone(),
        content,
        0,
        None,
        temporary_adjustments.cloned(),
    )?;
    target
        .damage_contributions
        .extend(gamedata::flag_damage_contributions(
            &stat_sources.masteries,
            part,
        ));
    Ok(Some(domain::calculate_damage(&material, &target)))
}

/// 連打技 1 回で積む <フラグ> の数から、次の爆発までに積み直すのに要る回数を出す。
/// 爆発させる技を差し込むときの `minimum_filler_uses`(`domain::RotationInsert`)。
fn flag_reapply_uses(form: domain::SkillForm, stacks: u8) -> u32 {
    let per_use = gamedata::flag_stacks_per_use(form);
    if per_use == 0 {
        return 0;
    }
    u32::from(gamedata::flag_stacks_to_apply(form, stacks)).div_ceil(u32::from(per_use))
}

/// 回しの候補 1 件ぶんの材料(1 回の結果・<フラグ> の爆発・積み直しの回数)。
struct RotationCandidateMaterial {
    skill: Skill,
    result: DamageResult,
    /// この技が起こす <フラグ> の爆発 1 回ぶん
    burst: Option<DamageResult>,
    /// 撃つ前に連打技を最低何回挟むか(<フラグ> の積み直し)
    minimum_filler_uses: u32,
}

impl RotationCandidateMaterial {
    /// 1 回で同時に入るダメージ(技本体 + それが起こす <フラグ> の爆発)。
    fn damage(&self) -> Vec<domain::RotationDamage> {
        let mut part = vec![domain::RotationDamage::of(&self.result)];
        if let Some(burst) = self.burst.as_ref() {
            part.push(domain::RotationDamage::of(burst));
        }
        part
    }
}

/// 回しの材料一式。計算タブの内訳(`build_rotation`)とキャラタブの候補
/// (`list_rotation_choices`)が**同じ 1 本**で作る。ここまでが「1 回ぶんを計算する」段で、
/// ここから先(どれを差し込むと何 DPS か)は domain の純関数だけで回せる。
struct RotationMaterials {
    candidates: Vec<RotationCandidateMaterial>,
    /// 既定の役割(`domain::choose_rotation`)
    roles: domain::RotationRoles,
    /// 主軸自身を差し込むときの積み直しの回数
    main_minimum_filler_uses: u32,
}

impl RotationMaterials {
    fn candidate_ids(&self) -> Vec<&str> {
        self.candidates
            .iter()
            .map(|candidate| candidate.skill.id.as_str())
            .collect()
    }
    fn skill_of<'a>(&'a self, main: &'a Skill, role: domain::RotationRole) -> &'a Skill {
        match role {
            domain::RotationRole::Main => main,
            domain::RotationRole::Candidate(index) => &self.candidates[index].skill,
        }
    }
    fn result_of<'a>(&'a self, body: &'a DamageResult, role: domain::RotationRole) -> &'a DamageResult {
        match role {
            domain::RotationRole::Main => body,
            domain::RotationRole::Candidate(index) => &self.candidates[index].result,
        }
    }
    fn minimum_filler_uses_of(&self, role: domain::RotationRole) -> u32 {
        match role {
            domain::RotationRole::Main => self.main_minimum_filler_uses,
            domain::RotationRole::Candidate(index) => self.candidates[index].minimum_filler_uses,
        }
    }
}

/// 同じキャラ・同じ形態のプレイヤー攻撃技を、主軸とまったく同じ経路
/// (`build_damage_input` → コンボ)で **1 度だけ**計算し、既定の役割まで決める。
#[allow(clippy::too_many_arguments)]
fn rotation_materials(
    base_stats: &domain::BaseStats,
    game_character_id: &str,
    style_dependency: Option<domain::SkillDependency>,
    stat_sources: &domain::StatSources,
    buffs: &BuffSelection,
    equipment: &domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    skill: &Skill,
    body: &DamageResult,
    flag_stacks: u8,
    enemy: &Enemy,
    content: &domain::Content,
    combo_count: u32,
    normal_attack_id: Option<&str>,
    temporary_adjustments: Option<&domain::Adjustments>,
) -> CommandResult<RotationMaterials> {
    // 1 回ぶんを主軸と同じ経路で計算する(解決済みの技と結果を返す)
    let calculate = |skill: Skill| -> CommandResult<(Skill, DamageResult)> {
        let (material, target) = build_damage_input(
            base_stats,
            game_character_id,
            style_dependency,
            stat_sources,
            buffs,
            equipment.clone(),
            common_skills,
            awakening,
            skill,
            enemy.clone(),
            content,
            combo_count,
            None,
            temporary_adjustments.cloned(),
        )?;
        let result = damage_with_optional_combo(&material, &target, combo_count, normal_attack_id)?;
        Ok((target.skill, result))
    };
    // 候補: 同じキャラ・同じ形態(形態を持つキャラ)のプレイヤー攻撃技。主軸は含めない
    let candidate_skills: Vec<Skill> = gamedata::skills_for(game_character_id)
        .into_iter()
        .filter(|s| s.attacker == domain::Attacker::Player)
        .map(|s| resolve_skill_variants(s, stat_sources))
        .filter(|s| s.form == skill.form && s.id != skill.id)
        .collect();
    // <フラグ> を爆発させる技は、爆発ぶんのダメージと積み直しの回数が付く
    let reapply_uses = |candidate: &Skill| match candidate.form {
        Some(form) if candidate.detonates_flag && flag_stacks > 0 => {
            flag_reapply_uses(form, flag_stacks)
        }
        _ => 0,
    };
    let mut candidates: Vec<RotationCandidateMaterial> = Vec::with_capacity(candidate_skills.len());
    for candidate in candidate_skills {
        let (candidate, result) = calculate(candidate)?;
        let burst = if candidate.detonates_flag && flag_stacks > 0 {
            build_flag_damage(
                base_stats,
                game_character_id,
                style_dependency,
                stat_sources,
                buffs,
                equipment,
                common_skills,
                awakening,
                &candidate,
                flag_stacks,
                gamedata::FlagPart::Burst,
                enemy,
                content,
                temporary_adjustments,
            )?
        } else {
            None
        };
        candidates.push(RotationCandidateMaterial {
            minimum_filler_uses: reapply_uses(&candidate),
            skill: candidate,
            result,
            burst,
        });
    }
    // 爆発させる技を撃つなら、その <フラグ> を積む技(同形態の 連 / 爆)を連打する
    let pinned_filler_id = (skill.detonates_flag && flag_stacks > 0)
        .then(|| gamedata::flag_applier_for(skill))
        .flatten()
        .map(|applier| applier.id);
    // 1 回で同時に入るダメージ(技本体 + それが起こす <フラグ> の爆発)。
    // 差し込むと DPS が上がるかを `choose_rotation` がその場で確かめるのに要る
    let damages: Vec<Vec<domain::RotationDamage>> = candidates
        .iter()
        .map(|candidate| candidate.damage())
        .collect();
    let roles = domain::choose_rotation(
        skill,
        body.cycle_seconds(),
        Some(domain::RotationDamage::of(body)),
        &candidates
            .iter()
            .zip(&damages)
            .map(|(candidate, damage)| domain::RotationCandidate {
                skill: &candidate.skill,
                seconds: candidate.result.cycle_seconds(),
                // 爆発させる技は、連打している主軸で積んだ <フラグ> を使う技だけ差し込める
                insertable: !(candidate.skill.detonates_flag && flag_stacks > 0)
                    || gamedata::flag_applier_for(&candidate.skill)
                        .is_some_and(|a| a.id == skill.id),
                damage,
                minimum_filler_uses: candidate.minimum_filler_uses,
            })
            .collect::<Vec<_>>(),
        pinned_filler_id.as_deref(),
    );
    Ok(RotationMaterials {
        main_minimum_filler_uses: reapply_uses(skill),
        candidates,
        roles,
    })
}

/// 回しを組む(`Rotation` 参照)。
///
/// 役割(どれを連打し、どれを差し込むか)は `domain::choose_rotation` が決める —
/// 計算タブもホームの到達一覧も同じ 1 本を通す。判定の基準は**実際の 1 回の所要時間**
/// (`DamageResult::cycle_seconds`)なので、候補もここで 1 回ぶんを計算する。
///
/// 候補は同じキャラ・同じ形態のプレイヤー攻撃技で、主軸とまったく同じ経路
/// (`build_damage_input` → コンボ)を通すので、速剣などの形態限定の効果も同じように効く。
/// 差し込む技が <フラグ> を爆発させるなら、連打技は同形態の 連 / 爆
/// (`gamedata::flag_applier_for`)に固定し、積み直しに要る回数を最低回数にして、
/// その技のダメージに爆発ぶんを足す。
///
/// 回しを組めない(差し込む技が無い・所要時間が出せない)なら `None`
/// (DPS は技そのものの値のまま)。
///
/// `insert_skill_ids` は画面から明示された差し込み(`None` = 既定、`Some([])` = 差し込まない)。
#[allow(clippy::too_many_arguments)]
fn build_rotation(
    base_stats: &domain::BaseStats,
    game_character_id: &str,
    style_dependency: Option<domain::SkillDependency>,
    stat_sources: &domain::StatSources,
    buffs: &BuffSelection,
    equipment: &domain::Equipment,
    common_skills: CommonSkills,
    awakening: domain::Awakening,
    skill: &Skill,
    body: &DamageResult,
    flag_stacks: u8,
    flag_burst: Option<&DamageResult>,
    enemy: &Enemy,
    content: &domain::Content,
    combo_count: u32,
    normal_attack_id: Option<&str>,
    temporary_adjustments: Option<&domain::Adjustments>,
    insert_skill_ids: Option<&[String]>,
) -> CommandResult<Option<Rotation>> {
    let materials = rotation_materials(
        base_stats,
        game_character_id,
        style_dependency,
        stat_sources,
        buffs,
        equipment,
        common_skills,
        awakening,
        skill,
        body,
        flag_stacks,
        enemy,
        content,
        combo_count,
        normal_attack_id,
        temporary_adjustments,
    )?;
    let roles = &materials.roles;
    // 主軸を**連打**しながら <フラグ> を爆発させる回しは、爆発をどの間隔で入れるか決まらない
    // (爆発は主軸 1 回につき 1 度だが、連打の回数は時間配分の結果として決まる)。
    // ADR-019 決定 8 と同じく、爆発を黙って落とした「確定値」を出さない —— 回しを組まずに
    // 返すと `combine_damage` が DPS を不明にする(ホーム評価も同じ規則)
    if flag_burst.is_some() && roles.filler == Some(domain::RotationRole::Main) {
        return Ok(None);
    }
    // 明示指定があれば既定の差し込みを置き換える(規則は domain。ホーム評価と同じ 1 本)。
    // 候補に無い id(形態違い・改名・消えた技)は domain が黙って落とす
    let insert_roles =
        domain::apply_explicit_inserts(roles, insert_skill_ids, &skill.id, &materials.candidate_ids());

    // 差し込む技 1 つぶんの材料(技・1 回の結果・爆発・CT・積み直しの回数)
    struct Insert {
        skill_id: String,
        skill_name: String,
        is_main: bool,
        result: Option<DamageResult>,
        burst: Option<DamageResult>,
        seconds: Option<f64>,
        cooldown_seconds: f64,
        minimum_filler_uses: u32,
    }
    let skill_of = |role: domain::RotationRole| -> &Skill { materials.skill_of(skill, role) };
    let result_of = |role: domain::RotationRole| -> &DamageResult { materials.result_of(body, role) };
    let mut inserts: Vec<Insert> = Vec::with_capacity(insert_roles.len());
    for role in insert_roles {
        let insert_skill = skill_of(role);
        let is_main = role == domain::RotationRole::Main;
        // <フラグ> を爆発させる技は、爆発ぶんのダメージと積み直しの回数が付く。
        // 主軸ぶんの爆発は計算済み(`FlagDamage::burst`)なのでここでは持たない
        let burst = match role {
            domain::RotationRole::Main => None,
            domain::RotationRole::Candidate(index) => materials.candidates[index].burst.clone(),
        };
        let minimum_filler_uses = materials.minimum_filler_uses_of(role);
        inserts.push(Insert {
            skill_id: insert_skill.id.clone(),
            skill_name: insert_skill.name.clone(),
            is_main,
            result: (!is_main).then(|| result_of(role).clone()),
            burst,
            seconds: result_of(role).cycle_seconds(),
            cooldown_seconds: insert_skill.cooldown_seconds.unwrap_or(0.0),
            minimum_filler_uses,
        });
    }

    let filler_result = roles.filler.map(result_of);
    let filler_seconds = filler_result.and_then(DamageResult::cycle_seconds);
    let Some(plan) = domain::plan_rotation(
        filler_seconds,
        &inserts
            .iter()
            .map(|insert| domain::RotationInsert {
                seconds: insert.seconds,
                cooldown_seconds: insert.cooldown_seconds,
                minimum_filler_uses: insert.minimum_filler_uses,
            })
            .collect::<Vec<_>>(),
    ) else {
        return Ok(None);
    };
    // 1 回で同時に入るダメージ(技本体 + それが起こす <フラグ> の爆発)
    let parts: Vec<Vec<domain::RotationDamage>> = inserts
        .iter()
        .map(|insert| {
            let mut part = vec![domain::RotationDamage::of(
                insert.result.as_ref().unwrap_or(body),
            )];
            let burst = insert
                .burst
                .as_ref()
                .or(if insert.is_main { flag_burst } else { None });
            if let Some(burst) = burst {
                part.push(domain::RotationDamage::of(burst));
            }
            part
        })
        .collect();
    let parts: Vec<&[domain::RotationDamage]> = parts.iter().map(Vec::as_slice).collect();
    let filler_damage = filler_result
        .map(domain::RotationDamage::of)
        .zip(filler_seconds);
    // 技ごとの取り分(帯と「この技が出している DPS」)。合計はその総和なので、
    // 内訳と合計が食い違わない(画面は足し算も割り算もしない)
    let Some(shares) = domain::rotation_shares(&plan, &parts, filler_damage) else {
        return Ok(None);
    };
    let (dps, expected_dps) = shares.totals();
    let dps_share = |share: &domain::RotationShare| {
        if expected_dps > 0.0 {
            share.expected_dps / expected_dps
        } else {
            0.0
        }
    };
    // 「その技を差し込まない回し」との差。同じ材料をもう一度通すだけ(計算はやり直さない)
    let plan_inserts: Vec<domain::RotationInsert> = inserts
        .iter()
        .map(|insert| domain::RotationInsert {
            seconds: insert.seconds,
            cooldown_seconds: insert.cooldown_seconds,
            minimum_filler_uses: insert.minimum_filler_uses,
        })
        .collect();
    let gain_of = |index: usize| {
        domain::insert_expected_dps_gain(
            expected_dps,
            filler_seconds,
            &plan_inserts,
            &parts,
            filler_damage,
            index,
        )
    };
    // 画面に出すのは実際に撃つ技だけ(撃てない差し込みは plan が落としている)
    let mut inserts: Vec<Option<Insert>> = inserts.into_iter().map(Some).collect();
    let inserts: Vec<RotationInsert> = plan
        .slots
        .iter()
        .enumerate()
        .filter_map(|(at, slot)| {
            let insert = inserts.get_mut(slot.insert)?.take()?;
            let share = shares.inserts.get(at)?;
            // 爆発は差し込む技 1 回につき 1 度なので、その間隔で割った DPS を持たせる
            let burst = insert.burst.map(|mut burst| {
                domain::apply_fixed_interval_dps(&mut burst, slot.interval_seconds);
                burst
            });
            Some(RotationInsert {
                // 主軸は「差し込まない」選択肢が無いので損得を出さない
                expected_dps_gain: (!insert.is_main).then(|| gain_of(slot.insert)).flatten(),
                skill_id: insert.skill_id,
                skill_name: insert.skill_name,
                is_main: insert.is_main,
                result: insert.result,
                burst,
                seconds: insert.seconds.unwrap_or(0.0),
                cooldown_seconds: insert.cooldown_seconds,
                interval_seconds: slot.interval_seconds,
                filler_uses: slot.filler_uses,
                cooldown_bound: slot.cooldown_bound,
                expected_dps: share.expected_dps,
                dps_share: dps_share(share),
                uses_per_minute: share.uses_per_second * 60.0,
            })
        })
        .collect();
    Ok(Some(Rotation {
        filler: roles
            .filler
            .zip(filler_seconds)
            .zip(shares.filler)
            .map(|((role, seconds), share)| {
                let filler_skill = skill_of(role);
                RotationFiller {
                    skill_id: filler_skill.id.clone(),
                    skill_name: filler_skill.name.clone(),
                    is_main: role == domain::RotationRole::Main,
                    result: (role != domain::RotationRole::Main).then(|| result_of(role).clone()),
                    seconds,
                    expected_dps: share.expected_dps,
                    dps_share: dps_share(&share),
                    uses_per_minute: share.uses_per_second * 60.0,
                }
            }),
        inserts,
        filler_share: plan.filler_share,
        crowded: plan.crowded,
        dps,
        expected_dps,
    }))
}

/// 本体 DPS + 召喚獣 DPS + <フラグ> の単純和(本体は召喚中も自由に撃てるため)。
/// 召喚獣も <フラグ> も無ければ本体の値のまま。
///
/// <フラグ> の乗せ方(ユーザー決定 2026-09-21):
/// - 1 発の主役数字 = 技の合計 + 爆発の合計(爆発は主軸が スレイ / クラッシュ のときだけ)
/// - 持続は主役数字に入れず、`期待値 ÷ 周期(1 秒)` を DPS に足す
/// - 爆発は差し込む技 1 回につき 1 度なので、回しの中でその技のダメージに足す
fn combine_damage(
    body: &DamageResult,
    summon: Option<&SummonDamage>,
    flag: Option<&FlagDamage>,
    rotation: Option<&Rotation>,
) -> CombinedDamage {
    let burst = flag.and_then(|f| f.burst.as_ref());
    // 技の火力の基準。CT 技を差し込むなら回し全体の DPS で出す(CT 技を連打できる前提にも、
    // 主軸の CT 中に何もしていない前提にもしない)。回しには主軸も爆発も入っているので、
    // それぞれの DPS を別に足さない
    let (mut dps, mut expected_dps) = match rotation {
        Some(rotation) => (Some(rotation.dps), Some(rotation.expected_dps)),
        // 回しを組めないのに爆発がある = 爆発をどの間隔で入れるか決まらない。
        // 爆発を無視した「確定値」を出さず、DPS は不明にする
        None if burst.is_some() => (None, None),
        None => (body.dps, body.expected_dps),
    };
    // 持続は 1 周とは無関係に 1 秒ごと入る。技の DPS が出せないときは足さない
    // (「技の火力は不明なのに討伐時間が出ている」画面にしない)
    if dps.is_some() {
        if let Some(duration) = flag.map(|f| &f.duration) {
            dps = domain::combine_dps(dps, duration.dps);
            expected_dps = domain::combine_expected_dps(expected_dps, duration.expected_dps);
        }
    }
    if let Some(summon) = summon {
        dps = domain::combine_dps(dps, summon.result.dps);
        expected_dps = domain::combine_expected_dps(expected_dps, summon.result.expected_dps);
    }
    let defeat_seconds = domain::defeat_seconds(body.enemy_hp, expected_dps);
    CombinedDamage {
        total_primary: body.total_primary + burst.map_or(0, |b| b.total_primary),
        dps,
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
    // 回しに差し込む CT 技(None = 既定で選ぶ、Some([]) = 差し込まない)
    rotation_skill_ids: Option<&[String]>,
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
    // <フラグ>(技とは別枠のダメージ)。積んでいなければ None。爆発は主軸が
    // スレイ / クラッシュ(`Skill::detonates_flag`)のときだけ出す
    let stacks = gamedata::flag_stacks(&stat_sources.character_skills);
    let flag_for = |part| {
        build_flag_damage(
            base_stats,
            game_character_id,
            style_dependency,
            stat_sources,
            buffs,
            &equipment,
            common_skills,
            awakening,
            &target.skill,
            stacks,
            part,
            &enemy,
            &content,
            temporary_adjustments.as_ref(),
        )
    };
    let mut duration = flag_for(gamedata::FlagPart::Duration)?;
    let mut burst = if target.skill.detonates_flag {
        flag_for(gamedata::FlagPart::Burst)?
    } else {
        None
    };
    // 持続は周期(1 秒)ごと。中ディレイを持たない別枠なので「この秒数に 1 回」を当てる
    if let Some(duration) = duration.as_mut() {
        domain::apply_fixed_interval_dps(duration, gamedata::FLAG_TICK_SECONDS);
    }
    // 回し(連打する技 + 差し込む CT 技)。差し込む CT 技が無いキャラは None
    let rotation = build_rotation(
        base_stats,
        game_character_id,
        style_dependency,
        stat_sources,
        buffs,
        &equipment,
        common_skills,
        awakening,
        &target.skill,
        &body,
        stacks,
        burst.as_ref(),
        &enemy,
        &content,
        combo_count,
        normal_attack_id,
        temporary_adjustments.as_ref(),
        rotation_skill_ids,
    )?;
    // 爆発は主軸 1 回につき 1 度なので、主軸を撃つ間隔で割った DPS を持たせる(内訳の 1 行ぶん)
    if let Some(burst) = burst.as_mut() {
        let interval = rotation
            .as_ref()
            .and_then(|rotation| rotation.inserts.iter().find(|insert| insert.is_main))
            .map(|insert| insert.interval_seconds)
            .or_else(|| body.cycle_seconds());
        if let Some(interval) = interval {
            domain::apply_fixed_interval_dps(burst, interval);
        }
    }
    let flag = match gamedata::flag_multiplier(stacks) {
        Some(multiplier) => {
            duration.map(|duration| FlagDamage {
                stacks,
                multiplier,
                duration,
                burst,
                tick_seconds: gamedata::FLAG_TICK_SECONDS,
                lasts_seconds: gamedata::FLAG_DURATION_SECONDS,
            })
        }
        None => None,
    };
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
    let combined = combine_damage(&body, summon.as_ref(), flag.as_ref(), rotation.as_ref());
    Ok(CharacterDamageResult {
        body,
        summon,
        flag,
        rotation,
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
    let rotation_skill_ids = character.rotation_skill_ids.clone();
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
        rotation_skill_ids.as_deref(),
    )
}

/// キャラタブの「差し込む CT 技」1 件ぶん。候補も既定 ON も損得も**回しの規則そのもの**
/// (`damage_for_character` → `build_rotation`)から出すので、画面は CT 判定を持たない。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RotationInsertChoice {
    pub skill_id: String,
    pub skill_name: String,
    pub cooldown_seconds: f64,
    /// 未設定(`rotation_skill_ids` が `None`)のとき既定で差し込まれる技か
    pub default_on: bool,
    /// いまの選択にこの技を足したときの期待 DPS の差(**負なら差し込むと下がる**)。
    /// 既に選んでいる技なら計算タブの内訳に出ている数と同じ。出せないなら `None`
    pub expected_dps_gain: Option<f64>,
}

/// 「差し込む CT 技」の候補一式(キャラタブ)。
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RotationChoices {
    /// 差し込める技。0 件なら画面は行ごと出さない
    pub candidates: Vec<RotationInsertChoice>,
    /// 合間に連打する技の名前(主軸が CT 技のときは自動で決まる)。主軸を連打するなら `None`
    pub filler_skill_name: Option<String>,
    /// いまの選択どおりに組んだ回しの期待 DPS(計算タブの `Rotation::expected_dps` と同じ値)。
    /// 画面が `expected_dps_gain` を**割合**で見せるための分母。組めないなら `None`
    pub expected_dps: Option<f64>,
}

/// 「差し込む CT 技」の候補・既定 ON・差し込んだときの損得を返す(キャラタブの入力欄)。
///
/// 候補の母集団は主軸と同じ形態のプレイヤー攻撃技で CT を持つものだが、**実際に差し込めるか**
/// (実効 CT が 1 回の所要時間を超えるか・所要時間が出せるか)は回しに通して決める —
/// 画面にも、ここにも、`rotation::effective_cooldown_seconds` の写しを置かない。
///
/// 損得は「いまの選択にその技を足した回し」で出す。既に選んでいる技なら計算タブの内訳
/// (`RotationInsert::expected_dps_gain`)と同じ数になる。
///
/// `skill_id` 以降は**計算タブの材料**。計算タブはそこで選び直した技・コンボ・一時調整で
/// 回しを組んでいるので、候補と損得も同じ材料で出す(チップの損得と実際の DPS の動きが
/// 食い違わないようにする)。キャラタブは `None` / `0` を渡す = 主軸・コンボなし・調整なし。
/// **依存種別(`style_dependency`)は計算タブと同じくキャラの主軸から決める** ——
/// `damage_for_character` がそうしているので、ここだけ別の技から取らない。
#[allow(clippy::too_many_arguments)]
pub fn list_rotation_choices(
    character: NewCharacter,
    buffs: BuffSelection,
    content_id: String,
    skill_id: Option<String>,
    combo_count: u32,
    combo_skill_type: Option<domain::ComboSkillType>,
    normal_attack_id: Option<String>,
    temporary_adjustments: Option<domain::Adjustments>,
) -> CommandResult<RotationChoices> {
    validate_character_draft(&character, &buffs)?;
    let style_dependency = character
        .main_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(|skill| skill.dependency);
    // 回しの主軸は「いま計算している技」。計算タブが技を選び直していなければキャラの主軸
    let Some(main_skill_id) = skill_id.or_else(|| character.main_skill_id.clone()) else {
        return Ok(RotationChoices::default());
    };
    let main_skill = find_skill(&main_skill_id)?;
    let (content, enemy) = find_content_with_enemy(&content_id)?;
    let normal_attack_id = normal_attack_id.as_deref();
    // 主軸の 1 回ぶん(計算タブとまったく同じ経路)
    let (material, target) = build_damage_input(
        &character.base_stats,
        &character.game_character_id,
        style_dependency,
        &character.stat_sources,
        &buffs,
        character.equipment.clone(),
        character.common_skills,
        character.awakening,
        main_skill,
        enemy.clone(),
        &content,
        combo_count,
        combo_skill_type,
        temporary_adjustments.clone(),
    )?;
    let body = damage_with_optional_combo(&material, &target, combo_count, normal_attack_id)?;
    let main = &target.skill;
    let flag_stacks = gamedata::flag_stacks(&character.stat_sources.character_skills);
    let main_burst = if main.detonates_flag && flag_stacks > 0 {
        build_flag_damage(
            &character.base_stats,
            &character.game_character_id,
            style_dependency,
            &character.stat_sources,
            &buffs,
            &character.equipment,
            character.common_skills,
            character.awakening,
            main,
            flag_stacks,
            gamedata::FlagPart::Burst,
            &enemy,
            &content,
            temporary_adjustments.as_ref(),
        )?
    } else {
        None
    };
    // 同形態の技の 1 回ぶんは**ここで 1 度だけ**計算する。以降は domain の純関数だけを回す
    let materials = rotation_materials(
        &character.base_stats,
        &character.game_character_id,
        style_dependency,
        &character.stat_sources,
        &buffs,
        &character.equipment,
        character.common_skills,
        character.awakening,
        main,
        &body,
        flag_stacks,
        &enemy,
        &content,
        combo_count,
        normal_attack_id,
        temporary_adjustments.as_ref(),
    )?;
    let roles = &materials.roles;
    let candidate_ids = materials.candidate_ids();
    let filler_result = roles.filler.map(|role| materials.result_of(&body, role));
    let filler_seconds = filler_result.and_then(DamageResult::cycle_seconds);
    let filler_damage = filler_result
        .map(domain::RotationDamage::of)
        .zip(filler_seconds);
    let filler_skill_name = roles
        .filler
        .filter(|role| *role != domain::RotationRole::Main)
        .map(|role| materials.skill_of(main, role).name.clone());

    let insert_of = |role: domain::RotationRole| domain::RotationInsert {
        seconds: materials.result_of(&body, role).cycle_seconds(),
        cooldown_seconds: materials
            .skill_of(main, role)
            .cooldown_seconds
            .unwrap_or(0.0),
        minimum_filler_uses: materials.minimum_filler_uses_of(role),
    };
    // 1 回で同時に入るダメージ(技本体 + それが起こす <フラグ> の爆発)
    let parts_of = |role: domain::RotationRole| {
        let mut part = vec![domain::RotationDamage::of(materials.result_of(&body, role))];
        let burst = match role {
            domain::RotationRole::Main => main_burst.as_ref(),
            domain::RotationRole::Candidate(index) => materials.candidates[index].burst.as_ref(),
        };
        if let Some(burst) = burst {
            part.push(domain::RotationDamage::of(burst));
        }
        part
    };

    // 既定(未設定のときに ON になる技)といま選んでいる組み合わせ
    let default_ids: Vec<String> = roles
        .inserts
        .iter()
        .filter(|role| **role != domain::RotationRole::Main)
        .map(|role| materials.skill_of(main, *role).id.clone())
        .collect();
    let selection: Vec<String> = character
        .rotation_skill_ids
        .clone()
        .unwrap_or_else(|| default_ids.clone());

    // いまの選択どおりに組んだ回しの期待 DPS(損得を割合で見せるための分母)。
    // 計算タブが出している `Rotation::expected_dps` と同じ材料・同じ規則
    let expected_dps_of = |ids: &[String]| -> Option<f64> {
        let insert_roles = domain::apply_explicit_inserts(roles, Some(ids), &main.id, &candidate_ids);
        let inserts: Vec<domain::RotationInsert> =
            insert_roles.iter().map(|role| insert_of(*role)).collect();
        let owned_parts: Vec<Vec<domain::RotationDamage>> =
            insert_roles.iter().map(|role| parts_of(*role)).collect();
        let parts: Vec<&[domain::RotationDamage]> = owned_parts.iter().map(Vec::as_slice).collect();
        let plan = domain::plan_rotation(filler_seconds, &inserts)?;
        domain::rotation_dps(&plan, &parts, filler_damage).map(|(_, expected)| expected)
    };
    // 差し込みが 1 つも無いなら、連打技を撃ち続けるだけの DPS が分母
    // (`insert_expected_dps_gain` が比べている相手と同じ)
    let expected_dps = expected_dps_of(&selection).or_else(|| {
        filler_damage
            .map(|(damage, seconds)| damage.total.expected(damage.critical_chance) / seconds)
    });

    let mut candidates = Vec::new();
    for index in 0..materials.candidates.len() {
        let role = domain::RotationRole::Candidate(index);
        let candidate = materials.skill_of(main, role);
        // 爆発させる技は、連打している主軸で積んだ <フラグ> を使う技だけ差し込める
        // (`choose_rotation` の insertable と同じ印)
        if candidate.detonates_flag
            && flag_stacks > 0
            && !gamedata::flag_applier_for(candidate).is_some_and(|a| a.id == main.id)
        {
            continue;
        }
        // 実際に差し込めるか(実効 CT があるか・所要時間が出せるか)は回しの規則そのものに問う
        if domain::plan_rotation(filler_seconds, &[insert_of(role)]).is_none() {
            continue;
        }
        let skill_id = candidate.id.clone();
        let mut ids = selection.clone();
        if !ids.contains(&skill_id) {
            ids.push(skill_id.clone());
        }
        // その技を**入れた**回しと**入れない**回しの差。ON なら「外したとの差」、
        // OFF なら「足したとの差」で、どちらも「入れると DPS がどう動くか」で符号が揃う
        let insert_roles =
            domain::apply_explicit_inserts(roles, Some(&ids), &main.id, &candidate_ids);
        let inserts: Vec<domain::RotationInsert> =
            insert_roles.iter().map(|role| insert_of(*role)).collect();
        let owned_parts: Vec<Vec<domain::RotationDamage>> =
            insert_roles.iter().map(|role| parts_of(*role)).collect();
        let parts: Vec<&[domain::RotationDamage]> =
            owned_parts.iter().map(Vec::as_slice).collect();
        let expected_dps_gain = (|| {
            let plan = domain::plan_rotation(filler_seconds, &inserts)?;
            let (_, expected) = domain::rotation_dps(&plan, &parts, filler_damage)?;
            let at = insert_roles.iter().position(|r| *r == role)?;
            domain::insert_expected_dps_gain(
                expected,
                filler_seconds,
                &inserts,
                &parts,
                filler_damage,
                at,
            )
        })();
        candidates.push(RotationInsertChoice {
            default_on: default_ids.contains(&skill_id),
            cooldown_seconds: candidate.cooldown_seconds.unwrap_or(0.0),
            skill_name: candidate.name.clone(),
            skill_id,
            expected_dps_gain,
        });
    }
    Ok(RotationChoices {
        candidates,
        filler_skill_name,
        expected_dps,
    })
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
    // 主軸スキルは手首補正の振り先にしか使わないので、カタログに無い id(スキルの改名を
    // またいだ古いキャラ)でもホームの到達一覧は出す(防御・対人と同じ扱い)
    let equipment_base = inputs.context(
        &character.base_stats,
        character.stat_sources.soul_link,
        &buffs,
        character
            .main_skill_id
            .as_deref()
            .and_then(gamedata::find_skill)
            .map(|skill| skill.dependency),
    );
    // スキルごとに変わるがコンテンツには依存しない値(依存種別の係数・カテゴリ寄与・
    // 属性値)は、コンテンツの数だけ繰り返さずキャラのスキル数ぶんだけ 1 回作る。
    // 召喚獣(熊・破壊精霊)が撃つスキルは本体の最良スキル判定には含めない(本体は召喚獣の
    // スキルを自分で振れないため)。召喚獣ぶんの期待 DPS は別に集計して後段で合算する
    // 1 スキルぶんの入力。<フラグ> の 2 本(持続 / 爆発)も同じ形で作る(二重実装しない)
    let input_of = |skill: Skill| SkillEvaluationInput {
        coefficients: coefficients_for(skill.attacker, skill.dependency),
        damage_contributions: damage_inputs::damage_contributions_of(
            &character.stat_sources,
            &buffs,
            &character.equipment,
            &skill,
        ),
        skill_added_damage_rate: damage_inputs::added_damage_rate_of(
            &character.stat_sources,
            &skill,
        ),
        element_value: damage_inputs::element_value_for(
            &character.game_character_id,
            &character.equipment,
            &character.stat_sources,
            &buffs,
            &skill,
        ),
        skill,
        flag: None,
        rotation: None,
    };
    // <フラグ>(技とは別枠のダメージ)。積んでいなければ 0 で、他キャラでは必ず 0
    let flag_stacks = gamedata::flag_stacks(&character.stat_sources.character_skills);
    // <フラグ> 1 本ぶんの入力。マスタリー3(欠片)の ±% は E1 として**ここにだけ**足す
    let flag_input_of = |base: &Skill, part| {
        gamedata::flag_skill(base, flag_stacks, part).map(|flag_skill| {
            let mut input = input_of(flag_skill);
            input.damage_contributions.extend(
                gamedata::flag_damage_contributions(&character.stat_sources.masteries, part),
            );
            Box::new(input)
        })
    };
    // 速剣・最大チャージは技データ側の差し替えなので、評価に使う技も解決してから入れる。
    // 召喚獣(熊・破壊精霊)が撃つスキルは本体が自分で振れないので入れない
    let player_skills: Vec<Skill> = skills
        .iter()
        .filter(|skill| skill.attacker == domain::Attacker::Player)
        .map(|skill| resolve_skill_variants(skill.clone(), &character.stat_sources))
        .collect();
    // 差し込んだときに付いてくるもの(<フラグ> の爆発と積み直しの回数)。
    // 計算タブ(`build_rotation`)と同じ組み立て
    let insert_material_of = |skill: &Skill| {
        let reapply = match skill.form {
            Some(form) if skill.detonates_flag && flag_stacks > 0 => Some(form),
            _ => None,
        };
        domain::RotationInsertMaterial {
            burst: reapply.and_then(|_| flag_input_of(skill, gamedata::FlagPart::Burst)),
            minimum_filler_uses: reapply.map_or(0, |form| flag_reapply_uses(form, flag_stacks)),
        }
    };
    // この技を主軸にしたときの回しの材料。**役割は決めない** — どれを連打してどれを
    // 差し込むかは domain(`rotation::choose_rotation`)が実際の所要時間を見て決める
    let rotation_of = |skill: &Skill| -> Option<domain::RotationEvaluationInput> {
        let candidates: Vec<domain::RotationCandidateEvaluationInput> = player_skills
            .iter()
            .filter(|s| s.form == skill.form && s.id != skill.id)
            .map(|candidate| domain::RotationCandidateEvaluationInput {
                // 爆発させる技は、連打している主軸で積んだ <フラグ> を使う技だけ差し込める
                insertable: !(candidate.detonates_flag && flag_stacks > 0)
                    || gamedata::flag_applier_for(candidate).is_some_and(|a| a.id == skill.id),
                material: insert_material_of(candidate),
                skill: Box::new(input_of(candidate.clone())),
            })
            .collect();
        if candidates.is_empty() {
            return None;
        }
        Some(domain::RotationEvaluationInput {
            candidates,
            // 爆発させる技を撃つなら、その <フラグ> を積む技(同形態の 連 / 爆)を連打する
            pinned_filler_id: (skill.detonates_flag && flag_stacks > 0)
                .then(|| gamedata::flag_applier_for(skill))
                .flatten()
                .map(|applier| applier.id),
            main: insert_material_of(skill),
            // 保存した「差し込む CT 技」は評価するどの技にも同じように当てる。「差し込まない」は
            // その人の戦い方の話なので、主軸以外を主軸に見立てた評価だけ差し込む形にしない。
            // その技の候補に無い id は domain 側が落とす(`apply_explicit_inserts`)
            insert_skill_ids: character.rotation_skill_ids.clone(),
        })
    };
    let skill_inputs: Vec<SkillEvaluationInput> = player_skills
        .iter()
        .map(|skill| {
            // 計算タブ(damage_for_character)と同じ組み立て: 持続は常に乗り、
            // 爆発は回しの中で「それを起こす技」に付く
            let flag = flag_input_of(skill, gamedata::FlagPart::Duration).map(|duration| {
                domain::FlagEvaluationInput {
                    duration,
                    tick_seconds: gamedata::FLAG_TICK_SECONDS,
                }
            });
            SkillEvaluationInput {
                flag,
                rotation: rotation_of(skill),
                ..input_of(skill.clone())
            }
        })
        .collect();
    // 召喚獣(熊・破壊精霊)ぶんの入力(召喚スキル未選択・召喚獣を持たないキャラは None)。
    let summon_input = character
        .summon_skill_id
        .as_deref()
        .map(find_skill)
        .transpose()?
        .map(&input_of);
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
    skill: Skill,
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
    // 敵データが無いコンテンツは候補の試算にも使えない(ここで弾く)
    let (content, _) = find_content_with_enemy(content_id)?;
    let skill = find_skill(skill_id)?;
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
        skill,
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
    /// 武器強化のように**表記は動かさず総量だけ増やす**候補があるので、片方だけでは拾えない。
    ///
    /// 総量と討伐時間は計算タブと同じ合算(`damage_for_character` の `combined` = 回し・
    /// <フラグ>・召喚獣込み)で出す。候補の効きめを計算タブと違う土俵で測らない。
    fn damage(
        &self,
        equipment: domain::Equipment,
        common_skills: CommonSkills,
    ) -> CommandResult<(i64, i64, Option<f64>)> {
        let c = self.character;
        let result = damage_for_character(
            &c.base_stats,
            &c.game_character_id,
            c.main_skill_id.as_deref(),
            c.summon_skill_id.as_deref(),
            &c.stat_sources,
            self.buffs,
            equipment,
            common_skills,
            c.awakening,
            &self.ctx.skill.id,
            &self.ctx.content.id,
            self.combo_count,
            self.combo_skill_type,
            None,
            self.temporary_adjustments.cloned(),
            c.rotation_skill_ids.as_deref(),
        )?;
        Ok((
            result.body.per_hit_primary,
            result.combined.total_primary,
            result.combined.defeat_seconds,
        ))
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
            rotation_skill_ids: None,
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

    /// 差し込む CT 技は「そのキャラが自分で撃つ」「CT を持つ」技だけ。
    #[test]
    fn 差し込むct技の検証は他キャラとct無しを拒否する() {
        let mut c = anais();
        // 他キャラのスキル
        c.rotation_skill_ids = Some(vec!["yefnen_slay".to_string()]);
        let error = super::validate_rotation_skills(&c).unwrap_err();
        assert!(error.message.contains("のスキルではありません"), "{}", error.message);
        // CT を持たないスキル
        c.rotation_skill_ids = Some(vec!["anais_angry_pixie".to_string()]);
        let error = super::validate_rotation_skills(&c).unwrap_err();
        assert!(error.message.contains("クールタイムの無い"), "{}", error.message);
        // 召喚獣が撃つスキル
        c.rotation_skill_ids = Some(vec!["anais_mica_even_bear".to_string()]);
        assert!(super::validate_rotation_skills(&c).is_err());
        // 未設定・差し込まないは通る
        c.rotation_skill_ids = None;
        assert!(super::validate_rotation_skills(&c).is_ok());
        c.rotation_skill_ids = Some(Vec::new());
        assert!(super::validate_rotation_skills(&c).is_ok());
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
            rotation_skill_ids: None,
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
            None,
        )
        .unwrap();
        assert!(without_summon.summon.is_none());
        assert_eq!(without_summon.combined.expected_dps, without_summon.body.expected_dps);
        assert_eq!(without_summon.combined.defeat_seconds, without_summon.body.defeat_seconds);
    }

    /// 技の性能を解決する順は **コンボスキルタイプ → 速剣 / 最大までチャージ** で固定する。
    /// 逆にすると、コンボタイプの表が速剣の倍率・段数を上書きしてしまう(いまは両方を持つ技が
    /// 無いので実害は出ないが、足したときに静かに壊れる)。この順で計算タブ・プレビュー・
    /// ホーム評価がすべて `build_damage_input` を通る。
    #[test]
    fn 技の性能はコンボタイプのあとに速剣とチャージで解決する() {
        let content = gamedata::content_areas()
            .into_iter()
            .flat_map(|area| area.contents)
            .find(|content| content.id == "ringo")
            .unwrap();
        let base_stats = BaseStats { stab: 100, hack: 100, int: 100, def: 1, mr: 1, dex: 1, agi: 1 };
        let build = |skill_id: &str, character: &str, skills: &[&str], masteries: &[&str], combo_type| {
            let mut stat_sources = StatSources::default();
            stat_sources.character_skills.skill_ids = skills.iter().map(|s| (*s).to_string()).collect();
            stat_sources.masteries.picked = masteries.iter().map(|s| (*s).to_string()).collect();
            let (_, target) = build_damage_input(
                &base_stats,
                character,
                None,
                &stat_sources,
                &BuffSelection::default(),
                Equipment::default(),
                CommonSkills::default(),
                domain::Awakening::default(),
                gamedata::find_skill(skill_id).unwrap(),
                gamedata::find_enemy("ringo_boss").unwrap(),
                &content,
                0,
                combo_type,
                None,
            )
            .unwrap();
            target.skill
        };

        // 速剣: ソードシェイプ系の倍率 ×0.9・段数 +1(wiki スキル性能一覧の「(速剣適用時)」行)
        let plain = build("yefnen_slay", "yefnen", &[], &[], None);
        assert!((plain.multiplier - 7.0).abs() < 1e-9);
        assert_eq!(plain.hit_count, 12);
        let swift = build("yefnen_slay", "yefnen", &["yefnen_swift_sword"], &[], None);
        assert!((swift.multiplier - 6.3).abs() < 1e-9);
        assert_eq!(swift.hit_count, 13);
        // チャージ: 段数が伸び、チャージ時間が中ディレイの外に乗る(【アックス特化】で半減)
        let charged = build("yefnen_slay_axe", "yefnen", &["yefnen_full_charge"], &[], None);
        assert_eq!(charged.hit_count, 17);
        assert!((charged.charge_seconds - 1.0).abs() < 1e-9);
        let halved = build(
            "yefnen_slay_axe",
            "yefnen",
            &["yefnen_full_charge"],
            &["yefnen_m1_2"],
            None,
        );
        assert!((halved.charge_seconds - 0.5).abs() < 1e-9);
        // コンボスキルタイプ(速剣・チャージを持たない技)は従来どおり解決される
        let swift_type = build(
            "maximin_continuous",
            "maximin",
            &[],
            &[],
            Some(domain::ComboSkillType::Instant),
        );
        assert!((swift_type.multiplier - 5.20).abs() < 1e-9);
        assert_eq!(swift_type.hit_count, 10);
    }

    /// <フラグ>(技とは別枠のダメージ)。スタック・爆発する技・マスタリー3 の効き方と、
    /// **技そのものの結果が一切変わらないこと**をまとめて確かめる。
    /// 回し(連打する技 + 差し込む CT 技)。計算タブとホームが同じ規則で動くこと、
    /// コンボ・中ディレイ減少が入っても式が崩れないことを確かめる。
    mod 回し {
        use super::*;

        /// イェフネン(斬り主軸・<フラグ> を積んだ状態)。回しの候補が同形態に 2 件以上ある
        fn yefnen(stacks: Option<u8>) -> NewCharacter {
            let mut stat_sources = StatSources::default();
            if let Some(stacks) = stacks {
                stat_sources.character_skills.skill_ids.push("yefnen_flag".to_string());
                stat_sources
                    .character_skills
                    .skill_levels
                    .insert("yefnen_flag".to_string(), stacks);
            }
            NewCharacter {
                name: "イェフネン".to_string(),
                game_character_id: "yefnen".to_string(),
                base_stats: BaseStats { stab: 1, hack: 300, int: 1, def: 1, mr: 1, dex: 1, agi: 1 },
                awakening: Awakening::default(),
                stat_sources,
                equipment: Equipment::default(),
                common_skills: CommonSkills::default(),
                main_skill_id: Some("yefnen_slay".to_string()),
                summon_skill_id: None,
                rotation_skill_ids: None,
                goal_content_id: None,
                default_buff_set_id: None,
            }
        }

        fn maximin(masteries: &[&str]) -> NewCharacter {
            let mut stat_sources = StatSources::default();
            stat_sources.character_skills.skill_ids =
                masteries.iter().map(|s| (*s).to_string()).collect();
            NewCharacter {
                name: "マクシミン".to_string(),
                game_character_id: "maximin".to_string(),
                base_stats: BaseStats {
                    stab: 300, hack: 300, int: 1, def: 1, mr: 1, dex: 1, agi: 1,
                },
                awakening: Awakening::default(),
                stat_sources,
                equipment: Equipment::default(),
                common_skills: CommonSkills::default(),
                main_skill_id: Some("maximin_storm_eye".to_string()),
                summon_skill_id: None,
                rotation_skill_ids: None,
                goal_content_id: None,
                default_buff_set_id: None,
            }
        }

        fn calc(
            character: &NewCharacter,
            skill_id: &str,
            combo_count: u32,
            normal_attack_id: Option<&str>,
        ) -> super::super::CharacterDamageResult {
            super::super::damage_for_character(
                &character.base_stats,
                &character.game_character_id,
                character.main_skill_id.as_deref(),
                None,
                &character.stat_sources,
                &BuffSelection::default(),
                character.equipment.clone(),
                character.common_skills,
                character.awakening,
                skill_id,
                "tutatur",
                combo_count,
                None,
                normal_attack_id,
                None,
                character.rotation_skill_ids.as_deref(),
            )
            .unwrap()
        }

        /// ホーム(全コンテンツ評価)と計算タブは同じ回しを通るので、同じキャラ・同じ
        /// コンテンツ・同じスキルなら期待 DPS が一致する。2 つの経路で別の数字が出ると
        /// 「ホームでは行けるのに計算タブでは届かない」が起きる。
        #[test]
        fn ホームと計算タブの期待dpsが一致する() {
            for masteries in [&[][..], &["maximin_clumsy_pair"][..]] {
                let character = maximin(masteries);
                let evals = super::super::evaluate_contents(
                    character.clone(),
                    BuffSelection::default(),
                    None,
                )
                .unwrap();
                let eval = evals.iter().find(|e| e.content_id == "tutatur").unwrap();
                let best = eval.damage.as_ref().unwrap();
                let calc = calc(&character, &best.skill_id, 0, None);
                // ホームは討伐時間に合算を反映するので、そこから期待 DPS を戻して比べる
                let hp = f64::from(i32::try_from(calc.body.enemy_hp.unwrap()).unwrap());
                let home_dps = hp / best.defeat_seconds.unwrap();
                let calc_dps = calc.combined.expected_dps.unwrap();
                assert!(
                    (home_dps - calc_dps).abs() < 1e-6,
                    "home={home_dps} calc={calc_dps} masteries={masteries:?}"
                );
            }
        }

        /// コンボ(通常攻撃を挟む)と中ディレイ減少が入っても、回しの DPS は
        /// 「差し込む技 ÷ 間隔 + 連打技 × 空いた時間」のまま。1 回の所要時間は
        /// コンボの 1 サイクル(`cycle_seconds`)で測る。
        #[test]
        fn コンボと中ディレイ減少つきの回し() {
            let mut character = maximin(&["maximin_clumsy_pair"]);
            // ウィンドストーム(CT 60s)は差し込むと DPS が下がるので既定では ON にならない。
            // 回しの組み立てそのものを見たいので、手で ON にした状態にする
            character.rotation_skill_ids = Some(vec!["maximin_wind_storm".to_string()]);
            let combo = calc(&character, "maximin_storm_eye", 3, Some("maximin_sword"));
            let rotation = combo.rotation.as_ref().unwrap();
            // 主軸(CT なし)を連打し、CT 技を差し込む
            let filler = rotation.filler.as_ref().unwrap();
            assert!(filler.is_main && filler.result.is_none());
            let insert = &rotation.inserts[0];
            assert!(!insert.is_main);
            assert!(insert.interval_seconds >= insert.cooldown_seconds - 1e-9);
            // 所要時間はコンボの 1 サイクル(通常攻撃 + スキル)
            let cycle = combo.body.combo.as_ref().unwrap();
            assert!((filler.seconds - cycle.seconds).abs() < 1e-12);
            // 手計算: 差し込み ÷ 間隔 + 連打の取り分 ÷ 1 サイクル
            let insert_result = insert.result.as_ref().unwrap();
            let insert_damage = insert_result
                .cycle_total()
                .expected(insert_result.critical_chance);
            let filler_damage = combo.body.cycle_total().expected(combo.body.critical_chance);
            let hand = insert_damage / insert.interval_seconds
                + rotation.filler_share / filler.seconds * filler_damage;
            assert!(
                (rotation.expected_dps - hand).abs() < 1e-6,
                "{} vs {hand}",
                rotation.expected_dps
            );
            assert_eq!(combo.combined.expected_dps, Some(rotation.expected_dps));
            // コンボなしとは別の値になる(サイクルが変わるので回しも変わる)
            let plain = calc(&character, "maximin_storm_eye", 0, None);
            assert!(
                (plain.combined.expected_dps.unwrap() - combo.combined.expected_dps.unwrap()).abs()
                    > 1e-6
            );
        }

        /// 検証用のキャラ(主軸だけ差し替える)。
        fn character_of(game_character_id: &str, main_skill_id: &str, stats: BaseStats) -> NewCharacter {
            NewCharacter {
                name: game_character_id.to_string(),
                game_character_id: game_character_id.to_string(),
                base_stats: stats,
                awakening: Awakening::default(),
                stat_sources: StatSources::default(),
                equipment: Equipment::default(),
                common_skills: CommonSkills::default(),
                main_skill_id: Some(main_skill_id.to_string()),
                summon_skill_id: None,
                rotation_skill_ids: None,
                goal_content_id: None,
                default_buff_set_id: None,
            }
        }
        const INT_300: BaseStats =
            BaseStats { stab: 1, hack: 1, int: 300, def: 1, mr: 1, dex: 1, agi: 1 };
        const HACK_300: BaseStats =
            BaseStats { stab: 1, hack: 300, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };

        const STAB_300: BaseStats =
            BaseStats { stab: 300, hack: 1, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };

        /// 中ディレイ減少 45%(フルスロットル)のルシアン。極・連撃(持続 10s = CT 10s)は
        /// 減少で 5.5s になり、**実効 CT を持つ差し込み技**に変わる境界。
        /// キャラタブの候補・計算タブの回し・ホームの評価が同じ役割・同じ期待 DPS を出す。
        #[test]
        fn 中ディレイ減少で極連撃は差し込む技に変わる() {
            use domain::{UltimateSkill, UltimateSkills};

            let choices = |character: NewCharacter| {
                super::super::list_rotation_choices(
                    character,
                    BuffSelection::default(),
                    "tutatur".to_string(),
                    None,
                    0,
                    None,
                    None,
                    None,
                )
                .unwrap()
            };
            // 減少なし: 持続 10s ≥ CT 10s なので「連打できる技」= 差し込み候補に出ない
            let plain = choices(character_of("lucian", "lucian_vortex", STAB_300));
            assert!(
                !plain.candidates.iter().any(|c| c.skill_id == "lucian_streak"),
                "{:?}",
                plain.candidates.iter().map(|c| &c.skill_id).collect::<Vec<_>>()
            );

            let mut character = character_of("lucian", "lucian_vortex", STAB_300);
            // ハイパーリミット Lv6 はオーグメント Lv5 で解放される(検証に引っかからない値)
            character.common_skills.augment_level = 5;
            character.common_skills.ultimate = UltimateSkills {
                slots: [Some(UltimateSkill::FullThrottle), None],
                super_limit: true,
                hyper_limit_level: 6,
            };
            let reduced = choices(character.clone());
            let streak = reduced
                .candidates
                .iter()
                .find(|c| c.skill_id == "lucian_streak")
                .expect("5.5s < CT 10s なので差し込める");

            // 計算タブ(build_rotation): 既定 ON の技がそのまま差し込まれ、期待 DPS も一致
            let preview = super::super::preview_damage(
                character.clone(),
                BuffSelection::default(),
                "lucian_vortex".to_string(),
                "tutatur".to_string(),
                0,
                None,
                None,
                None,
            )
            .unwrap();
            let rotation = preview.rotation.as_ref().unwrap();
            let default_on: Vec<&str> = reduced
                .candidates
                .iter()
                .filter(|c| c.default_on)
                .map(|c| c.skill_id.as_str())
                .collect();
            let inserted: Vec<&str> =
                rotation.inserts.iter().map(|i| i.skill_id.as_str()).collect();
            assert_eq!(inserted, default_on);
            // 差し込めるようになった(損得が出る)。既定 ON になるのは `Skill::power` が
            // 最大の技なので、極・連撃そのものが既定とは限らない(ADR-019 決定 5)
            assert!(streak.expected_dps_gain.unwrap() > 0.0);
            assert_eq!(default_on.len(), 1);
            assert!(
                (rotation.expected_dps - reduced.expected_dps.unwrap()).abs() < 1e-6,
                "{} vs {:?}",
                rotation.expected_dps,
                reduced.expected_dps
            );
            // 連打は主軸そのもの(主軸に実効 CT が無い)
            assert!(rotation.filler.as_ref().unwrap().is_main);
            assert_eq!(reduced.filler_skill_name, None);

            // ホーム(evaluate_contents): 同じキャラ・同じ対象で討伐時間が計算タブと一致する
            let evals =
                super::super::evaluate_contents(character.clone(), BuffSelection::default(), None)
                    .unwrap();
            let eval = evals.iter().find(|e| e.content_id == "tutatur").unwrap();
            let best = eval.damage.as_ref().unwrap();
            let calc = super::super::damage_for_character(
                &character.base_stats,
                &character.game_character_id,
                character.main_skill_id.as_deref(),
                None,
                &character.stat_sources,
                &BuffSelection::default(),
                character.equipment.clone(),
                character.common_skills,
                character.awakening,
                &best.skill_id,
                "tutatur",
                0,
                None,
                None,
                None,
                character.rotation_skill_ids.as_deref(),
            )
            .unwrap();
            assert!(
                (best.defeat_seconds.unwrap() - calc.combined.defeat_seconds.unwrap()).abs() < 1e-6
            );
        }

        /// チャネリング技(区分 `続`)は押している間 tick を繰り返す。wiki の段数は
        /// **1 tick ぶん**なので、1 回の使用ぶん(`cycle_total`)と DPS は tick 数を掛ける。
        /// 1 発の主役数字と合計ダメージはゲーム内の表示に合わせて 1 tick ぶんのまま。
        #[test]
        fn チャネリング技は1回の使用でtick数ぶん入る() {
            let skill = gamedata::find_skill("tichiel_sparkling_kite").unwrap();
            let channeling = skill.channeling.unwrap();
            assert_eq!(channeling.ticks, 10);
            assert!((channeling.tick_seconds - 1.0).abs() < 1e-12);
            assert_eq!(skill.hit_count, 10, "段数は 1 tick ぶん");

            let character = character_of("tichiel", "tichiel_sparkling_kite", INT_300);
            let result = calc(&character, "tichiel_sparkling_kite", 0, None);
            let body = &result.body;
            // 1 回の使用 = 10 tick。主役の 1 発・合計は 1 tick ぶんのまま
            assert_eq!(body.cycle_total().max, body.total.max * 10);
            assert_eq!(result.combined.total_primary, body.total_primary);
            // 所要時間は持続 10 秒(このキャラは中ディレイ減少を持たないので素のまま。
            // 減少があれば持続もその率で縮む。公式 2025-10-29 の追加案内 4-1)
            assert!((body.cycle_seconds().unwrap() - 10.0).abs() < 1e-9);
            // DPS は 1 回ぶん ÷ 1 回の秒数 = 1 tick ぶんが毎秒入る
            assert!(
                (body.dps.unwrap().max - body.total.max as f64).abs() < 1e-6,
                "{:?}",
                body.dps
            );
            // カイトは CT 10s ≤ 所要時間 10s なので「連打できる技」。差し込み候補に出さない
            let choices = super::super::list_rotation_choices(
                character_of("tichiel", "tichiel_fire_arrow", INT_300),
                BuffSelection::default(),
                "tutatur".to_string(),
                None,
                0,
                None,
                None,
                None,
            )
            .unwrap();
            assert!(
                !choices
                    .candidates
                    .iter()
                    .any(|c| c.skill_id == "tichiel_sparkling_kite"),
                "{:?}",
                choices.candidates.iter().map(|c| &c.skill_id).collect::<Vec<_>>()
            );
            // ティチエルの CT 技はどれも差し込むと下がるので、既定で ON になる技は無い
            assert!(choices.candidates.iter().all(|c| !c.default_on));
        }

        /// ルシアン 極・連撃(486%x10 を 1s 毎に 10 秒)の DPS は、tick を数える前の
        /// 「10 段を 10 秒に 1 回」の約 10 倍になる。
        #[test]
        fn チャネリング技のdpsはtick前の約10倍() {
            let character = character_of("lucian", "lucian_streak", HACK_300);
            let result = calc(&character, "lucian_streak", 0, None);
            let body = &result.body;
            let before = body.total.max as f64 / 10.0; // 10 秒に 1 回だった頃
            let after = body.dps.unwrap().max;
            assert!((after / before - 10.0).abs() < 1e-9, "{after} / {before}");
        }

        /// キャラタブの「差し込む CT 技」の候補。母集団・既定 ON・損得の符号・連打技の名前を
        /// まとめて固定する(画面はこの値をそのまま出す)。
        #[test]
        fn 差し込むct技の候補は母集団と既定と符号を返す() {
            let choices = |character: NewCharacter| {
                super::super::list_rotation_choices(
                    character,
                    BuffSelection::default(),
                    "tutatur".to_string(),
                    None,
                    0,
                    None,
                    None,
                    None,
                )
                .unwrap()
            };
            // ティチエル(乱打を連打): 候補は同形態の CT 技だけ。どれも差し込むと
            // DPS が下がるので**既定で ON になる技は無い**(ADR-019、ユーザー決定 2026-09-21)
            let mut tichiel = NewCharacter {
                name: "ティチエル".to_string(),
                game_character_id: "tichiel".to_string(),
                base_stats: BaseStats { stab: 1, hack: 1, int: 300, def: 1, mr: 1, dex: 1, agi: 1 },
                awakening: Awakening::default(),
                stat_sources: StatSources::default(),
                equipment: Equipment::default(),
                common_skills: CommonSkills::default(),
                main_skill_id: Some("tichiel_beating".to_string()),
                summon_skill_id: None,
                rotation_skill_ids: None,
                goal_content_id: None,
                default_buff_set_id: None,
            };
            let result = choices(tichiel.clone());
            assert!(!result.candidates.is_empty());
            // 母集団はそのキャラの CT を持つ技だけ(主軸は含めない)
            for candidate in &result.candidates {
                let skill = gamedata::find_skill(&candidate.skill_id).unwrap();
                assert_eq!(skill.attacker, domain::Attacker::Player, "{}", skill.id);
                assert!(skill.cooldown_seconds.is_some(), "{}", skill.id);
                assert_ne!(candidate.skill_id, "tichiel_beating");
                assert!(candidate.cooldown_seconds > 0.0);
            }
            // 下がる技は既定で ON にしない
            assert!(result.candidates.iter().all(|c| !c.default_on));
            assert!(
                result
                    .candidates
                    .iter()
                    .all(|c| c.expected_dps_gain.is_some_and(|gain| gain < 0.0)),
                "{:?}",
                result
                    .candidates
                    .iter()
                    .map(|c| (&c.skill_id, c.expected_dps_gain))
                    .collect::<Vec<_>>()
            );
            // 主軸を連打するので、合間に連打する技の名前は出さない
            assert_eq!(result.filler_skill_name, None);

            // 既定が「差し込みなし」なら、割合の分母は主軸を連打し続ける DPS
            let preview = super::super::preview_damage(
                tichiel.clone(),
                BuffSelection::default(),
                "tichiel_beating".to_string(),
                "tutatur".to_string(),
                0,
                None,
                None,
                None,
            )
            .unwrap();
            assert!(preview.rotation.is_none());
            assert!(
                (result.expected_dps.unwrap() - preview.body.expected_dps.unwrap()).abs() < 1e-6,
                "{:?} vs {:?}",
                result.expected_dps,
                preview.body.expected_dps
            );

            // 主軸未選択なら候補は空(行ごと出さない)
            tichiel.main_skill_id = None;
            assert!(choices(tichiel.clone()).candidates.is_empty());

            // CT 技を 1 つも持たないキャラ(ボリス)も空
            let boris = NewCharacter {
                name: "ボリス".to_string(),
                game_character_id: "boris".to_string(),
                base_stats: BaseStats { stab: 300, hack: 300, int: 1, def: 1, mr: 1, dex: 1, agi: 1 },
                main_skill_id: Some("boris_blur_sword".to_string()),
                ..tichiel.clone()
            };
            let boris = choices(boris);
            assert!(boris.candidates.is_empty());
            assert_eq!(boris.filler_skill_name, None);
        }

        /// イェフネン(主軸がスレイ = CT 技)は連打技が自動で決まり、その名前が返る。
        /// <フラグ> を積んだ状態では差し込むほうが DPS が高い(符号は正)。
        #[test]
        fn 主軸がct技なら連打技の名前が返り差し込みは正() {
            let mut character = yefnen(Some(10));
            character.main_skill_id = Some("yefnen_continuous".to_string());
            let result = super::super::list_rotation_choices(
                character,
                BuffSelection::default(),
                "tutatur".to_string(),
                None,
                0,
                None,
                None,
                None,
            )
            .unwrap();
            // 連を連打するので、差し込み候補はスレイ(その <フラグ> を使う技)
            let slay = result
                .candidates
                .iter()
                .find(|c| c.skill_id == "yefnen_slay")
                .unwrap();
            assert!(slay.default_on);
            assert!(slay.expected_dps_gain.unwrap() > 0.0);
            // 主軸(連)を連打しているので連打技の名前は出さない
            assert_eq!(result.filler_skill_name, None);

            // 主軸をスレイ(CT 10s)にすると、連打技は自動で決まりその名前が返る
            let main_is_ct = super::super::list_rotation_choices(
                yefnen(Some(10)),
                BuffSelection::default(),
                "tutatur".to_string(),
                None,
                0,
                None,
                None,
                None,
            )
            .unwrap();
            assert!(main_is_ct.filler_skill_name.is_some());
        }

        /// 保存済みの id が候補外(形態違い・主軸自身・消えた技)でも、計算は落ちず
        /// ホームの評価と同じ値になる。domain が黙って落とす方針の担保。
        #[test]
        fn 候補外のidが混ざっても計算は落ちずホームと一致する() {
            let mut character = yefnen(Some(10));
            character.rotation_skill_ids = Some(vec![
                // 形態違い(アックス)・主軸自身・カタログに無い id
                "yefnen_crash_axe".to_string(),
                "yefnen_slay".to_string(),
                "no_such_skill".to_string(),
            ]);
            let evals =
                super::super::evaluate_contents(character.clone(), BuffSelection::default(), None)
                    .unwrap();
            let eval = evals.iter().find(|e| e.content_id == "tutatur").unwrap();
            let best = eval.damage.as_ref().unwrap();
            let calc = super::super::damage_for_character(
                &character.base_stats,
                &character.game_character_id,
                character.main_skill_id.as_deref(),
                None,
                &character.stat_sources,
                &BuffSelection::default(),
                character.equipment.clone(),
                character.common_skills,
                character.awakening,
                &best.skill_id,
                "tutatur",
                0,
                None,
                None,
                None,
                character.rotation_skill_ids.as_deref(),
            )
            .unwrap();
            assert!(
                (best.defeat_seconds.unwrap() - calc.combined.defeat_seconds.unwrap()).abs() < 1e-6
            );
        }

        /// 差し込みの損得(`expected_dps_gain`)。既定は「1 回の火力が最大の CT 技を常に
        /// 差し込む」なので、連打技より効率が悪い技だと DPS が下がる。画面はこの値で
        /// 「差し込むと下がる」を出すため、符号が逆にならないことを固定する。
        #[test]
        fn 差し込みの損得は符号で分かる() {
            // ティチエル: 乱打を連打 + ギガブレイズ(CT 60s)。下がる技は既定で ON に
            // ならないので、手で ON にしたときの符号を見る
            let down = ["tichiel_giga_blaze".to_string()];
            let tichiel = super::super::damage_for_character(
                &BaseStats { stab: 1, hack: 1, int: 300, def: 1, mr: 1, dex: 1, agi: 1 },
                "tichiel",
                Some("tichiel_beating"),
                None,
                &StatSources::default(),
                &BuffSelection::default(),
                Equipment::default(),
                CommonSkills::default(),
                Awakening::default(),
                "tichiel_beating",
                "tutatur",
                0,
                None,
                None,
                None,
                Some(&down),
            )
            .unwrap();
            let insert = &tichiel.rotation.as_ref().unwrap().inserts[0];
            assert!(!insert.is_main);
            assert_eq!(insert.skill_id, "tichiel_giga_blaze");
            let gain = insert.expected_dps_gain.unwrap();
            assert!(gain < 0.0, "{} {gain}", insert.skill_id);
            // 差し込まなければその分だけ DPS が高い(= 主軸を連打しているだけの値)
            assert!(
                (tichiel.combined.expected_dps.unwrap() - gain
                    - tichiel.body.expected_dps.unwrap())
                .abs()
                    < 1e-6
            );

            // イェフネン: 連 を連打 + スレイ(<フラグ> を爆発させる)は差し込むほうが高い
            let mut stat_sources = StatSources::default();
            stat_sources.character_skills.skill_ids.push("yefnen_flag".to_string());
            stat_sources.character_skills.skill_levels.insert("yefnen_flag".to_string(), 10);
            let yefnen = super::super::damage_for_character(
                &BaseStats { stab: 1, hack: 300, int: 1, def: 1, mr: 1, dex: 1, agi: 1 },
                "yefnen",
                Some("yefnen_continuous"),
                None,
                &stat_sources,
                &BuffSelection::default(),
                Equipment::default(),
                CommonSkills::default(),
                Awakening::default(),
                "yefnen_continuous",
                "tutatur",
                0,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let insert = &yefnen.rotation.as_ref().unwrap().inserts[0];
            assert_eq!(insert.skill_id, "yefnen_slay");
            assert!(insert.expected_dps_gain.unwrap() > 0.0);
        }

        /// 主軸候補の並び(`Skill::main_skill_order`)。CT のある技は「CT ごとに 1 回」しか
        /// 撃てないので、継続火力の分母は中ディレイと CT の長いほう。
        /// チャネリング技は 1 回の使用で tick 数ぶん撃つので、その火力も tick 込み。
        #[test]
        fn 主軸候補の並びは単体優先で継続火力順() {
            let top = |character: &str| {
                super::super::list_skills(character.to_string())
                    .into_iter()
                    .take(3)
                    .map(|skill| skill.id)
                    .collect::<Vec<_>>()
            };
            // ルシアンの上位はチャネリング技(10 秒撃ち続けて 10 tick ぶん入る)
            assert_eq!(
                top("lucian"),
                [
                    "lucian_whirlwind_sword",
                    "lucian_streak",
                    "lucian_warriors_dance"
                ]
            );
            // イェフネンは CT 10 秒の スレイ 系より、連打できる 連 系が先に来る
            assert_eq!(
                top("yefnen"),
                [
                    "yefnen_continuous_axe",
                    "yefnen_continuous",
                    "yefnen_continuous_pike"
                ]
            );
        }
    }

    mod フラグ {
        use super::*;

        fn yefnen_stats() -> BaseStats {
            BaseStats {
                stab: 1,
                hack: 300,
                int: 1,
                def: 1,
                mr: 1,
                dex: 1,
                agi: 1,
            }
        }

        fn calc(
            skill_id: &str,
            stacks: Option<u8>,
            masteries: &[&str],
        ) -> super::super::CharacterDamageResult {
            let mut stat_sources = StatSources::default();
            if let Some(stacks) = stacks {
                stat_sources
                    .character_skills
                    .skill_ids
                    .push("yefnen_flag".to_string());
                stat_sources
                    .character_skills
                    .skill_levels
                    .insert("yefnen_flag".to_string(), stacks);
            }
            stat_sources.masteries.picked = masteries.iter().map(|s| (*s).to_string()).collect();
            super::super::damage_for_character(
                &yefnen_stats(),
                "yefnen",
                Some(skill_id),
                None,
                &stat_sources,
                &BuffSelection::default(),
                Equipment::default(),
                CommonSkills::default(),
                domain::Awakening::default(),
                skill_id,
                "tutatur",
                0,
                None,
                None,
                None,
                None,
            )
            .unwrap()
        }

        /// 主軸(スレイ)の 1 回ぶんを取り出す。回しの中で主軸は差し込む技
        fn main_insert(result: &super::super::CharacterDamageResult) -> &super::super::RotationInsert {
            result
                .rotation
                .as_ref()
                .unwrap()
                .inserts
                .iter()
                .find(|insert| insert.is_main)
                .unwrap()
        }

        /// スタック 0(= 画面で OFF。`skill_ids` に入っていない)なら <フラグ> は無い。
        /// 主軸(スレイ)は CT 10 秒の技なので回しは組まれ、1 発の主役数字は技そのもの、
        /// DPS は「連打技を挟んで CT ごとに 1 回」の値になる。
        #[test]
        fn スタック0でも主役数字は技そのもの() {
            let off = calc("yefnen_slay", None, &[]);
            assert!(off.flag.is_none());
            assert_eq!(off.combined.total_primary, off.body.total_primary);
            // 討伐時間は合算 DPS から出る(技単独の DPS からではない)
            assert_eq!(
                off.combined.defeat_seconds,
                domain::defeat_seconds(off.body.enemy_hp, off.combined.expected_dps)
            );
            assert_ne!(off.combined.defeat_seconds, off.body.defeat_seconds);
            let rotation = off.rotation.as_ref().unwrap();
            assert_eq!(off.combined.expected_dps, Some(rotation.expected_dps));
            // CT 10 秒の技を連打できる前提にしない
            assert!(rotation.expected_dps < off.body.expected_dps.unwrap());
        }

        /// <フラグ> を積んだほうが DPS が高い(OFF のほうが高い、という逆転が起きない)。
        #[test]
        fn フラグをonにするとdpsが上がる() {
            let off = calc("yefnen_slay", None, &[]);
            let on = calc("yefnen_slay", Some(10), &[]);
            assert!(
                on.combined.expected_dps.unwrap() > off.combined.expected_dps.unwrap(),
                "on={:?} off={:?}",
                on.combined.expected_dps,
                off.combined.expected_dps
            );
        }

        #[test]
        fn 爆発は主役数字に入り持続はdpsにだけ乗る() {
            let off = calc("yefnen_slay", None, &[]);
            let on = calc("yefnen_slay", Some(10), &[]);
            let flag = on.flag.as_ref().unwrap();
            let burst = flag.burst.as_ref().unwrap();
            // 技そのものは 1 ミリも変わらない
            assert_eq!(on.body.total_primary, off.body.total_primary);
            assert_eq!(on.body.expected_dps, off.body.expected_dps);
            // 1 発の主役数字 = 技 + 爆発
            assert_eq!(
                on.combined.total_primary,
                on.body.total_primary + burst.total_primary
            );
            // 持続は主役数字に入らない(DPS にだけ乗る)
            assert_eq!(
                on.combined.total_primary - on.body.total_primary,
                burst.total_primary
            );

            // 合算 DPS = (連打技 × 回数 + 主軸 + 爆発) ÷ 主軸の間隔 + 持続 ÷ 周期(1 秒)
            let insert = main_insert(&on);
            let filler = on.rotation.as_ref().unwrap().filler.as_ref().unwrap();
            let filler_result = filler.result.as_ref().unwrap();
            let cycle_damage = filler_result.total.expected(filler_result.critical_chance)
                * f64::from(insert.filler_uses)
                + on.body.total.expected(on.body.critical_chance)
                + burst.total.expected(burst.critical_chance);
            let duration_dps =
                flag.duration.total.expected(flag.duration.critical_chance) / flag.tick_seconds;
            assert!((flag.duration.expected_dps.unwrap() - duration_dps).abs() < 1e-9);
            assert!(
                (on.combined.expected_dps.unwrap()
                    - (cycle_damage / insert.interval_seconds + duration_dps))
                    .abs()
                    < 1e-6
            );
        }

        /// 回し基準にしたぶん、爆発の DPS は旧式(爆発 ÷ 主軸 1 回)より小さくなる。
        /// 間隔には積み直しの時間が入っているので当然だが、「過大だった」ことの回帰ガード。
        #[test]
        fn 回しのdpsは旧式より小さい() {
            let on = calc("yefnen_slay", Some(10), &[]);
            let flag = on.flag.as_ref().unwrap();
            let insert = main_insert(&on);
            let burst = flag.burst.as_ref().unwrap();
            let duration_dps = flag.duration.expected_dps.unwrap();
            // 旧式: 技の DPS + 爆発 ÷ 主軸 1 回の所要時間 + 持続
            let old = on.body.expected_dps.unwrap()
                + burst.total.expected(burst.critical_chance) / on.body.cycle_seconds().unwrap()
                + duration_dps;
            assert!(on.combined.expected_dps.unwrap() < old);
            // 主軸の間隔は CT より短くならない
            assert!(insert.interval_seconds >= insert.cooldown_seconds - 1e-9);
        }

        /// 積み直しの回数: 通常形態は 1 回 +2 なので S=10 で 5 回。
        /// ウルミは 1 回 +5 で爆発しても半分残るため積み直しは 5 → 1 回で足りるが、
        /// CT 10 秒を満たすまで回数が増える。
        #[test]
        fn 積み直しの回数はスタックとctで決まる() {
            let sword = calc("yefnen_slay", Some(10), &[]);
            let sword_insert = main_insert(&sword);
            assert_eq!(
                sword.rotation.as_ref().unwrap().filler.as_ref().unwrap().skill_id,
                "yefnen_continuous"
            );
            assert!(sword_insert.filler_uses >= 5);
            assert!(sword_insert.interval_seconds >= 10.0 - 1e-9);

            let urumi = calc("yefnen_slay_urumi", Some(10), &[]);
            let urumi_insert = main_insert(&urumi);
            assert_eq!(
                urumi.rotation.as_ref().unwrap().filler.as_ref().unwrap().skill_id,
                "yefnen_continuous_urumi"
            );
            // 爆発しても半分(5)残るので積み直しは 1 回ぶんだが、CT 10 秒に届かず回数が増える
            assert!(urumi_insert.cooldown_bound);
            assert!(urumi_insert.filler_uses > 1);
            assert!(urumi_insert.interval_seconds >= 10.0 - 1e-9);

            // 範囲技(クラッシュ)は同じ形態の 爆 で積み直す
            let crash = calc("yefnen_crash_axe", Some(10), &[]);
            assert_eq!(
                crash.rotation.as_ref().unwrap().filler.as_ref().unwrap().skill_id,
                "yefnen_explosion_axe"
            );

            // スタックを下げると積み直しの回数は増えない(CT 由来の下限は残る)
            let few = calc("yefnen_slay", Some(2), &[]);
            assert!(main_insert(&few).filler_uses <= sword_insert.filler_uses);
        }

        /// 主軸が積む技(連。CT なし)なら、主軸を連打しながら スレイ を差し込む回しになる。
        /// 1 発の主役数字は技そのもの(爆発させるのは差し込む技のほう)。
        #[test]
        fn 主軸が連なら連打しながらスレイを差し込む() {
            let on = calc("yefnen_continuous", Some(10), &[]);
            let flag = on.flag.as_ref().unwrap();
            assert!(flag.burst.is_none());
            assert_eq!(on.combined.total_primary, on.body.total_primary);
            let rotation = on.rotation.as_ref().unwrap();
            // 連打しているのは主軸そのもの
            let filler = rotation.filler.as_ref().unwrap();
            assert!(filler.is_main && filler.result.is_none());
            // 差し込むのは同形態でいちばん火力の高い CT 技(スレイ)で、爆発を伴う
            let insert = &rotation.inserts[0];
            assert_eq!(insert.skill_id, "yefnen_slay");
            assert!(!insert.is_main);
            assert!(insert.burst.is_some());
            assert!(insert.interval_seconds >= insert.cooldown_seconds - 1e-9);
            // 連打だけ(差し込まない)より DPS は高い
            let duration_dps = flag.duration.expected_dps.unwrap();
            assert!(
                on.combined.expected_dps.unwrap() > on.body.expected_dps.unwrap() + duration_dps
            );
        }

        /// 差し込む CT 技は「連打扱い」と「差し込まない」の間に入る。
        /// CT 技を連打できる前提(旧式)にも、撃たない前提にもしない。
        #[test]
        fn 差し込む技のdpsは連打と不使用の間() {
            let none = super::super::damage_for_character(
                &yefnen_stats(),
                "yefnen",
                Some("yefnen_continuous"),
                None,
                &StatSources::default(),
                &BuffSelection::default(),
                Equipment::default(),
                CommonSkills::default(),
                domain::Awakening::default(),
                "yefnen_continuous",
                "tutatur",
                0,
                None,
                None,
                None,
                // 明示で「差し込まない」
                Some(&[]),
            )
            .unwrap();
            assert!(none.rotation.is_none());
            let inserted = calc("yefnen_continuous", None, &[]);
            let spam = calc("yefnen_slay", None, &[]);
            let insert_dps = inserted.combined.expected_dps.unwrap();
            assert!(insert_dps > none.combined.expected_dps.unwrap());
            // 「スレイを連打できる」前提の値(body 単独)は超えない
            assert!(insert_dps < spam.body.expected_dps.unwrap());
        }

        /// 明示指定した 2 件はどちらも回しに入る(既定の 1 件に畳まれない)。
        #[test]
        fn 明示指定した差し込み2件は2段になる() {
            let ids = ["yefnen_slay".to_string(), "yefnen_crash".to_string()];
            let two = super::super::damage_for_character(
                &yefnen_stats(),
                "yefnen",
                Some("yefnen_continuous"),
                None,
                &StatSources::default(),
                &BuffSelection::default(),
                Equipment::default(),
                CommonSkills::default(),
                domain::Awakening::default(),
                "yefnen_continuous",
                "tutatur",
                0,
                None,
                None,
                None,
                Some(&ids),
            )
            .unwrap();
            let rotation = two.rotation.as_ref().unwrap();
            assert_eq!(rotation.inserts.len(), 2);
            let mut names: Vec<&str> =
                rotation.inserts.iter().map(|i| i.skill_id.as_str()).collect();
            names.sort_unstable();
            assert_eq!(names, ["yefnen_crash", "yefnen_slay"]);
            // 既定(未設定)は火力最大の 1 件だけ
            assert_eq!(
                calc("yefnen_continuous", None, &[])
                    .rotation
                    .as_ref()
                    .unwrap()
                    .inserts
                    .len(),
                1
            );
        }

        /// キャラに保存した「差し込む CT 技」は、計算タブ(`preview_damage`)にも
        /// ホームの評価(`evaluate_contents`)にも同じように効く。
        #[test]
        fn 保存した差し込む技は計算タブとホームの両方に効く() {
            let mut character = yefnen(None);
            character.main_skill_id = Some("yefnen_continuous".to_string());
            let preview = |c: &NewCharacter| {
                super::super::preview_damage(
                    c.clone(),
                    BuffSelection::default(),
                    "yefnen_continuous".to_string(),
                    "tutatur".to_string(),
                    0,
                    None,
                    None,
                    None,
                )
                .unwrap()
            };
            // 未設定 = 既定で 1 件差し込む
            let default_preview = preview(&character);
            assert_eq!(
                default_preview.rotation.as_ref().unwrap().inserts.len(),
                1
            );
            // 明示的に差し込まない → 回しを組まない = 技そのものの DPS
            character.rotation_skill_ids = Some(Vec::new());
            let none_preview = preview(&character);
            assert!(none_preview.rotation.is_none());
            assert!(
                none_preview.combined.expected_dps.unwrap()
                    < default_preview.combined.expected_dps.unwrap()
            );
        }

        /// 計算タブの「回し」の段は、キャラを保存せずに差し込みを試せる ——
        /// `preview_damage` に渡す一時の `rotation_skill_ids` がそのまま効き、
        /// 渡したキャラの保存値(`rotation_skill_ids`)は変わらない。
        #[test]
        fn 一時の差し込みはプレビューにだけ効く() {
            let mut character = yefnen(None);
            character.main_skill_id = Some("yefnen_continuous".to_string());
            character.rotation_skill_ids = None;
            let preview = |c: &NewCharacter| {
                super::super::preview_damage(
                    c.clone(),
                    BuffSelection::default(),
                    "yefnen_continuous".to_string(),
                    "tutatur".to_string(),
                    0,
                    None,
                    None,
                    None,
                )
                .unwrap()
            };
            // 画面が一時の選択を載せた写しを渡す(保存はしない)
            let mut temporary = character.clone();
            temporary.rotation_skill_ids = Some(Vec::new());
            assert!(preview(&temporary).rotation.is_none());
            // 元のキャラは触っていないので、既定どおりの回しのまま
            assert_eq!(character.rotation_skill_ids, None);
            assert!(preview(&character).rotation.is_some());
        }

        /// 技ごとの寄与(回しの段が出す数)を足すと回し全体の期待 DPS になり、
        /// 時間の占有と連打の取り分を足すと 1 になる。
        #[test]
        fn 回しの技ごとの取り分を足すと全体になる() {
            let mut character = yefnen(None);
            character.main_skill_id = Some("yefnen_continuous".to_string());
            let result = super::super::preview_damage(
                character,
                BuffSelection::default(),
                "yefnen_continuous".to_string(),
                "tutatur".to_string(),
                0,
                None,
                None,
                None,
            )
            .unwrap();
            let rotation = result.rotation.as_ref().unwrap();
            let filler = rotation.filler.as_ref().unwrap();
            assert!(!rotation.crowded);
            assert!(!rotation.inserts.is_empty());
            let sum: f64 = filler.expected_dps
                + rotation.inserts.iter().map(|i| i.expected_dps).sum::<f64>();
            assert!(
                (sum - rotation.expected_dps).abs() < 1e-6,
                "{sum} vs {}",
                rotation.expected_dps
            );
            let dps_share: f64 =
                filler.dps_share + rotation.inserts.iter().map(|i| i.dps_share).sum::<f64>();
            assert!((dps_share - 1.0).abs() < 1e-9, "{dps_share}");
            // 1 分あたりの回数は「間隔ごとに 1 回」「連打は空いた時間ぶん」そのもの
            let insert = &rotation.inserts[0];
            assert!((insert.uses_per_minute - 60.0 / insert.interval_seconds).abs() < 1e-9);
            assert!(
                (filler.uses_per_minute - rotation.filler_share * 60.0 / filler.seconds).abs() < 1e-9
            );
        }

        /// ホームの評価にも同じ選択が届く。差し込まない設定でも、ホームの討伐時間は
        /// 計算タブの合算と一致したまま(2 つの経路で別の数字を出さない)。
        #[test]
        fn 差し込まない設定でもホームと計算タブは一致する() {
            let mut character = yefnen(Some(10));
            character.rotation_skill_ids = Some(Vec::new());
            let evals =
                super::super::evaluate_contents(character.clone(), BuffSelection::default(), None)
                    .unwrap();
            let eval = evals.iter().find(|e| e.content_id == "tutatur").unwrap();
            let best = eval.damage.as_ref().unwrap();
            let calc = super::super::damage_for_character(
                &character.base_stats,
                &character.game_character_id,
                character.main_skill_id.as_deref(),
                None,
                &character.stat_sources,
                &BuffSelection::default(),
                character.equipment.clone(),
                character.common_skills,
                character.awakening,
                &best.skill_id,
                "tutatur",
                0,
                None,
                None,
                None,
                character.rotation_skill_ids.as_deref(),
            )
            .unwrap();
            assert!(
                (best.defeat_seconds.unwrap() - calc.combined.defeat_seconds.unwrap()).abs() < 1e-6
            );
            // 既定(差し込む)より遅い
            let (default_home, _) = home_and_calc(Some(10));
            assert!(best.defeat_seconds.unwrap() > default_home.unwrap());
        }

        #[test]
        fn スタック10の倍率と段数とcri倍率が出典どおり() {
            let on = calc("yefnen_slay", Some(10), &[]);
            let flag = on.flag.as_ref().unwrap();
            let burst = flag.burst.as_ref().unwrap();
            assert_eq!(flag.stacks, 10);
            assert!((flag.multiplier - 4.00).abs() < 1e-9);
            // 400% × 5 段 / Cri倍率 2.5(爆発)、400% × 1 段 / Cri倍率 2.0(持続)
            assert_eq!(burst.hit_count, 5);
            assert!((burst.effective_skill_multiplier - 4.00).abs() < 1e-9);
            assert_eq!(flag.duration.hit_count, 1);
            assert!((flag.duration.effective_skill_multiplier - 4.00).abs() < 1e-9);
            assert!((flag.tick_seconds - 1.0).abs() < 1e-9);
            assert!((flag.lasts_seconds - 120.0).abs() < 1e-9);
        }

        #[test]
        fn 連と爆では爆発せずスレイとクラッシュでは全形態で爆発する() {
            for id in ["yefnen_continuous", "yefnen_explosion_axe"] {
                let r = calc(id, Some(10), &[]);
                assert!(r.flag.as_ref().unwrap().burst.is_none(), "{id}");
                // 持続は積む技でも乗る
                assert!(r.flag.as_ref().unwrap().duration.expected_dps.is_some(), "{id}");
                assert_eq!(r.combined.total_primary, r.body.total_primary, "{id}");
            }
            for id in [
                "yefnen_slay",
                "yefnen_slay_pike",
                "yefnen_crash_axe",
                "yefnen_crash_urumi",
                "yefnen_slay_chisel",
            ] {
                let r = calc(id, Some(10), &[]);
                assert!(r.flag.as_ref().unwrap().burst.is_some(), "{id}");
                assert!(r.combined.total_primary > r.body.total_primary, "{id}");
            }
        }

        /// 欠片(マスタリー3)の ±% は <フラグ> の E1 にだけ入り、技には一切入らない。
        /// 値は「どの供給源がどのカテゴリにいくら積んだか」(トレース)で見る —
        /// 最終ダメージは敵の防御で下限に張り付くので比較に使えない。
        #[test]
        fn 欠片のマスタリーはフラグのe1にだけ入る() {
            let e1 = |result: &super::super::DamageResult, source: &str| -> Option<f64> {
                result
                    .trace
                    .category_contributions
                    .iter()
                    .find(|c| {
                        c.source == source && c.category == DamageCategory::SkillMultiplierRate
                    })
                    .map(|c| c.value)
            };
            let sharp = calc("yefnen_slay", Some(10), &["yefnen_m3_2"]);
            let flag = sharp.flag.as_ref().unwrap();
            assert_eq!(e1(&flag.duration, "鋭い欠片"), Some(0.20));
            assert_eq!(e1(flag.burst.as_ref().unwrap(), "鋭い欠片"), Some(-0.20));
            // 技には入らない
            assert_eq!(e1(&sharp.body, "鋭い欠片"), None);

            let sticky = calc("yefnen_slay", Some(10), &["yefnen_m3_3"]);
            let flag = sticky.flag.as_ref().unwrap();
            assert_eq!(e1(&flag.duration, "べたつく欠片"), Some(-0.10));
            assert_eq!(e1(flag.burst.as_ref().unwrap(), "べたつく欠片"), None);
            assert_eq!(e1(&sticky.body, "べたつく欠片"), None);

            // 技の結果はどの欠片でも不変(安定した欠片 = 素の <フラグ> と同じ)
            let plain = calc("yefnen_slay", Some(10), &["yefnen_m3_1"]);
            assert_eq!(sharp.body.total_primary, plain.body.total_primary);
            assert_eq!(sticky.body.total_primary, plain.body.total_primary);
            assert_eq!(sharp.body.expected_dps, plain.body.expected_dps);
        }

        /// 技の DPS が出せない(中ディレイ未収録)なら、<フラグ> の持続だけで
        /// 合算 DPS・討伐時間を立てない。「技の火力は不明なのに討伐時間が出ている」にしない。
        #[test]
        fn 技のdpsが不明なら合算dpsも不明() {
            let on = calc("yefnen_slay", Some(10), &[]);
            // 中ディレイが未収録で回しを組めなかった状況を作る
            // (gamedata の全技には中ディレイがあるので、手で外す)
            let flag = on.flag.clone().unwrap();
            let mut body = on.body.clone();
            body.dps = None;
            body.expected_dps = None;
            body.defeat_seconds = None;
            let combined = super::super::combine_damage(&body, None, Some(&flag), None);
            assert_eq!(combined.dps, None);
            assert_eq!(combined.expected_dps, None);
            assert_eq!(combined.defeat_seconds, None);
            // 1 発の主役数字(爆発ぶん)は DPS とは無関係に出る
            assert!(combined.total_primary > body.total_primary);
        }

        /// ホーム(全コンテンツ評価)の討伐時間は、計算タブの合算(`combined`)と同じ値に
        /// なる。2 つの経路で別の数字が出ると「ホームでは行けるのに計算タブでは届かない」
        /// が起きるので、同じスキル・同じコンテンツで突き合わせる。
        fn yefnen(stacks: Option<u8>) -> NewCharacter {
            let mut stat_sources = StatSources::default();
            if let Some(stacks) = stacks {
                stat_sources.character_skills.skill_ids.push("yefnen_flag".to_string());
                stat_sources.character_skills.skill_levels.insert("yefnen_flag".to_string(), stacks);
            }
            NewCharacter {
                name: "イェフネン".to_string(),
                game_character_id: "yefnen".to_string(),
                base_stats: yefnen_stats(),
                awakening: domain::Awakening::default(),
                stat_sources,
                equipment: Equipment::default(),
                common_skills: CommonSkills::default(),
                main_skill_id: Some("yefnen_slay".to_string()),
                summon_skill_id: None,
                rotation_skill_ids: None,
                goal_content_id: None,
                default_buff_set_id: None,
            }
        }

        /// ホームの評価と計算タブを同じコンテンツ・同じスキルで突き合わせる。
        fn home_and_calc(stacks: Option<u8>) -> (Option<f64>, Option<f64>) {
            let character = yefnen(stacks);
            let evals =
                super::super::evaluate_contents(character.clone(), BuffSelection::default(), None)
                    .unwrap();
            let eval = evals.iter().find(|e| e.content_id == "tutatur").unwrap();
            let best = eval.damage.as_ref().unwrap();
            let calc = super::super::damage_for_character(
                &character.base_stats,
                &character.game_character_id,
                character.main_skill_id.as_deref(),
                None,
                &character.stat_sources,
                &BuffSelection::default(),
                character.equipment.clone(),
                character.common_skills,
                character.awakening,
                &best.skill_id,
                "tutatur",
                0,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            (best.defeat_seconds, calc.combined.defeat_seconds)
        }

        #[test]
        fn ホームの討伐時間は計算タブの合算と一致する() {
            let (home, calc) = home_and_calc(Some(10));
            assert!(home.is_some() && calc.is_some());
            assert!((home.unwrap() - calc.unwrap()).abs() < 1e-6, "home={home:?} calc={calc:?}");
        }

        #[test]
        fn スタック0ならホームの討伐時間は従来どおり() {
            let (home, calc) = home_and_calc(None);
            assert!((home.unwrap() - calc.unwrap()).abs() < 1e-6);
            // <フラグ> を積むと討伐が速くなる(積まないときと同じ値にならない)
            let (with_flag, _) = home_and_calc(Some(10));
            assert!(with_flag.unwrap() < home.unwrap());
        }

        #[test]
        fn 他キャラのホーム評価は変わらない() {
            let mut boris = anais();
            boris.name = "ボリス".to_string();
            boris.game_character_id = "boris".to_string();
            boris.base_stats = BaseStats { stab: 300, hack: 300, int: 1, def: 1, mr: 1, dex: 1, agi: 1 };
            let evals =
                super::super::evaluate_contents(boris.clone(), BuffSelection::default(), None)
                    .unwrap();
            let eval = evals.iter().find(|e| e.content_id == "tutatur").unwrap();
            let best = eval.damage.as_ref().unwrap();
            // <フラグ> が無いキャラは本体単独の討伐時間のまま
            assert_eq!(
                best.defeat_seconds,
                domain::defeat_seconds(
                    gamedata::find_enemy("tutatur").unwrap().hp,
                    best.expected_dps
                )
            );
        }

        /// 強化候補の試算は計算タブと同じ合算経路(回し・<フラグ>・召喚獣込み)を通る。
        /// 候補の `total_primary` は、その候補を当てたキャラを計算タブに通した合算と一致する。
        #[test]
        fn 強化候補は計算タブと同じ合算で試算する() {
            let character = yefnen(Some(10));
            let candidates = super::super::list_upgrade_candidates(
                character.clone(),
                BuffSelection::default(),
                "yefnen_slay".to_string(),
                "tutatur".to_string(),
                0,
                None,
                None,
            )
            .unwrap();
            assert!(!candidates.is_empty());
            for candidate in &candidates {
                let applied = super::super::preview_damage(
                    candidate.applied.clone(),
                    BuffSelection::default(),
                    "yefnen_slay".to_string(),
                    "tutatur".to_string(),
                    0,
                    None,
                    None,
                    None,
                )
                .unwrap();
                assert_eq!(
                    candidate.total_primary, applied.combined.total_primary,
                    "{}",
                    candidate.id
                );
                // <フラグ> の爆発ぶんが入っている(技単独ではない)
                assert!(candidate.total_primary > applied.body.total_primary, "{}", candidate.id);
            }
        }

        #[test]
        fn 他キャラの結果は変わらない() {
            let before = super::super::damage_for_character(
                &BaseStats { stab: 300, hack: 300, int: 1, def: 1, mr: 1, dex: 1, agi: 1 },
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
                None,
            )
            .unwrap();
            assert!(before.flag.is_none());
            // ボリスの横薙ぎに CT は無く、差し込める CT 技も無いので回しは組まない
            assert!(before.rotation.is_none());
            assert_eq!(before.combined.total_primary, before.body.total_primary);
            assert_eq!(before.combined.dps, before.body.dps);
            assert_eq!(before.combined.expected_dps, before.body.expected_dps);
        }
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
