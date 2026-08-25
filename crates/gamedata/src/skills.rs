//! スキルカタログ。

use domain::{Element, Skill, SkillDependency};

use crate::Source;

pub const SKILLS_SOURCE: Source = Source {
    page: "Skill/ボリス「スキル性能一覧」(一覧表対応バージョン 2026/1/28)",
    retrieved_on: "2026-08-25",
    note: "攻撃力(倍率)・段数・Cri倍・属性・命中・Cri値・SLv を一覧表から転記。倍率 / 段数 / Cri倍は           旧リポ twtoolkit boris.json(Excel v4.00 由来)と全件一致。スキル命中は wiki 表記 +15           (計算式まとめ #AccuracyPoint の注記)。スキル Lv 別倍率は未対応",
};

struct SkillRecord {
    character_id: &'static str,
    id: &'static str,
    name: &'static str,
    dependency: SkillDependency,
    multiplier: f64,
    hit_count: u32,
    critical_multiplier: f64,
    element: Option<Element>,
    /// wiki 表記 +15 した実値
    accuracy: i64,
    critical_rate: i64,
    level: u8,
}

const SKILLS: &[SkillRecord] = &[
    // 剣系。wiki スキル性能一覧: 99% / Cri倍 2x / 属性 無 / 命中 83 / Cri値 8 / SLv1
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_yokogiri",
        name: "極・横斬り",
        dependency: SkillDependency::StabHack,
        multiplier: 0.99,
        hit_count: 1,
        critical_multiplier: 2.0,
        element: Some(Element::Neutral),
        accuracy: 98,
        critical_rate: 8,
        level: 1,
    },
    // 刀系。wiki: 109% / 2.5x / 無 / 命中 77 / Cri値 7 / SLv1
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_tategiri",
        name: "極・縦斬り",
        dependency: SkillDependency::Hack,
        multiplier: 1.09,
        hit_count: 1,
        critical_multiplier: 2.5,
        element: Some(Element::Neutral),
        accuracy: 92,
        critical_rate: 7,
        level: 1,
    },
    // 氷結系。wiki: 113% / 2.25x / 水 / 命中 77 / Cri値 7 / SLv1
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_ice_break",
        name: "極・アイスブレイク",
        dependency: SkillDependency::HackInt,
        multiplier: 1.13,
        hit_count: 1,
        critical_multiplier: 2.25,
        element: Some(Element::Water),
        accuracy: 92,
        critical_rate: 7,
        level: 1,
    },
    // 剣系。wiki: 545%x11 / 2.7x / 無 / 命中 87 / Cri値 13 / SLv10(追加効果 [暗黒])
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_zaneizan",
        name: "極・残影斬",
        dependency: SkillDependency::StabHack,
        multiplier: 5.45,
        hit_count: 11,
        critical_multiplier: 2.7,
        element: Some(Element::Neutral),
        accuracy: 102,
        critical_rate: 13,
        level: 10,
    },
    // 刀系。wiki: 550%x11 / 2.5x / 無 / 命中 77 / Cri値 6 / SLv10
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_ren",
        name: "極・連",
        dependency: SkillDependency::Hack,
        multiplier: 5.5,
        hit_count: 11,
        critical_multiplier: 2.5,
        element: Some(Element::Neutral),
        accuracy: 92,
        critical_rate: 6,
        level: 10,
    },
];

impl SkillRecord {
    fn to_skill(&self) -> Skill {
        Skill {
            id: self.id.to_string(),
            name: self.name.to_string(),
            dependency: self.dependency,
            multiplier: self.multiplier,
            hit_count: self.hit_count,
            critical_multiplier: self.critical_multiplier,
            element: self.element,
            accuracy: self.accuracy,
            critical_rate: self.critical_rate,
            level: self.level,
        }
    }
}

/// キャラクターのスキル一覧。
pub fn skills_for(character_id: &str) -> Vec<Skill> {
    SKILLS
        .iter()
        .filter(|s| s.character_id == character_id)
        .map(SkillRecord::to_skill)
        .collect()
}

pub fn find_skill(id: &str) -> Option<Skill> {
    SKILLS.iter().find(|s| s.id == id).map(SkillRecord::to_skill)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ボリスのスキルは5件() {
        let skills = skills_for("boris");
        assert_eq!(skills.len(), 5);
        assert!(skills_for("nope").is_empty());
    }

    #[test]
    fn id_で検索できる() {
        let s = find_skill("boris_goku_zaneizan").unwrap();
        assert_eq!(s.name, "極・残影斬");
        assert_eq!(s.hit_count, 11);
        assert_eq!(s.dependency, SkillDependency::StabHack);
        assert!(find_skill("nope").is_none());
    }

    #[test]
    fn id_は一意() {
        let mut ids: Vec<_> = SKILLS.iter().map(|s| s.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), SKILLS.len());
    }
}
