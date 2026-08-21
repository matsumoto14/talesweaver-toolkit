//! 覚醒倍率表(wiki: カテゴリN)。

use domain::Awakening;

use crate::{Source, LEGACY_TWTOOLKIT_RETRIEVED_ON};

pub const AWAKENING_SOURCE: Source = Source {
    page: "旧リポ twtoolkit awakening.json(stage5LookupTable)",
    retrieved_on: LEGACY_TWTOOLKIT_RETRIEVED_ON,
    note: "エタの意志 Lv 0〜80 の 81 点を全点転記。wiki Quest/覚醒クエスト・エタの意志で要裏取り",
};

/// 覚醒段階 0〜4 の倍率(段階 0・1 は 1.0)。
const STAGE_RATES: [f64; 5] = [1.0, 1.0, 1.2, 1.4, 1.6];

/// 覚醒段階 5(極限)でのエタの意志 Lv 0〜80 → 倍率。index = Lv。
const ETERNAL_RATES: [f64; 81] = [
    2.00, 2.01, 2.01, 2.02, 2.02, 2.03, 2.03, 2.04, 2.04, 2.05, // Lv 0〜9
    2.05, 2.06, 2.06, 2.07, 2.07, 2.08, 2.08, 2.09, 2.09, 2.10, // Lv 10〜19
    2.10, 2.15, 2.15, 2.16, 2.16, 2.17, 2.17, 2.18, 2.18, 2.19, // Lv 20〜29
    2.19, 2.20, 2.20, 2.21, 2.21, 2.22, 2.22, 2.23, 2.23, 2.24, // Lv 30〜39
    2.24, 2.29, 2.29, 2.30, 2.30, 2.31, 2.31, 2.32, 2.32, 2.33, // Lv 40〜49
    2.33, 2.34, 2.34, 2.35, 2.35, 2.36, 2.36, 2.37, 2.37, 2.38, // Lv 50〜59
    2.38, 2.40, 2.40, 2.41, 2.41, 2.42, 2.42, 2.43, 2.43, 2.44, // Lv 60〜69
    2.44, 2.45, 2.45, 2.46, 2.46, 2.47, 2.47, 2.48, 2.48, 2.49, // Lv 70〜79
    2.49, // Lv 80〜80
];

/// 覚醒倍率。段階 5 以上は極限扱いでエタの意志 Lv を参照する(Lv 80 超は 80 として扱う)。
pub fn awakening_rate(awakening: Awakening) -> f64 {
    let stage = usize::from(awakening.stage);
    if stage < STAGE_RATES.len() {
        return STAGE_RATES[stage];
    }
    let level = usize::from(awakening.eternal_level).min(ETERNAL_RATES.len() - 1);
    ETERNAL_RATES[level]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rate(stage: u8, eternal_level: u8) -> f64 {
        awakening_rate(Awakening { stage, eternal_level })
    }

    #[test]
    fn 段階ごとの倍率() {
        assert_eq!(rate(0, 0), 1.0);
        assert_eq!(rate(1, 0), 1.0);
        assert_eq!(rate(2, 0), 1.2);
        assert_eq!(rate(3, 0), 1.4);
        assert_eq!(rate(4, 0), 1.6);
        // 段階 4 以下ではエタ Lv を無視する
        assert_eq!(rate(4, 80), 1.6);
    }

    #[test]
    fn 極限はエタの意志lvごとの表を引く() {
        assert_eq!(rate(5, 0), 2.00);
        assert_eq!(rate(5, 1), 2.01);
        assert_eq!(rate(5, 10), 2.05);
        assert_eq!(rate(5, 21), 2.15);
        assert_eq!(rate(5, 30), 2.19);
        assert_eq!(rate(5, 40), 2.24);
        assert_eq!(rate(5, 60), 2.38);
        assert_eq!(rate(5, 80), 2.49);
        // 表は単調非減少
        assert!(ETERNAL_RATES.windows(2).all(|w| w[0] <= w[1]));
    }
}
