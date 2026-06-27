import type {
  ChatAttachmentPartSummary,
  ChatAttachmentPayload,
  ChatMessagePart,
  ComposerAttachment,
  ConfiguredModelSummary,
  ConfiguredSkillSummary,
  Translate,
} from "../../api/types";
import type { SelectedSkillPrefix } from "./MarkdownContent";

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
  skillIds: string[],
  message: string,
) {
  const links = skillIds
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
