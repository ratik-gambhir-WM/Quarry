// Adapted from https://stealth.pm/ui/model-picker (MIT; see ./LICENSE).
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../ui/select";
import { Input } from "../ui/input";
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
      <Select
        value={selected}
        onValueChange={(next) => {
          if (next === "default") onValueChange({ type: "default" });
          else if (next === "gpt-5.5") onValueChange({ type: "model", id: "gpt-5.5" });
          else if (next === "custom") onValueChange({ type: "model", id: "" });
        }}
        disabled={disabled}
      >
        <SelectTrigger
          aria-label="Model"
          className="group/model inline-flex h-8 max-w-full items-center gap-1.5 rounded-[8px] border-0 px-2 text-xs font-medium text-muted outline-none transition-[background-color,color] hover:bg-surface-container-high hover:text-on-surface focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-50 pointer-coarse:h-11 motion-reduce:transition-none [&>svg]:size-3.5"
        >
          <SelectValue key={selected} className="stealth-action-enter truncate">
            {choices.find((choice) => choice.value === selected)?.label}
          </SelectValue>
        </SelectTrigger>
        <SelectContent side="top" align="end" sideOffset={6} position="popper" collisionPadding={8} className="z-[70] w-64 rounded-[12px] border border-outline-variant bg-surface-container-lowest p-1 text-on-surface shadow-lg">
                {choices.map((choice) => (
                  <SelectItem key={choice.value} value={choice.value} textValue={choice.label} className="cursor-default rounded-[8px] py-2">
                    <span className="flex flex-col gap-0.5">
                      <span className="text-[13px] font-medium">{choice.label}</span>
                      <span className="text-xs text-muted">{choice.description}</span>
                    </span>
                  </SelectItem>
                ))}
        </SelectContent>
      </Select>
      {selected === "custom" && value.type === "model" ? (
        <label className="flex max-w-full flex-col gap-1 text-[11px] text-muted">
          Custom model ID
          <Input
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
