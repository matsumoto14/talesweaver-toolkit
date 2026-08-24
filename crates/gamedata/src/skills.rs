//! スキルカタログ。

use domain::{Element, Skill, SkillDependency};

use crate::{Source, LEGACY_TWTOOLKIT_RETRIEVED_ON};

pub const SKILLS_SOURCE: Source = Source {
    page: "旧リポ twtoolkit boris.json",
    retrieved_on: LEGACY_TWTOOLKIT_RETRIEVED_ON,
    note: "Excel ダメージ計算器 v4.00 由来。スキル Lv 別倍率は未対応。wiki のボリススキルページで要裏取り",
};

/// スキル属性の出典(wiki 属性システム「スキル属性」)。
pub const SKILL_ELEMENT_SOURCE: Source = Source {
    page: "属性システム",
    retrieved_on: "2026-08-25",
    note: "ボリスは 水属性=氷結系/氷撃斬/フローズンスレイ/フローズンブレイク、土属性=大地系、           黒属性=黒魔法系、無属性=共通系/剣系/刀系(縦斬り、連、円)。刀系のうち横斬り・残影斬は           どの属性の行にも無く読み取れないため None(`[仮]`)",
};

struct SkillRecord {
    character_id: &'static str,
    id: &'static str,
    name: &'static str,
    dependency: SkillDependency,
    multiplier: f64,
    hit_count: u32,
    critical_multiplier: f64,
    /// wiki 属性システム「スキル属性」から読み取れないものは `None` `[仮]`
    element: Option<Element>,
}

const SKILLS: &[SkillRecord] = &[
    // 刀系。無属性の行は「縦斬り、連、円」だけを挙げていて横斬りが無い `[仮]`
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_yokogiri",
        name: "極・横斬り",
        dependency: SkillDependency::StabHack,
        multiplier: 0.99,
        hit_count: 1,
        critical_multiplier: 2.0,
        element: None,
    },
    // 無属性(刀系: 縦斬り)
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_tategiri",
        name: "極・縦斬り",
        dependency: SkillDependency::Hack,
        multiplier: 1.09,
        hit_count: 1,
        critical_multiplier: 2.5,
        element: Some(Element::Neutral),
    },
    // 水属性(氷結系)
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_ice_break",
        name: "極・アイスブレイク",
        dependency: SkillDependency::HackInt,
        multiplier: 1.13,
        hit_count: 1,
        critical_multiplier: 2.25,
        element: Some(Element::Water),
    },
    // 刀系。無属性の行に残影斬が無い `[仮]`
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_zaneizan",
        name: "極・残影斬",
        dependency: SkillDependency::StabHack,
        multiplier: 5.45,
        hit_count: 11,
        critical_multiplier: 2.7,
        element: None,
    },
    // 無属性(刀系: 連)
    SkillRecord {
        character_id: "boris",
        id: "boris_goku_ren",
        name: "極・連",
        dependency: SkillDependency::Hack,
        multiplier: 5.5,
        hit_count: 11,
        critical_multiplier: 2.5,
        element: Some(Element::Neutral),
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
