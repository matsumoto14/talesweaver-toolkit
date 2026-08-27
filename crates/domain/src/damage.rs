//! 与ダメージ計算(docs/damage-formula.md §3)。
//!
//! ①能力値計算 → ②カテゴリ集計 → ③式の評価 → ④段数 の 4 段をここで束ねる。

use serde::{Deserialize, Serialize};

use crate::actual_delay::{
    actual_delay, ActualDelay, ActualDelayContribution, SkillUsesTable, SECONDS_PER_MINUTE,
};
use crate::attack_power::{
    attack_power_breakdown, random_part_max, stat_attack_power, AttackCoefficients,
    AttackPowerBreakdown,
};
use crate::category::{CategoryTotals, CategoryTrace, DamageCategory};
use crate::critical_rate::{critical_rate, CriticalRate, CriticalRateSources};
use crate::common_skill::CommonSkills;
use crate::enemy::Enemy;
use crate::defense::{accuracy_point, AccuracyCorrection};
use crate::equipment::{equipment_values_attack, Equipment, EquipmentCoefficients, EquipmentValues};
use crate::random_option::RandomOptionTotals;
use crate::rounding::{floor_int, trunc2};
use crate::skill::Skill;
use crate::stat_sources::{apply_pins, Adjustments, StatContribution};
use crate::stats::{effective_stats, BaseStats, StatModifierSet, StatTrace};

/// 3 コンボ以上で付くコンボボーナス(wiki: カテゴリH)。
const COMBO_BONUS_RATE: f64 = 0.15;
/// コンボボーナスが付くコンボ数。
const COMBO_BONUS_THRESHOLD: u32 = 3;
/// 属性差 1 あたりの属性差ボーナス(%)(wiki: カテゴリI)。
const ELEMENT_BONUS_PERCENT_PER_POINT: f64 = 0.625;
/// 対モンスターの与ダメージ下限。
const MIN_DAMAGE_TO_MONSTER: i64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageInput {
    pub base_stats: BaseStats,
    pub stat_modifiers: StatModifierSet,
    /// `stat_modifiers` の寄与内訳(ペット/ルーン/クラウン/聖物/バフ/調整値)。トレース表示用
    pub stat_contributions: Vec<StatContribution>,
    pub coefficients: AttackCoefficients,
    /// 装備補正。シエナのオーラの攻撃力増加(New1)とテシスコアのセット効果(K/L)に使う
    pub equipment: Equipment,
    /// 共通スキル(wiki: Skill/共通)。装備攻撃力強化倍率(カテゴリA の内訳)と
    /// シャープネスビジョンの割合追加ダメージ(§5 新-割合)に使う
    pub common_skills: CommonSkills,
    /// 装備の基本能力値の集計(`Equipment::base_totals`。アビリティ加算込み。呼び出し側が gamedata の
    /// 武器アビリティカタログを使って集計して渡す。domain は gamedata に依存できないため)
    pub equipment_base_totals: EquipmentValues,
    /// 装備の強化能力値の集計(`Equipment::enhanced_totals`。エンチャント + シエナのオーラ +
    /// 対象コンテンツの地域のテシスコア。地域の解決は呼び出し側が行う)
    pub equipment_enhanced_totals: EquipmentValues,
    /// 装備攻撃力の係数(wiki: カテゴリA の内訳)。スキル依存種別ごとに gamedata が持つ
    pub equipment_coefficients: EquipmentCoefficients,
    /// 命中P補正の係数(wiki: 計算式まとめ の依存表)。スキル依存種別ごとに gamedata が持つ
    pub accuracy_correction: AccuracyCorrection,
    /// ランダムオプションの集計(`Equipment::random_option_totals`。カタログの解決は呼び出し側)。
    /// カテゴリP(依存別)・カテゴリX5(攻撃ダメージ(特殊))・命中P への加算に使う
    pub random_options: RandomOptionTotals,
    /// 称号の無条件「ダメージ n% 増加」(`title_attack_damage_rate`。カタログの解決は呼び出し側)。
    /// **カテゴリX3 攻撃ダメージ(基本発動)**(上限 +80%)
    pub title_attack_damage_rate: f64,
    /// 対象地域・敵に一致した称号の割合追加ダメージ(§5 新-割合)。
    pub title_added_damage_rate: f64,
    /// キャラスキル・マスタリー・バフの、与ダメージ式のカテゴリへの寄与(カタログの解決は
    /// 呼び出し側)。効き先はカテゴリごとに違う(X4 攻撃ダメージ(スキル)、L 最終ダメージ、
    /// E1/E2 スキル倍率増加 …)ので、値だけでなく**どのカテゴリか**を持つ
    pub damage_contributions: Vec<(DamageCategory, f64)>,
    /// 武器の装備強化による追加固定ダメージ(wiki: 装備システム/装備強化、docs/damage-formula.md §5)。
    /// 与ダメージ式の外(A〜Y のいずれにも入らない)。無強化なら 0
    pub weapon_added_damage: i64,
    pub skill: Skill,
    pub enemy: Enemy,
    /// 覚醒倍率(wiki: カテゴリN)。1.0 = 補正なし
    pub awakening_rate: f64,
    /// 与ダメージの上限(wiki: Quest/覚醒クエスト「ダメージ上限は多段スキルでも1段ごとに適用」)。
    /// 覚醒段階とエタの意志 Lv で決まる(表は gamedata)
    pub damage_cap: i64,
    /// 最終能力値の上限(wiki: Quest/覚醒クエスト「各能力の上限値」/ エタの意志)。
    /// 同じく覚醒段階とエタの意志 Lv で決まる(表は gamedata)
    pub stat_cap: i64,
    pub combo_count: u32,
    /// スキルの属性に対応するキャラの属性値(wiki: カテゴリI の起点)。
    /// スキルの属性が未取込(`Skill::element` が `None`)なら 0
    pub element_value: i64,
    /// 能力値の固定(pin)。キャラの保存済み調整値
    pub pins: Adjustments,
    /// 計算リクエストの一時調整(キャラには保存しない)。ステごとに `pins` より優先する
    pub temporary_pins: Option<Adjustments>,
    /// クリティカル率の供給源(wiki: 計算式まとめ `#CriticalChance`)。ペット会心・極のルーン等
    pub critical_rate_sources: CriticalRateSources,
    /// 実測のスキル回数表(60 秒あたり)。DPS はここから出す(格子の外だけ式)。実データは gamedata
    pub skill_uses: SkillUsesTable,
    /// キャラスキル・マスタリーによる中ディレイ減少(wiki: ステータス「中ディレイ倍率B」)。
    /// カタログの解決は呼び出し側(`CharacterSkills::actual_delay_contributions`)。共通の供給源
    /// (フルスロットル / ランダムオプション / シエナのオーラ)は `calculate_damage` が自分で集める
    pub actual_delay_skills: Vec<ActualDelayContribution>,
}

impl DamageInput {
    /// 計算に必要な要素を組み立てる。
    /// ステータス補正(`stat_modifiers`/`stat_contributions`)・装備(`equipment`/`equipment_coefficients`)は
    /// 呼び出し側(コマンド)が組み立てて渡す(中立値の決め打ちはしない)。
    pub fn new(
        base_stats: BaseStats,
        stat_modifiers: StatModifierSet,
        stat_contributions: Vec<StatContribution>,
        coefficients: AttackCoefficients,
        equipment: Equipment,
        common_skills: CommonSkills,
        equipment_base_totals: EquipmentValues,
        equipment_enhanced_totals: EquipmentValues,
        equipment_coefficients: EquipmentCoefficients,
        accuracy_correction: AccuracyCorrection,
        random_options: RandomOptionTotals,
        title_attack_damage_rate: f64,
        title_added_damage_rate: f64,
        damage_contributions: Vec<(DamageCategory, f64)>,
        weapon_added_damage: i64,
        awakening_rate: f64,
        damage_cap: i64,
        stat_cap: i64,
        skill: Skill,
        enemy: Enemy,
        combo_count: u32,
        element_value: i64,
        pins: Adjustments,
        temporary_pins: Option<Adjustments>,
        actual_delay_skills: Vec<ActualDelayContribution>,
        critical_rate_sources: CriticalRateSources,
        skill_uses: SkillUsesTable,
    ) -> Self {
        Self {
            base_stats,
            stat_modifiers,
            stat_contributions,
            coefficients,
            equipment,
            common_skills,
            equipment_base_totals,
            equipment_enhanced_totals,
            equipment_coefficients,
            accuracy_correction,
            random_options,
            title_attack_damage_rate,
            title_added_damage_rate,
            damage_contributions,
            weapon_added_damage,
            skill,
            enemy,
            awakening_rate,
            damage_cap,
            stat_cap,
            combo_count,
            element_value,
            pins,
            temporary_pins,
            actual_delay_skills,
            critical_rate_sources,
            skill_uses,
        }
    }
}

/// 式の 1 段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormulaStep {
    pub name: String,
    pub expression: String,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageTriple {
    pub min: i64,
    pub max: i64,
    pub critical: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageTrace {
    pub stats: Vec<StatTrace>,
    /// ステ補正源(ペット/ルーン/クラウン/聖物/バフ/調整値)の寄与内訳
    pub stat_contributions: Vec<StatContribution>,
    /// 最大乱数(B = 最大)時のカテゴリ集計
    pub categories: Vec<CategoryTrace>,
    pub steps_min: Vec<FormulaStep>,
    pub steps_max: Vec<FormulaStep>,
    pub steps_critical: Vec<FormulaStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageResult {
    /// 1 段あたりの与ダメージ(ダメージ上限を適用したあと)
    pub per_hit: DamageTriple,
    /// 与ダメージ × 段数
    pub total: DamageTriple,
    pub hit_count: u32,
    /// 与ダメージの上限(1 段ごとに適用)
    pub damage_cap: i64,
    /// 上限で捨てられた分(1 段あたり)。すべて 0 なら上限に当たっていない
    pub capped_loss: DamageTriple,
    /// 割合追加ダメージ(§5「新-割合」)の Σ%。いまの供給源はシャープネスビジョンのみ
    pub added_damage_rate: f64,
    /// 割合追加ダメージの実額。**合計ダメージ**に乗る(1 段ごとではない)ので
    /// `total` にだけ含まれる
    pub added_damage: DamageTriple,
    /// 命中P(wiki: 計算式まとめ #AccuracyPoint)。敵の回避Pを 100 上回ると必中。
    /// スキル命中が wiki 未記載(`Skill::accuracy` が `None`)なら出せないので `None`
    pub accuracy_point: Option<i64>,
    /// クリティカル率(wiki: 計算式まとめ `#CriticalChance`)。
    /// **敵の AGI とクリティカル被撃率が両方そろっている敵でしか出せない**ので、
    /// 片方でも未記載(wiki が `?`)なら `None`。スキルの Cri値が未記載でも `None`
    pub critical_rate: Option<CriticalRate>,
    /// 中ディレイ(wiki: 計算式まとめ `#ActualDelay`)。スキルの「動作」列が秒で取れない
    /// (`Skill::base_actual_delay` が `None`)なら出せないので `None`
    pub actual_delay: Option<ActualDelay>,
    /// 1 秒あたりの与ダメージ(合計ダメージ / 中ディレイ)。中ディレイが出せないなら `None`
    pub dps: Option<DpsTriple>,
    pub trace: DamageTrace,
}

/// 1 秒あたりの与ダメージ(合計ダメージ / 中ディレイ)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DpsTriple {
    pub min: f64,
    pub max: f64,
    pub critical: f64,
}

/// 属性差ボーナス(wiki: カテゴリI)の Σ%。`floor((属性値 − 閾値) × 0.625) / 100`。範囲はキャップで 0..+50% に収める。
fn element_bonus_rate(element_value: i64, element_threshold: i64) -> f64 {
    floor_int((element_value - element_threshold) as f64 * ELEMENT_BONUS_PERCENT_PER_POINT) as f64
        / 100.0
}

/// 与ダメージ式(wiki §3)を評価する。`totals` には A〜Y がすべて入っている前提。
///
/// ```text
/// [ ( ( MAX( (A + B − C) * {D*E1 + E2} * {F*G} * H * I * J * New1 + K, K )
///       * L * V1 + M )
///     * Old * N * O * P * (1−Q) * R * (1−S) * T * (1−U) * (1−New2) * V2 + W )
///   * X * Y ]
/// ```
pub fn evaluate(totals: &CategoryTotals, critical: bool) -> (i64, Vec<FormulaStep>) {
    use DamageCategory::*;
    let g = |c: DamageCategory| totals.get(c);
    let mut steps = Vec::with_capacity(10);
    let mut step = |name: &str, expression: String, value: f64| {
        steps.push(FormulaStep { name: name.to_string(), expression, value });
        value
    };

    let base = step(
        "攻撃力−防御力",
        format!("A {} + B {} − C {}", g(AttackPower), g(AttackRandom), g(TargetDefense)),
        g(AttackPower) + g(AttackRandom) - g(TargetDefense),
    );
    let skill = step(
        "スキル倍率",
        format!("{{D {} × E1 {} + E2 {}}}", g(SkillMultiplier), g(SkillMultiplierRate), g(SkillMultiplierFixed)),
        trunc2(g(SkillMultiplier) * g(SkillMultiplierRate) + g(SkillMultiplierFixed)),
    );
    let crit = if critical {
        step(
            "クリティカル",
            format!("{{F {} × G {}}}", g(CriticalMultiplier), g(CriticalDamageRate)),
            trunc2(g(CriticalMultiplier) * g(CriticalDamageRate)),
        )
    } else {
        // 非クリティカル時は `{F×G}` ごと 1.0。F(Cri倍率)はクリティカル時にだけ代入され、
        // G(クリティカルダメージ増加)の供給源は wiki ステータス [G] の表がすべて
        // 「クリティカルダメージ増加」(スコープアイ / 致命のルーン / ソウルリンク / 称号 /
        // プシーキーの刻印)で、非クリティカルの一撃には乗らない(取得 2026-08-25)。
        step("クリティカル", "非クリティカル({F × G} = 1.0)".to_string(), 1.0)
    };
    let bonus = step(
        "コンボ・属性・カット率・オーラ",
        format!("H {} × I {} × J {} × New1 {}", g(ComboBonus), g(ElementBonus), g(PlayerCutRate), g(SienaAuraAttackRate)),
        g(ComboBonus) * g(ElementBonus) * g(PlayerCutRate) * g(SienaAuraAttackRate),
    );
    let product = base * skill * crit * bonus;
    let inner = step(
        "最終ダメージ固定値(下限)",
        format!("MAX({product:.4} + K {k}, K {k})", k = g(FinalDamageFixed)),
        (product + g(FinalDamageFixed)).max(g(FinalDamageFixed)),
    );
    let mid = step(
        "最終ダメージ・カット率A・被害減少",
        format!("{inner:.4} × L {} × V1 {} + M {}", g(FinalDamageRate), g(CutRateA), g(DamageReduction)),
        inner * g(FinalDamageRate) * g(CutRateA) + g(DamageReduction),
    );
    let outer_factors = g(AttackDamageLegacy)
        * g(AwakeningDamage)
        * g(PhysicalMagicDamageRate)
        * g(DependencyDamageRate)
        * g(DamageAbsorb)
        * g(TakenDamageRate)
        * g(TakenDamageReduction)
        * g(DamageAmplify)
        * g(DamageResistance)
        * g(DamageMitigation)
        * g(CutRateB);
    let outer = step(
        "各種ダメージ増減",
        format!(
            "{mid:.4} × Old {} × N {} × O {} × P {} × (1−Q) {} × R {} × (1−S) {} × T {} × (1−U) {} × (1−New2) {} × V2 {} + W {}",
            g(AttackDamageLegacy),
            g(AwakeningDamage),
            g(PhysicalMagicDamageRate),
            g(DependencyDamageRate),
            g(DamageAbsorb),
            g(TakenDamageRate),
            g(TakenDamageReduction),
            g(DamageAmplify),
            g(DamageResistance),
            g(DamageMitigation),
            g(CutRateB),
            g(BasicTriggerDamageFixed),
        ),
        mid * outer_factors + g(BasicTriggerDamageFixed),
    );
    let final_value = step(
        "攻撃ダメージ・PVP補正",
        format!("{outer:.4} × X {} × Y {}", g(AttackDamageRate), g(PvpCorrection)),
        outer * g(AttackDamageRate) * g(PvpCorrection),
    );
    let floored = floor_int(final_value);
    step("切捨て", format!("[{final_value:.4}]"), floored as f64);
    let damage = floored.max(MIN_DAMAGE_TO_MONSTER);
    step("対モンスター下限", format!("MAX({floored}, {MIN_DAMAGE_TO_MONSTER})"), damage as f64);
    (damage, steps)
}

pub fn calculate_damage(input: &DamageInput) -> DamageResult {
    use DamageCategory::*;

    // ① 能力値計算
    let (mut stats, mut stat_traces) =
        effective_stats(&input.base_stats, &input.stat_modifiers, input.stat_cap);
    apply_pins(&mut stats, &mut stat_traces, &input.pins, input.temporary_pins.as_ref());

    // ② カテゴリ集計
    let stat_attack = stat_attack_power(&stats, &input.coefficients);
    let attack = attack_power_breakdown(
        stat_attack,
        equipment_values_attack(&input.equipment_base_totals, &input.equipment_coefficients.base),
        equipment_values_attack(
            &input.equipment_enhanced_totals,
            &input.equipment_coefficients.enhanced,
        ),
        input.common_skills.equipment_attack_rate(),
    );
    let mut totals = CategoryTotals::neutral();
    totals.add(AttackPower, attack.value as f64);
    totals.add(TargetDefense, input.enemy.defense as f64);
    totals.add(SkillMultiplier, input.skill.multiplier);
    totals.add(CriticalMultiplier, input.skill.critical_multiplier);
    if input.combo_count >= COMBO_BONUS_THRESHOLD {
        totals.add(ComboBonus, COMBO_BONUS_RATE);
    }
    totals.add(ElementBonus, element_bonus_rate(input.element_value, input.enemy.element_threshold));
    totals.add(DamageReduction, input.enemy.damage_reduction as f64);
    totals.add(AwakeningDamage, input.awakening_rate - 1.0);
    totals.add(CutRateA, input.enemy.cut_rate_a - 1.0);
    // シエナのオーラの追加オプション「攻撃力増加」(wiki: New1。実際は与ダメージ割合増加)
    totals.add(SienaAuraAttackRate, input.equipment.siena_attack_rate());
    // テシスコアのセット効果(wiki: コアセット効果。全地域で発動するので対象コンテンツの地域を問わない)
    let core_set_bonus = input.equipment.thesis_cores.set_bonus();
    totals.add(FinalDamageFixed, core_set_bonus.final_damage_fixed as f64);
    totals.add(FinalDamageRate, core_set_bonus.final_damage_rate);
    // ランダムオプション(wiki: ランダムオプション)。依存別攻撃力増加はスキルの依存種別が
    // 一致したときだけ乗る(カテゴリP)、攻撃ダメージ増加はカテゴリX
    totals.add(
        DependencyDamageRate,
        input.random_options.dependency_damage_rate.get(input.skill.dependency),
    );
    // カテゴリX は X1〜X6 の合計で、**上限が子ごとに違う**(X3 +80% / X4 +65% / X5 未記載)。
    // 親の `AttackDamageRate` は子の合計として読み出されるので、ここでは子に足す
    totals.add(AttackDamageBasicTrigger, input.title_attack_damage_rate);
    totals.add(AttackDamageSpecial, input.random_options.attack_damage_rate);
    // キャラスキル・マスタリー・バフ。効き先はカテゴリごとに違う
    for (category, value) in &input.damage_contributions {
        totals.add(*category, *value);
    }
    // 極限スキル「スコープアイ」(wiki: Skill/極限)。カテゴリG はクリティカル時にだけ乗る
    totals.add(CriticalDamageRate, input.common_skills.ultimate.critical_damage_rate());

    let mut totals_min = totals.clone();
    totals_min.add(AttackRandom, 1.0);
    let mut totals_max = totals;
    totals_max.add(AttackRandom, random_part_max(stat_attack, stats.dex));

    // ③ 式の評価
    let (min, steps_min) = evaluate(&totals_min, false);
    let (max, steps_max) = evaluate(&totals_max, false);
    let (critical, steps_critical) = evaluate(&totals_max, true);

    // 攻撃力(A)の内訳(ステ攻撃力/装備攻撃力/装備攻撃力強化倍率)。A は B(乱数)を含まないため
    // min/max/critical のすべてで同じ内訳になる。`evaluate` は `totals` からしか値を作れず
    // 内訳を持たないため、ここで先頭に付け足す。
    let attack_breakdown = attack_power_breakdown_steps(&attack);
    let mut steps_min: Vec<FormulaStep> = attack_breakdown.iter().cloned().chain(steps_min).collect();
    let mut steps_max: Vec<FormulaStep> = attack_breakdown.iter().cloned().chain(steps_max).collect();
    let mut steps_critical: Vec<FormulaStep> = attack_breakdown.into_iter().chain(steps_critical).collect();

    // ④ 段数
    // 極限スキル「フルスロットル」(wiki: Skill/極限)。ハイパーリミット Lv4 以降で
    // **単体チャネリングスキル**の段数が +1〜+3 される。他のスキルには乗らない
    let added_hits = if input.skill.single_target_channeling {
        input.common_skills.ultimate.added_hit_count()
    } else {
        0
    };
    let hit_count = input.skill.hit_count + added_hits;
    let hits = i64::from(hit_count);

    // §5 武器強化の追加固定ダメージ(与ダメージ式の外)。1 体あたり per-hit 追加 = INT(追加ダメージ / hits)、
    // 合計追加 = per-hit 追加 × hits(wiki: 装備システム/装備強化のヒット分割仕様)。
    let per_hit_added = if hit_count > 0 { input.weapon_added_damage / hits } else { 0 };
    let (min, max, critical) = (min + per_hit_added, max + per_hit_added, critical + per_hit_added);
    if per_hit_added != 0 {
        let step = FormulaStep {
            name: "武器強化(追加固定ダメージ)".to_string(),
            expression: format!("INT({} / {hits}) = {per_hit_added}", input.weapon_added_damage),
            value: per_hit_added as f64,
        };
        steps_min.push(step.clone());
        steps_max.push(step.clone());
        steps_critical.push(step);
    }

    // 命中P(wiki: 計算式まとめ #AccuracyPoint)。与ダメージ式には入らないが、必中に必要な
    // 命中P(狩り場情報一覧)と見比べられるように結果に載せる。
    let accuracy = input.skill.accuracy.map(|skill_accuracy| {
        accuracy_point(
            &stats,
            &input.accuracy_correction,
            input.equipment_base_totals.accuracy + input.equipment_enhanced_totals.accuracy,
            skill_accuracy,
            input.random_options.accuracy_point,
        )
    });

    // ダメージ上限(wiki: Quest/覚醒クエスト。多段スキルでも 1 段ごとに適用)。
    // 捨てられた分は 0 と区別できるように別で持つ(UI が「上限で捨てた分」を出す)。
    let cap = |value: i64| value.min(input.damage_cap);
    let (capped_min, capped_max, capped_critical) = (cap(min), cap(max), cap(critical));
    let capped_loss =
        DamageTriple { min: min - capped_min, max: max - capped_max, critical: critical - capped_critical };
    if capped_loss.max > 0 {
        let step = FormulaStep {
            name: "ダメージ上限".to_string(),
            expression: format!("MIN(生値, {}) ※1 段ごとに適用", input.damage_cap),
            value: input.damage_cap as f64,
        };
        steps_min.push(step.clone());
        steps_max.push(step.clone());
        steps_critical.push(step);
    }
    let (min, max, critical) = (capped_min, capped_max, capped_critical);

    // §5 割合追加ダメージ(新-割合)。「合計ダメージ + 追加ダメージ(武器強化)」に掛かるので、
    // 1 段ごとではなく段数を掛けたあとの合計に乗せる。武器強化の追加固定ダメージは
    // すでに per-hit に入っているので、`合計` がそのまま算出基準になる。
    // 供給源はシャープネスビジョン、武器のランダムOP、対象条件に一致した称号。
    // OP 側は発動条件を満たしている前提で入れる。
    let added_rate = input.common_skills.sharpness_vision_rate()
        + input.random_options.added_damage_rate
        + input.title_added_damage_rate;
    let sum = DamageTriple { min: min * hits, max: max * hits, critical: critical * hits };
    let added = DamageTriple {
        min: floor_int(sum.min as f64 * added_rate),
        max: floor_int(sum.max as f64 * added_rate),
        critical: floor_int(sum.critical as f64 * added_rate),
    };
    if added.max != 0 {
        let step = FormulaStep {
            name: "割合追加ダメージ(合計に乗る)".to_string(),
            expression: format!(
                "合計 × {:.0}% ※シャープネスビジョン {:.0}% + ランダムOP {:.0}% + 称号 {:.0}%",
                added_rate * 100.0,
                input.common_skills.sharpness_vision_rate() * 100.0,
                input.random_options.added_damage_rate * 100.0,
                input.title_added_damage_rate * 100.0
            ),
            value: added_rate,
        };
        steps_min.push(step.clone());
        steps_max.push(step.clone());
        steps_critical.push(step);
    }

    // クリティカル率(wiki: 計算式まとめ `#CriticalChance`)。与ダメージ式には入らないが、
    // 「クリティカル ×N」がどれくらいの頻度で出るのかを読めるように結果に載せる。
    // 対象のAGI・クリティカル被撃率(狩り場情報一覧)は `?` の行が多く、被撃率は −250〜−930% と
    // 支配的なので、**両方そろっている敵でだけ**出す。スキルの Cri値が未記載でも出さない。
    let critical_chance = match (input.enemy.agi, input.enemy.critical_taken_rate, input.skill.critical_rate) {
        (Some(target_agi), Some(taken_rate), Some(skill_critical_rate)) => Some(critical_rate(
            input.equipment_base_totals.critical + input.equipment_enhanced_totals.critical,
            stats.agi,
            target_agi,
            skill_critical_rate as f64,
            &input.critical_rate_sources,
            input.equipment.siena_critical_rate(),
            taken_rate,
        )),
        _ => None,
    };

    // 中ディレイ(wiki: 計算式まとめ `#ActualDelay`)。減少値の供給源は
    // 極限スキル「フルスロットル」/ カフス(盾+)のランダムオプション / シエナのオーラ /
    // キャラのパッシブ(呼び出し側がカタログを引いて渡す)。
    let delay = input.skill.base_actual_delay.map(|base| {
        let mut contributions = Vec::new();
        let full_throttle = input.common_skills.ultimate.actual_delay_reduction();
        if full_throttle != 0.0 {
            contributions
                .push(ActualDelayContribution { source: "フルスロットル".into(), rate: full_throttle });
        }
        let random_option = input.random_options.actual_delay_reduction;
        if random_option != 0.0 {
            contributions.push(ActualDelayContribution {
                source: "ランダムオプション(カフス)".into(),
                rate: random_option,
            });
        }
        let siena = input.equipment.siena_actual_delay_reduction();
        if siena != 0.0 {
            contributions
                .push(ActualDelayContribution { source: "シエナのオーラ".into(), rate: siena });
        }
        contributions.extend(input.actual_delay_skills.iter().cloned());
        actual_delay(
            base,
            input.skill.actual_delay_fixed,
            contributions,
            input.combo_count,
            &input.skill_uses,
        )
    });
    let total = DamageTriple {
        min: sum.min + added.min,
        max: sum.max + added.max,
        critical: sum.critical + added.critical,
    };
    // DPS は「合計ダメージ × 60 秒あたりのスキル回数 / 60」。回数は実測表(格子の外だけ式)。
    let dps = delay.as_ref().map(|d| {
        let per_second = |total: i64| total as f64 * d.uses_per_minute / SECONDS_PER_MINUTE;
        DpsTriple {
            min: per_second(total.min),
            max: per_second(total.max),
            critical: per_second(total.critical),
        }
    });

    DamageResult {
        per_hit: DamageTriple { min, max, critical },
        total,
        hit_count,
        damage_cap: input.damage_cap,
        capped_loss,
        added_damage_rate: added_rate,
        added_damage: added,
        accuracy_point: accuracy,
        critical_rate: critical_chance,
        actual_delay: delay,
        dps,
        trace: DamageTrace {
            stats: stat_traces,
            stat_contributions: input.stat_contributions.clone(),
            categories: totals_max.trace(),
            steps_min,
            steps_max,
            steps_critical,
        },
    }
}

/// 攻撃力(A)の内訳を表す `FormulaStep` 4 件(ステ攻撃力/装備攻撃力/装備攻撃力強化倍率/攻撃力(A))。
fn attack_power_breakdown_steps(attack: &AttackPowerBreakdown) -> Vec<FormulaStep> {
    let AttackPowerBreakdown { stat_attack, enhance_rate, .. } = *attack;
    let equipment_attack = attack.equipment_attack();
    vec![
        FormulaStep { name: "ステ攻撃力".to_string(), expression: format!("{stat_attack:.4}"), value: stat_attack },
        FormulaStep {
            name: "装備攻撃力".to_string(),
            expression: format!(
                "基本 {:.4} + 強化 {:.4}",
                attack.equipment_base_attack, attack.equipment_enhanced_attack
            ),
            value: equipment_attack,
        },
        FormulaStep {
            name: "装備攻撃力強化倍率".to_string(),
            expression: format!("{enhance_rate:.4}"),
            value: enhance_rate,
        },
        FormulaStep {
            name: "攻撃力(A)".to_string(),
            expression: format!(
                "[{stat_attack:.4} + {equipment_attack:.4}] + [{equipment_attack:.4}/25 × {enhance_rate:.4}] × 25"
            ),
            value: attack.value as f64,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::CategoryKind;
    use crate::siena::{SienaAura, SienaExtraKind, SienaExtraSlot, SienaSlot, SienaValueKind};
    use crate::skill::SkillDependency;
    use crate::stat_sources::StatAdjustment;

    /// 追加オプションを 1 個持つオーラ。追加オプションは段階 3 以上で解放されるので
    /// 能力値スロットを 3 個埋める(段階 = スロット数)。
    fn siena_extra(kind: SienaValueKind, extra: SienaExtraKind, value: f64) -> SienaAura {
        SienaAura {
            slots: vec![SienaSlot { kind, value: 1 }; 3],
            extras: vec![SienaExtraSlot { kind: extra, value }],
        }
    }
    use crate::stats::{PinSource, StatKind};

    fn input() -> DamageInput {
        DamageInput {
            base_stats: BaseStats { stab: 500, hack: 500, int: 0, def: 0, mr: 0, dex: 100, agi: 0 },
            stat_modifiers: StatModifierSet::default(),
            stat_contributions: Vec::new(),
            coefficients: AttackCoefficients { primary: (StatKind::Stab, 1.8), secondary: (StatKind::Hack, 1.8) },
            equipment: Equipment::default(),
            common_skills: CommonSkills::default(),
            equipment_base_totals: EquipmentValues::default(),
            equipment_enhanced_totals: EquipmentValues::default(),
            title_attack_damage_rate: 0.0,
            title_added_damage_rate: 0.0,
            damage_contributions: Vec::new(),
            equipment_coefficients: EquipmentCoefficients::default(),
            // STAB+HACK 依存(ボーナスなし、ペナルティ (STAB+HACK)/200)
            accuracy_correction: AccuracyCorrection {
                bonus: None,
                penalty_primary: StatKind::Stab,
                penalty_secondary: Some(StatKind::Hack),
                penalty_divisor: 200.0,
            },
            random_options: RandomOptionTotals::default(),
            weapon_added_damage: 0,
            // テストは上限に当たらない値を既定にする(上限の挙動は専用テストで見る)
            damage_cap: i64::MAX,
            stat_cap: i64::MAX,
            skill: Skill {
                id: "s".into(),
                name: "テスト斬り".into(),
                dependency: SkillDependency::StabHack,
                multiplier: 0.99,
                hit_count: 1,
                critical_multiplier: 2.0,
                element: crate::element::Element::Water,
                target: Some(crate::skill::SkillTarget::Single),
                accuracy: Some(92),
                critical_rate: Some(7),
                level: 1,
                single_target_channeling: false,
                base_actual_delay: Some(1.4),
                actual_delay_fixed: false,
            },
            enemy: Enemy {
                id: "e".into(),
                name: "テスト敵".into(),
                defense: 990,
                damage_reduction: 0,
                cut_rate_a: 1.0,
                element_threshold: 90,
                agi: None,
                critical_taken_rate: None,
            },
            awakening_rate: 1.0,
            combo_count: 0,
            element_value: 0,
            pins: Adjustments::default(),
            temporary_pins: None,
            actual_delay_skills: Vec::new(),
            critical_rate_sources: CriticalRateSources::default(),
            // 実測表の挙動は actual_delay.rs のテストで見る。ここは式にフォールバックさせる
            skill_uses: SkillUsesTable {
                reduction_percents: Vec::new(),
                base_delays: Vec::new(),
                uses: Vec::new(),
            },
        }
    }

    // 基準値(手計算):
    //   ステ攻撃力 = 500×1.8 + 500×1.8 = 1800 → A = 1800
    //   B最大 = {(1800 + 100×3)/18} + 1 = {116.666..} + 1 = 117.66
    //   min : (1800 + 1 − 990) × {0.99} = 811 × 0.99 = 802.89 → 802
    //   max : (1800 + 117.66 − 990) × 0.99 = 927.66 × 0.99 = 918.3834 → 918
    //   crit: 918.3834 × {2.0 × 1.0} = 1836.7668 → 1836
    #[test]
    fn 命中pはdexと装備命中とスキル命中から依存ペナルティを引く() {
        let mut i = input();
        i.equipment_base_totals.accuracy = 30;
        i.equipment_enhanced_totals.accuracy = 20;
        // DEX 100 + 装備 50 + スキル命中 92 − [(STAB500 + HACK500)/200] = 242 − 5 = 237
        assert_eq!(calculate_damage(&i).accuracy_point, Some(237));
    }

    #[test]
    fn ダメージ上限は1段ごとに適用され捨てられた分を残す() {
        let mut i = input();
        i.skill.hit_count = 3;
        let uncapped = calculate_damage(&i);
        assert!(uncapped.capped_loss.max == 0);

        i.damage_cap = uncapped.per_hit.max - 100;
        let capped = calculate_damage(&i);
        assert_eq!(capped.per_hit.max, i.damage_cap);
        assert_eq!(capped.capped_loss.max, 100);
        // 上限は 1 段ごとなので合計は 上限 × 段数
        assert_eq!(capped.total.max, i.damage_cap * 3);
        assert_eq!(capped.damage_cap, i.damage_cap);
        assert!(capped.trace.steps_max.iter().any(|s| s.name == "ダメージ上限"));
    }

    #[test]
    fn 攻撃力_乱数_防御力_スキル倍率_cri倍率() {
        let r = calculate_damage(&input());
        assert_eq!(r.per_hit, DamageTriple { min: 802, max: 918, critical: 1836 });
        assert_eq!(r.total, r.per_hit);
        assert_eq!(r.hit_count, 1);
        assert_eq!(r.trace.stats.len(), 7);
        assert_eq!(r.trace.categories.len(), 36);
        let a = r.trace.categories.iter().find(|c| c.symbol == "A").unwrap();
        assert_eq!(a.value, 1800.0);
        assert_eq!(a.kind, CategoryKind::Assigned);
    }

    #[test]
    fn 段数を掛けた合計() {
        let mut i = input();
        i.skill.hit_count = 11;
        let r = calculate_damage(&i);
        assert_eq!(r.per_hit, DamageTriple { min: 802, max: 918, critical: 1836 });
        assert_eq!(r.total, DamageTriple { min: 802 * 11, max: 918 * 11, critical: 1836 * 11 });
    }

    // コンボ 3 以上で H = 1.15:
    //   min : 802.89 × 1.15 = 923.3235 → 923
    //   max : 918.3834 × 1.15 = 1056.14091 → 1056
    //   crit: 1836.7668 × 1.15 = 2112.28182 → 2112
    #[test]
    fn コンボボーナス() {
        let mut i = input();
        i.combo_count = 2;
        assert_eq!(calculate_damage(&i).per_hit.min, 802);
        i.combo_count = 3;
        let r = calculate_damage(&i);
        assert_eq!(r.per_hit, DamageTriple { min: 923, max: 1056, critical: 2112 });
    }

    // 覚醒倍率 1.2: max 918.3834 × 1.2 = 1102.06 → 1102
    #[test]
    fn 覚醒ダメージ() {
        let mut i = input();
        i.awakening_rate = 1.2;
        let r = calculate_damage(&i);
        assert_eq!(r.per_hit.max, 1102);
        let n = r.trace.categories.iter().find(|c| c.symbol == "N").unwrap();
        assert!((n.value - 0.2).abs() < 1e-12);
    }

    // 属性値 170 − 閾値 90 = 80 → 80 × 0.625 = 50 → I = 1.50: min 802.89 × 1.5 = 1204.335 → 1204
    // 属性値 1000 でも上限 +50% で同じ。属性値 0 は負 → 下限 0%
    #[test]
    fn 属性差ボーナス() {
        let mut i = input();
        i.element_value = 170;
        assert_eq!(calculate_damage(&i).per_hit.min, 1204);
        i.element_value = 1000;
        assert_eq!(calculate_damage(&i).per_hit.min, 1204);
        // 属性値 100 − 90 = 10 → 6.25 → floor 6 → 1.06: 802.89 × 1.06 = 851.0634 → 851
        i.element_value = 100;
        assert_eq!(calculate_damage(&i).per_hit.min, 851);
        i.element_value = 0;
        assert_eq!(calculate_damage(&i).per_hit.min, 802);
    }

    // 被害減少 −100、カット率A 0.5:
    //   min: 802.89 × 0.5 − 100 = 301.445 → 301
    #[test]
    fn 被害減少とカット率a() {
        let mut i = input();
        i.enemy.damage_reduction = -100;
        i.enemy.cut_rate_a = 0.5;
        assert_eq!(calculate_damage(&i).per_hit.min, 301);
    }

    // pin で STAB を 2000 に固定すると、ステ由来攻撃力の計算に反映されて結果が変わる。
    // trace.stats の STAB 行には pinned_from に元の 500 が残る。
    #[test]
    fn pinで能力値を固定すると攻撃力計算に反映されpinned_fromが記録される() {
        let mut i = input();
        i.pins.stab = StatAdjustment { add: 0, pin: Some(2000) };
        let r = calculate_damage(&i);
        assert_ne!(r.per_hit.min, 802);
        let stab_trace = r.trace.stats.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab_trace.pinned_from, Some(500));
        assert_eq!(stab_trace.effective, 2000);
    }

    #[test]
    fn temporary_pinsが保存済みpinを一時的に上書きしpin_sourceがtemporaryになる() {
        let mut i = input();
        i.pins.stab = StatAdjustment { add: 0, pin: Some(500) };
        i.temporary_pins = Some(Adjustments {
            stab: StatAdjustment { add: 0, pin: Some(999) },
            ..Default::default()
        });
        let r = calculate_damage(&i);
        let stab_trace = r.trace.stats.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab_trace.effective, 999);
        assert_eq!(stab_trace.pin_source, Some(PinSource::Temporary));
    }

    #[test]
    fn 防御力が攻撃力を上回ると対モンスター下限の1() {
        let mut i = input();
        i.enemy.defense = 5000;
        let r = calculate_damage(&i);
        assert_eq!(r.per_hit, DamageTriple { min: 1, max: 1, critical: 1 });
        assert_eq!(r.trace.steps_min.last().unwrap().name, "対モンスター下限");
    }

    #[test]
    fn 丸め境界_スキル倍率は小数2位切捨て() {
        use DamageCategory::*;
        let mut t = CategoryTotals::neutral();
        t.add(AttackPower, 1000.0);
        t.add(AttackRandom, 0.0);
        t.add(SkillMultiplier, 1.0);
        t.add(SkillMultiplierRate, 0.123); // {1.0 × 1.123} = 1.12
        t.add(CriticalMultiplier, 2.0);
        // 1000 × 1.12 = 1120
        assert_eq!(evaluate(&t, false).0, 1120);
        // クリティカル: G +30% → {2.0 × 1.3} = 2.6 → 1120 × 2.6 = 2912
        t.add(CriticalDamageRate, 0.3);
        assert_eq!(evaluate(&t, true).0, 2912);
        // G +0.7% → {2.0 × 1.007 = 2.014} = 2.01 → 1120 × 2.01 = 2251.2 → 2251
        let mut t2 = t.clone();
        t2.add(CriticalDamageRate, -0.3 + 0.007);
        assert_eq!(evaluate(&t2, true).0, 2251);
    }

    #[test]
    fn 最終ダメージ固定値が下限になる() {
        use DamageCategory::*;
        let mut t = CategoryTotals::neutral();
        t.add(AttackPower, 100.0);
        t.add(TargetDefense, 500.0);
        t.add(SkillMultiplier, 1.0);
        t.add(CriticalMultiplier, 2.0);
        t.add(FinalDamageFixed, 300.0);
        // (100 − 500) × 1 + 300 = −100 → MAX(−100, 300) = 300
        assert_eq!(evaluate(&t, false).0, 300);
    }

    // 外側の倍率群の位置関係(手計算):
    //   (A 2000 + B 0 − C 1000) × {D 1.0} = 1000
    //   × (1−Q 0.3) = 700、+ W 100 = 800(W は Old〜V2 の積の外・X の内側)
    //   × X 1.5 = 1200
    //   W が X の外側にあれば 1000 × 0.7 × 1.5 + 100 = 1150 になるので区別できる
    #[test]
    fn 外側の倍率群の位置_吸収と基本発動固定値と攻撃ダメージ() {
        use DamageCategory::*;
        let mut t = CategoryTotals::neutral();
        t.add(AttackPower, 2000.0);
        t.add(TargetDefense, 1000.0);
        t.add(SkillMultiplier, 1.0);
        t.add(CriticalMultiplier, 2.0);
        t.add(DamageAbsorb, 0.3);
        t.add(BasicTriggerDamageFixed, 100.0);
        t.add(AttackDamageSkill, 0.5);
        assert_eq!(evaluate(&t, false).0, 1200);
        // M は L・V1 の後、Old 以降の前: (1000 × 1.0 × 1.0 + M −200) × 0.7 + 100 = 660 → × 1.5 = 990
        t.add(DamageReduction, -200.0);
        assert_eq!(evaluate(&t, false).0, 990);
    }

    /// 配線漏れ防止: 全カテゴリに非中立値を入れたとき結果が変わること。
    #[test]
    fn 全カテゴリが式に配線されている() {
        use DamageCategory::*;
        let mut base = CategoryTotals::neutral();
        base.add(AttackPower, 2000.0);
        base.add(AttackRandom, 50.0);
        base.add(TargetDefense, 500.0);
        base.add(SkillMultiplier, 1.5);
        base.add(CriticalMultiplier, 2.0);
        let (normal, _) = evaluate(&base, false);
        let (critical, _) = evaluate(&base, true);
        assert!(normal > 1 && critical > normal);

        for category in DamageCategory::ALL {
            let mut t = base.clone();
            let delta = match category.kind() {
                CategoryKind::Assigned => t.value(category) * 1.5 + 1.0,
                CategoryKind::Fixed => 100.0,
                CategoryKind::Rate => 0.1,
            };
            // カテゴリX の親は子(X3/X4/X5)の合計として読むので、加算は子に入れる
            let target =
                if category == AttackDamageRate { AttackDamageSkill } else { category };
            t.add(target, delta);
            let (n, _) = evaluate(&t, false);
            let (c, steps) = evaluate(&t, true);
            assert_ne!(c, critical, "{}({}) がクリティカル時の結果に影響していない", category.label(), category.wiki_symbol());
            let only_critical = matches!(category, CriticalMultiplier | CriticalDamageRate);
            if !only_critical {
                assert_ne!(n, normal, "{}({}) が結果に影響していない", category.label(), category.wiki_symbol());
            }
            // X3/X4/X5 は親 X としてまとめて式に出る
            let symbol = if DamageCategory::ATTACK_DAMAGE_CHILDREN.contains(&category) {
                AttackDamageRate.wiki_symbol()
            } else {
                category.wiki_symbol()
            };
            assert!(
                steps.iter().any(|s| s.expression.contains(&format!("{symbol} "))
                    || s.expression.contains(&format!("{symbol}) "))),
                "{symbol} がトレース式に現れない"
            );
        }
    }

    #[test]
    fn トレースに全カテゴリが出る() {
        let r = calculate_damage(&input());
        let symbols: Vec<&str> = r.trace.categories.iter().map(|c| c.symbol.as_str()).collect();
        for c in DamageCategory::ALL {
            assert!(symbols.contains(&c.wiki_symbol()));
        }
        // 攻撃力(A)の内訳 4 段(ステ攻撃力/装備攻撃力/装備攻撃力強化倍率/攻撃力(A)) + 従来の 10 段。
        assert_eq!(r.trace.steps_min.len(), 14);
        assert_eq!(r.trace.steps_min.len(), r.trace.steps_critical.len());
        let names: Vec<&str> = r.trace.steps_min.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            &names[..4],
            ["ステ攻撃力", "装備攻撃力", "装備攻撃力強化倍率", "攻撃力(A)"]
        );
    }

    // 受け入れ条件1: 装備補正を入れたキャラは中盤以降の敵(兄弟の鍛冶場相当)でも下限1にならない。
    // ボリス素ステ例(STAB 310 HACK 310)+ 装備(base 突400 斬400、enhanced 突200 斬200、SW Lv6。
    // 基本 400 はネオテシス武器(wiki 装備強化: 蒼穹 410〜888)相当で現実的な値)。
    #[test]
    fn 装備補正があると中盤の敵に対しても下限1にならない() {
        let mut i = input();
        i.base_stats = BaseStats { stab: 310, hack: 310, int: 1, def: 1, mr: 1, dex: 100, agi: 1 };
        i.common_skills = CommonSkills { strong_weapon_level: 6, ..Default::default() };
        i.equipment_base_totals = EquipmentValues { thrust: 400, slash: 400, ..Default::default() };
        i.equipment_enhanced_totals = EquipmentValues { thrust: 200, slash: 200, ..Default::default() };
        i.equipment_coefficients = EquipmentCoefficients {
            base: crate::equipment::EquipmentRates { thrust: 14.5, slash: 14.5, magic_attack: 0.0, magic_defense: 0.0 },
            enhanced: crate::equipment::EquipmentRates {
                thrust: 28.75,
                slash: 28.75,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
        };
        // 兄弟の鍛冶場相当の敵(旧リポ由来。crates/gamedata/src/enemies.rs と同じ値)。
        i.enemy = Enemy {
            id: "brothers_forge".into(),
            name: "兄弟の鍛冶場".into(),
            defense: 7050,
            damage_reduction: -5850,
            cut_rate_a: 0.405,
            agi: Some(1552),
            critical_taken_rate: None,
            element_threshold: 120,
        };
        let r = calculate_damage(&i);
        assert!(r.per_hit.min > 1, "装備ありなら下限1を超えるはず: {:?}", r.per_hit);

        // 回帰確認: 装備 default なら従来どおり下限1のまま。
        let mut without_equipment = i.clone();
        without_equipment.equipment = Equipment::default();
        without_equipment.equipment_base_totals = EquipmentValues::default();
        without_equipment.equipment_enhanced_totals = EquipmentValues::default();
        let r2 = calculate_damage(&without_equipment);
        assert_eq!(r2.per_hit, DamageTriple { min: 1, max: 1, critical: 1 });
    }

    // §5 武器強化の追加固定ダメージ(与ダメージ式の外)。goal 文書の例: 追加 2488・9hit → per-hit 276。
    #[test]
    fn 武器強化の追加固定ダメージはhit数で分割してper_hitとtotalに加算される() {
        let mut i = input();
        i.skill.hit_count = 9;
        i.weapon_added_damage = 2488;
        let base = calculate_damage(&input());
        let r = calculate_damage(&i);
        // INT(2488 / 9) = 276
        assert_eq!(r.per_hit.min, base.per_hit.min + 276);
        assert_eq!(r.per_hit.max, base.per_hit.max + 276);
        assert_eq!(r.per_hit.critical, base.per_hit.critical + 276);
        assert_eq!(r.total.min, r.per_hit.min * 9);
        assert_eq!(r.total.max, r.per_hit.max * 9);
        assert_eq!(r.total.critical, r.per_hit.critical * 9);
        assert_eq!(
            r.trace.steps_min.last().unwrap().name,
            "武器強化(追加固定ダメージ)"
        );
    }

    #[test]
    fn weapon_added_damageが0ならトレース段は増えず挙動は現行と変わらない() {
        let r = calculate_damage(&input());
        assert_eq!(r.trace.steps_min.len(), 14);
        assert_ne!(r.trace.steps_min.last().unwrap().name, "武器強化(追加固定ダメージ)");
    }

    /// マスタリーの「攻撃ダメージ +n%」も同じカテゴリX(称号・ランダムOP と合算)。
    #[test]
    fn マスタリーの攻撃ダメージ増加はXに乗る() {
        let mut i = input();
        i.damage_contributions = vec![(DamageCategory::AttackDamageSkill, 0.05)];
        let r = calculate_damage(&i);
        let x = r.trace.categories.iter().find(|c| c.symbol == "X").unwrap();
        assert!((x.value - 0.05).abs() < 1e-12);
        assert!(r.per_hit.max > calculate_damage(&input()).per_hit.max);
    }

    /// 称号の無条件「ダメージ n% 増加」は wiki: ステータス `#z4747f51` の
    /// [X3] 攻撃ダメージ(基本発動)なので、カテゴリX に合流する。
    #[test]
    fn 称号のダメージ増加はXに乗る() {
        let mut i = input();
        i.random_options.attack_damage_rate = 0.30;
        i.title_attack_damage_rate = 0.20;
        let r = calculate_damage(&i);
        let x = r.trace.categories.iter().find(|c| c.symbol == "X").unwrap();
        // ランダムOP 30% + 称号 20% = Σ +50%
        assert!((x.value - 0.50).abs() < 1e-12);
        assert!((x.factor - 1.50).abs() < 1e-12);
    }

    #[test]
    fn 条件付き称号はper_hitを変えず合計に割合追加ダメージを加える() {
        let base = calculate_damage(&input());
        let mut i = input();
        i.skill.hit_count = 3;
        i.title_added_damage_rate = 0.20;
        let result = calculate_damage(&i);

        assert_eq!(result.per_hit, base.per_hit);
        assert_eq!(result.added_damage_rate, 0.20);
        assert_eq!(result.added_damage.max, (result.per_hit.max * 3) / 5);
        assert_eq!(result.total.max, result.per_hit.max * 3 + result.added_damage.max);
        let step = result.trace.steps_max.iter().find(|s| s.name == "割合追加ダメージ(合計に乗る)").unwrap();
        assert!(step.expression.contains("称号 20%"));
    }

    #[test]
    fn シエナのオーラの攻撃力増加はNew1に乗る() {
        let base = calculate_damage(&input()).per_hit.max;

        let mut i = input();
        i.equipment.parts.weapon.siena = siena_extra(SienaValueKind::Thrust, SienaExtraKind::AttackRate, 10.0);
        i.equipment.parts.armor.siena = siena_extra(SienaValueKind::Stab, SienaExtraKind::AttackRate, 5.0);
        let boosted = calculate_damage(&i);

        // New1 は Σ% = +15% として集計される
        let new1 = boosted
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "New1")
            .expect("New1 のトレースがある");
        assert!((new1.value - 0.15).abs() < 1e-12);
        assert!((new1.factor - 1.15).abs() < 1e-12);
        assert!(boosted.per_hit.max > base);
    }

    #[test]
    fn テシスコアのセット効果は最終ダメージのkとlに乗る() {
        use crate::thesis_core::{CoreRegion, CoreSet, CoreType, ThesisCore, CORE_SLOT_COUNT};

        // 進化0 強化4 が 6 個 → 最終ダメージ(固定値)+800
        let mut i = input();
        *i.equipment.thesis_cores.get_mut(CoreRegion::Abyss) = CoreSet {
            slots: [Some(ThesisCore { core_type: CoreType::Slash, evolution: 0, enhancement: 4 });
                CORE_SLOT_COUNT],
        };
        let fixed = calculate_damage(&i);
        let k = fixed.trace.categories.iter().find(|c| c.symbol == "K").unwrap();
        assert_eq!(k.value, 800.0);

        // 進化4 強化4 が 6 個 → 最終ダメージ +5%(K は 0 に戻る)
        let mut i = input();
        *i.equipment.thesis_cores.get_mut(CoreRegion::Eclipse) = CoreSet {
            slots: [Some(ThesisCore { core_type: CoreType::Slash, evolution: 4, enhancement: 4 });
                CORE_SLOT_COUNT],
        };
        let rate = calculate_damage(&i);
        let l = rate.trace.categories.iter().find(|c| c.symbol == "L").unwrap();
        assert!((l.value - 0.05).abs() < 1e-12);
        assert_eq!(rate.trace.categories.iter().find(|c| c.symbol == "K").unwrap().value, 0.0);
        assert!(rate.per_hit.max > calculate_damage(&input()).per_hit.max);
    }

    // wiki: K は上限 1000。進化1 強化4 の 6 セット(+1,400)はキャップに当たる
    #[test]
    fn テシスコアの最終ダメージ固定値は上限1000でキャップされる() {
        use crate::thesis_core::{CoreRegion, CoreSet, CoreType, ThesisCore, CORE_SLOT_COUNT};

        let mut i = input();
        *i.equipment.thesis_cores.get_mut(CoreRegion::Mercurial) = CoreSet {
            slots: [Some(ThesisCore { core_type: CoreType::Thrust, evolution: 1, enhancement: 4 });
                CORE_SLOT_COUNT],
        };
        let result = calculate_damage(&i);
        let k = result.trace.categories.iter().find(|c| c.symbol == "K").unwrap();
        assert_eq!(k.raw, 1_400.0);
        assert_eq!(k.value, 1_000.0);
    }

    // --- ランダムオプション ---------------------------------------------

    #[test]
    fn ランダムオプションの依存別攻撃力増加は依存が一致したときだけ乗る() {
        use crate::random_option::DependencyRates;

        // スキルは STAB+HACK 依存。一致する枠だけがカテゴリP に入る
        let mut i = input();
        i.random_options.dependency_damage_rate =
            DependencyRates { stab_hack: 0.10, stab: 0.25, ..Default::default() };
        let result = calculate_damage(&i);
        let p = result.trace.categories.iter().find(|c| c.symbol == "P").unwrap();
        assert!((p.value - 0.10).abs() < 1e-12);
        assert!(result.per_hit.max > calculate_damage(&input()).per_hit.max);
    }

    // wiki §4: カテゴリP は上限 +73%
    #[test]
    fn ランダムオプションの依存別攻撃力増加は上限73パーセントで頭打ち() {
        use crate::random_option::DependencyRates;

        let mut i = input();
        i.random_options.dependency_damage_rate =
            DependencyRates { stab_hack: 1.00, ..Default::default() };
        let result = calculate_damage(&i);
        let p = result.trace.categories.iter().find(|c| c.symbol == "P").unwrap();
        assert!((p.raw - 1.00).abs() < 1e-12);
        assert!((p.value - 0.73).abs() < 1e-12);
    }

    #[test]
    fn ランダムオプションの攻撃ダメージ増加はカテゴリXに入る() {
        let mut i = input();
        i.random_options.attack_damage_rate = 0.30;
        let result = calculate_damage(&i);
        let x = result.trace.categories.iter().find(|c| c.symbol == "X").unwrap();
        assert!((x.value - 0.30).abs() < 1e-12);
    }

    #[test]
    fn ランダムオプションの命中率増加は命中Pに足される() {
        let mut i = input();
        i.random_options.accuracy_point = 20;
        let base = calculate_damage(&input()).accuracy_point.unwrap();
        assert_eq!(calculate_damage(&i).accuracy_point.unwrap(), base + 20);
    }

    // --- クリティカル率(wiki: 計算式まとめ `#CriticalChance`)-----------------

    #[test]
    fn クリティカル率は敵のagiと被撃率が両方そろったときだけ出す() {
        let mut i = input();
        i.base_stats.agi = 300;
        i.equipment_base_totals.critical = 200;
        i.skill.critical_rate = Some(13);

        // どちらも未記載(wiki が `?`)なら出さない
        assert!(calculate_damage(&i).critical_rate.is_none());

        // AGI だけでも出さない(被撃率は −250〜−930% と支配的で、無いと桁違いに外れる)
        i.enemy.agi = Some(1420);
        assert!(calculate_damage(&i).critical_rate.is_none());

        i.enemy.critical_taken_rate = Some(-350.0);
        let r = calculate_damage(&i).critical_rate.unwrap();
        assert_eq!(r.equipment_critical, 200);
        assert_eq!(r.agi, 300);
        assert_eq!(r.target_agi, 1420);
        assert!((r.target_taken_rate - -350.0).abs() < 1e-12);

        // スキルの Cri値が wiki 未記載なら出せない
        i.skill.critical_rate = None;
        assert!(calculate_damage(&i).critical_rate.is_none());
    }

    #[test]
    fn クリティカル率増加は結果を押し上げる() {
        use crate::critical_rate::CriticalRateSources;

        let mut i = input();
        i.base_stats.agi = 300;
        i.equipment_base_totals.critical = 200;
        i.enemy.agi = Some(1420);
        i.enemy.critical_taken_rate = Some(-100.0);
        let base = calculate_damage(&i).critical_rate.unwrap();

        i.critical_rate_sources = CriticalRateSources {
            pet: true,
            ultimate_rune: true,
            architect_lab_stage: 0,
            deadly_blow: false,
        };
        let boosted = calculate_damage(&i).critical_rate.unwrap();
        assert!((boosted.bonus - 20.0).abs() < 1e-12);
        assert!(boosted.raw > base.raw);
    }

    // wiki Quest/覚醒クエスト「各能力の上限値」/ エタの意志: 最終能力値の上限
    #[test]
    fn 最終能力値の上限は攻撃力に効きトレースに捨てた分が出る() {
        let mut i = input();
        let uncapped = calculate_damage(&i);
        assert!(uncapped.trace.stats.iter().all(|t| t.capped_loss == 0));

        i.stat_cap = 400;
        let capped = calculate_damage(&i);
        let stab = capped.trace.stats.iter().find(|t| t.kind == StatKind::Stab).unwrap();
        assert_eq!(stab.effective, 400);
        assert_eq!(stab.stat_cap, 400);
        assert_eq!(stab.capped_loss, 100); // 素ステ 500 → 上限 400
        // ステ攻撃力が減るので与ダメージも減る
        assert!(capped.per_hit.max < uncapped.per_hit.max);
    }

    // --- 中ディレイ・DPS(wiki: 計算式まとめ `#ActualDelay`)------------------

    #[test]
    fn 中ディレイの供給源はフルスロットルとroとシエナとキャラパッシブ() {
        use crate::actual_delay::ActualDelayContribution;
        use crate::ultimate_skill::{UltimateSkill, UltimateSkills};

        let mut i = input();
        i.common_skills.ultimate = UltimateSkills {
            slots: [Some(UltimateSkill::FullThrottle), None],
            super_limit: true,
            hyper_limit_level: 6,
        };
        i.random_options.actual_delay_reduction = 0.03;
        i.equipment.parts.shield.siena =
            siena_extra(SienaValueKind::Thrust, SienaExtraKind::ActualDelay, 2.0);
        i.actual_delay_skills =
            vec![ActualDelayContribution { source: "剣の司祭".into(), rate: 0.05 }];

        let delay = calculate_damage(&i).actual_delay.unwrap();
        assert_eq!(delay.contributions.len(), 4);
        // フルスロットル 45% + RO 3% + シエナ 2% + パッシブ 5% = 55%
        assert!((delay.reduction - 0.55).abs() < 1e-12);
        assert!((delay.value - 1.4 * 0.45).abs() < 1e-12);
    }

    #[test]
    fn dpsは合計ダメージを中ディレイで割る() {
        let result = calculate_damage(&input());
        let delay = result.actual_delay.unwrap();
        let dps = result.dps.unwrap();
        assert!((dps.max - result.total.max as f64 / delay.value).abs() < 1e-9);
        assert!((dps.critical - result.total.critical as f64 / delay.value).abs() < 1e-9);
    }

    #[test]
    fn 動作が取れないスキルは中ディレイもdpsも出さない() {
        let mut i = input();
        i.skill.base_actual_delay = None;
        let result = calculate_damage(&i);
        assert!(result.actual_delay.is_none());
        assert!(result.dps.is_none());
    }

    // --- 極限スキル(wiki: Skill/極限)-----------------------------------

    #[test]
    fn スコープアイはクリティカル時だけダメージを増やす() {
        use crate::ultimate_skill::{UltimateSkill, UltimateSkills};

        let mut i = input();
        i.common_skills.ultimate = UltimateSkills {
            slots: [Some(UltimateSkill::ScopeEye), None],
            super_limit: true,
            hyper_limit_level: 6,
        };
        let base = calculate_damage(&input());
        let with_scope = calculate_damage(&i);
        // 非クリティカルは {F × G} ごと 1.0 なので変わらない
        assert_eq!(with_scope.per_hit.max, base.per_hit.max);
        // クリティカルは G +40% ぶん増える
        assert!(with_scope.per_hit.critical > base.per_hit.critical);
        let g = with_scope.trace.categories.iter().find(|c| c.symbol == "G").unwrap();
        assert!((g.value - 0.40).abs() < 1e-12);
    }

    #[test]
    fn フルスロットルの段数は単体チャネリングスキルにだけ乗る() {
        use crate::ultimate_skill::{UltimateSkill, UltimateSkills};

        let ultimate = UltimateSkills {
            slots: [Some(UltimateSkill::FullThrottle), None],
            super_limit: true,
            hyper_limit_level: 6,
        };

        // 単体チャネリングではないスキル(テスト既定)は段数が変わらない
        let mut plain = input();
        plain.common_skills.ultimate = ultimate;
        assert_eq!(calculate_damage(&plain).hit_count, input().skill.hit_count);

        // 単体チャネリングなら +3
        let mut channeling = input();
        channeling.skill.hit_count = 10;
        channeling.skill.single_target_channeling = true;
        let without = calculate_damage(&channeling);
        channeling.common_skills.ultimate = ultimate;
        let with_throttle = calculate_damage(&channeling);
        assert_eq!(without.hit_count, 10);
        assert_eq!(with_throttle.hit_count, 13);
        assert_eq!(with_throttle.total.max, with_throttle.per_hit.max * 13);
    }

    #[test]
    fn ワイドフォーカスは与ダメージを変えない() {
        use crate::ultimate_skill::{UltimateSkill, UltimateSkills};

        let mut i = input();
        i.skill.single_target_channeling = true;
        i.common_skills.ultimate = UltimateSkills {
            slots: [Some(UltimateSkill::WideFocus), None],
            super_limit: true,
            hyper_limit_level: 6,
        };
        let mut base = input();
        base.skill.single_target_channeling = true;
        assert_eq!(calculate_damage(&i).total, calculate_damage(&base).total);
    }
}
