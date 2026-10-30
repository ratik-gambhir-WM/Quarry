import { Icon } from "../../ui/Icon";

export type DealStep = "details" | "companies" | "sources";
export type StepperOrientation = "horizontal" | "vertical";

const steps: { id: DealStep; label: string }[] = [
  { id: "details", label: "Deal details" },
  { id: "companies", label: "Companies" },
  { id: "sources", label: "Metadata" },
];

type DealStepperProps = {
  disabled: boolean;
  onStepChange: (step: DealStep) => void;
  orientation: StepperOrientation;
  step: DealStep;
};

export function DealStepper({ disabled, onStepChange, orientation, step }: DealStepperProps) {
  const current = steps.findIndex((item) => item.id === step);
  return (
    <ol aria-label="Add deal progress" className={`deal-stepper deal-stepper--${orientation}`}>
      {steps.map((item, index) => {
        const complete = index < current;
        const label = (
          <>
            <span className={`deal-stepper-indicator ${complete ? "is-complete" : index === current ? "is-current" : ""}`} aria-hidden="true">
              {complete ? <Icon name="check" className="deal-stepper-check h-3.5 w-3.5" /> : index + 1}
            </span>
            <span className="deal-stepper-label">{item.label}</span>
            <span className="sr-only">{complete ? ", completed" : index === current ? ", current step" : ", upcoming"}</span>
          </>
        );
        return (
          <li key={item.id} aria-current={index === current ? "step" : undefined} className="deal-stepper-step">
            {index < steps.length - 1 && <span aria-hidden="true" className={`deal-stepper-rail ${complete ? "is-complete" : ""}`} />}
            {complete ? (
              <button type="button" disabled={disabled} onClick={() => onStepChange(item.id)} className="deal-stepper-target">
                {label}
              </button>
            ) : <span className="deal-stepper-target">{label}</span>}
          </li>
        );
      })}
    </ol>
  );
}
