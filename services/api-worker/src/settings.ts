/**
 * 管理画面から変えられる設定(モデル・effort)。KV `API` の 1 キーに JSON で持ち、/ask と /react のたびに
 * wrangler.toml の既定値へ上書きする。未保存・壊れた値は無視して既定値のまま(KV の反映は最大 60 秒遅れる)。
 */
import { PRICING } from "./pricing";

const SETTINGS_KEY = "settings";

/** 選べるモデルは単価を持っているものだけ(管理画面の費用が「不明」にならないように)。 */
export const MODEL_CHOICES = Object.keys(PRICING);
export const EFFORT_CHOICES = ["", "low", "medium", "high", "xhigh", "max"] as const;

export interface Settings {
  understandModel: string;
  selectModel: string;
  /** 空なら送らない(モデルの既定)。 */
  selectEffort: string;
}

interface SettingsEnv {
  API?: KVNamespace;
  SELECT_MODEL?: string;
  UNDERSTAND_MODEL?: string;
  SELECT_EFFORT?: string;
}

function defaultsOf(env: SettingsEnv): Settings {
  return {
    understandModel: env.UNDERSTAND_MODEL ?? "",
    selectModel: env.SELECT_MODEL ?? "",
    selectEffort: env.SELECT_EFFORT ?? "",
  };
}

/** 選べない組み合わせならエラー文、良ければ null。Haiku 4.5 は effort を受け付けない。 */
export function validateSettings(s: Settings): string | null {
  if (!MODEL_CHOICES.includes(s.understandModel)) return `理解のモデルが選べないものです: ${s.understandModel}`;
  if (!MODEL_CHOICES.includes(s.selectModel)) return `選択のモデルが選べないものです: ${s.selectModel}`;
  if (!(EFFORT_CHOICES as readonly string[]).includes(s.selectEffort)) return `effort が選べないものです: ${s.selectEffort}`;
  if (s.selectEffort !== "" && s.selectModel === "claude-haiku-4-5") return "claude-haiku-4-5 は effort を受け付けません";
  return null;
}

/** 保存値(あれば)を既定値に重ねた、いま効いている設定。 */
export async function loadSettings(env: SettingsEnv): Promise<{ settings: Settings; defaults: Settings; saved: boolean }> {
  const defaults = defaultsOf(env);
  const raw = env.API ? await env.API.get(SETTINGS_KEY) : null;
  if (!raw) return { settings: defaults, defaults, saved: false };
  try {
    const parsed = JSON.parse(raw) as Partial<Settings>;
    const merged: Settings = { ...defaults, ...parsed };
    if (validateSettings(merged) !== null) return { settings: defaults, defaults, saved: false };
    return { settings: merged, defaults, saved: true };
  } catch {
    return { settings: defaults, defaults, saved: false };
  }
}

export async function saveSettings(kv: KVNamespace, s: Settings): Promise<void> {
  await kv.put(SETTINGS_KEY, JSON.stringify(s));
}

export async function resetSettings(kv: KVNamespace): Promise<void> {
  await kv.delete(SETTINGS_KEY);
}

/** env のモデル・effort を保存値で上書きした写し。 */
export async function withSettings<E extends SettingsEnv>(env: E): Promise<E> {
  const { settings } = await loadSettings(env);
  return {
    ...env,
    UNDERSTAND_MODEL: settings.understandModel,
    SELECT_MODEL: settings.selectModel,
    SELECT_EFFORT: settings.selectEffort,
  };
}
