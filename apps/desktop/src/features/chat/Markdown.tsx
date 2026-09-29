import { useState, type ReactElement, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button } from "@offline-ai/ui";
function Code({
  children,
  className,
}: {
  children?: ReactNode;
  className?: string | undefined;
}): ReactElement {
  const [copied, setCopied] = useState(false);
  const text = typeof children === "string" ? children : "";
  if (!className && !text.includes("\n")) return <code>{children}</code>;
  return (
    <span className="block">
      <span className="mb-2 flex items-center justify-between text-xs text-muted-foreground">
        <span>{className?.replace("language-", "") ?? "Code"}</span>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            void navigator.clipboard.writeText(text).then(
              () => {
                setCopied(true);
              },
              () => {
                setCopied(false);
              },
            );
          }}
        >
          {copied ? "Copied" : "Copy code"}
        </Button>
      </span>
      <code className={className}>{children}</code>
    </span>
  );
}
/** Discard HTML and disable remote resources to keep generated content offline. */
type MarkdownNode = { type: string; value?: string; url?: string; children?: MarkdownNode[] };
export function Markdown({
  text,
  sourcePrefix = "",
  sourceCount = 0,
}: {
  text: string;
  sourcePrefix?: string;
  sourceCount?: number;
}): ReactElement {
  const citationPlugin = () => (tree: MarkdownNode) => {
    const visit = (node: MarkdownNode) => {
      if (!node.children || node.type === "link") return;
      node.children = node.children.flatMap((child) => {
        if (child.type !== "text" || !child.value || !sourcePrefix) {
          visit(child);
          return [child];
        }
        const parts: MarkdownNode[] = [];
        let cursor = 0;
        for (const match of child.value.matchAll(/\[(\d+)\]/g)) {
          const index = Number(match[1]);
          if (index < 1 || index > sourceCount) continue;
          parts.push({ type: "text", value: child.value.slice(cursor, match.index) });
          parts.push({
            type: "link",
            url: `#source-${sourcePrefix}-${String(index)}`,
            children: [{ type: "text", value: match[0] }],
          });
          cursor = match.index + match[0].length;
        }
        parts.push({ type: "text", value: child.value.slice(cursor) });
        return parts;
      });
    };
    visit(tree);
  };
  return (
    <div className="markdown min-w-0 break-words">
      <ReactMarkdown
        remarkPlugins={[remarkGfm, citationPlugin]}
        skipHtml
        urlTransform={(url) => (url.startsWith(`#source-${sourcePrefix}-`) ? url : "")}
        components={{
          code: Code,
          img: ({ alt }) => <span>[Image: {alt ?? "image"}]</span>,
          a: ({ children, href }) =>
            href?.startsWith(`#source-${sourcePrefix}-`) && sourcePrefix ? (
              <button
                type="button"
                className="underline"
                aria-label={`Open source ${href.split("-").at(-1) ?? ""}`}
                onClick={() => {
                  const target = document.getElementById(href.slice(1));
                  if (target instanceof HTMLDetailsElement) {
                    target.open = true;
                    target.querySelector("summary")?.focus();
                    if (typeof target.scrollIntoView === "function")
                      target.scrollIntoView({ block: "nearest" });
                  }
                }}
              >
                {children}
              </button>
            ) : (
              <span className="underline">{children}</span>
            ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
}
