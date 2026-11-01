import { useState } from "react";
import { ConnectSharePointModal } from "../components/data-room/ConnectSharePointModal";
import { DataRoomWorkspace } from "../components/data-room/DataRoomWorkspace";
import { UploadFilesModal } from "../components/data-room/UploadFilesModal";
import { getDealRoomPath } from "../data/workspace";
import { useDataRoomContents } from "../hooks/useDataRoomContents";
import { useDealRoom } from "./DealRoomPage";

type ActiveModal = "none" | "sharepoint" | "upload";

export function DataRoomPage() {
  const { deal, email, navigationState } = useDealRoom();
  const contents = useDataRoomContents(
    deal.room.id,
    Boolean(deal.persisted?.capabilities?.localSource),
  );
  const [activeModal, setActiveModal] = useState<ActiveModal>("none");

  function closeUploadModal() {
    setActiveModal("none");
    contents.reloadStored();
  }

  return (
    <>
      <DataRoomWorkspace
        contents={contents}
        dealId={deal.room.id}
        dealName={deal.room.name}
        dealRoomPath={getDealRoomPath(deal.room.id)}
        email={email}
        key={deal.room.id}
        navigationState={navigationState}
        onConnectToSharePoint={
          deal.persisted?.capabilities?.localSource
            ? () => setActiveModal("sharepoint")
            : undefined
        }
        onUploadFiles={
          deal.persisted?.capabilities?.upload
            ? () => setActiveModal("upload")
            : undefined
        }
      />
      {activeModal === "upload" ? (
        <UploadFilesModal
          dealId={deal.room.id}
          onClose={closeUploadModal}
          userId={email ?? ""}
        />
      ) : null}
      {activeModal === "sharepoint" ? (
        <ConnectSharePointModal onClose={() => setActiveModal("none")} />
      ) : null}
    </>
  );
}
