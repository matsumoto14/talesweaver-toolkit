//! 回し(連打する技 1 つ + 差し込む CT 技 0〜数個)の役割・時間配分・DPS。
//!
//! CT のある技は撃ってから CT が明けるまで撃てないので、その間は連打技を撃っている。
//! 差し込む技 i(1 回 `c_i` 秒・クールタイム `CT_i`)と連打技(1 回 `c_f` 秒)について
//!
//! ```text
//! k_i = max(n_i, ceil((CT_i − c_i) / c_f))   // 連打技を挟む回数
//! T_i = c_i + k_i × c_f                       // 差し込む技 1 回あたりの間隔(≥ CT_i)
//! DPS = Σ d_i / T_i + clamp0(1 − Σ c_i / T_i) / c_f × d_f
//! ```
//!
//! `n_i` は「撃つ前に連打技を最低何回挟むか」(イェフネンの <フラグ> を積み直す回数)。
//! 差し込む技が占める時間 `Σ c_i / T_i` を 1 から引いた残りが連打技に回る。
//!
//! ここは**役割の決め方と回数・時間の規則**だけを持つ。候補が同じキャラ・同じ形態かの
//! 絞り込みと、1 回の所要時間・ダメージの計算は呼び出し側(commands)がする。

use crate::damage::{DamageResult, DamageTriple, DpsTriple};
use crate::skill::Skill;

/// 実効クールタイム。**1 回の所要時間以下の CT は連打を妨げない**ので「CT なし」として扱う
/// (中ディレイのほうが長い技 — 撃ち切るのに CT より時間がかかるチャネリング技や、
/// CT が 1 秒しかない技)。所要時間が出せない(中ディレイ未収録の)技は CT をそのまま返す。
///
/// 計算タブもホームの到達一覧もこの 1 本で判定する(基準は実際の 1 回の所要時間 =
/// `DamageResult::cycle_seconds`。コンボ中は通常攻撃を挟んだ 1 サイクル)。
pub fn effective_cooldown_seconds(skill: &Skill, seconds: Option<f64>) -> Option<f64> {
    let cooldown = skill.cooldown_seconds?;
    match seconds {
        Some(seconds) if cooldown <= seconds => None,
        _ => Some(cooldown),
    }
}

/// 回しの役割を選ぶときの候補 1 件(同じキャラ・同じ形態のプレイヤー攻撃技。主軸は含めない)。
#[derive(Debug, Clone, Copy)]
pub struct RotationCandidate<'a> {
    pub skill: &'a Skill,
    /// 1 回撃つのにかかる時間(秒)。中ディレイ未収録なら `None`
    pub seconds: Option<f64>,
    /// **主軸を連打するとき**に差し込んでよいか。<フラグ> を爆発させる技は、主軸で積んだ
    /// ぶんを使う技だけ `true`(判定は gamedata 側)。連打技の候補としてはこの印を見ない
    pub insertable: bool,
    /// 1 回で同時に入るダメージ(技本体と、それが起こす <フラグ> の爆発)。
    /// 差し込むと DPS が上がるかを**この場で**確かめるために要る
    pub damage: &'a [RotationDamage],
    /// 撃つ前に連打技を最低何回挟むか(<フラグ> の積み直し)。他は 0
    pub minimum_filler_uses: u32,
}

/// 回しの中での立ち位置。`Main` は主軸そのもの。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationRole {
    Main,
    /// `candidates` の index
    Candidate(usize),
}

/// どの技を連打し、どの技を差し込むか。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationRoles {
    /// 連打する技。`None` = 連打できる技が無い(CT が明くのを待つだけ)
    pub filler: Option<RotationRole>,
    /// 差し込む CT 技(既定)。空なら回しにならない
    pub inserts: Vec<RotationRole>,
}

/// 候補 1 件を差し込んだときの期待 DPS の差。連打技だけの回しと比べる(負なら下がる)。
/// 差し込めない(所要時間が出せない・実効 CT が無い・積み直せない)なら `None`。
pub fn candidate_expected_dps_gain(
    candidate: &RotationCandidate<'_>,
    filler: Option<(RotationDamage, f64)>,
) -> Option<f64> {
    let filler_seconds = filler.map(|(_, seconds)| seconds);
    let inserts = [RotationInsert {
        seconds: candidate.seconds,
        cooldown_seconds: candidate.skill.cooldown_seconds?,
        minimum_filler_uses: candidate.minimum_filler_uses,
    }];
    let damages = [candidate.damage];
    let plan = plan_rotation(filler_seconds, &inserts)?;
    let (_, expected) = rotation_dps(&plan, &damages, filler)?;
    insert_expected_dps_gain(expected, filler_seconds, &inserts, &damages, filler, 0)
}

/// 主軸と候補から役割を決める(計算タブ・ホームで共通の規則)。
///
/// - 主軸に実効 CT が無ければ**主軸が連打技**で、差し込むと DPS が**上がる**候補のうち
///   `Skill::power`(倍率 × 段数 × tick 数)が最大の 1 つを差し込む。上がる候補が無ければ
///   差し込まない(既定で下がる技を ON にしない。ADR-019、ユーザー決定 2026-09-21)
/// - 主軸に実効 CT があれば**主軸が差し込む技**で、連打技は実効 CT の無い候補から選ぶ。
///   並びは主軸候補と同じ(単体優先 → 継続火力順。`Skill::main_skill_order`)
/// - `pinned_filler_id` は <フラグ> の積み直しで連打技が決まっているとき(同形態の 連 / 爆)。
///   その技が候補に無い・所要時間が出せないなら連打技なし
///
/// `main_damage` は主軸 1 回で同時に入るダメージ(連打技として撃つぶん)。損得の判定に使う。
pub fn choose_rotation(
    main: &Skill,
    main_seconds: Option<f64>,
    main_damage: Option<RotationDamage>,
    candidates: &[RotationCandidate<'_>],
    pinned_filler_id: Option<&str>,
) -> RotationRoles {
    // 連打技は 1 回の所要時間が出せないと使えない(何回挟めるか決まらない)
    let usable = |candidate: &RotationCandidate<'_>| candidate.seconds.is_some();
    if effective_cooldown_seconds(main, main_seconds).is_none() {
        // 主軸を連打しているところへ差し込むので、比べる相手は「主軸だけを撃ち続ける」回し
        let filler = main_damage.zip(main_seconds);
        let insert = candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.insertable
                    && usable(c)
                    && effective_cooldown_seconds(c.skill, c.seconds).is_some()
                    // 差し込むと下がる技は既定で ON にしない(画面が畳んだ先で出す)
                    && candidate_expected_dps_gain(c, filler).is_some_and(|gain| gain > 0.0)
            })
            .max_by(|(_, a), (_, b)| {
                a.skill
                    .power
                    .partial_cmp(&b.skill.power)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| RotationRole::Candidate(index));
        return RotationRoles {
            filler: Some(RotationRole::Main),
            inserts: insert.into_iter().collect(),
        };
    }
    let filler = match pinned_filler_id {
        Some(id) => candidates
            .iter()
            .position(|c| c.skill.id == id && usable(c)),
        None => candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                usable(c) && effective_cooldown_seconds(c.skill, c.seconds).is_none()
            })
            .min_by(|(_, a), (_, b)| Skill::main_skill_order(a.skill, b.skill))
            .map(|(index, _)| index),
    };
    RotationRoles {
        filler: filler.map(RotationRole::Candidate),
        inserts: vec![RotationRole::Main],
    }
}

/// 画面から明示された差し込み(`explicit`)を役割に当てる。`None` = 既定のまま、
/// `Some([])` = 差し込まない、`Some([id, …])` = その技だけ。
///
/// 主軸自身が差し込む技(主軸に実効 CT がある)のときは、主軸は「差し込まない」選択肢が
/// 無いので必ず残す。同じ技を 2 回入れない(主軸の id を指定されても重ならない)。
/// 候補に無い id は落とす(候補の並びは `candidate_ids` の index がそのまま
/// `RotationRole::Candidate` になる)。
///
/// 計算タブ(`commands::build_rotation`)もホーム評価(`content_evaluation`)もこの 1 本を通る。
pub fn apply_explicit_inserts(
    roles: &RotationRoles,
    explicit: Option<&[String]>,
    main_id: &str,
    candidate_ids: &[&str],
) -> Vec<RotationRole> {
    let Some(ids) = explicit else {
        return roles.inserts.clone();
    };
    let mut chosen: Vec<RotationRole> = roles
        .inserts
        .iter()
        .copied()
        .filter(|role| *role == RotationRole::Main)
        .collect();
    for id in ids {
        let role = if id == main_id {
            RotationRole::Main
        } else {
            match candidate_ids.iter().position(|c| *c == id) {
                Some(index) => RotationRole::Candidate(index),
                None => continue,
            }
        };
        if !chosen.contains(&role) {
            chosen.push(role);
        }
    }
    chosen
}

/// 差し込む CT 技 1 つぶんの時間(`c_i` / `CT_i` / `n_i`)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationInsert {
    /// 1 回撃つのにかかる時間(秒)= `c_i`。出せないなら `None`
    pub seconds: Option<f64>,
    /// クールタイム(秒)= `CT_i`
    pub cooldown_seconds: f64,
    /// 撃つ前に連打技を最低何回挟むか = `n_i`(<フラグ> の積み直し)。他は 0
    pub minimum_filler_uses: u32,
}

/// 差し込む技を撃つ間隔(`T_i`)を決めているもの。**画面はこの分類に文言を当てるだけ**で、
/// 「CT 律速か積み直し律速か」を回数や秒から推し量らない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationPace {
    /// CT が明くのを待っている(`n_i` より多く連打を挟んでいる / 連打技が無くて待つ)
    Cooldown,
    /// <フラグ> の積み直しで決まっている(CT はもう明けている)
    Reapply,
    /// 差し込みだけで時間が埋まり、頻度を縮めた(`crowded`)
    Crowded,
    /// 何にも縛られていない(撃ち終わったらすぐ次が撃てる)
    Free,
}

/// 差し込む技 1 回あたりの間隔に、何も入らない時間があるならその正体。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationIdle {
    /// CT が明くのを待っている(連打する技が無い)
    Wait,
    /// 他の差し込みを撃っている(`crowded`)
    OtherInserts,
}

/// 差し込む技 1 つぶんの答え(`k_i` / `T_i` と、その間隔の内訳)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationSlot {
    /// `inserts` のどれの答えか
    pub insert: usize,
    /// 1 回あたり挟む連打技の回数 = `k_i`。`crowded`(差し込みだけで時間が埋まる)なら
    /// 連打は入らないので 0
    pub filler_uses: u32,
    /// この技を撃つ間隔(秒)= `T_i`。CT より短くならない
    pub interval_seconds: f64,
    /// そのうち連打技が占める秒(= `k_i × c_f`)。連打が入らないなら 0
    pub filler_seconds: f64,
    /// そのうち何も入らない秒(CT 待ち・詰まって空いたぶん)。無ければ 0
    pub idle_seconds: f64,
    /// 何も入らない時間の正体。`idle_seconds` が 0 なら `None`
    pub idle: Option<RotationIdle>,
    /// 間隔を決めているもの(CT 律速 / 積み直し律速 / 詰まっている / 何もなし)
    pub pace: RotationPace,
}

/// 回し 1 周ぶんの時間配分。
#[derive(Debug, Clone, PartialEq)]
pub struct RotationPlan {
    /// 採用した差し込みぶん。撃てない技(所要時間が出せない・CT が所要時間以下・
    /// 連打技が無いのに積み直しが要る)は落ちている
    pub slots: Vec<RotationSlot>,
    /// 連打技に回る時間の割合(0〜1)。差し込む技で埋まっていれば 0
    pub filler_share: f64,
    /// 差し込む技だけで時間が埋まり、間隔を伸ばして詰めたか(全部は CT どおりに撃てない)
    pub crowded: bool,
}

/// 回しを組む。`filler_seconds` は連打技 1 回の所要時間(`None` = 連打する技が無い。
/// CT の待ち時間は空くだけになる)。
///
/// 撃てない差し込みは落とす(1 件だけ落として残りで組む)。1 つも残らなければ `None`
/// (回しを組まない = DPS は技そのものの値のまま)。
pub fn plan_rotation(
    filler_seconds: Option<f64>,
    inserts: &[RotationInsert],
) -> Option<RotationPlan> {
    let filler_seconds = filler_seconds.filter(|&seconds| seconds > 0.0);
    let mut slots: Vec<RotationSlot> = Vec::with_capacity(inserts.len());
    let mut occupied = 0.0;
    for (index, insert) in inserts.iter().enumerate() {
        // 所要時間が出せない技は回しに組み込めない
        let Some(seconds) = insert.seconds.filter(|&seconds| seconds > 0.0) else {
            continue;
        };
        // CT が所要時間以下なら連打できるので差し込みではない
        if insert.cooldown_seconds <= seconds {
            continue;
        }
        // 連打技が無いのに積み直しが要る技は撃てない(<フラグ> を積み直せない)
        if filler_seconds.is_none() && insert.minimum_filler_uses > 0 {
            continue;
        }
        let slot = match filler_seconds {
            Some(filler_seconds) => {
                // CT を満たすのに要る回数。CT 待ちの間は連打技を撃っている扱い
                let for_cooldown =
                    ((insert.cooldown_seconds - seconds) / filler_seconds).ceil() as u32;
                let filler_uses = insert.minimum_filler_uses.max(for_cooldown);
                RotationSlot {
                    insert: index,
                    filler_uses,
                    interval_seconds: seconds + f64::from(filler_uses) * filler_seconds,
                    filler_seconds: f64::from(filler_uses) * filler_seconds,
                    idle_seconds: 0.0,
                    idle: None,
                    pace: if for_cooldown > insert.minimum_filler_uses {
                        RotationPace::Cooldown
                    } else if insert.minimum_filler_uses > 0 {
                        RotationPace::Reapply
                    } else {
                        RotationPace::Free
                    },
                }
            }
            // 連打する技が無いときは CT が明けるのを待つだけ
            None => RotationSlot {
                insert: index,
                filler_uses: 0,
                interval_seconds: insert.cooldown_seconds,
                filler_seconds: 0.0,
                idle_seconds: insert.cooldown_seconds - seconds,
                idle: Some(RotationIdle::Wait),
                pace: RotationPace::Cooldown,
            },
        };
        occupied += seconds / slot.interval_seconds;
        slots.push(slot);
    }
    if slots.is_empty() {
        return None;
    }
    // 差し込む技だけで時間が埋まったら、全部を CT どおりには撃てない。頻度を等倍で縮めて
    // 合計の占有を 1 に収める(差し込みぶんも連打ぶんも過大にしない)
    let crowded = occupied > 1.0;
    if crowded {
        for slot in &mut slots {
            slot.interval_seconds *= occupied;
            // 差し込みだけで時間が埋まっているので、合間に連打は入らない
            // (`filler_share` も 0)。「合間に k 回」と噛み合わない数を残さない
            slot.filler_uses = 0;
            slot.filler_seconds = 0.0;
            // 空いた時間は他の差し込みを撃っている(待っているのではない)
            let seconds = inserts[slot.insert].seconds.unwrap_or(0.0);
            slot.idle_seconds = (slot.interval_seconds - seconds).max(0.0);
            slot.idle = Some(RotationIdle::OtherInserts);
            slot.pace = RotationPace::Crowded;
        }
    }
    Some(RotationPlan {
        slots,
        filler_share: if filler_seconds.is_some() && !crowded {
            1.0 - occupied
        } else {
            0.0
        },
        crowded,
    })
}

/// 回しに入る技 1 回ぶんのダメージ(側ごとの合計とクリ率)。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationDamage {
    /// 1 回ぶんの合計ダメージ。コンボ中は挟む通常攻撃ぶんを含む
    pub total: DamageTriple,
    pub critical_chance: f64,
}

impl RotationDamage {
    /// 技 1 回ぶんの計算結果から(`DamageResult::cycle_total`)。
    pub fn of(result: &DamageResult) -> Self {
        Self {
            total: result.cycle_total(),
            critical_chance: result.critical_chance,
        }
    }
}

/// 回しの中の 1 つ(差し込む技 1 件、または連打技)の取り分。**画面が技ごとの寄与を
/// 組み立て直さずに済むように、`rotation_dps` が足している項をそのまま 1 件ずつ返す。**
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationShare {
    /// 側(最小 / 最大 / クリ)ごとの DPS 寄与
    pub dps: DpsTriple,
    /// 期待 DPS の寄与。全部足すと回しの期待 DPS
    pub expected_dps: f64,
    /// 1 秒あたりに撃つ回数(差し込みは `1 / T_i`、連打は空いた時間ぶん)
    pub uses_per_second: f64,
}

/// 回しの取り分一式。`inserts` は `plan.slots` と同じ並び。
#[derive(Debug, Clone, PartialEq)]
pub struct RotationShares {
    pub inserts: Vec<RotationShare>,
    /// 連打技の取り分。連打する技が無いなら `None`
    pub filler: Option<RotationShare>,
}

impl RotationShares {
    /// 回し全体の DPS(側ごと)と期待値 = 取り分の総和。
    pub fn totals(&self) -> (DpsTriple, f64) {
        let mut dps = DpsTriple { min: 0.0, max: 0.0, critical: 0.0 };
        let mut expected = 0.0;
        for share in self.inserts.iter().chain(self.filler.iter()) {
            dps.min += share.dps.min;
            dps.max += share.dps.max;
            dps.critical += share.dps.critical;
            expected += share.expected_dps;
        }
        (dps, expected)
    }
}

/// 回しの取り分を技ごとに出す。`inserts` は `plan_rotation` に渡したのと同じ並びで、
/// 1 回に同時に入るダメージ(技本体と、それが起こす <フラグ> の爆発)をまとめて渡す。
/// `filler` は連打技 1 回ぶんとその所要時間。側(最小 / 最大 / クリ)ごとに足し、
/// 期待値は技ごとのクリ率で按分してから足す。
pub fn rotation_shares(
    plan: &RotationPlan,
    inserts: &[&[RotationDamage]],
    filler: Option<(RotationDamage, f64)>,
) -> Option<RotationShares> {
    let share = |damages: &[RotationDamage], uses_per_second: f64| {
        let mut dps = DpsTriple { min: 0.0, max: 0.0, critical: 0.0 };
        let mut expected = 0.0;
        for one in damages {
            dps.min += one.total.min as f64 * uses_per_second;
            dps.max += one.total.max as f64 * uses_per_second;
            dps.critical += one.total.critical as f64 * uses_per_second;
            expected += one.total.expected(one.critical_chance) * uses_per_second;
        }
        RotationShare {
            dps,
            expected_dps: expected,
            uses_per_second,
        }
    };
    let mut slots = Vec::with_capacity(plan.slots.len());
    for slot in &plan.slots {
        if slot.interval_seconds <= 0.0 {
            return None;
        }
        slots.push(share(inserts.get(slot.insert)?, 1.0 / slot.interval_seconds));
    }
    let filler = match filler {
        Some((damage, seconds)) => {
            if seconds <= 0.0 {
                return None;
            }
            Some(share(&[damage], plan.filler_share / seconds))
        }
        None => None,
    };
    Some(RotationShares { inserts: slots, filler })
}

/// 回しの DPS(側ごと)と期待値。技ごとの取り分(`rotation_shares`)の総和で、
/// **合計と内訳が食い違わない**ようにここは足すだけにする。
pub fn rotation_dps(
    plan: &RotationPlan,
    inserts: &[&[RotationDamage]],
    filler: Option<(RotationDamage, f64)>,
) -> Option<(DpsTriple, f64)> {
    Some(rotation_shares(plan, inserts, filler)?.totals())
}

/// その差し込みを**外した**回しとの期待 DPS の差(負なら「差し込むと下がる」)。
///
/// 外した回しは同じ材料をもう一度通すだけ(ダメージの計算はやり直さない)。差し込みが
/// 1 つも残らないなら、連打技だけを撃ち続ける DPS と比べる。比べようが無い
/// (連打技も残りの差し込みも無い)なら `None`。
pub fn insert_expected_dps_gain(
    expected_dps: f64,
    filler_seconds: Option<f64>,
    inserts: &[RotationInsert],
    damages: &[&[RotationDamage]],
    filler: Option<(RotationDamage, f64)>,
    skipped: usize,
) -> Option<f64> {
    let kept: Vec<RotationInsert> = inserts
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != skipped)
        .map(|(_, insert)| *insert)
        .collect();
    let kept_damages: Vec<&[RotationDamage]> = damages
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != skipped)
        .map(|(_, damage)| *damage)
        .collect();
    let without = match plan_rotation(filler_seconds, &kept) {
        Some(plan) => rotation_dps(&plan, &kept_damages, filler).map(|(_, expected)| expected)?,
        // 差し込みが残らないなら連打技を撃ち続けるだけ
        None => {
            let (damage, seconds) = filler.filter(|(_, seconds)| *seconds > 0.0)?;
            damage.total.expected(damage.critical_chance) / seconds
        }
    };
    Some(expected_dps - without)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{SkillDependency, SkillTarget};

    fn insert(seconds: f64, cooldown_seconds: f64, minimum_filler_uses: u32) -> RotationInsert {
        RotationInsert {
            seconds: Some(seconds),
            cooldown_seconds,
            minimum_filler_uses,
        }
    }

    fn skill(id: &str, power: f64, delay: f64, cooldown: Option<f64>) -> Skill {
        let mut skill = Skill::for_test(id, SkillDependency::Stab);
        skill.power = power;
        skill.base_actual_delay = Some(delay);
        skill.cooldown_seconds = cooldown;
        skill.target = Some(SkillTarget::Single);
        skill.power_per_second = Skill::compute_power_per_second(power, Some(delay), cooldown);
        skill
    }

    fn candidate<'a>(
        skill: &'a Skill,
        seconds: Option<f64>,
        damage: &'a [RotationDamage],
    ) -> RotationCandidate<'a> {
        RotationCandidate {
            skill,
            seconds,
            insertable: true,
            damage,
            minimum_filler_uses: 0,
        }
    }

    /// 積み直しに要る回数(`n_i`)は切り上げ済みの値で渡ってくる。CT を満たしていれば
    /// その回数のまま。スレイ(1.4s)+ 連(2.0s)× 5 = 11.4s は CT 10s を満たす。
    #[test]
    fn 挟む回数は積み直しとctの多いほう() {
        let plan = plan_rotation(Some(2.0), &[insert(1.4, 10.0, 5)]).unwrap();
        let slot = plan.slots[0];
        assert_eq!(slot.filler_uses, 5);
        // CT はもう明けているので、間隔を決めているのは積み直し
        assert_eq!(slot.pace, RotationPace::Reapply);
        assert_eq!(slot.idle, None);
        assert!((slot.filler_seconds - 10.0).abs() < 1e-12);
        assert!((slot.interval_seconds - 11.4).abs() < 1e-12);
        // 連打技に回るのは 1 周のうち主軸の 1.4s を除いたぶん
        assert!((plan.filler_share - (1.0 - 1.4 / 11.4)).abs() < 1e-12);
        assert!(!plan.crowded);
    }

    #[test]
    fn ctに満たないときは連打の回数で埋める() {
        let plan = plan_rotation(Some(1.0), &[insert(1.4, 10.0, 1)]).unwrap();
        let slot = plan.slots[0];
        assert_eq!(slot.pace, RotationPace::Cooldown);
        assert_eq!(slot.filler_uses, 9); // (10 − 1.4) / 1.0 の切り上げ
        // 1 周は「この技 + 連打ぶん」で埋まり、空き時間は無い
        assert!((slot.filler_seconds - 9.0).abs() < 1e-12);
        assert_eq!(slot.idle_seconds, 0.0);
        assert!(slot.interval_seconds >= 10.0);
        // 1 回減らすと CT 未満
        assert!((f64::from(slot.filler_uses - 1) + 1.4) < 10.0);
    }

    #[test]
    fn 間隔はctより短くならない() {
        for minimum in 0..=10u32 {
            let plan = plan_rotation(Some(0.8), &[insert(1.2, 10.0, minimum)]).unwrap();
            assert!(plan.slots[0].interval_seconds >= 10.0 - 1e-9, "n={minimum}");
        }
    }

    /// CT が 1 回の所要時間以下の技は連打できるので、回しに入れない。
    #[test]
    fn ctが所要時間以下なら差し込まない() {
        assert_eq!(plan_rotation(Some(2.0), &[insert(1.0, 1.0, 0)]), None);
        // 1 件だけ落として残りで組む
        let plan = plan_rotation(Some(1.0), &[insert(1.0, 1.0, 0), insert(2.0, 10.0, 0)]).unwrap();
        assert_eq!(plan.slots.len(), 1);
        assert_eq!(plan.slots[0].insert, 1);
    }

    /// 所要時間が出せない技も落とすだけで、回し全体は捨てない。
    #[test]
    fn 所要時間が出せない差し込みだけ外す() {
        let unknown = RotationInsert { seconds: None, cooldown_seconds: 10.0, minimum_filler_uses: 0 };
        let plan = plan_rotation(Some(1.0), &[unknown, insert(2.0, 10.0, 0)]).unwrap();
        assert_eq!(plan.slots.len(), 1);
        assert_eq!(plan.slots[0].insert, 1);
        assert_eq!(plan_rotation(Some(1.0), &[unknown]), None);
    }

    #[test]
    fn 差し込む技が2つなら占有時間を足す() {
        let plan = plan_rotation(Some(1.0), &[insert(2.0, 10.0, 0), insert(1.0, 20.0, 0)]).unwrap();
        // 1 つ目: (10 − 2) / 1 = 8 回 → 10s に 1 回
        assert_eq!(plan.slots[0].filler_uses, 8);
        assert!((plan.slots[0].interval_seconds - 10.0).abs() < 1e-12);
        // 2 つ目: (20 − 1) / 1 = 19 回 → 20s に 1 回
        assert_eq!(plan.slots[1].filler_uses, 19);
        assert!((plan.slots[1].interval_seconds - 20.0).abs() < 1e-12);
        // 連打技に回るのは 1 − (2/10 + 1/20)
        assert!((plan.filler_share - (1.0 - (0.2 + 0.05))).abs() < 1e-12);
    }

    /// 占有が 1 を超えたら、差し込みの頻度を等倍で縮めて合計を 1 に収める。
    #[test]
    fn 占有が1を超えたら間隔を伸ばして詰める() {
        let heavy = [insert(9.0, 10.0, 0), insert(9.0, 10.0, 0), insert(9.0, 10.0, 0)];
        let plan = plan_rotation(Some(1.0), &heavy).unwrap();
        assert!(plan.crowded);
        assert_eq!(plan.filler_share, 0.0);
        // 連打に回る時間が無いので、合間に挟む回数も 0
        assert!(plan.slots.iter().all(|slot| slot.filler_uses == 0));
        // 空いた時間は他の差し込みぶん(待っているのではない)
        for slot in &plan.slots {
            assert_eq!(slot.idle, Some(RotationIdle::OtherInserts));
            assert_eq!(slot.pace, RotationPace::Crowded);
            assert!((slot.idle_seconds - (slot.interval_seconds - 9.0)).abs() < 1e-12);
        }
        // 素の間隔は 3 件とも 10s(占有 2.7)→ 27s に 1 回ずつで占有はちょうど 1
        let occupied: f64 = plan
            .slots
            .iter()
            .map(|slot| 9.0 / slot.interval_seconds)
            .sum();
        assert!((occupied - 1.0).abs() < 1e-12);
        for slot in &plan.slots {
            assert!((slot.interval_seconds - 27.0).abs() < 1e-12);
        }
    }

    #[test]
    fn 連打する技が無ければctを待つだけ() {
        let plan = plan_rotation(None, &[insert(1.4, 10.0, 0)]).unwrap();
        assert_eq!(plan.slots[0].filler_uses, 0);
        assert!((plan.slots[0].interval_seconds - 10.0).abs() < 1e-12);
        assert_eq!(plan.filler_share, 0.0);
        // 撃ったあとは CT が明くまで待つだけ(待ち = 10 − 1.4)
        assert_eq!(plan.slots[0].idle, Some(RotationIdle::Wait));
        assert!((plan.slots[0].idle_seconds - 8.6).abs() < 1e-12);
        assert_eq!(plan.slots[0].pace, RotationPace::Cooldown);
        // 連打技が無いと <フラグ> を積み直せないので、その差し込みは撃てない
        assert_eq!(plan_rotation(None, &[insert(1.4, 10.0, 5)]), None);
    }

    #[test]
    fn 差し込む技が無ければ回しを組まない() {
        assert_eq!(plan_rotation(Some(1.0), &[]), None);
    }

    /// 1 回ぶんのダメージ(クリ率 0 = 期待値は最大乱数側)。
    fn damage(total: i64) -> RotationDamage {
        RotationDamage {
            total: DamageTriple { min: total, max: total, critical: total },
            critical_chance: 0.0,
        }
    }

    /// 差し込み 1 つの DPS は「(主軸 + 連打 × k) ÷ 間隔」と一致する。
    #[test]
    fn 差し込み1つのdpsは手計算と一致する() {
        let plan = plan_rotation(Some(1.0), &[insert(1.4, 10.0, 0)]).unwrap();
        let slot = plan.slots[0];
        let (dps, expected) =
            rotation_dps(&plan, &[&[damage(1000)][..]], Some((damage(100), 1.0))).unwrap();
        let hand = (1000.0 + 100.0 * f64::from(slot.filler_uses)) / slot.interval_seconds;
        assert!((dps.max - hand).abs() < 1e-9, "{dps:?} vs {hand}");
        assert!((expected - hand).abs() < 1e-9);
    }

    /// 差し込み 2 件でも、各技の頻度 × ダメージ + 連打の取り分で足し合わせる。
    #[test]
    fn 差し込み2件のdpsは頻度ごとに足す() {
        let plan = plan_rotation(Some(1.0), &[insert(2.0, 10.0, 0), insert(1.0, 20.0, 0)]).unwrap();
        let (_, expected) = rotation_dps(
            &plan,
            &[&[damage(1000)][..], &[damage(500)][..]],
            Some((damage(100), 1.0)),
        )
        .unwrap();
        let hand = 1000.0 / 10.0 + 500.0 / 20.0 + plan.filler_share / 1.0 * 100.0;
        assert!((expected - hand).abs() < 1e-9, "{expected} vs {hand}");
    }

    /// 占有が 1 を超えたら差し込み側も縮む(連打の取り分は 0)。
    #[test]
    fn 占有が1を超えたらdpsも縮む() {
        let heavy = [damage(900)];
        let inserts = [insert(9.0, 10.0, 0), insert(9.0, 10.0, 0), insert(9.0, 10.0, 0)];
        let plan = plan_rotation(Some(1.0), &inserts).unwrap();
        let parts: Vec<&[RotationDamage]> = vec![&heavy[..], &heavy[..], &heavy[..]];
        let (_, expected) = rotation_dps(&plan, &parts, Some((damage(100), 1.0))).unwrap();
        // 27s に 1 回ずつ × 3 件 = 900 × 3 / 27
        assert!((expected - 900.0 * 3.0 / 27.0).abs() < 1e-9, "{expected}");
        // 詰まっていない 1 件ぶんの 3 倍よりは小さい
        let single = plan_rotation(Some(1.0), &inserts[..1]).unwrap();
        let (_, loose) = rotation_dps(&single, &[&heavy[..]], Some((damage(100), 1.0))).unwrap();
        assert!(expected < loose * 3.0);
    }

    /// 技ごとの寄与を足すと回しの期待 DPS になり、時間の占有と連打の取り分を足すと 1 になる。
    #[test]
    fn 技ごとの取り分を足すと回し全体になる() {
        let inserts = [insert(2.0, 10.0, 0), insert(1.0, 20.0, 0)];
        let plan = plan_rotation(Some(1.0), &inserts).unwrap();
        let parts = [&[damage(1000)][..], &[damage(500)][..]];
        let filler = Some((damage(100), 1.0));
        let shares = rotation_shares(&plan, &parts, filler).unwrap();
        let (_, expected) = rotation_dps(&plan, &parts, filler).unwrap();
        let sum: f64 = shares
            .inserts
            .iter()
            .chain(shares.filler.iter())
            .map(|share| share.expected_dps)
            .sum();
        assert!((sum - expected).abs() < 1e-9, "{sum} vs {expected}");
        // 差し込み 1 件ぶんは「1 回のダメージ ÷ その間隔」そのもの
        assert!((shares.inserts[0].expected_dps - 1000.0 / 10.0).abs() < 1e-9);
        // 連打の回数は空いた時間ぶん(`filler_share ÷ 連打 1 回の秒数`)
        let filler_share = shares.filler.unwrap();
        assert!((filler_share.uses_per_second - plan.filler_share / 1.0).abs() < 1e-12);
    }

    /// 差し込みだけで時間が埋まったら、連打の取り分は 0。
    #[test]
    fn 詰まった回しでは連打の取り分が0() {
        let inserts = [insert(9.0, 10.0, 0), insert(9.0, 10.0, 0), insert(9.0, 10.0, 0)];
        let plan = plan_rotation(Some(1.0), &inserts).unwrap();
        let heavy = [damage(900)];
        let parts: Vec<&[RotationDamage]> = vec![&heavy[..], &heavy[..], &heavy[..]];
        let shares = rotation_shares(&plan, &parts, Some((damage(100), 1.0))).unwrap();
        let filler = shares.filler.unwrap();
        assert_eq!(filler.expected_dps, 0.0);
        assert_eq!(filler.uses_per_second, 0.0);
    }

    /// 差し込みの損得: 1 回の火力が低い技を差し込むと、連打していたぶんが減って DPS が下がる。
    #[test]
    fn 差し込みの損得は外した回しとの差() {
        let inserts = [insert(2.0, 10.0, 0)];
        let filler = damage(100);
        // 連打技(1s で 100)より遅い差し込み(2s で 150)は、差し込むと下がる
        let weak_damage = [damage(150)];
        let weak: Vec<&[RotationDamage]> = vec![&weak_damage[..]];
        let plan = plan_rotation(Some(1.0), &inserts).unwrap();
        let (_, expected) = rotation_dps(&plan, &weak, Some((filler, 1.0))).unwrap();
        let loss =
            insert_expected_dps_gain(expected, Some(1.0), &inserts, &weak, Some((filler, 1.0)), 0)
                .unwrap();
        assert!(loss < 0.0, "{loss}");
        // 連打技だけ(100 / 1s)と比べているか
        assert!((expected - (100.0 + loss)).abs() < 1e-9);
        // 1 回の火力が大きい差し込みなら上がる
        let strong_damage = [damage(5000)];
        let strong: Vec<&[RotationDamage]> = vec![&strong_damage[..]];
        let (_, expected) = rotation_dps(&plan, &strong, Some((filler, 1.0))).unwrap();
        let gain =
            insert_expected_dps_gain(expected, Some(1.0), &inserts, &strong, Some((filler, 1.0)), 0)
                .unwrap();
        assert!(gain > 0.0, "{gain}");
    }

    /// 主軸に実効 CT が無ければ主軸を連打し、いちばん火力の高い CT 技を差し込む。
    #[test]
    fn 主軸が連打技なら火力最大のct技を差し込む() {
        let main = skill("main", 10.0, 1.0, None);
        let weak = skill("weak_ct", 20.0, 1.0, Some(10.0));
        let strong = skill("strong_ct", 50.0, 1.0, Some(10.0));
        let spammable = skill("spammable", 80.0, 1.0, None);
        // 主軸 1 回 100 より重いので、どちらを差し込んでも DPS は上がる
        let (weak_d, strong_d, spam_d) = ([damage(1000)], [damage(5000)], [damage(9000)]);
        let candidates = [
            candidate(&weak, Some(1.0), &weak_d),
            candidate(&strong, Some(1.0), &strong_d),
            candidate(&spammable, Some(1.0), &spam_d),
        ];
        let roles = choose_rotation(&main, Some(1.0), Some(damage(100)), &candidates, None);
        assert_eq!(roles.filler, Some(RotationRole::Main));
        assert_eq!(roles.inserts, vec![RotationRole::Candidate(1)]);
    }

    /// 差し込むと DPS が下がる候補は既定で選ばない(画面が畳んだ先で出す)。
    /// 上がる候補が無ければ既定は「差し込みなし」。
    #[test]
    fn 差し込むと下がる候補は既定で選ばない() {
        let main = skill("main", 10.0, 1.0, None);
        let heavy_but_slow = skill("heavy_ct", 80.0, 1.0, Some(10.0));
        let light = skill("light_ct", 20.0, 1.0, Some(10.0));
        // 主軸 1 回 100 より軽い = 連打していたぶんが減って下がる
        let (heavy_d, light_d) = ([damage(50)], [damage(1000)]);
        let candidates = [
            candidate(&heavy_but_slow, Some(1.0), &heavy_d),
            candidate(&light, Some(1.0), &light_d),
        ];
        // 火力(power)は heavy_ct のほうが大きいが、差し込むと下がるので選ばれない
        let roles = choose_rotation(&main, Some(1.0), Some(damage(100)), &candidates, None);
        assert_eq!(roles.inserts, vec![RotationRole::Candidate(1)]);
        // 上がる候補が無ければ差し込まない
        let only_down = [candidate(&heavy_but_slow, Some(1.0), &heavy_d)];
        let roles = choose_rotation(&main, Some(1.0), Some(damage(100)), &only_down, None);
        assert_eq!(roles.inserts, Vec::new());
        assert_eq!(roles.filler, Some(RotationRole::Main));
    }

    /// 差し込めない印(<フラグ> を積んでいない技で爆発させる)の候補は選ばない。
    #[test]
    fn 差し込めない印の候補は選ばない() {
        let main = skill("main", 10.0, 1.0, None);
        let strong = skill("strong_ct", 50.0, 1.0, Some(10.0));
        let weak = skill("weak_ct", 20.0, 1.0, Some(10.0));
        let (strong_d, weak_d) = ([damage(9000)], [damage(1000)]);
        let candidates = [
            RotationCandidate {
                skill: &strong,
                seconds: Some(1.0),
                insertable: false,
                damage: &strong_d,
                minimum_filler_uses: 0,
            },
            candidate(&weak, Some(1.0), &weak_d),
        ];
        let roles = choose_rotation(&main, Some(1.0), Some(damage(100)), &candidates, None);
        assert_eq!(roles.inserts, vec![RotationRole::Candidate(1)]);
    }

    /// 主軸に実効 CT があれば主軸が差し込み側で、連打技は CT なしの候補から選ぶ。
    /// CT が所要時間以下の候補は「CT なし」= 連打技の候補になる。
    #[test]
    fn 主軸がct技なら連打技を選ぶ() {
        let main = skill("main", 100.0, 1.4, Some(10.0));
        let ct = skill("other_ct", 90.0, 1.0, Some(10.0));
        let short_ct = skill("short_ct", 30.0, 2.0, Some(1.0));
        let plain = skill("plain", 20.0, 1.0, None);
        // 主軸が差し込み側なので、候補のダメージは役割の決定に使われない
        let none: [RotationDamage; 0] = [];
        let candidates = [
            candidate(&ct, Some(1.0), &none),
            candidate(&short_ct, Some(2.0), &none),
            candidate(&plain, Some(1.0), &none),
        ];
        let roles = choose_rotation(&main, Some(1.4), Some(damage(100)), &candidates, None);
        assert_eq!(roles.inserts, vec![RotationRole::Main]);
        // power_per_second は short_ct が 15、plain が 20 なので plain が先
        assert_eq!(roles.filler, Some(RotationRole::Candidate(2)));
        // <フラグ> の積み直しで連打技が決まっているならそれに従う
        let pinned =
            choose_rotation(&main, Some(1.4), Some(damage(100)), &candidates, Some("short_ct"));
        assert_eq!(pinned.filler, Some(RotationRole::Candidate(1)));
        // 決まっている技が候補に無ければ連打技なし
        let missing =
            choose_rotation(&main, Some(1.4), Some(damage(100)), &candidates, Some("nothing"));
        assert_eq!(missing.filler, None);
    }

    /// 明示指定は既定の差し込みを置き換える。主軸が差し込み側ならそれは残り、
    /// 候補に無い id は落ちる。`Some([])` は「差し込まない」。
    #[test]
    fn 明示指定は既定の差し込みを置き換える() {
        let ids = ["a".to_string(), "c".to_string()];
        let candidates = ["a", "b"];
        // 主軸が連打技(既定は候補 1 件)
        let roles = RotationRoles {
            filler: Some(RotationRole::Main),
            inserts: vec![RotationRole::Candidate(1)],
        };
        assert_eq!(
            apply_explicit_inserts(&roles, None, "main", &candidates),
            vec![RotationRole::Candidate(1)]
        );
        assert_eq!(
            apply_explicit_inserts(&roles, Some(&[]), "main", &candidates),
            Vec::new()
        );
        // "c" は候補に無いので落ちる
        assert_eq!(
            apply_explicit_inserts(&roles, Some(&ids), "main", &candidates),
            vec![RotationRole::Candidate(0)]
        );
        // 主軸が差し込み側なら、明示指定でも主軸は残る(重複もしない)
        let main_insert = RotationRoles {
            filler: Some(RotationRole::Candidate(0)),
            inserts: vec![RotationRole::Main],
        };
        let with_main = ["main".to_string(), "b".to_string()];
        assert_eq!(
            apply_explicit_inserts(&main_insert, Some(&with_main), "main", &candidates),
            vec![RotationRole::Main, RotationRole::Candidate(1)]
        );
    }

    /// 実効 CT: CT が 1 回の所要時間以下なら「CT なし」。所要時間が出せないなら CT のまま。
    #[test]
    fn 実効ctは所要時間と比べて決まる() {
        let ct = skill("ct", 10.0, 1.0, Some(10.0));
        assert_eq!(effective_cooldown_seconds(&ct, Some(1.0)), Some(10.0));
        assert_eq!(effective_cooldown_seconds(&ct, Some(10.0)), None);
        assert_eq!(effective_cooldown_seconds(&ct, Some(12.0)), None);
        assert_eq!(effective_cooldown_seconds(&ct, None), Some(10.0));
        let plain = skill("plain", 10.0, 1.0, None);
        assert_eq!(effective_cooldown_seconds(&plain, Some(1.0)), None);
    }
}
