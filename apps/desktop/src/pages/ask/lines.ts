// ゼリッピの定型文(コード)。LLM が書くのは結論文(lead)の 1 文だけで、残りはすべてここの
// 定型文(design-system §00 / s6.txt「ゼリッピの役」)。性格・口調は AGENTS.md / s6.txt のとおり:
// 語尾は「ッピ」だけを借り、正直さ(候補に無いことは言わない)をそのまま文にする。一人称は使わない。
//
// 同じ状態に複数の定型文を持てる形にしてある(質問文のハッシュで選ぶ = 同じ質問には同じ文が返る)。
import blinkStill from "../../assets/ask/motion/blink-still.webp";
import blinkGif from "../../assets/ask/motion/blink.gif";
import idleStill from "../../assets/ask/motion/idle-still.webp";
import idleGif from "../../assets/ask/motion/idle.gif";
import joyStill from "../../assets/ask/motion/joy-still.webp";
import joyGif from "../../assets/ask/motion/joy.gif";
import sadStill from "../../assets/ask/motion/sad-still.webp";
import sadGif from "../../assets/ask/motion/sad.gif";
import thinkStill from "../../assets/ask/motion/think-still.webp";
import thinkGif from "../../assets/ask/motion/think.gif";
import { AskHttpError, type Aspect, type AskNoneReason, type ProgressStep, type WrongReason } from "../../ask";

/** ゼリッピの表情。応答の kind とエラーの種類から決める(s6.txt「ゼリッピの役」) */
export type Expression = "answered" | "thinking" | "notfound" | "offline";

/** 立ち絵の型。応答の表情に、会話がまだ 1 つも無いときの待機を足したもの */
export type Pose = Expression | "idle";

/** 型ごとの 8 コマのアニメ GIF。色は原画のまま・地(--bg-mid)に焼いてある
 *  (GIF の透過は 1bit しかなく、透過のままだとふちがギザギザになるため) */
export const POSE_GIF: Record<Pose, string> = {
  idle: idleGif, // 何も起きていない
  answered: joyGif, // 目を細めて笑う
  thinking: thinkGif, // 羽を口元に当てて考える
  notfound: sadGif, // 困る → 汗 → 泣く → しょんぼり
  offline: blinkGif, // 目を閉じて動かない
};

/** 動きを消す設定のときに出す 1 コマ目。GIF は再生を止められないので絵ごと差し替える */
export const POSE_STILL: Record<Pose, string> = {
  idle: idleStill,
  answered: joyStill,
  thinking: thinkStill,
  notfound: sadStill, // 3 コマ目(困り眉で口を閉じた顔)
  offline: blinkStill,
};

const THINKING: readonly string[] = ["wiki をめくってるッピ……"];

/** SSE の `progress` ステップごとの一言(stage2-spec.md desktop 9)。考え中の吹き出しの文を差し替える
 *  (`use:changed` で、文が変わったときだけ動く)。 */
const PROGRESS_LINE: Record<ProgressStep, string> = {
  understand: "質問を読んでるッピ……",
  search: "wiki をめくってるッピ……",
  select: "どれを見せるか選んでるッピ……",
  app_data: "アプリのデータも見るッピ……",
  outline: "目次を見てるッピ……",
  rows: "表を見てるッピ……",
};

export const progressLine = (step: ProgressStep): string => PROGRESS_LINE[step];

/** 検索は当たったが、聞かれたものそのものではない(近い候補を出す) */
const NOT_FOUND: readonly string[] = ["それは wiki に無かったッピ。近いのはここッピ。"];
/** 0 件 */
const EMPTY: readonly string[] = ["見つからないッピ……。"];
/** /health 失敗・/search・/ask のネットワーク失敗、502/503 */
const OFFLINE: readonly string[] = ["……wiki に届かないッピ。"];
/** 最初のあいさつ・雑談 */
const SMALLTALK: readonly string[] = ["wiki のことなら任せるッピ。書いてあることだけ探してくるッピ。"];
/** wiki の範囲外(ダメージ計算など) */
const OTHER: readonly string[] = ["それは wiki に書いてないッピ。"];
/** ダメージ・DPS の質問(wiki には無いが、アプリの計算タブが出せる) */
const DAMAGE_CALC: readonly string[] = ["ダメージや DPS は wiki に載ってないッピ。でもアプリが出せるッピ。"];
/** 1 日の上限(429) */
const RATE_LIMIT: readonly string[] = ["今日はもう聞きすぎッピ。明日また来るッピ。"];

/** 質問文のハッシュで定型文を選ぶ。1 文しかない状態は常にその 1 文になる */
function pick(lines: readonly string[], seed: string): string {
  let hash = 0;
  for (let i = 0; i < seed.length; i += 1) {
    hash = (hash * 31 + seed.charCodeAt(i)) >>> 0;
  }
  return lines[hash % lines.length] ?? lines[0] ?? "";
}

export const thinkingLine = (question: string): string => pick(THINKING, question);
export const notFoundLine = (question: string): string => pick(NOT_FOUND, question);
export const emptyLine = (question: string): string => pick(EMPTY, question);
export const offlineLine = (question: string): string => pick(OFFLINE, question);
export const smalltalkLine = (question: string): string => pick(SMALLTALK, question);
export const otherLine = (question: string): string => pick(OTHER, question);
export const damageCalcLine = (question: string): string => pick(DAMAGE_CALC, question);
export const rateLimitLine = (question: string): string => pick(RATE_LIMIT, question);

/** `kind:"none"` の reason ごとの文と表情(s6.txt / stage1-spec.md 16) */
export function noneLine(
  reason: AskNoneReason,
  question: string,
  hasSearch: boolean,
): { message: string; expression: Expression } {
  switch (reason) {
    case "smalltalk":
      return { message: smalltalkLine(question), expression: "answered" };
    case "other":
      return { message: otherLine(question), expression: "answered" };
    case "damage_calc":
      // 「無い」で終わらせない —— 計算タブが出せるので、下の打ち手(DamageCalcHandoff)へ続ける
      return { message: damageCalcLine(question), expression: "answered" };
    default:
      // llm_none / verification_failed / no_terms は段階 0 の「見つからない」+ 検索結果
      return { message: hasSearch ? notFoundLine(question) : emptyLine(question), expression: "notfound" };
  }
}

/** 手順は残ったが結論文が検証で落ちたとき(dropped: lead)。
 *  結論文が無い = 言い切れていないので、「書いてある」と言い切らない(2026-09-24) */
export const noLeadLine = (): string => "ぴったりの答えは無かったけど、近いのはここッピ。";

/** 候補に答えが無かった観点(missing)ごとの一言 */
const ASPECT_LABEL: Record<Aspect, string> = {
  what: "それが何か",
  how: "やり方",
  materials: "必要なもの",
  where: "手に入る場所",
  condition: "条件",
  numbers: "具体的な数値",
};

export const missingLine = (aspect: Aspect): string => `${ASPECT_LABEL[aspect]}は wiki に見当たらなかったッピ。`;

/** wiki と食い違うとき、コードが重ねる導入文(s6.txt「wiki と食い違うとき」)。確度で口調を変える */
export const CORRECTION_INTRO_CONFIRMED = "wiki にはこう書いてあるけど、公式お知らせではこうッピ。";
export const CORRECTION_INTRO_APPARENT = "wiki にはこう書いてあるけど、アプリのデータではこうみたいッピ。";

/** リアクションの返事(s6.txt「リアクション」) */
export const REACTION_HELPFUL_LINE = "任せるッピ!";
export const REACTION_WRONG_LINE = "……調べ直すッピ。";

export const WRONG_REASON_LABEL: Record<WrongReason, string> = {
  off_topic: "聞きたいことと違う",
  outdated: "情報が古い",
  unclear: "分かりにくい",
};

/** 「この値は違う」を押した直後の一言(段階 2〜3。stage2-spec.md desktop 14) */
export const VALUE_WRONG_ACK_LINE = "伝えたッピ。よければ詳しく:";

/** 続きバッジの文言(stage2-spec.md desktop 11) */
/** page はフルパス(`Skill/マキシミン`)。表示は末尾の名前だけ */
export const followupLine = (page: string): string => `${page.split("/").pop() || page} の続きとして答えたよ`;

/** 次の一手の見出し(stage2-spec.md desktop 13) */
export const NEXT_LABEL = "続けて聞く";

const BURST: readonly string[] = ["ちょっと待つッピ。続けて聞きすぎッピ。"];
export const burstLine = (question: string): string => pick(BURST, question);

// --- ダメージ・DPS の質問の打ち手(DamageCalcHandoff.svelte)。すべてコードの定型文 ---

export const DAMAGE_CALC_TITLE = "火力の高い組み合わせ";
export const DAMAGE_CALC_FILLER_LABEL = "連打する技(CT なし・継続火力順)";
export const DAMAGE_CALC_INSERT_LABEL = "差し込む技(CT あり)";
/** 登録キャラの実データで出したとき */
export const DAMAGE_CALC_NOTE = "差し込みの増減はあなたの装備・バフ・対象で計算した値。計算タブの「スキル回し」で組み替えられる";
/** 静的データ(スキルの性能)だけで出したとき */
export const DAMAGE_CALC_GENERIC_NOTE = "並びは技の性能(段数 ÷ 中ディレイ・1 回ぶんの火力)から。実際の DPS は装備・バフ・対象で変わる";
export const DAMAGE_CALC_REGISTER_NOTE = "キャラを登録すると、あなたの装備で差し込みの増減まで出るッピ";
export const DAMAGE_CALC_NO_SKILLS = "どのキャラの話か分からなかったッピ。計算タブでキャラと敵を選ぶと出せるッピ。";

// --- 困りごとの型「勝てない」の打ち手(Playbook.svelte)。すべてコードの定型文(s6.txt「困りごとの型」) ---

export const PLAYBOOK_INTRO = "勝てないときは、だいたいこの 2 つッピ。";
export const PLAYBOOK_TAG = "アプリの打ち手 · あなたのキャラで計算";
export const PLAYBOOK_NO_CHARACTER = "キャラを登録すると、あなたの装備で打ち手を出すッピ。";
export const PLAYBOOK_COST_ORDER_NOTE = "費用の安い順。あなたの登録データから";
export const PLAYBOOK_GO_TO_CALC = "計算タブで見る";

export const PLAYBOOK_CRIT_TITLE = "火力が出ない — クリティカルが出ていない";
export const PLAYBOOK_DEFENSE_TITLE = "痛すぎる — 受けるダメージが大きい";

/** クリティカル率の 4 供給源。費用の安い順(stage3-spec.md A-3) */
export const CRITICAL_SOURCE_LABELS = {
  pet: "ペット会心(クリティカル率 ×1.1)",
  deadly_blow: "致命打(+100)",
  ultimate_rune: "極のルーン(+20)",
  architect_lab_stage: "設計者の研究室(クリティカル率増加)",
} as const;

/** 防御側の「次にできること」は domain に無いので定型文 4 行(費用の安い順。stage3-spec.md A-3) */
export const PLAYBOOK_DEFENSE_MOVES: readonly string[] = [
  "防御・回避のバフを使う",
  "ステータスの固定上昇源(ペット・ルーン・カード)",
  "装備の防御補正を上げる",
  "エンチャントする(費用が高い)",
];

/** /ask 失敗時の表情と文。429(1 日の上限)だけ性格どおりの拗ねる言い回しにする(s6.txt「性格」) */
export function askErrorLine(e: unknown, question: string): { message: string; expression: Expression } {
  const expression: Expression = "offline";
  if (e instanceof AskHttpError && e.status === 429) {
    // 連打(1 分)と 1 日の上限は文が違う。サーバーの本文で見分ける(「1 日」を含むのが上限)
    return { message: e.message.includes("1 日") ? rateLimitLine(question) : burstLine(question), expression };
  }
  return { message: offlineLine(question), expression };
}
