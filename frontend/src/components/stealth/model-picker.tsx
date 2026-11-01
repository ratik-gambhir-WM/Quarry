// Adapted from https://stealth.pm/ui/model-picker (MIT; see ./LICENSE).
import { Select } from "@base-ui/react/select";
import { Check, ChevronDown } from "lucide-react";
import { isChatModelValid, type ChatModelSelection } from "../chat/chatModel";

const choices = [
  { value: "default", label: "Server default", description: "Use the model configured on the server" },
  { value: "gpt-5.5", label: "GPT-5.5", description: "Request gpt-5.5 for this response" },
  { value: "custom", label: "Custom model ID", description: "Use a model available to your server account" },
];

export function ModelPicker({ value, onValueChange, disabled }: {
  value: ChatModelSelection;
  onValueChange(value: ChatModelSelection): void;
  disabled: boolean;
}) {
  const selected = value.type === "default" ? "default" : value.id === "gpt-5.5" ? "gpt-5.5" : "custom";
  return (
    <div className="flex min-w-0 flex-col items-end gap-1.5">
      <Select.Root
        value={selected}
        onValueChange={(next) => {
          if (next === "default") onValueChange({ type: "default" });
          else if (next === "gpt-5.5") onValueChange({ type: "model", id: "gpt-5.5" });
          else if (next === "custom") onValueChange({ type: "model", id: "" });
        }}
        items={choices}
        disabled={disabled}
      >
        <Select.Trigger
          aria-label="Model"
          className="group/model inline-flex h-8 max-w-full items-center gap-1.5 rounded-[8px] px-2 text-xs font-medium text-muted outline-none transition-[background-color,color] hover:bg-surface-container-high hover:text-on-surface focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-50 pointer-coarse:h-11 motion-reduce:transition-none"
        >
          <Select.Value key={selected} className="stealth-action-enter truncate" />
          <Select.Icon className="transition-transform duration-200 group-data-popup-open/model:rotate-180 motion-reduce:transition-none">
            <ChevronDown className="size-3.5" />
          </Select.Icon>
        </Select.Trigger>
        <Select.Portal>
          <Select.Positioner side="top" align="end" sideOffset={6} alignItemWithTrigger={false} collisionPadding={8} className="z-[70]">
            <Select.Popup className="w-64 max-w-[var(--available-width)] origin-[var(--transform-origin)] rounded-[12px] border border-outline-variant bg-surface-container-lowest p-1 text-on-surface shadow-lg outline-none transition-[opacity,scale,translate] duration-150 data-starting-style:translate-y-1 data-starting-style:scale-95 data-starting-style:opacity-0 data-ending-style:opacity-0 motion-reduce:transition-none">
              <Select.List>
                {choices.map((choice) => (
                  <Select.Item key={choice.value} value={choice.value} className="grid cursor-default grid-cols-[16px_1fr] gap-2 rounded-[8px] px-2 py-2 outline-none data-highlighted:bg-surface-container-high">
                    <Select.ItemIndicator><Check className="stealth-action-enter size-3.5" /></Select.ItemIndicator>
                    <span className="col-start-2 flex flex-col gap-0.5">
                      <Select.ItemText className="text-[13px] font-medium">{choice.label}</Select.ItemText>
                      <span className="text-xs text-muted">{choice.description}</span>
                    </span>
                  </Select.Item>
                ))}
              </Select.List>
            </Select.Popup>
          </Select.Positioner>
        </Select.Portal>
      </Select.Root>
      {selected === "custom" && value.type === "model" ? (
        <label className="flex max-w-full flex-col gap-1 text-[11px] text-muted">
          Custom model ID
          <input
            aria-invalid={!isChatModelValid(value)}
            className="h-8 w-40 max-w-full rounded-[6px] border border-outline-variant bg-transparent px-2 text-xs text-on-surface outline-none focus-visible:ring-2 focus-visible:ring-primary"
            value={value.id}
            onChange={(event) => onValueChange({ type: "model", id: event.target.value })}
            disabled={disabled}
            maxLength={128}
            autoComplete="off"
            spellCheck={false}
            placeholder="Enter a model ID"
          />
        </label>
      ) : null}
    </div>
  );
}
