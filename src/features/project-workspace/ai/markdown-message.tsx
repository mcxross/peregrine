import React from "react";

import { cn } from "@/lib/utils";

type MarkdownMessageProps = {
  className?: string;
  content: string;
};

type MarkdownBlock =
  | {
      code: string;
      language: string;
      type: "code";
    }
  | {
      level: number;
      text: string;
      type: "heading";
    }
  | {
      items: string[];
      type: "ordered-list" | "unordered-list";
    }
  | {
      text: string;
      type: "paragraph";
    };

export function MarkdownMessage({ className, content }: MarkdownMessageProps) {
  return (
    <div className={cn("space-y-2 break-words", className)}>
      {parseMarkdownBlocks(content).map((block, index) => (
        <MarkdownBlockView block={block} key={`${block.type}-${index}`} />
      ))}
    </div>
  );
}

function MarkdownBlockView({ block }: { block: MarkdownBlock }) {
  switch (block.type) {
    case "code":
      return (
        <pre className="ai-liquid-code-block overflow-auto rounded-[10px] p-2 text-[11px] leading-5 text-muted-foreground">
          <code>{block.code}</code>
        </pre>
      );
    case "heading":
      return (
        <h3
          className={cn(
            "font-semibold leading-5 text-foreground",
            block.level <= 2 ? "text-sm" : "text-[13px]",
          )}
        >
          {renderInlineMarkdown(block.text)}
        </h3>
      );
    case "ordered-list":
      return (
        <ol className="list-decimal space-y-1 pl-4 text-[12px] leading-5 text-foreground">
          {block.items.map((item, index) => (
            <li key={`${item}-${index}`}>{renderInlineMarkdown(item)}</li>
          ))}
        </ol>
      );
    case "unordered-list":
      return (
        <ul className="list-disc space-y-1 pl-4 text-[12px] leading-5 text-foreground">
          {block.items.map((item, index) => (
            <li key={`${item}-${index}`}>{renderInlineMarkdown(item)}</li>
          ))}
        </ul>
      );
    case "paragraph":
      return (
        <p className="whitespace-pre-wrap text-[12px] leading-5 text-foreground">
          {renderInlineMarkdown(block.text)}
        </p>
      );
  }
}

function parseMarkdownBlocks(content: string) {
  const lines = content.replace(/\r\n/g, "\n").split("\n");
  const blocks: MarkdownBlock[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index] ?? "";

    if (!line.trim()) {
      index += 1;
      continue;
    }

    const codeFence = line.match(/^```(\w+)?\s*$/);
    if (codeFence) {
      const codeLines: string[] = [];
      index += 1;

      while (index < lines.length && !/^```\s*$/.test(lines[index] ?? "")) {
        codeLines.push(lines[index] ?? "");
        index += 1;
      }

      blocks.push({
        code: codeLines.join("\n"),
        language: codeFence[1] ?? "",
        type: "code",
      });
      index += 1;
      continue;
    }

    const heading = line.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      blocks.push({
        level: heading[1].length,
        text: heading[2].trim(),
        type: "heading",
      });
      index += 1;
      continue;
    }

    const unorderedItem = parseUnorderedListItem(line);
    if (unorderedItem) {
      const items: string[] = [];

      while (index < lines.length) {
        const item = parseUnorderedListItem(lines[index] ?? "");
        if (!item) {
          break;
        }

        items.push(item);
        index += 1;
      }

      blocks.push({ items, type: "unordered-list" });
      continue;
    }

    const orderedItem = parseOrderedListItem(line);
    if (orderedItem) {
      const items: string[] = [];

      while (index < lines.length) {
        const item = parseOrderedListItem(lines[index] ?? "");
        if (!item) {
          break;
        }

        items.push(item);
        index += 1;
      }

      blocks.push({ items, type: "ordered-list" });
      continue;
    }

    const paragraphLines: string[] = [];

    while (index < lines.length && shouldContinueParagraph(lines[index] ?? "")) {
      paragraphLines.push((lines[index] ?? "").trim());
      index += 1;
    }

    blocks.push({
      text: paragraphLines.join(" "),
      type: "paragraph",
    });
  }

  return blocks;
}

function shouldContinueParagraph(line: string) {
  return (
    Boolean(line.trim()) &&
    !/^```/.test(line) &&
    !/^(#{1,4})\s+/.test(line) &&
    !parseUnorderedListItem(line) &&
    !parseOrderedListItem(line)
  );
}

function parseUnorderedListItem(line: string) {
  return line.match(/^\s*[-*]\s+(.+)$/)?.[1]?.trim() ?? null;
}

function parseOrderedListItem(line: string) {
  return line.match(/^\s*\d+[.)]\s+(.+)$/)?.[1]?.trim() ?? null;
}

function renderInlineMarkdown(text: string): React.ReactNode[] {
  const nodes: React.ReactNode[] = [];
  const tokenPattern = /(`[^`]+`|\*\*[^*]+\*\*)/g;
  let cursor = 0;

  for (const match of text.matchAll(tokenPattern)) {
    const token = match[0];
    const index = match.index ?? 0;

    if (index > cursor) {
      nodes.push(text.slice(cursor, index));
    }

    if (token.startsWith("`")) {
      nodes.push(
        <code
          className="ai-liquid-inline-code rounded px-1 py-0.5 font-mono text-[0.92em] text-foreground"
          key={`code-${index}`}
        >
          {token.slice(1, -1)}
        </code>,
      );
    } else {
      nodes.push(
        <strong className="font-semibold text-foreground" key={`strong-${index}`}>
          {renderInlineMarkdown(token.slice(2, -2))}
        </strong>,
      );
    }

    cursor = index + token.length;
  }

  if (cursor < text.length) {
    nodes.push(text.slice(cursor));
  }

  return nodes;
}
