//! 中ディレイ(スキルの発動間隔)。出典: wiki 計算式まとめ `#ActualDelay` /
//! ステータス「中ディレイ倍率A / 倍率B」/ 装備システム/シエナのオーラ(取得 2026-08-25)。
//!
//! ```text
//! 中ディレイ = 基本中ディレイ × (1 − 中ディレイ減少値) × (2 コンボ以上なら 0.5)   (下限 0.3s)
//! ```
//!
//! - **基本中ディレイ**は wiki スキル性能一覧の「動作」列(`Skill::base_actual_delay`)
//! - **中ディレイ減少値**(= 倍率B の減少分)は上限 70%。供給源は極限スキル「フルスロットル」/
//!   カフス(盾+)のランダムオプション / シエナのオーラの追加オプション / キャラのパッシブ・マスタリー
//! - **コンボボーナス**(2 コンボ以上で ×0.5)は倍率A で、減少値の上限 70% の**対象外**。
//!   ただし**チャネリング技(区分 `続`)には掛けない**(ADR-019。持続だけ半分になって
//!   攻撃回数が据え置きだと DPS がほぼ 2 倍になるが、効くという根拠が無い)
//! - wiki が「(固定)」と書いている中ディレイ(極・ギガブレイズ 等)は減少が効かない

use serde::{Deserialize, Serialize};

/// 中ディレイ減少値の上限(wiki `#ActualDelay`「中ディレイ減少値の上限は70%」。
/// = ステータス「中ディレイ倍率B (初期値:100%、下限30%)」)。
pub const ACTUAL_DELAY_REDUCTION_MAX: f64 = 0.70;
/// 中ディレイの下限(wiki `#ActualDelay`「中ディレイの下限は0.3s」)。
pub const ACTUAL_DELAY_MIN: f64 = 0.3;
/// コンボボーナスの倍率A(wiki ステータス「中ディレイ倍率A / コンボ / 2コンボ以上のコンボボーナス −50%」)。
const COMBO_DELAY_RATE: f64 = 0.5;
/// コンボボーナスが付くコンボ数。
pub const COMBO_DELAY_THRESHOLD: u32 = 2;
/// 実測表の計測時間(秒)。表は「60 秒あたりのスキル回数」(ユーザー確定 2026-08-25)。
pub const SECONDS_PER_MINUTE: f64 = 60.0;

/// 中ディレイ減少の供給源 1 件(トレース表示用)。値は Σ% の小数表現(−5% → 0.05)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActualDelayContribution {
    pub source: String,
    pub rate: f64,
}

/// 中ディレイの内訳。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActualDelay {
    /// 基本中ディレイ(秒)。wiki スキル性能一覧の「動作」列
    pub base: f64,
    /// 上限を掛ける前の中ディレイ減少値
    pub reduction_raw: f64,
    /// 上限(70%)適用後の中ディレイ減少値
    pub reduction: f64,
    /// 倍率A(コンボボーナス)。2 コンボ以上で 0.5
    pub combo_rate: f64,
    /// 下限 0.3s を掛ける前の中ディレイ(秒)。`基本 × (1 − 減少) × 倍率A`
    pub raw: f64,
    /// チャージ時間(秒)。中ディレイ減少も倍率A も効かないので、下限を取ったあとに足す。
    /// チャージしない技は 0
    pub charge: f64,
    /// 中ディレイ(秒)。下限 0.3s 適用後 + チャージ時間
    pub value: f64,
    /// 下限 0.3s で頭打ちになったか
    pub floored: bool,
    /// wiki が「(固定)」と書いている中ディレイ(減少が効かない)
    pub fixed: bool,
    /// 中ディレイ減少の供給源の内訳
    pub contributions: Vec<ActualDelayContribution>,
    /// 60 秒あたりのスキル回数。実測表(`SkillUsesTable`)の格子に収まれば実測値、
    /// 外なら `60 / value`(式)。DPS はこの回数から出す
    pub uses_per_minute: f64,
    /// `uses_per_minute` が実測表由来か(`false` = 式から出した)
    pub uses_measured: bool,
}

/// 中ディレイを出す。`contributions` は減少値の供給源(Σ% の小数表現)。
///
/// `uses` は実測のスキル回数表。格子に収まるときは 60 秒あたりの回数をそこから引く
/// (式の `60 / 中ディレイ` は overhead を含まないので実測より 3〜14% 多く出る)。
/// コンボボーナス(2 コンボ以上で ×0.5)は実測表に無いので、その場合は式で出す。
///
/// `charge_seconds` はチャージで段数を伸ばす技(`Skill::full_charge`)がチャージに費やす時間。
/// 中ディレイ減少も下限も効かず 1 回の所要時間に丸ごと乗るので、下限を取ったあとに足す。
/// 実測のスキル回数表はチャージ無しの計測なので、チャージしているときは式で出す。
///
/// `channeling` はチャネリング技(区分 `続`。中ディレイ = 撃ち続ける持続時間)か。
/// **コンボボーナス(倍率A ×0.5)は掛けない** —— 持続を半分にすると攻撃回数(tick 10)は
/// そのままで DPS がほぼ 2 倍になるが、コンボが反復周期に効くという根拠が無い(ADR-019)。
pub fn actual_delay(
    base: f64,
    fixed: bool,
    contributions: Vec<ActualDelayContribution>,
    combo_count: u32,
    uses: &SkillUsesTable,
    charge_seconds: f64,
    channeling: bool,
) -> ActualDelay {
    let reduction_raw: f64 = contributions.iter().map(|c| c.rate).sum();
    // 「(固定)」の中ディレイには減少が乗らない(コンボボーナスは倍率A なので別枠)
    let reduction = if fixed {
        0.0
    } else {
        reduction_raw.min(ACTUAL_DELAY_REDUCTION_MAX)
    };
    let combo_rate = if combo_count >= COMBO_DELAY_THRESHOLD && !channeling {
        COMBO_DELAY_RATE
    } else {
        1.0
    };
    let raw = base * (1.0 - reduction) * combo_rate;
    let floored = raw.max(ACTUAL_DELAY_MIN);
    let charge = charge_seconds.max(0.0);
    let value = floored + charge;
    // 実測表はコンボボーナス無し・チャージ無し・減少が効くスキルの計測なので、その条件のときだけ使う
    let measured = (!fixed && combo_rate == 1.0 && charge == 0.0)
        .then(|| uses.uses_per_minute(base, reduction))
        .flatten();
    ActualDelay {
        base,
        reduction_raw,
        reduction,
        combo_rate,
        raw,
        charge,
        value,
        floored: floored > raw,
        fixed,
        contributions,
        uses_per_minute: measured.unwrap_or(SECONDS_PER_MINUTE / value),
        uses_measured: measured.is_some(),
    }
}

/// 魔法人形(アナイスのミカベア / ルシベア)の攻撃間隔の固定オーバーヘッド(秒)。
/// wiki 計算式まとめ「熊の攻撃間隔 = 基本中ディレイ × (1 − 中ディレイ減少値) + 0.0705秒」
/// (2026-09-18 取得)。本体の実測回数表(`SkillUsesTable`)は本体プレイヤーの実測なので
/// 熊には使わない(常に式で出す)。
const SUMMON_DELAY_OVERHEAD: f64 = 0.0705;

/// 熊(魔法人形)の 60 秒あたりのスキル回数。コンボボーナスは乗らない(熊はコンボしない)ので
/// `combo_rate` は考慮しない。`delay_reduction` は上限 70% 適用後の値を渡す。
pub fn summon_uses_per_minute(base_delay_seconds: f64, delay_reduction: f64) -> f64 {
    SECONDS_PER_MINUTE / (base_delay_seconds * (1.0 - delay_reduction) + SUMMON_DELAY_OVERHEAD)
}

/// 実測のスキル回数表(**60 秒あたり**)。行 = 総中ディレイ減少 %、列 = 基本中ディレイ(秒)。
///
/// wiki `#ActualDelay` 自身が「中ディレイ減少値を特定の値まで上げると、スキルの発動頻度が
/// 急激に変化する場合がある(丸め処理や通信頻度、入力間隔の都合)」と書いているとおり、
/// 実際の発動回数は `60 / 中ディレイ` にならない(詠唱・入力の overhead で 3〜14% 少ない)。
/// 式より実測のほうが正確なので、**格子の中に収まるときは実測回数で DPS を出す**。
/// 実データは gamedata(ユーザー提供の計測表)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillUsesTable {
    /// 総中ディレイ減少 %(昇順)
    pub reduction_percents: Vec<f64>,
    /// 基本中ディレイ(秒。昇順)
    pub base_delays: Vec<f64>,
    /// `uses[減少 %][基本中ディレイ]` = 60 秒あたりのスキル回数
    pub uses: Vec<Vec<f64>>,
}

/// 昇順の格子 `axis` の中で `value` を挟む 2 点の添字と、その間の比を返す。
/// 格子の外なら `None`(呼び出し側は式にフォールバックする)。
fn bracket(axis: &[f64], value: f64) -> Option<(usize, usize, f64)> {
    let first = *axis.first()?;
    let last = *axis.last()?;
    if value < first || value > last {
        return None;
    }
    let upper = axis.iter().position(|a| *a >= value)?;
    if upper == 0 {
        return Some((0, 0, 0.0));
    }
    let lower = upper - 1;
    let span = axis[upper] - axis[lower];
    let ratio = if span == 0.0 {
        0.0
    } else {
        (value - axis[lower]) / span
    };
    Some((lower, upper, ratio))
}

impl SkillUsesTable {
    /// 60 秒あたりのスキル回数(格子の内側は 2 次元の線形補間)。
    /// 基本中ディレイか総減少値が格子の外なら `None`。
    pub fn uses_per_minute(&self, base_delay: f64, reduction: f64) -> Option<f64> {
        let (r0, r1, rt) = bracket(&self.reduction_percents, reduction * 100.0)?;
        let (d0, d1, dt) = bracket(&self.base_delays, base_delay)?;
        let at = |r: usize, d: usize| self.uses.get(r).and_then(|row| row.get(d)).copied();
        let lower = at(r0, d0)? + (at(r0, d1)? - at(r0, d0)?) * dt;
        let upper = at(r1, d0)? + (at(r1, d1)? - at(r1, d0)?) * dt;
        Some(lower + (upper - lower) * rt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(source: &str, rate: f64) -> ActualDelayContribution {
        ActualDelayContribution {
            source: source.to_string(),
            rate,
        }
    }

    /// 実測表を使わないテスト用の空表(格子が無いので必ず式にフォールバックする)。
    fn no_table() -> SkillUsesTable {
        SkillUsesTable {
            reduction_percents: Vec::new(),
            base_delays: Vec::new(),
            uses: Vec::new(),
        }
    }

    /// ユーザー提供の計測表の一部(総減少 48% / 64% × 基本 0.8s / 1.6s)。
    fn table() -> SkillUsesTable {
        SkillUsesTable {
            reduction_percents: vec![48.0, 64.0],
            base_delays: vec![0.8, 1.6],
            uses: vec![vec![135.0, 69.0], vec![176.0, 100.0]],
        }
    }

    // wiki 計算式まとめ: 熊の攻撃間隔 = 基本中ディレイ × (1 − 中ディレイ減少値) + 0.0705秒
    #[test]
    fn 熊の攻撃間隔は基本中ディレイに0_0705秒を足した式で出す() {
        // 減少 0、基本 1s → 60 / 1.0705
        assert!((summon_uses_per_minute(1.0, 0.0) - 60.0 / 1.0705).abs() < 1e-9);
        // 減少 0.35、基本 0.8s → 60 / (0.8×0.65 + 0.0705)
        assert!((summon_uses_per_minute(0.8, 0.35) - 60.0 / (0.8 * 0.65 + 0.0705)).abs() < 1e-9);
    }

    // wiki `#ActualDelay`: 中ディレイ = 基本 × (1 − 減少値) × (2 コンボ以上なら 0.5)
    #[test]
    fn 減少値とコンボボーナスが掛かる() {
        let d = actual_delay(1.4, false, vec![c("フルスロットル", 0.45)], 0, &no_table(), 0.0, false);
        assert!((d.value - 1.4 * 0.55).abs() < 1e-12);
        assert_eq!(d.combo_rate, 1.0);

        // 2 コンボ以上でさらに半分
        let d = actual_delay(1.4, false, vec![c("フルスロットル", 0.45)], 2, &no_table(), 0.0, false);
        assert!((d.value - 1.4 * 0.55 * 0.5).abs() < 1e-12);
    }

    /// チャネリング技(区分 `続`)の中ディレイは「撃ち続ける持続時間」なので、コンボの
    /// 倍率A を掛けない(持続だけ半分になって攻撃回数は tick 10 のまま = DPS 2 倍を防ぐ)。
    #[test]
    fn チャネリング技にはコンボボーナスを掛けない() {
        let d = actual_delay(10.0, false, Vec::new(), 2, &no_table(), 0.0, true);
        assert_eq!(d.combo_rate, 1.0);
        assert!((d.value - 10.0).abs() < 1e-12);
        // 中ディレイ減少は効く(公式 2025-10-29 の追加案内)
        let reduced = actual_delay(10.0, false, vec![c("フルスロットル", 0.45)], 2, &no_table(), 0.0, true);
        assert!((reduced.value - 5.5).abs() < 1e-12);
    }

    // wiki `#ActualDelay`: 減少値の上限 70%。コンボボーナスは対象外
    #[test]
    fn 減少値は70パーセントで頭打ち() {
        let d = actual_delay(
            1.4,
            false,
            vec![c("A", 0.45), c("B", 0.30), c("C", 0.05)],
            0,
            &no_table(),
            0.0,
            false,
        );
        assert!((d.reduction_raw - 0.80).abs() < 1e-12);
        assert_eq!(d.reduction, 0.70);
        assert!((d.value - 1.4 * 0.30).abs() < 1e-12);
    }

    // wiki `#ActualDelay`: 中ディレイの下限は 0.3s
    #[test]
    fn 下限は0_3秒() {
        let d = actual_delay(0.8, false, vec![c("A", 0.70)], 2, &no_table(), 0.0, false);
        // 0.8 × 0.30 × 0.5 = 0.12 → 下限 0.3
        assert_eq!(d.value, ACTUAL_DELAY_MIN);
        assert!(d.floored);

        let d = actual_delay(1.4, false, Vec::new(), 0, &no_table(), 0.0, false);
        assert!(!d.floored);
    }

    // wiki スキル性能一覧の「(固定)」は減少が効かない(極・ギガブレイズ 等)
    #[test]
    fn 固定の中ディレイには減少が乗らない() {
        let d = actual_delay(0.8, true, vec![c("フルスロットル", 0.45)], 0, &no_table(), 0.0, false);
        assert_eq!(d.reduction, 0.0);
        assert!((d.value - 0.8).abs() < 1e-12);
        // コンボボーナス(倍率A)は固定でも掛かる
        let d = actual_delay(0.8, true, vec![c("フルスロットル", 0.45)], 3, &no_table(), 0.0, false);
        assert!((d.value - 0.4).abs() < 1e-12);
    }

    // チャージ(スレイ・アックス / クラッシュ・アックス)は中ディレイ減少も下限も効かず、
    // 1 回の所要時間に丸ごと乗る。実測表はチャージ無しの計測なので式で出す。
    #[test]
    fn チャージ時間は減少も下限も受けずに足される() {
        let d = actual_delay(1.8, false, vec![c("A", 0.50)], 0, &no_table(), 1.0, false);
        assert_eq!(d.charge, 1.0);
        assert!((d.value - (1.8 * 0.50 + 1.0)).abs() < 1e-12);
        // 下限 0.3s はチャージを足す前に取る(チャージぶんが目減りしない)
        let floored = actual_delay(0.8, false, vec![c("A", 0.70)], 2, &no_table(), 0.5, false);
        assert!(floored.floored);
        assert!((floored.value - (ACTUAL_DELAY_MIN + 0.5)).abs() < 1e-12);
        // 実測表の格子に収まっていてもチャージ中は式(表はチャージ無しの計測)
        let charged = actual_delay(0.8, false, vec![c("A", 0.48)], 0, &table(), 1.0, false);
        assert!(!charged.uses_measured);
        assert!((charged.uses_per_minute - 60.0 / charged.value).abs() < 1e-9);
    }

    // ユーザー提供の計測表(60 秒あたりのスキル回数)。格子の中は実測、外は式。
    #[test]
    fn 実測表の格子の中は実測回数で外は式にフォールバックする() {
        // 総減少 48%(フルスロットル 45% + カフス RO 3%)・基本 0.8s → 実測 135 回/分
        let d = actual_delay(0.8, false, vec![c("A", 0.48)], 0, &table(), 0.0, false);
        assert!(d.uses_measured);
        assert!((d.uses_per_minute - 135.0).abs() < 1e-9);
        // 式なら 60 / (0.8 × 0.52) = 144.2 回/分。実測のほうが少ない(詠唱・入力の overhead)
        assert!(d.uses_per_minute < 60.0 / d.value);

        // 格子の外(総減少 20%)は式で出す
        let d = actual_delay(0.8, false, vec![c("A", 0.20)], 0, &table(), 0.0, false);
        assert!(!d.uses_measured);
        assert!((d.uses_per_minute - 60.0 / d.value).abs() < 1e-9);
    }

    #[test]
    fn 実測表は縦横とも線形補間する() {
        // 基本 1.2s は 0.8s(135)と 1.6s(69)の中点 → 102
        let d = actual_delay(1.2, false, vec![c("A", 0.48)], 0, &table(), 0.0, false);
        assert!((d.uses_per_minute - 102.0).abs() < 1e-9);
        // 総減少 56% は 48%(135)と 64%(176)の中点 → 155.5
        let d = actual_delay(0.8, false, vec![c("A", 0.56)], 0, &table(), 0.0, false);
        assert!((d.uses_per_minute - 155.5).abs() < 1e-9);
    }

    // 実測表はコンボボーナス無し・減少が効くスキルの計測なので、その条件を外れたら式で出す
    #[test]
    fn コンボボーナスと固定中ディレイでは実測表を使わない() {
        let combo = actual_delay(0.8, false, vec![c("A", 0.48)], 2, &table(), 0.0, false);
        assert!(!combo.uses_measured);
        let fixed = actual_delay(0.8, true, vec![c("A", 0.48)], 0, &table(), 0.0, false);
        assert!(!fixed.uses_measured);
    }
}
