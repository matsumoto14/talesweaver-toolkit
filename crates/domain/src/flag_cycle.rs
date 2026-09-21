//! <フラグ>(イェフネンの、技とは別枠のダメージ)を爆発させるときの「積み直しの 1 周」。
//!
//! スレイ / クラッシュ は撃つたびに <フラグ> を消費するので、「毎回そのスタック数が乗る」
//! 前提の DPS は過大になる。実際には
//!
//! ```text
//! 積む技(連 / 爆)× n 回 → 主軸(スレイ / クラッシュ)1 回 → 爆発
//! ```
//!
//! の 1 周を繰り返す。さらに スレイ / クラッシュ には CT があるので、1 周が CT より
//! 短くなることはない(余った時間は積む技を撃っている = 回数が増える)。
//!
//! 「1 回で積む数」「爆発で残る量」「CT」は gamedata / 技データが持ち、
//! ここは**回数と 1 周の時間を出す規則だけ**を持つ。

/// 積み直しの 1 周(回数と時間)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlagCyclePlan {
    /// 1 周で積む技を撃つ回数
    pub uses: u32,
    /// 1 周の時間(秒)= 回数 × 積む技の所要時間 + 主軸の所要時間
    pub seconds: f64,
    /// CT を満たすために回数を増やしたか(積み直しに要る回数より多く撃っている)
    pub cooldown_bound: bool,
}

/// 1 周を組む。
///
/// - `stacks_to_apply`: 次の爆発までに積み直す量(爆発で全部消える形態はスタック数そのもの。
///   ウルミは半分残るのでその差分)
/// - `stacks_per_use`: 積む技 1 回で積む数
/// - `applier_seconds` / `main_seconds`: 積む技 / 主軸を 1 回撃つのにかかる時間
///   (コンボ中は通常攻撃を挟んだ 1 サイクル。`DamageResult::cycle_seconds`)
/// - `cooldown_seconds`: 主軸の CT。1 周はこれより短くならない
///
/// どちらかの所要時間が 0 以下なら `None`(1 周を組めない = DPS を出さない)。
pub fn plan_flag_cycle(
    stacks_to_apply: u8,
    stacks_per_use: u8,
    applier_seconds: f64,
    main_seconds: f64,
    cooldown_seconds: f64,
) -> Option<FlagCyclePlan> {
    if applier_seconds <= 0.0 || main_seconds <= 0.0 || stacks_per_use == 0 {
        return None;
    }
    // 積み直しに要る回数(切り上げ)。積み直す量が 0 でも、爆発のたびに 1 回は積み直す
    // 余地がある形態は無いので 0 回を許す(ウルミで半分残り、それで足りるとき)
    let needed = u32::from(stacks_to_apply).div_ceil(u32::from(stacks_per_use));
    // CT を満たすのに要る回数。CT 待ちの間は積む技を撃っている扱い
    let for_cooldown = if cooldown_seconds > main_seconds {
        ((cooldown_seconds - main_seconds) / applier_seconds).ceil().max(0.0) as u32
    } else {
        0
    };
    let uses = needed.max(for_cooldown);
    Some(FlagCyclePlan {
        uses,
        seconds: f64::from(uses) * applier_seconds + main_seconds,
        cooldown_bound: for_cooldown > needed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 積み直しに要る回数は切り上げ() {
        // スタック 10 を 1 回 2 ずつ → 5 回。1 周 11.4s は CT 10s を満たすので回数は増えない
        let plan = plan_flag_cycle(10, 2, 2.0, 1.4, 10.0).unwrap();
        assert_eq!(plan.uses, 5);
        assert!(!plan.cooldown_bound);
        assert!((plan.seconds - 11.4).abs() < 1e-12);
        // 1 回 3 ずつなら 4 回(10 / 3 の切り上げ)
        assert_eq!(plan_flag_cycle(10, 3, 1.0, 1.4, 0.0).unwrap().uses, 4);
    }

    #[test]
    fn ctに満たないときは積む技の回数で埋める() {
        // 積み直しは 1 回で足りるが、1 周 2.4s では CT 10s に満たない
        let plan = plan_flag_cycle(5, 5, 1.0, 1.4, 10.0).unwrap();
        assert!(plan.cooldown_bound);
        assert_eq!(plan.uses, 9); // (10 − 1.4) / 1.0 の切り上げ
        assert!(plan.seconds >= 10.0);
        // CT を満たす最小の回数(1 回減らすと CT 未満)
        assert!((f64::from(plan.uses - 1) * 1.0 + 1.4) < 10.0);
    }

    #[test]
    fn 一周はctより短くならない() {
        for stacks in 0..=10u8 {
            let plan = plan_flag_cycle(stacks, 2, 0.8, 1.2, 10.0).unwrap();
            assert!(plan.seconds >= 10.0 - 1e-9, "stacks={stacks}");
        }
    }

    #[test]
    fn スタックを下げると回数が減る() {
        let many = plan_flag_cycle(10, 2, 2.5, 1.4, 0.0).unwrap();
        let few = plan_flag_cycle(4, 2, 2.5, 1.4, 0.0).unwrap();
        assert!(few.uses < many.uses);
        assert!(few.seconds < many.seconds);
    }

    #[test]
    fn 所要時間が出せないなら1周を組まない() {
        assert_eq!(plan_flag_cycle(10, 2, 0.0, 1.4, 10.0), None);
        assert_eq!(plan_flag_cycle(10, 2, 1.0, 0.0, 10.0), None);
        assert_eq!(plan_flag_cycle(10, 0, 1.0, 1.4, 10.0), None);
    }
}
