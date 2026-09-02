//! 与ダメージ計算の入力のうち、複数の gamedata カタログをまたいで集計する値。
//!
//! gamedata のカタログを解決して domain の計算関数(メソッド含む)を呼ぶだけの接着層。
//! カタログ解決は commands の役目なので gamedata ではなくここに置く(docs/architecture.md)。

/// 与ダメージ式のカテゴリへの寄与
/// (キャラスキル + マスタリー + バフ + 装備アビリティ + 装備アイテムの装着時効果)。
/// カタログが分かれているのでここでまとめる。
pub fn damage_contributions_of(
    sources: &domain::StatSources,
    buffs: &domain::BuffSelection,
    equipment: &domain::Equipment,
    dependency: domain::SkillDependency,
) -> Vec<domain::DamageContribution> {
    let mut out = sources
        .character_skills
        .damage_contributions(gamedata::character_skill_catalog(), &sources.masteries);
    out.extend(equipment.ability_damage_contributions(&gamedata::equipment_abilities()));
    out.extend(gamedata::item_damage_contributions(equipment, dependency));
    out.extend(
        sources
            .masteries
            .damage_contributions(gamedata::mastery_catalog()),
    );
    out.extend(domain::stat_sources::buff_damage_contributions(
        buffs,
        &gamedata::buff_catalog(),
    ));
    out.extend(sources.soul_link.damage_contributions());
    out
}

/// 中ディレイ減少の寄与(キャラスキル + マスタリー)。
/// キャラスキルとマスタリーはカタログが別なので、ここでまとめる。
pub fn actual_delay_contributions(
    skills: &domain::CharacterSkills,
    masteries: &domain::Masteries,
) -> Vec<domain::ActualDelayContribution> {
    let mut out = skills.actual_delay_contributions(gamedata::character_skill_catalog(), masteries);
    let catalog = gamedata::mastery_catalog();
    for id in &masteries.picked {
        if let Some(def) = catalog.iter().find(|d| d.id == id.as_str()) {
            if let domain::SkillEffect::ActualDelay { percent } = def.effect {
                out.push(domain::ActualDelayContribution {
                    source: format!("マスタリー【{}】", def.name),
                    rate: percent / 100.0,
                });
            }
        }
    }
    out
}

/// 属性値の内訳。キャラの基礎属性値(gamedata)+ 装備の属性強化(部位ごとに 0〜9)+
/// 装備以外の供給源(ペット / モンスターカード / ルーン / 頭アビ / カフスアビ)。合計は上限 255。
pub fn element_preview(
    game_character_id: &str,
    equipment: &domain::Equipment,
    stat_sources: &domain::StatSources,
) -> domain::ElementPreview {
    domain::ElementPreview::new(
        gamedata::element_base(game_character_id),
        equipment.element_values(stat_sources.elements.selected()),
        stat_sources
            .elements
            .values(gamedata::element_source_catalog()),
    )
}

/// スキルの属性に対応するキャラの属性値(wiki: カテゴリI の起点)。
pub fn element_value_for(
    game_character_id: &str,
    equipment: &domain::Equipment,
    stat_sources: &domain::StatSources,
    skill: &domain::Skill,
) -> i64 {
    element_preview(game_character_id, equipment, stat_sources)
        .total
        .get(skill.element.effective_for_attack(stat_sources.elements.selected()))
}
