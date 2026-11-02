import { describe, expect, it } from "vitest";

import { loadSettings, saveSettings, validateSettings, withSettings } from "../src/settings";

function fakeKv(initial: Record<string, string> = {}): KVNamespace {
  const store = new Map(Object.entries(initial));
  return {
    get: async (k: string) => store.get(k) ?? null,
    put: async (k: string, v: string) => { store.set(k, v); },
    delete: async (k: string) => { store.delete(k); },
  } as unknown as KVNamespace;
}

const ENV = { SELECT_MODEL: "claude-haiku-5-5", UNDERSTAND_MODEL: "claude-haiku-5-5", SELECT_EFFORT: "" };

describe("settings", () => {
  it("未保存なら wrangler.toml の既定値", async () => {
    const r = await loadSettings({ ...ENV, API: fakeKv() });
    expect(r.saved).toBe(false);
    expect(r.settings).toEqual({ understandModel: "claude-haiku-5-5", selectModel: "claude-haiku-5-5", selectEffort: "" });
  });

  it("保存値が env を上書きする", async () => {
    const kv = fakeKv();
    await saveSettings(kv, { understandModel: "claude-haiku-5-5", selectModel: "claude-sonnet-5", selectEffort: "low" });
    const env = await withSettings({ ...ENV, API: kv });
    expect(env.SELECT_MODEL).toBe("claude-sonnet-5");
    expect(env.SELECT_EFFORT).toBe("low");
  });

  it("選べない保存値は無視して既定値", async () => {
    const kv = fakeKv({ settings: JSON.stringify({ selectModel: "gpt-x" }) });
    const r = await loadSettings({ ...ENV, API: kv });
    expect(r.saved).toBe(false);
    expect(r.settings.selectModel).toBe("claude-haiku-5-5");
  });

  it("単価の無いモデルと Haiku 4.5 + effort は弾く", () => {
    expect(validateSettings({ understandModel: "x", selectModel: "claude-haiku-5-5", selectEffort: "" })).not.toBeNull();
    expect(validateSettings({ understandModel: "claude-haiku-5-5", selectModel: "claude-haiku-4-5", selectEffort: "low" })).not.toBeNull();
    expect(validateSettings({ understandModel: "claude-haiku-5-5", selectModel: "claude-opus-5-5", selectEffort: "high" })).toBeNull();
  });
});
