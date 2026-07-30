import type { ModelMetadataRecord } from "../../api/types";

const MODEL_DEVELOPER_CATALOG_ALIASES: Record<string, readonly string[]> = {
  moonshot: ["moonshotai"],
};

const CATALOG_DEVELOPER_TO_UI_DEVELOPER = new Map(
  Object.entries(MODEL_DEVELOPER_CATALOG_ALIASES).flatMap(
    ([developer, aliases]) => aliases.map((alias) => [alias, developer]),
  ),
);

export function developerForModelMetadata(providerId: string) {
  const normalizedProviderId = normalizeDeveloperToken(providerId);
  return (
    CATALOG_DEVELOPER_TO_UI_DEVELOPER.get(normalizedProviderId) ??
    normalizedProviderId
  );
}

export function modelsForDeveloper(
  models: ModelMetadataRecord[],
  developer: string,
) {
  const developerTokens = modelDeveloperCatalogTokens(developer);

  if (!developerTokens.length) {
    return [];
  }

  return models.filter((model) => {
    const modelKey = normalizeDeveloperToken(model.key);
    return developerTokens.some((token) => modelKey.startsWith(`${token}/`));
  });
}

export function modelIdForDeveloper(
  model: ModelMetadataRecord,
  developer: string,
) {
  const modelKey = normalizeDeveloperToken(model.key);
  const providerPrefix = modelDeveloperCatalogTokens(developer).find((token) =>
    modelKey.startsWith(`${token}/`),
  );

  return stripDeveloperPrefix(
    providerPrefix ? model.key.slice(providerPrefix.length + 1) : model.modelId,
    developer,
  );
}

function modelDeveloperCatalogTokens(developer: string) {
  const normalizedDeveloper = normalizeDeveloperToken(developer);

  if (!normalizedDeveloper) {
    return [];
  }

  return [
    normalizedDeveloper,
    ...(MODEL_DEVELOPER_CATALOG_ALIASES[normalizedDeveloper] ?? []),
  ].map(normalizeDeveloperToken);
}

function stripDeveloperPrefix(modelId: string, developer: string) {
  let value = modelId.trim();

  for (;;) {
    const prefix = modelDeveloperCatalogTokens(developer)
      .map((token) => `${token}/`)
      .find((token) => normalizeDeveloperToken(value).startsWith(token));
    if (!prefix) {
      return value;
    }

    value = value.slice(prefix.length);
  }
}

function normalizeDeveloperToken(value: string) {
  return value.trim().toLowerCase();
}
