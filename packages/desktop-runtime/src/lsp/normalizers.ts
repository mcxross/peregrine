import { fileUri, relativePathFromFileUri } from "./path-utils";
import type {
  MoveAnalyzerCompletionItem,
  MoveAnalyzerCompletionList,
  MoveAnalyzerDiagnostic,
  MoveAnalyzerDiagnosticSeverity,
  MoveAnalyzerHover,
  MoveAnalyzerPosition,
  MoveAnalyzerRange,
  MoveAnalyzerResolvedLocation,
  MoveAnalyzerTextEdit,
  MoveAnalyzerWorkspaceEdit,
} from "./types";

export function textDocumentPositionParams(
  rootPath: string,
  path: string,
  position: MoveAnalyzerPosition,
) {
  return {
    position,
    textDocument: {
      uri: fileUri(rootPath, path),
    },
  };
}

export function normalizeMoveAnalyzerHover(result: unknown): MoveAnalyzerHover | null {
  if (!isRecord(result) || !("contents" in result)) {
    return null;
  }

  return {
    contents: result.contents as MoveAnalyzerHover["contents"],
    range: isRange(result.range) ? result.range : undefined,
  };
}

export function normalizeMoveAnalyzerCompletionList(
  result: unknown,
): MoveAnalyzerCompletionList | null {
  if (!result) {
    return null;
  }

  if (Array.isArray(result)) {
    return {
      isIncomplete: false,
      items: result.filter(isCompletionItem),
    };
  }

  if (!isRecord(result) || !Array.isArray(result.items)) {
    return null;
  }

  return {
    isIncomplete: result.isIncomplete === true,
    items: result.items.filter(isCompletionItem),
  };
}

export function normalizeMoveAnalyzerLocations(
  rootPath: string,
  result: unknown,
): MoveAnalyzerResolvedLocation[] {
  if (!result) {
    return [];
  }

  const locations = Array.isArray(result) ? result : [result];

  return locations.flatMap((location) => {
    if (isLocation(location)) {
      const path = relativePathFromFileUri(rootPath, location.uri);

      return path ? [{ path, range: location.range, uri: location.uri }] : [];
    }

    if (isLocationLink(location)) {
      const path = relativePathFromFileUri(rootPath, location.targetUri);

      return path
        ? [{
            path,
            range: location.targetSelectionRange ?? location.targetRange,
            uri: location.targetUri,
          }]
        : [];
    }

    return [];
  });
}

export function normalizeMoveAnalyzerWorkspaceEdit(
  rootPath: string,
  result: unknown,
): MoveAnalyzerWorkspaceEdit | null {
  if (!isRecord(result)) {
    return null;
  }

  const editsByPath: Record<string, MoveAnalyzerTextEdit[]> = {};

  if (isRecord(result.changes)) {
    for (const [uri, edits] of Object.entries(result.changes)) {
      addTextEdits(rootPath, editsByPath, uri, edits);
    }
  }

  if (Array.isArray(result.documentChanges)) {
    for (const change of result.documentChanges) {
      if (!isRecord(change) || !isRecord(change.textDocument) || typeof change.textDocument.uri !== "string") {
        continue;
      }

      addTextEdits(rootPath, editsByPath, change.textDocument.uri, change.edits);
    }
  }

  return Object.keys(editsByPath).length ? { editsByPath } : null;
}

export function normalizeMoveAnalyzerPublishDiagnostics(
  rootPath: string,
  params: unknown,
): { diagnostics: MoveAnalyzerDiagnostic[]; path: string } | null {
  if (!isPublishDiagnosticsParams(params)) {
    return null;
  }

  const path = relativePathFromFileUri(rootPath, params.uri);

  if (!path) {
    return null;
  }

  return {
    diagnostics: params.diagnostics.map(normalizeDiagnostic),
    path,
  };
}

export function showMoveAnalyzerMessageText(params: unknown) {
  if (!isRecord(params)) {
    return null;
  }

  const isErrorOrWarning = params.type === 1 || params.type === 2;

  return isErrorOrWarning && typeof params.message === "string"
    ? params.message
    : null;
}

function addTextEdits(
  rootPath: string,
  editsByPath: Record<string, MoveAnalyzerTextEdit[]>,
  uri: string,
  edits: unknown,
) {
  if (!Array.isArray(edits)) {
    return;
  }

  const path = relativePathFromFileUri(rootPath, uri);

  if (!path) {
    return;
  }

  const textEdits = edits.filter(isTextEdit);

  if (!textEdits.length) {
    return;
  }

  editsByPath[path] = [...(editsByPath[path] ?? []), ...textEdits];
}

function isCompletionItem(value: unknown): value is MoveAnalyzerCompletionItem {
  if (!isRecord(value) || typeof value.label !== "string") {
    return false;
  }

  return value.textEdit == null || isTextEdit(value.textEdit);
}

function isTextEdit(value: unknown): value is MoveAnalyzerTextEdit {
  return isRecord(value)
    && typeof value.newText === "string"
    && isRange(value.range);
}

function isLocation(value: unknown): value is {
  range: MoveAnalyzerRange;
  uri: string;
} {
  return isRecord(value)
    && typeof value.uri === "string"
    && isRange(value.range);
}

function isLocationLink(value: unknown): value is {
  targetRange: MoveAnalyzerRange;
  targetSelectionRange?: MoveAnalyzerRange;
  targetUri: string;
} {
  return isRecord(value)
    && typeof value.targetUri === "string"
    && isRange(value.targetRange)
    && (value.targetSelectionRange == null || isRange(value.targetSelectionRange));
}

function isRange(value: unknown): value is MoveAnalyzerRange {
  return isRecord(value)
    && isPosition(value.start)
    && isPosition(value.end);
}

function isPosition(value: unknown): value is MoveAnalyzerPosition {
  return isRecord(value)
    && typeof value.character === "number"
    && typeof value.line === "number";
}

function normalizeDiagnostic(diagnostic: LspDiagnostic): MoveAnalyzerDiagnostic {
  return {
    message: diagnostic.message,
    range: diagnostic.range,
    severity: diagnosticSeverity(diagnostic.severity),
    source: diagnostic.source ?? "move-analyzer",
  };
}

function diagnosticSeverity(severity: number | undefined): MoveAnalyzerDiagnosticSeverity {
  switch (severity) {
    case 1:
      return "error";
    case 2:
      return "warning";
    case 3:
      return "info";
    case 4:
      return "hint";
    default:
      return "error";
  }
}

type LspDiagnostic = {
  message: string;
  range: {
    end: { character: number; line: number };
    start: { character: number; line: number };
  };
  source?: string | null;
  severity?: number;
};

function isPublishDiagnosticsParams(value: unknown): value is {
  diagnostics: LspDiagnostic[];
  uri: string;
} {
  return isRecord(value)
    && typeof value.uri === "string"
    && Array.isArray(value.diagnostics);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object";
}
