//! ビアヌのインクリ(wiki: 装備システム/インクリ。クライアント DB
//! `db/dm_00000_0419.csv`(EncryptDataTemplate、ItemId 5600000〜5600005)。ユーザー確認 2026-09-17)。
//!
//! 装備の「インクリ回数」を 1 ずつ積み上げる消耗行為。6 種類あり、ロード / 加護 / 祝福 / 王室は
//! 失敗すると装備が破壊されるが、**ビアヌとエタインクリは失敗しても何も起きない**。成功率(10万分率)は
//! ロード 21000・加護 26000・祝福 31000・王室 36000・エタインクリ 1000 の固定値で、ビアヌだけ
//! それまでの成功回数 `inkri_count` に応じて `70 - 5 * inkri_count`(下限 10、= n≥12 で 0.010%)まで下がる。
//! エタインクリ(ItemId 5600005)はエタレベルが装備条件の装備(セイクリッド系)でだけ使え、
//! SEED のほかに「エタインクリ呪文書」を 1 回 1 枚消費する(wiki「エタインクリ費用」節、2026-09-18 追加)。
//!
//! ゲームでは成功のたびに装備の合成回数も 1 減り、1/4 まで減ると止まるが、このシミュレータは
//! 合成回数を追わない — 「n 回目の成功までに何回・いくらかかるか」を積み上げて見るための道具で、
//! 装備側の残量は利用者が知っている(ユーザー判断 2026-09-18)。

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
    /// エタインクリ(ItemId 5600005)。失敗しても装備は破壊されない。エタレベル装備専用。
    Eta,
}

impl InkriKind {
    pub const ALL: [InkriKind; 6] = [
        InkriKind::Lord,
        InkriKind::Grace,
        InkriKind::Blessing,
        InkriKind::Royal,
        InkriKind::Vianu,
        InkriKind::Eta,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InkriKind::Lord => "ロードのインクリ",
            InkriKind::Grace => "加護のインクリ",
            InkriKind::Blessing => "祝福のインクリ",
            InkriKind::Royal => "王室のインクリ",
            InkriKind::Vianu => "ビアヌのインクリ",
            InkriKind::Eta => "エタインクリ",
        }
    }

    /// wiki の確率表記(低確率/中確率/極めて低確率)。実際の成功率は `success_rate` で持つ。
    pub fn probability_label(self) -> &'static str {
        match self {
            InkriKind::Lord | InkriKind::Grace | InkriKind::Eta => "低確率",
            InkriKind::Blessing | InkriKind::Royal => "中確率",
            InkriKind::Vianu => "極めて低確率",
        }
    }

    /// 失敗時に装備が破壊されるか(ビアヌとエタインクリは破壊されない)。
    pub fn destroys_on_failure(self) -> bool {
        !matches!(self, InkriKind::Vianu | InkriKind::Eta)
    }

    /// 1 回ごとに「エタインクリ呪文書」を消費するか(エタインクリだけ)。
    pub fn consumes_scroll(self) -> bool {
        matches!(self, InkriKind::Eta)
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
            InkriKind::Eta => 1_000,
        }
    }
}

/// 「エタインクリ呪文書」1 枚の値段(wiki「装備システム/インクリ」エタインクリ費用節、2026-09-18)。
/// 3 つの商店で通貨が違うので 3 つとも持つ。TP はルイノの「エタインクリ袋」(100 枚入り 46,800TP)の 1 枚あたり。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EtaScrollPrice {
    /// ルーンの庭園「フォレスト」: 1 枚 2億 SEED
    pub seed: i64,
    /// ルーンの庭園「トードー」: 1 枚 3万 ELSO
    pub elso: i64,
    /// ナルビク / クラド フリーマーケット「ルイノ」: エタインクリ袋(100 枚)46,800TP → 1 枚 468TP
    pub tp: i64,
}

pub const ETA_SCROLL_PRICE: EtaScrollPrice = EtaScrollPrice {
    seed: 200_000_000,
    elso: 30_000,
    tp: 46_800 / 100,
};

/// 装備 1 個のインクリに関わる状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipmentInkriState {
    /// これまでの成功回数(ビアヌの成功率を下げる)
    pub inkri_count: i64,
    /// 破壊済みか
    pub destroyed: bool,
}

impl EquipmentInkriState {
    /// 未使用の初期状態(インクリ回数 0、未破壊)。
    pub fn fresh() -> Self {
        Self {
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
}

/// この状態でインクリを実行できるか。
pub fn check_can_attempt(state: EquipmentInkriState) -> Result<(), InkriBlockReason> {
    if state.destroyed {
        return Err(InkriBlockReason::Destroyed);
    }
    Ok(())
}

/// 1 回の試行の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InkriAttemptOutcome {
    /// 成功(インクリ回数 +1)
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
    /// 破壊されるまでの間、指定回数だけ試す
    Fixed { attempts: i64 },
    /// 成功する(または破壊される)まで、上限回数を超えない範囲で試す
    UntilSuccess { max_attempts: i64 },
}

/// 積み上げの 1 段 = ある回数から次の成功に向けた試行のかたまり。
/// `succeeded` が false の段はまだ成功していない(まとめ試行の末尾、または破壊で終わった)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InkriStep {
    /// この段に入った時点のインクリ回数(0 なら 1 回目の成功を目指す段)
    pub from_count: i64,
    pub attempts: i64,
    pub succeeded: bool,
    /// この段で使った SEED(費用未収録なら `None`)。種類を途中で替えても段ごとの額が狂わないよう段が持つ
    pub seed: Option<i64>,
}

/// まとめ試行の結果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InkriBatchResult {
    pub attempts_made: i64,
    pub successes: i64,
    pub destroyed: bool,
    pub final_state: EquipmentInkriState,
    pub last_outcome: Option<InkriAttemptOutcome>,
    /// 消費 SEED(`seed_cost_per_attempt` が `None` = 費用未収録のときは `None`)
    pub consumed_seed: Option<i64>,
    /// 今回の試行を段ごとに割った内訳(成功で段が閉じる。最後の段だけ開いたままのことがある)
    pub steps: Vec<InkriStep>,
}

/// N 回、または成功するまで(上限回数つき)試す。破壊された時点で打ち切る。
/// `seed_cost_per_attempt` は gamedata 側のビアヌ費用(未収録装備は `None`)。
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
    let mut steps: Vec<InkriStep> = Vec::new();
    for _ in 0..limit.max(0) {
        if check_can_attempt(state).is_err() {
            break;
        }
        let roll = rng.next_roll();
        let (next_state, outcome) =
            attempt(state, kind, roll).expect("check_can_attempt を通したあとなので必ず Ok");
        let success = matches!(outcome, InkriAttemptOutcome::Success);
        match steps.last_mut() {
            Some(step) if !step.succeeded => {
                step.attempts += 1;
                step.succeeded = success;
                step.seed = seed_cost_per_attempt.map(|cost| cost * step.attempts);
            }
            _ => steps.push(InkriStep {
                from_count: state.inkri_count,
                attempts: 1,
                succeeded: success,
                seed: seed_cost_per_attempt,
            }),
        }
        state = next_state;
        attempts_made += 1;
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
        steps,
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
    fn エタインクリは1パーセント固定で破壊されない() {
        assert_eq!(InkriKind::Eta.success_rate(0), 1_000);
        assert_eq!(InkriKind::Eta.success_rate(20), 1_000);
        assert!(!InkriKind::Eta.destroys_on_failure());
        assert!(InkriKind::Eta.consumes_scroll());
        let state = EquipmentInkriState::fresh();
        let (next, outcome) = attempt(state, InkriKind::Eta, 99_999).unwrap();
        assert_eq!(outcome, InkriAttemptOutcome::FailureNoChange);
        assert_eq!(next, state);
    }

    #[test]
    fn ビアヌの成功率は13段で下限10に張り付く() {
        let expected = [70, 65, 60, 55, 50, 45, 40, 35, 30, 25, 20, 15, 10];
        for (n, &rate) in expected.iter().enumerate() {
            assert_eq!(InkriKind::Vianu.success_rate(n as i64), rate, "n={n}");
        }
        // n=12 以降は下限 10 に張り付く
        assert_eq!(InkriKind::Vianu.success_rate(13), 10);
        assert_eq!(InkriKind::Vianu.success_rate(100), 10);
    }

    #[test]
    fn 成功するとインクリ回数が増える() {
        let state = EquipmentInkriState::fresh();
        let (next, outcome) = attempt(state, InkriKind::Lord, 0).unwrap();
        assert_eq!(outcome, InkriAttemptOutcome::Success);
        assert_eq!(next.inkri_count, 1);
        assert!(!next.destroyed);
    }

    #[test]
    fn 固定4種は失敗すると破壊される() {
        let state = EquipmentInkriState::fresh();
        let (next, outcome) = attempt(state, InkriKind::Lord, 99_999).unwrap();
        assert_eq!(outcome, InkriAttemptOutcome::FailureDestroyed);
        assert!(next.destroyed);
        assert_eq!(next.inkri_count, 0, "破壊時はインクリ回数を変えない");
    }

    #[test]
    fn ビアヌは失敗しても何も変わらない() {
        let state = EquipmentInkriState::fresh();
        let (next, outcome) = attempt(state, InkriKind::Vianu, 99_999).unwrap();
        assert_eq!(outcome, InkriAttemptOutcome::FailureNoChange);
        assert_eq!(next, state);
    }

    #[test]
    fn 破壊済みは実行できない() {
        let state = EquipmentInkriState {
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
    fn 破壊された時点で打ち切る() {
        // シード1の初手 roll=88969 は Royal(36000)に対して失敗 = 破壊
        let state = EquipmentInkriState::fresh();
        let mut rng = InkriRng::new(1);
        let result = run_batch(
            state,
            InkriKind::Royal,
            InkriBatchMode::Fixed { attempts: 10 },
            &mut rng,
            None,
        );
        assert_eq!(result.attempts_made, 1);
        assert!(result.destroyed);
        assert_eq!(
            result.steps,
            vec![InkriStep {
                from_count: 0,
                attempts: 1,
                succeeded: false,
                seed: None,
            }]
        );
    }

    #[test]
    fn シード固定で結果が再現する() {
        let state = EquipmentInkriState::fresh();
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
        let state = EquipmentInkriState::fresh();
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
        let state = EquipmentInkriState::fresh();
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
        let state = EquipmentInkriState::fresh();
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
        assert_eq!(
            result.steps,
            vec![InkriStep {
                from_count: 0,
                attempts: 1,
                succeeded: true,
                seed: None,
            }]
        );
    }

    #[test]
    fn 段の内訳は成功で閉じ末尾だけ開いたまま() {
        // ビアヌを十分な回数回すと、段の合計 = 試行回数、閉じた段の数 = 成功回数、
        // from_count は入った時点の回数から 1 ずつ増える
        let state = EquipmentInkriState {
            inkri_count: 3,
            destroyed: false,
        };
        let mut rng = InkriRng::new(2026);
        let result = run_batch(
            state,
            InkriKind::Vianu,
            InkriBatchMode::Fixed { attempts: 20_000 },
            &mut rng,
            Some(1),
        );
        assert!(result.successes >= 2, "successes={}", result.successes);
        let total: i64 = result.steps.iter().map(|s| s.attempts).sum();
        assert_eq!(total, result.attempts_made);
        let seed: i64 = result.steps.iter().map(|s| s.seed.unwrap()).sum();
        assert_eq!(Some(seed), result.consumed_seed);
        let closed = result.steps.iter().filter(|s| s.succeeded).count() as i64;
        assert_eq!(closed, result.successes);
        for (i, step) in result.steps.iter().enumerate() {
            assert_eq!(step.from_count, 3 + i as i64);
        }
        // 開いた段があるなら最後の 1 つだけ
        let open_positions: Vec<usize> = result
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.succeeded)
            .map(|(i, _)| i)
            .collect();
        assert!(open_positions.len() <= 1);
        if let Some(&pos) = open_positions.first() {
            assert_eq!(pos, result.steps.len() - 1);
        }
        assert_eq!(result.final_state.inkri_count, 3 + result.successes);
    }
}
