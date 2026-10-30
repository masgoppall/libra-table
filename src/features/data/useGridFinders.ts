// Jump to column or row (⌘J) and search and replace within the table (⌥⌘R) of a data
// tab; both move the grid's focus through one `reveal`.

import { cellText } from "@/lib/cells";
import { useJumpSheet, useReveal } from "./JumpSheet";
import { type FindContext, useFindReplace } from "./useFindReplace";

export function useGridFinders(x: Omit<FindContext, "go"> & { rowOffset: number }) {
  const reveal = useReveal();
  const jump = useJumpSheet(reveal.go);
  const find = useFindReplace({ ...x, go: reveal.go });
  const openJump = (focusRow: number | null) =>
    jump.show(
      {
        columns: x.columns,
        visible: x.visible,
        rowCount: x.rowCount,
        rowOffset: x.rowOffset,
        text: (r, c) => cellText(x.getCell(r, c)),
      },
      focusRow,
    );
  return {
    reveal: reveal.reveal,
    isMatch: find.isMatch,
    openJump,
    openFind: () => find.show(x.editable),
    findBar: find.node,
    jumpSheet: jump.node,
  };
}
