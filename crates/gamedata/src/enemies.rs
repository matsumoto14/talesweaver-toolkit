//! 敵カタログ。

use domain::Enemy;

use crate::{Source, LEGACY_TWTOOLKIT_RETRIEVED_ON};

pub const ENEMIES_SOURCE: Source = Source {
    page: "旧リポ twtoolkit monsters.json",
    retrieved_on: LEGACY_TWTOOLKIT_RETRIEVED_ON,
    note: "af63 → 被害減少 M(符号反転して負値で保持)、af64 → カット率A V1。wiki 狩り場情報一覧で要裏取り",
};

struct EnemyRecord {
    id: &'static str,
    name: &'static str,
    defense: i64,
    damage_reduction: i64,
    cut_rate_a: f64,
    element_threshold: i64,
}

const ENEMIES: &[EnemyRecord] = &[
    EnemyRecord {
        id: "tutatur",
        name: "トゥタトゥール",
        defense: 990,
        damage_reduction: 0,
        cut_rate_a: 1.0,
        element_threshold: 90,
    },
    EnemyRecord {
        id: "brothers_forge",
        name: "兄弟の鍛冶場",
        defense: 7050,
        damage_reduction: -5850,
        cut_rate_a: 0.405,
        element_threshold: 120,
    },
    EnemyRecord {
        id: "odin_rank",
        name: "オーディン(ランク)",
        defense: 59220,
        damage_reduction: -4550,
        cut_rate_a: 0.49,
        element_threshold: 125,
    },
];

impl EnemyRecord {
    fn to_enemy(&self) -> Enemy {
        Enemy {
            id: self.id.to_string(),
            name: self.name.to_string(),
            defense: self.defense,
            damage_reduction: self.damage_reduction,
            cut_rate_a: self.cut_rate_a,
            element_threshold: self.element_threshold,
        }
    }
}

pub fn enemies() -> Vec<Enemy> {
    ENEMIES.iter().map(EnemyRecord::to_enemy).collect()
}

pub fn find_enemy(id: &str) -> Option<Enemy> {
    ENEMIES.iter().find(|e| e.id == id).map(EnemyRecord::to_enemy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 敵は3体() {
        assert_eq!(enemies().len(), 3);
    }

    #[test]
    fn id_で検索でき被害減少は負値() {
        let e = find_enemy("brothers_forge").unwrap();
        assert_eq!(e.defense, 7050);
        assert_eq!(e.damage_reduction, -5850);
        assert!((e.cut_rate_a - 0.405).abs() < 1e-12);
        assert!(find_enemy("nope").is_none());
        assert!(enemies().iter().all(|e| e.damage_reduction <= 0));
    }
}
