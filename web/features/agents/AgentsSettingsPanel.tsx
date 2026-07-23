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
  AgentExecutionWorkspaceMode,
  ConfiguredModelSummary,
  ThinkingLevelSummary,
} from "../../api/types";
import { useI18n } from "../../shared/i18n";
import {
  Modal,
  SettingsButton,
  SettingsInput,
  SettingsSelect,
  SettingsTextArea,
} from "../../shared/ui";
import {
  defaultThinkingLevelForModel,
  isModelThinkingLevelSupported,
  thinkingLevelOptionsForModel,
} from "../../shared/thinking-levels";

const AGENT_EXECUTION_WORKSPACE_MODES: AgentExecutionWorkspaceMode[] = [
  "shared",
  "isolated_worktree",
];
const DEFAULT_AGENT_DEFINITION_ID = "agent-definition-default";
const REVIEW_AGENT_DEFINITION_ID = "agent-definition-review";
const IMAGE_AGENT_DEFINITION_ID = "agent-definition-image-gen";

type AgentDefinitionDraft = {
  allowedTools: string[];
  allowedExecutionWorkspaceModes: AgentExecutionWorkspaceMode[];
  canCreateInstances: boolean;
  canDelegate: boolean;
  description: string;
  maxInstances: string;
  maxOutputTokens: string;
  modelId: string;
  name: string;
  systemPrompt: string;
  thinkingLevel: string;
  allowedAgentDefinitionIds: string[];
};

export function AgentsSettingsPanel({
  agentTools,
  defaultRolePrompts,
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
  thinkingLevels,
}: {
  agentTools: string[];
  defaultRolePrompts: Record<string, string>;
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
  thinkingLevels: ThinkingLevelSummary[];
}) {
  const { t } = useI18n();
  const enabledModels = useMemo(
    () =>
      models.filter(
        (model) =>
          model.enabled &&
          model.canEnable &&
          modelOutputsText(model) &&
          model.activeProviderId !== null &&
          model.providerIds.length > 0,
      ),
    [models],
  );
  const modelNameById = useMemo(
    () => new Map(models.map((model) => [model.id, model.displayName])),
    [models],
  );
  const [dialogMode, setDialogMode] = useState<"create" | "edit" | null>(null);
  const [editingDefinitionId, setEditingDefinitionId] = useState<string | null>(null);
  const [draft, setDraft] = useState<AgentDefinitionDraft>(() =>
    emptyAgentDefinitionDraft(enabledModels[0]),
  );
  const editingDefinition =
    definitions.find((definition) => definition.id === editingDefinitionId) ?? null;
  const selectedModel = enabledModels.find((model) => model.id === draft.modelId) ?? null;
  const thinkingOptions = useMemo(
    () => thinkingLevelOptionsForModel(selectedModel, thinkingLevels),
    [selectedModel, thinkingLevels],
  );
  const selectableTools = useMemo(
    () => [...new Set([...agentTools, ...draft.allowedTools])].sort(),
    [agentTools, draft.allowedTools],
  );
  const defaultRolePrompt =
    editingDefinition && isBuiltinAgentDefinition(editingDefinition.id)
      ? defaultRolePrompts[editingDefinition.id]
      : null;
  const canSubmit =
    draft.name.trim().length > 0 &&
    draft.description.trim().length > 0 &&
    draft.modelId.trim().length > 0 &&
    draft.systemPrompt.trim().length > 0 &&
    Number.parseInt(draft.maxInstances, 10) > 0 &&
    draft.allowedExecutionWorkspaceModes.length > 0;

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
    setDraft(emptyAgentDefinitionDraft(enabledModels[0]));
    setDialogMode("create");
  }

  function openEditDialog(definition: AgentDefinitionSettings) {
    const model = enabledModels.find((item) => item.id === definition.modelId) ?? null;
    setEditingDefinitionId(definition.id);
    setDraft({
      ...agentDefinitionToDraft(definition),
      thinkingLevel: normalizeAgentThinkingLevel(model, definition.modelOptions.thinkingLevel),
    });
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
      thinkingLevel: defaultThinkingLevelForModel(model),
    });
  }

  function restoreDefaultRolePrompt() {
    if (!defaultRolePrompt) {
      return;
    }
    updateDraft({
      systemPrompt: defaultRolePrompt,
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

  function toggleAllowedExecutionWorkspaceMode(
    mode: AgentExecutionWorkspaceMode,
    checked: boolean,
  ) {
    setDraft((current) => ({
      ...current,
      allowedExecutionWorkspaceModes: checked
        ? [...current.allowedExecutionWorkspaceModes, mode].filter(uniqueString)
        : current.allowedExecutionWorkspaceModes.filter((item) => item !== mode),
    }));
  }

  async function submitDefinition(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const payload = draftToAgentDefinitionInput({
      ...draft,
      thinkingLevel: normalizeAgentThinkingLevel(selectedModel, draft.thinkingLevel),
    });
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
    <section className="rounded-2xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface)_85%,transparent)] p-4 shadow-[var(--overlay-shadow)]">
      <div className="flex items-center justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2">
          <Bot aria-hidden="true" className="size-5 shrink-0 text-[var(--accent-soft-foreground)]" />
          <h3 className="truncate text-sm font-semibold text-[var(--foreground)]">
            {t("Agent definitions")}
          </h3>
        </div>
        <SettingsButton
          aria-label={t("Add agent definition")}
          className="inline-flex size-9 items-center justify-center rounded-lg bg-[var(--accent)] text-white shadow-[var(--overlay-shadow)] transition hover:bg-[var(--accent)] active:translate-y-px disabled:cursor-not-allowed disabled:bg-[var(--default)] disabled:shadow-none"
          disabled={operationKey !== null}
          onClick={openCreateDialog}
          title={t("Add agent definition")}
          type="button"
        >
          <Plus aria-hidden="true" className="size-4" />
        </SettingsButton>
      </div>

      <label className="mt-4 flex items-center justify-between gap-3 rounded-lg border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_80%,transparent)] px-3 py-2">
        <span className="text-sm font-semibold text-[var(--muted)]">
          {t("Default Team mode for new chats")}
        </span>
        <SettingsInput
          checked={defaultTeamModeEnabled}
          className="size-4 accent-[var(--accent)]"
          disabled={isSavingDefaultTeamMode}
          onChange={(event) =>
            void onDefaultTeamModeEnabledChange(event.target.checked)
          }
          type="checkbox"
        />
      </label>

      {error ? (
        <div className="mt-3 rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
          {error}
        </div>
      ) : null}

      <div className="mt-4 grid gap-2">
        {definitions.map((definition) => {
          const isBuiltin = isBuiltinAgentDefinition(definition.id);

          return (
            <article
              className="group flex items-start gap-3 rounded-xl border border-[var(--border)] bg-[var(--surface-secondary)]/65 px-3 py-3 transition hover:border-[var(--accent)] hover:bg-[var(--accent-soft)]/45"
              key={definition.id}
            >
              <div className="min-w-0 flex-1">
                <div className="flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1">
                  <h4 className="truncate text-sm font-semibold text-[var(--foreground)]">
                    {definition.name}
                  </h4>
                  <span className="truncate text-xs font-medium text-[var(--muted)]">
                    {modelNameById.get(definition.modelId) ?? definition.modelId}
                    <span aria-hidden="true"> · </span>
                    {t("Resolved by model routing")}
                  </span>
                </div>
                <p className="mt-1 text-sm leading-5 text-[var(--muted)]">
                  {definition.description}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-1">
                <SettingsButton
                  aria-label={t("Edit agent {name}", { name: definition.name })}
                  className="inline-flex size-8 items-center justify-center rounded-lg text-[var(--muted)] transition hover:bg-[var(--surface)] hover:text-[var(--accent-soft-foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                  disabled={operationKey !== null}
                  onClick={() => openEditDialog(definition)}
                  title={t("Edit")}
                  type="button"
                >
                  <Pencil aria-hidden="true" className="size-4" />
                </SettingsButton>
                {!isBuiltin ? (
                  <SettingsButton
                    aria-label={t("Delete agent {name}", { name: definition.name })}
                    className="inline-flex size-8 items-center justify-center rounded-lg text-[var(--muted)] transition hover:bg-[var(--danger-soft)] hover:text-[var(--danger)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--danger)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                    disabled={operationKey !== null}
                    onClick={() => void deleteDefinition(definition)}
                    title={t("Delete")}
                    type="button"
                  >
                    <Trash2 aria-hidden="true" className="size-4" />
                  </SettingsButton>
                ) : null}
              </div>
            </article>
          );
        })}
        {isLoading ? (
          <div className="flex items-center justify-center gap-2 py-8 text-sm text-[var(--muted)]">
            <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
            {t("Loading agent definitions…")}
          </div>
        ) : definitions.length === 0 ? (
          <div className="rounded-xl border border-dashed border-[var(--border)] px-4 py-8 text-center text-sm text-[var(--muted)]">
            {t("No agent definitions")}
          </div>
        ) : null}
      </div>

      {dialogMode ? (
        <Modal.Backdrop isDismissable isOpen onOpenChange={(open) => !open && closeDialog()}>
          <Modal.Container placement="center" size="lg">
          <Modal.Dialog
            aria-label={dialogMode === "edit" ? t("Edit agent") : t("Create agent")}
            className="my-auto w-[min(94vw,52rem)] rounded-2xl border border-[var(--border)] bg-[var(--surface)] p-4 shadow-[var(--overlay-shadow)]"
          >
          <form onSubmit={(event) => void submitDefinition(event)}>
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                {dialogMode === "edit" ? (
                  <Pencil aria-hidden="true" className="size-5 shrink-0 text-[var(--accent-soft-foreground)]" />
                ) : (
                  <Plus aria-hidden="true" className="size-5 shrink-0 text-[var(--accent-soft-foreground)]" />
                )}
                <h3 className="truncate text-base font-semibold text-[var(--foreground)]">
                  {dialogMode === "edit" ? t("Edit agent") : t("Create agent")}
                </h3>
              </div>
              <SettingsButton
                aria-label={t("Close agent dialog")}
                className="inline-flex size-9 items-center justify-center rounded-lg text-[var(--muted)] transition hover:bg-[var(--surface-secondary)] hover:text-[var(--foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                disabled={operationKey !== null}
                onClick={closeDialog}
                title={t("Close")}
                type="button"
              >
                <X aria-hidden="true" className="size-4" />
              </SettingsButton>
            </div>

            {error ? (
              <div className="mt-3 rounded-lg border border-[var(--danger)] bg-[var(--danger-soft)] px-3 py-2 text-sm text-[var(--danger)]">
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
                <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">
                  {t("Description")}
                </span>
                <SettingsInput
                  className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
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
                label={t("Thinking")}
                onChange={(thinkingLevel) => updateDraft({ thinkingLevel })}
                value={draft.thinkingLevel}
              >
                <option value="">{t("Model default")}</option>
                {thinkingOptions.map((level) => (
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
              <div className="block md:col-span-2">
                <span className="mb-1.5 flex items-center justify-between gap-3">
                  <span className="text-xs font-semibold text-[var(--muted)]">
                    {t("Agent role prompt")}
                  </span>
                  {defaultRolePrompt ? (
                    <SettingsButton
                      className="inline-flex h-8 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-xs font-semibold text-[var(--muted)] transition hover:bg-[var(--surface-secondary)] hover:text-[var(--accent-soft-foreground)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)] disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                      disabled={operationKey !== null}
                      onClick={restoreDefaultRolePrompt}
                      type="button"
                    >
                      {t("Restore default Agent role prompt")}
                    </SettingsButton>
                  ) : null}
                </span>
                <SettingsTextArea
                  aria-label={t("Agent role prompt")}
                  className="min-h-40 w-full resize-y rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 py-2 font-mono text-sm leading-6 text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
                  onChange={(event) => updateDraft({ systemPrompt: event.target.value })}
                  value={draft.systemPrompt}
                />
              </div>
              <details className="group/tools relative md:col-span-2">
                <summary className="flex h-10 cursor-pointer list-none items-center justify-between rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition marker:content-none focus-visible:border-[var(--accent)] focus-visible:ring-2 focus-visible:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]">
                  <span className="font-medium">{t("Allowed tools")}</span>
                  <span className="text-xs text-[var(--muted)]">
                    {t("{count} selected", { count: draft.allowedTools.length })}
                  </span>
                </summary>
                <div className="absolute z-10 mt-1 max-h-64 w-full overflow-y-auto rounded-xl border border-[var(--border)] bg-[var(--surface)] p-2 shadow-[var(--overlay-shadow)]">
                  {selectableTools.map((tool) => (
                    <AgentCheckbox
                      checked={draft.allowedTools.includes(tool)}
                      key={tool}
                      label={tool}
                      onChange={(checked) => toggleAllowedTool(tool, checked)}
                    />
                  ))}
                  {!selectableTools.length ? (
                    <p className="px-2 py-3 text-sm text-[var(--muted)]">
                      {t("No tools available")}
                    </p>
                  ) : null}
                </div>
              </details>
            </div>

            <fieldset className="mt-4 rounded-xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_70%,transparent)] px-3 py-3">
              <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
                {t("Workspace isolation mode")}
              </legend>
              <div className="grid gap-2">
                <AgentCheckbox
                  checked={draft.allowedExecutionWorkspaceModes.includes("shared")}
                  description={t(
                    "Uses the current chat workspace directly. Simpler, but file changes land in the shared workspace.",
                  )}
                  label={t("Shared workspace")}
                  onChange={(checked) => toggleAllowedExecutionWorkspaceMode("shared", checked)}
                />
                <AgentCheckbox
                  checked={draft.allowedExecutionWorkspaceModes.includes("isolated_worktree")}
                  description={t(
                    "Creates a Foco-managed Git worktree for the instance. File changes stay isolated until you explicitly merge or delete them.",
                  )}
                  label={t("Isolated workspace")}
                  onChange={(checked) =>
                    toggleAllowedExecutionWorkspaceMode("isolated_worktree", checked)
                  }
                />
              </div>
            </fieldset>

            <fieldset className="mt-4 rounded-xl border border-[var(--border)] bg-[color-mix(in_oklab,var(--surface-secondary)_70%,transparent)] px-3 py-3">
              <legend className="px-1 text-xs font-semibold text-[var(--muted)]">
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
              <SettingsButton
                className="inline-flex h-10 items-center justify-center rounded-lg border border-[var(--border)] bg-[var(--surface)] px-4 text-sm font-semibold text-[var(--muted)] transition hover:bg-[var(--surface-secondary)] active:translate-y-px disabled:cursor-not-allowed disabled:text-[var(--muted)]"
                disabled={operationKey !== null}
                onClick={closeDialog}
                type="button"
              >
                {t("Cancel")}
              </SettingsButton>
              <SettingsButton
                className="inline-flex h-10 items-center justify-center gap-2 rounded-lg bg-[var(--accent)] px-4 text-sm font-semibold text-white shadow-[var(--overlay-shadow)] transition hover:bg-[var(--accent)] active:translate-y-px disabled:cursor-not-allowed disabled:bg-[var(--default)] disabled:shadow-none"
                disabled={!canSubmit || operationKey !== null}
                type="submit"
              >
                {operationKey === "agent-definition-save" ? (
                  <LoaderCircle aria-hidden="true" className="size-4 animate-spin" />
                ) : (
                  <CheckCircle2 aria-hidden="true" className="size-4" />
                )}
                <span>{dialogMode === "edit" ? t("Save") : t("Create")}</span>
              </SettingsButton>
            </div>
          </form>
          </Modal.Dialog>
          </Modal.Container>
        </Modal.Backdrop>
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
      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">{label}</span>
      <SettingsInput
        autoFocus={autoFocus}
        className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition placeholder:text-[var(--muted)] focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
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
      <span className="mb-1.5 block text-xs font-semibold text-[var(--muted)]">{label}</span>
      <SettingsSelect
        className="h-10 w-full rounded-lg border border-[var(--border)] bg-[var(--surface)] px-3 text-sm text-[var(--foreground)] outline-none transition focus:border-[var(--accent)] focus:ring-2 focus:ring-[color-mix(in_oklab,var(--accent)_28%,transparent)]"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {children}
      </SettingsSelect>
    </label>
  );
}

function AgentCheckbox({
  checked,
  description,
  label,
  onChange,
}: {
  checked: boolean;
  description?: string;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex min-w-0 cursor-pointer items-start justify-between gap-3 rounded-lg px-3 py-2 text-sm text-[var(--foreground)] transition hover:bg-[var(--surface-secondary)]">
      <span className="min-w-0">
        <span className="block truncate font-medium">{label}</span>
        {description ? (
          <span className="mt-1 block text-xs leading-5 text-[var(--muted)]">
            {description}
          </span>
        ) : null}
      </span>
      <SettingsInput
        checked={checked}
        className="mt-0.5 size-4 shrink-0 accent-[var(--accent)]"
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
    allowedExecutionWorkspaceModes: [...AGENT_EXECUTION_WORKSPACE_MODES],
    allowedTools: [],
    canCreateInstances: false,
    canDelegate: false,
    description: "",
    maxInstances: "1",
    maxOutputTokens: "",
    modelId: model?.id ?? "",
    name: "",
    systemPrompt: "",
    thinkingLevel: defaultThinkingLevelForModel(model),
  };
}

function isBuiltinAgentDefinition(id: string) {
  return (
    id === DEFAULT_AGENT_DEFINITION_ID ||
    id === REVIEW_AGENT_DEFINITION_ID ||
    id === IMAGE_AGENT_DEFINITION_ID
  );
}

function modelOutputsText(model: ConfiguredModelSummary) {
  return model.outputModalities.length === 0 || model.outputModalities.includes("text");
}

function agentDefinitionToDraft(
  definition: AgentDefinitionSettings,
): AgentDefinitionDraft {
  return {
    allowedAgentDefinitionIds: definition.permissions.allowedAgentDefinitionIds,
    allowedExecutionWorkspaceModes:
      definition.allowedExecutionWorkspaceModes ?? [...AGENT_EXECUTION_WORKSPACE_MODES],
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
    systemPrompt: definition.systemPrompt,
    thinkingLevel: definition.modelOptions.thinkingLevel ?? "",
  };
}

function draftToAgentDefinitionInput(
  draft: AgentDefinitionDraft,
): AgentDefinitionInput {
  return {
    allowedExecutionWorkspaceModes: draft.allowedExecutionWorkspaceModes,
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
    systemPrompt: draft.systemPrompt.trim(),
  };
}

function normalizeAgentThinkingLevel(
  model: ConfiguredModelSummary | null,
  thinkingLevel: string | null | undefined,
) {
  return isModelThinkingLevelSupported(model, thinkingLevel) ? thinkingLevel : "";
}

function uniqueString<T extends string>(value: T, index: number, values: T[]) {
  return values.indexOf(value) === index;
}
