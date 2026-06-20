import {
  Bot,
  CheckCircle2,
  LoaderCircle,
  Pencil,
  Plus,
  Trash2,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";

import type {
  AgentDefinitionInput,
  AgentDefinitionSettings,
  ConfiguredModelSummary,
  ConfiguredProviderSummary,
  SystemPromptSummary,
  ThinkingLevelSummary,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";

type AgentDefinitionDraft = {
  allowedTools: string[];
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
  agentTools,
  defaultTeamModeEnabled,
  definitions,
  error,
  isLoading,
  isSavingDefaultTeamMode,
  operationKey,
  models,
  onCreateDefinition,
  onDefaultTeamModeEnabledChange,
  onDeleteDefinition,
  onUpdateDefinition,
  providers,
  systemPrompts,
  thinkingLevels,
}: {
  agentTools: string[];
  defaultTeamModeEnabled: boolean;
  definitions: AgentDefinitionSettings[];
  error: string | null;
  isLoading: boolean;
  isSavingDefaultTeamMode: boolean;
  operationKey: string | null;
  models: ConfiguredModelSummary[];
  onCreateDefinition: (definition: AgentDefinitionInput) => Promise<boolean>;
  onDefaultTeamModeEnabledChange: (enabled: boolean) => Promise<void>;
  onDeleteDefinition: (id: string) => Promise<void>;
  onUpdateDefinition: (
    id: string,
    definition: AgentDefinitionInput,
  ) => Promise<boolean>;
  providers: ConfiguredProviderSummary[];
  systemPrompts: SystemPromptSummary[];
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
  const providerNameById = useMemo(
    () => new Map(providers.map((provider) => [provider.id, provider.name])),
    [providers],
  );
  const modelNameById = useMemo(
    () => new Map(models.map((model) => [model.id, model.displayName])),
    [models],
  );
  const [dialogMode, setDialogMode] = useState<"create" | "edit" | null>(null);
  const [editingDefinitionId, setEditingDefinitionId] = useState<string | null>(null);
  const [draft, setDraft] = useState<AgentDefinitionDraft>(() =>
    emptyAgentDefinitionDraft(enabledModels[0], systemPrompts[0]),
  );
  const editingDefinition =
    definitions.find((definition) => definition.id === editingDefinitionId) ?? null;
  const selectedModel = enabledModels.find((model) => model.id === draft.modelId) ?? null;
  const selectableProviders = selectedModel
    ? selectedModel.providerIds
    : providers.filter((provider) => provider.enabled).map((provider) => provider.id);
  const selectableTools = useMemo(
    () => [...new Set([...agentTools, ...draft.allowedTools])].sort(),
    [agentTools, draft.allowedTools],
  );
  const selectedSystemPromptName =
    systemPrompts.find((prompt) => prompt.content === draft.systemPrompt)?.name ?? null;
  const canSubmit =
    draft.name.trim().length > 0 &&
    draft.description.trim().length > 0 &&
    draft.modelId.trim().length > 0 &&
    draft.providerId.trim().length > 0 &&
    draft.systemPrompt.trim().length > 0 &&
    Number.parseInt(draft.maxInstances, 10) > 0;

  useEffect(() => {
    if (!dialogMode) {
      return;
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape" && operationKey === null) {
        setDialogMode(null);
      }
    }

    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [dialogMode, operationKey]);

  function updateDraft(patch: Partial<AgentDefinitionDraft>) {
    setDraft((current) => ({ ...current, ...patch }));
  }

  function openCreateDialog() {
    setEditingDefinitionId(null);
    setDraft(emptyAgentDefinitionDraft(enabledModels[0], systemPrompts[0]));
    setDialogMode("create");
  }

  function openEditDialog(definition: AgentDefinitionSettings) {
    setEditingDefinitionId(definition.id);
    setDraft(agentDefinitionToDraft(definition));
    setDialogMode("edit");
  }

  function closeDialog() {
    if (operationKey === null) {
      setDialogMode(null);
    }
  }

  function selectModel(modelId: string) {
    const model = enabledModels.find((item) => item.id === modelId) ?? null;
    updateDraft({
      modelId,
      providerId: model?.activeProviderId ?? model?.providerIds[0] ?? "",
    });
  }

  function toggleAllowedTool(tool: string, checked: boolean) {
    setDraft((current) => ({
      ...current,
      allowedTools: checked
        ? [...current.allowedTools, tool].filter(uniqueString)
        : current.allowedTools.filter((item) => item !== tool),
    }));
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
    const saved = editingDefinition
      ? await onUpdateDefinition(editingDefinition.id, payload)
      : await onCreateDefinition(payload);
    if (saved) {
      setDialogMode(null);
    }
  }

  async function deleteDefinition(definition: AgentDefinitionSettings) {
    if (!window.confirm(t("Delete agent definition?"))) {
      return;
    }

    await onDeleteDefinition(definition.id);
  }

  return (
    <section className="rounded-2xl border border-stone-200 bg-white/85 p-4 shadow-[0_18px_42px_rgba(75,63,42,0.07)]">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <Bot aria-hidden="true" className="size-5 shrink-0 text-teal-700" />
          <h3 className="truncate text-sm font-semibold text-stone-950">
            {t("Agent definitions")}
          </h3>
        </div>
        <button
          aria-label={t("Add agent definition")}
          className="inline-flex size-9 items-center justify-center rounded-lg bg-teal-800 text-white shadow-[0_10px_24px_rgba(15,118,110,0.2)] transition hover:bg-teal-900 active:translate-y-px disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
          disabled={operationKey !== null}
          onClick={openCreateDialog}
          title={t("Add agent definition")}
          type="button"
        >
          <Plus aria-hidden="true" className="size-4" />
        </button>
      </div>

      <label className="mt-4 flex items-center justify-between gap-3 rounded-lg border border-stone-200 bg-stone-50/80 px-3 py-2">
        <span className="text-sm font-semibold text-stone-700">
          {t("Default Team mode for new chats")}
        </span>
        <input
          checked={defaultTeamModeEnabled}
          className="size-4 accent-teal-700"
          disabled={isSavingDefaultTeamMode}
          onChange={(event) =>
            void onDefaultTeamModeEnabledChange(event.target.checked)
          }
          type="checkbox"
        />
      </label>

      {error ? (
        <div className="mt-3 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
          {error}
        </div>
      ) : null}

      <div className="mt-4 grid gap-2">
        {definitions.map((definition) => (
          <article
            className="group flex items-start gap-3 rounded-xl border border-stone-200 bg-stone-50/65 px-3 py-3 transition hover:border-teal-200 hover:bg-teal-50/45"
            key={definition.id}
          >
            <div className="min-w-0 flex-1">
              <div className="flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1">
                <h4 className="truncate text-sm font-semibold text-stone-950">
                  {definition.name}
                </h4>
                <span className="truncate text-xs font-medium text-stone-500">
                  {modelNameById.get(definition.modelId) ?? definition.modelId}
                  <span aria-hidden="true"> · </span>
                  {providerNameById.get(definition.providerId) ?? definition.providerId}
                </span>
              </div>
              <p className="mt-1 text-sm leading-5 text-stone-600">
                {definition.description}
              </p>
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <button
                aria-label={t("Edit agent {name}", { name: definition.name })}
                className="inline-flex size-8 items-center justify-center rounded-lg text-stone-500 transition hover:bg-white hover:text-teal-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-600 disabled:cursor-not-allowed disabled:text-stone-300"
                disabled={operationKey !== null}
                onClick={() => openEditDialog(definition)}
                title={t("Edit")}
                type="button"
              >
                <Pencil aria-hidden="true" className="size-4" />
              </button>
              <button
                aria-label={t("Delete agent {name}", { name: definition.name })}
                className="inline-flex size-8 items-center justify-center rounded-lg text-stone-500 transition hover:bg-rose-50 hover:text-rose-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-500 disabled:cursor-not-allowed disabled:text-stone-300"
                disabled={operationKey !== null}
                onClick={() => void deleteDefinition(definition)}
                title={t("Delete")}
                type="button"
              >
                <Trash2 aria-hidden="true" className="size-4" />
              </button>
            </div>
          </article>
        ))}
        {isLoading ? (
          <div className="flex items-center justify-center gap-2 py-8 text-sm text-stone-500">
            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            {t("Loading agent definitions...")}
          </div>
        ) : definitions.length === 0 ? (
          <div className="rounded-xl border border-dashed border-stone-300 px-4 py-8 text-center text-sm text-stone-500">
            {t("No agent definitions")}
          </div>
        ) : null}
      </div>

      {dialogMode ? (
        <div
          aria-label={t("Close agent dialog backdrop")}
          className="fixed inset-0 z-50 grid place-items-center overflow-y-auto bg-stone-950/35 p-4 backdrop-blur-sm"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              closeDialog();
            }
          }}
          role="presentation"
        >
          <form
            aria-label={dialogMode === "edit" ? t("Edit agent") : t("Create agent")}
            aria-modal="true"
            className="my-auto w-[min(94vw,52rem)] rounded-2xl border border-stone-200 bg-white p-4 shadow-[0_30px_80px_rgba(33,31,28,0.28)]"
            onSubmit={(event) => void submitDefinition(event)}
            role="dialog"
          >
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                {dialogMode === "edit" ? (
                  <Pencil aria-hidden="true" className="size-5 shrink-0 text-teal-700" />
                ) : (
                  <Plus aria-hidden="true" className="size-5 shrink-0 text-teal-700" />
                )}
                <h3 className="truncate text-base font-semibold text-stone-950">
                  {dialogMode === "edit" ? t("Edit agent") : t("Create agent")}
                </h3>
              </div>
              <button
                aria-label={t("Close agent dialog")}
                className="inline-flex size-9 items-center justify-center rounded-lg text-stone-500 transition hover:bg-stone-100 hover:text-stone-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-600 disabled:cursor-not-allowed disabled:text-stone-300"
                disabled={operationKey !== null}
                onClick={closeDialog}
                title={t("Close")}
                type="button"
              >
                <X aria-hidden="true" className="size-4" />
              </button>
            </div>

            {error ? (
              <div className="mt-3 rounded-lg border border-rose-200 bg-rose-50 px-3 py-2 text-sm text-rose-700">
                {error}
              </div>
            ) : null}

            <div className="mt-4 grid gap-3 md:grid-cols-2">
              <AgentTextField
                autoFocus
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
              <AgentSelect label={t("Model")} value={draft.modelId} onChange={selectModel}>
                <option value="">{t("Select model")}</option>
                {enabledModels.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.displayName}
                  </option>
                ))}
              </AgentSelect>
              <AgentSelect
                label={t("Provider")}
                onChange={(providerId) => updateDraft({ providerId })}
                value={draft.providerId}
              >
                <option value="">{t("Select provider")}</option>
                {selectableProviders.map((providerId) => (
                  <option key={providerId} value={providerId}>
                    {providerNameById.get(providerId) ?? providerId}
                  </option>
                ))}
              </AgentSelect>
              <AgentSelect
                label={t("Thinking")}
                onChange={(thinkingLevel) => updateDraft({ thinkingLevel })}
                value={draft.thinkingLevel}
              >
                <option value="">{t("Model default")}</option>
                {thinkingLevels.map((level) => (
                  <option key={level.value} value={level.value}>
                    {t(level.label)}
                  </option>
                ))}
              </AgentSelect>
              <AgentTextField
                inputMode="numeric"
                label={t("Max output tokens")}
                onChange={(value) => updateDraft({ maxOutputTokens: value })}
                placeholder={
                  selectedModel?.maxOutputTokens ? String(selectedModel.maxOutputTokens) : ""
                }
                type="number"
                value={draft.maxOutputTokens}
              />
              <label className="block md:col-span-2">
                <span className="mb-1.5 block text-xs font-semibold text-stone-600">
                  {t("System prompt")}
                </span>
                <select
                  className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
                  onChange={(event) => {
                    const prompt = systemPrompts.find(
                      (item) => item.name === event.target.value,
                    );
                    updateDraft({ systemPrompt: prompt?.content ?? draft.systemPrompt });
                  }}
                  value={selectedSystemPromptName ?? "__current"}
                >
                  {selectedSystemPromptName === null && draft.systemPrompt ? (
                    <option value="__current">{t("Current custom prompt")}</option>
                  ) : null}
                  {!systemPrompts.length ? (
                    <option value="__current">{t("No system prompts configured")}</option>
                  ) : null}
                  {systemPrompts.map((prompt) => (
                    <option key={prompt.name} value={prompt.name}>
                      {prompt.name}
                    </option>
                  ))}
                </select>
              </label>
              <details className="group/tools relative md:col-span-2">
                <summary className="flex h-10 cursor-pointer list-none items-center justify-between rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition marker:content-none focus-visible:border-teal-700 focus-visible:ring-2 focus-visible:ring-teal-100">
                  <span className="font-medium">{t("Allowed tools")}</span>
                  <span className="text-xs text-stone-500">
                    {t("{count} selected", { count: draft.allowedTools.length })}
                  </span>
                </summary>
                <div className="absolute z-10 mt-1 max-h-64 w-full overflow-y-auto rounded-xl border border-stone-200 bg-white p-2 shadow-[0_18px_42px_rgba(75,63,42,0.16)]">
                  {selectableTools.map((tool) => (
                    <AgentCheckbox
                      checked={draft.allowedTools.includes(tool)}
                      key={tool}
                      label={tool}
                      onChange={(checked) => toggleAllowedTool(tool, checked)}
                    />
                  ))}
                  {!selectableTools.length ? (
                    <p className="px-2 py-3 text-sm text-stone-500">
                      {t("No tools available")}
                    </p>
                  ) : null}
                </div>
              </details>
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
                  .filter((definition) => definition.id !== editingDefinition?.id)
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

            <div className="mt-4 flex justify-end gap-2">
              <button
                className="inline-flex h-10 items-center justify-center rounded-lg border border-stone-200 bg-white px-4 text-sm font-semibold text-stone-700 transition hover:bg-stone-50 active:translate-y-px disabled:cursor-not-allowed disabled:text-stone-300"
                disabled={operationKey !== null}
                onClick={closeDialog}
                type="button"
              >
                {t("Cancel")}
              </button>
              <button
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-teal-800 px-4 text-sm font-semibold text-white shadow-[0_12px_28px_rgba(15,118,110,0.22)] transition hover:bg-teal-900 active:translate-y-px disabled:cursor-not-allowed disabled:bg-stone-300 disabled:shadow-none"
                disabled={!canSubmit || operationKey !== null}
                type="submit"
              >
                {operationKey === "agent-definition-save" ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <CheckCircle2 aria-hidden="true" className="size-4" />
                )}
                <span>{dialogMode === "edit" ? t("Save") : t("Create")}</span>
              </button>
            </div>
          </form>
        </div>
      ) : null}
    </section>
  );
}

function AgentTextField({
  autoFocus,
  inputMode,
  label,
  onChange,
  placeholder,
  type = "text",
  value,
}: {
  autoFocus?: boolean;
  inputMode?: "numeric";
  label: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: "number" | "text";
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">{label}</span>
      <input
        autoFocus={autoFocus}
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

function AgentSelect({
  children,
  label,
  onChange,
  value,
}: {
  children: React.ReactNode;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-semibold text-stone-600">{label}</span>
      <select
        className="h-10 w-full rounded-lg border border-stone-300 bg-white px-3 text-sm text-stone-900 outline-none transition focus:border-teal-700 focus:ring-2 focus:ring-teal-100"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {children}
      </select>
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
    <label className="flex min-w-0 cursor-pointer items-center justify-between gap-3 rounded-lg px-3 py-2 text-sm text-stone-800 transition hover:bg-stone-50">
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
  systemPrompt: SystemPromptSummary | undefined,
): AgentDefinitionDraft {
  return {
    allowedAgentDefinitionIds: [],
    allowedTools: [],
    canCreateInstances: false,
    canDelegate: false,
    description: "",
    maxInstances: "1",
    maxOutputTokens: "",
    modelId: model?.id ?? "",
    name: "",
    providerId: model?.activeProviderId ?? model?.providerIds[0] ?? "",
    systemPrompt: systemPrompt?.content ?? "",
    thinkingLevel: "",
  };
}

function agentDefinitionToDraft(
  definition: AgentDefinitionSettings,
): AgentDefinitionDraft {
  return {
    allowedAgentDefinitionIds: definition.permissions.allowedAgentDefinitionIds,
    allowedTools: definition.allowedTools,
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
  return {
    allowedTools: draft.allowedTools,
    description: draft.description.trim(),
    maxInstances: Number.parseInt(draft.maxInstances, 10),
    modelId: draft.modelId,
    modelOptions: {
      maxOutputTokens: draft.maxOutputTokens.trim()
        ? Number.parseInt(draft.maxOutputTokens, 10)
        : null,
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
