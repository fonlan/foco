import {
  Children,
  isValidElement,
  memo,
  useEffect,
  useId,
  useRef,
  useState,
  type ComponentPropsWithoutRef,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";

import { Check, Copy } from "lucide-react";
import ReactMarkdown, { defaultUrlTransform } from "react-markdown";
import type { Components, UrlTransform } from "react-markdown";
import rehypeKatex from "rehype-katex";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import remarkGfm from "remark-gfm";
import remarkMath from "remark-math";

import type { PluggableList } from "unified";

import { useI18n } from "../../shared/i18n";
import { Button } from "../../shared/ui";
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
};

const MERMAID_THEME_COLORS = {
  lineColor: ["--border", "rgba(0, 0, 0, 0)"],
  primaryBorderColor: ["--accent", "#3b82f6"],
  primaryColor: ["--surface", "#ffffff"],
  primaryTextColor: ["--foreground", "#343438"],
  secondaryBorderColor: ["--border-secondary", "#e4e4e7"],
  secondaryColor: ["--surface-secondary", "#f8f8f8"],
  tertiaryColor: ["--accent-soft", "#e8f0ff"],
} as const;

export function mermaidThemeVariables() {
  return {
    fontFamily:
      "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif",
    ...Object.fromEntries(
      Object.entries(MERMAID_THEME_COLORS).map(([key, [token, fallback]]) => [
        key,
        resolveMermaidColor(token, fallback),
      ]),
    ),
  };
}

function resolveMermaidColor(token: string, fallback: string) {
  if (typeof document === "undefined") {
    return fallback;
  }

  const probe = document.createElement("span");
  probe.style.color = `var(${token})`;
  document.body.append(probe);
  const resolved = getComputedStyle(probe).color;
  probe.remove();

  return mermaidCompatibleColor(resolved) ?? fallback;
}

export function mermaidCompatibleColor(value: string) {
  const color = value.trim();
  if (/^(?:#[0-9a-f]{3,8}|rgba?\()/i.test(color)) {
    return color;
  }

  return oklchToRgba(color);
}

function oklchToRgba(value: string) {
  const match = value.match(
    /^oklch\(\s*([+-]?(?:\d*\.)?\d+%?)\s+([+-]?(?:\d*\.)?\d+)\s+([+-]?(?:\d*\.)?\d+)(?:deg)?(?:\s*\/\s*([+-]?(?:\d*\.)?\d+%?))?\s*\)$/i,
  );
  if (!match) {
    return null;
  }

  const [lightnessValue, chromaValue, hueValue, alphaValue] = match.slice(1);
  const lightness = parseCssNumber(lightnessValue, 1);
  const chroma = Number(chromaValue);
  const hue = Number(hueValue);
  const alpha = alphaValue ? parseCssNumber(alphaValue, 1) : 1;
  if (![lightness, chroma, hue, alpha].every(Number.isFinite)) {
    return null;
  }

  const hueRadians = (hue * Math.PI) / 180;
  const a = chroma * Math.cos(hueRadians);
  const b = chroma * Math.sin(hueRadians);
  const l = (lightness + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (lightness - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (lightness - 0.0894841775 * a - 1.291485548 * b) ** 3;
  const red = linearToSrgb(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s);
  const green = linearToSrgb(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s);
  const blue = linearToSrgb(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s);

  const channels = [red, green, blue].map((channel) => Math.round(clamp(channel, 0, 1) * 255));
  const normalizedAlpha = clamp(alpha, 0, 1);
  return normalizedAlpha === 1
    ? `rgb(${channels.join(", ")})`
    : `rgba(${channels.join(", ")}, ${normalizedAlpha})`;
}

function parseCssNumber(value: string, percentageScale: number) {
  return value.endsWith("%")
    ? Number(value.slice(0, -1)) / 100 * percentageScale
    : Number(value);
}

function linearToSrgb(value: number) {
  return value <= 0.0031308
    ? value * 12.92
    : 1.055 * value ** (1 / 2.4) - 0.055;
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}

function mermaidConfig() {
  return {
    ...MERMAID_CONFIG,
    themeVariables: mermaidThemeVariables(),
  };
}

let mermaidRuntimePromise: Promise<MermaidRuntime> | null = null;

const MARKDOWN_COMPONENTS: Components = {
  a: MarkdownAnchor,
  img({ alt, ...props }) {
    return <img alt={alt ?? ""} loading="lazy" {...props} />;
  },
  pre({ children, node: _node, ...props }) {
    const mermaidDefinition = mermaidDefinitionFromPreChildren(children);
    if (mermaidDefinition !== null) {
      return <MermaidDiagram definition={mermaidDefinition} />;
    }

    return <CodeBlock preProps={props}>{children}</CodeBlock>;
  },
};

type MarkdownAnchorProps = ComponentPropsWithoutRef<"a"> & {
  node?: unknown;
};

/**
 * Shared markdown anchor: force a safe new-tab target, and for plain primary
 * activation explicitly open via window.open so list gesture handlers cannot
 * leave the link inert. Modifier / non-primary clicks keep native semantics.
 */
function MarkdownAnchor({
  children,
  href,
  node: _node,
  onClick,
  ...props
}: MarkdownAnchorProps) {
  return (
    <a
      {...props}
      href={href}
      onClick={(event) => handleMarkdownAnchorClick(event, href, onClick)}
      rel="noopener noreferrer"
      target="_blank"
    >
      {children}
    </a>
  );
}

/** Primary-activation handler for markdown links; safe to unit-test in isolation. */
export function handleMarkdownAnchorClick(
  event: ReactMouseEvent<HTMLAnchorElement>,
  href: string | undefined,
  onClick?: ComponentPropsWithoutRef<"a">["onClick"],
) {
  onClick?.(event);
  if (event.defaultPrevented || !href) {
    return;
  }

  // Let the browser handle modified / non-primary activation (new tab, download, etc.).
  if (
    event.button !== 0 ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey
  ) {
    return;
  }

  event.preventDefault();
  window.open(href, "_blank", "noopener,noreferrer");
}

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

function CodeBlock({
  children,
  preProps,
}: {
  children: ReactNode;
  preProps: ComponentPropsWithoutRef<"pre">;
}) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);
  const resetCopiedTimerRef = useRef<number | null>(null);
  const label = copied ? t("Copied code") : t("Copy code");
  const Icon = copied ? Check : Copy;

  useEffect(() => {
    return () => {
      if (resetCopiedTimerRef.current !== null) {
        window.clearTimeout(resetCopiedTimerRef.current);
      }
    };
  }, []);

  async function copyCode() {
    await navigator.clipboard.writeText(codeTextFromPreChildren(children));
    setCopied(true);
    if (resetCopiedTimerRef.current !== null) {
      window.clearTimeout(resetCopiedTimerRef.current);
    }
    resetCopiedTimerRef.current = window.setTimeout(() => {
      setCopied(false);
      resetCopiedTimerRef.current = null;
    }, 1400);
  }

  return (
    <div className="markdown-code-block">
      <Button
        aria-label={label}
        className="markdown-code-copy-button"
        onPress={() => void copyCode()}
      >
        <Icon aria-hidden="true" size={14} />
      </Button>
      <pre {...preProps}>{children}</pre>
    </div>
  );
}

function MermaidDiagram({ definition }: { definition: string }) {
  const { t } = useI18n();
  const reactId = useId();
  const baseRenderId = `foco-mermaid-${reactId.replaceAll(":", "")}`;
  const containerRef = useRef<HTMLDivElement | null>(null);
  const renderCounterRef = useRef(0);
  const [error, setError] = useState<string | null>(null);
  const [svg, setSvg] = useState("");
  const [themeRevision, setThemeRevision] = useState(0);

  useEffect(() => {
    const root = document.documentElement;
    const observer = new MutationObserver(() => {
      setThemeRevision((current) => current + 1);
    });

    observer.observe(root, {
      attributeFilter: ["class", "data-theme"],
      attributes: true,
    });

    return () => observer.disconnect();
  }, []);

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
  }, [definition, baseRenderId, themeRevision]);

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
  mermaidRuntimePromise ??= import("mermaid").then((module) => module.default);
  const runtime = await mermaidRuntimePromise;
  runtime.initialize(mermaidConfig());
  return runtime;
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

function codeTextFromPreChildren(children: ReactNode) {
  const childNodes = Children.toArray(children);
  if (childNodes.length === 1) {
    const child = childNodes[0];
    if (isValidElement<{ children?: ReactNode }>(child)) {
      return Children.toArray(child.props.children).join("");
    }
  }

  return childNodes.join("");
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
