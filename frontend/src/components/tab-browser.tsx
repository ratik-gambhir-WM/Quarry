// Adapted from Movin's tab-browser registry component for controlled document tabs.
import { useEffect, useRef, type KeyboardEvent, type ReactNode } from "react";
import { FileText, Plus, X } from "lucide-react";
import "./tab-browser.css";

export type BrowserTab = { id: string; label: string };

type BrowserTabsProps = {
  tabs: BrowserTab[];
  selectedId: string;
  id: string;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onAdd: () => void;
  children: ReactNode;
};

export function BrowserTabs({
  tabs,
  selectedId,
  id,
  onSelect,
  onClose,
  onAdd,
  children,
}: BrowserTabsProps) {
  const buttons = useRef(new Map<string, HTMLButtonElement>());
  const addButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    buttons.current
      .get(selectedId)
      ?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [selectedId]);

  function close(tabId: string) {
    const index = tabs.findIndex((tab) => tab.id === tabId);
    const remaining = tabs.filter((tab) => tab.id !== tabId);
    const next =
      tabId === selectedId
        ? remaining[Math.min(index, remaining.length - 1)]
        : remaining.find((tab) => tab.id === selectedId);
    onClose(tabId);
    if (next) buttons.current.get(next.id)?.focus();
    else addButton.current?.focus();
  }

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const next = {
      ArrowRight: (index + 1) % tabs.length,
      ArrowLeft: (index + tabs.length - 1) % tabs.length,
      Home: 0,
      End: tabs.length - 1,
    }[event.key];
    if (event.key === "Delete") {
      event.preventDefault();
      close(tabs[index].id);
    } else if (next !== undefined) {
      event.preventDefault();
      onSelect(tabs[next].id);
      buttons.current.get(tabs[next].id)?.focus();
    }
  }

  return (
    <div className="tabs-browser">
      <div className="tabs-browser__strip">
        <div
          className="tabs-browser__list"
          role="tablist"
          aria-label="Open documents"
        >
          {tabs.map((tab, index) => (
            <div
              className="tabs-browser__item"
              data-selected={selectedId === tab.id}
              key={tab.id}
              role="presentation"
            >
              <button
                ref={(element) => {
                  if (element) buttons.current.set(tab.id, element);
                  else buttons.current.delete(tab.id);
                }}
                className="tabs-browser__tab"
                type="button"
                role="tab"
                id={`${id}-tab-${index}`}
                aria-selected={selectedId === tab.id}
                aria-controls={`${id}-panel-${index}`}
                tabIndex={selectedId === tab.id ? 0 : -1}
                onClick={() => onSelect(tab.id)}
                onKeyDown={(event) => onKeyDown(event, index)}
                title={tab.label}
              >
                <FileText size={14} aria-hidden="true" />
                <span className="tabs-browser__name">{tab.label}</span>
              </button>
              <button
                className="tabs-browser__close"
                type="button"
                aria-label={`Close ${tab.label}`}
                onClick={() => close(tab.id)}
              >
                <X size={12} aria-hidden="true" />
              </button>
            </div>
          ))}
        </div>
        <button
          ref={addButton}
          className="tabs-browser__add"
          type="button"
          aria-label="Add document tab"
          title="Add document tab"
          onClick={onAdd}
        >
          <Plus size={16} aria-hidden="true" />
        </button>
      </div>
      {tabs.map((tab, index) => (
        <div
          className="tabs-browser__page"
          role="tabpanel"
          id={`${id}-panel-${index}`}
          aria-labelledby={`${id}-tab-${index}`}
          hidden={selectedId !== tab.id}
          key={tab.id}
          tabIndex={0}
        >
          {selectedId === tab.id ? children : null}
        </div>
      ))}
    </div>
  );
}
