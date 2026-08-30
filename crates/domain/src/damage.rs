//! 与ダメージ計算(docs/damage-formula.md §3)。
//!
//! ①能力値計算 → ②カテゴリ集計 → ③式の評価 → ④段数 の 4 段をここで束ねる。

use serde::{Deserialize, Serialize};

use crate::actual_delay::{
    actual_delay, ActualDelay, ActualDelayContribution, SkillUsesTable, SECONDS_PER_MINUTE,
};
use crate::attack_power::{
    attack_power_breakdown, random_part_max, stat_attack_parts, stat_attack_power,
    AttackCoefficients, AttackPowerBreakdown, StatAttackPart,
};
use crate::category::{CategoryTotals, CategoryTrace, DamageCategory};
use crate::common_skill::{CommonSkills, RateContribution};
use crate::critical_rate::{critical_rate, CriticalRate, CriticalRateSources};
use crate::defense::{accuracy_point, AccuracyCorrection};
use crate::enemy::Enemy;
use crate::equipment::{
    equipment_attack_parts, equipment_values_attack, sum_equipment_value_sources, Equipment,
    EquipmentAttackPart, EquipmentCoefficients, EquipmentValueSource, EquipmentValues,
};
use crate::random_option::RandomOptionTotals;
use crate::rounding::{floor_int, trunc2};
use crate::skill::Skill;
use crate::stat_sources::{apply_pins, fill_contribution_effects, Adjustments, StatContribution};
use crate::stats::{effective_stats, BaseStats, StatModifierSet, StatTrace};

/// 与ダメージ式のカテゴリ集計 1 行ぶんの寄与(トレース表示用)。「なぜこの数字?」パネルの
/// カテゴリ材料行の掘り下げ(供給源一覧)に使う。`source` はスキル名・マスタリー名・
/// バフ名・アビリティ名など人が読める名前
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageContribution {
    pub source: String,
    pub category: DamageCategory,
    pub value: f64,
}

/// 3 コンボ以上で付くコンボボーナス(wiki: カテゴリH)。
pub const COMBO_BONUS_RATE: f64 = 0.15;
/// コンボボーナスが付くコンボ数。
pub const COMBO_BONUS_THRESHOLD: u32 = 3;
/// 属性差 1 あたりの属性差ボーナス(%)(wiki: カテゴリI)。
pub const ELEMENT_BONUS_PERCENT_PER_POINT: f64 = 0.625;
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
    /// 装備の基本能力値の供給源内訳(`Equipment::base_sources`。部位実測値・部位アビリティ・
    /// 称号・手首補正。呼び出し側が gamedata の武器アビリティカタログを使って集計して渡す。
    /// domain は gamedata に依存できないため)。合計は `equipment_base_totals()` を使う
    /// (合計と内訳の二重持ちはしない)
    pub equipment_base_sources: Vec<EquipmentValueSource>,
    /// 装備の強化能力値の供給源内訳(`Equipment::enhanced_sources`。エンチャント・シエナの
    /// オーラ・対象コンテンツの地域のテシスコア。地域の解決は呼び出し側が行う)。合計は
    /// `equipment_enhanced_totals()` を使う
    pub equipment_enhanced_sources: Vec<EquipmentValueSource>,
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
    /// E1/E2 スキル倍率増加 …)ので、値だけでなく**どのカテゴリか**を持つ。`source` は
    /// トレースの「カテゴリ供給源内訳」にそのまま使う
    pub damage_contributions: Vec<DamageContribution>,
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
    /// 計算リクエストの一時調整(キャラには保存しない)。能力値の固定(pin)を含む
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
        equipment_base_sources: Vec<EquipmentValueSource>,
        equipment_enhanced_sources: Vec<EquipmentValueSource>,
        equipment_coefficients: EquipmentCoefficients,
        accuracy_correction: AccuracyCorrection,
        random_options: RandomOptionTotals,
        title_attack_damage_rate: f64,
        title_added_damage_rate: f64,
        damage_contributions: Vec<DamageContribution>,
        weapon_added_damage: i64,
        awakening_rate: f64,
        damage_cap: i64,
        stat_cap: i64,
        skill: Skill,
        enemy: Enemy,
        combo_count: u32,
        element_value: i64,
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
            equipment_base_sources,
            equipment_enhanced_sources,
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
            temporary_pins,
            actual_delay_skills,
            critical_rate_sources,
            skill_uses,
        }
    }

    /// 装備の基本能力値の合計(`equipment_base_sources` の Σ。計算を二重に書かない)。
    pub fn equipment_base_totals(&self) -> EquipmentValues {
        sum_equipment_value_sources(&self.equipment_base_sources)
    }

    /// 装備の強化能力値の合計(`equipment_enhanced_sources` の Σ)。
    pub fn equipment_enhanced_totals(&self) -> EquipmentValues {
        sum_equipment_value_sources(&self.equipment_enhanced_sources)
    }
}

/// 式の 1 段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FormulaStep {
    pub name: String,
    pub expression: String,
    pub value: f64,
    /// この段を終えた時点の到達値(式の途中積)。倍率だけを返す段でも、ここまでの積を
    /// UI が再計算せずに読めるようにする。式の外の段(攻撃力の内訳など)は `value` と同じ
    pub reached: f64,
    /// この段が消費したカテゴリ(wiki §3)。UI が「この段の材料」を引くのに使う。
    /// 式の外で足す段(攻撃力の内訳・武器強化・ダメージ上限など)は空
    pub categories: Vec<DamageCategory>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageTriple {
    pub min: i64,
    pub max: i64,
    pub critical: i64,
}

impl DamageTriple {
    /// 主役のダメージ値(計算タブ・ホームの表示、コンテンツ到達判定が使う値)。
    /// クリ発生率(`critical_chance`、0..1)が 0 より大きいならクリティカル値、
    /// 0(クリが出ないスキル)なら非クリの最大値を返す(ユーザー判断 2026-08-29)。
    /// この選択規則の実装はここ 1 箇所だけにし、他はすべてこれを呼ぶ。
    pub fn primary(&self, critical_chance: f64) -> i64 {
        if critical_chance > 0.0 { self.critical } else { self.max }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageTrace {
    pub stats: Vec<StatTrace>,
    /// 攻撃力(A)の内訳(ステ攻撃力 / 装備攻撃力 / 強化倍率の加算分)
    pub attack: AttackPowerBreakdown,
    /// ステ補正源(ペット/ルーン/クラウン/聖物/バフ/調整値)の寄与内訳。
    /// `effect` に「そのステを何ポイント動かしたか」の実数が入っている
    pub stat_contributions: Vec<StatContribution>,
    /// ステ攻撃力に実際に使っている依存ステごとの内訳(能力値・係数・寄与)。
    /// 合計 = ステ攻撃力
    pub stat_attack_parts: Vec<StatAttackPart>,
    /// 最大乱数(B = 最大)時のカテゴリ集計
    pub categories: Vec<CategoryTrace>,
    /// カテゴリ集計に実際に値を足した供給源の一覧(非 0 のみ)。「なぜこの数字?」パネルの
    /// カテゴリ材料行を掘り下げたときの供給源表に使う
    pub category_contributions: Vec<DamageContribution>,
    /// 装備攻撃力の内訳(層 × 装備値種別)。Σcontribution = 装備攻撃力
    pub equipment_attack_parts: Vec<EquipmentAttackPart>,
    /// 装備攻撃力強化倍率の供給源(パワーウェポン/ストロングウェポン)。Σvalue = 強化倍率
    pub equipment_enhance_sources: Vec<RateContribution>,
    pub steps_min: Vec<FormulaStep>,
    pub steps_max: Vec<FormulaStep>,
    pub steps_critical: Vec<FormulaStep>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageResult {
    /// 1 段あたりの与ダメージ(スキル分のみ。ダメージ上限を適用したあと)。
    /// ゲームの表記ダメージに相当し、武器強化の追加固定ダメージは含まない
    pub per_hit: DamageTriple,
    /// 実際に敵へ入る総量: `skill_total + weapon_added_total + 割合追加ダメージ`
    pub total: DamageTriple,
    /// 与ダメージ(表記ダメージ)の合計 = `per_hit × 段数`
    pub skill_total: DamageTriple,
    /// 武器強化の追加固定ダメージの合計 = `weapon_added_per_hit × 段数`
    pub weapon_added_total: i64,
    /// 武器の装備強化による追加固定ダメージ(1 段あたり。`INT(weapon_added_damage / 段数)`)。
    /// 与ダメージ式の外・ダメージ上限の対象外で、ゲームは表記ダメージと別枠で表示する
    pub weapon_added_per_hit: i64,
    /// 主役の 1 段あたりダメージ(`per_hit.primary(critical_chance)`)。ゲームの表記ダメージ
    /// (スキル分のみ・武器強化の追加固定ダメージを含まない)。計算タブ・ホームが表示に使う値で、
    /// コンテンツ到達判定もこの値で判定する(ユーザー判断 2026-08-29 / 2026-08-30)
    pub per_hit_primary: i64,
    /// 主役の合計ダメージ(`total.primary(critical_chance)`)
    pub total_primary: i64,
    pub hit_count: u32,
    /// コンボスキルタイプを解決したあとのスキル倍率。
    pub effective_skill_multiplier: f64,
    /// コンボスキルタイプを解決したあとの基本中ディレイ。
    pub effective_base_actual_delay: Option<f64>,
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
    /// クリティカル率(0..1)。`critical_rate` が `Some` ならその値を 0..1 にクランプしたもの。
    /// wiki 未記載で `critical_rate` が `None` のときは **1.0(クリティカル確定扱い)**
    /// (未記載は確定扱い(ユーザー判断 2026-08-29))。
    pub critical_chance: f64,
    /// クリ率を考慮した DPS の期待値(`dps.max × (1 − p) + dps.critical × p`)。
    /// `dps` が `None` なら `None`
    pub expected_dps: Option<f64>,
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
    let mut step = |name: &str,
                    expression: String,
                    value: f64,
                    reached: f64,
                    categories: Vec<DamageCategory>| {
        steps.push(FormulaStep {
            name: name.to_string(),
            expression,
            value,
            reached,
            categories,
        });
        value
    };

    let base = step(
        "攻撃力−防御力",
        format!(
            "A {} + B {} − C {}",
            g(AttackPower),
            g(AttackRandom),
            g(TargetDefense)
        ),
        g(AttackPower) + g(AttackRandom) - g(TargetDefense),
        g(AttackPower) + g(AttackRandom) - g(TargetDefense),
        vec![AttackPower, AttackRandom, TargetDefense],
    );
    let skill = step(
        "スキル倍率",
        format!(
            "{{D {} × E1 {} + E2 {}}}",
            g(SkillMultiplier),
            g(SkillMultiplierRate),
            g(SkillMultiplierFixed)
        ),
        trunc2(g(SkillMultiplier) * g(SkillMultiplierRate) + g(SkillMultiplierFixed)),
        base * trunc2(g(SkillMultiplier) * g(SkillMultiplierRate) + g(SkillMultiplierFixed)),
        vec![SkillMultiplier, SkillMultiplierRate, SkillMultiplierFixed],
    );
    let crit = if critical {
        step(
            "クリティカル",
            format!(
                "{{F {} × G {}}}",
                g(CriticalMultiplier),
                g(CriticalDamageRate)
            ),
            trunc2(g(CriticalMultiplier) * g(CriticalDamageRate)),
            base * skill * trunc2(g(CriticalMultiplier) * g(CriticalDamageRate)),
            vec![CriticalMultiplier, CriticalDamageRate],
        )
    } else {
        // 非クリティカル時は `{F×G}` ごと 1.0。F(Cri倍率)はクリティカル時にだけ代入され、
        // G(クリティカルダメージ増加)の供給源は wiki ステータス [G] の表がすべて
        // 「クリティカルダメージ増加」(スコープアイ / 致命のルーン / ソウルリンク / 称号 /
        // プシーキーの刻印)で、非クリティカルの一撃には乗らない(取得 2026-08-25)。
        step(
            "クリティカル",
            "非クリティカル({F × G} = 1.0)".to_string(),
            1.0,
            base * skill,
            vec![CriticalMultiplier, CriticalDamageRate],
        )
    };
    let bonus = step(
        "コンボ・属性・カット率・オーラ",
        format!(
            "H {} × I {} × J {} × New1 {}",
            g(ComboBonus),
            g(ElementBonus),
            g(PlayerCutRate),
            g(SienaAuraAttackRate)
        ),
        g(ComboBonus) * g(ElementBonus) * g(PlayerCutRate) * g(SienaAuraAttackRate),
        base * skill * crit * g(ComboBonus) * g(ElementBonus) * g(PlayerCutRate) * g(SienaAuraAttackRate),
        vec![ComboBonus, ElementBonus, PlayerCutRate, SienaAuraAttackRate],
    );
    let product = base * skill * crit * bonus;
    let inner = step(
        "最終ダメージ固定値(下限)",
        format!("MAX({product:.4} + K {k}, K {k})", k = g(FinalDamageFixed)),
        (product + g(FinalDamageFixed)).max(g(FinalDamageFixed)),
        (product + g(FinalDamageFixed)).max(g(FinalDamageFixed)),
        vec![FinalDamageFixed],
    );
    let mid = step(
        "最終ダメージ・カット率A・被害減少",
        format!(
            "{inner:.4} × L {} × V1 {} + M {}",
            g(FinalDamageRate),
            g(CutRateA),
            g(DamageReduction)
        ),
        inner * g(FinalDamageRate) * g(CutRateA) + g(DamageReduction),
        inner * g(FinalDamageRate) * g(CutRateA) + g(DamageReduction),
        vec![FinalDamageRate, CutRateA, DamageReduction],
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
        mid * outer_factors + g(BasicTriggerDamageFixed),
        vec![
            AttackDamageLegacy,
            AwakeningDamage,
            PhysicalMagicDamageRate,
            DependencyDamageRate,
            DamageAbsorb,
            TakenDamageRate,
            TakenDamageReduction,
            DamageAmplify,
            DamageResistance,
            DamageMitigation,
            CutRateB,
            BasicTriggerDamageFixed,
        ],
    );
    let final_value = step(
        "攻撃ダメージ・PVP補正",
        format!(
            "{outer:.4} × X {} × Y {}",
            g(AttackDamageRate),
            g(PvpCorrection)
        ),
        outer * g(AttackDamageRate) * g(PvpCorrection),
        outer * g(AttackDamageRate) * g(PvpCorrection),
        vec![AttackDamageRate, PvpCorrection],
    );
    let floored = floor_int(final_value);
    step(
        "切捨て",
        format!("[{final_value:.4}]"),
        floored as f64,
        floored as f64,
        Vec::new(),
    );
    let damage = floored.max(MIN_DAMAGE_TO_MONSTER);
    step(
        "対モンスター下限",
        format!("MAX({floored}, {MIN_DAMAGE_TO_MONSTER})"),
        damage as f64,
        damage as f64,
        Vec::new(),
    );
    (damage, steps)
}

/// `totals.add` と同時にトレース(`DamageContribution`)へも積む。非 0 のみ積むことで
/// 「値を入れた供給源」だけを掘り下げ表示に出す。加算とトレースがずれないよう、
/// `calculate_damage` はこの経由でだけ `totals` に値を足す(カテゴリ集計に使わない
/// 内訳専用の値は対象外)。
fn add_traced(
    totals: &mut CategoryTotals,
    trace: &mut Vec<DamageContribution>,
    category: DamageCategory,
    source: &str,
    value: f64,
) {
    totals.add(category, value);
    if value != 0.0 {
        trace.push(DamageContribution {
            source: source.to_string(),
            category,
            value,
        });
    }
}

pub fn calculate_damage(input: &DamageInput) -> DamageResult {
    use DamageCategory::*;

    // ① 能力値計算
    let (mut stats, mut stat_traces) =
        effective_stats(&input.base_stats, &input.stat_modifiers, input.stat_cap);
    apply_pins(&mut stats, &mut stat_traces, input.temporary_pins.as_ref());

    // ② カテゴリ集計
    let attack_parts = stat_attack_parts(&stats, &input.coefficients);
    let stat_attack = stat_attack_power(&stats, &input.coefficients);
    let equipment_base_totals = input.equipment_base_totals();
    let equipment_enhanced_totals = input.equipment_enhanced_totals();
    let attack = attack_power_breakdown(
        stat_attack,
        equipment_values_attack(&equipment_base_totals, &input.equipment_coefficients.base),
        equipment_values_attack(
            &equipment_enhanced_totals,
            &input.equipment_coefficients.enhanced,
        ),
        input.common_skills.equipment_attack_rate(),
    );
    let mut totals = CategoryTotals::neutral();
    let mut category_contributions: Vec<DamageContribution> = Vec::new();
    totals.add(AttackPower, attack.value as f64);
    add_traced(
        &mut totals,
        &mut category_contributions,
        TargetDefense,
        "敵の防御力",
        input.enemy.defense as f64,
    );
    add_traced(
        &mut totals,
        &mut category_contributions,
        SkillMultiplier,
        "スキル倍率",
        input.skill.multiplier,
    );
    add_traced(
        &mut totals,
        &mut category_contributions,
        CriticalMultiplier,
        "スキルのクリティカル倍率",
        input.skill.critical_multiplier,
    );
    if input.combo_count >= COMBO_BONUS_THRESHOLD {
        add_traced(
            &mut totals,
            &mut category_contributions,
            ComboBonus,
            "コンボボーナス(3コンボ以上)",
            COMBO_BONUS_RATE,
        );
    }
    add_traced(
        &mut totals,
        &mut category_contributions,
        ElementBonus,
        "属性差ボーナス",
        element_bonus_rate(input.element_value, input.enemy.element_threshold),
    );
    add_traced(
        &mut totals,
        &mut category_contributions,
        DamageReduction,
        "敵の被害減少",
        input.enemy.damage_reduction as f64,
    );
    add_traced(
        &mut totals,
        &mut category_contributions,
        AwakeningDamage,
        "覚醒",
        input.awakening_rate - 1.0,
    );
    add_traced(
        &mut totals,
        &mut category_contributions,
        CutRateA,
        "敵のカット率A",
        input.enemy.cut_rate_a - 1.0,
    );
    // シエナのオーラの追加オプション「攻撃力増加」(wiki: New1。実際は与ダメージ割合増加)
    add_traced(
        &mut totals,
        &mut category_contributions,
        SienaAuraAttackRate,
        "シエナのオーラ【攻撃力増加】",
        input.equipment.siena_attack_rate(),
    );
    // テシスコアのセット効果(wiki: コアセット効果。全地域で発動するので対象コンテンツの地域を問わない)
    let core_set_bonus = input.equipment.thesis_cores.set_bonus();
    add_traced(
        &mut totals,
        &mut category_contributions,
        FinalDamageFixed,
        "テシスコア セット効果",
        core_set_bonus.final_damage_fixed as f64,
    );
    add_traced(
        &mut totals,
        &mut category_contributions,
        FinalDamageRate,
        "テシスコア セット効果",
        core_set_bonus.final_damage_rate,
    );
    // ランダムオプション(wiki: ランダムオプション)。依存別攻撃力増加はスキルの依存種別が
    // 一致したときだけ乗る(カテゴリP)、攻撃ダメージ増加はカテゴリX
    add_traced(
        &mut totals,
        &mut category_contributions,
        DependencyDamageRate,
        "ランダムOP【依存別攻撃力増加】",
        input
            .random_options
            .dependency_damage_rate
            .get(input.skill.dependency),
    );
    add_traced(
        &mut totals,
        &mut category_contributions,
        DamageAmplify,
        "ランダムOP【ダメージ増幅】",
        input
            .random_options
            .damage_amplify_for(input.skill.dependency),
    );
    // カテゴリX は X1〜X6 の合計で、**上限が子ごとに違う**(X3 +80% / X4 +65% / X5 未記載)。
    // 親の `AttackDamageRate` は子の合計として読み出されるので、ここでは子に足す
    add_traced(
        &mut totals,
        &mut category_contributions,
        AttackDamageBasicTrigger,
        "称号【ダメージ増加】",
        input.title_attack_damage_rate,
    );
    add_traced(
        &mut totals,
        &mut category_contributions,
        AttackDamageSpecial,
        "ランダムOP【攻撃ダメージ増加(特殊)】",
        input.random_options.attack_damage_rate,
    );
    // キャラスキル・マスタリー・バフ・装備アビリティ・装備アイテム。効き先はカテゴリごとに違う
    for c in &input.damage_contributions {
        add_traced(
            &mut totals,
            &mut category_contributions,
            c.category,
            &c.source,
            c.value,
        );
    }
    // 極限スキル「スコープアイ」(wiki: Skill/極限)。カテゴリG はクリティカル時にだけ乗る
    add_traced(
        &mut totals,
        &mut category_contributions,
        CriticalDamageRate,
        "極限【スコープアイ】",
        input.common_skills.ultimate.critical_damage_rate(),
    );

    let mut totals_min = totals.clone();
    totals_min.add(AttackRandom, 1.0);
    let mut totals_max = totals;
    add_traced(
        &mut totals_max,
        &mut category_contributions,
        AttackRandom,
        "乱数(最大)",
        random_part_max(stat_attack, stats.dex),
    );

    // ③ 式の評価
    let (min, steps_min) = evaluate(&totals_min, false);
    let (max, steps_max) = evaluate(&totals_max, false);
    let (critical, steps_critical) = evaluate(&totals_max, true);

    // 攻撃力(A)の内訳(ステ攻撃力/装備攻撃力/装備攻撃力強化倍率)。A は B(乱数)を含まないため
    // min/max/critical のすべてで同じ内訳になる。`evaluate` は `totals` からしか値を作れず
    // 内訳を持たないため、ここで先頭に付け足す。
    let attack_breakdown = attack_power_breakdown_steps(&attack);
    let mut steps_min: Vec<FormulaStep> =
        attack_breakdown.iter().cloned().chain(steps_min).collect();
    let mut steps_max: Vec<FormulaStep> =
        attack_breakdown.iter().cloned().chain(steps_max).collect();
    let mut steps_critical: Vec<FormulaStep> =
        attack_breakdown.into_iter().chain(steps_critical).collect();

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

    // ダメージ上限(wiki: Quest/覚醒クエスト。多段スキルでも 1 段ごとに適用)。
    // **与ダメージ(スキル分)にだけ効く**(docs/damage-formula.md §3「『与ダメージ』には
    // ダメージ上限がある。『追加ダメージ』には上限がない」)。武器強化の追加固定ダメージは
    // 式の外・上限の対象外なので、上限はここで先に掛ける。
    // 捨てられた分は 0 と区別できるように別で持つ(UI が「上限で捨てた分」を出す)。
    let cap = |value: i64| value.min(input.damage_cap);
    let (capped_min, capped_max, capped_critical) = (cap(min), cap(max), cap(critical));
    let capped_loss = DamageTriple {
        min: min - capped_min,
        max: max - capped_max,
        critical: critical - capped_critical,
    };
    if capped_loss.max > 0 {
        let step = FormulaStep {
            name: "ダメージ上限".to_string(),
            expression: format!("MIN(生値, {}) ※1 段ごとに適用", input.damage_cap),
            value: input.damage_cap as f64,
            reached: input.damage_cap as f64,
            categories: Vec::new(),
        };
        steps_min.push(step.clone());
        steps_max.push(step.clone());
        steps_critical.push(step);
    }
    let (min, max, critical) = (capped_min, capped_max, capped_critical);

    // §5 武器強化の追加固定ダメージ(与ダメージ式の外・ダメージ上限の対象外)。
    // 1 体あたり per-hit 追加 = INT(追加ダメージ / hits)、合計追加 = per-hit 追加 × hits
    // (wiki: 装備システム/装備強化のヒット分割仕様)。ゲームは表記ダメージ(与ダメージ)と
    // この追加ダメージを別々に表示するため、`per_hit` には足し込まない。
    let weapon_added_per_hit = if hit_count > 0 {
        input.weapon_added_damage / hits
    } else {
        0
    };
    if weapon_added_per_hit != 0 {
        let step = FormulaStep {
            name: "武器強化(追加固定ダメージ)".to_string(),
            expression: format!(
                "INT({} / {hits}) = {weapon_added_per_hit} ※上限なし・式の外",
                input.weapon_added_damage
            ),
            value: weapon_added_per_hit as f64,
            reached: weapon_added_per_hit as f64,
            categories: Vec::new(),
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
            equipment_base_totals.accuracy + equipment_enhanced_totals.accuracy,
            skill_accuracy,
            input.random_options.accuracy_point,
        )
    });

    // §5 割合追加ダメージ(新-割合)。「合計ダメージ(与ダメージ合計 + 武器強化追加合計)」に
    // 掛かるので、1 段ごとではなく段数を掛けたあとの合計に乗せる。
    // 供給源はシャープネスビジョン、武器のランダムOP、対象条件に一致した称号。
    // OP 側は発動条件を満たしている前提で入れる。
    let random_option_added_rate = input
        .random_options
        .added_damage_rate_for(input.skill.dependency);
    let added_rate = input.common_skills.sharpness_vision_rate()
        + random_option_added_rate
        + input.title_added_damage_rate;
    let sum = DamageTriple {
        min: min * hits,
        max: max * hits,
        critical: critical * hits,
    };
    let weapon_added_total = weapon_added_per_hit * hits;
    let added_damage_base = DamageTriple {
        min: sum.min + weapon_added_total,
        max: sum.max + weapon_added_total,
        critical: sum.critical + weapon_added_total,
    };
    let added = DamageTriple {
        min: floor_int(added_damage_base.min as f64 * added_rate),
        max: floor_int(added_damage_base.max as f64 * added_rate),
        critical: floor_int(added_damage_base.critical as f64 * added_rate),
    };
    if added.max != 0 {
        let step = FormulaStep {
            name: "割合追加ダメージ(合計に乗る)".to_string(),
            expression: format!(
                "合計 × {:.0}% ※シャープネスビジョン {:.0}% + ランダムOP {:.0}% + 称号 {:.0}%",
                added_rate * 100.0,
                input.common_skills.sharpness_vision_rate() * 100.0,
                random_option_added_rate * 100.0,
                input.title_added_damage_rate * 100.0
            ),
            value: added_rate,
            reached: added_rate,
            categories: Vec::new(),
        };
        steps_min.push(step.clone());
        steps_max.push(step.clone());
        steps_critical.push(step);
    }

    // クリティカル率(wiki: 計算式まとめ `#CriticalChance`)。与ダメージ式には入らないが、
    // 「クリティカル ×N」がどれくらいの頻度で出るのかを読めるように結果に載せる。
    // 対象のAGI・クリティカル被撃率(狩り場情報一覧)は `?` の行が多く、被撃率は −250〜−930% と
    // 支配的なので、**両方そろっている敵でだけ**出す。スキルの Cri値が未記載でも出さない。
    let critical_chance = match (
        input.enemy.agi,
        input.enemy.critical_taken_rate,
        input.skill.critical_rate,
    ) {
        (Some(target_agi), Some(taken_rate), Some(skill_critical_rate)) => Some(critical_rate(
            equipment_base_totals.critical + equipment_enhanced_totals.critical,
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
            contributions.push(ActualDelayContribution {
                source: "フルスロットル".into(),
                rate: full_throttle,
            });
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
            contributions.push(ActualDelayContribution {
                source: "シエナのオーラ".into(),
                rate: siena,
            });
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
        min: sum.min + weapon_added_total + added.min,
        max: sum.max + weapon_added_total + added.max,
        critical: sum.critical + weapon_added_total + added.critical,
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

    // クリ率(0..1)。未記載(`critical_chance` が `None`)は確定扱い(ユーザー判断 2026-08-29)。
    let critical_chance_ratio = match &critical_chance {
        Some(c) => (c.value / 100.0).clamp(0.0, 1.0),
        None => 1.0,
    };
    let expected_dps = dps
        .as_ref()
        .map(|d| d.max * (1.0 - critical_chance_ratio) + d.critical * critical_chance_ratio);

    let per_hit = DamageTriple { min, max, critical };
    DamageResult {
        per_hit,
        total,
        skill_total: sum,
        weapon_added_total,
        weapon_added_per_hit,
        per_hit_primary: per_hit.primary(critical_chance_ratio),
        total_primary: total.primary(critical_chance_ratio),
        hit_count,
        effective_skill_multiplier: input.skill.multiplier,
        effective_base_actual_delay: input.skill.base_actual_delay,
        damage_cap: input.damage_cap,
        capped_loss,
        added_damage_rate: added_rate,
        added_damage: added,
        accuracy_point: accuracy,
        critical_rate: critical_chance,
        actual_delay: delay,
        dps,
        critical_chance: critical_chance_ratio,
        expected_dps,
        trace: DamageTrace {
            stats: stat_traces,
            attack,
            stat_contributions: {
                let mut contributions = input.stat_contributions.clone();
                fill_contribution_effects(
                    &mut contributions,
                    &input.base_stats,
                    &input.stat_modifiers,
                );
                contributions
            },
            stat_attack_parts: attack_parts,
            categories: totals_max.trace(),
            category_contributions,
            equipment_attack_parts: equipment_attack_parts(
                &input.equipment_base_sources,
                &input.equipment_enhanced_sources,
                &input.equipment_coefficients,
            ),
            equipment_enhance_sources: input.common_skills.equipment_attack_rate_sources(),
            steps_min,
            steps_max,
            steps_critical,
        },
    }
}

/// 攻撃力(A)の内訳を表す `FormulaStep` 4 件(ステ攻撃力/装備攻撃力/装備攻撃力強化倍率/攻撃力(A))。
fn attack_power_breakdown_steps(attack: &AttackPowerBreakdown) -> Vec<FormulaStep> {
    let AttackPowerBreakdown {
        stat_attack,
        enhance_rate,
        ..
    } = *attack;
    let equipment_attack = attack.equipment_attack();
    vec![
        FormulaStep {
            name: "ステ攻撃力".to_string(),
            expression: format!("{stat_attack:.4}"),
            value: stat_attack,
            reached: stat_attack,
            categories: Vec::new(),
        },
        FormulaStep {
            name: "装備攻撃力".to_string(),
            expression: format!(
                "基本 {:.4} + 強化 {:.4}",
                attack.equipment_base_attack, attack.equipment_enhanced_attack
            ),
            value: equipment_attack,
            reached: equipment_attack,
            categories: Vec::new(),
        },
        FormulaStep {
            name: "装備攻撃力強化倍率".to_string(),
            expression: format!("{enhance_rate:.4}"),
            value: enhance_rate,
            reached: enhance_rate,
            categories: Vec::new(),
        },
        FormulaStep {
            name: "攻撃力(A)".to_string(),
            expression: format!(
                "[{stat_attack:.4} + {equipment_attack:.4}] + [{equipment_attack:.4}/25 × {enhance_rate:.4}] × 25"
            ),
            value: attack.value as f64,
            reached: attack.value as f64,
            categories: Vec::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::CategoryKind;
    use crate::equipment::PartSlot;
    use crate::siena::{
        RegisteredSienaAura, SienaAura, SienaExtraKind, SienaExtraSlot, SienaSlot, SienaValueKind,
    };
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
    fn set_siena(equipment: &mut Equipment, slot: PartSlot, aura: SienaAura) {
        let list = equipment.siena.get_mut(slot).expect("シエナ対象部位");
        list.registered = vec![RegisteredSienaAura {
            id: 1,
            label: String::new(),
            aura,
        }];
        list.selected_id = Some(1);
    }
    use crate::stats::StatKind;

    fn input() -> DamageInput {
        DamageInput {
            base_stats: BaseStats {
                stab: 500,
                hack: 500,
                int: 0,
                def: 0,
                mr: 0,
                dex: 100,
                agi: 0,
            },
            stat_modifiers: StatModifierSet::default(),
            stat_contributions: Vec::new(),
            coefficients: AttackCoefficients {
                primary: (StatKind::Stab, 1.8),
                secondary: (StatKind::Hack, 1.8),
            },
            equipment: Equipment::default(),
            common_skills: CommonSkills::default(),
            equipment_base_sources: Vec::new(),
            equipment_enhanced_sources: Vec::new(),
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
                combo_variants: Vec::new(),
                power: Skill::compute_power(0.99, 1),
                power_per_second: Skill::compute_power_per_second(
                    Skill::compute_power(0.99, 1),
                    Some(1.4),
                ),
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

    /// テスト用: 装備基本能力値を単一の供給源として設定する(内訳の中身は見ないテストが使う)。
    fn set_equipment_base(i: &mut DamageInput, values: EquipmentValues) {
        i.equipment_base_sources = vec![EquipmentValueSource {
            source: "テスト".into(),
            values,
        }];
    }

    /// テスト用: 装備強化能力値を単一の供給源として設定する。
    fn set_equipment_enhanced(i: &mut DamageInput, values: EquipmentValues) {
        i.equipment_enhanced_sources = vec![EquipmentValueSource {
            source: "テスト".into(),
            values,
        }];
    }

    // 基準値(手計算):
    //   ステ攻撃力 = 500×1.8 + 500×1.8 = 1800 → A = 1800
    //   B最大 = {(1800 + 100×3)/18} + 1 = {116.666..} + 1 = 117.66
    //   min : (1800 + 1 − 990) × {0.99} = 811 × 0.99 = 802.89 → 802
    //   max : (1800 + 117.66 − 990) × 0.99 = 927.66 × 0.99 = 918.3834 → 918
    //   crit: 918.3834 × {2.0 × 1.0} = 1836.7668 → 1836
    #[test]
    fn トレースのステ攻撃力内訳は依存ステだけで合計がステ攻撃力になる() {
        let result = calculate_damage(&input());
        let parts = &result.trace.stat_attack_parts;
        // 依存 StabHack の 2 ステだけ(全 7 ステは並べない)
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].kind, StatKind::Stab);
        assert_eq!(parts[1].kind, StatKind::Hack);
        let stat_attack = result
            .trace
            .steps_max
            .iter()
            .find(|s| s.name == "ステ攻撃力")
            .expect("ステ攻撃力の段")
            .value;
        let sum: f64 = parts.iter().map(|p| p.contribution).sum();
        assert!((sum - stat_attack).abs() < 1e-9);
    }

    #[test]
    fn 式の各段は消費したカテゴリを申告する() {
        // UI は「この段の材料」を step.categories から引く。式に現れるカテゴリが
        // どの段にも申告されていないと、その材料が画面から消える。
        let r = calculate_damage(&input());
        let declared: Vec<DamageCategory> = r
            .trace
            .steps_max
            .iter()
            .flat_map(|s| s.categories.iter().copied())
            .collect();
        for category in DamageCategory::ALL {
            if DamageCategory::ATTACK_DAMAGE_CHILDREN.contains(&category) {
                continue; // 攻撃ダメージ(X)に合算されるので、親の X として申告される
            }
            assert_eq!(
                declared.iter().filter(|d| **d == category).count(),
                1,
                "{}({}) を申告している段が 1 つでない",
                category.label(),
                category.wiki_symbol()
            );
        }
    }

    #[test]
    fn 命中pはdexと装備命中とスキル命中から依存ペナルティを引く() {
        let mut i = input();
        set_equipment_base(
            &mut i,
            EquipmentValues {
                accuracy: 30,
                ..Default::default()
            },
        );
        set_equipment_enhanced(
            &mut i,
            EquipmentValues {
                accuracy: 20,
                ..Default::default()
            },
        );
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
        assert!(capped
            .trace
            .steps_max
            .iter()
            .any(|s| s.name == "ダメージ上限"));
    }

    // ダメージ上限は与ダメージ(スキル分)にだけ効く。武器強化の追加固定ダメージは
    // 上限を超えて合計に残る(docs/damage-formula.md §3/§5)。
    #[test]
    fn ダメージ上限は与ダメージにだけ効き武器強化の追加固定は上限を超えて合計に残る() {
        let mut i = input();
        i.skill.hit_count = 3;
        i.weapon_added_damage = 900; // INT(900/3) = 300
        let uncapped = calculate_damage(&i);
        assert_eq!(uncapped.weapon_added_per_hit, 300);
        assert!(uncapped.capped_loss.max == 0);

        // 上限をスキル分の per_hit ぎりぎりに設定する。武器強化を含めた値より小さい。
        i.damage_cap = uncapped.per_hit.max - 100;
        let capped = calculate_damage(&i);
        assert_eq!(capped.per_hit.max, i.damage_cap);
        assert_eq!(capped.capped_loss.max, 100);
        // 武器強化の追加固定は上限の影響を受けず 300 のまま
        assert_eq!(capped.weapon_added_per_hit, 300);
        // 合計 = (上限 + 武器強化300) × 3段
        assert_eq!(capped.total.max, (i.damage_cap + 300) * 3);
    }

    #[test]
    fn 攻撃力_乱数_防御力_スキル倍率_cri倍率() {
        let r = calculate_damage(&input());
        assert_eq!(
            r.per_hit,
            DamageTriple {
                min: 802,
                max: 918,
                critical: 1836
            }
        );
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
        assert_eq!(
            r.per_hit,
            DamageTriple {
                min: 802,
                max: 918,
                critical: 1836
            }
        );
        assert_eq!(
            r.total,
            DamageTriple {
                min: 802 * 11,
                max: 918 * 11,
                critical: 1836 * 11
            }
        );
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
        assert_eq!(
            r.per_hit,
            DamageTriple {
                min: 923,
                max: 1056,
                critical: 2112
            }
        );
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

    // temporary_pins で STAB を 2000 に固定すると、ステ由来攻撃力の計算に反映されて結果が変わる。
    // trace.stats の STAB 行には pinned_from に元の 500 が残る。
    #[test]
    fn 一時調整のpinで能力値を固定すると攻撃力計算に反映されpinned_fromが記録される() {
        let mut i = input();
        i.temporary_pins = Some(Adjustments {
            stab: StatAdjustment {
                add: 0,
                pin: Some(2000),
            },
            ..Default::default()
        });
        let r = calculate_damage(&i);
        assert_ne!(r.per_hit.min, 802);
        let stab_trace = r
            .trace
            .stats
            .iter()
            .find(|t| t.kind == StatKind::Stab)
            .unwrap();
        assert_eq!(stab_trace.pinned_from, Some(500));
        assert_eq!(stab_trace.effective, 2000);
    }

    #[test]
    fn 防御力が攻撃力を上回ると対モンスター下限の1() {
        let mut i = input();
        i.enemy.defense = 5000;
        let r = calculate_damage(&i);
        assert_eq!(
            r.per_hit,
            DamageTriple {
                min: 1,
                max: 1,
                critical: 1
            }
        );
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
    fn ソウルリンクgはクリティカルだけに効きlは45パーセントで止まる() {
        let base = calculate_damage(&input());
        let mut i = input();
        i.damage_contributions = vec![
            DamageContribution {
                source: "ソウルリンク".into(),
                category: DamageCategory::CriticalDamageRate,
                value: 0.30,
            },
            DamageContribution {
                source: "ソウルリンク".into(),
                category: DamageCategory::FinalDamageRate,
                value: 0.20,
            },
            DamageContribution {
                source: "既存L".into(),
                category: DamageCategory::FinalDamageRate,
                value: 0.40,
            },
        ];
        let result = calculate_damage(&i);
        // G は非クリティカルへ入らない。L は非クリ・クリの両方へ同じように効く。
        assert_eq!(result.per_hit.max, (base.per_hit.max as f64 * 1.45) as i64);
        assert!(result.per_hit.critical > (base.per_hit.critical as f64 * 1.45) as i64);
        let l = result.trace.categories.iter()
            .find(|row| row.category == DamageCategory::FinalDamageRate).unwrap();
        assert!((l.raw - 0.60).abs() < 1e-9);
        assert_eq!(l.value, 0.45);
        assert_eq!(l.factor, 1.45);
        assert!(result.trace.category_contributions.iter().any(|row| {
            row.source == "ソウルリンク" && row.category == DamageCategory::CriticalDamageRate
        }));
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
            let target = if category == AttackDamageRate {
                AttackDamageSkill
            } else {
                category
            };
            t.add(target, delta);
            let (n, _) = evaluate(&t, false);
            let (c, steps) = evaluate(&t, true);
            assert_ne!(
                c,
                critical,
                "{}({}) がクリティカル時の結果に影響していない",
                category.label(),
                category.wiki_symbol()
            );
            let only_critical = matches!(category, CriticalMultiplier | CriticalDamageRate);
            if !only_critical {
                assert_ne!(
                    n,
                    normal,
                    "{}({}) が結果に影響していない",
                    category.label(),
                    category.wiki_symbol()
                );
            }
            // X3/X4/X5 は親 X としてまとめて式に出る
            let symbol = if DamageCategory::ATTACK_DAMAGE_CHILDREN.contains(&category) {
                AttackDamageRate.wiki_symbol()
            } else {
                category.wiki_symbol()
            };
            assert!(
                steps
                    .iter()
                    .any(|s| s.expression.contains(&format!("{symbol} "))
                        || s.expression.contains(&format!("{symbol}) "))),
                "{symbol} がトレース式に現れない"
            );
        }
    }

    #[test]
    fn トレースに全カテゴリが出る() {
        let r = calculate_damage(&input());
        let symbols: Vec<&str> = r
            .trace
            .categories
            .iter()
            .map(|c| c.symbol.as_str())
            .collect();
        for c in DamageCategory::ALL {
            assert!(symbols.contains(&c.wiki_symbol()));
        }
        // 攻撃力(A)の内訳 4 段(ステ攻撃力/装備攻撃力/装備攻撃力強化倍率/攻撃力(A)) + 従来の 10 段。
        assert_eq!(r.trace.steps_min.len(), 14);
        assert_eq!(r.trace.steps_min.len(), r.trace.steps_critical.len());
        let names: Vec<&str> = r.trace.steps_min.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            &names[..4],
            [
                "ステ攻撃力",
                "装備攻撃力",
                "装備攻撃力強化倍率",
                "攻撃力(A)"
            ]
        );
    }

    // 受け入れ条件1: 装備補正を入れたキャラは中盤以降の敵(兄弟の鍛冶場相当)でも下限1にならない。
    // ボリス素ステ例(STAB 310 HACK 310)+ 装備(base 突400 斬400、enhanced 突200 斬200、SW Lv6。
    // 基本 400 はネオテシス武器(wiki 装備強化: 蒼穹 410〜888)相当で現実的な値)。
    #[test]
    fn 装備補正があると中盤の敵に対しても下限1にならない() {
        let mut i = input();
        i.base_stats = BaseStats {
            stab: 310,
            hack: 310,
            int: 1,
            def: 1,
            mr: 1,
            dex: 100,
            agi: 1,
        };
        i.common_skills = CommonSkills {
            strong_weapon_level: 6,
            ..Default::default()
        };
        set_equipment_base(
            &mut i,
            EquipmentValues {
                thrust: 400,
                slash: 400,
                ..Default::default()
            },
        );
        set_equipment_enhanced(
            &mut i,
            EquipmentValues {
                thrust: 200,
                slash: 200,
                ..Default::default()
            },
        );
        i.equipment_coefficients = EquipmentCoefficients {
            base: crate::equipment::EquipmentRates {
                thrust: 14.5,
                slash: 14.5,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
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
        assert!(
            r.per_hit.min > 1,
            "装備ありなら下限1を超えるはず: {:?}",
            r.per_hit
        );

        // 回帰確認: 装備 default なら従来どおり下限1のまま。
        let mut without_equipment = i.clone();
        without_equipment.equipment = Equipment::default();
        without_equipment.equipment_base_sources = Vec::new();
        without_equipment.equipment_enhanced_sources = Vec::new();
        let r2 = calculate_damage(&without_equipment);
        assert_eq!(
            r2.per_hit,
            DamageTriple {
                min: 1,
                max: 1,
                critical: 1
            }
        );
    }

    // §5 武器強化の追加固定ダメージ(与ダメージ式の外)。goal 文書の例: 追加 2488・9hit → per-hit 276。
    // per_hit(表記ダメージ)には足し込まず、`weapon_added_per_hit` に別で持つ。
    #[test]
    fn 武器強化の追加固定ダメージはhit数で分割してweapon_added_per_hitとtotalに加算される() {
        let mut i = input();
        i.skill.hit_count = 9;
        i.weapon_added_damage = 2488;
        let base = calculate_damage(&input());
        let r = calculate_damage(&i);
        // INT(2488 / 9) = 276
        assert_eq!(r.per_hit, base.per_hit);
        assert_eq!(r.weapon_added_per_hit, 276);
        assert_eq!(r.total.min, (r.per_hit.min + 276) * 9);
        assert_eq!(r.total.max, (r.per_hit.max + 276) * 9);
        assert_eq!(r.total.critical, (r.per_hit.critical + 276) * 9);
        assert_eq!(
            r.trace.steps_min.last().unwrap().name,
            "武器強化(追加固定ダメージ)"
        );
    }

    #[test]
    fn ソウルリンク武器強化倍率の後にhit分割する() {
        let mut i = input();
        i.skill.hit_count = 9;
        i.weapon_added_damage = crate::SoulLinkStatus {
            weapon_enhance_level: 10,
            ..Default::default()
        }
        .weapon_added_damage(2488);
        let base = calculate_damage(&input());
        let result = calculate_damage(&i);
        // Lv10で 2倍した 4,976 を9段へ分割し、1段あたり552。per_hit は変わらない。
        assert_eq!(result.per_hit.max, base.per_hit.max);
        assert_eq!(result.weapon_added_per_hit, 552);
        assert_eq!(result.total.max, (result.per_hit.max + 552) * 9);
    }

    #[test]
    fn weapon_added_damageが0ならトレース段は増えず挙動は現行と変わらない() {
        let r = calculate_damage(&input());
        assert_eq!(r.trace.steps_min.len(), 14);
        assert_ne!(
            r.trace.steps_min.last().unwrap().name,
            "武器強化(追加固定ダメージ)"
        );
    }

    /// マスタリーの「攻撃ダメージ +n%」も同じカテゴリX(称号・ランダムOP と合算)。
    #[test]
    fn マスタリーの攻撃ダメージ増加はXに乗る() {
        let mut i = input();
        i.damage_contributions = vec![DamageContribution {
            source: "テスト".into(),
            category: DamageCategory::AttackDamageSkill,
            value: 0.05,
        }];
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
        assert_eq!(
            result.total.max,
            result.per_hit.max * 3 + result.added_damage.max
        );
        let step = result
            .trace
            .steps_max
            .iter()
            .find(|s| s.name == "割合追加ダメージ(合計に乗る)")
            .unwrap();
        assert!(step.expression.contains("称号 20%"));
    }

    #[test]
    fn 命中時ランダムOPは依存種別に合うカテゴリTだけ乗り追加ダメージには入らない() {
        let mut physical = input();
        physical.random_options.added_damage_rate = 0.10;
        physical.random_options.physical_added_damage_rate = 0.14;
        physical.random_options.magic_added_damage_rate = 0.15;
        physical.random_options.physical_damage_amplify = 0.10;
        physical.random_options.magic_damage_amplify = 0.20;
        let physical_result = calculate_damage(&physical);
        assert!((physical_result.added_damage_rate - 0.24).abs() < 1e-12);
        let physical_t = physical_result
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "T")
            .unwrap();
        assert!((physical_t.value - 0.10).abs() < 1e-12);
        assert!((physical_t.factor - 1.10).abs() < 1e-12);

        let mut magic = physical;
        magic.skill.dependency = SkillDependency::HackInt;
        let magic_result = calculate_damage(&magic);
        assert!((magic_result.added_damage_rate - 0.25).abs() < 1e-12);
        let magic_t = magic_result
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "T")
            .unwrap();
        assert!((magic_t.value - 0.20).abs() < 1e-12);
    }

    #[test]
    fn シエナのオーラの攻撃力増加はNew1に乗る() {
        let base = calculate_damage(&input()).per_hit.max;

        let mut i = input();
        set_siena(
            &mut i.equipment,
            PartSlot::Weapon,
            siena_extra(SienaValueKind::Thrust, SienaExtraKind::AttackRate, 10.0),
        );
        set_siena(
            &mut i.equipment,
            PartSlot::Armor,
            siena_extra(SienaValueKind::Stab, SienaExtraKind::AttackRate, 5.0),
        );
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
            slots: [Some(ThesisCore {
                core_type: CoreType::Slash,
                evolution: 0,
                enhancement: 4,
            }); CORE_SLOT_COUNT],
        };
        let fixed = calculate_damage(&i);
        let k = fixed
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "K")
            .unwrap();
        assert_eq!(k.value, 800.0);

        // 進化4 強化4 が 6 個 → 最終ダメージ +5%(K は 0 に戻る)
        let mut i = input();
        *i.equipment.thesis_cores.get_mut(CoreRegion::Eclipse) = CoreSet {
            slots: [Some(ThesisCore {
                core_type: CoreType::Slash,
                evolution: 4,
                enhancement: 4,
            }); CORE_SLOT_COUNT],
        };
        let rate = calculate_damage(&i);
        let l = rate
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "L")
            .unwrap();
        assert!((l.value - 0.05).abs() < 1e-12);
        assert_eq!(
            rate.trace
                .categories
                .iter()
                .find(|c| c.symbol == "K")
                .unwrap()
                .value,
            0.0
        );
        assert!(rate.per_hit.max > calculate_damage(&input()).per_hit.max);
    }

    // wiki: K は上限 1000。進化1 強化4 の 6 セット(+1,400)はキャップに当たる
    #[test]
    fn テシスコアの最終ダメージ固定値は上限1000でキャップされる() {
        use crate::thesis_core::{CoreRegion, CoreSet, CoreType, ThesisCore, CORE_SLOT_COUNT};

        let mut i = input();
        *i.equipment.thesis_cores.get_mut(CoreRegion::Mercurial) = CoreSet {
            slots: [Some(ThesisCore {
                core_type: CoreType::Thrust,
                evolution: 1,
                enhancement: 4,
            }); CORE_SLOT_COUNT],
        };
        let result = calculate_damage(&i);
        let k = result
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "K")
            .unwrap();
        assert_eq!(k.raw, 1_400.0);
        assert_eq!(k.value, 1_000.0);
    }

    // --- ランダムオプション ---------------------------------------------

    #[test]
    fn ランダムオプションの依存別攻撃力増加は依存が一致したときだけ乗る() {
        use crate::random_option::DependencyRates;

        // スキルは STAB+HACK 依存。一致する枠だけがカテゴリP に入る
        let mut i = input();
        i.random_options.dependency_damage_rate = DependencyRates {
            stab_hack: 0.10,
            stab: 0.25,
            ..Default::default()
        };
        let result = calculate_damage(&i);
        let p = result
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "P")
            .unwrap();
        assert!((p.value - 0.10).abs() < 1e-12);
        assert!(result.per_hit.max > calculate_damage(&input()).per_hit.max);
    }

    // wiki §4: カテゴリP は上限 +73%
    #[test]
    fn ランダムオプションの依存別攻撃力増加は上限73パーセントで頭打ち() {
        use crate::random_option::DependencyRates;

        let mut i = input();
        i.random_options.dependency_damage_rate = DependencyRates {
            stab_hack: 1.00,
            ..Default::default()
        };
        let result = calculate_damage(&i);
        let p = result
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "P")
            .unwrap();
        assert!((p.raw - 1.00).abs() < 1e-12);
        assert!((p.value - 0.73).abs() < 1e-12);
    }

    #[test]
    fn ランダムオプションの攻撃ダメージ増加はカテゴリXに入る() {
        let mut i = input();
        i.random_options.attack_damage_rate = 0.30;
        let result = calculate_damage(&i);
        let x = result
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "X")
            .unwrap();
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
        set_equipment_base(
            &mut i,
            EquipmentValues {
                critical: 200,
                ..Default::default()
            },
        );
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
        set_equipment_base(
            &mut i,
            EquipmentValues {
                critical: 200,
                ..Default::default()
            },
        );
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

    // クリ率が wiki 未記載(critical_rate が None)ならクリティカル確定扱い(ユーザー判断 2026-08-29)。
    #[test]
    fn クリ率が未記載ならクリティカル確定扱いで期待値はクリdpsと一致する() {
        let result = calculate_damage(&input());
        assert!(result.critical_rate.is_none());
        assert_eq!(result.critical_chance, 1.0);
        assert_eq!(result.expected_dps, result.dps.map(|d| d.critical));
    }

    // クリ率 40% なら期待値は非クリdpsとクリdpsの線形補間になる。
    #[test]
    fn クリ率がある場合の期待dpsは線形補間になる() {
        let mut i = input();
        i.base_stats.agi = 300;
        set_equipment_base(
            &mut i,
            EquipmentValues {
                critical: 200,
                ..Default::default()
            },
        );
        i.enemy.agi = Some(1420);
        i.enemy.critical_taken_rate = Some(0.0);
        i.skill.critical_rate = Some(13);
        let result = calculate_damage(&i);
        let p = result.critical_rate.unwrap().value / 100.0;
        // 意図的にクリ率が 0% でも 100% でもないケースになっている前提
        assert!(p > 0.0 && p < 1.0);
        assert!((result.critical_chance - p).abs() < 1e-12);
        let dps = result.dps.unwrap();
        let expected = dps.max * (1.0 - p) + dps.critical * p;
        assert!((result.expected_dps.unwrap() - expected).abs() < 1e-9);
    }

    // wiki Quest/覚醒クエスト「各能力の上限値」/ エタの意志: 最終能力値の上限
    #[test]
    fn 最終能力値の上限は攻撃力に効きトレースに捨てた分が出る() {
        let mut i = input();
        let uncapped = calculate_damage(&i);
        assert!(uncapped.trace.stats.iter().all(|t| t.capped_loss == 0));

        i.stat_cap = 400;
        let capped = calculate_damage(&i);
        let stab = capped
            .trace
            .stats
            .iter()
            .find(|t| t.kind == StatKind::Stab)
            .unwrap();
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
        set_siena(
            &mut i.equipment,
            PartSlot::Shield,
            siena_extra(SienaValueKind::Thrust, SienaExtraKind::ActualDelay, 2.0),
        );
        i.actual_delay_skills = vec![ActualDelayContribution {
            source: "剣の司祭".into(),
            rate: 0.05,
        }];

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
        let g = with_scope
            .trace
            .categories
            .iter()
            .find(|c| c.symbol == "G")
            .unwrap();
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

    /// カテゴリ供給源内訳(トレースの掘り下げ用)は、非 0 の供給源だけを積み、
    /// 同じカテゴリの合計はカテゴリ集計値(キャップ適用前の生値)と一致する。
    #[test]
    fn カテゴリ供給源内訳の合計はカテゴリの生値と一致する() {
        let mut i = input();
        i.enemy.damage_reduction = -100;
        i.damage_contributions = vec![DamageContribution {
            source: "テストスキル".into(),
            category: DamageCategory::AttackDamageSkill,
            value: 0.05,
        }];
        let r = calculate_damage(&i);

        let m: f64 = r
            .trace
            .category_contributions
            .iter()
            .filter(|c| c.category == DamageCategory::DamageReduction)
            .map(|c| c.value)
            .sum();
        assert_eq!(m, -100.0);
        assert!(r
            .trace
            .category_contributions
            .iter()
            .any(|c| c.source == "テストスキル" && c.category == DamageCategory::AttackDamageSkill));
        // 0 の供給源(コンボボーナス。既定 combo_count = 0)は積まれない
        assert!(!r
            .trace
            .category_contributions
            .iter()
            .any(|c| c.category == DamageCategory::ComboBonus));
    }

    /// 装備攻撃力の内訳(層 × 装備値種別)の合計は装備攻撃力(基本+強化)と一致する。
    #[test]
    fn 装備攻撃力の内訳の合計は装備攻撃力と一致する() {
        let mut i = input();
        set_equipment_base(
            &mut i,
            EquipmentValues {
                thrust: 400,
                slash: 400,
                ..Default::default()
            },
        );
        set_equipment_enhanced(
            &mut i,
            EquipmentValues {
                thrust: 200,
                slash: 200,
                ..Default::default()
            },
        );
        i.equipment_coefficients = EquipmentCoefficients {
            base: crate::equipment::EquipmentRates {
                thrust: 14.5,
                slash: 14.5,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
            enhanced: crate::equipment::EquipmentRates {
                thrust: 28.75,
                slash: 28.75,
                magic_attack: 0.0,
                magic_defense: 0.0,
            },
        };
        let r = calculate_damage(&i);
        let sum: f64 = r
            .trace
            .equipment_attack_parts
            .iter()
            .map(|p| p.contribution)
            .sum();
        let equipment_attack = r
            .trace
            .steps_max
            .iter()
            .find(|s| s.name == "装備攻撃力")
            .unwrap()
            .value;
        assert!((sum - equipment_attack).abs() < 1e-9);
        assert_eq!(r.trace.equipment_attack_parts.len(), 4);

        // 各行の供給源(sources)の Σ が行(part)と一致する(単一供給源なので amount/contribution が丸ごと乗る)。
        for part in &r.trace.equipment_attack_parts {
            let source_amount: i64 = part.sources.iter().map(|s| s.amount).sum();
            let source_contribution: f64 = part.sources.iter().map(|s| s.contribution).sum();
            assert_eq!(source_amount, part.amount);
            assert!((source_contribution - part.contribution).abs() < 1e-9);
            assert_eq!(part.sources.len(), 1);
            assert_eq!(part.sources[0].source, "テスト");
        }
    }

    /// 装備攻撃力強化倍率の供給源(パワーウェポン/ストロングウェポン)の合計は強化倍率と一致する。
    #[test]
    fn 装備攻撃力強化倍率の供給源の合計は強化倍率と一致する() {
        let mut i = input();
        i.common_skills = CommonSkills {
            power_weapon: true,
            strong_weapon_level: 3,
            ..Default::default()
        };
        let r = calculate_damage(&i);
        let sum: f64 = r
            .trace
            .equipment_enhance_sources
            .iter()
            .map(|s| s.value)
            .sum();
        let enhance_rate = r
            .trace
            .steps_max
            .iter()
            .find(|s| s.name == "装備攻撃力強化倍率")
            .unwrap()
            .value;
        assert!((sum - enhance_rate).abs() < 1e-12);
        assert_eq!(r.trace.equipment_enhance_sources.len(), 2);
    }
}
