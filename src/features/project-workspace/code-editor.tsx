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
  foldGutter,
  indentOnInput,
  StreamLanguage,
  syntaxHighlighting,
} from "@codemirror/language";
import { shell } from "@codemirror/legacy-modes/mode/shell";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import { EditorState, type Extension } from "@codemirror/state";
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import React from "react";

type CodeEditorProps = {
  language: string;
  value: string;
  onChange: (value: string) => void;
};

export function CodeEditor({ language, value, onChange }: CodeEditorProps) {
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

  return <div ref={hostRef} className="min-h-0 flex-1 overflow-hidden" />;
}

function editorExtensions(language: string, onChange: (value: string) => void) {
  return [
    lineNumbers(),
    foldGutter(),
    history(),
    drawSelection(),
    indentOnInput(),
    bracketMatching(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
    keymap.of([indentWithTab, ...defaultKeymap, ...historyKeymap]),
    editorTheme,
    EditorView.lineWrapping,
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
  },
  { dark: true },
);
