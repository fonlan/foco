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
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";

import type { PluggableList } from "unified";

import { useI18n } from "../../shared/i18n";
import type { MarkdownImageUrlTransform } from "./MarkdownContent";

type MarkdownRendererProps = {
  allowHtml: boolean;
  content: string;
  imageUrlTransform?: MarkdownImageUrlTransform;
};

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

const MARKDOWN_REHYPE_PLUGINS: PluggableList = [rehypeKatex];
const MARKDOWN_SANITIZE_SCHEMA = {
  ...defaultSchema,
  protocols: {
    ...defaultSchema.protocols,
    src: [...(defaultSchema.protocols?.src ?? []), "data"],
  },
};
const MARKDOWN_HTML_REHYPE_PLUGINS: PluggableList = [
  rehypeRaw,
  [rehypeSanitize, MARKDOWN_SANITIZE_SCHEMA],
  rehypeKatex,
];

export const MarkdownRenderer = memo(function MarkdownRenderer({
  allowHtml,
  content,
  imageUrlTransform,
}: MarkdownRendererProps) {
  return (
    <ReactMarkdown
      components={MARKDOWN_COMPONENTS}
      rehypePlugins={allowHtml ? MARKDOWN_HTML_REHYPE_PLUGINS : MARKDOWN_REHYPE_PLUGINS}
      remarkPlugins={[remarkGfm, remarkMath]}
      urlTransform={markdownUrlTransform(imageUrlTransform)}
    >
      {content}
    </ReactMarkdown>
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

function markdownUrlTransform(imageUrlTransform?: MarkdownImageUrlTransform): UrlTransform {
  return (url, key, node) => {
    if (key === "src" && node.tagName === "img" && safeBase64ImageUrl(url)) {
      return url;
    }

    if (key === "src" && node.tagName === "img") {
      return imageUrlTransform?.(url) ?? defaultUrlTransform(url);
    }

    return defaultUrlTransform(url);
  };
}

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

function errorMessage(value: unknown) {
  if (value instanceof Error) {
    return value.message;
  }
  return String(value);
}
