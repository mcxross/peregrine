export type MoveAnalyzerDiagnosticSeverity = "error" | "warning" | "info" | "hint";

export type MoveAnalyzerPosition = {
  character: number;
  line: number;
};

export type MoveAnalyzerRange = {
  end: MoveAnalyzerPosition;
  start: MoveAnalyzerPosition;
};

export type MoveAnalyzerDiagnostic = {
  message: string;
  range: MoveAnalyzerRange;
  severity: MoveAnalyzerDiagnosticSeverity;
  source?: string | null;
};

export type MoveAnalyzerResolvedLocation = {
  path: string;
  range: MoveAnalyzerRange;
  uri: string;
};

export type MoveAnalyzerHover = {
  contents: MoveAnalyzerHoverContents;
  range?: MoveAnalyzerRange;
};

export type MoveAnalyzerHoverContents =
  | string
  | MoveAnalyzerMarkupContent
  | MoveAnalyzerMarkedString
  | MoveAnalyzerMarkedString[];

export type MoveAnalyzerMarkupContent = {
  kind?: "markdown" | "plaintext" | string;
  value: string;
};

export type MoveAnalyzerMarkedString =
  | string
  | {
      language: string;
      value: string;
    };

export type MoveAnalyzerCompletionList = {
  isIncomplete?: boolean;
  items: MoveAnalyzerCompletionItem[];
};

export type MoveAnalyzerCompletionContext = {
  triggerCharacter?: "." | ":" | "{" | null;
  triggerKind: 1 | 2;
};

export type MoveAnalyzerCompletionItem = {
  detail?: string | null;
  documentation?: MoveAnalyzerHoverContents | null;
  filterText?: string | null;
  insertText?: string | null;
  insertTextFormat?: number | null;
  kind?: number | null;
  label: string;
  sortText?: string | null;
  textEdit?: MoveAnalyzerTextEdit | null;
};

export type MoveAnalyzerTextEdit = {
  newText: string;
  range: MoveAnalyzerRange;
};

export type MoveAnalyzerWorkspaceEdit = {
  editsByPath: Record<string, MoveAnalyzerTextEdit[]>;
};
