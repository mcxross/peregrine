import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { css } from "@codemirror/lang-css";
import { html } from "@codemirror/lang-html";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { markdown } from "@codemirror/lang-markdown";
import { rust } from "@codemirror/lang-rust";
import { yaml } from "@codemirror/lang-yaml";
import {
  bracketMatching,
  HighlightStyle,
  indentOnInput,
  StreamLanguage,
  syntaxHighlighting,
} from "@codemirror/language";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import {
  EditorState,
  RangeSetBuilder,
  StateEffect,
  StateField,
  type Extension,
} from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  WidgetType,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";
import React from "react";

type CodeEditorProps = {
  complexityHighlights?: ComplexityHighlight[];
  jumpRequest?: CodeEditorJumpRequest | null;
  language: string;
  sourceSelectionRequest?: CodeEditorSourceSelectionRequest | null;
  sourceSpanHighlights?: CodeEditorSourceSpanHighlight[];
  value: string;
  onCursorByteOffsetChange?: (byteOffset: number) => void;
  onChange: (value: string) => void;
};

export type CodeEditorJumpRequest = {
  line: number;
  token: number;
};

export type CodeEditorSourceSelectionRequest = {
  endByte: number;
  focus?: boolean;
  startByte: number;
  token: number;
};

export type CodeEditorSourceSpanHighlight = {
  endByte: number;
  id: string;
  isActive?: boolean;
  isExiting?: boolean;
  startByte: number;
  title?: string;
};

export type ComplexityHighlight = {
  endLine: number;
  message?: string;
  score: number;
  severity?: "info" | "warning" | "error";
  startLine: number;
  target: string;
  threshold: number | null;
};

const EMPTY_COMPLEXITY_HIGHLIGHTS: ComplexityHighlight[] = [];
const EMPTY_SOURCE_SPAN_HIGHLIGHTS: CodeEditorSourceSpanHighlight[] = [];

const setComplexityHighlights = StateEffect.define<ComplexityHighlight[]>();
const setSourceSpanHighlights = StateEffect.define<CodeEditorSourceSpanHighlight[]>();
const LONG_MOVE_ADDRESS_PATTERN = /(?:0x)?[0-9a-fA-F]{33,64}(?=::)/g;

// Code editors need stable token contrast independent of app accent/theme swaps.
const editorHighlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: "#f472b6", fontWeight: "600" },
  { tag: tags.controlKeyword, color: "#fb7185", fontWeight: "600" },
  { tag: tags.operatorKeyword, color: "#f472b6", fontWeight: "600" },
  { tag: tags.moduleKeyword, color: "#22d3ee", fontWeight: "600" },
  { tag: tags.bool, color: "#fbbf24", fontWeight: "600" },
  { tag: tags.null, color: "#fbbf24" },
  { tag: tags.number, color: "#fbbf24" },
  { tag: tags.string, color: "#a7f3d0" },
  { tag: tags.character, color: "#a7f3d0" },
  { tag: tags.escape, color: "#fde68a" },
  { tag: tags.comment, color: "#f59e0b", fontStyle: "italic" },
  { tag: tags.lineComment, color: "#f59e0b", fontStyle: "italic" },
  { tag: tags.blockComment, color: "#f59e0b", fontStyle: "italic" },
  { tag: tags.name, color: "#f8fafc" },
  { tag: tags.variableName, color: "#f8fafc" },
  { tag: tags.definition(tags.variableName), color: "#93c5fd", fontWeight: "600" },
  { tag: tags.function(tags.variableName), color: "#7dd3fc", fontWeight: "600" },
  { tag: tags.function(tags.definition(tags.variableName)), color: "#7dd3fc", fontWeight: "700" },
  { tag: tags.typeName, color: "#5eead4", fontWeight: "600" },
  { tag: tags.className, color: "#5eead4", fontWeight: "600" },
  { tag: tags.propertyName, color: "#c4b5fd" },
  { tag: tags.attributeName, color: "#c4b5fd" },
  { tag: tags.labelName, color: "#fca5a5" },
  { tag: tags.operator, color: "#e2e8f0" },
  { tag: tags.punctuation, color: "#cbd5e1" },
  { tag: tags.brace, color: "#f8fafc" },
  { tag: tags.squareBracket, color: "#f8fafc" },
  { tag: tags.paren, color: "#f8fafc" },
  { tag: tags.angleBracket, color: "#f8fafc" },
  { tag: tags.invalid, color: "#fecaca", textDecoration: "underline wavy #ef4444" },
]);

const complexityHighlightField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, transaction) {
    let nextDecorations = decorations.map(transaction.changes);

    for (const effect of transaction.effects) {
      if (effect.is(setComplexityHighlights)) {
        nextDecorations = buildComplexityDecorations(transaction.state, effect.value);
      }
    }

    return nextDecorations;
  },
  provide: (field) => EditorView.decorations.from(field),
});

const compactMoveAddressField = StateField.define<DecorationSet>({
  create(state) {
    return buildCompactMoveAddressDecorations(state);
  },
  update(decorations, transaction) {
    if (transaction.docChanged) {
      return buildCompactMoveAddressDecorations(transaction.state);
    }

    return decorations.map(transaction.changes);
  },
  provide: (field) => EditorView.decorations.from(field),
});

const sourceSpanHighlightField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(decorations, transaction) {
    let nextDecorations = decorations.map(transaction.changes);

    for (const effect of transaction.effects) {
      if (effect.is(setSourceSpanHighlights)) {
        nextDecorations = buildSourceSpanDecorations(transaction.state, effect.value);
      }
    }

    return nextDecorations;
  },
  provide: (field) => EditorView.decorations.from(field),
});

class CompactMoveAddressWidget extends WidgetType {
  constructor(
    private readonly fullAddress: string,
    private readonly compactAddress: string,
  ) {
    super();
  }

  eq(other: CompactMoveAddressWidget) {
    return this.fullAddress === other.fullAddress
      && this.compactAddress === other.compactAddress;
  }

  toDOM() {
    const element = document.createElement("span");

    element.className = "cm-compact-move-address";
    element.textContent = this.compactAddress;
    element.title = this.fullAddress;

    return element;
  }
}

export function CodeEditor({
  complexityHighlights = EMPTY_COMPLEXITY_HIGHLIGHTS,
  jumpRequest = null,
  language,
  sourceSelectionRequest = null,
  sourceSpanHighlights = EMPTY_SOURCE_SPAN_HIGHLIGHTS,
  value,
  onCursorByteOffsetChange,
  onChange,
}: CodeEditorProps) {
  const hostRef = React.useRef<HTMLDivElement | null>(null);
  const editorRef = React.useRef<EditorView | null>(null);
  const onChangeRef = React.useRef(onChange);
  const onCursorByteOffsetChangeRef = React.useRef(onCursorByteOffsetChange);

  React.useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  React.useEffect(() => {
    onCursorByteOffsetChangeRef.current = onCursorByteOffsetChange;
  }, [onCursorByteOffsetChange]);

  React.useEffect(() => {
    if (!hostRef.current) {
      return;
    }

    console.info("[CodeEditor] creating editor", {
      language,
      sourceLength: value.length,
    });
    const editor = new EditorView({
      parent: hostRef.current,
      state: EditorState.create({
        doc: value,
        extensions: editorExtensions(language, (nextValue) => {
          onChangeRef.current(nextValue);
        }, (byteOffset) => {
          onCursorByteOffsetChangeRef.current?.(byteOffset);
        }),
      }),
    });

    editorRef.current = editor;
    editor.dispatch({
      effects: setComplexityHighlights.of(complexityHighlights),
    });
    editor.dispatch({
      effects: setSourceSpanHighlights.of(sourceSpanHighlights),
    });

    return () => {
      console.info("[CodeEditor] destroying editor", {
        language,
      });
      editor.destroy();
      editorRef.current = null;
    };
  }, [language]);

  React.useEffect(() => {
    const editor = editorRef.current;

    if (!editor || editor.state.doc.toString() === value) {
      return;
    }

    editor.dispatch({
      changes: {
        from: 0,
        to: editor.state.doc.length,
        insert: value,
      },
    });
  }, [value]);

  React.useEffect(() => {
    const editor = editorRef.current;

    if (!editor) {
      return;
    }

    editor.dispatch({
      effects: setComplexityHighlights.of(complexityHighlights),
    });
  }, [complexityHighlights]);

  React.useEffect(() => {
    const editor = editorRef.current;

    if (!editor) {
      return;
    }

    editor.dispatch({
      effects: setSourceSpanHighlights.of(sourceSpanHighlights),
    });
  }, [sourceSpanHighlights]);

  React.useEffect(() => {
    const editor = editorRef.current;

    if (!editor || !jumpRequest) {
      if (jumpRequest && !editor) {
        console.warn("[CodeEditor] jump requested before editor was ready", jumpRequest);
      }
      return;
    }

    const clampedLine = clampLineNumber(jumpRequest.line, editor.state.doc.lines);
    console.info("[CodeEditor] applying jump request", {
      clampedLine,
      lineCount: editor.state.doc.lines,
      requestedLine: jumpRequest.line,
      token: jumpRequest.token,
    });
    const line = editor.state.doc.line(clampedLine);

    editor.dispatch({
      selection: { anchor: line.from },
    });
    smoothScrollEditorToPosition(editor, line.from);
    editor.focus();
  }, [jumpRequest]);

  React.useEffect(() => {
    const editor = editorRef.current;

    if (!editor || !sourceSelectionRequest) {
      return;
    }

    const source = editor.state.doc.toString();
    const from = byteOffsetToPosition(source, sourceSelectionRequest.startByte);
    const rawTo = byteOffsetToPosition(source, sourceSelectionRequest.endByte);
    const to = Math.max(from, rawTo);
    const head = to > from ? to : Math.min(editor.state.doc.length, from + 1);

    editor.dispatch({
      selection: { anchor: from, head },
    });
    smoothScrollEditorToPosition(editor, from);

    if (sourceSelectionRequest.focus) {
      editor.focus();
    }
  }, [sourceSelectionRequest]);

  return <div ref={hostRef} className="min-h-0 flex-1 overflow-hidden" />;
}

function editorExtensions(
  language: string,
  onChange: (value: string) => void,
  onCursorByteOffsetChange: (byteOffset: number) => void,
) {
  return [
    lineNumbers(),
    history(),
    drawSelection(),
    indentOnInput(),
    bracketMatching(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    syntaxHighlighting(editorHighlightStyle, { fallback: true }),
    keymap.of([indentWithTab, ...defaultKeymap, ...historyKeymap]),
    complexityHighlightField,
    sourceSpanHighlightField,
    language.toLowerCase() === "move" ? compactMoveAddressField : [],
    editorTheme,
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onChange(update.state.doc.toString());
      }
      if (update.selectionSet) {
        onCursorByteOffsetChange(
          positionToByteOffset(
            update.state.doc.toString(),
            update.state.selection.main.from,
          ),
        );
      }
    }),
    languageExtension(language),
  ];
}

function buildCompactMoveAddressDecorations(state: EditorState) {
  const builder = new RangeSetBuilder<Decoration>();
  const text = state.doc.toString();

  LONG_MOVE_ADDRESS_PATTERN.lastIndex = 0;

  for (const match of text.matchAll(LONG_MOVE_ADDRESS_PATTERN)) {
    if (match.index == null) {
      continue;
    }

    const fullAddress = match[0];
    const compactAddress = compactMoveAddress(fullAddress);

    if (compactAddress === fullAddress) {
      continue;
    }

    builder.add(
      match.index,
      match.index + fullAddress.length,
      Decoration.replace({
        widget: new CompactMoveAddressWidget(fullAddress, compactAddress),
      }),
    );
  }

  return builder.finish();
}

function buildSourceSpanDecorations(
  state: EditorState,
  highlights: CodeEditorSourceSpanHighlight[],
) {
  if (!highlights.length) {
    return Decoration.none;
  }

  const builder = new RangeSetBuilder<Decoration>();
  const source = state.doc.toString();

  for (const highlight of [...highlights].sort((left, right) => left.startByte - right.startByte)) {
    const from = byteOffsetToPosition(source, highlight.startByte);
    const to = Math.max(from, byteOffsetToPosition(source, highlight.endByte));
    const classes = [
      "cm-bytecodeSourceSpan",
      highlight.isActive ? "cm-bytecodeSourceSpanActive" : "",
      highlight.isExiting ? "cm-bytecodeSourceSpanExit" : "",
    ].filter(Boolean).join(" ");

    if (from >= state.doc.length && to >= state.doc.length) {
      continue;
    }

    builder.add(
      from,
      to > from ? to : Math.min(state.doc.length, from + 1),
      Decoration.mark({
        attributes: highlight.title ? { title: highlight.title } : undefined,
        class: classes,
      }),
    );
  }

  return builder.finish();
}

function byteOffsetToPosition(source: string, byteOffset: number) {
  const target = Math.max(0, Math.floor(byteOffset));
  let consumedBytes = 0;
  let position = 0;

  while (position < source.length && consumedBytes < target) {
    const codePoint = source.codePointAt(position) ?? 0;
    const codeUnits = codePoint > 0xffff ? 2 : 1;
    const bytes = utf8ByteLength(codePoint);

    if (consumedBytes + bytes > target) {
      break;
    }

    consumedBytes += bytes;
    position += codeUnits;
  }

  return position;
}

function positionToByteOffset(source: string, position: number) {
  const target = Math.min(source.length, Math.max(0, Math.floor(position)));
  let byteOffset = 0;
  let cursor = 0;

  while (cursor < target) {
    const codePoint = source.codePointAt(cursor) ?? 0;
    cursor += codePoint > 0xffff ? 2 : 1;
    byteOffset += utf8ByteLength(codePoint);
  }

  return byteOffset;
}

function utf8ByteLength(codePoint: number) {
  if (codePoint <= 0x7f) {
    return 1;
  }
  if (codePoint <= 0x7ff) {
    return 2;
  }
  if (codePoint <= 0xffff) {
    return 3;
  }

  return 4;
}

function compactMoveAddress(address: string) {
  const hasPrefix = address.startsWith("0x") || address.startsWith("0X");
  const prefix = hasPrefix ? address.slice(0, 2) : "";
  const hex = hasPrefix ? address.slice(2) : address;

  if (hex.length <= 16) {
    return address;
  }

  return `${prefix}${hex.slice(0, 8)}...${hex.slice(-4)}`;
}

function languageExtension(language: string): Extension {
  switch (language.toLowerCase()) {
    case "css":
      return css();
    case "html":
      return html();
    case "javascript":
      return javascript({ jsx: false, typescript: false });
    case "json":
      return json();
    case "jsx":
      return javascript({ jsx: true, typescript: false });
    case "markdown":
      return markdown();
    case "move":
      return rust();
    case "rust":
      return rust();
    case "shell":
      return StreamLanguage.define(shell);
    case "toml":
      return StreamLanguage.define(toml);
    case "tsx":
      return javascript({ jsx: true, typescript: true });
    case "typescript":
      return javascript({ jsx: false, typescript: true });
    case "yaml":
      return yaml();
    default:
      return [];
  }
}

function buildComplexityDecorations(
  state: EditorState,
  highlights: ComplexityHighlight[],
) {
  if (!highlights.length) {
    return Decoration.none;
  }

  const lines = new Map<number, {
    isEnd: boolean;
    isStart: boolean;
    severity: ComplexityHighlight["severity"];
    title: string;
  }>();

  for (const highlight of highlights) {
    const startLine = clampLineNumber(highlight.startLine, state.doc.lines);
    const endLine = clampLineNumber(
      Math.max(highlight.startLine, highlight.endLine),
      state.doc.lines,
    );
    const title = complexityTitle(highlight);

    for (let lineNumber = startLine; lineNumber <= endLine; lineNumber += 1) {
      const existing = lines.get(lineNumber);

      lines.set(lineNumber, {
        isEnd: Boolean(existing?.isEnd) || lineNumber === endLine,
        isStart: Boolean(existing?.isStart) || lineNumber === startLine,
        severity: highestSeverity(existing?.severity, highlight.severity),
        title: existing?.title ? `${existing.title}\n${title}` : title,
      });
    }
  }

  const builder = new RangeSetBuilder<Decoration>();

  for (const [lineNumber, line] of [...lines.entries()].sort((left, right) => left[0] - right[0])) {
    const docLine = state.doc.line(lineNumber);
    const severityClass = line.severity === "error" ? "cm-complexityLineError" : "cm-complexityLineWarning";
    const classes = [
      "cm-complexityLine",
      severityClass,
      line.isStart ? "cm-complexityLineStart" : "",
      line.isEnd ? "cm-complexityLineEnd" : "",
    ].filter(Boolean).join(" ");

    builder.add(
      docLine.from,
      docLine.from,
      Decoration.line({
        attributes: {
          title: line.title,
        },
        class: classes,
      }),
    );
  }

  return builder.finish();
}

function clampLineNumber(lineNumber: number, lineCount: number) {
  if (!Number.isFinite(lineNumber)) {
    return 1;
  }

  return Math.min(lineCount, Math.max(1, Math.floor(lineNumber)));
}

function complexityTitle(highlight: ComplexityHighlight) {
  const threshold = highlight.threshold == null
    ? ""
    : `, threshold ${highlight.threshold}`;
  return highlight.message || `${highlight.target}: complexity ${highlight.score}${threshold}`;
}

function highestSeverity(
  current: ComplexityHighlight["severity"],
  next: ComplexityHighlight["severity"],
) {
  const rank = { info: 0, warning: 1, error: 2 } as const;
  const currentRank = current ? rank[current] : -1;
  const nextRank = next ? rank[next] : -1;

  return nextRank > currentRank ? next : current;
}

function preferredScrollBehavior(): ScrollBehavior {
  if (typeof window === "undefined") {
    return "auto";
  }

  return window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth";
}

function smoothScrollEditorToPosition(editor: EditorView, position: number) {
  const behavior = preferredScrollBehavior();

  if (behavior === "auto") {
    editor.dispatch({
      effects: EditorView.scrollIntoView(position, { y: "center" }),
    });
    return;
  }

  requestAnimationFrame(() => {
    const scrollDOM = editor.scrollDOM;
    const block = editor.lineBlockAt(Math.min(editor.state.doc.length, Math.max(0, position)));
    const maxTop = Math.max(0, scrollDOM.scrollHeight - scrollDOM.clientHeight);
    const targetTop = Math.min(
      maxTop,
      Math.max(0, block.top + block.height / 2 - scrollDOM.clientHeight * 0.42),
    );

    scrollDOM.scrollTo({
      behavior,
      top: targetTop,
    });
  });
}

const editorTheme = EditorView.theme(
  {
    "&": {
      backgroundColor: "#070a0f",
      color: "#f4f7fb",
      height: "100%",
    },
    "&.cm-focused": {
      outline: "none",
    },
    ".cm-content": {
      caretColor: "#f8fafc",
      minHeight: "100%",
      padding: "20px 0",
    },
    ".cm-line": {
      color: "#f4f7fb",
      padding: "0 20px",
    },
    ".cm-line span": {
      textDecorationThickness: "1px",
      textUnderlineOffset: "3px",
    },
    ".cm-cursor": {
      borderLeftColor: "#f8fafc",
      borderLeftWidth: "2px",
    },
    ".cm-gutters": {
      backgroundColor: "#0b1018",
      borderRight: "1px solid #202938",
      color: "#9ca8ba",
    },
    ".cm-gutterElement": {
      paddingLeft: "10px",
      paddingRight: "10px",
    },
    ".cm-scroller": {
      fontFamily:
        'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Monaco, Consolas, "Liberation Mono", monospace',
      fontSize: "13px",
      lineHeight: "1.55",
      scrollBehavior: "smooth",
    },
    ".cm-activeLine": {
      backgroundColor: "#121927",
    },
    ".cm-activeLineGutter": {
      backgroundColor: "#121927",
      color: "#f8fafc",
    },
    ".cm-compact-move-address": {
      color: "inherit",
      textDecoration: "underline dotted #94a3b8",
      textUnderlineOffset: "3px",
    },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
      backgroundColor: "#24496f",
    },
    ".cm-content ::selection": {
      backgroundColor: "#24496f",
      color: "#f8fafc",
    },
    ".cm-selectionMatch": {
      backgroundColor: "#78350f",
      outline: "1px solid #f59e0b",
    },
    ".cm-line.cm-complexityLine": {
      backgroundColor: "#2b210f",
      boxShadow: "inset 3px 0 0 #f59e0b",
    },
    ".cm-line.cm-complexityLineError": {
      backgroundColor: "#2f1215",
      boxShadow: "inset 3px 0 0 #fb7185",
    },
    ".cm-line.cm-complexityLineStart": {
      borderTopLeftRadius: "4px",
      borderTopRightRadius: "4px",
    },
    ".cm-line.cm-complexityLineEnd": {
      borderBottomLeftRadius: "4px",
      borderBottomRightRadius: "4px",
    },
    ".cm-activeLine.cm-complexityLine": {
      backgroundColor: "#3a2b12",
    },
    ".cm-activeLine.cm-complexityLineError": {
      backgroundColor: "#42181d",
    },
    ".cm-bytecodeSourceSpan": {
      backgroundColor: "#102a43",
      borderBottom: "1px solid #38bdf8",
      borderRadius: "2px",
      transition: "background-color 220ms ease, box-shadow 220ms ease, border-color 220ms ease",
      animation: "cmBytecodeSourceSpanEnter 180ms ease-out both",
    },
    ".cm-bytecodeSourceSpanActive": {
      backgroundColor: "#173f63",
      boxShadow: "inset 3px 0 0 #38bdf8, inset 0 -1px 0 #7dd3fc",
    },
    ".cm-bytecodeSourceSpanExit": {
      animation: "cmBytecodeSourceSpanExit 260ms ease-in forwards",
      backgroundColor: "#111827",
      borderBottomColor: "transparent",
      boxShadow: "inset 0 0 0 transparent",
    },
    "@keyframes cmBytecodeSourceSpanEnter": {
      from: {
        backgroundColor: "transparent",
        boxShadow: "inset 0 0 0 transparent",
      },
      to: {
        backgroundColor: "#173f63",
        boxShadow: "inset 3px 0 0 #38bdf8, inset 0 -1px 0 #7dd3fc",
      },
    },
    "@keyframes cmBytecodeSourceSpanExit": {
      from: {
        backgroundColor: "#173f63",
        boxShadow: "inset 3px 0 0 #38bdf8, inset 0 -1px 0 #7dd3fc",
      },
      to: {
        backgroundColor: "transparent",
        boxShadow: "inset 0 0 0 transparent",
      },
    },
    "@media (prefers-reduced-motion: reduce)": {
      ".cm-scroller": {
        scrollBehavior: "auto",
      },
      ".cm-bytecodeSourceSpan": {
        animation: "none",
        transition: "none",
      },
      ".cm-bytecodeSourceSpanExit": {
        animation: "none",
      },
    },
  },
  { dark: true },
);
