const deliverableSections = [
  {
    emptyMessage: "You have no completed slides",
    id: "deliverables-completed",
    showDivider: false,
    title: "Completed Slide(s)",
  },
  {
    emptyMessage: "You have no slides in progress",
    id: "deliverables-in-progress",
    showDivider: true,
    title: "In-progress slides",
  },
] as const;

export function DeliverablesView() {
  return (
    <div className="-mt-2 flex h-full min-h-[600px] flex-col">
      <div className="grid min-h-0 flex-1 grid-rows-2">
        {deliverableSections.map((section) => (
          <section
            aria-labelledby={section.id}
            className="flex min-h-0 flex-col"
            key={section.id}
          >
            <div
              className={`flex items-center justify-between ${
                section.showDivider ? "border-b border-outline-variant pb-3" : "pb-1"
              }`}
            >
              <h2 className="text-sm font-semibold text-text-main" id={section.id}>
                {section.title}
              </h2>
            </div>
            <div className="flex min-h-0 flex-1 items-center justify-center px-4 py-6 text-center">
              <p className="text-sm text-muted">{section.emptyMessage}</p>
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
