import { type FormEvent, useRef, useState } from "react";
import {
  SourceFilesStep,
  type SelectedSourceFiles,
} from "../../deal-room/DealSourceFields";
import { X } from "lucide-react";
import { useDealSaveWorkflow, type AddDealFormState } from "./useDealSaveWorkflow";
import { runtime } from "@quarry/runtime";
import type {
  LocalDealDataRoom,
} from "../../../data/dealExtraction";
import { Button } from "../../ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../ui/dialog";
import { Field, FieldError, FieldLabel } from "../../ui/field";
import { Icon } from "../../ui/Icon";
import { Input } from "../../ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../../ui/tabs";
import { DealStepper, type DealStep, type StepperOrientation } from "./DealStepper";
import "./deal-stepper.css";

import {
  DealCompanyRolesEditor,
  DealDetailsEditor,
  DealFieldsEditor,
} from "./DealFieldsEditor";
import type { DealFields } from "../../../data/dealModel";
import type { SavedDeal } from "../../../data/dealExtraction";
import { useWorkspace } from "../../../app/WorkspaceProvider";

type AddDealModalProps = {
  email?: string;
  initialDeal?: SavedDeal;
  onClose: () => void;
};

const emptyForm: AddDealFormState = {
  dealName: "",
  dealCode: null,
  engagementType: "buy_side",
  transactionType: "unknown",
  status: "pipeline",
  startDate: null,
  expectedCloseDate: null,
  actualCloseDate: null,
  engagementCompletedDate: null,
  parties: [],
  scopes: [],
  factSheetLink: "",
  localPath: "",
  rlLink: "",
  sharepointLink: "",
  sowLink: "",
};

const emptySourceFiles: SelectedSourceFiles = {
  projectTimelineFile: null,
  sowFile: null,
};

export function AddDealModal({
  email,
  onClose,
  initialDeal,
}: AddDealModalProps) {
  const returnFocusRef = useRef(
    document.activeElement instanceof HTMLElement &&
      document.activeElement !== document.body
      ? document.activeElement
      : null,
  );
  const { saveCompanyResult } = useWorkspace();
  const [form, setForm] = useState<AddDealFormState>(() =>
    initialDeal ? { ...emptyForm, ...initialDeal } : emptyForm,
  );
  const [localDataRoom, setLocalDataRoom] = useState<LocalDealDataRoom | null>(
    null,
  );
  const [teamName, setTeamName] = useState("");
  const [selectedSourceFiles, setSelectedSourceFiles] =
    useState(emptySourceFiles);
  const [step, setStep] = useState<DealStep>(
    "details",
  );
  const [stepperOrientation, setStepperOrientation] = useState<StepperOrientation>("horizontal");
  const [companyFormOpen, setCompanyFormOpen] = useState(false);
  const formRef = useRef<HTMLFormElement>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [fieldError, setFieldError] = useState("");
  const workflow = useDealSaveWorkflow({
    email, initialDeal, form, teamName, selectedSourceFiles, localDataRoom, onClose,
    onCoreFailure: () => changeStep("companies"),
  });
  const { createdDeal, isSubmitting, error: submitError } = workflow;

  function updateField(field: keyof AddDealFormState, value: string) {
    setForm((current) => ({ ...current, [field]: value }));
    setFieldError("");
    workflow.clearError();
  }

  async function chooseLocalFolder() {
    setFieldError("");
    try {
      const selection = await runtime.platform.selectDealDataRoom();
      if (selection) {
        setLocalDataRoom(selection);
        updateField("localPath", selection.rootPath);
        setSelectedSourceFiles(emptySourceFiles);
      }
    } catch (error) {
      setFieldError(errorMessage(error));
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (isSubmitting || companyFormOpen) return;
    if (initialDeal) {
      await saveDeal(false);
    } else if (step === "details") {
      changeStep("companies");
    } else if (step === "companies") {
      changeStep("team");
    } else if (step === "team") {
      changeStep("sources");
    } else {
      await saveDeal(true);
    }
  }

  function changeStep(next: typeof step) {
    setStep(next);
    workflow.clearError();
    window.requestAnimationFrame(() => {
      titleRef.current?.focus();
      if (scrollRef.current) scrollRef.current.scrollTop = 0;
    });
  }

  function updateDealFields(fields: DealFields) {
    setForm((current) => ({ ...current, ...fields }));
    workflow.clearError();
  }

  async function saveDeal(includeMetadata: boolean) {
    if (!email) {
      setFieldError("Sign in again before adding a deal.");
      return;
    }
    if (!form.transactionType) {
      setFieldError("Select a transaction type.");
      return;
    }
    await workflow.save(includeMetadata);
  }

  return (
    <Dialog
      onOpenChange={(open) => {
        if (!open && !isSubmitting) onClose();
      }}
      open
    >
      <DialogContent
        className="deal-dialog flex max-h-[calc(100dvh-2rem)] flex-col gap-0 overflow-hidden rounded-2xl border border-outline-variant bg-surface-container-lowest p-0 shadow-xl sm:max-w-[720px]"
        onOpenAutoFocus={(event) => {
          const input = formRef.current?.querySelector<HTMLInputElement>("input:not(:disabled)");
          if (input) {
            event.preventDefault();
            input.focus();
          }
        }}
        onCloseAutoFocus={(event) => {
          if (returnFocusRef.current?.isConnected) {
            event.preventDefault();
            returnFocusRef.current.focus();
          }
        }}
        onEscapeKeyDown={(event) => {
          if (isSubmitting) event.preventDefault();
        }}
        onPointerDownOutside={(event) => {
          if (isSubmitting) event.preventDefault();
        }}
        showCloseButton={false}
      >
        <form
          ref={formRef}
          className="flex max-h-[calc(100dvh-2rem)] min-h-0 flex-col"
          onSubmit={handleSubmit}
        >
          <DialogHeader className="relative shrink-0 gap-1 px-5 pb-1 pt-5 pr-14 sm:px-6 sm:pt-6">
            <DialogDescription className="order-first text-xs font-medium uppercase tracking-[0.12em] text-muted">
              {initialDeal
                ? "Active Deals"
                : `Step ${step === "details" ? 1 : step === "companies" ? 2 : step === "team" ? 3 : 4} of 4`}
            </DialogDescription>
            <DialogTitle
              ref={titleRef}
              tabIndex={-1}
              className="text-lg font-semibold tracking-tight text-text-main"
            >
              {initialDeal
                ? "Edit deal"
                : step === "sources"
                  ? "Add deal metadata"
                  : step === "team"
                    ? "Create team"
                    : step === "companies"
                      ? "Assign companies and roles"
                      : "Add deal"}
            </DialogTitle>
          </DialogHeader>

          <Tabs
            value={stepperOrientation}
            onValueChange={(value) => {
              if (value === "horizontal" || value === "vertical") setStepperOrientation(value);
            }}
            className="flex min-h-0 flex-1 flex-col"
          >
            {!initialDeal && (
              <div className="flex justify-end px-5 pt-2 sm:px-6">
                <TabsList aria-label="Stepper position" className="rounded-lg bg-surface-container-low p-0.5">
                  <TabsTrigger value="horizontal" className="rounded-md px-3 py-1 text-xs font-medium data-[state=active]:bg-surface-container-lowest data-[state=active]:shadow-sm">Top</TabsTrigger>
                  <TabsTrigger value="vertical" className="rounded-md px-3 py-1 text-xs font-medium data-[state=active]:bg-surface-container-lowest data-[state=active]:shadow-sm">Side</TabsTrigger>
                </TabsList>
              </div>
            )}
            <div className={`flex min-h-0 flex-1 ${stepperOrientation === "vertical" && !initialDeal ? "flex-row" : "flex-col"}`}>
              {!initialDeal && (
                <>
                  <TabsContent value="horizontal" tabIndex={-1} className="shrink-0 px-5 pb-1 pt-3 sm:px-6">
                    <DealStepper step={step} orientation="horizontal" onStepChange={changeStep} disabled={isSubmitting || companyFormOpen || Boolean(createdDeal)} />
                  </TabsContent>
                  <TabsContent value="vertical" tabIndex={-1} className="w-28 shrink-0 border-r border-outline-variant px-3 pt-4 sm:w-44 sm:px-6">
                    <DealStepper step={step} orientation="vertical" onStepChange={changeStep} disabled={isSubmitting || companyFormOpen || Boolean(createdDeal)} />
                  </TabsContent>
                </>
              )}
          <div ref={scrollRef} className="min-h-0 min-w-0 flex-1 overflow-y-auto px-5 py-4 sm:px-6">
            <div hidden={step !== "details"}>
              <fieldset
                className="grid gap-4"
                disabled={isSubmitting || step !== "details"}
              >
                {initialDeal ? (
                  <DealFieldsEditor
                    onCompanyFormChange={setCompanyFormOpen}
                    onCompanySaved={saveCompanyResult}
                    userEmail={email ?? ""}
                    fields={form}
                    initialCompanies={initialDeal.parties.map((p) => p.company)}
                    onChange={updateDealFields}
                  />
                ) : (
                  <DealDetailsEditor
                    fields={form}
                    onChange={updateDealFields}
                  />
                )}
                {initialDeal && companyFormOpen ? (
                  <p className="text-sm text-muted-foreground">
                    Save or cancel the company before continuing.
                  </p>
                ) : null}
                {!initialDeal && runtime.target === "desktop" ? (
                  <LocalFolderField
                    disabled={isSubmitting}
                    error={fieldError}
                    onChoose={chooseLocalFolder}
                    value={form.localPath}
                  />
                ) : null}
              </fieldset>
            </div>
            {!initialDeal ? (
              <div hidden={step !== "companies"}>
                <fieldset disabled={isSubmitting || step !== "companies"}>
                  <DealCompanyRolesEditor
                    onCompanyFormChange={setCompanyFormOpen}
                    onCompanySaved={saveCompanyResult}
                    userEmail={email ?? ""}
                    fields={form}
                    onChange={updateDealFields}
                  />
                  {companyFormOpen ? (
                    <p className="mt-3 text-sm text-muted-foreground">
                      Save or cancel the company before continuing.
                    </p>
                  ) : null}
                </fieldset>
              </div>
            ) : null}
            {step === "team" ? (
              <fieldset className="space-y-4" disabled={isSubmitting}>
                <p className="text-sm text-muted">
                  Name the team for this deal. You can add members in deal Settings after creation.
                  Each deal can have one team. Team assignments do not grant deal access.
                </p>
                <Field>
                  <FieldLabel htmlFor="add-deal-team-name">Team name (optional)</FieldLabel>
                  <Input
                    id="add-deal-team-name"
                    maxLength={100}
                    value={teamName}
                    onChange={(event) => setTeamName(event.target.value)}
                    aria-describedby="add-deal-team-help"
                  />
                  <p id="add-deal-team-help" className="text-sm text-muted">
                    Leave blank to set up a team later in Settings.
                  </p>
                </Field>
              </fieldset>
            ) : null}
            {step === "sources" ? (
              <fieldset disabled={isSubmitting}>
                <SourceFilesStep
                  availableFiles={localDataRoom?.files ?? null}
                  form={form}
                  onChange={(field, file) => {
                    setSelectedSourceFiles((current) => ({
                      ...current,
                      [field]: file,
                    }));
                    workflow.clearError();
                  }}
                  onUpdateField={updateField}
                  selected={selectedSourceFiles}
                />
              </fieldset>
            ) : null}

            {isSubmitting ? <p className="sr-only" role="status">
              {workflow.stage === "core" ? "Saving deal…" : workflow.stage === "team" ? "Saving team…" : "Saving metadata…"}
            </p> : null}
            {fieldError ? <p role="alert" className="mt-3 text-sm text-error">{fieldError}</p> : null}
            {submitError ? (
              <p
                className="mt-4 rounded-lg border border-error/25 bg-error/8 px-3 py-2 text-sm font-medium text-error"
                role="alert"
              >
                {submitError}
              </p>
            ) : null}
          </div>
            </div>
          </Tabs>

          <DialogFooter className="mx-0 mb-0 shrink-0 rounded-none border-outline-variant bg-surface-container-low/60 px-5 py-4 sm:px-6">
            {step === "details" ? (
              <DialogClose asChild>
                <Button disabled={isSubmitting} type="button" variant="outline">
                  Cancel
                </Button>
              </DialogClose>
            ) : (
              <Button
                disabled={
                  isSubmitting || companyFormOpen || Boolean(createdDeal)
                }
                onClick={() =>
                  changeStep(step === "sources" ? "team" : step === "team" ? "companies" : "details")
                }
                type="button"
                variant="outline"
              >
                Back
              </Button>
            )}
            {step === "sources" ? (
              <Button
                disabled={isSubmitting}
                onClick={() => void saveDeal(false)}
                type="button"
                variant="outline"
              >
                Skip metadata
              </Button>
            ) : null}
            <Button
              className="min-w-28"
              disabled={isSubmitting || companyFormOpen}
              type="submit"
            >
              {isSubmitting ? (
                <span
                  aria-hidden="true"
                  className="size-3.5 rounded-full border-2 border-on-action/30 border-t-on-action motion-safe:animate-spin"
                />
              ) : null}
              {isSubmitting
                ? "Saving..."
                : initialDeal
                  ? "Save deal"
                  : step === "sources"
                    ? "Finish deal"
                    : "Next"}
            </Button>
          </DialogFooter>
        </form>
        <DialogClose asChild>
          <Button
            aria-label={
              initialDeal ? "Close edit deal dialog" : "Close add deal dialog"
            }
            className="absolute right-4 top-4"
            disabled={isSubmitting}
            size="icon-sm"
            type="button"
            variant="ghost"
          >
            <X aria-hidden="true" />
          </Button>
        </DialogClose>
      </DialogContent>
    </Dialog>
  );
}

function LocalFolderField({
  disabled,
  error,
  onChoose,
  value,
}: {
  disabled: boolean;
  error: string;
  onChoose: () => void;
  value: string;
}) {
  return (
    <Field data-invalid={Boolean(error)}>
      <div className="flex items-center justify-between">
        <FieldLabel htmlFor="add-deal-local-path">
          Local data room folder
        </FieldLabel>
        <span className="text-xs text-muted-foreground">Optional</span>
      </div>
      <div className="flex gap-3">
        <Input
          aria-invalid={Boolean(error)}
          className="min-w-0 flex-1"
          id="add-deal-local-path"
          placeholder="Choose a folder"
          readOnly
          value={value}
        />
        <Button
          disabled={disabled}
          onClick={onChoose}
          type="button"
          variant="outline"
        >
          <Icon className="h-4 w-4" name="folderOpen" /> Browse
        </Button>
      </div>
      <FieldError>{error}</FieldError>
    </Field>
  );
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
