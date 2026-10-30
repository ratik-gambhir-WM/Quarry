import { Navigate, useLocation } from "react-router-dom";
import { getDealActivityPath } from "../../data/workspace";
import { useDealRoom } from "../DealRoomPage";

// Preserve old activity bookmarks while rendering activity as a Deal Room tab.
export function DealActivityPage() {
  const { deal } = useDealRoom();
  const location = useLocation();
  return (
    <Navigate
      replace
      state={location.state}
      to={getDealActivityPath(deal.room.id)}
    />
  );
}
