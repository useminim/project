import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
  it("renders an empty main landmark", () => {
    expect(renderToString(<App />)).toBe("<main></main>");
  });
});
