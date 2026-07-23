import { describe, expect, it } from "vitest";

import { ANALYTICS_CHART_COLORS, chartColor } from "./constants";

describe("analytics chart palette", () => {
  it("keeps its primary series independent from the neutral interaction accent", () => {
    expect(ANALYTICS_CHART_COLORS[0]).toBe("var(--chart-primary)");
    expect(chartColor(ANALYTICS_CHART_COLORS.length)).toBe(
      "var(--chart-primary)",
    );
  });
});
