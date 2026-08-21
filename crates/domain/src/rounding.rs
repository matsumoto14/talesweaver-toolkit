//! 丸め関数。wiki の `[]`(小数点以下切捨)と `{}`(小数第3位以下切捨)を型で固定する。
//! 式の中に裸の `floor()` / `as` を書かない(docs/architecture.md)。

/// 浮動小数演算で 2.9999999999 のようになった値を 3 として扱うための許容誤差。
const EPSILON: f64 = 1e-9;

/// wiki の `[]`: 小数点以下切捨(負数は負の無限大方向 = Excel の INT と同じ)。
pub fn floor_int(value: f64) -> i64 {
    (value + EPSILON).floor() as i64
}

/// wiki の `{}`: 小数第2位まで適用し第3位以下を切捨てる。
pub fn trunc2(value: f64) -> f64 {
    (value * 100.0 + EPSILON).floor() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_int_は小数点以下を切捨てる() {
        assert_eq!(floor_int(3.0), 3);
        assert_eq!(floor_int(3.999), 3);
        assert_eq!(floor_int(0.0), 0);
        assert_eq!(floor_int(-0.5), -1);
        assert_eq!(floor_int(-3.2), -4);
    }

    #[test]
    fn floor_int_は浮動小数誤差を吸収する() {
        // 0.1 * 3 = 0.30000000000000004 のような誤差で 1 繰り下がらない
        assert_eq!(floor_int(2.9999999999999996), 3);
        assert_eq!(floor_int(1.1 * 3.0 * 10.0), 33); // 33.00000000000001 → 33
    }

    #[test]
    fn trunc2_は小数第3位以下を切捨てる() {
        assert_eq!(trunc2(1.239), 1.23);
        assert_eq!(trunc2(1.2), 1.2);
        assert_eq!(trunc2(0.0), 0.0);
        assert_eq!(trunc2(5.0), 5.0);
        assert_eq!(trunc2(-1.239), -1.24);
    }

    #[test]
    fn trunc2_は浮動小数誤差を吸収する() {
        // 1.15 * 100 = 114.99999999999999 だが 1.15 のまま
        assert_eq!(trunc2(1.15), 1.15);
        assert_eq!(trunc2(0.29), 0.29);
        assert_eq!(trunc2(2.7 * 1.1), 2.97); // 2.9700000000000006
    }
}
