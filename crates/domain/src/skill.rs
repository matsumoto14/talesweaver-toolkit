//! スキル。倍率(wiki: カテゴリD)・段数・Cri倍率(カテゴリF)を持つ。

use serde::{Deserialize, Serialize};

use crate::defense::AttackType;
use crate::element::Element;
use crate::equipment::EquipmentStatKind;
use crate::equipment_class::WeaponClass;

/// スキルの依存種別。ステ由来攻撃力の係数(`AttackCoefficients`)を選ぶキー。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillDependency {
    Stab,
    Hack,
    Int,
    Mr,
    StabHack,
    HackInt,
}

impl SkillDependency {
    pub const ALL: [SkillDependency; 6] = [
        SkillDependency::Stab,
        SkillDependency::Hack,
        SkillDependency::Int,
        SkillDependency::Mr,
        SkillDependency::StabHack,
        SkillDependency::HackInt,
    ];

    /// 物理攻撃か魔法攻撃か。回避P・命中時ランダムOP・サブアームの専用対など、
    /// 「物理 / 魔法のどちらか」で分かれる規則はすべてここを見る。
    pub fn attack_type(self) -> AttackType {
        match self {
            SkillDependency::Stab | SkillDependency::Hack | SkillDependency::StabHack => {
                AttackType::Physical
            }
            SkillDependency::Int | SkillDependency::Mr | SkillDependency::HackInt => {
                AttackType::Magic
            }
        }
    }

    /// このスキルでエンチャントして意味のある装備補正(画面のエンチャント案内の対象)。
    /// 装備攻撃力係数が非 0 のステすべてではなく、**実際に伸ばす主要なステ**を返す
    /// (突き依存に斬りを、魔攻依存に魔防を案内しない)。
    pub fn enchant_stats(self) -> &'static [EquipmentStatKind] {
        use EquipmentStatKind::*;
        match self {
            SkillDependency::Stab => &[Thrust],
            SkillDependency::Hack => &[Slash],
            SkillDependency::StabHack => &[Thrust, Slash],
            SkillDependency::Int => &[MagicAttack],
            SkillDependency::Mr => &[MagicDefense],
            SkillDependency::HackInt => &[Slash, MagicAttack],
        }
    }
}

/// スキルを実際に撃つ主体。攻撃力の係数(`AttackCoefficients`)・装備係数・命中P補正を
/// どちらの表から引くかを決める(wiki 計算式まとめ `STAB(熊)` 行、2026-09-18 取得)。
///
/// アナイスの魔法人形(ミカベア / ルシベア)は自分でスキルを撃ち、本体とは別の係数で
/// 攻撃力を持つ。破壊精霊(アンフェル / グレシス / イグニー)も本体とは別の攻撃主体として
/// 召喚に入るが、wiki 計算式まとめの依存表に「精霊」行が無い(あるのは `STAB(熊)` だけ)
/// ため、係数は本体と同じ `dependency` の行(INT)に委譲する(2026-09-18 確認)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Attacker {
    #[default]
    Player,
    MagicDoll,
    DestructionSpirit,
}

/// 対象指定(wiki スキル性能一覧の「対象指定」列)。
///
/// 単体は 1 体、範囲は位置指定・方向指定・自分中心・設置などをまとめたもの。
/// 計算には使わない — **どのスキルを主軸にするかを選ぶときの手がかり**として持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillTarget {
    Single,
    Area,
}

/// 「連」系スキルで選べるコンボスキルタイプ。
/// 3 コンボ以上のダメージボーナス(`DamageInput::combo_count`)とは別の仕組み。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComboSkillType {
    General,
    Instant,
    Chain,
}

/// コンボスキルタイプごとの基礎性能。対応スキルだけがこの一覧を持つ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComboSkillVariant {
    pub combo_type: ComboSkillType,
    pub multiplier: f64,
    pub hit_count: u32,
    pub base_actual_delay: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboSkillTypeError {
    pub skill_id: String,
    pub combo_type: ComboSkillType,
}

impl std::fmt::Display for ComboSkillTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "スキル '{}' はコンボタイプ '{:?}' に対応していません",
            self.skill_id, self.combo_type
        )
    }
}

impl std::error::Error for ComboSkillTypeError {}

/// 武器形態(wiki「Skill/イェフネン」スキル性能一覧、2026-09-21 取得)。
///
/// イェフネンは同じ 4 技(連 / 爆 / スレイ / クラッシュ)を形態ごとに別の性能で撃つ。
/// 形態はキャラの状態ではなく**技そのものの属性**なので、保存せず主軸スキルから逆引きする。
/// 形態を持たないキャラのスキルは `None`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillForm {
    Sword,
    Pike,
    Axe,
    Urumi,
    Chisel,
}

impl SkillForm {
    /// wiki スキル性能一覧に出てくる順。
    pub const ALL: [SkillForm; 5] = [
        SkillForm::Sword,
        SkillForm::Pike,
        SkillForm::Axe,
        SkillForm::Urumi,
        SkillForm::Chisel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SkillForm::Sword => "ソード",
            SkillForm::Pike => "パイク",
            SkillForm::Axe => "アックス",
            SkillForm::Urumi => "ウルミ",
            SkillForm::Chisel => "チゼル",
        }
    }
}

/// 速剣(パッシブ)を習得しているときの性能(wiki スキル性能一覧の「(速剣適用時)」行)。
/// ソードシェイプ系 4 技だけが持つ。倍率は素の ×0.9、段数は表の実値。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SwiftSword {
    pub multiplier: f64,
    pub hit_count: u32,
}

/// 最大までチャージしたときの性能(wiki スキル性能一覧の段数幅 `8〜17` の上側)。
/// チャージ時間は中ディレイ減少が効かず、1 回の所要時間に丸ごと乗る。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FullCharge {
    pub hit_count: u32,
    /// チャージ時間(秒)。マスタリー【アックス特化】で半減する
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub dependency: SkillDependency,
    /// スキル倍率(wiki: カテゴリD)
    pub multiplier: f64,
    /// 段数
    pub hit_count: u32,
    /// Cri倍率(wiki: カテゴリF)
    pub critical_multiplier: f64,
    /// スキルの属性(wiki: 各キャラのスキルページ「スキル性能一覧」の属性列)
    pub element: Element,
    /// このスキルの実用武器種。依存能力だけでは刀 / 太刀 / 大剣を区別できないスキルにだけ入る
    /// (空 = 依存能力の系統で絞る)
    #[serde(default)]
    pub weapon_classes: Vec<WeaponClass>,
    /// 単体 / 範囲(wiki: 同じ表の対象指定列)。`None` = wiki の行と突き合わせできなかった
    /// (未収録として `?` で出す。0 や「単体」で埋めない)
    #[serde(default)]
    pub target: Option<SkillTarget>,
    /// スキル命中(wiki 計算式まとめ `#AccuracyPoint`: 「当Wikiのスキル命中は実際の数値から
    /// 15 引いた値が記載されているため +15 する必要がある」。**+15 済みの実値**を持つ)。
    /// wiki の表が `-` の行は `None` = 未記載で、命中Pを出せない
    pub accuracy: Option<i64>,
    /// スキルクリティカル率(wiki スキル性能一覧の「Cri値」)。`None` = wiki 未記載
    pub critical_rate: Option<i64>,
    /// スキル Lv(wiki スキル性能一覧の SLv。倍率は Lv 別未対応なのでこの Lv の値を持つ)
    pub level: u8,
    /// 単体チャネリングスキルか(wiki スキル性能一覧の 区分に `続` を含み、対象指定が `単体`。
    /// 凡例は wiki「Skill#f8e303fb」)。極限スキル「フルスロットル」の段数増加はこれにだけ乗る
    #[serde(default)]
    pub single_target_channeling: bool,
    /// 基本中ディレイ(秒)。wiki スキル性能一覧の「動作」列。
    /// 秒数として読めない行(表記が `0` 等)は `None` = 中ディレイ・DPS を出せない
    #[serde(default)]
    pub base_actual_delay: Option<f64>,
    /// 中ディレイが固定で減少が効かない(wiki スキル性能一覧の「(固定)」表記)
    #[serde(default)]
    pub actual_delay_fixed: bool,
    /// 通常攻撃(wiki スキル性能一覧の `†` = 基本攻撃)。コンボで間に挟むのはこれ
    #[serde(default)]
    pub normal_attack: bool,
    /// コンボインターバル(秒)。通常攻撃だけが持つ(wiki 計算式まとめ `#g7881516`)。
    /// 通常攻撃の中ディレイが終わってから数え始め、終わるまで次の行動が撃てないので、
    /// **最速コンボでは「次に使うスキルの中ディレイの下限」**として効く。
    /// `None` = wiki の CI 値表に無い(下限を出せないので、その旨を表示する)
    #[serde(default)]
    pub combo_interval: Option<f64>,
    /// 対応するコンボスキルタイプ。空ならタイプ選択非対応。
    #[serde(default)]
    pub combo_variants: Vec<ComboSkillVariant>,
    /// 1 回ぶんの火力の目安(倍率 × 段数)。主軸スキル候補の並び順に使う(UI 側は再計算しない)
    pub power: f64,
    /// 継続火力の目安(倍率 × 段数 ÷ 基本中ディレイ)。基本中ディレイ不明なら比較不能
    pub power_per_second: Option<f64>,
    /// このスキルを実際に撃つ主体(wiki `STAB(熊)` 行、2026-09-18 取得)。既定は `Player`。
    /// gamedata の副表(`MAGIC_DOLL_SKILLS`)から `to_skill()` が付ける
    #[serde(default)]
    pub attacker: Attacker,
    /// 召喚獣の型(wiki「Skill/アナイス」の型分け、2026-09-18 追記)。本体の主軸スキルの
    /// うち「どの人形 / 精霊で戦うか」を決めるスキル(ベアステップ・陣)はその型を、召喚スキル
    /// (`attacker != Player`)は自分が属する型を持つ。型を決めない主軸(共通スキル・守護精霊)や
    /// 他キャラのスキルは `None`。主軸を選んだら召喚欄をこの型の候補だけに絞り、既存の召喚選択が
    /// 型外なら型内の先頭(ダメージ最大)へ差し替える(フロントは対応表を持たず、この値だけ見る)
    #[serde(default)]
    pub summon_form: Option<SummonForm>,
    /// 武器形態(wiki「Skill/イェフネン」)。gamedata の副表(`SKILL_FORMS`)が付ける。
    /// 形態を持たないキャラのスキルは `None`
    #[serde(default)]
    pub form: Option<SkillForm>,
    /// 速剣(パッシブ)を習得しているときの性能。ソードシェイプ系 4 技だけが持つ
    #[serde(default)]
    pub swift_sword: Option<SwiftSword>,
    /// チャージで段数が増える技の、最大までチャージしたときの性能
    #[serde(default)]
    pub full_charge: Option<FullCharge>,
    /// 今回の計算でチャージに費やす時間(秒)。**解決後にだけ入る**値で、チャージしていない
    /// ときは 0。中ディレイ減少が効かないので、中ディレイの外で 1 回の所要時間に足す
    #[serde(default)]
    pub charge_seconds: f64,
    /// この技が敵に <フラグ> を積むか(イェフネンの 連 / 爆。全形態)。
    /// 積まれた <フラグ> は 1 秒ごとに持続ダメージを出す — 技とは**別枠**のダメージ
    #[serde(default)]
    pub applies_flag: bool,
    /// この技が積まれた <フラグ> を爆発させるか(イェフネンの スレイ / クラッシュ。全形態)。
    /// 爆発は技 1 回につき 1 度で、1 発の主役数字は技 + 爆発の合計になる
    #[serde(default)]
    pub detonates_flag: bool,
    /// クールタイム(秒)。wiki スキル性能一覧の CT 列。`None` = CT なし(連打できる)。
    /// 連続して撃てない技は、この秒数を 1 周の下限として DPS に効く
    #[serde(default)]
    pub cooldown_seconds: Option<f64>,
}

/// `Skill::summon_form` が指す召喚獣の型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SummonForm {
    MicaBear,
    RucyBear,
    Anferu,
    Gureshisu,
    Igni,
}

impl Skill {
    /// 1 回ぶんの火力の目安(倍率 × 段数)。0 段のスキルは無いので段数は最低 1 扱い。
    /// 主軸候補の順。対ボスで使う単体スキルを先にし、その中を中ディレイ込みの継続火力順
    /// (`power_per_second`)にする。依存種別では絞らない(斬り・物理複合・魔剣など、
    /// 別ビルドの入口を候補から消さないため)。中ディレイ不明のものは既知のものより後ろで、
    /// 1 回ぶんの火力(`power`)順。
    pub fn main_skill_order(a: &Skill, b: &Skill) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let single = |s: &Skill| s.target == Some(SkillTarget::Single);
        match (single(a), single(b)) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
        match (a.power_per_second, b.power_per_second) {
            (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(Ordering::Equal),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => b.power.partial_cmp(&a.power).unwrap_or(Ordering::Equal),
        }
    }

    pub fn compute_power(multiplier: f64, hit_count: u32) -> f64 {
        multiplier * f64::from(hit_count.max(1))
    }

    /// 倍率・段数・チャージ時間を変えたあとに火力の目安を付け直す。
    /// 継続火力は 1 回の所要時間(基本中ディレイ + チャージ時間)で割る。
    fn refresh_power(&mut self) {
        self.power = Self::compute_power(self.multiplier, self.hit_count);
        self.power_per_second = Self::compute_power_per_second(
            self.power,
            self.base_actual_delay.map(|d| d + self.charge_seconds),
        );
    }

    /// 速剣(パッシブ)を習得しているときの性能に差し替える。速剣の行を持たない技
    /// (ソードシェイプ系以外)はそのまま返すので、形態で分岐する if を呼び出し側に書かない。
    pub fn resolve_swift_sword(&self) -> Skill {
        let mut resolved = self.clone();
        let Some(variant) = self.swift_sword else {
            return resolved;
        };
        resolved.multiplier = variant.multiplier;
        resolved.hit_count = variant.hit_count;
        resolved.refresh_power();
        resolved
    }

    /// 最大までチャージしたときの性能に差し替える。`halved` はマスタリー【アックス特化】
    /// (チャージタイム半減)を取っているか。チャージできない技はそのまま返す。
    pub fn resolve_full_charge(&self, halved: bool) -> Skill {
        let mut resolved = self.clone();
        let Some(charge) = self.full_charge else {
            return resolved;
        };
        resolved.hit_count = charge.hit_count;
        resolved.charge_seconds = if halved {
            charge.seconds / 2.0
        } else {
            charge.seconds
        };
        resolved.refresh_power();
        resolved
    }

    /// テスト用の最小スキル(倍率・段数などは判定に関係しない既定値)。
    #[cfg(test)]
    pub(crate) fn for_test(id: &str, dependency: SkillDependency) -> Skill {
        Skill {
            id: id.to_string(),
            name: id.to_string(),
            dependency,
            multiplier: 1.0,
            hit_count: 1,
            critical_multiplier: 1.0,
            element: Element::Neutral,
            weapon_classes: Vec::new(),
            target: None,
            accuracy: None,
            critical_rate: None,
            level: 1,
            single_target_channeling: false,
            base_actual_delay: None,
            actual_delay_fixed: false,
            normal_attack: false,
            combo_interval: None,
            combo_variants: Vec::new(),
            power: 1.0,
            power_per_second: None,
            attacker: Attacker::Player,
            summon_form: None,
            form: None,
            swift_sword: None,
            full_charge: None,
            charge_seconds: 0.0,
            applies_flag: false,
            detonates_flag: false,
            cooldown_seconds: None,
        }
    }

    /// 継続火力の目安(倍率 × 段数 ÷ 基本中ディレイ)。基本中ディレイが未収録 / 0 以下なら比較不能
    pub fn compute_power_per_second(power: f64, base_actual_delay: Option<f64>) -> Option<f64> {
        base_actual_delay
            .filter(|&delay| delay > 0.0)
            .map(|delay| power / delay)
    }

    /// 選択したコンボスキルタイプとシエナのオーラから、今回の計算に使う性能を解決する。
    pub fn resolve_combo_variant(
        &self,
        combo_type: ComboSkillType,
        siena_actual_delay_reduction: f64,
    ) -> Result<Self, ComboSkillTypeError> {
        let variant = self
            .combo_variants
            .iter()
            .find(|variant| variant.combo_type == combo_type)
            .ok_or_else(|| ComboSkillTypeError {
                skill_id: self.id.clone(),
                combo_type,
            })?;
        let mut resolved = self.clone();
        resolved.multiplier = variant.multiplier;
        resolved.hit_count = variant.hit_count;
        resolved.base_actual_delay = Some(variant.base_actual_delay);
        if combo_type == ComboSkillType::Chain {
            // wiki 表は 2% 刻み。段数は 6% ごと、倍率は各 6% 区間で +0/+10/+20pt。
            let step = ((siena_actual_delay_reduction.max(0.0) * 100.0 + 1e-9) / 2.0)
                .floor()
                .min(8.0) as u32;
            resolved.hit_count += step / 3;
            resolved.multiplier += f64::from(step % 3) * 0.10;
        }
        resolved.refresh_power();
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combo_skill() -> Skill {
        Skill {
            id: "continuous".into(),
            name: "極・連".into(),
            dependency: SkillDependency::Hack,
            multiplier: 5.55,
            hit_count: 11,
            critical_multiplier: 2.5,
            element: Element::Neutral,
            weapon_classes: Vec::new(),
            target: Some(SkillTarget::Single),
            accuracy: Some(100),
            critical_rate: Some(5),
            level: 10,
            single_target_channeling: false,
            base_actual_delay: Some(1.4),
            actual_delay_fixed: false,
            normal_attack: false,
            combo_interval: None,
            combo_variants: vec![
                ComboSkillVariant {
                    combo_type: ComboSkillType::General,
                    multiplier: 5.55,
                    hit_count: 11,
                    base_actual_delay: 1.4,
                },
                ComboSkillVariant {
                    combo_type: ComboSkillType::Instant,
                    multiplier: 5.20,
                    hit_count: 10,
                    base_actual_delay: 1.0,
                },
                ComboSkillVariant {
                    combo_type: ComboSkillType::Chain,
                    multiplier: 5.20,
                    hit_count: 12,
                    base_actual_delay: 1.6,
                },
            ],
            power: Skill::compute_power(5.55, 11),
            power_per_second: Skill::compute_power_per_second(
                Skill::compute_power(5.55, 11),
                Some(1.4),
            ),
            attacker: Attacker::Player,
            summon_form: None,
            form: None,
            swift_sword: None,
            full_charge: None,
            charge_seconds: 0.0,
            applies_flag: false,
            detonates_flag: false,
            cooldown_seconds: None,
        }
    }

    #[test]
    fn 連撃はシエナ減少率を2パーセント刻みの下側閾値で解決する() {
        let skill = combo_skill();
        let cases = [
            (0.00, 5.20, 12),
            (0.02, 5.30, 12),
            (0.04, 5.40, 12),
            (0.06, 5.20, 13),
            (0.08, 5.30, 13),
            (0.10, 5.40, 13),
            (0.12, 5.20, 14),
            (0.14, 5.30, 14),
            (0.16, 5.40, 14),
        ];
        for (reduction, multiplier, hit_count) in cases {
            let resolved = skill
                .resolve_combo_variant(ComboSkillType::Chain, reduction)
                .unwrap();
            assert!(
                (resolved.multiplier - multiplier).abs() < 1e-12,
                "{reduction}"
            );
            assert_eq!(resolved.hit_count, hit_count, "{reduction}");
            assert_eq!(resolved.base_actual_delay, Some(1.6));
        }
        let below = skill
            .resolve_combo_variant(ComboSkillType::Chain, 0.039)
            .unwrap();
        assert!((below.multiplier - 5.30).abs() < 1e-12);
    }

    #[test]
    fn 未対応タイプは拒否する() {
        let mut skill = combo_skill();
        skill.combo_variants.clear();
        assert!(skill
            .resolve_combo_variant(ComboSkillType::General, 0.0)
            .is_err());
    }
}
