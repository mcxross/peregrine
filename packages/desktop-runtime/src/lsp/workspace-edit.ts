import type {
  MoveAnalyzerPosition,
  MoveAnalyzerTextEdit,
} from "./types";

export function applyMoveAnalyzerTextEdits(
  source: string,
  edits: MoveAnalyzerTextEdit[],
) {
  return [...edits]
    .sort((left, right) => {
      const leftOffset = lspPositionToStringOffset(source, left.range.start);
      const rightOffset = lspPositionToStringOffset(source, right.range.start);

      return rightOffset - leftOffset;
    })
    .reduce((nextSource, edit) => {
      const from = lspPositionToStringOffset(nextSource, edit.range.start);
      const rawTo = lspPositionToStringOffset(nextSource, edit.range.end);
      const to = Math.max(from, rawTo);

      return `${nextSource.slice(0, from)}${edit.newText}${nextSource.slice(to)}`;
    }, source);
}

function lspPositionToStringOffset(
  source: string,
  position: MoveAnalyzerPosition,
) {
  const targetLine = Math.max(0, Math.floor(position.line));
  const targetCharacter = Math.max(0, Math.floor(position.character));
  let line = 0;
  let offset = 0;

  while (line < targetLine && offset < source.length) {
    const nextLineBreak = source.indexOf("\n", offset);

    if (nextLineBreak === -1) {
      return source.length;
    }

    offset = nextLineBreak + 1;
    line += 1;
  }

  const lineEnd = source.indexOf("\n", offset);
  const end = lineEnd === -1 ? source.length : lineEnd;

  return Math.min(end, offset + targetCharacter);
}
