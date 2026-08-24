//! 装備補正(wiki: カテゴリA の内訳「装備攻撃力」)。docs/damage-formula.md §4 A、§5(武器強化)。
//!
//! 装備は部位別(12 スロット)で持つ(docs/claude/goals/2026-08-24-equipment-parts.md)。
//! 「基本能力値」= 部位ごとの実測補正値 + 武器アビリティの加算。
//! 「強化能力値」= 部位ごとのエンチャント値の合計。

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 装備補正 4 種(突き/斬り/魔攻/魔防)。基本能力値・エンチャント値のどちらも同じ形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EquipmentValues {
    #[serde(default)]
    pub thrust: i64,
    #[serde(default)]
    pub slash: i64,
    #[serde(default)]
    pub magic_attack: i64,
    #[serde(default)]
    pub magic_defense: i64,
}

/// 装備補正 4 値の値域上限(wiki に明記なし。実用上の安全域として暫定採用)`[仮]`。
pub const EQUIPMENT_VALUE_MAX: i64 = 9999;
/// ストロングウェポンの Lv 上限(wiki Skill/共通: Lv1〜6)。
pub const STRONG_WEAPON_LEVEL_MAX: u8 = 6;
/// 装備強化の Lv 上限(wiki: 装備システム/装備強化。+1〜+15)。
pub const ENHANCE_LEVEL_MAX: u8 = 15;
/// +12 以上で追加固定ダメージがレンジ振り(MR)になる境界(wiki: +11 覚醒までは確定値)。
pub const ENHANCE_LEVEL_RANDOM_RANGE_MIN: u8 = 12;
/// +12 以上の追加固定ダメージ実測値の上限(wiki に明記なし。+15 最上位帯でも数百万に収まる
/// 実用上の安全域として暫定採用)`[仮]`。
pub const ENHANCE_ADDED_DAMAGE_MAX: i64 = 9_999_999;

impl EquipmentValues {
    fn validate(&self) -> Result<(), EquipmentError> {
        for (field, value) in [
            ("突き攻撃力", self.thrust),
            ("斬り攻撃力", self.slash),
            ("魔法攻撃力", self.magic_attack),
            ("魔法防御力", self.magic_defense),
        ] {
            if !(0..=EQUIPMENT_VALUE_MAX).contains(&value) {
                return Err(EquipmentError::ValueOutOfRange { field, value, max: EQUIPMENT_VALUE_MAX });
            }
        }
        Ok(())
    }

    fn add(self, other: EquipmentValues) -> EquipmentValues {
        EquipmentValues {
            thrust: self.thrust + other.thrust,
            slash: self.slash + other.slash,
            magic_attack: self.magic_attack + other.magic_attack,
            magic_defense: self.magic_defense + other.magic_defense,
        }
    }
}

/// 装備部位(wiki: 装備システム ページ冒頭の表。9 部位 + 効果/AF/レリック)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartSlot {
    Weapon,
    Armor,
    Helm,
    Shield,
    ShieldPlus,
    Head,
    Body,
    Hand,
    Leg,
    Effect,
    Artifact,
    Relic,
}

impl PartSlot {
    /// この部位が装備強化(+1〜+15)を持てるか(wiki: 装備システム/装備強化。武器・鎧のみ)。
    pub fn allows_enhance(self) -> bool {
        matches!(self, PartSlot::Weapon | PartSlot::Armor)
    }

    /// この部位が武器アビリティを持てるか(wiki: 装備システム/アビリティ。武器のみが火力に効く)。
    pub fn allows_abilities(self) -> bool {
        matches!(self, PartSlot::Weapon)
    }
}

/// 装備部位 1 つ。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentPart {
    /// gamedata カタログ参照(`EquipmentItem::id`)。`None` = 未装備またはカスタム
    #[serde(default)]
    pub item_id: Option<String>,
    /// カタログ外アイテムの表示名 `[仮]`
    #[serde(default)]
    pub custom_name: Option<String>,
    /// 実測の基本能力値(カタログ選択時は UI がレンジ中央を既定セットし、MR 個人差は上書きする)
    #[serde(default)]
    pub base: EquipmentValues,
    /// エンチャント値(強化能力値)
    #[serde(default)]
    pub enchant: EquipmentValues,
    /// 装備強化 Lv(0..=15)。武器・鎧以外は 0 のみ許可
    #[serde(default)]
    pub enhance_level: u8,
    /// +12 以上の追加固定ダメージ実測値の上書き。+11 以下は式で確定するため `None` 固定
    #[serde(default)]
    pub enhance_added_damage: Option<i64>,
    /// 装備アビリティ id(武器のみ非空を許可)
    #[serde(default)]
    pub abilities: Vec<String>,
}

impl EquipmentPart {
    fn validate(&self, slot: PartSlot) -> Result<(), EquipmentError> {
        self.base.validate()?;
        self.enchant.validate()?;
        if self.enhance_level > ENHANCE_LEVEL_MAX {
            return Err(EquipmentError::EnhanceLevelOutOfRange {
                slot,
                value: self.enhance_level,
                max: ENHANCE_LEVEL_MAX,
            });
        }
        if self.enhance_level > 0 && !slot.allows_enhance() {
            return Err(EquipmentError::EnhanceNotAllowed { slot });
        }
        if self.enhance_added_damage.is_some() && self.enhance_level < ENHANCE_LEVEL_RANDOM_RANGE_MIN {
            return Err(EquipmentError::EnhanceAddedDamageNotAllowed { slot, enhance_level: self.enhance_level });
        }
        if let Some(added) = self.enhance_added_damage {
            if !(0..=ENHANCE_ADDED_DAMAGE_MAX).contains(&added) {
                return Err(EquipmentError::EnhanceAddedDamageOutOfRange {
                    slot,
                    value: added,
                    max: ENHANCE_ADDED_DAMAGE_MAX,
                });
            }
        }
        if !self.abilities.is_empty() && !slot.allows_abilities() {
            return Err(EquipmentError::AbilitiesNotAllowed { slot });
        }
        Ok(())
    }
}

/// 装備補正の値域・部位制約違反。
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum EquipmentError {
    #[error("装備補正の{field}は 0〜{max} の範囲で指定してください(指定値 {value})")]
    ValueOutOfRange { field: &'static str, value: i64, max: i64 },
    #[error("ストロングウェポンの Lv は 0〜{max} です(指定値 {value})")]
    StrongWeaponLevelOutOfRange { value: u8, max: u8 },
    #[error("{slot:?} の装備強化 Lv は 0〜{max} です(指定値 {value})")]
    EnhanceLevelOutOfRange { slot: PartSlot, value: u8, max: u8 },
    #[error("{slot:?} は装備強化の対象外です(武器・鎧のみ)")]
    EnhanceNotAllowed { slot: PartSlot },
    #[error("{slot:?} の追加固定ダメージ上書きは強化 Lv {enhance_level} では指定できません(+12 以上のみ)")]
    EnhanceAddedDamageNotAllowed { slot: PartSlot, enhance_level: u8 },
    #[error("{slot:?} の追加固定ダメージは 0〜{max} の範囲で指定してください(指定値 {value})")]
    EnhanceAddedDamageOutOfRange { slot: PartSlot, value: i64, max: i64 },
    #[error("{slot:?} は装備アビリティの対象外です(武器のみ)")]
    AbilitiesNotAllowed { slot: PartSlot },
}

/// 武器アビリティ定義(gamedata がカタログを持つ。domain の `BuffDefinition` と同じ依存方向)。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EquipmentAbilityDef {
    pub id: &'static str,
    pub name: &'static str,
    /// 装備攻撃力(基本能力値)への加算値
    pub values: EquipmentValues,
}

/// キャラの装備補正一式(部位別装備 12 スロット + パワーウェポン/ストロングウェポン)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Equipment {
    #[serde(default)]
    pub parts: EquipmentParts,
    /// パワーウェポン(wiki Skill/共通: 自身の装備補正を2%増加。Lv1 のみ、ストロングウェポンと重複可)
    #[serde(default)]
    pub power_weapon: bool,
    /// ストロングウェポンの Lv(0 = 未使用、1〜6 = 該当 Lv。wiki Skill/共通: 3/6/9/12/15/18%)
    #[serde(default)]
    pub strong_weapon_level: u8,
}

/// 12 部位。named field で持つ(`parts.weapon` 等)。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentParts {
    #[serde(default)]
    pub weapon: EquipmentPart,
    #[serde(default)]
    pub armor: EquipmentPart,
    #[serde(default)]
    pub helm: EquipmentPart,
    #[serde(default)]
    pub shield: EquipmentPart,
    #[serde(default)]
    pub shield_plus: EquipmentPart,
    #[serde(default)]
    pub head: EquipmentPart,
    #[serde(default)]
    pub body: EquipmentPart,
    #[serde(default)]
    pub hand: EquipmentPart,
    #[serde(default)]
    pub leg: EquipmentPart,
    #[serde(default)]
    pub effect: EquipmentPart,
    #[serde(default)]
    pub artifact: EquipmentPart,
    #[serde(default)]
    pub relic: EquipmentPart,
}

impl EquipmentParts {
    /// 12 部位を `(PartSlot, &EquipmentPart)` で列挙する。
    pub fn iter(&self) -> [(PartSlot, &EquipmentPart); 12] {
        [
            (PartSlot::Weapon, &self.weapon),
            (PartSlot::Armor, &self.armor),
            (PartSlot::Helm, &self.helm),
            (PartSlot::Shield, &self.shield),
            (PartSlot::ShieldPlus, &self.shield_plus),
            (PartSlot::Head, &self.head),
            (PartSlot::Body, &self.body),
            (PartSlot::Hand, &self.hand),
            (PartSlot::Leg, &self.leg),
            (PartSlot::Effect, &self.effect),
            (PartSlot::Artifact, &self.artifact),
            (PartSlot::Relic, &self.relic),
        ]
    }
}

impl Equipment {
    pub fn validate(&self) -> Result<(), EquipmentError> {
        for (slot, part) in self.parts.iter() {
            part.validate(slot)?;
        }
        if self.strong_weapon_level > STRONG_WEAPON_LEVEL_MAX {
            return Err(EquipmentError::StrongWeaponLevelOutOfRange {
                value: self.strong_weapon_level,
                max: STRONG_WEAPON_LEVEL_MAX,
            });
        }
        Ok(())
    }

    /// 基本能力値の合計(Σ part.base + Σ 武器アビリティの加算値)。
    pub fn base_totals(&self, abilities: &[EquipmentAbilityDef]) -> EquipmentValues {
        let mut total = EquipmentValues::default();
        for (_, part) in self.parts.iter() {
            total = total.add(part.base);
        }
        for ability_id in &self.parts.weapon.abilities {
            if let Some(def) = abilities.iter().find(|a| a.id == *ability_id) {
                total = total.add(def.values);
            }
        }
        total
    }

    /// 強化能力値の合計(Σ part.enchant)。
    pub fn enhanced_totals(&self) -> EquipmentValues {
        let mut total = EquipmentValues::default();
        for (_, part) in self.parts.iter() {
            total = total.add(part.enchant);
        }
        total
    }

    /// 装備攻撃力強化倍率(wiki: カテゴリA の内訳)。
    /// パワーウェポン(+2%)+ ストロングウェポン Lv × 3%。
    pub fn enhance_rate(&self) -> f64 {
        let power_weapon_rate = if self.power_weapon { 0.02 } else { 0.0 };
        power_weapon_rate + f64::from(self.strong_weapon_level) * 0.03
    }
}

/// 装備補正 4 種それぞれに掛かる係数(基本/強化のどちらか片方)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentRates {
    pub thrust: f64,
    pub slash: f64,
    pub magic_attack: f64,
    pub magic_defense: f64,
}

/// 装備攻撃力の係数(基本能力値用/強化能力値用)。スキル依存種別ごとに gamedata が持つ。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EquipmentCoefficients {
    pub base: EquipmentRates,
    pub enhanced: EquipmentRates,
}

/// 装備攻撃力(wiki: カテゴリA の内訳)。`Σ(基本値 × 基本係数) + Σ(強化値 × 強化係数)`。
/// `base`/`enhanced` は呼び出し側が `Equipment::base_totals`/`enhanced_totals` で集計して渡す。
pub fn equipment_attack_power(
    base: &EquipmentValues,
    enhanced: &EquipmentValues,
    c: &EquipmentCoefficients,
) -> f64 {
    base.thrust as f64 * c.base.thrust
        + base.slash as f64 * c.base.slash
        + base.magic_attack as f64 * c.base.magic_attack
        + base.magic_defense as f64 * c.base.magic_defense
        + enhanced.thrust as f64 * c.enhanced.thrust
        + enhanced.slash as f64 * c.enhanced.slash
        + enhanced.magic_attack as f64 * c.enhanced.magic_attack
        + enhanced.magic_defense as f64 * c.enhanced.magic_defense
}

/// 武器系統ごとの強化補正一次式の係数(wiki: 装備システム/装備強化)。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct EnhanceRates {
    pub thrust: f64,
    pub slash: f64,
    pub magic_attack: f64,
    pub magic_defense: f64,
}

/// 武器の追加固定ダメージ(wiki: 装備システム/装備強化、docs/damage-formula.md §5。与ダメージ式の外)。
///
/// `補正 = 突き×r.thrust + 斬り×r.slash + 魔攻×r.magic_attack + 魔防×r.magic_defense`
/// (アビリティによる補正は含めない = `weapon_base` は part.base の実測値そのもの)。
/// `追加効果 = INT(INT(補正) × 倍率)`。結果が奇数なら −1。
pub fn weapon_added_damage(weapon_base: &EquipmentValues, rates: &EnhanceRates, multiplier: f64) -> i64 {
    let correction = weapon_base.thrust as f64 * rates.thrust
        + weapon_base.slash as f64 * rates.slash
        + weapon_base.magic_attack as f64 * rates.magic_attack
        + weapon_base.magic_defense as f64 * rates.magic_defense;
    let inner = correction.trunc();
    let added = (inner * multiplier).trunc() as i64;
    if added % 2 != 0 {
        added - 1
    } else {
        added
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coefficients() -> EquipmentCoefficients {
        EquipmentCoefficients {
            base: EquipmentRates { thrust: 14.5, slash: 14.5, magic_attack: 0.0, magic_defense: 0.0 },
            enhanced: EquipmentRates { thrust: 28.75, slash: 28.75, magic_attack: 0.0, magic_defense: 0.0 },
        }
    }

    fn equipment_with(weapon_base: EquipmentValues, weapon_enchant: EquipmentValues) -> Equipment {
        Equipment {
            parts: EquipmentParts {
                weapon: EquipmentPart { base: weapon_base, enchant: weapon_enchant, ..Default::default() },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn 装備攻撃力は基本と強化の合計() {
        let eq = equipment_with(
            EquipmentValues { thrust: 150, slash: 150, ..Default::default() },
            EquipmentValues { thrust: 60, slash: 60, ..Default::default() },
        );
        let base = eq.base_totals(&[]);
        let enhanced = eq.enhanced_totals();
        // 150*14.5*2 + 60*28.75*2 = 4350 + 3450 = 7800
        assert!((equipment_attack_power(&base, &enhanced, &coefficients()) - 7800.0).abs() < 1e-9);
    }

    #[test]
    fn 装備なしなら装備攻撃力は0() {
        let eq = Equipment::default();
        let base = eq.base_totals(&[]);
        let enhanced = eq.enhanced_totals();
        assert_eq!(equipment_attack_power(&base, &enhanced, &coefficients()), 0.0);
    }

    #[test]
    fn base_totalsはアビリティ込み_enchantedはenchant側() {
        let mut eq = equipment_with(
            EquipmentValues { thrust: 100, slash: 200, ..Default::default() },
            EquipmentValues { thrust: 10, slash: 20, ..Default::default() },
        );
        eq.parts.weapon.abilities = vec!["sharp-blade-e".to_string()];
        eq.parts.armor.base = EquipmentValues { magic_defense: 50, ..Default::default() };

        let abilities = vec![EquipmentAbilityDef {
            id: "sharp-blade-e",
            name: "E-鋭い刃",
            values: EquipmentValues { slash: 9, ..Default::default() },
        }];
        let base = eq.base_totals(&abilities);
        assert_eq!(base, EquipmentValues { thrust: 100, slash: 209, magic_defense: 50, ..Default::default() });

        let enhanced = eq.enhanced_totals();
        assert_eq!(enhanced, EquipmentValues { thrust: 10, slash: 20, ..Default::default() });
    }

    #[test]
    fn 強化倍率はパワーウェポンとストロングウェポンの合計() {
        assert_eq!(Equipment::default().enhance_rate(), 0.0);
        let pw = Equipment { power_weapon: true, ..Default::default() };
        assert!((pw.enhance_rate() - 0.02).abs() < 1e-12);
        let sw6 = Equipment { strong_weapon_level: 6, ..Default::default() };
        assert!((sw6.enhance_rate() - 0.18).abs() < 1e-12);
        let both = Equipment { power_weapon: true, strong_weapon_level: 6, ..Default::default() };
        assert!((both.enhance_rate() - 0.20).abs() < 1e-12);
    }

    #[test]
    fn 値域違反は拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.base.thrust = EQUIPMENT_VALUE_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::ValueOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.parts.weapon.enchant.magic_defense = -1;
        assert!(matches!(eq.validate(), Err(EquipmentError::ValueOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.strong_weapon_level = STRONG_WEAPON_LEVEL_MAX + 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::StrongWeaponLevelOutOfRange { .. })));

        let mut eq = Equipment::default();
        eq.parts.weapon.base.thrust = EQUIPMENT_VALUE_MAX;
        eq.strong_weapon_level = STRONG_WEAPON_LEVEL_MAX;
        assert!(eq.validate().is_ok());
    }

    #[test]
    fn 武器以外の強化レベルは拒否する() {
        let mut eq = Equipment::default();
        eq.parts.helm.enhance_level = 1;
        assert!(matches!(eq.validate(), Err(EquipmentError::EnhanceNotAllowed { slot: PartSlot::Helm })));

        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = ENHANCE_LEVEL_MAX;
        assert!(eq.validate().is_ok());
        let mut eq2 = Equipment::default();
        eq2.parts.armor.enhance_level = ENHANCE_LEVEL_MAX;
        assert!(eq2.validate().is_ok());

        let mut over = Equipment::default();
        over.parts.weapon.enhance_level = ENHANCE_LEVEL_MAX + 1;
        assert!(matches!(over.validate(), Err(EquipmentError::EnhanceLevelOutOfRange { .. })));
    }

    #[test]
    fn 強化11以下でのadded_damage上書きは拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = 11;
        eq.parts.weapon.enhance_added_damage = Some(100);
        assert!(matches!(
            eq.validate(),
            Err(EquipmentError::EnhanceAddedDamageNotAllowed { slot: PartSlot::Weapon, .. })
        ));

        let mut ok = Equipment::default();
        ok.parts.weapon.enhance_level = 12;
        ok.parts.weapon.enhance_added_damage = Some(140);
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn 武器以外のアビリティは拒否する() {
        let mut eq = Equipment::default();
        eq.parts.helm.abilities = vec!["sharp-blade-e".to_string()];
        assert!(matches!(eq.validate(), Err(EquipmentError::AbilitiesNotAllowed { slot: PartSlot::Helm })));

        let mut ok = Equipment::default();
        ok.parts.weapon.abilities = vec!["sharp-blade-e".to_string()];
        assert!(ok.validate().is_ok());
    }

    // wiki 例(goal 文書): HACK系(斬り×6.67 + 突き×1.00)・突100/斬300
    // → INT(300×6.67+100×1.00) = INT(2001+100) = INT(2101) = 2101
    // +10 倍率 28.8 → INT(2101×28.8) = INT(60508.8) = 60508(偶数なのでそのまま)
    #[test]
    fn 武器追加固定ダメージ_hack系の式() {
        let rates = EnhanceRates { thrust: 1.00, slash: 6.67, magic_attack: 0.0, magic_defense: 0.0 };
        let weapon = EquipmentValues { thrust: 100, slash: 300, ..Default::default() };
        assert_eq!(weapon_added_damage(&weapon, &rates, 28.8), 60508);
    }

    #[test]
    fn 武器追加固定ダメージ_奇数なら1引く() {
        // 補正 = 101(突き×1.0)、倍率 1.0(+2 相当) → INT(101×1.0) = 101(奇数) → 100
        let rates = EnhanceRates { thrust: 1.0, slash: 0.0, magic_attack: 0.0, magic_defense: 0.0 };
        let weapon = EquipmentValues { thrust: 101, ..Default::default() };
        assert_eq!(weapon_added_damage(&weapon, &rates, 1.0), 100);
    }

    #[test]
    fn 追加固定ダメージ上書きの値域違反は拒否する() {
        let mut eq = Equipment::default();
        eq.parts.weapon.enhance_level = 12;
        eq.parts.weapon.enhance_added_damage = Some(-1);
        assert!(matches!(eq.validate(), Err(EquipmentError::EnhanceAddedDamageOutOfRange { .. })));

        eq.parts.weapon.enhance_added_damage = Some(ENHANCE_ADDED_DAMAGE_MAX + 1);
        assert!(matches!(eq.validate(), Err(EquipmentError::EnhanceAddedDamageOutOfRange { .. })));

        eq.parts.weapon.enhance_added_damage = Some(ENHANCE_ADDED_DAMAGE_MAX);
        assert!(eq.validate().is_ok());
    }
}
