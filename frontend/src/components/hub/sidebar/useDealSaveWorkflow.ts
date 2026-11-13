import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import { useWorkspace } from "../../../app/WorkspaceProvider";
import {
  isLocalDealSourceFile,
  type SourceFileSelection,
  type SelectedSourceFiles,
} from "../../deal-room/DealSourceFields";
import type { DealFields } from "../../../data/dealModel";
import type {
  DealExtractionLocationState,
  LocalDealDataRoom,
  LocalDealFileContents,
  SaveDealInput,
  SaveDealResponse,
  SavedDeal,
} from "../../../data/dealExtraction";

export type AddDealFormState = DealFields & {
  factSheetLink: string;
  localPath: string;
  rlLink: string;
  sharepointLink: string;
  sowLink: string;
};

type SaveStage = "idle" | "core" | "team" | "metadata" | "complete" | "error";

export function useDealSaveWorkflow({
  email, initialDeal, form, teamName, selectedSourceFiles, localDataRoom, onClose, onCoreFailure,
}: {
  email?: string;
  initialDeal?: SavedDeal;
  form: AddDealFormState;
  teamName: string;
  selectedSourceFiles: SelectedSourceFiles;
  localDataRoom: LocalDealDataRoom | null;
  onClose: () => void;
  onCoreFailure: () => void;
}) {
  const loadedDeal = useRef(initialDeal).current;
  const navigate = useNavigate();
  const { saveDealResult } = useWorkspace();
  const [progress, setProgress] = useState<{ stage: SaveStage; error: string }>({ stage: "idle", error: "" });
  const [createdDeal, setCreatedDeal] = useState<SaveDealResponse | null>(null);
  const createdDealRef = useRef<SaveDealResponse | null>(null);
  const teamSaved = useRef(false);
  const teamCreationAttempted = useRef(false);
  const submissionInFlight = useRef(false);
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  async function saveDeal(includeMetadata: boolean) {
    if (!mounted.current || submissionInFlight.current) return;
    submissionInFlight.current = true;
    setProgress({ stage: "core", error: "" });
    let savedCore = createdDealRef.current;
    let failedStage: "core" | "team" | "metadata" = "core";
    try {
      if (!savedCore) {
        const input = buildSaveDealInput(form);
        savedCore = loadedDeal
          ? await runtime.api.updateDeal(
              loadedDeal.dealId,
              { ...editableFields(input), expectedRevision: loadedDeal.revision },
            )
          : await runtime.api.createDeal(input);
        if (!mounted.current) return;
        saveDealResult(savedCore);
        if (!initialDeal) {
          createdDealRef.current = savedCore;
          setCreatedDeal(savedCore);
        }
      }
      if (initialDeal) {
        onClose();
        return;
      }
      failedStage = "team";
      if (teamName.trim() && !teamSaved.current) {
        setProgress({ stage: "team", error: "" });
        await reconcileTeam(savedCore.deal.dealId);
      }
      if (!mounted.current) return;
      failedStage = "metadata";
      setProgress({ stage: "metadata", error: "" });
      if (includeMetadata) await saveSources(savedCore);
      else finishWithoutMetadata(savedCore);
    } catch (error) {
      if (!mounted.current) return;
      if (savedCore && !initialDeal) {
        setProgress({
          stage: "error",
          error: failedStage === "team"
            ? `Deal created, but the team could not be saved. Retry Finish deal. ${errorMessage(error)}`
            : `Deal created, but metadata could not be saved. Retry Finish deal or skip metadata. ${errorMessage(error)}`,
        });
      } else {
        if (!initialDeal) onCoreFailure();
        setProgress({ stage: "error", error: errorMessage(error) });
      }
    } finally {
      submissionInFlight.current = false;
      if (mounted.current) setProgress((current) =>
        current.stage === "error" ? current : { stage: "complete", error: "" });
    }
  }

  async function reconcileTeam(dealId: string) {
    if (!teamName.trim() || teamSaved.current) return;
    // A failed response can follow a committed create. Reconcile before replaying it.
    if (teamCreationAttempted.current) {
      const teams = await runtime.api.listDealTeams(dealId, { limit: 100 });
      if (!mounted.current) return;
      const normalizedName = teamName.trim().replace(/\p{White_Space}+/gu, " ").trim();
      teamSaved.current = teams.items.some((team) => team.name === normalizedName);
      if (!teamSaved.current && teams.items.length > 0) {
        throw new Error(
          "This deal already has a different team. Close this dialog and review it in Settings.",
        );
      }
    }
    if (!teamSaved.current) {
      teamCreationAttempted.current = true;
      await runtime.api.createDealTeam(dealId, { name: teamName.trim() });
      if (!mounted.current) return;
      teamSaved.current = true;
    }
  }

  async function saveSources(core: SaveDealResponse) {
    const selections = [
      selectedSourceFiles.sowFile,
      selectedSourceFiles.projectTimelineFile,
    ].filter((file): file is SourceFileSelection => file !== null);
    const uploads =
      localDataRoom && selections.length > 0
        ? (
            await runtime.platform.readDealSourceFiles({
              paths: selections
                .filter(isLocalDealSourceFile)
                .map((file) => file.path),
              rootPath: localDataRoom.rootPath,
            })
          ).map(localFileContentsToFile)
        : selections.filter((file): file is File => file instanceof File);
    if (!mounted.current) return;
    const response = await runtime.api.saveDealMetadata(core.deal.dealId, {
      expectedRevision: core.metadata.revision,
      factSheetLink: optionalLink(form.factSheetLink),
      files: uploads,
      rlLink: optionalLink(form.rlLink),
      sharepointLink: optionalLink(form.sharepointLink),
      sowLink: optionalLink(form.sowLink),
    });
    if (!mounted.current) return;
    saveDealResult(response, sourceFileName(selectedSourceFiles.sowFile));
    navigate(`/hub/deals/${encodeURIComponent(response.deal.dealId)}`, {
      state: {
        email,
        result: response,
        sowSourceName: sourceFileName(selectedSourceFiles.sowFile),
      } satisfies DealExtractionLocationState,
    });
  }

  function finishWithoutMetadata(core: SaveDealResponse) {
    navigate(`/hub/deals/${encodeURIComponent(core.deal.dealId)}`, {
      state: {
        email,
        result: {
          ...core,
          extraction: { keyQuestions: [] },
          files: [],
        },
      } satisfies DealExtractionLocationState,
    });
  }

  return {
    createdDeal,
    stage: progress.stage,
    error: progress.error,
    isSubmitting: progress.stage === "core" || progress.stage === "team" || progress.stage === "metadata",
    clearError: () => setProgress((current) => ({ ...current, error: "" })),
    save: saveDeal,
  };
}

function editableFields(form: DealFields): DealFields {
  return {
    dealName: form.dealName.trim(),
    dealCode: form.dealCode?.trim() || null,
    engagementType: form.engagementType,
    transactionType: form.transactionType,
    status: form.status,
    startDate: form.startDate,
    expectedCloseDate: form.expectedCloseDate,
    actualCloseDate: form.actualCloseDate,
    engagementCompletedDate: form.engagementCompletedDate,
    parties: form.parties.map(({ companyId, role, notes }) => ({
      companyId,
      role,
      notes,
    })),
    scopes: form.scopes,
  };
}
function buildSaveDealInput(form: AddDealFormState): SaveDealInput {
  return {
    ...editableFields(form),
    localPath:
      runtime.target === "desktop" ? form.localPath.trim() || null : null,
    sharepointLink: null,
  };
}

function optionalLink(value: string) {
  return value.trim() || null;
}

function sourceFileName(file: SourceFileSelection | null) {
  if (!file) return undefined;
  return isLocalDealSourceFile(file) ? file.filename : file.name;
}

function localFileContentsToFile(file: LocalDealFileContents) {
  const bytes = Uint8Array.from(atob(file.dataBase64), (character) =>
    character.charCodeAt(0),
  );
  return new File([bytes], file.filename, { type: file.mimeType });
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
