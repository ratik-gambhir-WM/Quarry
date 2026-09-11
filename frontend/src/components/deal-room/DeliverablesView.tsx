import { Button } from "../ui/button";
import { Icon } from "../ui/Icon";

const deliverableSections = [
  {
    emptyMessage: "You have no completed slides",
    id: "deliverables-completed",
    title: "Completed Slide(s)",
  },
  {
    emptyMessage: "You have no slides in progress",
    id: "deliverables-in-progress",
    title: "In-progress slides",
  },
  {
    emptyMessage: "You have no templates",
    id: "deliverables-templates",
    title: "Start from templates",
  },
] as const;

export function DeliverablesView() {
  return (
    <div className="flex h-full min-h-[600px] flex-col">
      <h1 className="text-[12px] font-semibold leading-none text-text-main">Deliverables</h1>

      <div className="mt-5 grid min-h-0 flex-1 grid-rows-3">
        {deliverableSections.map((section) => (
          <section
            aria-labelledby={section.id}
            className="flex min-h-0 flex-col"
            key={section.id}
          >
            <div className="flex items-center justify-between border-b border-outline-variant pb-3">
              <h2 className="text-sm font-semibold text-text-main" id={section.id}>
                {section.title}
              </h2>
              {section.id === "deliverables-templates" ? (
                <Button
                  aria-label="Add template"
                  className="text-muted"
                  disabled
                  size="icon-sm"
                  type="button"
                  variant="ghost"
                >
                  <Icon className="h-4 w-4" name="plus" />
                </Button>
              ) : null}
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
