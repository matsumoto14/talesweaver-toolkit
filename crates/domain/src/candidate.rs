//! 「次に変えるなら / おすすめ強化」候補の列挙・試算後の並び順(docs/claude/goals 2026-08-29)。
//!
//! 候補の列挙(どんな変更が候補になるか)と、試算結果の並び順(「届かせるなら」に正直に)を
//! ここに置く。実際のダメージ試算(preview_damage 相当)は呼び出し側(`commands` crate。
//! gamedata カタログを知るレイヤー)が行い、結果だけ `rank_candidates` に渡す。
//! 候補が gamedata カタログ固有の選定(武器の上位品探し)を要るときだけ、その 1 候補は
//! 呼び出し側で組み立てる(`domain` は gamedata に依存できない。`EquipmentCatalogEntry` と
//! 同じ依存方向)。

use serde::{Deserialize, Serialize};

use crate::common_skill::{
    CommonSkills, POWER_WEAPON_RATE, SHARPNESS_VISION, SHARPNESS_VISION_LEVEL_MAX,
    STRONG_WEAPON_LEVEL_MAX, STRONG_WEAPON_RATE_PER_LEVEL,
};
use crate::equipment::{
    EquipmentStatKind, Equipment, EquipmentCoefficients, EquipmentEnhanceType, EquipmentValues, EnhanceGrade,
    PartSlot, ENHANCE_LEVEL_MAX, ENHANCE_LEVEL_RANDOM_RANGE_MIN,
};
use crate::rounding::round_int;
use crate::siena::SIENA_STAGE_MAX;

/// 手間タグ。UI の表示専用(判定・計算には使わない)。新種別を足すときは既存と同格の
/// 「手間の大きさ」を表す言葉にする。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateCost {
    /// 保存済みの設定を ON にする・段を上げるだけ(共通スキルのトグル等)。手間がほぼ無い
    QuickWin,
    /// 装備のエンチャントを伸ばす
    Enchant,
    /// 装備そのものを差し替える
    EquipmentUpdate,
    /// 装備強化の Lv を上げる
    Enhance,
    /// シエナのオーラを増幅する
    Aura,
}

/// 候補 1 件ぶんの変更(適用済み)。呼び出し側はこれをそのまま元キャラの
/// `equipment`/`common_skills` に差し替えて試算する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateChange {
    pub id: String,
    pub label: String,
    pub cost: CandidateCost,
    pub equipment: Equipment,
    pub common_skills: CommonSkills,
}

/// パワーウェポン ON / ストロングウェポン上限 / シャープネスビジョン上限。装備には触れない。
pub fn quick_win_candidates(equipment: &Equipment, common_skills: &CommonSkills) -> Vec<CandidateChange> {
    let mut out = Vec::new();
    if !common_skills.power_weapon {
        let mut skills = *common_skills;
        skills.power_weapon = true;
        out.push(CandidateChange {
            id: "pw".to_string(),
            label: format!("パワーウェポンを ON に(装備攻撃力強化 +{}%)", round_int(POWER_WEAPON_RATE * 100.0)),
            cost: CandidateCost::QuickWin,
            equipment: equipment.clone(),
            common_skills: skills,
        });
    }
    if common_skills.strong_weapon_level < STRONG_WEAPON_LEVEL_MAX {
        let mut skills = *common_skills;
        skills.strong_weapon_level = STRONG_WEAPON_LEVEL_MAX;
        // Lv2 以降はオーグメントの Lv が要る(wiki Skill/共通)
        skills.augment_level = skills.augment_level.max(STRONG_WEAPON_LEVEL_MAX - 1);
        let rate = f64::from(STRONG_WEAPON_LEVEL_MAX) * STRONG_WEAPON_RATE_PER_LEVEL;
        out.push(CandidateChange {
            id: "sw".to_string(),
            label: format!(
                "ストロングウェポンを Lv{STRONG_WEAPON_LEVEL_MAX} に(装備攻撃力強化 +{}%)",
                round_int(rate * 100.0)
            ),
            cost: CandidateCost::QuickWin,
            equipment: equipment.clone(),
            common_skills: skills,
        });
    }
    if common_skills.sharpness_vision_level < SHARPNESS_VISION_LEVEL_MAX {
        let mut skills = *common_skills;
        skills.sharpness_vision_level = SHARPNESS_VISION_LEVEL_MAX;
        // 割合追加ダメージ(§5 新-割合)。**表記ダメージ(1 段)は動かず合計ダメージだけ増える**ので、
        // この候補の効きは `rank_candidates` の合計側(`delta_total_pct`)にしか出ない
        let rate = SHARPNESS_VISION[SHARPNESS_VISION_LEVEL_MAX as usize - 1];
        out.push(CandidateChange {
            id: "sv".to_string(),
            label: format!(
                "シャープネスビジョンを Lv{SHARPNESS_VISION_LEVEL_MAX} に(割合追加ダメージ +{}%)",
                round_int(rate * 100.0)
            ),
            cost: CandidateCost::QuickWin,
            equipment: equipment.clone(),
            common_skills: skills,
        });
    }
    out
}

/// 部位別の装備強化 +1(武器・鎧のみ。他部位は装備強化の対象外)。
/// `weapon_enhance_type` / `armor_enhance_type` は呼び出し側が解決した強化補正種別
/// (`part.enhance_type` が無ければカタログの item_id から引く。commands.rs の
/// `weapon_added_damage`/`armor_added_hp` と同じ解決順)。`None` なら種別が決められず
/// 強化 Lv を上げられないので候補を出さない。
pub fn enhance_candidates(
    equipment: &Equipment,
    common_skills: &CommonSkills,
    weapon_enhance_type: Option<EquipmentEnhanceType>,
    armor_enhance_type: Option<EquipmentEnhanceType>,
) -> Vec<CandidateChange> {
    let mut out = Vec::new();
    for (slot, resolved_type) in [
        (PartSlot::Weapon, weapon_enhance_type),
        (PartSlot::Armor, armor_enhance_type),
    ] {
        let Some(part) = equipment.parts.get(slot).selected() else {
            continue;
        };
        let Some(kind) = resolved_type else {
            continue;
        };
        if part.enhance_level >= ENHANCE_LEVEL_MAX {
            continue;
        }
        let new_level = part.enhance_level + 1;
        let mut eq = equipment.clone();
        let mut_part = eq
            .parts
            .get_mut(slot)
            .selected_mut()
            .expect("selected part exists (checked above)");
        mut_part.enhance_level = new_level;
        mut_part.enhance_type = Some(kind);
        if new_level >= ENHANCE_LEVEL_RANDOM_RANGE_MIN {
            // UI(EquipmentPane)の既定と同じく等級上端を使う
            mut_part.enhance_grade = Some(mut_part.enhance_grade.unwrap_or(EnhanceGrade::Highest));
        }
        out.push(CandidateChange {
            id: format!("enhance-{slot:?}").to_lowercase(),
            label: format!("{}の強化 +{} → +{new_level}", slot.label(), part.enhance_level),
            cost: CandidateCost::Enhance,
            equipment: eq,
            common_skills: *common_skills,
        });
    }
    out
}

/// エンチャントの候補にする 4 種(与ダメージ式に入る値だけ。命中/Cri補正/回避/敏捷は
/// この一覧の目的の外なので出さない)。
pub const ENCHANT_CANDIDATE_STATS: [EquipmentStatKind; 4] = [
    EquipmentStatKind::Thrust,
    EquipmentStatKind::Slash,
    EquipmentStatKind::MagicAttack,
    EquipmentStatKind::MagicDefense,
];

/// スキル依存種別(`SkillDependency`)が実際に装備攻撃力へ効かせる装備値(装備攻撃力係数が
/// 基本/強化のどちらかで非 0 のもの)。エンチャント候補をこれに絞るのに使う。
/// ゲームのルール表(依存種別 → 見るステ)を UI 側に持たせず、装備攻撃力係数(gamedata)という
/// 既存の唯一の正から引く。
pub fn enchant_dependency_keys(coefficients: &EquipmentCoefficients) -> Vec<EquipmentStatKind> {
    ENCHANT_CANDIDATE_STATS
        .into_iter()
        .filter(|&kind| coefficients.base.get(kind) != 0.0 || coefficients.enhanced.get(kind) != 0.0)
        .collect()
}

/// 部位・ステごとに「上限まで積んだ場合」を候補にする(ユーザー指摘: 中途半端な刻みは
/// 効果がほぼ無く、`rank_candidates` の順位も意味を持たない。上限まで一気に進める)。
/// `caps` は呼び出し側がカタログから引いた部位ごとのエンチャント上限
/// (カタログ item が付いている部位だけ渡ってくる)。`allowed_keys` は候補にするステ
/// (`enchant_dependency_keys` で絞ったもの。空なら絞らず 4 種すべてを候補にする)。
pub fn enchant_candidates(
    equipment: &Equipment,
    common_skills: &CommonSkills,
    caps: &[(PartSlot, EquipmentValues)],
    allowed_keys: &[EquipmentStatKind],
) -> Vec<CandidateChange> {
    let mut out = Vec::new();
    for &(slot, cap) in caps {
        let Some(part) = equipment.parts.get(slot).selected() else {
            continue;
        };
        for kind in ENCHANT_CANDIDATE_STATS {
            if !allowed_keys.is_empty() && !allowed_keys.contains(&kind) {
                continue;
            }
            let current = part.enchant.get(kind);
            let max = cap.get(kind);
            if current >= max {
                continue;
            }
            let mut eq = equipment.clone();
            let mut_part = eq
                .parts
                .get_mut(slot)
                .selected_mut()
                .expect("selected part exists (checked above)");
            *mut_part.enchant.get_mut(kind) = max;
            out.push(CandidateChange {
                id: format!("enchant-{slot:?}-{}", kind.key()).to_lowercase(),
                label: format!(
                    "{}のエンチャント {} +{}({current} → {max} 上限)",
                    slot.label(),
                    kind.label(),
                    max - current,
                ),
                cost: CandidateCost::Enchant,
                equipment: eq,
                common_skills: *common_skills,
            });
        }
    }
    out
}

/// 装着中オーラがある部位ごとに増幅段階 +1。追加スロットの中身は選べないので、
/// 既存スロットのうち最大値のものと同種・同値で試算する(label に前提を書く)。
pub fn aura_candidates(equipment: &Equipment, common_skills: &CommonSkills) -> Vec<CandidateChange> {
    let mut out = Vec::new();
    for (slot, aura) in equipment.siena.iter_selected() {
        if aura.stage() >= SIENA_STAGE_MAX {
            continue;
        }
        let Some(seed) = aura.slots.iter().max_by_key(|s| s.value).copied() else {
            continue;
        };
        let old_stage = aura.stage();
        let mut eq = equipment.clone();
        let entry = eq
            .siena
            .get_mut(slot)
            .expect("allowed siena slot")
            .selected_mut()
            .expect("selected aura exists (checked above)");
        entry.aura.slots.push(seed);
        let new_stage = entry.aura.stage();
        out.push(CandidateChange {
            id: format!("aura-{slot:?}").to_lowercase(),
            label: format!(
                "{}のオーラ {old_stage} → {new_stage}段階({}で試算)",
                slot.label(),
                seed.kind.label()
            ),
            cost: CandidateCost::Aura,
            equipment: eq,
            common_skills: *common_skills,
        });
    }
    out
}

/// 装備・共通スキルだけで決まる候補をまとめて列挙する(gamedata カタログの上位品探しを要る
/// 「装備更新」候補は含まない。呼び出し側が別途組み立てて追加する)。
pub fn list_candidate_changes(
    equipment: &Equipment,
    common_skills: &CommonSkills,
    weapon_enhance_type: Option<EquipmentEnhanceType>,
    armor_enhance_type: Option<EquipmentEnhanceType>,
    enchant_caps: &[(PartSlot, EquipmentValues)],
    enchant_allowed_keys: &[EquipmentStatKind],
) -> Vec<CandidateChange> {
    let mut out = quick_win_candidates(equipment, common_skills);
    out.extend(enhance_candidates(
        equipment,
        common_skills,
        weapon_enhance_type,
        armor_enhance_type,
    ));
    out.extend(enchant_candidates(
        equipment,
        common_skills,
        enchant_caps,
        enchant_allowed_keys,
    ));
    out.extend(aura_candidates(equipment, common_skills));
    out
}

/// 候補 1 件の試算結果。**2 本立て**で持つ:
/// - `per_hit_primary`: 表記ダメージ(1 段)。ゲームが出す数字で、コンテンツ到達判定の基準
/// - `total_primary`: 実際に敵へ入る総量(表記 × 段数 + 武器強化の追加固定 + 割合追加)
///
/// シャープネスビジョンや武器強化のように**表記は動かさず総量だけ増やす**ものがあるので、
/// 片方だけでは「効いているのに 0% と出る」候補が生まれる(docs/damage-formula.md §5)。
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateOutcome {
    pub id: String,
    pub per_hit_primary: i64,
    pub total_primary: i64,
}

/// 試算後の候補 1 件(並び替え済み)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedCandidate {
    pub id: String,
    pub per_hit_primary: i64,
    pub total_primary: i64,
    /// 表記ダメージ(1 段)の伸び率。ユーザーがふだん見ている数字
    pub delta_pct: i32,
    /// 実際に敵へ入る総量の伸び率。表記が動かない候補はこちらにだけ出る
    pub delta_total_pct: i32,
    /// 必要 /hit 以上か。`need_per_hit` が無いコンテンツでは常に `false`。
    pub reaches: bool,
}

/// 試算結果を「届かせるなら」に正直な順へ並べる。
///
/// - **表記も総量も現状を超えない**候補は除外する。表記が動かず総量だけ増えるもの
///   (シャープネスビジョン・武器強化)は残る。
/// - `need_per_hit` があるとき: 届く候補のうち増分が最小のものを先頭に固定し、残りは
///   `per_hit_primary` 降順(同値は `total_primary` 降順)。届く候補が無ければ全体を同じ順に並べる。
/// - 到達判定と並び順の主キーが表記ダメージなのは、ゲームの表示とコンテンツの必要 /hit が
///   その値だから。総量は伸び率の 2 本目として添える(ユーザー判断 2026-09-01)。
pub fn rank_candidates(
    items: Vec<CandidateOutcome>,
    base_per_hit: i64,
    base_total: i64,
    need_per_hit: Option<i64>,
) -> Vec<RankedCandidate> {
    let pct = |value: i64, base: i64| -> i32 {
        if base > 0 {
            round_int(((value as f64 / base as f64) - 1.0) * 100.0) as i32
        } else {
            0
        }
    };
    let mut ranked: Vec<RankedCandidate> = items
        .into_iter()
        .map(|o| RankedCandidate {
            delta_pct: pct(o.per_hit_primary, base_per_hit),
            delta_total_pct: pct(o.total_primary, base_total),
            reaches: need_per_hit.is_some_and(|need| o.per_hit_primary >= need),
            id: o.id,
            per_hit_primary: o.per_hit_primary,
            total_primary: o.total_primary,
        })
        .filter(|r| r.per_hit_primary > base_per_hit || r.total_primary > base_total)
        .collect();

    let by_damage = |a: &RankedCandidate, b: &RankedCandidate| {
        b.per_hit_primary
            .cmp(&a.per_hit_primary)
            .then(b.total_primary.cmp(&a.total_primary))
    };
    if let Some(need) = need_per_hit {
        if let Some(pin_idx) = ranked
            .iter()
            .enumerate()
            .filter(|(_, r)| r.per_hit_primary >= need)
            .min_by_key(|(_, r)| r.per_hit_primary)
            .map(|(i, _)| i)
        {
            let pin = ranked.remove(pin_idx);
            ranked.sort_by(by_damage);
            ranked.insert(0, pin);
            return ranked;
        }
    }
    ranked.sort_by(by_damage);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::EquipmentRates;
    use crate::equipment::{EquipmentPart, EquipmentPartList};
    use crate::siena::{RegisteredSienaAura, SienaAura, SienaAuraList, SienaSlot, SienaValueKind};

    fn catalog_part(item_id: &str) -> EquipmentPart {
        EquipmentPart {
            id: 1,
            item_id: Some(item_id.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn quick_winはパワーウェポン未onとswレベル未満とsv未上限のときだけ出る() {
        let equipment = Equipment::default();
        let common_skills = CommonSkills::default();
        let candidates = quick_win_candidates(&equipment, &common_skills);
        assert_eq!(candidates.len(), 3);
        assert!(candidates.iter().any(|c| c.id == "pw" && c.common_skills.power_weapon));
        assert!(candidates.iter().any(|c| c.id == "sw"
            && c.common_skills.strong_weapon_level == STRONG_WEAPON_LEVEL_MAX
            && c.common_skills.augment_level == STRONG_WEAPON_LEVEL_MAX - 1));
        assert!(candidates.iter().any(|c| c.id == "sv"
            && c.common_skills.sharpness_vision_level == SHARPNESS_VISION_LEVEL_MAX));

        let maxed = CommonSkills {
            power_weapon: true,
            strong_weapon_level: STRONG_WEAPON_LEVEL_MAX,
            sharpness_vision_level: SHARPNESS_VISION_LEVEL_MAX,
            ..Default::default()
        };
        assert!(quick_win_candidates(&equipment, &maxed).is_empty());
    }

    #[test]
    fn 強化候補は種別が解決できないと出ない() {
        let mut equipment = Equipment::default();
        equipment.parts.weapon = EquipmentPartList::from(catalog_part("w1"));
        let common_skills = CommonSkills::default();
        assert!(enhance_candidates(&equipment, &common_skills, None, None).is_empty());

        let candidates = enhance_candidates(
            &equipment,
            &common_skills,
            Some(EquipmentEnhanceType::WeaponStab),
            None,
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "enhance-weapon");
        let part = candidates[0].equipment.parts.weapon.selected().unwrap();
        assert_eq!(part.enhance_level, 1);
        assert_eq!(part.enhance_type, Some(EquipmentEnhanceType::WeaponStab));
    }

    #[test]
    fn 強化候補はカスタム装備でも種別が解決できれば出る() {
        // item_id が無い(カタログ外)装備でも、part.enhance_type が入っていて
        // 呼び出し側の解決結果(resolved_type)が Some なら候補に上がる。
        let mut equipment = Equipment::default();
        let mut part = EquipmentPart::default();
        part.enhance_type = Some(EquipmentEnhanceType::WeaponStab);
        equipment.parts.weapon = EquipmentPartList::from(part);
        let common_skills = CommonSkills::default();
        let candidates = enhance_candidates(
            &equipment,
            &common_skills,
            Some(EquipmentEnhanceType::WeaponStab),
            None,
        );
        assert_eq!(candidates.len(), 1);
        let part = candidates[0].equipment.parts.weapon.selected().unwrap();
        assert_eq!(part.enhance_level, 1);
    }

    #[test]
    fn 強化候補は12到達で等級を自動で埋める() {
        let mut equipment = Equipment::default();
        let mut part = catalog_part("w1");
        part.enhance_level = 11;
        part.enhance_type = Some(EquipmentEnhanceType::WeaponStab);
        equipment.parts.weapon = EquipmentPartList::from(part);
        let common_skills = CommonSkills::default();
        let candidates = enhance_candidates(
            &equipment,
            &common_skills,
            Some(EquipmentEnhanceType::WeaponStab),
            None,
        );
        let applied = candidates[0].equipment.parts.weapon.selected().unwrap();
        assert_eq!(applied.enhance_level, 12);
        assert_eq!(applied.enhance_grade, Some(EnhanceGrade::Highest));
    }

    #[test]
    fn 強化候補は上限で出なくなる() {
        let mut equipment = Equipment::default();
        let mut part = catalog_part("w1");
        part.enhance_level = ENHANCE_LEVEL_MAX;
        part.enhance_type = Some(EquipmentEnhanceType::WeaponStab);
        part.enhance_grade = Some(EnhanceGrade::Highest);
        equipment.parts.weapon = EquipmentPartList::from(part);
        let common_skills = CommonSkills::default();
        assert!(enhance_candidates(
            &equipment,
            &common_skills,
            Some(EquipmentEnhanceType::WeaponStab),
            None
        )
        .is_empty());
    }

    #[test]
    fn 依存ステの絞り込みは係数が非0の2種だけ残す() {
        let coefficients = EquipmentCoefficients {
            base: EquipmentRates {
                thrust: 0.0,
                slash: 14.5,
                magic_attack: 14.5,
                magic_defense: 0.0,
            },
            enhanced: EquipmentRates {
                thrust: 0.0,
                slash: 28.75,
                magic_attack: 28.75,
                magic_defense: 0.0,
            },
        };
        let keys = enchant_dependency_keys(&coefficients);
        assert_eq!(
            keys,
            vec![EquipmentStatKind::Slash, EquipmentStatKind::MagicAttack]
        );
    }

    #[test]
    fn エンチャント候補は依存ステで絞り込める() {
        let mut equipment = Equipment::default();
        let mut part = catalog_part("w1");
        part.enchant = EquipmentValues {
            thrust: 10,
            slash: 10,
            magic_attack: 10,
            ..Default::default()
        };
        equipment.parts.weapon = EquipmentPartList::from(part);
        let common_skills = CommonSkills::default();
        let caps = [(
            PartSlot::Weapon,
            EquipmentValues {
                thrust: 100,
                slash: 100,
                magic_attack: 100,
                ..Default::default()
            },
        )];
        // 斬りだけに絞ると、突き・魔攻の候補は出ない
        let candidates = enchant_candidates(&equipment, &common_skills, &caps, &[EquipmentStatKind::Slash]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "enchant-weapon-slash");
    }

    #[test]
    fn エンチャント候補はカタログ外装備でも上限が解決できれば出る() {
        // item_id が無い(カスタム)装備でも、呼び出し側が resolve_enchant_caps で
        // 実測上限を解決していれば caps に載り、候補に上がる。
        let mut equipment = Equipment::default();
        let mut part = EquipmentPart::default();
        part.enchant = EquipmentValues {
            thrust: 10,
            ..Default::default()
        };
        equipment.parts.weapon = EquipmentPartList::from(part);
        let common_skills = CommonSkills::default();
        let caps = [(
            PartSlot::Weapon,
            EquipmentValues {
                thrust: 50,
                ..Default::default()
            },
        )];
        let candidates = enchant_candidates(&equipment, &common_skills, &caps, &[]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "enchant-weapon-thrust");
    }

    #[test]
    fn エンチャント候補はカタログ外装備で上限未解決なら出ない() {
        // caps に載らない(呼び出し側が未収録と判断した)部位は候補にしない。
        let mut equipment = Equipment::default();
        let mut part = EquipmentPart::default();
        part.enchant = EquipmentValues {
            thrust: 10,
            ..Default::default()
        };
        equipment.parts.weapon = EquipmentPartList::from(part);
        let common_skills = CommonSkills::default();
        assert!(enchant_candidates(&equipment, &common_skills, &[], &[]).is_empty());
    }

    #[test]
    fn エンチャントは上限まで一気に進む() {
        let mut equipment = Equipment::default();
        let mut part = catalog_part("w1");
        part.enchant = EquipmentValues {
            thrust: 23,
            slash: 10,
            ..Default::default()
        };
        equipment.parts.weapon = EquipmentPartList::from(part);
        let common_skills = CommonSkills::default();
        let caps = [(
            PartSlot::Weapon,
            EquipmentValues {
                thrust: 100,
                slash: 50,
                ..Default::default()
            },
        )];
        let candidates = enchant_candidates(&equipment, &common_skills, &caps, &[]);
        // 突き・斬りそれぞれ 1 候補ずつ(魔攻/魔防は現在も上限も 0 なので出ない)
        assert_eq!(candidates.len(), 2);
        let thrust = candidates.iter().find(|c| c.id == "enchant-weapon-thrust").unwrap();
        let applied = thrust.equipment.parts.weapon.selected().unwrap();
        // 上限までまとめて進み、他フィールドは変えない
        assert_eq!(applied.enchant.thrust, 100);
        assert_eq!(applied.enchant.slash, 10);
        assert!(thrust.label.contains("+77"));
        assert!(thrust.label.contains("23"));
        assert!(thrust.label.contains("100"));
        let slash = candidates.iter().find(|c| c.id == "enchant-weapon-slash").unwrap();
        assert!(slash.label.contains("+40"));
    }

    #[test]
    fn エンチャントは上限到達で候補が出ない() {
        let mut equipment = Equipment::default();
        let mut part = catalog_part("w1");
        part.enchant = EquipmentValues {
            thrust: 100,
            ..Default::default()
        };
        equipment.parts.weapon = EquipmentPartList::from(part);
        let common_skills = CommonSkills::default();
        let caps = [(
            PartSlot::Weapon,
            EquipmentValues {
                thrust: 100,
                ..Default::default()
            },
        )];
        assert!(enchant_candidates(&equipment, &common_skills, &caps, &[]).is_empty());
    }

    #[test]
    fn オーラは装着中の部位ごとに最大値スロットと同種同値を積み増す() {
        let mut equipment = Equipment::default();
        let aura = SienaAura {
            slots: vec![
                SienaSlot {
                    kind: SienaValueKind::Thrust,
                    value: 5,
                },
                SienaSlot {
                    kind: SienaValueKind::Slash,
                    value: 8,
                },
            ],
            extras: vec![],
        };
        equipment.siena.weapon = SienaAuraList {
            registered: vec![RegisteredSienaAura {
                id: 1,
                label: String::new(),
                aura,
            }],
            selected_id: Some(1),
        };
        let common_skills = CommonSkills::default();
        let candidates = aura_candidates(&equipment, &common_skills);
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].label.contains("3段階"));
        let applied_aura = candidates[0].equipment.siena.weapon.selected().unwrap().aura.clone();
        assert_eq!(applied_aura.stage(), 3);
        assert_eq!(applied_aura.slots[2].kind, SienaValueKind::Slash);
        assert_eq!(applied_aura.slots[2].value, 8);
    }

    #[test]
    fn オーラは未装着なら候補が出ない() {
        let equipment = Equipment::default();
        let common_skills = CommonSkills::default();
        assert!(aura_candidates(&equipment, &common_skills).is_empty());
    }

    #[test]
    fn 列挙は装備未登録オーラなしでもパニックしない() {
        let equipment = Equipment::default();
        let common_skills = CommonSkills::default();
        let changes = list_candidate_changes(&equipment, &common_skills, None, None, &[], &[]);
        // PW/SW/SV のクイックウィンだけは常に出る
        assert_eq!(changes.len(), 3);
    }

    /// 表記と総量が同じ比で動く候補(ふつうの攻撃力候補)。base は per_hit 100 / total 300
    fn outcome(id: &str, per_hit: i64) -> CandidateOutcome {
        CandidateOutcome {
            id: id.to_string(),
            per_hit_primary: per_hit,
            total_primary: per_hit * 3,
        }
    }

    #[test]
    fn 並び順は届く最小増分を先頭に残りは降順() {
        let items = vec![outcome("a", 120), outcome("b", 150), outcome("c", 90), outcome("d", 200)];
        // need = 130 -> a(120) は届かない、b(150)/d(200) は届く。最小増分は b。
        // c(90) は base(100) を下回る(悪化)ので除外される。
        let ranked = rank_candidates(items, 100, 300, Some(130));
        let ids: Vec<_> = ranked.iter().map(|r| r.id.as_str()).collect();
        // 届く(b, d)のうち増分最小の b が先頭固定。残りは per_hit 降順(d, a)。
        assert_eq!(ids, vec!["b", "d", "a"]);
    }

    #[test]
    fn need無しは降順のみ() {
        let items = vec![outcome("a", 120), outcome("b", 150)];
        let ranked = rank_candidates(items, 100, 300, None);
        assert_eq!(ranked[0].id, "b");
        assert_eq!(ranked[1].id, "a");
        assert!(!ranked[0].reaches && !ranked[1].reaches);
    }

    #[test]
    fn 現状比0の候補は除外する() {
        let items = vec![outcome("a", 100), outcome("b", 101)];
        // base=100: a は表記も総量も現状どまりなので除外。b は両方超えるので残す。
        let ranked = rank_candidates(items, 100, 300, None);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].id, "b");
    }

    #[test]
    fn 悪化する候補は除外する() {
        let items = vec![
            outcome("a", 90),  // 悪化(delta 負)
            outcome("b", 100), // 現状維持
            outcome("c", 110), // 改善
        ];
        let ranked = rank_candidates(items, 100, 300, None);
        let ids: Vec<_> = ranked.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["c"]);
    }

    #[test]
    fn 表記が動かず総量だけ増える候補も残り伸び率は総量側に出る() {
        // シャープネスビジョン: per_hit は現状のまま、total だけ +40%
        let items = vec![CandidateOutcome {
            id: "sv".to_string(),
            per_hit_primary: 100,
            total_primary: 420,
        }];
        let ranked = rank_candidates(items, 100, 300, Some(130));
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].delta_pct, 0);
        assert_eq!(ranked[0].delta_total_pct, 40);
        // 到達判定は表記ダメージ基準なので、総量が伸びても「届く」にはならない
        assert!(!ranked[0].reaches);
    }

    #[test]
    fn 表記も総量も動かない候補は除外する() {
        let items = vec![CandidateOutcome {
            id: "a".to_string(),
            per_hit_primary: 100,
            total_primary: 300,
        }];
        assert!(rank_candidates(items, 100, 300, None).is_empty());
    }
}
