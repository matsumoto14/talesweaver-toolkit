//! ビアヌのインクリ(wiki: 装備システム/インクリ。クライアント DB
//! `db/dm_00000_0419.csv`(EncryptDataTemplate、ItemId 5600000〜5600004)。ユーザー確認 2026-09-17)。
//!
//! 装備の「合成回数」を 1 減らし「インクリ回数」を 1 増やす消耗行為。5 種類あり、ロード / 加護 /
//! 祝福 / 王室は失敗すると装備が破壊されるが、**ビアヌだけは失敗しても何も起きない**(合成回数も
//! インクリ回数も変わらない)。成功率(10万分率)はロード 21000・加護 26000・祝福 31000・王室 36000
//! の固定値で、ビアヌだけそれまでの成功回数 `inkri_count` に応じて `70 - 5 * inkri_count`
//! (下限 10、= n≥12 で 0.010%)まで下がる。エタインクリ(ItemId 5600005)は対象外(作らない)。
//!
//! 「合成回数が1/4になるまでインクリできる」(wiki)の端数処理は wiki に明記が無いため、
//! [仮] 切り上げと推定して `min_synth_threshold` に実装する。

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// インクリの種類(wiki: 装備システム/インクリ の表、クライアント DB `dm_00000_0419.csv`)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InkriKind {
    /// ロードのインクリ(ItemId 5600000)
    Lord,
    /// 加護のインクリ(ItemId 5600001)
    Grace,
    /// 祝福のインクリ(ItemId 5600002)
    Blessing,
    /// 王室のインクリ(ItemId 5600003)
    Royal,
    /// ビアヌのインクリ(ItemId 5600004)。失敗しても装備は破壊されない。
    Vianu,
}

impl InkriKind {
    pub const ALL: [InkriKind; 5] = [
        InkriKind::Lord,
        InkriKind::Grace,
        InkriKind::Blessing,
        InkriKind::Royal,
        InkriKind::Vianu,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InkriKind::Lord => "ロードのインクリ",
            InkriKind::Grace => "加護のインクリ",
            InkriKind::Blessing => "祝福のインクリ",
            InkriKind::Royal => "王室のインクリ",
            InkriKind::Vianu => "ビアヌのインクリ",
        }
    }

    /// wiki の確率表記(低確率/中確率/極めて低確率)。実際の成功率は `success_rate` で持つ。
    pub fn probability_label(self) -> &'static str {
        match self {
            InkriKind::Lord | InkriKind::Grace => "低確率",
            InkriKind::Blessing | InkriKind::Royal => "中確率",
            InkriKind::Vianu => "極めて低確率",
        }
    }

    /// 失敗時に装備が破壊されるか(ビアヌだけ破壊されない)。
    pub fn destroys_on_failure(self) -> bool {
        !matches!(self, InkriKind::Vianu)
    }

    /// 成功率(10万分率。roll は `0..100_000` を渡す)。ビアヌはそれまでの成功回数
    /// `inkri_count` に応じて下がる(`70 - 5 * inkri_count`、下限 10)。
    pub fn success_rate(self, inkri_count: i64) -> i64 {
        match self {
            InkriKind::Lord => 21_000,
            InkriKind::Grace => 26_000,
            InkriKind::Blessing => 31_000,
            InkriKind::Royal => 36_000,
            InkriKind::Vianu => (70 - 5 * inkri_count.max(0)).max(10),
        }
    }
}

/// 装備 1 個のインクリに関わる状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentInkriState {
    /// 現在の合成回数(減っていく)
    pub synth_current: i64,
    /// 合成回数の初期上限(装備固有。gamedata の `InkriTarget::synth_max`)
    pub synth_max: i64,
    /// これまでの成功回数(ビアヌの成功率を下げる)
    pub inkri_count: i64,
    /// 破壊済みか
    pub destroyed: bool,
}

impl EquipmentInkriState {
    /// 未使用の初期状態(合成回数は上限のまま、インクリ回数 0、未破壊)。
    pub fn fresh(synth_max: i64) -> Self {
        Self {
            synth_current: synth_max,
            synth_max,
            inkri_count: 0,
            destroyed: false,
        }
    }
}

/// 実行できない理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error, Serialize, Deserialize)]
pub enum InkriBlockReason {
    #[error("この装備は破壊されています")]
    Destroyed,
    #[error(
        "合成回数が {current} 回(上限 {max} 回の1/4 = {threshold} 回)以下のため、\
         これ以上インクリできません"
    )]
    SynthTooLow {
        current: i64,
        max: i64,
        threshold: i64,
    },
}

/// 合成回数の下限([仮] 端数は切り上げと推定。wiki は端数処理を明記していない)。
/// この値**以下**になるとインクリできない(= 1/4 を割り込む一歩手前で止まる)。
pub fn min_synth_threshold(synth_max: i64) -> i64 {
    (synth_max + 3) / 4
}

/// この状態でインクリを実行できるか。
pub fn check_can_attempt(state: EquipmentInkriState) -> Result<(), InkriBlockReason> {
    if state.destroyed {
        return Err(InkriBlockReason::Destroyed);
    }
    let threshold = min_synth_threshold(state.synth_max);
    if state.synth_current <= threshold {
        return Err(InkriBlockReason::SynthTooLow {
            current: state.synth_current,
            max: state.synth_max,
            threshold,
        });
    }
    Ok(())
}

/// 1 回の試行の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InkriAttemptOutcome {
    /// 成功(合成回数 -1、インクリ回数 +1)
    Success,
    /// 失敗して装備が破壊された(ロード/加護/祝福/王室)
    FailureDestroyed,
    /// 失敗したが何も変わらなかった(ビアヌ)
    FailureNoChange,
}

/// 1 回の試行(`roll` は呼び出し側が渡す `0..100_000` の一様乱数)。
pub fn attempt(
    state: EquipmentInkriState,
    kind: InkriKind,
    roll: i64,
) -> Result<(EquipmentInkriState, InkriAttemptOutcome), InkriBlockReason> {
    check_can_attempt(state)?;
    let rate = kind.success_rate(state.inkri_count);
    if roll < rate {
        let next = EquipmentInkriState {
            synth_current: state.synth_current - 1,
            inkri_count: state.inkri_count + 1,
            ..state
        };
        Ok((next, InkriAttemptOutcome::Success))
    } else if kind.destroys_on_failure() {
        let next = EquipmentInkriState {
            destroyed: true,
            ..state
        };
        Ok((next, InkriAttemptOutcome::FailureDestroyed))
    } else {
        Ok((state, InkriAttemptOutcome::FailureNoChange))
    }
}

/// domain 内で完結する小さな決定的 PRNG(splitmix64)。テストや再現実行のために
/// シードだけで結果が決まる。外部クレートに依存しない(このリポジトリの依存最小方針)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InkriRng(u64);

impl InkriRng {
    /// シードから初期化する。`0` は splitmix64 の弱い初期状態になりやすいので、
    /// 既知の定数で下駄を履かせる(結果はシード値ごとに決定的なまま)。
    pub fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// `0..100_000` の一様乱数(インクリの roll に使う)。値域が小さいので `%` の偏りは無視できる。
    pub fn next_roll(&mut self) -> i64 {
        (self.next_u64() % 100_000) as i64
    }
}

/// まとめて試す回数の指定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InkriBatchMode {
    /// 実行できなくなるか destroyed になるまでの間、指定回数だけ試す
    Fixed { attempts: i64 },
    /// 成功する(または実行できなくなる/破壊される)まで、上限回数を超えない範囲で試す
    UntilSuccess { max_attempts: i64 },
}

/// まとめ試行の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InkriBatchResult {
    pub attempts_made: i64,
    pub successes: i64,
    pub destroyed: bool,
    pub final_state: EquipmentInkriState,
    pub last_outcome: Option<InkriAttemptOutcome>,
    /// 消費 SEED(`seed_cost_per_attempt` が `None` = 費用未収録のときは `None`)
    pub consumed_seed: Option<i64>,
}

/// N 回、または成功するまで(上限回数つき)試す。実行できなくなった(1/4 到達)/ 破壊された
/// 時点で打ち切る。`seed_cost_per_attempt` は gamedata 側のビアヌ費用(未収録装備は `None`)。
pub fn run_batch(
    mut state: EquipmentInkriState,
    kind: InkriKind,
    mode: InkriBatchMode,
    rng: &mut InkriRng,
    seed_cost_per_attempt: Option<i64>,
) -> InkriBatchResult {
    let limit = match mode {
        InkriBatchMode::Fixed { attempts } => attempts,
        InkriBatchMode::UntilSuccess { max_attempts } => max_attempts,
    };
    let mut attempts_made = 0i64;
    let mut successes = 0i64;
    let mut last_outcome = None;
    for _ in 0..limit.max(0) {
        if check_can_attempt(state).is_err() {
            break;
        }
        let roll = rng.next_roll();
        let (next_state, outcome) =
            attempt(state, kind, roll).expect("check_can_attempt を通したあとなので必ず Ok");
        state = next_state;
        attempts_made += 1;
        let success = matches!(outcome, InkriAttemptOutcome::Success);
        if success {
            successes += 1;
        }
        last_outcome = Some(outcome);
        if state.destroyed {
            break;
        }
        if success && matches!(mode, InkriBatchMode::UntilSuccess { .. }) {
            break;
        }
    }
    InkriBatchResult {
        attempts_made,
        successes,
        destroyed: state.destroyed,
        final_state: state,
        last_outcome,
        consumed_seed: seed_cost_per_attempt.map(|cost| cost * attempts_made),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 固定4種の成功率は10万分率の表通り() {
        assert_eq!(InkriKind::Lord.success_rate(0), 21_000);
        assert_eq!(InkriKind::Grace.success_rate(0), 26_000);
        assert_eq!(InkriKind::Blessing.success_rate(0), 31_000);
        assert_eq!(InkriKind::Royal.success_rate(0), 36_000);
        // インクリ回数が増えても固定4種は変わらない
        assert_eq!(InkriKind::Lord.success_rate(5), 21_000);
    }

    #[test]
    fn ビアヌの成功率は13段で下限10に張り付く() {
        let expected = [
            70, 65, 60, 55, 50, 45, 40, 35, 30, 25, 20, 15, 10,
        ];
        for (n, &rate) in expected.iter().enumerate() {
            assert_eq!(InkriKind::Vianu.success_rate(n as i64), rate, "n={n}");
        }
        // n=12 以降は下限 10 に張り付く
        assert_eq!(InkriKind::Vianu.success_rate(13), 10);
        assert_eq!(InkriKind::Vianu.success_rate(100), 10);
    }

    #[test]
    fn 成功すると合成回数が減りインクリ回数が増える() {
        let state = EquipmentInkriState::fresh(8);
        let (next, outcome) = attempt(state, InkriKind::Lord, 0).unwrap();
        assert_eq!(outcome, InkriAttemptOutcome::Success);
        assert_eq!(next.synth_current, 7);
        assert_eq!(next.inkri_count, 1);
        assert!(!next.destroyed);
    }

    #[test]
    fn 固定4種は失敗すると破壊される() {
        let state = EquipmentInkriState::fresh(8);
        let (next, outcome) = attempt(state, InkriKind::Lord, 99_999).unwrap();
        assert_eq!(outcome, InkriAttemptOutcome::FailureDestroyed);
        assert!(next.destroyed);
        assert_eq!(next.synth_current, 8, "破壊時は合成回数を変えない");
    }

    #[test]
    fn ビアヌは失敗しても何も変わらない() {
        let state = EquipmentInkriState::fresh(8);
        let (next, outcome) = attempt(state, InkriKind::Vianu, 99_999).unwrap();
        assert_eq!(outcome, InkriAttemptOutcome::FailureNoChange);
        assert_eq!(next, state);
    }

    #[test]
    fn 破壊済みは実行できない() {
        let state = EquipmentInkriState {
            synth_current: 5,
            synth_max: 8,
            inkri_count: 0,
            destroyed: true,
        };
        assert_eq!(
            check_can_attempt(state),
            Err(InkriBlockReason::Destroyed)
        );
        assert!(attempt(state, InkriKind::Lord, 0).is_err());
    }

    #[test]
    fn 合成回数が上限の4分の1以下だと実行できない() {
        // max=8 → threshold=2。current<=2 で不可、current=3 なら可
        assert_eq!(min_synth_threshold(8), 2);
        let blocked = EquipmentInkriState {
            synth_current: 2,
            synth_max: 8,
            inkri_count: 0,
            destroyed: false,
        };
        assert!(matches!(
            check_can_attempt(blocked),
            Err(InkriBlockReason::SynthTooLow { .. })
        ));
        let allowed = EquipmentInkriState {
            synth_current: 3,
            ..blocked
        };
        assert!(check_can_attempt(allowed).is_ok());
    }

    #[test]
    fn 端数切り上げの上限計算() {
        // 7 の 1/4 = 1.75 → 切り上げ 2([仮]。テストで固定できるようにしておく)
        assert_eq!(min_synth_threshold(7), 2);
        assert_eq!(min_synth_threshold(6), 2);
        assert_eq!(min_synth_threshold(5), 2);
        assert_eq!(min_synth_threshold(4), 1);
        assert_eq!(min_synth_threshold(3), 1);
        assert_eq!(min_synth_threshold(1), 1);
    }

    #[test]
    fn 上限到達で実行が止まる場合はそこで打ち切る() {
        // max=4, threshold=1。current=2 から始めると 1 回で threshold に到達し、次は不可
        let state = EquipmentInkriState::fresh(4);
        let mut rng = InkriRng::new(1);
        let result = run_batch(
            state,
            InkriKind::Lord,
            InkriBatchMode::Fixed { attempts: 10 },
            &mut rng,
            None,
        );
        // 上限4から threshold=1まで、成功が続く限り最大3回で打ち切られる(1/4以下で停止)
        assert!(result.attempts_made <= 3);
        assert!(result.final_state.synth_current > min_synth_threshold(4) || result.destroyed);
    }

    #[test]
    fn シード固定で結果が再現する() {
        let state = EquipmentInkriState::fresh(100);
        let mode = InkriBatchMode::Fixed { attempts: 20 };
        let mut rng_a = InkriRng::new(42);
        let mut rng_b = InkriRng::new(42);
        let result_a = run_batch(state, InkriKind::Vianu, mode, &mut rng_a, Some(3_000_000));
        let result_b = run_batch(state, InkriKind::Vianu, mode, &mut rng_b, Some(3_000_000));
        assert_eq!(result_a, result_b);
    }

    #[test]
    fn シードが違えば結果も変わる() {
        // 事前に splitmix64 の初手を計算して確定させた組(シード1→roll 88969、シード4→roll 5741)。
        // Royal(36000)に対して 1 は失敗(破壊)、4 は成功になる。
        let state = EquipmentInkriState::fresh(100);
        let mode = InkriBatchMode::Fixed { attempts: 1 };
        let mut rng_destroyed = InkriRng::new(1);
        let result_destroyed = run_batch(state, InkriKind::Royal, mode, &mut rng_destroyed, None);
        assert!(result_destroyed.destroyed);

        let mut rng_success = InkriRng::new(4);
        let result_success = run_batch(state, InkriKind::Royal, mode, &mut rng_success, None);
        assert!(!result_success.destroyed);
        assert_eq!(result_success.successes, 1);
        assert_ne!(result_destroyed, result_success);
    }

    #[test]
    fn 消費seedは費用が未収録なら常にnone() {
        let state = EquipmentInkriState::fresh(100);
        let mut rng = InkriRng::new(7);
        let result = run_batch(
            state,
            InkriKind::Lord,
            InkriBatchMode::Fixed { attempts: 3 },
            &mut rng,
            None,
        );
        assert_eq!(result.consumed_seed, None);
    }

    #[test]
    fn 成功するまでモードは成功した時点で打ち切る() {
        // シード4の初手 roll=5741 は Lord(21000)に対して成功する
        let state = EquipmentInkriState::fresh(1000);
        let mut rng = InkriRng::new(4);
        let result = run_batch(
            state,
            InkriKind::Lord,
            InkriBatchMode::UntilSuccess { max_attempts: 1000 },
            &mut rng,
            None,
        );
        assert_eq!(result.attempts_made, 1);
        assert_eq!(result.successes, 1);
        assert_eq!(result.last_outcome, Some(InkriAttemptOutcome::Success));
    }
}
