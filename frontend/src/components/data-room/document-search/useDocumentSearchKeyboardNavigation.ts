import { useState } from "react";

type SelectionOrigin = "init" | "keyboard" | "pointer";

export function useDocumentSearchKeyboardNavigation(itemIds: string[]) {
  const itemKey = itemIds.join("\u0000");
  const [selection, setSelection] = useState({
    itemKey,
    index: itemIds.length > 0 ? 0 : -1,
    origin: "init" as SelectionOrigin,
  });
  const selectedIndex = selection.itemKey === itemKey
    ? selection.index
    : itemIds.length > 0 ? 0 : -1;
  const selectionOrigin = selection.itemKey === itemKey ? selection.origin : "init";

  function move(direction: 1 | -1) {
    const index = itemIds.length
      ? ((selectedIndex >= 0 ? selectedIndex : 0) + direction + itemIds.length) % itemIds.length
      : -1;
    setSelection({ itemKey, index, origin: "keyboard" });
  }

  function hoverIndex(index: number) {
    if (index >= 0 && index < itemIds.length) {
      setSelection({ itemKey, index, origin: "pointer" });
    }
  }

  return {
    hoverIndex,
    moveDown: () => move(1),
    moveUp: () => move(-1),
    selectedIndex,
    selectionOrigin,
  };
}
