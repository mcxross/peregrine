import {
  autocompletion,
  startCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, hoverTooltip, keymap, type Tooltip } from "@codemirror/view";

import type {
  MoveAnalyzerCompletionItem,
  MoveAnalyzerCompletionContext,
  MoveAnalyzerCompletionList,
  MoveAnalyzerHover,
  MoveAnalyzerHoverContents,
  MoveAnalyzerPosition,
  MoveAnalyzerRange,
  MoveAnalyzerResolvedLocation,
  MoveAnalyzerWorkspaceEdit,
} from "@/features/project-workspace/editor/lsp/types";

export type CodeEditorMoveAnalyzerFeatures = {
  completion: (
    position: MoveAnalyzerPosition,
    context?: MoveAnalyzerCompletionContext,
  ) => Promise<MoveAnalyzerCompletionList | null>;
  definition: (position: MoveAnalyzerPosition) => Promise<MoveAnalyzerResolvedLocation[]>;
  hover: (position: MoveAnalyzerPosition) => Promise<MoveAnalyzerHover | null>;
  references: (position: MoveAnalyzerPosition) => Promise<MoveAnalyzerResolvedLocation[]>;
  rename: (position: MoveAnalyzerPosition, newName: string) => Promise<MoveAnalyzerWorkspaceEdit | null>;
};

export type CodeEditorMoveAnalyzerBridge = CodeEditorMoveAnalyzerFeatures & {
  applyWorkspaceEdit: (edit: MoveAnalyzerWorkspaceEdit) => Promise<void> | void;
  openLocation: (location: MoveAnalyzerResolvedLocation) => void;
};

const COMPLETION_VALID_FOR = /^[A-Za-z0-9_]*$/;
const IDENTIFIER_COMPLETION_PREFIX = /[A-Za-z_][A-Za-z0-9_]*$/;
const MIN_AUTO_IDENTIFIER_LENGTH = 2;

export function moveAnalyzerEditorExtensions(
  language: string,
  bridge: CodeEditorMoveAnalyzerBridge,
): Extension {
  if (language.toLowerCase() !== "move") {
    return [];
  }

  return [
    autocompletion({
      activateOnTyping: true,
      activateOnTypingDelay: 300,
      override: [(context) => moveAnalyzerCompletions(context, bridge)],
      selectOnOpen: false,
    }),
    hoverTooltip((view, position) => moveAnalyzerHover(view, position, bridge), {
      hideOnChange: true,
      hoverTime: 650,
    }),
    keymap.of([
      { key: "Ctrl-Space", run: startCompletion },
      {
        key: "F12",
        run: (view) => {
          void requestDefinition(view, view.state.selection.main.head, bridge);
          return true;
        },
      },
      {
        key: "Mod-b",
        run: (view) => {
          void requestDefinition(view, view.state.selection.main.head, bridge);
          return true;
        },
      },
      {
        key: "F2",
        run: (view) => {
          void requestRename(view, view.state.selection.main.head, bridge);
          return true;
        },
      },
    ]),
    EditorView.domEventHandlers({
      mousedown(event, view) {
        if (event.button !== 0 || (!event.metaKey && !event.ctrlKey)) {
          return false;
        }

        const position = view.posAtCoords({
          x: event.clientX,
          y: event.clientY,
        });

        if (position == null) {
          return false;
        }

        event.preventDefault();
        void requestDefinition(view, position, bridge);
        return true;
      },
    }),
  ];
}

export function offsetToLspPosition(
  state: EditorState,
  offset: number,
): MoveAnalyzerPosition {
  const position = Math.min(state.doc.length, Math.max(0, Math.floor(offset)));
  const line = state.doc.lineAt(position);

  return {
    character: position - line.from,
    line: line.number - 1,
  };
}

export function lspPositionToOffset(
  state: EditorState,
  position: MoveAnalyzerPosition,
) {
  const lineNumber = clampLineNumber(position.line + 1, state.doc.lines);
  const line = state.doc.line(lineNumber);
  const character = Number.isFinite(position.character)
    ? Math.max(0, Math.floor(position.character))
    : 0;

  return Math.min(line.to, line.from + character);
}

function moveAnalyzerCompletions(
  context: CompletionContext,
  bridge: CodeEditorMoveAnalyzerFeatures,
): CompletionResult | null | Promise<CompletionResult | null> {
  const completionTarget = moveCompletionTarget(context);

  if (!completionTarget) {
    return null;
  }

  return bridge.completion(
    offsetToLspPosition(context.state, completionTarget.requestOffset),
    completionTarget.lspContext,
  ).then((completionList) => {
    if (!completionList?.items.length || context.aborted) {
      return null;
    }

    return {
      from: completionListFrom(context.state, completionList, completionTarget.from),
      options: completionList.items.map(completionOption),
      validFor: COMPLETION_VALID_FOR,
    };
  });
}

function moveCompletionTarget(context: CompletionContext): {
  from: number;
  lspContext: MoveAnalyzerCompletionContext;
  requestOffset: number;
} | null {
  const accessTarget = accessCompletionTarget(context.state, context.pos);

  if (accessTarget) {
    return accessTarget;
  }

  const prefix = context.matchBefore(IDENTIFIER_COMPLETION_PREFIX);

  if (!prefix) {
    return null;
  }

  if (!context.explicit && prefix.text.length < MIN_AUTO_IDENTIFIER_LENGTH) {
    return null;
  }

  return {
    from: prefix.from,
    lspContext: { triggerKind: context.explicit ? 1 : 2 },
    requestOffset: context.pos,
  };
}

function accessCompletionTarget(
  state: EditorState,
  offset: number,
): {
  from: number;
  lspContext: MoveAnalyzerCompletionContext;
  requestOffset: number;
} | null {
  const line = state.doc.lineAt(offset);
  const linePrefix = state.sliceDoc(line.from, offset);
  const dotIndex = linePrefix.lastIndexOf(".");
  const colonColonIndex = linePrefix.lastIndexOf("::");
  const lbraceIndex = linePrefix.lastIndexOf("{");
  const triggerIndex = Math.max(dotIndex, colonColonIndex, lbraceIndex);

  if (triggerIndex < 0) {
    return null;
  }

  const textAfterTrigger = linePrefix.slice(
    triggerIndex + (triggerIndex === colonColonIndex ? 2 : 1),
  );

  if (!/^[A-Za-z0-9_]*$/.test(textAfterTrigger)) {
    return null;
  }

  if (triggerIndex === colonColonIndex) {
    const requestOffset = line.from + triggerIndex + 2;

    return {
      from: requestOffset,
      lspContext: {
        triggerCharacter: ":",
        triggerKind: 2,
      },
      requestOffset,
    };
  }

  const triggerCharacter = triggerIndex === dotIndex ? "." : "{";
  const requestOffset = line.from + triggerIndex + 1;

  return {
    from: requestOffset,
    lspContext: {
      triggerCharacter,
      triggerKind: 2,
    },
    requestOffset,
  };
}

function moveAnalyzerHover(
  view: EditorView,
  position: number,
  bridge: CodeEditorMoveAnalyzerFeatures,
): Promise<Tooltip | null> {
  const lspPosition = offsetToLspPosition(view.state, position);

  return bridge.hover(lspPosition).then((hover) => {
    const text = hover ? hoverText(hover.contents) : "";

    if (!hover || !text.trim()) {
      return null;
    }

    const range = hover.range ? rangeToOffsets(view.state, hover.range) : null;

    return {
      above: true,
      create() {
        const dom = document.createElement("div");
        dom.className = "cm-moveAnalyzerHover";
        dom.textContent = text;
        return { dom };
      },
      end: range?.to,
      pos: range?.from ?? position,
    };
  });
}

async function requestDefinition(
  view: EditorView,
  offset: number,
  bridge: CodeEditorMoveAnalyzerBridge,
) {
  const locations = await bridge.definition(offsetToLspPosition(view.state, offset));
  const target = locations[0];

  if (!target) {
    return false;
  }

  bridge.openLocation(target);
  return true;
}

async function requestRename(
  view: EditorView,
  offset: number,
  bridge: CodeEditorMoveAnalyzerBridge,
) {
  const currentName = symbolAt(view.state, offset);
  const nextName = window.prompt("New symbol name", currentName);

  if (!nextName || nextName === currentName) {
    return false;
  }

  const trimmedName = nextName.trim();

  if (!trimmedName || trimmedName === currentName) {
    return false;
  }

  const workspaceEdit = await bridge.rename(
    offsetToLspPosition(view.state, offset),
    trimmedName,
  );

  if (!workspaceEdit) {
    return false;
  }

  await bridge.applyWorkspaceEdit(workspaceEdit);
  return true;
}

function symbolAt(state: EditorState, offset: number) {
  const doc = state.doc.toString();
  const position = Math.min(doc.length, Math.max(0, offset));
  const left = doc.slice(0, position).match(/[A-Za-z_][A-Za-z0-9_]*$/)?.[0] ?? "";
  const right = doc.slice(position).match(/^[A-Za-z0-9_]*/)?.[0] ?? "";

  return `${left}${right}`;
}

function completionListFrom(
  state: EditorState,
  completionList: MoveAnalyzerCompletionList,
  fallbackFrom: number,
) {
  const textEditRanges = completionList.items
    .map((item) => item.textEdit?.range)
    .filter((range): range is MoveAnalyzerRange => Boolean(range))
    .map((range) => rangeToOffsets(state, range).from);

  return textEditRanges.length
    ? Math.min(...textEditRanges)
    : fallbackFrom;
}

function completionOption(item: MoveAnalyzerCompletionItem): Completion {
  const textEdit = item.textEdit ?? null;
  const text = sanitizeInsertText(textEdit?.newText ?? item.insertText ?? item.label);
  const documentation = item.documentation ? hoverText(item.documentation) : "";

  return {
    apply: textEdit
      ? (view) => {
          const range = rangeToOffsets(view.state, textEdit.range);

          view.dispatch({
            changes: {
              from: range.from,
              insert: text,
              to: range.to,
            },
            selection: {
              anchor: range.from + text.length,
            },
            userEvent: "input.complete",
          });
        }
      : text,
    detail: item.detail ?? undefined,
    info: documentation
      ? () => {
          const dom = document.createElement("div");
          dom.className = "cm-moveAnalyzerCompletionInfo";
          dom.textContent = documentation;
          return dom;
        }
      : undefined,
    label: item.label,
    sortText: item.sortText ?? item.filterText ?? undefined,
    type: completionKind(item.kind),
  };
}

function rangeToOffsets(
  state: EditorState,
  range: MoveAnalyzerRange,
) {
  const from = lspPositionToOffset(state, range.start);
  const rawTo = lspPositionToOffset(state, range.end);

  return {
    from,
    to: Math.max(from, rawTo),
  };
}

function hoverText(contents: MoveAnalyzerHoverContents): string {
  return normalizeHoverText(rawHoverText(contents));
}

function rawHoverText(contents: MoveAnalyzerHoverContents): string {
  if (typeof contents === "string") {
    return contents;
  }

  if (Array.isArray(contents)) {
    return contents.map(rawHoverText).filter(Boolean).join("\n\n");
  }

  if ("value" in contents && typeof contents.value === "string") {
    return contents.value;
  }

  return "";
}

function normalizeHoverText(text: string) {
  const normalized = text.replace(/\r\n/g, "\n").trim();
  const singleFence = normalized.match(/^```[A-Za-z0-9_+-]*\n([\s\S]*?)\n```$/);

  if (singleFence) {
    return singleFence[1].trim();
  }

  return normalized
    .replace(/```[A-Za-z0-9_+-]*\n([\s\S]*?)\n```/g, (_match, code: string) => code.trim())
    .replace(/`([^`]+)`/g, "$1")
    .trim();
}

function sanitizeInsertText(text: string) {
  return text
    .replace(/\$\{\d+:([^}]+)\}/g, "$1")
    .replace(/\$\d+/g, "");
}

function completionKind(kind: number | null | undefined) {
  switch (kind) {
    case 2:
      return "method";
    case 3:
      return "function";
    case 5:
    case 10:
      return "property";
    case 6:
      return "variable";
    case 7:
      return "class";
    case 8:
      return "interface";
    case 9:
      return "namespace";
    case 13:
    case 20:
      return "enum";
    case 14:
      return "keyword";
    case 21:
      return "constant";
    case 22:
    case 25:
      return "type";
    default:
      return "text";
  }
}

function clampLineNumber(lineNumber: number, lineCount: number) {
  if (!Number.isFinite(lineNumber)) {
    return 1;
  }

  return Math.min(lineCount, Math.max(1, Math.floor(lineNumber)));
}
