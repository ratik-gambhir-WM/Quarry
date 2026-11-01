import { BookOpen, BriefcaseBusiness, House, MessageCircle, UserRound, Vault } from "lucide-react";
import { NavLink } from "react-router-dom";

const destinations = [
  { label: "Home", to: "/hub", icon: House, end: true },
  { label: "Deals", to: "/hub/deals", icon: BriefcaseBusiness },
  { label: "Assistant", to: "/hub/assistant", icon: MessageCircle },
  { label: "Notebook", to: "/hub/notebook", icon: BookOpen },
  { label: "Vault", to: "/hub/initiatives/vault", icon: Vault },
  { label: "Account", to: "/hub/account", icon: UserRound },
];

export function MobileWorkspaceNavigation() {
  return (
    <nav aria-label="Workspace" className="mobile-workspace-navigation grid shrink-0 grid-cols-6 gap-1 border-t border-outline-variant bg-background px-2 pt-1 lg:hidden">
      {destinations.map(({ label, to, icon: Icon, end }) => (
        <NavLink
          key={to}
          to={to}
          end={end}
          className={({ isActive }) => `flex min-h-12 min-w-0 flex-col items-center justify-center gap-1 rounded-lg px-1 text-[10px] font-medium transition-colors ${isActive ? "bg-primary/10 text-primary" : "text-muted hover:bg-surface-container-high"}`}
        >
          <Icon aria-hidden="true" className="size-4" />
          <span>{label}</span>
        </NavLink>
      ))}
    </nav>
  );
}
