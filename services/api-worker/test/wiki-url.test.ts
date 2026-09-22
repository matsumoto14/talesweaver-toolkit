import { describe, expect, it } from "vitest";

import { wikiUrl } from "../src/wiki-url";

describe("wikiUrl", () => {
  it("page.url とアンカーを連結する", () => {
    expect(wikiUrl("https://talewiki.com/?%A5%A8%A5%BF", "h2_1")).toBe(
      "https://talewiki.com/?%A5%A8%A5%BF#h2_1",
    );
  });

  it("見出しより前(anchor top)は連結しない", () => {
    expect(wikiUrl("https://talewiki.com/?%A5%A8%A5%BF", "top")).toBe(
      "https://talewiki.com/?%A5%A8%A5%BF",
    );
  });

  it("アンカーが空文字でも連結しない", () => {
    expect(wikiUrl("https://talewiki.com/?%A5%A8%A5%BF", "")).toBe(
      "https://talewiki.com/?%A5%A8%A5%BF",
    );
  });
});
