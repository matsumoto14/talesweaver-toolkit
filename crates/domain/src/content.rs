//! コンテンツ(狩り場・ボス)と入場条件の判定。ロードマップ機能の最小版。
//!
//! 「このキャラでどのコンテンツに行けるか」を、目安ダメージ(火力)と入場条件の
//! 2 軸で判定する。判定に使う値はすべて登録キャラのデータから取れるものに限る
//! (ユーザーに判定用の値を入力させない。docs/ux-guidelines.md 原則1)。
//! コンテンツの実データ(目安・条件の数値)は gamedata が持つ。
//!
//! 装備条件は「使うスキルの依存種別で比較先が決まる」(swiki コンテンツ入場条件+
//! ユーザー確認 2026-08-24): 物理/魔法/複合のいずれか 1 つを満たせばよい。判定には
//! そのキャラの最大ダメージスキルの依存種別を使う。

use serde::{Deserialize, Serialize};

use crate::awakening::Awakening;
use crate::equipment::EquipmentValues;
use crate::skill::SkillDependency;
use crate::thesis_core::CoreRegion;

/// ゲーム内で称号などの地域限定効果を判定する地域。
/// テシスコアの地域とは独立した概念として扱う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameRegion {
    LostIsland,
    ShinchouNest,
    ArklonUnderground,
    Praba,
}

/// 入場条件。判定に使う値は登録キャラのデータから取れるものに限る。
///
/// ルーンレベル(ルーンマスターLv)・共通スキルコンプ等、現行のキャラモデルに無い値は
/// 条件にしない(判定できない条件をデータに持たせると「常に未達」か「常に無視」の
/// どちらかの嘘になる)。それらは `Content::entry_note` に表示専用で持つ。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentRequirement {
    /// 覚醒段階がこの値以上
    AwakeningStage(u8),
    /// エタの意志 Lv がこの値以上
    EternalLevel(u8),
    /// 装備補正。判定値は基本能力値+強化能力値(エンチャント・シエナのオーラ)で、
    /// テシスコアの能力値増加は対象ダンジョン内限定のため含めない。
    /// 比較先は判定に使うスキルの依存種別で選ぶ:
    /// Stab/Hack/Int → `single` を 突き/斬り/魔攻 と比較、Mr → `mr` を 魔防 と比較、
    /// StabHack/HackInt → `composite` を 突き+斬り / 斬り+魔攻 と比較。
    /// 値 0 は「その系統の条件なし」(チェックを生成しない)。
    EquipmentBySkill {
        single: i64,
        mr: i64,
        composite: i64,
    },
    /// テシスコアの火力補正合計がこの値以上(swiki の「コア N」)。
    /// 判定対象は `Content::core_region` の地域のコアセット(地域不明なら全地域の最大値)。
    ThesisCoreTotal(i64),
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
    /// `equipment` は基本能力値+強化能力値(テシスコア除く)の集計済み合計。
    /// `dependency` は判定に使うスキルの依存種別(スキル未収録キャラは None)。
    /// `thesis_core_total` はこのコンテンツの地域で効くテシスコアの火力補正合計。
    pub fn check(
        &self,
        equipment: &EquipmentValues,
        awakening: Awakening,
        dependency: Option<SkillDependency>,
        thesis_core_total: i64,
    ) -> RequirementCheck {
        let (label, current, required) = match *self {
            ContentRequirement::AwakeningStage(v) => {
                ("覚醒段階", i64::from(awakening.stage), i64::from(v))
            }
            ContentRequirement::EternalLevel(v) => (
                "エタの意志 Lv",
                i64::from(awakening.eternal_level),
                i64::from(v),
            ),
            ContentRequirement::EquipmentBySkill {
                single,
                mr,
                composite,
            } => match dependency {
                None => ("装備補正(スキル未収録のため判定不可)", 0, single),
                Some(SkillDependency::Stab) => ("装備 突き", equipment.thrust, single),
                Some(SkillDependency::Hack) => ("装備 斬り", equipment.slash, single),
                Some(SkillDependency::Int) => {
                    ("装備 魔攻", equipment.magic_attack, single)
                }
                Some(SkillDependency::Mr) => ("装備 魔防", equipment.magic_defense, mr),
                Some(SkillDependency::StabHack) => (
                    "装備 突き+斬り",
                    equipment.thrust + equipment.slash,
                    composite,
                ),
                Some(SkillDependency::HackInt) => (
                    "装備 斬り+魔攻",
                    equipment.slash + equipment.magic_attack,
                    composite,
                ),
            },
            ContentRequirement::ThesisCoreTotal(v) => ("テシスコア 合計", thesis_core_total, v),
        };
        RequirementCheck {
            label: label.to_string(),
            current,
            required,
            ok: current >= required,
        }
    }
}

/// 段数違いの同一コンテンツ(例: レリックの聖域 10段〜19段)をまとめる系列。
/// 一覧はこの単位で 1 行に畳み、段は難易度ステッパーで切り替える。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentSeries {
    /// 系列 id(同じ系列の `Content` が共有する)
    pub id: String,
    /// 系列の表示名(例: レリックの聖域)
    pub name: String,
    /// この `Content` の段(難易度)
    pub step: u32,
}

/// コンテンツ(ボス・狩り場)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Content {
    pub id: String,
    pub name: String,
    /// 段数違いの系列に属するなら系列情報。単独のコンテンツは `None`
    pub series: Option<ContentSeries>,
    /// 対応する敵データ(`Enemy::id`)。敵データが無い(入場条件のみ判定する)コンテンツは None
    pub enemy_id: Option<String>,
    /// 実用的に周回できる 1 ヒット(最大)の目安ダメージ。敵データが無いコンテンツは None
    pub need_per_hit: Option<i64>,
    pub requirements: Vec<ContentRequirement>,
    /// このコンテンツで効くテシスコアの地域(wiki: テシスコア「実装済みダンジョンコア」の
    /// 発動場所。対応が取れないコンテンツは None = コアの能力値増加は乗らない)
    pub core_region: Option<CoreRegion>,
    /// 称号など、ゲーム内の地域限定効果を判定する地域。
    pub game_region: Option<GameRegion>,
    /// 判定対象外の入場条件の注記(ルーンレベル・共通スキルコンプ等。表示専用)
    pub entry_note: Option<String>,
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
    /// 1 ヒットの主役値(`DamageTriple::primary`。クリ発生率 > 0 ならクリティカル、
    /// 0 なら非クリ最大。ユーザー判断 2026-08-29)
    pub per_hit_primary: i64,
    /// 合計の主役値 = 1 ヒットの主役値 × 段数
    pub total_primary: i64,
}

/// 火力の到達段。目安(`Content::need_per_hit`)に対する 1 ヒット主役値の比で決める。
/// 段の境目はホームの一覧・スポットライト・計算タブの到達バッジで共通
/// (「余裕 / 通る / ぎりぎり / 届かない」)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReachTier {
    /// 目安の 1.3 倍以上
    Comfortable,
    /// 目安以上
    Reached,
    /// 目安の 0.8 倍以上
    Close,
    Short,
}

impl ReachTier {
    /// 目安が 0 以下(火力不問)なら `Reached`。
    pub fn of(per_hit: i64, need: i64) -> Self {
        if need <= 0 {
            return ReachTier::Reached;
        }
        // 比は整数のまま比べる(1.3 倍 = 13/10、0.8 倍 = 8/10)
        if per_hit * 10 >= need * 13 {
            ReachTier::Comfortable
        } else if per_hit >= need {
            ReachTier::Reached
        } else if per_hit * 10 >= need * 8 {
            ReachTier::Close
        } else {
            ReachTier::Short
        }
    }

    /// 目安に届いているか(`Comfortable` / `Reached`)。
    pub fn reaches(self) -> bool {
        matches!(self, ReachTier::Comfortable | ReachTier::Reached)
    }
}

/// コンテンツ 1 件の判定結果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentEvaluation {
    pub content_id: String,
    /// そのキャラの最大ダメージスキルでの火力。スキル未収録・敵データなしは None
    pub damage: Option<BestSkillDamage>,
    pub checks: Vec<RequirementCheck>,
    pub entry_ok: bool,
    /// 火力の到達段。目安なし・ダメージ不明は None
    pub reach: Option<ReachTier>,
    /// 火力が目安に届いているか。敵データなし(need 無し)は火力不問で true、
    /// ダメージ不明(スキル未収録)は false
    pub reaches_need: bool,
    /// 火力(目安到達)と入場条件の両方を満たすか
    pub clear: bool,
}

/// コンテンツ 1 件を判定する。
///
/// `dependency` は装備条件の比較先を選ぶスキル依存種別(敵ありコンテンツは最大ダメージ
/// スキル、敵なしコンテンツはキャラの代表スキル。スキル未収録キャラは None)。
pub fn evaluate_content(
    content: &Content,
    damage: Option<BestSkillDamage>,
    equipment: &EquipmentValues,
    awakening: Awakening,
    dependency: Option<SkillDependency>,
    thesis_core_total: i64,
) -> ContentEvaluation {
    let checks: Vec<RequirementCheck> = content
        .requirements
        .iter()
        .map(|r| r.check(equipment, awakening, dependency, thesis_core_total))
        // required 0 は「その系統の条件なし」(例: 表で複合列が "-" のコンテンツ)
        .filter(|c| c.required > 0)
        .collect();
    let entry_ok = checks.iter().all(|c| c.ok);
    let reach = content
        .need_per_hit
        .and_then(|need| damage.as_ref().map(|d| ReachTier::of(d.per_hit_primary, need)));
    let reaches_need = match content.need_per_hit {
        None => true,
        Some(_) => reach.is_some_and(ReachTier::reaches),
    };
    ContentEvaluation {
        content_id: content.id.clone(),
        damage,
        checks,
        entry_ok,
        reach,
        reaches_need,
        clear: entry_ok && reaches_need,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 到達段は目安との比で4段に分かれる() {
        assert_eq!(ReachTier::of(130, 100), ReachTier::Comfortable);
        assert_eq!(ReachTier::of(129, 100), ReachTier::Reached);
        assert_eq!(ReachTier::of(100, 100), ReachTier::Reached);
        assert_eq!(ReachTier::of(99, 100), ReachTier::Close);
        assert_eq!(ReachTier::of(80, 100), ReachTier::Close);
        assert_eq!(ReachTier::of(79, 100), ReachTier::Short);
        assert_eq!(ReachTier::of(0, 0), ReachTier::Reached);
        assert!(ReachTier::Reached.reaches());
        assert!(!ReachTier::Close.reaches());
    }

    fn equipment(
        thrust: i64,
        slash: i64,
        magic_attack: i64,
        magic_defense: i64,
    ) -> EquipmentValues {
        EquipmentValues {
            thrust,
            slash,
            magic_attack,
            magic_defense,
            ..Default::default()
        }
    }

    fn content(need: Option<i64>, requirements: Vec<ContentRequirement>) -> Content {
        Content {
            id: "c".into(),
            name: "テスト".into(),
            enemy_id: need.map(|_| "e".into()),
            need_per_hit: need,
            requirements,
            core_region: None,
            game_region: None,
            series: None,
            entry_note: None,
            team_note: None,
        }
    }

    const EQ_REQ: ContentRequirement = ContentRequirement::EquipmentBySkill {
        single: 1500,
        mr: 1700,
        composite: 2100,
    };

    #[test]
    fn 装備条件はスキル依存で比較先が変わる() {
        let eq = equipment(1500, 1400, 1600, 1699);

        let c = EQ_REQ.check(&eq, Awakening::default(), Some(SkillDependency::Stab), 0);
        assert!(c.ok);
        assert_eq!((c.current, c.required), (1500, 1500));
        assert!(
            !EQ_REQ
                .check(&eq, Awakening::default(), Some(SkillDependency::Hack), 0)
                .ok
        );
        assert!(
            EQ_REQ
                .check(&eq, Awakening::default(), Some(SkillDependency::Int), 0)
                .ok
        );

        let c = EQ_REQ.check(&eq, Awakening::default(), Some(SkillDependency::Mr), 0);
        assert!(!c.ok);
        assert_eq!((c.current, c.required), (1699, 1700));

        let c = EQ_REQ.check(
            &eq,
            Awakening::default(),
            Some(SkillDependency::StabHack),
            0,
        );
        assert!(c.ok);
        assert_eq!((c.current, c.required), (2900, 2100));
        let c = EQ_REQ.check(&eq, Awakening::default(), Some(SkillDependency::HackInt), 0);
        assert!(c.ok);
        assert_eq!(c.current, 3000);

        // スキル未収録は判定不可(ok=false)
        assert!(!EQ_REQ.check(&eq, Awakening::default(), None, 0).ok);
    }

    #[test]
    fn 覚醒とエタの判定は境界値を含む() {
        let eq = EquipmentValues::default();
        let aw = Awakening {
            stage: 5,
            eternal_level: 41,
        };
        assert!(
            ContentRequirement::AwakeningStage(5)
                .check(&eq, aw, None, 0)
                .ok
        );
        assert!(
            ContentRequirement::EternalLevel(41)
                .check(&eq, aw, None, 0)
                .ok
        );
        assert!(
            !ContentRequirement::EternalLevel(42)
                .check(&eq, aw, None, 0)
                .ok
        );
    }

    #[test]
    fn クリア判定は火力と入場条件の両方を見る() {
        let eq = equipment(400, 300, 0, 0);
        let aw = Awakening::default();
        let dmg = |per: i64| {
            Some(BestSkillDamage {
                skill_id: "s".into(),
                per_hit_primary: per,
                total_primary: per,
            })
        };
        let dep = Some(SkillDependency::Stab);

        // 火力・条件とも満たす
        let e = evaluate_content(
            &content(
                Some(1000),
                vec![ContentRequirement::EquipmentBySkill {
                    single: 100,
                    mr: 0,
                    composite: 0,
                }],
            ),
            dmg(1000),
            &eq,
            aw,
            dep,
            0,
        );
        assert!(e.entry_ok && e.reaches_need && e.clear);

        // 火力だけ未達
        let e = evaluate_content(&content(Some(1001), vec![]), dmg(1000), &eq, aw, dep, 0);
        assert!(e.entry_ok && !e.reaches_need && !e.clear);

        // 条件だけ未達
        let e = evaluate_content(
            &content(Some(1000), vec![ContentRequirement::EternalLevel(1)]),
            dmg(1000),
            &eq,
            aw,
            dep,
            0,
        );
        assert!(!e.entry_ok && e.reaches_need && !e.clear);
    }

    #[test]
    fn 敵データなしコンテンツは条件のみで判定する() {
        let eq = EquipmentValues::default();
        let aw = Awakening {
            stage: 3,
            eternal_level: 0,
        };
        let e = evaluate_content(
            &content(None, vec![ContentRequirement::AwakeningStage(3)]),
            None,
            &eq,
            aw,
            None,
            0,
        );
        assert!(e.entry_ok && e.reaches_need && e.clear);
        assert!(e.damage.is_none());

        let e = evaluate_content(
            &content(None, vec![ContentRequirement::AwakeningStage(4)]),
            None,
            &eq,
            aw,
            None,
            0,
        );
        assert!(!e.entry_ok && !e.clear);
    }

    #[test]
    fn required_0の装備条件はチェックを生成しない() {
        // 表で複合列が "-" のコンテンツ(リンゴ等): 複合スキルのキャラには条件なし
        let e = evaluate_content(
            &content(
                None,
                vec![ContentRequirement::EquipmentBySkill {
                    single: 800,
                    mr: 980,
                    composite: 0,
                }],
            ),
            None,
            &EquipmentValues::default(),
            Awakening::default(),
            Some(SkillDependency::StabHack),
            0,
        );
        assert!(e.checks.is_empty());
        assert!(e.entry_ok);
    }

    #[test]
    fn スキル未収録はダメージ不明としてクリア不可() {
        let e = evaluate_content(
            &content(Some(1), vec![]),
            None,
            &EquipmentValues::default(),
            Awakening::default(),
            None,
            0,
        );
        assert!(e.entry_ok);
        assert!(!e.reaches_need && !e.clear);
        assert!(e.damage.is_none());
    }

    #[test]
    fn テシスコア条件は合計値で判定する() {
        let req = ContentRequirement::ThesisCoreTotal(120);
        let eq = EquipmentValues::default();
        let aw = Awakening::default();

        let c = req.check(&eq, aw, None, 119);
        assert!(!c.ok);
        assert_eq!((c.current, c.required), (119, 120));
        assert!(req.check(&eq, aw, None, 120).ok);

        // required 0(条件なし)はチェックを生成しない
        let e = evaluate_content(
            &content(None, vec![ContentRequirement::ThesisCoreTotal(0)]),
            None,
            &eq,
            aw,
            None,
            0,
        );
        assert!(e.checks.is_empty());
        assert!(e.entry_ok);
    }
}
