import {
  Children,
  isValidElement,
  memo,
  useEffect,
  useId,
  useRef,
  useState,
  type ReactNode,
} from "react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import type { Components, UrlTransform } from "react-markdown";
import rehypeKatex from "rehype-katex";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";

import { useI18n } from "../../shared/i18n";

type SelectedSkillPrefix = {
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

type MermaidRuntime = {
  initialize: (config: Record<string, unknown>) => void;
  render: (
    id: string,
    definition: string,
  ) => Promise<{
    bindFunctions?: (element: Element) => void;
    svg: string;
  }>;
};

const MERMAID_CONFIG: Record<string, unknown> = {
  flowchart: {
    curve: "basis",
  },
  htmlLabels: false,
  securityLevel: "strict",
  startOnLoad: false,
  theme: "base",
  themeVariables: {
    fontFamily:
      "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif",
    lineColor: "#78716c",
    primaryBorderColor: "#0f766e",
    primaryColor: "#f5f5f4",
    primaryTextColor: "#1c1917",
    secondaryBorderColor: "#a8a29e",
    secondaryColor: "#fafaf9",
    tertiaryColor: "#ccfbf1",
  },
};
let mermaidRuntimePromise: Promise<MermaidRuntime> | null = null;

const MARKDOWN_COMPONENTS: Components = {
  img({ alt, ...props }) {
    return <img alt={alt ?? ""} loading="lazy" {...props} />;
  },
  pre({ children, node: _node, ...props }) {
    const mermaidDefinition = mermaidDefinitionFromPreChildren(children);
    if (mermaidDefinition !== null) {
      return <MermaidDiagram definition={mermaidDefinition} />;
    }

    return <pre {...props}>{children}</pre>;
  },
};

export const MarkdownContent = memo(function MarkdownContent({
  content,
  isError = false,
  isUser,
  renderMode = "full",
  selectedSkillPrefix,
  variant = "message",
}: {
  content: string;
  isError?: boolean;
  isUser: boolean;
  renderMode?: MarkdownRenderMode;
  selectedSkillPrefix: SelectedSkillPrefixResolver;
  variant?: "message" | "reasoning";
}) {
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
        <div className="whitespace-pre-wrap break-words">{markdownContent}</div>
      ) : (
        <ReactMarkdown
          components={MARKDOWN_COMPONENTS}
          rehypePlugins={[rehypeKatex]}
          remarkPlugins={[remarkGfm, remarkMath]}
          urlTransform={markdownUrlTransform}
        >
          {markdownContent}
        </ReactMarkdown>
      ) : null}
    </div>
  );
});

function MermaidDiagram({ definition }: { definition: string }) {
  const { t } = useI18n();
  const reactId = useId();
  const baseRenderId = `foco-mermaid-${reactId.replaceAll(":", "")}`;
  const containerRef = useRef<HTMLDivElement | null>(null);
  const renderCounterRef = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [svg, setSvg] = useState("");

  useEffect(() => {
    let cancelled = false;
    renderCounterRef.current += 1;
    const renderId = `${baseRenderId}-${renderCounterRef.current}`;

    async function renderDiagram() {
      setError(null);
      setSvg("");

      try {
        const mermaid = await loadMermaidRuntime();
        if (cancelled) {
          return;
        }
        const result = await mermaid.render(renderId, definition);
        if (cancelled) {
          return;
        }
        setSvg(result.svg);
        window.setTimeout(() => {
          if (!cancelled && containerRef.current) {
            result.bindFunctions?.(containerRef.current);
          }
        }, 0);
      } catch (renderError) {
        if (!cancelled) {
          setError(errorMessage(renderError));
        }
      }
    }

    void renderDiagram();

    return () => {
      cancelled = true;
    };
  }, [definition, baseRenderId]);

  if (error !== null) {
    return (
      <div className="mermaid-diagram mermaid-diagram-error">
        <div className="mermaid-diagram-error-title">
          {t("Mermaid diagram failed to render.")}
        </div>
        <div className="mermaid-diagram-error-message">{error}</div>
        <pre>
          <code>{definition}</code>
        </pre>
      </div>
    );
  }

  return (
    <div
      aria-label="Mermaid diagram"
      className={`mermaid-diagram ${svg ? "" : "mermaid-diagram-loading"}`}
      dangerouslySetInnerHTML={svg ? { __html: svg } : undefined}
      ref={containerRef}
      role="img"
    />
  );
}

async function loadMermaidRuntime() {
  mermaidRuntimePromise ??= import("mermaid").then((module) => {
    module.default.initialize(MERMAID_CONFIG);
    return module.default;
  });

  return mermaidRuntimePromise;
}

const markdownUrlTransform: UrlTransform = (url, key, node) => {
  if (key === "src" && node.tagName === "img" && safeBase64ImageUrl(url)) {
    return url;
  }

  return defaultUrlTransform(url);
};

function safeBase64ImageUrl(url: string) {
  return /^data:image\/(?:avif|bmp|gif|jpe?g|png|webp);base64,[a-z0-9+/=\s]+$/i.test(
    url,
  );
}

function mermaidDefinitionFromPreChildren(children: ReactNode) {
  const childNodes = Children.toArray(children);
  if (childNodes.length !== 1) {
    return null;
  }

  const child = childNodes[0];
  if (!isValidElement<{ className?: string; children?: ReactNode }>(child)) {
    return null;
  }

  const className = child.props.className ?? "";
  if (!/\blanguage-mermaid\b/i.test(className)) {
    return null;
  }

  const definition = Children.toArray(child.props.children).join("").trim();
  return definition ? definition : null;
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

function errorMessage(value: unknown) {
  if (value instanceof Error) {
    return value.message;
  }
  return String(value);
}
