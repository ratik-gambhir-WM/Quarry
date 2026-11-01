import { createRoot } from "react-dom/client";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { ThemeModeProvider } from "../../hooks/useThemeMode";
import { Button } from "../../components/ui/stealth/button";
import { UserProvider } from "../../auth/UserProvider";
import { WorkspaceProvider } from "../../app/WorkspaceProvider";
import { DealRoomPage } from "../../pages/DealRoomPage";
import { DealRoomOverviewPage } from "../../pages/deal-room/DealRoomOverviewPage";
import { runtime } from "@quarry/runtime";
import { testDeal } from "../../test/fixtures/deals";
import "../../index.css";
const deal={...testDeal({dealId:"DEAL-PREVIEW",dealName:"Component Preview"}),metadata:null};
const members=[{userId:1,username:"preview-owner",firstName:"Preview",lastName:"Owner",role:"owner",revision:1,joinedAt:"2026-11-01",teamIds:[]},{userId:2,username:"preview-member",firstName:"Preview",lastName:"Member",role:"contributor",revision:1,joinedAt:"2026-11-01",teamIds:[]}];
const teams=[{teamId:"00000000-0000-4000-8000-000000000001",name:"Technology",revision:1,userIds:[1]}];
Object.assign(runtime.api,{
 listDeals:async()=>[deal],getDeal:async()=>deal,
 listDealMembers:async()=>({items:members,nextCursor:null}),listDealTeams:async()=>({items:teams,nextCursor:null}),
 addDealMember:async()=>{},changeDealMemberRole:async()=>{},transferDealOwnership:async()=>{},removeDealMember:async()=>{},
 createDealTeam:async()=>{},renameDealTeam:async()=>{},addDealTeamMember:async()=>{},removeDealTeamMember:async()=>{},deleteDealTeam:async()=>{},
 searchDealDocumentChunksByKeyword:async()=>[],searchDocumentChunksByKeyword:async()=>[],
});
createRoot(document.getElementById("root")!).render(<MemoryRouter initialEntries={["/hub/deals/DEAL-PREVIEW?section=settings"]}><ThemeModeProvider><UserProvider><WorkspaceProvider dataSource="api"><Routes><Route path="/hub/deals/:dealId" element={<DealRoomPage/>}><Route index element={<DealRoomOverviewPage/>}/></Route></Routes></WorkspaceProvider><div className="fixed bottom-3 right-3 z-[100] rounded-lg bg-background p-2 text-xs shadow-lg">Synthetic preview · <Button onClick={() => {document.documentElement.dataset.theme=document.documentElement.dataset.theme==='dark'?'slate-frost':'dark';}}>Toggle preview theme</Button></div></UserProvider></ThemeModeProvider></MemoryRouter>);
