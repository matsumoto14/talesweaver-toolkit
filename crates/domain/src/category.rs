//! 与ダメージ式のカテゴリ(docs/damage-formula.md §4)と集計。
//!
//! wiki の記号(A〜Y, New1/New2 など)は `wiki_symbol()` とドキュメントコメントにだけ置き、識別子には使わない。

use serde::{Deserialize, Serialize};

/// カテゴリの種別(wiki §3「種別の意味」)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryKind {
    /// 代入: 計算結果・倍率をそのまま入れる(後から入れた値で置き換わる)
    Assigned,
    /// 固定値: 整数。初期値 0 に加減算
    Fixed,
    /// 割合: 初期値 0%(=係数 1.0)に加減算。同一カテゴリ内は加算
    Rate,
}

/// 集計値の範囲。割合は Σ% の小数表現(+45% → 0.45)、固定値はそのままの値。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CategoryCap {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl CategoryCap {
    const fn max(max: f64) -> Self {
        Self {
            min: None,
            max: Some(max),
        }
    }

    const fn range(min: f64, max: f64) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    fn clamp(&self, value: f64) -> f64 {
        let value = match self.min {
            Some(min) => value.max(min),
            None => value,
        };
        match self.max {
            Some(max) => value.min(max),
            None => value,
        }
    }
}

/// 与ダメージ式に現れる全カテゴリ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageCategory {
    /// wiki: A 攻撃力
    AttackPower,
    /// wiki: B 攻撃力乱数部分
    AttackRandom,
    /// wiki: C 攻撃対象の防御力
    TargetDefense,
    /// wiki: D スキル倍率
    SkillMultiplier,
    /// wiki: E1 スキル倍率増加(割合)
    SkillMultiplierRate,
    /// wiki: E2 スキル倍率増加(固定値)
    SkillMultiplierFixed,
    /// wiki: F Cri倍率
    CriticalMultiplier,
    /// wiki: G クリティカルダメージ増加
    CriticalDamageRate,
    /// wiki: H コンボボーナス
    ComboBonus,
    /// wiki: I 属性差ボーナス
    ElementBonus,
    /// wiki: J カット率(プレイヤー)
    PlayerCutRate,
    /// wiki: New1 攻撃力増加(シエナのオーラ)
    SienaAuraAttackRate,
    /// wiki: K 最終ダメージ(固定値)
    FinalDamageFixed,
    /// wiki: L 最終ダメージ
    FinalDamageRate,
    /// wiki: V1 カット率A
    CutRateA,
    /// wiki: M 被害減少
    DamageReduction,
    /// wiki: Old 攻撃ダメージII
    AttackDamageLegacy,
    /// wiki: N 覚醒ダメージ
    AwakeningDamage,
    /// wiki: O 物理/魔法ダメージ増加
    PhysicalMagicDamageRate,
    /// wiki: P 特定依存ダメージ増加
    DependencyDamageRate,
    /// wiki: Q 物理/魔法ダメージ吸収(式では 1−Q)
    DamageAbsorb,
    /// wiki: R 物理/魔法被ダメージ倍率
    TakenDamageRate,
    /// wiki: S 被ダメージ減少(式では 1−S)
    TakenDamageReduction,
    /// wiki: T ダメージ増幅
    DamageAmplify,
    /// wiki: U ダメージ耐性(式では 1−U)
    DamageResistance,
    /// wiki: New2 ダメージ緩和(式では 1−New2)
    DamageMitigation,
    /// wiki: V2 カット率B
    CutRateB,
    /// wiki: W 攻撃ダメージ(基本発動)(固定値)
    BasicTriggerDamageFixed,
    /// wiki: X 攻撃ダメージ。**X1〜X6 の合計**で、それ自体には上限が無い。
    /// 値は子カテゴリ(上限適用後)の和として出す — `add` してはいけない
    AttackDamageRate,
    /// wiki: X1 攻撃ダメージ(イザベル)(上限 +50%)。消費アイテム
    AttackDamageIsabel,
    /// wiki: X2 攻撃ダメージ(一般)(上限 +30%)。消費アイテム・バフ・ルーン
    AttackDamageGeneral,
    /// wiki: X3 攻撃ダメージ(基本発動)(上限 +80%)。称号「ダメージ増加」・武器/手アビリティ
    AttackDamageBasicTrigger,
    /// wiki: X4 攻撃ダメージ(スキル)(上限 +65%)。キャラスキル・マスタリー
    AttackDamageSkill,
    /// wiki: X5 攻撃ダメージ(特殊)(wiki は上限「+??%」= 未記載)。ランダムオプション・AF
    AttackDamageSpecial,
    /// wiki: X6 攻撃ダメージ(日本独自)(上限 +30%)。消費アイテム・装備
    AttackDamageJapan,
    /// wiki: Y PVP補正
    PvpCorrection,
}

impl DamageCategory {
    /// 式に現れる順。
    pub const ALL: [DamageCategory; 36] = [
        DamageCategory::AttackPower,
        DamageCategory::AttackRandom,
        DamageCategory::TargetDefense,
        DamageCategory::SkillMultiplier,
        DamageCategory::SkillMultiplierRate,
        DamageCategory::SkillMultiplierFixed,
        DamageCategory::CriticalMultiplier,
        DamageCategory::CriticalDamageRate,
        DamageCategory::ComboBonus,
        DamageCategory::ElementBonus,
        DamageCategory::PlayerCutRate,
        DamageCategory::SienaAuraAttackRate,
        DamageCategory::FinalDamageFixed,
        DamageCategory::FinalDamageRate,
        DamageCategory::CutRateA,
        DamageCategory::DamageReduction,
        DamageCategory::AttackDamageLegacy,
        DamageCategory::AwakeningDamage,
        DamageCategory::PhysicalMagicDamageRate,
        DamageCategory::DependencyDamageRate,
        DamageCategory::DamageAbsorb,
        DamageCategory::TakenDamageRate,
        DamageCategory::TakenDamageReduction,
        DamageCategory::DamageAmplify,
        DamageCategory::DamageResistance,
        DamageCategory::DamageMitigation,
        DamageCategory::CutRateB,
        DamageCategory::BasicTriggerDamageFixed,
        DamageCategory::AttackDamageRate,
        DamageCategory::AttackDamageIsabel,
        DamageCategory::AttackDamageGeneral,
        DamageCategory::AttackDamageBasicTrigger,
        DamageCategory::AttackDamageSkill,
        DamageCategory::AttackDamageSpecial,
        DamageCategory::AttackDamageJapan,
        DamageCategory::PvpCorrection,
    ];

    /// カテゴリX の子(wiki: X = X1+…+X6)。収録済みのぶんだけ持つ。
    /// X1(イザベル)・X2(一般)・X6(日本独自)は未収録
    pub const ATTACK_DAMAGE_CHILDREN: [DamageCategory; 6] = [
        DamageCategory::AttackDamageIsabel,
        DamageCategory::AttackDamageGeneral,
        DamageCategory::AttackDamageBasicTrigger,
        DamageCategory::AttackDamageSkill,
        DamageCategory::AttackDamageSpecial,
        DamageCategory::AttackDamageJapan,
    ];

    pub fn kind(self) -> CategoryKind {
        use DamageCategory::*;
        match self {
            AttackPower | AttackRandom | SkillMultiplier | SkillMultiplierFixed
            | CriticalMultiplier => CategoryKind::Assigned,
            TargetDefense | FinalDamageFixed | DamageReduction | BasicTriggerDamageFixed => {
                CategoryKind::Fixed
            }
            SkillMultiplierRate
            | CriticalDamageRate
            | ComboBonus
            | ElementBonus
            | PlayerCutRate
            | SienaAuraAttackRate
            | FinalDamageRate
            | CutRateA
            | AttackDamageLegacy
            | AwakeningDamage
            | PhysicalMagicDamageRate
            | DependencyDamageRate
            | DamageAbsorb
            | TakenDamageRate
            | TakenDamageReduction
            | DamageAmplify
            | DamageResistance
            | DamageMitigation
            | CutRateB
            | AttackDamageRate
            | AttackDamageIsabel
            | AttackDamageGeneral
            | AttackDamageBasicTrigger
            | AttackDamageSkill
            | AttackDamageSpecial
            | AttackDamageJapan
            | PvpCorrection => CategoryKind::Rate,
        }
    }

    /// プレイヤーが積み上げられるカテゴリか(「一番効いている / 次に伸ばす」の候補)。
    /// 代入(A〜D / F)・敵側(C / M / V1 / Q / R / S / U / New2 / V2)・PVP(Y)・
    /// 子を持つ親(X)・旧仕様(Old)は候補にしない。
    /// 段(D / F)で比べるとスキル固有の値が常勝して努力の範疇外になり、足した実数で比べると
    /// 後段ほど大きな値に掛かって最後の段が構造的に常勝する(ユーザー指摘 2026-08-29)ので、
    /// 候補の中を倍率(`factor`)で比べる。
    pub fn is_effort(self) -> bool {
        use DamageCategory::*;
        !matches!(
            self,
            AttackPower
                | AttackRandom
                | SkillMultiplier
                | CriticalMultiplier
                | TargetDefense
                | DamageReduction
                | CutRateA
                | DamageAbsorb
                | TakenDamageRate
                | TakenDamageReduction
                | DamageResistance
                | DamageMitigation
                | CutRateB
                | AttackDamageLegacy
                | AttackDamageRate
                | PvpCorrection
        )
    }

    /// 供給源の値が **% 表記**かどうか(集計へ入れるときに 1/100 する)。
    ///
    /// 割合カテゴリのほか、E2「スキル倍率増加(固定値)」も含める。wiki ステータスの E2 供給源は
    /// すべて %(兜の「スキル攻撃力増加」+1〜10% / +5〜60%、アナイス「ディトネート特化」+100%)で、
    /// スキル倍率(D)と同じ「1.0 = 100%」の目盛りに足す値だからそのままの数値では 100 倍になる。
    /// 旧リポ twtoolkit の Excel v4.00 実装(`skillTerm = skillMultiplier + helmetAbility / 100`)と一致。
    pub fn is_percent_source(self) -> bool {
        matches!(self.kind(), CategoryKind::Rate) || matches!(self, DamageCategory::SkillMultiplierFixed)
    }

    /// 式の中で `(1 − 値)` として掛かる割合カテゴリ。
    pub fn is_subtractive(self) -> bool {
        use DamageCategory::*;
        matches!(
            self,
            DamageAbsorb | TakenDamageReduction | DamageResistance | DamageMitigation
        )
    }

    /// 集計値の上限・下限(wiki §4)。**上限は子カテゴリごとに違う**ので、
    /// サブカテゴリを持つ X の親自身には上限が無い(子に掛けてから足す)。
    pub fn cap(self) -> Option<CategoryCap> {
        use DamageCategory::*;
        match self {
            ElementBonus => Some(CategoryCap::range(0.0, 0.50)),
            FinalDamageFixed => Some(CategoryCap::max(1000.0)),
            FinalDamageRate => Some(CategoryCap::max(0.45)),
            // 初期100%・下限30%・上限300% → Σ% は -70%..+200%
            AttackDamageLegacy => Some(CategoryCap::range(-0.70, 2.00)),
            DependencyDamageRate => Some(CategoryCap::max(0.73)),
            DamageAbsorb => Some(CategoryCap::range(0.0, 0.70)),
            DamageResistance => Some(CategoryCap::max(0.62)),
            DamageMitigation => Some(CategoryCap::max(0.40)),
            BasicTriggerDamageFixed => Some(CategoryCap::max(1000.0)),
            AttackDamageIsabel => Some(CategoryCap::max(0.50)),
            AttackDamageGeneral => Some(CategoryCap::max(0.30)),
            AttackDamageBasicTrigger => Some(CategoryCap::max(0.80)),
            AttackDamageSkill => Some(CategoryCap::max(0.65)),
            // X5 は wiki が「上限:+??%」と書いていて値が分からない。決め打ちしない
            AttackDamageSpecial => None,
            AttackDamageJapan => Some(CategoryCap::max(0.30)),
            _ => None,
        }
    }

    /// wiki の記号(トレース表示用)。
    pub fn wiki_symbol(self) -> &'static str {
        use DamageCategory::*;
        match self {
            AttackPower => "A",
            AttackRandom => "B",
            TargetDefense => "C",
            SkillMultiplier => "D",
            SkillMultiplierRate => "E1",
            SkillMultiplierFixed => "E2",
            CriticalMultiplier => "F",
            CriticalDamageRate => "G",
            ComboBonus => "H",
            ElementBonus => "I",
            PlayerCutRate => "J",
            SienaAuraAttackRate => "New1",
            FinalDamageFixed => "K",
            FinalDamageRate => "L",
            CutRateA => "V1",
            DamageReduction => "M",
            AttackDamageLegacy => "Old",
            AwakeningDamage => "N",
            PhysicalMagicDamageRate => "O",
            DependencyDamageRate => "P",
            DamageAbsorb => "Q",
            TakenDamageRate => "R",
            TakenDamageReduction => "S",
            DamageAmplify => "T",
            DamageResistance => "U",
            DamageMitigation => "New2",
            CutRateB => "V2",
            BasicTriggerDamageFixed => "W",
            AttackDamageRate => "X",
            AttackDamageIsabel => "X1",
            AttackDamageGeneral => "X2",
            AttackDamageBasicTrigger => "X3",
            AttackDamageSkill => "X4",
            AttackDamageSpecial => "X5",
            AttackDamageJapan => "X6",
            PvpCorrection => "Y",
        }
    }

    /// 日本語名(wiki §4)。
    pub fn label(self) -> &'static str {
        use DamageCategory::*;
        match self {
            AttackPower => "攻撃力",
            AttackRandom => "攻撃力乱数部分",
            TargetDefense => "攻撃対象の防御力",
            SkillMultiplier => "スキル倍率",
            SkillMultiplierRate => "スキル倍率増加(割合)",
            SkillMultiplierFixed => "スキル倍率増加(固定値)",
            CriticalMultiplier => "Cri倍率",
            CriticalDamageRate => "クリティカルダメージ増加",
            ComboBonus => "コンボボーナス",
            ElementBonus => "属性差ボーナス",
            PlayerCutRate => "カット率(プレイヤー)",
            SienaAuraAttackRate => "攻撃力増加(シエナのオーラ)",
            FinalDamageFixed => "最終ダメージ(固定値)",
            FinalDamageRate => "最終ダメージ",
            CutRateA => "カット率A",
            DamageReduction => "被害減少",
            AttackDamageLegacy => "攻撃ダメージII",
            AwakeningDamage => "覚醒ダメージ",
            PhysicalMagicDamageRate => "物理/魔法ダメージ増加",
            DependencyDamageRate => "特定依存ダメージ増加",
            DamageAbsorb => "物理/魔法ダメージ吸収",
            TakenDamageRate => "物理/魔法被ダメージ倍率",
            TakenDamageReduction => "被ダメージ減少",
            DamageAmplify => "ダメージ増幅",
            DamageResistance => "ダメージ耐性",
            DamageMitigation => "ダメージ緩和",
            CutRateB => "カット率B",
            BasicTriggerDamageFixed => "攻撃ダメージ(基本発動)(固定値)",
            AttackDamageRate => "攻撃ダメージ",
            AttackDamageIsabel => "攻撃ダメージ(イザベル)",
            AttackDamageGeneral => "攻撃ダメージ(一般)",
            AttackDamageBasicTrigger => "攻撃ダメージ(基本発動)",
            AttackDamageSkill => "攻撃ダメージ(スキル)",
            AttackDamageSpecial => "攻撃ダメージ(特殊)",
            AttackDamageJapan => "攻撃ダメージ(日本独自)",
            PvpCorrection => "PVP補正",
        }
    }

    /// `ALL` と `values` の添字。variant の宣言順 = `ALL` の順(テストで固定)。
    fn index(self) -> usize {
        self as usize
    }
}

/// 1 カテゴリの集計結果(トレース用)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryTrace {
    pub category: DamageCategory,
    pub symbol: String,
    pub label: String,
    pub kind: CategoryKind,
    /// キャップ適用前の生の集計値。割合は Σ% の小数表現(+15% → 0.15)、固定値は合計、代入はその値
    pub raw: f64,
    /// キャップ適用後の集計値
    pub value: f64,
    /// 式で使われる値(キャップ適用後)。割合は 1+Σ%(減算系は 1−Σ%)、それ以外は value と同じ
    pub factor: f64,
    pub cap: Option<CategoryCap>,
}

/// 全カテゴリの集計値(パイプライン②)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CategoryTotals {
    /// `DamageCategory::ALL` と同じ順。キャップ適用前の生の集計値(Σ)。
    /// serde が配列を扱えるのは 32 要素までなので `Vec` で持つ
    values: Vec<f64>,
}

impl CategoryTotals {
    /// 中立値。割合 = +0%(係数 1.0)、固定値 = 0、代入 = 0.0(呼び出し側が必ず代入する)。
    pub fn neutral() -> Self {
        Self {
            values: vec![0.0; DamageCategory::ALL.len()],
        }
    }

    /// 値を入れる。割合・固定値は同一カテゴリ内で加算、代入は置き換え。
    /// キャップは Σ に対して読み出し時(`value` / `get`)に適用する。
    pub fn add(&mut self, category: DamageCategory, value: f64) {
        debug_assert_ne!(
            category,
            DamageCategory::AttackDamageRate,
            "カテゴリX は子(X3/X4/X5)の合計。供給源は子に足す"
        );
        let slot = &mut self.values[category.index()];
        *slot = match category.kind() {
            CategoryKind::Assigned => value,
            CategoryKind::Fixed | CategoryKind::Rate => *slot + value,
        };
    }

    /// キャップ適用前の生の集計値(割合は Σ%)。
    /// カテゴリX は子(X3/X4/X5)の生の和 — 親に直接足す供給源は無い。
    pub fn raw(&self, category: DamageCategory) -> f64 {
        if category == DamageCategory::AttackDamageRate {
            return DamageCategory::ATTACK_DAMAGE_CHILDREN
                .iter()
                .map(|c| self.raw(*c))
                .sum();
        }
        self.values[category.index()]
    }

    /// キャップ適用後の集計値(割合は Σ%)。
    ///
    /// カテゴリX は**子ごとに上限を掛けてから足す**(X3 は +80%、X4 は +65%)。
    /// 親でまとめて上限を掛けると、片方が上限に届いていてももう片方が伸びてしまう。
    pub fn value(&self, category: DamageCategory) -> f64 {
        if category == DamageCategory::AttackDamageRate {
            return DamageCategory::ATTACK_DAMAGE_CHILDREN
                .iter()
                .map(|c| self.value(*c))
                .sum();
        }
        let raw = self.raw(category);
        match category.cap() {
            Some(cap) => cap.clamp(raw),
            None => raw,
        }
    }

    /// 式で使う値。割合は `1 + Σ%`(減算系は `1 − Σ%`)、固定値・代入はそのまま。
    pub fn get(&self, category: DamageCategory) -> f64 {
        let raw = self.value(category);
        match category.kind() {
            CategoryKind::Rate if category.is_subtractive() => 1.0 - raw,
            CategoryKind::Rate => 1.0 + raw,
            CategoryKind::Assigned | CategoryKind::Fixed => raw,
        }
    }

    pub fn trace(&self) -> Vec<CategoryTrace> {
        DamageCategory::ALL
            .iter()
            .map(|&category| CategoryTrace {
                category,
                symbol: category.wiki_symbol().to_string(),
                label: category.label().to_string(),
                kind: category.kind(),
                raw: self.raw(category),
                value: self.value(category),
                factor: self.get(category),
                cap: category.cap(),
            })
            .collect()
    }
}

impl Default for CategoryTotals {
    fn default() -> Self {
        Self::neutral()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use DamageCategory::*;

    #[test]
    fn all_は全カテゴリを重複なく宣言順に持つ() {
        assert_eq!(DamageCategory::ALL.len(), 36);
        let mut symbols: Vec<_> = DamageCategory::ALL
            .iter()
            .map(|c| c.wiki_symbol())
            .collect();
        symbols.sort_unstable();
        symbols.dedup();
        assert_eq!(symbols.len(), 36);
        // `index()` は variant の宣言順に依存するので ALL の並びと一致させる
        for (i, c) in DamageCategory::ALL.iter().enumerate() {
            assert_eq!(
                c.index(),
                i,
                "{} の位置が ALL と一致しない",
                c.wiki_symbol()
            );
        }
    }

    #[test]
    fn 中立値は割合1固定0() {
        let t = CategoryTotals::neutral();
        assert_eq!(t.get(FinalDamageRate), 1.0);
        assert_eq!(t.get(DamageAbsorb), 1.0);
        assert_eq!(t.get(FinalDamageFixed), 0.0);
        assert_eq!(t.get(AttackPower), 0.0);
        assert_eq!(t.trace().len(), 36);
    }

    /// wiki: X = X1+…+X6 で、**上限は子ごとに違う**(X3 +80% / X4 +65%)。
    /// 親でまとめて上限を掛けると、片方が上限に届いていてももう片方が伸びてしまう。
    #[test]
    fn カテゴリxは子ごとに上限を掛けてから足す() {
        let mut t = CategoryTotals::neutral();
        t.add(AttackDamageBasicTrigger, 1.00); // 称号など。上限 +80%
        t.add(AttackDamageSkill, 0.90); // キャラスキル・マスタリー。上限 +65%
        t.add(AttackDamageSpecial, 0.20); // ランダムオプション。上限は wiki 未記載
        assert_eq!(t.value(AttackDamageBasicTrigger), 0.80);
        assert_eq!(t.value(AttackDamageSkill), 0.65);
        assert_eq!(t.value(AttackDamageSpecial), 0.20);
        // 親は上限適用後の和(0.80 + 0.65 + 0.20 = 1.65)。親自身に上限は無い
        assert!((t.value(AttackDamageRate) - 1.65).abs() < 1e-12);
        assert!((t.get(AttackDamageRate) - 2.65).abs() < 1e-12);
        assert_eq!(t.raw(AttackDamageRate), 2.10);
    }

    #[test]
    fn 割合は同一カテゴリ内で加算される() {
        let mut t = CategoryTotals::neutral();
        t.add(FinalDamageRate, 0.2);
        t.add(FinalDamageRate, 0.1);
        assert!((t.get(FinalDamageRate) - 1.3).abs() < 1e-12);
    }

    #[test]
    fn 減算系の割合は1から引く() {
        let mut t = CategoryTotals::neutral();
        t.add(DamageAbsorb, 0.3);
        assert!((t.get(DamageAbsorb) - 0.7).abs() < 1e-12);
        assert!((t.value(DamageAbsorb) - 0.3).abs() < 1e-12);
    }

    #[test]
    fn 固定値は加算され代入は置き換わる() {
        let mut t = CategoryTotals::neutral();
        t.add(DamageReduction, -100.0);
        t.add(DamageReduction, -50.0);
        assert_eq!(t.get(DamageReduction), -150.0);
        t.add(SkillMultiplier, 1.5);
        t.add(SkillMultiplier, 2.0);
        assert_eq!(t.get(SkillMultiplier), 2.0);
    }

    #[test]
    fn キャップは合計に対して適用される() {
        let mut t = CategoryTotals::neutral();
        t.add(FinalDamageRate, 0.5);
        assert!((t.get(FinalDamageRate) - 1.45).abs() < 1e-12);
        assert!((t.raw(FinalDamageRate) - 0.5).abs() < 1e-12);
        // +0.5 − 0.1 = +0.4(キャップ 0.45 未満なのでそのまま)。add ごとに clamp していれば 0.35 になってしまう
        t.add(FinalDamageRate, -0.1);
        assert!((t.value(FinalDamageRate) - 0.40).abs() < 1e-12);
        assert!((t.get(FinalDamageRate) - 1.40).abs() < 1e-12);
        let trace = t.trace();
        let l = trace.iter().find(|c| c.symbol == "L").unwrap();
        assert!(
            (l.raw - 0.40).abs() < 1e-12
                && (l.value - 0.40).abs() < 1e-12
                && (l.factor - 1.40).abs() < 1e-12
        );
    }

    #[test]
    fn キャップが適用される() {
        let mut t = CategoryTotals::neutral();
        t.add(ElementBonus, -0.3);
        assert_eq!(t.get(ElementBonus), 1.0);
        t.add(ElementBonus, 0.9);
        assert!((t.get(ElementBonus) - 1.5).abs() < 1e-12);
        t.add(FinalDamageFixed, 1500.0);
        assert_eq!(t.get(FinalDamageFixed), 1000.0);
        t.add(AttackDamageLegacy, -0.9);
        assert!((t.get(AttackDamageLegacy) - 0.3).abs() < 1e-12);
        t.add(AttackDamageLegacy, 5.0);
        assert!((t.get(AttackDamageLegacy) - 3.0).abs() < 1e-12);
    }
}
