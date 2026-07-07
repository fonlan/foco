import type {
  ConfiguredModelSummary,
  ModelMetadataRecord,
  ThinkingLevelSummary,
} from "../api/types";

type ThinkingLevelModel = Pick<
  ConfiguredModelSummary,
  "supportsThinking" | "supportedThinkingLevels" | "thinkingLevel"
> | Pick<ModelMetadataRecord, "reasoning" | "supportedThinkingLevels"> | null | undefined;

function modelSupportsThinking(model: ThinkingLevelModel): boolean {
  if (!model) {
    return false;
  }

  const supportsThinking =
    "supportsThinking" in model ? model.supportsThinking : model.reasoning;
  return Boolean(supportsThinking && model.supportedThinkingLevels.length > 0);
}

export function isModelThinkingLevelSupported(
  model: ThinkingLevelModel,
  thinkingLevel: string | null | undefined,
): thinkingLevel is string {
  return Boolean(
    thinkingLevel &&
      modelSupportsThinking(model) &&
      model?.supportedThinkingLevels.includes(thinkingLevel),
  );
}

export function normalizeThinkingLevelForModel(
  model: ThinkingLevelModel,
  thinkingLevel: string | null | undefined,
): string {
  return isModelThinkingLevelSupported(model, thinkingLevel) ? thinkingLevel : "";
}

export function defaultThinkingLevelForModel(model: ThinkingLevelModel): string {
  return normalizeThinkingLevelForModel(
    model,
    model && "thinkingLevel" in model ? model.thinkingLevel : null,
  );
}

export function thinkingLevelOptionsForModel(
  model: ThinkingLevelModel,
  thinkingLevels: ThinkingLevelSummary[],
): ThinkingLevelSummary[] {
  if (!modelSupportsThinking(model)) {
    return [];
  }

  const supported = new Set(model?.supportedThinkingLevels ?? []);
  return thinkingLevels.filter((level) => supported.has(level.value));
}
