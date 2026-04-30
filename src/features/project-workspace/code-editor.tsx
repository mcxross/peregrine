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
  defaultHighlightStyle,
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
} from "@codemirror/view";
import React from "react";

type CodeEditorProps = {
  complexityHighlights?: ComplexityHighlight[];
  jumpRequest?: CodeEditorJumpRequest | null;
  language: string;
  value: string;
  onChange: (value: string) => void;
};

export type CodeEditorJumpRequest = {
  line: number;
  token: number;
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

const setComplexityHighlights = StateEffect.define<ComplexityHighlight[]>();

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

export function CodeEditor({
  complexityHighlights = EMPTY_COMPLEXITY_HIGHLIGHTS,
  jumpRequest = null,
  language,
  value,
  onChange,
}: CodeEditorProps) {
  const hostRef = React.useRef<HTMLDivElement | null>(null);
  const editorRef = React.useRef<EditorView | null>(null);
  const onChangeRef = React.useRef(onChange);

  React.useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  React.useEffect(() => {
    if (!hostRef.current) {
      return;
    }

    const editor = new EditorView({
      parent: hostRef.current,
      state: EditorState.create({
        doc: value,
        extensions: editorExtensions(language, (nextValue) => {
          onChangeRef.current(nextValue);
        }),
      }),
    });

    editorRef.current = editor;
    editor.dispatch({
      effects: setComplexityHighlights.of(complexityHighlights),
    });

    return () => {
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

    if (!editor || !jumpRequest) {
      return;
    }

    const line = editor.state.doc.line(clampLineNumber(jumpRequest.line, editor.state.doc.lines));

    editor.dispatch({
      selection: { anchor: line.from },
      effects: EditorView.scrollIntoView(line.from, { y: "center" }),
    });
    editor.focus();
  }, [jumpRequest]);

  return <div ref={hostRef} className="min-h-0 flex-1 overflow-hidden" />;
}

function editorExtensions(language: string, onChange: (value: string) => void) {
  return [
    lineNumbers(),
    history(),
    drawSelection(),
    indentOnInput(),
    bracketMatching(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    keymap.of([indentWithTab, ...defaultKeymap, ...historyKeymap]),
    complexityHighlightField,
    editorTheme,
    EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        onChange(update.state.doc.toString());
      }
    }),
    languageExtension(language),
  ];
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

const editorTheme = EditorView.theme(
  {
    "&": {
      backgroundColor: "var(--background)",
      color: "var(--foreground)",
      height: "100%",
    },
    "&.cm-focused": {
      outline: "none",
    },
    ".cm-content": {
      caretColor: "var(--foreground)",
      minHeight: "100%",
      padding: "20px 0",
    },
    ".cm-cursor": {
      borderLeftColor: "var(--foreground)",
    },
    ".cm-gutters": {
      backgroundColor: "var(--background)",
      borderRight: "1px solid var(--border)",
      color: "var(--muted-foreground)",
    },
    ".cm-line": {
      padding: "0 20px",
    },
    ".cm-scroller": {
      fontFamily:
        'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Monaco, Consolas, "Liberation Mono", monospace',
      fontSize: "13px",
      lineHeight: "1.55",
    },
    ".cm-activeLine": {
      backgroundColor: "var(--muted)",
    },
    ".cm-activeLineGutter": {
      backgroundColor: "var(--muted)",
      color: "var(--foreground)",
    },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": {
      backgroundColor: "color-mix(in oklch, var(--primary) 30%, transparent)",
    },
    ".cm-line.cm-complexityLine": {
      backgroundColor: "color-mix(in oklch, var(--chart-5) 13%, transparent)",
      boxShadow:
        "inset 3px 0 0 color-mix(in oklch, var(--chart-5) 82%, transparent)",
    },
    ".cm-line.cm-complexityLineError": {
      backgroundColor: "color-mix(in oklch, var(--destructive) 14%, transparent)",
      boxShadow:
        "inset 3px 0 0 color-mix(in oklch, var(--destructive) 80%, transparent)",
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
      backgroundColor: "color-mix(in oklch, var(--chart-5) 20%, var(--muted))",
    },
    ".cm-activeLine.cm-complexityLineError": {
      backgroundColor: "color-mix(in oklch, var(--destructive) 20%, var(--muted))",
    },
  },
  { dark: true },
);
