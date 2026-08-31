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
    CommonSkills, POWER_WEAPON_RATE, STRONG_WEAPON_LEVEL_MAX, STRONG_WEAPON_RATE_PER_LEVEL,
};
use crate::equipment::{
    Equipment, EquipmentCoefficients, EquipmentEnhanceType, EquipmentRates, EquipmentValues, EnhanceGrade,
    PartSlot, ENHANCE_LEVEL_MAX, ENHANCE_LEVEL_RANDOM_RANGE_MIN,
};
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

/// パワーウェポン ON / ストロングウェポン上限。装備には触れない。
pub fn quick_win_candidates(equipment: &Equipment, common_skills: &CommonSkills) -> Vec<CandidateChange> {
    let mut out = Vec::new();
    if !common_skills.power_weapon {
        let mut skills = *common_skills;
        skills.power_weapon = true;
        out.push(CandidateChange {
            id: "pw".to_string(),
            label: format!("パワーウェポンを ON に(装備攻撃力強化 +{}%)", (POWER_WEAPON_RATE * 100.0).round() as i64),
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
                (rate * 100.0).round() as i64
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
/// この一覧の目的の外なので出さない)。`(serde キー, 表示名, getter, setter)`。
const ENCHANT_CANDIDATE_FIELDS: [(
    &str,
    &str,
    fn(&EquipmentValues) -> i64,
    fn(&mut EquipmentValues, i64),
); 4] = [
    ("thrust", EquipmentValues::THRUST_LABEL, |v| v.thrust, |v, x| v.thrust = x),
    ("slash", EquipmentValues::SLASH_LABEL, |v| v.slash, |v, x| v.slash = x),
    ("magic_attack", EquipmentValues::MAGIC_ATTACK_LABEL, |v| v.magic_attack, |v, x| {
        v.magic_attack = x
    }),
    ("magic_defense", EquipmentValues::MAGIC_DEFENSE_LABEL, |v| v.magic_defense, |v, x| {
        v.magic_defense = x
    }),
];

/// スキル依存種別(`SkillDependency`)が実際に装備攻撃力へ効かせる装備値 2 種(装備攻撃力係数が
/// 基本/強化のどちらかで非 0 のもの)。エンチャント候補をこの 2 種に絞るのに使う
/// (命中/Cri/回避/敏捷はこの一覧の目的の外なので、そもそも `ENCHANT_CANDIDATE_FIELDS` に無い)。
/// ゲームのルール表(依存種別 → 見るステ)を UI 側に持たせず、装備攻撃力係数(gamedata)という
/// 既存の唯一の正から引く。
pub fn enchant_dependency_keys(coefficients: &EquipmentCoefficients) -> Vec<&'static str> {
    ENCHANT_CANDIDATE_FIELDS
        .iter()
        .filter(|&&(key, _, _, _)| {
            let rate = |r: &EquipmentRates| match key {
                "thrust" => r.thrust,
                "slash" => r.slash,
                "magic_attack" => r.magic_attack,
                "magic_defense" => r.magic_defense,
                _ => 0.0,
            };
            rate(&coefficients.base) != 0.0 || rate(&coefficients.enhanced) != 0.0
        })
        .map(|&(key, _, _, _)| key)
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
    allowed_keys: &[&str],
) -> Vec<CandidateChange> {
    let mut out = Vec::new();
    for &(slot, cap) in caps {
        let Some(part) = equipment.parts.get(slot).selected() else {
            continue;
        };
        for &(key, label, get, set) in &ENCHANT_CANDIDATE_FIELDS {
            if !allowed_keys.is_empty() && !allowed_keys.contains(&key) {
                continue;
            }
            let current = get(&part.enchant);
            let max = get(&cap);
            if current >= max {
                continue;
            }
            let mut eq = equipment.clone();
            let mut_part = eq
                .parts
                .get_mut(slot)
                .selected_mut()
                .expect("selected part exists (checked above)");
            set(&mut mut_part.enchant, max);
            out.push(CandidateChange {
                id: format!("enchant-{slot:?}-{key}").to_lowercase(),
                label: format!(
                    "{}のエンチャント {label} +{}({current} → {max} 上限)",
                    slot.label(),
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
    enchant_allowed_keys: &[&str],
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

/// 試算後の候補 1 件(並び替え済み)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedCandidate {
    pub id: String,
    pub per_hit_primary: i64,
    pub delta_pct: i32,
    /// 必要 /hit 以上か。`need_per_hit` が無いコンテンツでは常に `false`。
    pub reaches: bool,
}

/// 試算結果 `(id, per_hit_primary)` を「届かせるなら」に正直な順へ並べる。
///
/// - 現状(`base`)を超えない(`per_hit_primary <= base`)候補は改善しない・悪化するのどちらも除外する。
/// - `need_per_hit` があるとき: 届く候補のうち増分が最小のものを先頭に固定し、残りは
///   `per_hit_primary` 降順。届く候補が無ければ全体を `per_hit_primary` 降順にする。
/// - `need_per_hit` が無いときは `per_hit_primary` 降順のみ。
pub fn rank_candidates(
    items: Vec<(String, i64)>,
    base: i64,
    need_per_hit: Option<i64>,
) -> Vec<RankedCandidate> {
    let mut ranked: Vec<RankedCandidate> = items
        .into_iter()
        .map(|(id, per_hit_primary)| {
            let delta_pct = if base > 0 {
                (((per_hit_primary as f64 / base as f64) - 1.0) * 100.0).round() as i32
            } else {
                0
            };
            let reaches = need_per_hit.is_some_and(|need| per_hit_primary >= need);
            RankedCandidate {
                id,
                per_hit_primary,
                delta_pct,
                reaches,
            }
        })
        .filter(|r| r.per_hit_primary > base)
        .collect();

    if let Some(need) = need_per_hit {
        if let Some(pin_idx) = ranked
            .iter()
            .enumerate()
            .filter(|(_, r)| r.per_hit_primary >= need)
            .min_by_key(|(_, r)| r.per_hit_primary)
            .map(|(i, _)| i)
        {
            let pin = ranked.remove(pin_idx);
            ranked.sort_by(|a, b| b.per_hit_primary.cmp(&a.per_hit_primary));
            ranked.insert(0, pin);
            return ranked;
        }
    }
    ranked.sort_by(|a, b| b.per_hit_primary.cmp(&a.per_hit_primary));
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn quick_winはパワーウェポン未onとswレベル未満のときだけ出る() {
        let equipment = Equipment::default();
        let common_skills = CommonSkills::default();
        let candidates = quick_win_candidates(&equipment, &common_skills);
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().any(|c| c.id == "pw" && c.common_skills.power_weapon));
        assert!(candidates.iter().any(|c| c.id == "sw"
            && c.common_skills.strong_weapon_level == STRONG_WEAPON_LEVEL_MAX
            && c.common_skills.augment_level == STRONG_WEAPON_LEVEL_MAX - 1));

        let maxed = CommonSkills {
            power_weapon: true,
            strong_weapon_level: STRONG_WEAPON_LEVEL_MAX,
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
        assert_eq!(keys, vec!["slash", "magic_attack"]);
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
        let candidates = enchant_candidates(&equipment, &common_skills, &caps, &["slash"]);
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
        // PW/SW のクイックウィンだけは常に出る
        assert_eq!(changes.len(), 2);
    }

    #[test]
    fn 並び順は届く最小増分を先頭に残りは降順() {
        let items = vec![
            ("a".to_string(), 120),
            ("b".to_string(), 150),
            ("c".to_string(), 90),
            ("d".to_string(), 200),
        ];
        // need = 130 -> a(120) は届かない、b(150)/d(200) は届く。最小増分は b。
        // c(90) は base(100) を下回る(悪化)ので除外される。
        let ranked = rank_candidates(items, 100, Some(130));
        let ids: Vec<_> = ranked.iter().map(|r| r.id.as_str()).collect();
        // 届く(b, d)のうち増分最小の b が先頭固定。残りは per_hit 降順(d, a)。
        assert_eq!(ids, vec!["b", "d", "a"]);
    }

    #[test]
    fn need無しは降順のみ() {
        let items = vec![
            ("a".to_string(), 120),
            ("b".to_string(), 150),
        ];
        let ranked = rank_candidates(items, 100, None);
        assert_eq!(ranked[0].id, "b");
        assert_eq!(ranked[1].id, "a");
        assert!(!ranked[0].reaches && !ranked[1].reaches);
    }

    #[test]
    fn 現状比0の候補は除外する() {
        let items = vec![("a".to_string(), 100), ("b".to_string(), 101)];
        // base=100: a は delta 0% かつ per_hit<=base なので除外。b は per_hit>base なので残す。
        let ranked = rank_candidates(items, 100, None);
        assert_eq!(ranked.len(), 1);
        assert_eq!(ranked[0].id, "b");
    }

    #[test]
    fn 悪化する候補は除外する() {
        let items = vec![
            ("a".to_string(), 90),  // 悪化(delta 負)
            ("b".to_string(), 100), // 現状維持
            ("c".to_string(), 110), // 改善
        ];
        let ranked = rank_candidates(items, 100, None);
        let ids: Vec<_> = ranked.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["c"]);
    }
}
