import { SidebarFrame } from "./SidebarFrame";
import { SidebarLink } from "./SidebarLink";
import { SidebarSection } from "./SidebarSection";
import type { HomeSidebarProps } from "./sidebarTypes";

export function HomeWorkspaceSidebar({
  activeHomeSection,
  email,
  navigationState,
  tools,
}: HomeSidebarProps) {
  return (
    <SidebarFrame
      alignedHeader
      email={email}
      navigationState={navigationState}
      sidebarIcon="home"
      sidebarLabel="Deal Hub"
      showHeaderBackButton={false}
    >
      <nav className="space-y-1">
        <SidebarLink
          homeSection={activeHomeSection}
          href="/hub/initiatives/vault"
          icon="folderOpen"
          label="Vault"
          navigationState={navigationState}
        />
      </nav>

      <SidebarSection title="Research">
        <SidebarLink
          homeSection={activeHomeSection}
          href="/hub/summarize"
          icon="telescope"
          label="Explore"
          navigationState={navigationState}
        />
        <SidebarLink icon="search" label="Topics" />
      </SidebarSection>

      <SidebarSection title="Workspace">
        <SidebarLink
          homeSection={activeHomeSection}
          href="/hub/deals"
          icon="boxes"
          label="Deals"
          navigationState={navigationState}
        />
        <SidebarLink icon="bookOpen" label="Notebook" />
        <SidebarLink icon="dataset" label="Templates" />
      </SidebarSection>

      <div className="mt-3 border-t border-outline-variant/70 pt-3">
        <nav className="space-y-1">
          {tools.map((item) => (
            <SidebarLink
              homeSection={activeHomeSection}
              href={item.href}
              icon={item.icon}
              key={item.name}
              label={item.name}
              navigationState={navigationState}
            />
          ))}
        </nav>
      </div>
    </SidebarFrame>
  );
}
