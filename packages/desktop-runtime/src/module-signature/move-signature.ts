export type MoveSignatureToken = {
  kind:
    | "ability"
    | "identifier"
    | "keyword"
    | "module"
    | "number"
    | "plain"
    | "punctuation"
    | "type";
  value: string;
};

const MOVE_SIGNATURE_TOKEN_PATTERN =
  /(::|[A-Za-z_][A-Za-z0-9_]*|\d+|[{}()[\]<>,:;.=*&]|\s+|.)/g;
const MOVE_KEYWORDS = new Set([
  "acquires",
  "entry",
  "fun",
  "has",
  "friend",
  "mut",
  "native",
  "package",
  "public",
  "struct",
]);
const MOVE_ABILITIES = new Set(["copy", "drop", "key", "store"]);
const MOVE_PRIMITIVE_TYPES = new Set([
  "address",
  "bool",
  "signer",
  "u8",
  "u16",
  "u32",
  "u64",
  "u128",
  "u256",
  "vector",
]);

export function tokenizeMoveSignature(source: string): MoveSignatureToken[] {
  return Array.from(source.matchAll(MOVE_SIGNATURE_TOKEN_PATTERN), (match) => {
    const value = match[0];

    if (/^\s+$/.test(value)) {
      return { kind: "plain", value };
    }

    if (MOVE_KEYWORDS.has(value)) {
      return { kind: "keyword", value };
    }

    if (MOVE_ABILITIES.has(value)) {
      return { kind: "ability", value };
    }

    if (MOVE_PRIMITIVE_TYPES.has(value)) {
      return { kind: "type", value };
    }

    if (/^\d+$/.test(value)) {
      return { kind: "number", value };
    }

    if (value === "::") {
      return { kind: "module", value };
    }

    if (/^[{}()[\]<>,:;.=*&]$/.test(value)) {
      return { kind: "punctuation", value };
    }

    if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(value)) {
      return { kind: "identifier", value };
    }

    return { kind: "plain", value };
  });
}
