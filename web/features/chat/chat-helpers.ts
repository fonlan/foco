import type {
  AppLanguageId,
  ChatAttachmentPartSummary,
  ChatAttachmentPayload,
  ChatMessagePart,
  ComposerAttachment,
  ConfiguredModelSummary,
  ConfiguredSkillSummary,
  NativeSelectedFile,
  Translate,
} from "../../api/types";
import type { SelectedSkillPrefix } from "./MarkdownContent";

type ToolDisplayLabels = {
  en: string;
  "zh-CN": string;
};

const TOOL_DISPLAY_NAMES: Record<string, ToolDisplayLabels> = {
  agent_cancel_task: { en: "Cancel Task", "zh-CN": "取消任务" },
  agent_create_instances: { en: "Create Agents", "zh-CN": "创建智能体" },
  agent_delegate_task: { en: "Delegate Task", "zh-CN": "委派任务" },
  agent_get_task: { en: "Get Task", "zh-CN": "获取任务" },
  agent_list: { en: "List Agents", "zh-CN": "列出智能体" },
  agent_send_message: { en: "Send Message", "zh-CN": "发送消息" },
  agent_transfer_task: { en: "Transfer Task", "zh-CN": "转交任务" },
  agent_wait_tasks: { en: "Wait Tasks", "zh-CN": "等待任务" },
  ask_question: { en: "Ask", "zh-CN": "询问" },
  apply_patch: { en: "Patch", "zh-CN": "修补" },
  create_plan: { en: "Create Plan", "zh-CN": "创建计划" },
  create_todo_graph: { en: "Create Todos", "zh-CN": "创建待办" },
  update_todo_graph: { en: "Update Todos", "zh-CN": "更新待办" },
  delete_plan: { en: "Delete Plan", "zh-CN": "删除计划" },
  edit_file: { en: "Edit", "zh-CN": "编辑" },
  find_files: { en: "Glob", "zh-CN": "查找" },
  get_plans: { en: "Get Plans", "zh-CN": "获取计划" },
  get_todo_graph: { en: "Todos", "zh-CN": "获取待办" },
  git_branch: { en: "Branch", "zh-CN": "分支" },
  git_status: { en: "Git", "zh-CN": "Git" },
  graph_explore: { en: "Explore", "zh-CN": "代码探索" },
  graph_find_callees: { en: "Callees", "zh-CN": "查找被调" },
  graph_find_callers: { en: "Callers", "zh-CN": "查找调用" },
  graph_find_children: { en: "Children", "zh-CN": "查找成员" },
  graph_find_references: { en: "References", "zh-CN": "查找引用" },
  graph_find_symbols: { en: "Symbols", "zh-CN": "查找符号" },
  graph_related_files: { en: "Related", "zh-CN": "相关文件" },
  image_gen: { en: "ImageGen", "zh-CN": "生图" },
  memory_search: { en: "Memory Search", "zh-CN": "搜索记忆" },
  memory_write: { en: "Memory Write", "zh-CN": "写入记忆" },
  "mcp__context7__query-docs": {
    en: "Query Documentation",
    "zh-CN": "查询文档",
  },
  "mcp__context7__resolve-library-id": {
    en: "Resolve Library ID",
    "zh-CN": "解析库 ID",
  },
  read_file: { en: "Read", "zh-CN": "读取" },
  read_spec: { en: "Read Spec", "zh-CN": "读取 Spec" },
  get_command_output: { en: "Command Output", "zh-CN": "命令输出" },
  stop_command: { en: "Stop Command", "zh-CN": "停止命令" },
  run_command: { en: "Run", "zh-CN": "运行" },
  search_text: { en: "Grep", "zh-CN": "搜索" },
  sleep: { en: "Sleep", "zh-CN": "等待" },
  update_plan: { en: "Update Plan", "zh-CN": "更新计划" },
  update_plan_step: { en: "Update Step", "zh-CN": "更新步骤" },
  update_spec: { en: "Update Spec", "zh-CN": "更新 Spec" },
  web_fetch: { en: "Fetch", "zh-CN": "Web获取" },
  web_search: { en: "Web Search", "zh-CN": "Web搜索" },
  write_file: { en: "Write", "zh-CN": "写入" },
};

export function toolDisplayName(toolName: string, language: AppLanguageId) {
  const known = TOOL_DISPLAY_NAMES[toolName];
  return known ? known[language] : toolName;
}

export function isSkillAvailableForWorkspace(
  skill: ConfiguredSkillSummary,
  workspaceId: string | null,
) {
  return skill.enabled && (skill.scope !== "workspace" || skill.workspaceId === workspaceId);
}

export function activeSkillQuery(value: string) {
  const match = /(^|\s)\/([^\s/]*)$/.exec(value);
  return match ? match[2] : null;
}

export function removeActiveSkillToken(value: string) {
  return value.replace(/(^|\s)\/[^\s/]*$/, (_match, prefix: string) => prefix);
}

export function selectedSkillPrefix(
  content: string,
  isUser: boolean,
): SelectedSkillPrefix | null {
  if (!isUser) {
    return null;
  }

  const blockPrefix = selectedSkillBlockPrefix(content);
  if (blockPrefix) {
    return blockPrefix;
  }

  let remaining = content.trimStart();
  const skills: Array<{ name: string; path: string }> = [];

  while (true) {
    const match = /^\[\$([^\]\n]+)\]\(([^)\n]+)\)(?:\s+|$)/.exec(remaining);
    if (!match) {
      break;
    }

    const path = decodeMarkdownHref(match[2].trim());
    if (!path.replaceAll("\\", "/").endsWith("SKILL.md")) {
      break;
    }

    skills.push({
      name: match[1].trim(),
      path,
    });
    remaining = remaining.slice(match[0].length);
  }

  if (!skills.length) {
    return null;
  }

  return {
    remaining,
    skills,
  };
}

export function messageWithSelectedSkills(
  skills: ConfiguredSkillSummary[],
  skillIds: string[] | null | undefined,
  message: string,
) {
  const links = (Array.isArray(skillIds) ? skillIds : [])
    .filter((skillId): skillId is string => typeof skillId === "string")
    .map((skillId) => skills.find((skill) => skill.key === skillId))
    .filter((skill): skill is ConfiguredSkillSummary => Boolean(skill))
    .map((skill) => `[$${skill.name}](${skill.path})`);

  return links.length ? `${links.join(" ")} ${message}` : message;
}

export async function fileToBase64(file: File): Promise<string> {
  return arrayBufferToBase64(await file.arrayBuffer());
}

export async function fileToComposerAttachment(file: File): Promise<ComposerAttachment> {
  const name = file.name.trim();
  const contentType = fileContentType(file);

  if (!name) {
    throw new Error("attachment name must not be empty");
  }

  if (!contentType) {
    throw new Error(`attachment ${name} content type is missing`);
  }

  const contentBase64 = arrayBufferToBase64(await file.arrayBuffer());
  const previewDataUrl = contentType.startsWith("image/")
    ? `data:${contentType};base64,${contentBase64}`
    : null;

  return {
    id: localChatAttachmentId(),
    name,
    contentType,
    contentBase64,
    path: undefined,
    previewDataUrl,
    sizeBytes: file.size,
  };
}

export function composerAttachmentFromSelectedFile(
  file: NativeSelectedFile,
): ComposerAttachment {
  const name = file.name.trim();
  const contentType = file.contentType.trim();
  if (!name) {
    throw new Error("attachment name must not be empty");
  }
  if (!contentType) {
    throw new Error(`attachment ${name} content type is missing`);
  }
  const contentBase64 = file.contentBase64 ?? undefined;
  return {
    id: localChatAttachmentId(),
    name,
    contentType,
    contentBase64,
    path: file.path,
    previewDataUrl: contentType.startsWith("image/") && contentBase64
      ? `data:${contentType};base64,${contentBase64}`
      : null,
    sizeBytes: file.sizeBytes,
  };
}

export function chatAttachmentPayload(
  attachment: ComposerAttachment,
): ChatAttachmentPayload {
  const payload: ChatAttachmentPayload = {
    id: attachment.id,
    name: attachment.name,
    contentType: attachment.contentType,
    sizeBytes: attachment.sizeBytes,
  };
  if (attachment.contentBase64) {
    payload.contentBase64 = attachment.contentBase64;
  }
  if (attachment.path) {
    payload.path = attachment.path;
  }

  return payload;
}

export function userMessageParts(
  content: string,
  attachments: ChatAttachmentPayload[],
): ChatMessagePart[] {
  const parts: ChatMessagePart[] = [];
  if (content) {
    parts.push({ type: "text", text: content });
  }
  parts.push(
    ...attachments.map((attachment) => ({
      type: "attachment" as const,
      attachment: attachmentPartFromPayload(attachment),
    })),
  );
  return parts;
}

export function formatFileSize(sizeBytes: number) {
  const units = ["B", "KB", "MB", "GB"];
  let value = sizeBytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const formatted =
    unitIndex === 0 || value >= 10 ? value.toFixed(0) : value.toFixed(1);
  return `${formatted} ${units[unitIndex]}`;
}

export function skillScopeLabel(skill: ConfiguredSkillSummary, t: Translate) {
  if (skill.scope === "global") {
    return t("Global skill");
  }

  return skill.workspaceName
    ? t("Workspace skill {name}", { name: skill.workspaceName })
    : t("Workspace skill");
}

export function unsupportedAttachmentMessage(
  model: ConfiguredModelSummary | null,
  attachment: Pick<ComposerAttachment, "contentType" | "name">,
  t: Translate,
) {
  const modality = unsupportedAttachmentInputModality(model, attachment.contentType);
  if (!modality) {
    return null;
  }
  return t("Selected model does not support {type} attachments: {name}", {
    name: attachment.name,
    type: ATTACHMENT_INPUT_MODALITY_LABELS[modality] ?? modality,
  });
}

export function unsupportedFileAttachmentMessage(
  model: ConfiguredModelSummary | null,
  file: File,
  t: Translate,
) {
  const contentType = fileContentType(file);
  if (!contentType) {
    return null;
  }
  return unsupportedAttachmentMessage(
    model,
    { contentType, name: file.name.trim() || file.name },
    t,
  );
}

function selectedSkillBlockPrefix(content: string): SelectedSkillPrefix | null {
  const remaining = content.trimStart();
  const markdownPrefix = selectedSkillMarkdownPrefix(remaining);
  if (markdownPrefix) {
    return markdownPrefix;
  }

  return selectedSkillXmlPrefix(remaining);
}

function selectedSkillMarkdownPrefix(remaining: string): SelectedSkillPrefix | null {
  if (!remaining.startsWith("# Selected Skills")) {
    return null;
  }

  const closingMarker = "\n## End Selected Skills\n";
  const endIndex = remaining.lastIndexOf(closingMarker);
  if (endIndex < 0) {
    return null;
  }

  const block = remaining.slice(0, endIndex);
  const metadata = /```json\s*\n([\s\S]*?)\n```/.exec(block)?.[1];
  if (!metadata) {
    return null;
  }

  try {
    const parsed: unknown = JSON.parse(metadata);
    if (!Array.isArray(parsed)) {
      return null;
    }
    const skills = parsed.filter(
      (skill): skill is { name: string; path: string } =>
        Boolean(
          skill &&
            typeof skill === "object" &&
            "name" in skill &&
            typeof skill.name === "string" &&
            skill.name &&
            "path" in skill &&
            typeof skill.path === "string" &&
            skill.path,
        ),
    );
    if (!skills.length) {
      return null;
    }

    return {
      remaining: remaining.slice(endIndex + closingMarker.length).trimStart(),
      skills,
    };
  } catch {
    return null;
  }
}

// Keep parsing the old prefix so existing chat history still renders cleanly.
function selectedSkillXmlPrefix(remaining: string): SelectedSkillPrefix | null {
  if (!remaining.startsWith("<selected_skills>")) {
    return null;
  }

  const closingTag = "</selected_skills>";
  const endIndex = remaining.indexOf(closingTag);
  if (endIndex < 0) {
    return null;
  }

  const block = remaining.slice(0, endIndex + closingTag.length);
  const skills = [...block.matchAll(/<skill\b([^>]*)>/g)]
    .map((match) => {
      const name = /(?:^|\s)name="([^"]*)"/.exec(match[1])?.[1];
      const path = /(?:^|\s)path="([^"]*)"/.exec(match[1])?.[1];

      return name && path
        ? { name: decodeXmlAttribute(name), path: decodeXmlAttribute(path) }
        : null;
    })
    .filter((skill): skill is { name: string; path: string } => Boolean(skill));

  if (!skills.length) {
    return null;
  }

  return {
    remaining: remaining.slice(endIndex + closingTag.length).trimStart(),
    skills,
  };
}

function decodeXmlAttribute(value: string) {
  return value
    .replaceAll("&quot;", '"')
    .replaceAll("&apos;", "'")
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&amp;", "&");
}

function decodeMarkdownHref(value: string) {
  try {
    return decodeURI(value);
  } catch {
    return value;
  }
}

function fileContentType(file: File) {
  const explicitType = file.type.trim();
  if (explicitType) {
    return explicitType;
  }

  const extension = file.name.trim().toLowerCase().split(".").pop() ?? "";
  const extensionTypes: Record<string, string> = {
    bat: "text/plain",
    c: "text/plain",
    cmd: "text/plain",
    cpp: "text/plain",
    cs: "text/plain",
    css: "text/css",
    csv: "text/csv",
    go: "text/plain",
    h: "text/plain",
    hpp: "text/plain",
    htm: "text/html",
    html: "text/html",
    java: "text/plain",
    js: "text/javascript",
    json: "application/json",
    jsx: "text/javascript",
    m4a: "audio/mp4",
    md: "text/markdown",
    mkv: "video/x-matroska",
    mov: "video/quicktime",
    mp3: "audio/mpeg",
    mp4: "video/mp4",
    ogg: "audio/ogg",
    pdf: "application/pdf",
    ps1: "text/plain",
    py: "text/x-python",
    rs: "text/plain",
    sh: "text/x-shellscript",
    toml: "application/toml",
    ts: "text/typescript",
    tsx: "text/typescript",
    txt: "text/plain",
    wav: "audio/wav",
    webm: "video/webm",
    xml: "application/xml",
    yaml: "application/yaml",
    yml: "application/yaml",
  };

  return extensionTypes[extension] ?? "";
}

function localChatAttachmentId() {
  return localRandomId("attachment");
}

function localRandomId(fallbackPrefix?: string) {
  const randomUUID = globalThis.crypto?.randomUUID;
  if (randomUUID) {
    return randomUUID.call(globalThis.crypto);
  }

  // ponytail: fallback is for local attachment ids only; secure tokens use App's required UUID path.
  const suffix = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return fallbackPrefix ? `${fallbackPrefix}-${suffix}` : suffix;
}

function arrayBufferToBase64(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer);
  const chunkSize = 0x8000;
  let binary = "";

  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
  }

  return btoa(binary);
}

function attachmentPartFromPayload(
  attachment: ChatAttachmentPayload,
): ChatAttachmentPartSummary {
  return {
    id: attachment.id,
    name: attachment.name,
    contentType: attachment.contentType,
    path: attachment.path ?? null,
    previewDataUrl: attachment.contentType.startsWith("image/") &&
      attachment.contentBase64
      ? `data:${attachment.contentType};base64,${attachment.contentBase64}`
      : null,
    sizeBytes: attachment.sizeBytes,
  };
}

const ATTACHMENT_INPUT_MODALITY_LABELS: Record<string, string> = {
  audio: "audio",
  image: "image",
  pdf: "PDF",
  video: "video",
};

function attachmentInputModality(contentType: string) {
  const normalized = contentType.trim().toLowerCase().split(";")[0]?.trim() ?? "";
  if (normalized.startsWith("image/")) {
    return "image";
  }
  if (normalized.startsWith("audio/")) {
    return "audio";
  }
  if (normalized.startsWith("video/")) {
    return "video";
  }
  if (normalized === "application/pdf") {
    return "pdf";
  }
  return null;
}

export function unsupportedAttachmentInputModality(
  model: ConfiguredModelSummary | null,
  contentType: string,
) {
  const modality = attachmentInputModality(contentType);
  if (!modality) {
    return null;
  }
  return model?.inputModalities.includes(modality) ? null : modality;
}
