import { useCallback, useEffect, useState } from "react";

type SelectionOrigin = "init" | "keyboard" | "pointer";

export function useDocumentSearchKeyboardNavigation(itemIds: string[]) {
  const [selectedIndex, setSelectedIndex] = useState(itemIds.length > 0 ? 0 : -1);
  const [selectionOrigin, setSelectionOrigin] = useState<SelectionOrigin>("init");
  const itemKey = itemIds.join("\u0000");

  useEffect(() => {
    setSelectedIndex(itemIds.length > 0 ? 0 : -1);
    setSelectionOrigin("init");
  }, [itemIds.length, itemKey]);

  const move = useCallback(
    (direction: 1 | -1) => {
      if (itemIds.length === 0) {
        setSelectedIndex(-1);
        return;
      }
      setSelectedIndex((current) => {
        const safeCurrent = current >= 0 ? current : 0;
        return (safeCurrent + direction + itemIds.length) % itemIds.length;
      });
      setSelectionOrigin("keyboard");
    },
    [itemIds.length],
  );

  const hoverIndex = useCallback(
    (index: number) => {
      if (index < 0 || index >= itemIds.length) {
        return;
      }
      setSelectedIndex(index);
      setSelectionOrigin("pointer");
    },
    [itemIds.length],
  );

  return {
    hoverIndex,
    moveDown: () => move(1),
    moveUp: () => move(-1),
    selectedIndex,
    selectionOrigin,
  };
}
