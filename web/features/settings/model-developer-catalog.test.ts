import type { ModelMetadataRecord } from "../../api/types";
import { describe, expect, it } from "vitest";

import {
  developerForModelMetadata,
  modelIdForDeveloper,
  modelsForDeveloper,
} from "./model-developer-catalog";

const moonshotModel = {
  key: "moonshotai/kimi-k2",
  modelId: "kimi-k2",
} as ModelMetadataRecord;

describe("model developer catalog", () => {
  it("includes moonshotai catalog entries for the Moonshot developer", () => {
    expect(modelsForDeveloper([moonshotModel], "moonshot")).toEqual([
      moonshotModel,
    ]);
  });

  it("removes the moonshotai catalog prefix from the selected model id", () => {
    expect(modelIdForDeveloper(moonshotModel, "moonshot")).toBe("kimi-k2");
  });

  it("maps the Moonshot catalog provider back to the UI developer", () => {
    expect(developerForModelMetadata("moonshotai")).toBe("moonshot");
  });
});
