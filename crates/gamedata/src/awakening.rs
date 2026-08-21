//! 覚醒倍率表(wiki: カテゴリN)。

use domain::Awakening;

use crate::{Source, LEGACY_TWTOOLKIT_RETRIEVED_ON};

pub const AWAKENING_SOURCE: Source = Source {
    page: "旧リポ twtoolkit awakening.json",
    retrieved_on: LEGACY_TWTOOLKIT_RETRIEVED_ON,
    note: "エタの意志 Lv は既知点(0/10/20/40/80)のみ。未知 Lv は直近下位の既知点を採用。wiki Quest/覚醒クエスト・エタの意志で要裏取り",
};

/// 覚醒段階 0〜4 の倍率(段階 0・1 は 1.0)。
const STAGE_RATES: [f64; 5] = [1.0, 1.0, 1.2, 1.4, 1.6];

/// 覚醒段階 5(極限)でのエタの意志 Lv → 倍率の既知点。Lv 昇順。
const ETERNAL_RATES: [(u8, f64); 5] = [(0, 2.00), (10, 2.05), (20, 2.10), (40, 2.24), (80, 2.49)];

/// 覚醒倍率。段階 5 以上は極限扱いでエタの意志 Lv を参照し、未知 Lv は直近下位の既知点を使う。
pub fn awakening_rate(awakening: Awakening) -> f64 {
    let stage = usize::from(awakening.stage);
    if stage < STAGE_RATES.len() {
        return STAGE_RATES[stage];
    }
    ETERNAL_RATES
        .iter()
        .rev()
        .find(|(level, _)| *level <= awakening.eternal_level)
        .map(|(_, rate)| *rate)
        .unwrap_or(ETERNAL_RATES[0].1)
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
    fn 極限はエタの意志lvの直近下位の既知点() {
        assert_eq!(rate(5, 0), 2.00);
        assert_eq!(rate(5, 9), 2.00);
        assert_eq!(rate(5, 10), 2.05);
        assert_eq!(rate(5, 39), 2.10);
        assert_eq!(rate(5, 40), 2.24);
        assert_eq!(rate(5, 80), 2.49);
    }
}
