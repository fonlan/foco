import {
  Bot,
  CheckCircle2,
  LoaderCircle,
  Pencil,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";

import type {
  AgentDefinitionInput,
  AgentDefinitionSettings,
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  ThinkingLevelSummary,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";

type AgentDefinitionDraft = {
  allowedTools: string;
  canCreateInstances: boolean;
  canDelegate: boolean;
  description: string;
  maxInstances: string;
  maxOutputTokens: string;
  modelId: string;
  name: string;
  providerId: string;
  systemPrompt: string;
  thinkingLevel: string;
  allowedAgentDefinitionIds: string[];
};

export function AgentsSettingsPanel({
  definitions,
  error,
  isLoading,
  operationKey,
  models,
  onCreateDefinition,
  onDeleteDefinition,
  onRefreshDefinitions,
  onUpdateDefinition,
  providers,
  thinkingLevels,
}: {
  definitions: AgentDefinitionSettings[];
  error: string | null;
  isLoading: boolean;
  operationKey: string | null;
  models: ConfiguredModelSummary[];
  onCreateDefinition: (definition: AgentDefinitionInput) => Promise<void>;
  onDeleteDefinition: (id: string) => Promise<void>;
  onRefreshDefinitions: () => Promise<unknown>;
  onUpdateDefinition: (id: string, definition: AgentDefinitionInput) => Promise<void>;
  providers: ConfiguredProviderSummary[];
  thinkingLevels: ThinkingLevelSummary[];
}) {
  const { t } = useI18n();
  const enabledModels = useMemo(
    () =>
      models.filter(
        (model) =>
          model.enabled &&
          model.canEnable &&
          model.activeProviderId !== null &&
          model.providerIds.length > 0,
      ),
    [models],
  );
  const [selectedDefinitionId, setSelectedDefinitionId] = useState<string>("");
  const selectedDefinition =
    definitions.find((definition) => definition.id === selectedDefinitionId) ??
    null;
  const providerNameById = useMemo(
    () => new Map(providers.map((provider) => [provider.id, provider.name])),
    [providers],
  );
  const [draft, setDraft] = useState<AgentDefinitionDraft>(() =>
    emptyAgentDefinitionDraft(enabledModels[0]),
  );
  const selectedModel = enabledModels.find((model) => model.id === draft.modelId) ?? null;
  const selectableProviders = selectedModel
    ? selectedModel.providerIds
    : providers.filter((provider) => provider.enabled).map((provider) => provider.id);
  const canSubmit =
    draft.name.trim().length > 0 &&
    draft.description.trim().length > 0 &&
    draft.modelId.trim().length > 0 &&
    draft.providerId.trim().length > 0 &&
    draft.systemPrompt.trim().length > 0 &&
    Number.parseInt(draft.maxInstances, 10) > 0;

  useEffect(() => {
    if (selectedDefinition) {
      setDraft(agentDefinitionToDraft(selectedDefinition));
      return;
    }

    setDraft(emptyAgentDefinitionDraft(enabledModels[0]));
  }, [enabledModels, selectedDefinition]);

  function updateDraft(patch: Partial<AgentDefinitionDraft>) {
    setDraft((current) => ({ ...current, ...patch }));
  }

  function selectModel(modelId: string) {
    const model = enabledModels.find((item) => item.id === modelId) ?? null;
    updateDraft({
      modelId,
      providerId: model?.activeProviderId ?? model?.providerIds[0] ?? "",
    });
  }

  function toggleAllowedDefinition(id: string, checked: boolean) {
    setDraft((current) => ({
      ...current,
      allowedAgentDefinitionIds: checked
        ? [...current.allowedAgentDefinitionIds, id].filter(uniqueString)
        : current.allowedAgentDefinitionIds.filter((item) => item !== id),
    }));
  }

  async function submitDefinition(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const payload = draftToAgentDefinitionInput(draft);
    if (selectedDefinition) {
      await onUpdateDefinition(selectedDefinition.id, payload);
    } else {
      await onCreateDefinition(payload);
    }
  }

  async function deleteSelectedDefinition() {
    if (!selectedDefinition) {
      return;
    }

    if (!window.confirm(t("Delete agent definition?"))) {
      return;
    }

    await onDeleteDefinition(selectedDefinition.id);
    setSelectedDefinitionId("");
  }

  return (
    <section className="grid gap-4 xl:grid-cols-[minmax(240px,320px),minmax(0,1fr)]">
      <div className="rounded-2xl border border-stone-200 bg-white/85 p-3 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
        <div className="flex items-center justify-between gap-2">
          <div className="flex min-w-0 items-center gap-2">
            <Bot aria-hidden="true" className="size-5 shrink-0 text-teal-700" />
            <h3 className="truncate text-sm font-semibold text-stone-950">
              {t("Agent definitions")}
            </h3>
          </div>
          <button
            aria-label={t("Refresh")}
            className="inline-flex size-9 items-center justify-center rounded-lg border border-stone-200 bg-white text-stone-700 hover:border-teal-200 hover:bg-teal-50"
            disabled={isLoading}
            onClick={() => void onRefreshDefinitions()}
            title={t("Refresh")}
            type="button"
          >
            <RefreshCw
              aria-hidden="true"
              className={`size-4 ${isLoading ? "animate-spin" : ""}`}
            />
          </button>
        </div>

        {error ? (
          <div className="mt-3 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
            {error}
          </div>
        ) : null}

        <button
          className={`mt-3 flex w-full items-center gap-2 rounded-lg border px-3 py-2 text-left text-sm font-semibold ${
            selectedDefinitionId === ""
              ? "border-teal-700 bg-teal-50 text-teal-950"
              : "border-stone-200 bg-white text-stone-800 hover:border-teal-200 hover:bg-teal-50/70"
          }`}
          onClick={() => setSelectedDefinitionId("")}
          type="button"
        >
          <Plus aria-hidden="true" className="size-4 shrink-0" />
          <span className="truncate">{t("New agent")}</span>
        </button>

        <div className="mt-3 grid gap-2">
          {definitions.map((definition) => (
            <button
              className={`rounded-lg border px-3 py-2 text-left ${
                selectedDefinitionId === definition.id
                  ? "border-teal-700 bg-teal-50 text-teal-950"
                  : "border-stone-200 bg-white text-stone-800 hover:border-teal-200 hover:bg-teal-50/70"
              }`}
              key={definition.id}
              onClick={() => setSelectedDefinitionId(definition.id)}
              type="button"
            >
              <span className="block truncate text-sm font-semibold">
                {definition.name}
              </span>
              <span className="mt-0.5 block truncate text-xs text-stone-500">
                {providerNameById.get(definition.providerId) ?? definition.providerId} / {definition.modelId}
              </span>
            </button>
          ))}
        </div>
      </div>

      <form
        className="rounded-2xl border border-stone-200 bg-white/85 p-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]"
        onSubmit={(event) => void submitDefinition(event)}
      >
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2">
            {selectedDefinition ? (
              <Pencil aria-hidden="true" className="size-5 shrink-0 text-teal-700" />
            ) : (
              <Plus aria-hidden="true" className="size-5 shrink-0 text-teal-700" />
            )}
            <h3 className="truncate text-sm font-semibold text-stone-950">
              {selectedDefinition ? t("Edit agent") : t("Create agent")}
            </h3>
          </div>
          {selectedDefinition ? (
            <button
              aria-label={t("Delete")}
              className="inline-flex size-9 items-center justify-center rounded-lg border border-rose-200 bg-white text-rose-700 hover:bg-rose-50 disabled:cursor-not-allowed disabled:text-stone-300"
              disabled={operationKey !== null}
              onClick={() => void deleteSelectedDefinition()}
              title={t("Delete")}
              type="button"
            >
              <Trash2 aria-hidden="true" className="size-4" />
            </button>
          ) : null}
        </div>

        <div className="mt-4 grid gap-3 md:grid-cols-2">
          <AgentTextField
            label={t("Name")}
            onChange={(value) => updateDraft({ name: value })}
            value={draft.name}
          />
          <AgentTextField
            inputMode="numeric"
            label={t("Max instances")}
            onChange={(value) => updateDraft({ maxInstances: value })}
            type="number"
            value={draft.maxInstances}
          />
          <label className="block md:col-span-2">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Description")}
            </span>
            <input
              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) => updateDraft({ description: event.target.value })}
              value={draft.description}
            />
          </label>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Model")}
            </span>
            <select
              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) => selectModel(event.target.value)}
              value={draft.modelId}
            >
              <option value="">{t("Select model")}</option>
              {enabledModels.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.displayName}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Provider")}
            </span>
            <select
              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) => updateDraft({ providerId: event.target.value })}
              value={draft.providerId}
            >
              <option value="">{t("Select provider")}</option>
              {selectableProviders.map((providerId) => (
                <option key={providerId} value={providerId}>
                  {providerNameById.get(providerId) ?? providerId}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Thinking")}
            </span>
            <select
              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) => updateDraft({ thinkingLevel: event.target.value })}
              value={draft.thinkingLevel}
            >
              <option value="">{t("Model default")}</option>
              {thinkingLevels.map((level) => (
                <option key={level.value} value={level.value}>
                  {t(level.label)}
                </option>
              ))}
            </select>
          </label>
          <AgentTextField
            inputMode="numeric"
            label={t("Max output tokens")}
            onChange={(value) => updateDraft({ maxOutputTokens: value })}
            placeholder={selectedModel?.maxOutputTokens ? String(selectedModel.maxOutputTokens) : ""}
            type="number"
            value={draft.maxOutputTokens}
          />
          <label className="block md:col-span-2">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("Allowed tools")}
            </span>
            <input
              className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) => updateDraft({ allowedTools: event.target.value })}
              placeholder="read_file, edit_file, run_command"
              value={draft.allowedTools}
            />
          </label>
          <label className="block md:col-span-2">
            <span className="mb-1.5 block text-xs font-semibold text-stone-600">
              {t("System prompt")}
            </span>
            <textarea
              className="min-h-36 w-full resize-y rounded-lg border border-stone-300 bg-white px-3 py-2 text-sm leading-6 text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
              onChange={(event) => updateDraft({ systemPrompt: event.target.value })}
              value={draft.systemPrompt}
            />
          </label>
        </div>

        <fieldset className="mt-4 rounded-xl border border-stone-200 bg-stone-50/70 px-3 py-3">
          <legend className="px-1 text-xs font-semibold text-stone-600">
            {t("Permissions")}
          </legend>
          <div className="grid gap-3 md:grid-cols-2">
            <AgentCheckbox
              checked={draft.canDelegate}
              label={t("Can delegate tasks")}
              onChange={(checked) => updateDraft({ canDelegate: checked })}
            />
            <AgentCheckbox
              checked={draft.canCreateInstances}
              label={t("Can create instances")}
              onChange={(checked) => updateDraft({ canCreateInstances: checked })}
            />
          </div>
          <div className="mt-3 grid gap-2 md:grid-cols-2">
            {definitions
              .filter((definition) => definition.id !== selectedDefinition?.id)
              .map((definition) => (
                <AgentCheckbox
                  checked={draft.allowedAgentDefinitionIds.includes(definition.id)}
                  key={definition.id}
                  label={definition.name}
                  onChange={(checked) => toggleAllowedDefinition(definition.id, checked)}
                />
              ))}
          </div>
        </fieldset>

        <div className="mt-4 flex justify-end">
          <button
            className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-4 text-sm font-semibold text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] hover:bg-teal-900 disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
            disabled={!canSubmit || operationKey !== null}
            type="submit"
          >
            {operationKey === "agent-definition-save" ? (
              <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            ) : (
              <CheckCircle2 aria-hidden="true" className="size-4" />
            )}
            <span>{selectedDefinition ? t("Save") : t("Create")}</span>
          </button>
        </div>
      </form>
    </section>
  );
}

function AgentTextField({
  inputMode,
  label,
  onChange,
  placeholder,
  type = "text",
  value,
}: {
  inputMode?: "numeric";
  label: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: "number" | "text";
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">
        {label}
      </span>
      <input
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition placeholder:text-stone-400 focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        inputMode={inputMode}
        min={type === "number" ? 1 : undefined}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        step={type === "number" ? 1 : undefined}
        type={type}
        value={value}
      />
    </label>
  );
}

function AgentCheckbox({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex min-w-0 cursor-pointer items-center justify-between gap-3 rounded-lg border border-stone-200 bg-white px-3 py-2 text-sm text-stone-800">
      <span className="truncate font-medium">{label}</span>
      <input
        checked={checked}
        className="size-4 shrink-0 accent-teal-700"
        onChange={(event) => onChange(event.target.checked)}
        type="checkbox"
      />
    </label>
  );
}

function emptyAgentDefinitionDraft(
  model: ConfiguredModelSummary | undefined,
): AgentDefinitionDraft {
  return {
    allowedAgentDefinitionIds: [],
    allowedTools: "",
    canCreateInstances: false,
    canDelegate: false,
    description: "",
    maxInstances: "1",
    maxOutputTokens: "",
    modelId: model?.id ?? "",
    name: "",
    providerId: model?.activeProviderId ?? model?.providerIds[0] ?? "",
    systemPrompt: "",
    thinkingLevel: "",
  };
}

function agentDefinitionToDraft(
  definition: AgentDefinitionSettings,
): AgentDefinitionDraft {
  return {
    allowedAgentDefinitionIds: definition.permissions.allowedAgentDefinitionIds,
    allowedTools: definition.allowedTools.join(", "),
    canCreateInstances: definition.permissions.canCreateInstances,
    canDelegate: definition.permissions.canDelegate,
    description: definition.description,
    maxInstances: String(definition.maxInstances),
    maxOutputTokens: definition.modelOptions.maxOutputTokens
      ? String(definition.modelOptions.maxOutputTokens)
      : "",
    modelId: definition.modelId,
    name: definition.name,
    providerId: definition.providerId,
    systemPrompt: definition.systemPrompt,
    thinkingLevel: definition.modelOptions.thinkingLevel ?? "",
  };
}

function draftToAgentDefinitionInput(
  draft: AgentDefinitionDraft,
): AgentDefinitionInput {
  const maxOutputTokens = draft.maxOutputTokens.trim()
    ? Number.parseInt(draft.maxOutputTokens, 10)
    : null;

  return {
    allowedTools: draft.allowedTools
      .split(/[\s,]+/)
      .map((tool) => tool.trim())
      .filter(Boolean)
      .filter(uniqueString),
    description: draft.description.trim(),
    maxInstances: Number.parseInt(draft.maxInstances, 10),
    modelId: draft.modelId,
    modelOptions: {
      maxOutputTokens,
      thinkingLevel: draft.thinkingLevel || null,
    },
    name: draft.name.trim(),
    permissions: {
      allowedAgentDefinitionIds: draft.allowedAgentDefinitionIds,
      canCreateInstances: draft.canCreateInstances,
      canDelegate: draft.canDelegate,
    },
    providerId: draft.providerId,
    systemPrompt: draft.systemPrompt.trim(),
  };
}

function uniqueString(value: string, index: number, values: string[]) {
  return values.indexOf(value) === index;
}
