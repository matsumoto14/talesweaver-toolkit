//! コンテンツ(狩り場・ボス)と入場条件の判定。ロードマップ機能の最小版。
//!
//! 「このキャラでどのコンテンツに行けるか」を、目安ダメージ(火力)と入場条件の
//! 2 軸で判定する。判定に使う値はすべて登録キャラのデータから取れるものに限る
//! (ユーザーに判定用の値を入力させない。docs/ux-guidelines.md 原則1)。
//! コンテンツの実データ(目安・条件の数値)は gamedata が持つ。

use serde::{Deserialize, Serialize};

use crate::awakening::Awakening;
use crate::equipment::EquipmentValues;

/// 入場条件。判定に使う値は登録キャラのデータから取れるものに限る。
///
/// テシスコア等、現行のキャラモデルに無い値を条件にしない(判定できない条件を
/// データに持たせると「常に未達」か「常に無視」のどちらかの嘘になる)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentRequirement {
    /// 装備の基本能力値「突き」がこの値以上
    EquipmentThrust(i64),
    /// 装備の基本能力値「突き+斬り」の合計がこの値以上
    EquipmentThrustSlash(i64),
    /// エタの意志 Lv がこの値以上
    EternalLevel(u8),
}

/// 入場条件 1 件の判定結果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementCheck {
    pub label: String,
    pub current: i64,
    pub required: i64,
    pub ok: bool,
}

impl ContentRequirement {
    /// `equipment_base` は `Equipment::base_totals` で集計済みの基本能力値。
    pub fn check(&self, equipment_base: &EquipmentValues, awakening: Awakening) -> RequirementCheck {
        let (label, current, required) = match *self {
            ContentRequirement::EquipmentThrust(v) => ("装備 突き(基本)", equipment_base.thrust, v),
            ContentRequirement::EquipmentThrustSlash(v) => {
                ("装備 突き+斬り(基本)", equipment_base.thrust + equipment_base.slash, v)
            }
            ContentRequirement::EternalLevel(v) => {
                ("エタの意志 Lv", i64::from(awakening.eternal_level), i64::from(v))
            }
        };
        RequirementCheck { label: label.to_string(), current, required, ok: current >= required }
    }
}

/// コンテンツ(ボス・狩り場)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Content {
    pub id: String,
    pub name: String,
    /// 対応する敵データ(`Enemy::id`)。防御力・被害減少等はそちらを使う
    pub enemy_id: String,
    /// 実用的に周回できる 1 ヒット(最大)の目安ダメージ
    pub need_per_hit: i64,
    pub requirements: Vec<ContentRequirement>,
    /// チーム条件の注記(無ければ None)
    pub team_note: Option<String>,
}

/// エリア(コンテンツのグループ)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentArea {
    pub id: String,
    pub name: String,
    pub contents: Vec<Content>,
}

/// キャラのスキルで出せる最大ダメージ(コンテンツ判定の火力側の入力)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BestSkillDamage {
    pub skill_id: String,
    /// 1 ヒット(最大)
    pub per_hit_max: i64,
    /// 合計(最大)= 1 ヒット × 段数
    pub total_max: i64,
}

/// コンテンツ 1 件の判定結果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentEvaluation {
    pub content_id: String,
    /// そのキャラの最大ダメージスキルでの火力。スキル未収録キャラは None
    pub damage: Option<BestSkillDamage>,
    pub checks: Vec<RequirementCheck>,
    pub entry_ok: bool,
    /// 火力が目安に届いているか。ダメージ不明(スキル未収録)は false
    pub reaches_need: bool,
    /// 火力(目安到達)と入場条件の両方を満たすか
    pub clear: bool,
}

/// コンテンツ 1 件を判定する。`damage` はスキル未収録キャラでは None。
pub fn evaluate_content(
    content: &Content,
    damage: Option<BestSkillDamage>,
    equipment_base: &EquipmentValues,
    awakening: Awakening,
) -> ContentEvaluation {
    let checks: Vec<RequirementCheck> =
        content.requirements.iter().map(|r| r.check(equipment_base, awakening)).collect();
    let entry_ok = checks.iter().all(|c| c.ok);
    let reaches_need =
        damage.as_ref().is_some_and(|d| d.per_hit_max >= content.need_per_hit);
    ContentEvaluation {
        content_id: content.id.clone(),
        damage,
        checks,
        entry_ok,
        reaches_need,
        clear: entry_ok && reaches_need,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equipment(thrust: i64, slash: i64) -> EquipmentValues {
        EquipmentValues { thrust, slash, ..Default::default() }
    }

    fn content(need: i64, requirements: Vec<ContentRequirement>) -> Content {
        Content {
            id: "c".into(),
            name: "テスト".into(),
            enemy_id: "e".into(),
            need_per_hit: need,
            requirements,
            team_note: None,
        }
    }

    #[test]
    fn 入場条件の判定は境界値を含む() {
        let eq = equipment(400, 300);
        let aw = Awakening { stage: 5, eternal_level: 41 };

        let c = ContentRequirement::EquipmentThrust(400).check(&eq, aw);
        assert!(c.ok);
        assert_eq!((c.current, c.required), (400, 400));
        assert!(!ContentRequirement::EquipmentThrust(401).check(&eq, aw).ok);

        let c = ContentRequirement::EquipmentThrustSlash(700).check(&eq, aw);
        assert!(c.ok);
        assert_eq!(c.current, 700);
        assert!(!ContentRequirement::EquipmentThrustSlash(701).check(&eq, aw).ok);

        assert!(ContentRequirement::EternalLevel(41).check(&eq, aw).ok);
        assert!(!ContentRequirement::EternalLevel(42).check(&eq, aw).ok);
    }

    #[test]
    fn クリア判定は火力と入場条件の両方を見る() {
        let eq = equipment(400, 300);
        let aw = Awakening::default();
        let dmg = |per: i64| Some(BestSkillDamage { skill_id: "s".into(), per_hit_max: per, total_max: per });

        // 火力・条件とも満たす
        let e = evaluate_content(&content(1000, vec![ContentRequirement::EquipmentThrust(100)]), dmg(1000), &eq, aw);
        assert!(e.entry_ok && e.reaches_need && e.clear);

        // 火力だけ未達
        let e = evaluate_content(&content(1001, vec![]), dmg(1000), &eq, aw);
        assert!(e.entry_ok && !e.reaches_need && !e.clear);

        // 条件だけ未達
        let e = evaluate_content(&content(1000, vec![ContentRequirement::EternalLevel(1)]), dmg(1000), &eq, aw);
        assert!(!e.entry_ok && e.reaches_need && !e.clear);
    }

    #[test]
    fn スキル未収録はダメージ不明としてクリア不可() {
        let e = evaluate_content(&content(1, vec![]), None, &equipment(0, 0), Awakening::default());
        assert!(e.entry_ok);
        assert!(!e.reaches_need && !e.clear);
        assert!(e.damage.is_none());
    }
}
