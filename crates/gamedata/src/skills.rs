//! スキルカタログ。

use domain::{Skill, SkillDependency};

use crate::{Source, LEGACY_TWTOOLKIT_RETRIEVED_ON};

pub const SKILLS_SOURCE: Source = Source {
    page: "旧リポ twtoolkit boris.json",
    retrieved_on: LEGACY_TWTOOLKIT_RETRIEVED_ON,
    note: "Excel ダメージ計算器 v4.00 由来。スキル Lv 別倍率は未対応。wiki のボリススキルページで要裏取り",
};

struct SkillRecord {
    character_id: &'static str,
    id: &'static str,
    name: &'static str,
    dependency: SkillDependency,
    multiplier: f64,
    hit_count: u32,
    critical_multiplier: f64,
}

const SKILLS: &[SkillRecord] = &[
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_yokogiri",
        name: "極・横斬り",
        dependency: SkillDependency::StabHack,
        multiplier: 0.99,
        hit_count: 1,
        critical_multiplier: 2.0,
    },
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_tategiri",
        name: "極・縦斬り",
        dependency: SkillDependency::Hack,
        multiplier: 1.09,
        hit_count: 1,
        critical_multiplier: 2.5,
    },
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_ice_break",
        name: "極・アイスブレイク",
        dependency: SkillDependency::HackInt,
        multiplier: 1.13,
        hit_count: 1,
        critical_multiplier: 2.25,
    },
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_zaneizan",
        name: "極・残影斬",
        dependency: SkillDependency::StabHack,
        multiplier: 5.45,
        hit_count: 11,
        critical_multiplier: 2.7,
    },
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_ren",
        name: "極・連",
        dependency: SkillDependency::Hack,
        multiplier: 5.5,
        hit_count: 11,
        critical_multiplier: 2.5,
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
