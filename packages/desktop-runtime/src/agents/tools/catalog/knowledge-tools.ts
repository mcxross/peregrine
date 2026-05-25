import type { DeterministicToolSpec } from "@peregrine/agent-runtime";

import { readOnlyAction } from "../actions";
import { defineAgentTool } from "../define-tool";
import { toolFailure, toolSuccess } from "../executors";

const DOC_LOADERS = import.meta.glob<string>(
  "/knowledge/sui-move/source/**/*.{md,mdx,move,toml,yml,yaml,json,txt}",
  {
    import: "default",
    query: "?raw",
  },
);

type SuiMoveCorpus =
  | "all"
  | "move-book"
  | "sui-docs"
  | "sui-prover"
  | "review-guidance";

type KnowledgeDoc = {
  content: string;
  corpus: Exclude<SuiMoveCorpus, "all">;
  path: string;
  title: string;
};

let docCache: Promise<KnowledgeDoc[]> | null = null;

export function createKnowledgeTools(): DeterministicToolSpec[] {
  return [
    defineAgentTool<
      { corpus?: SuiMoveCorpus; limit?: number; query: string },
      { results: Array<Omit<KnowledgeDoc, "content"> & { excerpt: string; score: number }> }
    >({
      id: "rust.knowledge.sui_move.search",
      title: "Search Sui Move knowledge",
      description:
        "Search Peregrine's vendored Sui/Move documentation snapshot for exact language, framework, prover, and security-review guidance.",
      inputSchema: {
        type: "object",
        properties: {
          query: {
            type: "string",
            description: "Search query, such as public fun entry visibility or dynamic field cleanup.",
          },
          corpus: {
            type: "string",
            enum: ["all", "move-book", "sui-docs", "sui-prover", "review-guidance"],
            description: "Optional documentation corpus to search.",
          },
          limit: {
            type: "number",
            description: "Maximum number of results to return. Defaults to 6 and is capped at 12.",
          },
        },
        required: ["query"],
        additionalProperties: false,
      },
      action: readOnlyAction("Search the local vendored Sui/Move knowledge base."),
      execute: async (input) => {
        const query = input.query.trim();
        if (!query) {
          return toolFailure("Sui/Move knowledge search requires a non-empty query.");
        }

        const docs = await loadKnowledgeDocs();
        const corpus = input.corpus ?? "all";
        const tokens = tokenize(query);
        const limit = Math.max(1, Math.min(12, Math.floor(input.limit ?? 6)));
        const results = docs
          .filter((doc) => corpus === "all" || doc.corpus === corpus)
          .map((doc) => ({
            doc,
            score: scoreDoc(doc, tokens),
          }))
          .filter((candidate) => candidate.score > 0)
          .sort((left, right) => right.score - left.score)
          .slice(0, limit)
          .map(({ doc, score }) => ({
            path: doc.path,
            corpus: doc.corpus,
            title: doc.title,
            score,
            excerpt: excerptFor(doc.content, tokens),
          }));

        return toolSuccess(
          { results },
          `Found ${results.length} Sui/Move knowledge result${results.length === 1 ? "" : "s"} for "${query}".`,
          [
            {
              kind: "toolOutput",
              source: "rust.knowledge.sui_move.search",
              summary: `Searched vendored Sui/Move docs for "${query}".`,
              raw: { query, corpus, resultCount: results.length },
            },
          ],
        );
      },
    }),
    defineAgentTool<
      { maxChars?: number; path: string },
      KnowledgeDoc & { truncated: boolean }
    >({
      id: "rust.knowledge.sui_move.read",
      title: "Read Sui Move knowledge doc",
      description:
        "Read a bounded excerpt from a vendored Sui/Move documentation file returned by the knowledge search tool.",
      inputSchema: {
        type: "object",
        properties: {
          path: {
            type: "string",
            description:
              "Knowledge document path returned by rust.knowledge.sui_move.search.",
          },
          maxChars: {
            type: "number",
            description: "Maximum characters to return. Defaults to 6000 and is capped at 20000.",
          },
        },
        required: ["path"],
        additionalProperties: false,
      },
      action: readOnlyAction("Read a bounded local Sui/Move knowledge document."),
      execute: async (input) => {
        const docs = await loadKnowledgeDocs();
        const doc = findDocByPath(docs, input.path);

        if (!doc) {
          return toolFailure(`No vendored Sui/Move knowledge document matched ${input.path}.`);
        }

        const maxChars = Math.max(500, Math.min(20_000, Math.floor(input.maxChars ?? 6_000)));
        const content = doc.content.slice(0, maxChars);

        return toolSuccess(
          {
            ...doc,
            content,
            truncated: content.length < doc.content.length,
          },
          `Read ${content.length} characters from ${doc.path}.`,
          [
            {
              kind: "toolOutput",
              source: "rust.knowledge.sui_move.read",
              summary: `Read vendored Sui/Move doc ${doc.path}.`,
              raw: { path: doc.path, corpus: doc.corpus, title: doc.title },
            },
          ],
        );
      },
    }),
  ];
}

async function loadKnowledgeDocs() {
  docCache ??= Promise.all(
    Object.entries(DOC_LOADERS).map(async ([path, loader]) => {
      const content = await loader();

      return {
        content,
        corpus: corpusForPath(path),
        path,
        title: titleFor(path, content),
      };
    }),
  );

  return docCache;
}

function corpusForPath(path: string): KnowledgeDoc["corpus"] {
  if (path.includes("/move-book-docs/")) return "move-book";
  if (path.includes("/sui-prover-docs/")) return "sui-prover";
  if (
    path.includes("/move-code-review/")
    || path.includes("/move-code-quality/")
    || path.includes("/move-pr-review/")
  ) {
    return "review-guidance";
  }
  return "sui-docs";
}

function titleFor(path: string, content: string) {
  const heading = content.match(/^#\s+(.+)$/m)?.[1]?.trim();
  if (heading) return heading.replace(/[{}]/g, "");

  const filename = path.split("/").pop() ?? path;
  return filename.replace(/\.(mdx?|move|toml|ya?ml|json|txt)$/i, "");
}

function tokenize(query: string) {
  return query
    .toLowerCase()
    .split(/[^a-z0-9_:$<>.-]+/i)
    .map((token) => token.trim())
    .filter((token) => token.length >= 2);
}

function scoreDoc(doc: KnowledgeDoc, tokens: string[]) {
  const haystack = `${doc.title}\n${doc.path}\n${doc.content}`.toLowerCase();
  let score = 0;

  for (const token of tokens) {
    if (!token) continue;
    const titleMatches = countOccurrences(doc.title.toLowerCase(), token);
    const pathMatches = countOccurrences(doc.path.toLowerCase(), token);
    const contentMatches = countOccurrences(haystack, token);
    score += titleMatches * 8 + pathMatches * 5 + Math.min(contentMatches, 20);
  }

  return score;
}

function countOccurrences(value: string, token: string) {
  let count = 0;
  let index = value.indexOf(token);

  while (index !== -1) {
    count += 1;
    index = value.indexOf(token, index + token.length);
  }

  return count;
}

function excerptFor(content: string, tokens: string[]) {
  const lower = content.toLowerCase();
  const firstMatch = tokens
    .map((token) => lower.indexOf(token))
    .filter((index) => index >= 0)
    .sort((left, right) => left - right)[0] ?? 0;
  const start = Math.max(0, firstMatch - 240);
  const end = Math.min(content.length, start + 900);

  return content
    .slice(start, end)
    .replace(/\s+/g, " ")
    .trim();
}

function findDocByPath(docs: KnowledgeDoc[], inputPath: string) {
  const normalized = inputPath.trim().replace(/^\/+/, "");

  return docs.find((doc) => doc.path === inputPath)
    ?? docs.find((doc) => doc.path.replace(/^\/+/, "") === normalized)
    ?? docs.find((doc) => doc.path.endsWith(normalized));
}
