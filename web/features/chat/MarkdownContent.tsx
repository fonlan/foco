import { lazy, memo, Suspense } from "react";

export type SelectedSkillPrefix = {
  remaining: string;
  skills: Array<{
    name: string;
    path: string;
  }>;
};

export type SelectedSkillPrefixResolver = (
  content: string,
  isUser: boolean,
) => SelectedSkillPrefix | null;

type MarkdownRenderMode = "full" | "streaming";

export type MarkdownImageUrlTransform = (url: string) => string | null;

type MarkdownContentProps = {
  allowHtml?: boolean;
  content: string;
  imageUrlTransform?: MarkdownImageUrlTransform;
  isError?: boolean;
  isUser: boolean;
  renderMode?: MarkdownRenderMode;
  selectedSkillPrefix: SelectedSkillPrefixResolver;
  variant?: "message" | "reasoning";
};

const MarkdownRenderer = lazy(() =>
  import("./MarkdownRenderer").then((module) => ({
    default: module.MarkdownRenderer,
  })),
);

export const MarkdownContent = memo(function MarkdownContent({
  allowHtml = false,
  content,
  imageUrlTransform,
  isError = false,
  isUser,
  renderMode = "full",
  selectedSkillPrefix,
  variant = "message",
}: MarkdownContentProps) {
  const skillPrefix = selectedSkillPrefix(content, isUser);
  const displayContent = skillPrefix?.remaining ?? content;
  const markdownContent =
    renderMode === "streaming"
      ? displayContent
      : deferIncompleteMermaidBlocks(displayContent);

  return (
    <div
      className={`markdown-content min-w-0 break-words text-sm leading-6 ${
        isUser ? "markdown-content-user" : "markdown-content-assistant"
      } ${variant === "reasoning" ? "markdown-content-reasoning" : ""} ${
        isError ? "text-rose-700" : ""
      }`}
    >
      {skillPrefix ? (
        <div className="message-skill-chip-row">
          {skillPrefix.skills.map((skill) => (
            <span
              aria-label={skill.path}
              className="message-skill-chip"
              key={`${skill.name}-${skill.path}`}
              title={skill.path}
            >
              {skill.name}
            </span>
          ))}
        </div>
      ) : null}
      {markdownContent ? renderMode === "streaming" ? (
        // ponytail: plain streaming tail skips plugin churn; use tail-only markdown parsing if this becomes too limited.
        <PlainMarkdownText content={markdownContent} />
      ) : (
        <Suspense
          // ponytail: full markdown loads after the chat shell; if the plain-text flash becomes noisy, preload after messages settle.
          fallback={<PlainMarkdownText content={markdownContent} />}
        >
          <MarkdownRenderer
            allowHtml={allowHtml}
            content={markdownContent}
            imageUrlTransform={imageUrlTransform}
          />
        </Suspense>
      ) : null}
    </div>
  );
});

function PlainMarkdownText({ content }: { content: string }) {
  return <div className="whitespace-pre-wrap break-words">{content}</div>;
}

function deferIncompleteMermaidBlocks(content: string) {
  const lines = content.match(/[^\r\n]*(?:\r\n|\n|\r|$)/g) ?? [];
  const nonEmptyLines = lines.filter((line) => line.length > 0);
  if (nonEmptyLines.length === 0) {
    return content;
  }

  let activeFence: MarkdownFence | null = null;
  for (let index = 0; index < nonEmptyLines.length; index += 1) {
    const line = nonEmptyLines[index];

    if (activeFence !== null) {
      if (isFenceClosingLine(line, activeFence)) {
        activeFence = null;
      }
      continue;
    }

    const fence = parseMarkdownFence(line);
    if (fence !== null) {
      activeFence = {
        ...fence,
        lineIndex: index,
      };
    }
  }

  if (activeFence?.language !== "mermaid") {
    return content;
  }

  const nextLines = [...nonEmptyLines];
  nextLines[activeFence.lineIndex] = neutralizeMermaidFenceLine(
    nextLines[activeFence.lineIndex],
  );
  return nextLines.join("");
}

type MarkdownFence = {
  char: "`" | "~";
  length: number;
  language: string | null;
  lineIndex: number;
};

function parseMarkdownFence(line: string) {
  const body = line.replace(/(?:\r\n|\n|\r)$/, "");
  const match = /^([ \t]{0,3})(`{3,}|~{3,})([^\r\n]*)$/.exec(body);
  if (!match) {
    return null;
  }

  const marker = match[2];
  const language = match[3].trim().split(/\s+/, 1)[0]?.toLowerCase() || null;
  return {
    char: marker[0] as "`" | "~",
    length: marker.length,
    language,
    lineIndex: -1,
  };
}

function isFenceClosingLine(line: string, fence: MarkdownFence) {
  const body = line.replace(/(?:\r\n|\n|\r)$/, "");
  const escapedChar = fence.char === "`" ? "`" : "~";
  return new RegExp(`^[ \t]{0,3}${escapedChar}{${fence.length},}[ \t]*$`).test(
    body,
  );
}

function neutralizeMermaidFenceLine(line: string) {
  const lineEnding = line.match(/(?:\r\n|\n|\r)$/)?.[0] ?? "";
  const body = line.slice(0, line.length - lineEnding.length);
  const match = /^([ \t]{0,3})(`{3,}|~{3,})([^\r\n]*)$/.exec(body);
  if (!match) {
    return line;
  }

  return `${match[1]}${match[2]}text${lineEnding}`;
}
