import { describe, it } from "vitest";

describe.skip("App test suite split", () => {
  it("documents that App integration coverage lives in feature-grouped files", () => {
    // App-level integration coverage is split across app-*.test.tsx files
    // so Vitest can run the suite with file-level parallelism.
  });
});
