//! ゲーム内キャラクター(操作キャラ)と、スキル依存種別ごとのステ由来攻撃力係数。

use domain::{AttackCoefficients, SkillDependency, StatKind};
use serde::Serialize;

use crate::{Source, LEGACY_TWTOOLKIT_RETRIEVED_ON};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GameCharacter {
    pub id: &'static str,
    pub name: &'static str,
}

const CHARACTERS: &[GameCharacter] = &[GameCharacter { id: "boris", name: "ボリス" }];

pub fn characters() -> &'static [GameCharacter] {
    CHARACTERS
}

pub fn find_character(id: &str) -> Option<&'static GameCharacter> {
    CHARACTERS.iter().find(|c| c.id == id)
}

/// ステ由来攻撃力係数の出典。
pub const ATTACK_COEFFICIENTS_SOURCE: Source = Source {
    page: "旧リポ twtoolkit rawStatCoefficients.json",
    retrieved_on: LEGACY_TWTOOLKIT_RETRIEVED_ON,
    note: "Excel ダメージ計算器 v4.00 由来。wiki Skill#formula / 計算式まとめ#BaseAttackPower で要裏取り",
};

/// スキル依存種別ごとのステ由来攻撃力係数(wiki: カテゴリA の内訳)。
///
/// 全キャラ共通(旧リポのデータ構造に同じ)。
pub fn attack_coefficients(dependency: SkillDependency) -> AttackCoefficients {
    use StatKind::*;
    let (primary, secondary) = match dependency {
        SkillDependency::Stab => ((Stab, 2.1), (Hack, 1.08)),
        SkillDependency::Hack => ((Hack, 2.1), (Stab, 1.08)),
        SkillDependency::Int => ((Int, 2.4), (Mr, 0.6)),
        SkillDependency::Mr => ((Mr, 2.55), (Int, 0.45)),
        SkillDependency::StabHack => ((Stab, 1.8), (Hack, 1.8)),
        SkillDependency::HackInt => ((Hack, 1.8), (Int, 1.8)),
    };
    AttackCoefficients { primary, secondary }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{stat_attack_power, EffectiveStats};

    #[test]
    fn ボリスが登録されている() {
        assert_eq!(characters().len(), 1);
        assert_eq!(find_character("boris").unwrap().name, "ボリス");
        assert!(find_character("nope").is_none());
    }

    #[test]
    fn 依存種別ごとの係数() {
        let stats = EffectiveStats { stab: 100, hack: 200, int: 300, mr: 400, ..Default::default() };
        let power = |d| stat_attack_power(&stats, &attack_coefficients(d));
        // 1.08×HACK + 2.1×STAB = 216 + 210
        assert!((power(SkillDependency::Stab) - 426.0).abs() < 1e-9);
        // 1.08×STAB + 2.1×HACK = 108 + 420
        assert!((power(SkillDependency::Hack) - 528.0).abs() < 1e-9);
        // 2.4×INT + 0.6×MR = 720 + 240
        assert!((power(SkillDependency::Int) - 960.0).abs() < 1e-9);
        // 0.45×INT + 2.55×MR = 135 + 1020
        assert!((power(SkillDependency::Mr) - 1155.0).abs() < 1e-9);
        // 1.8×(STAB+HACK) = 540
        assert!((power(SkillDependency::StabHack) - 540.0).abs() < 1e-9);
        // 1.8×(HACK+INT) = 900
        assert!((power(SkillDependency::HackInt) - 900.0).abs() < 1e-9);
    }
}
