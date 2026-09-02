//! 実測ダメージから敵の防御力・カット率を逆算するための最小条件
//! (手順は docs/enemy-verification.md)。
//!
//! 敵側は 防御力 C(引き算)・カット率 K(掛け算)・被害減少 M(引き算)で効き方が違い、
//! 実測は `y = (x − C) × K − M` の直線になる。傾き K と切片を分けて出すには
//! **攻撃力 x を変えた点が 2 つ以上**要る。1 点を何度測っても直線は引けない。

/// 防御力とカット率を分けて逆算するのに要る、攻撃力の異なる点の数。
pub const MEASUREMENT_SEPARABLE_MIN_ATTACKS: usize = 2;

/// 集めた実測点の攻撃力(未記入は `None`)から、防御力とカット率を分けて逆算できるか。
pub fn can_separate_defense_and_cut_rate(attacks: &[Option<i64>]) -> bool {
    let distinct: std::collections::HashSet<i64> = attacks.iter().flatten().copied().collect();
    distinct.len() >= MEASUREMENT_SEPARABLE_MIN_ATTACKS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 攻撃力の異なる点が2つ以上あれば分けられる() {
        assert!(can_separate_defense_and_cut_rate(&[Some(1000), Some(1200)]));
    }

    #[test]
    fn 同じ攻撃力を何度測っても分けられない() {
        assert!(!can_separate_defense_and_cut_rate(&[
            Some(1000),
            Some(1000),
            Some(1000)
        ]));
    }

    #[test]
    fn 攻撃力未記入の点は数えない() {
        assert!(!can_separate_defense_and_cut_rate(&[Some(1000), None]));
    }
}
