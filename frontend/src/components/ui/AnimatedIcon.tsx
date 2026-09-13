import { BookOpenIcon } from "./book-open";
import { CalendarDaysIcon } from "./calendar-days";
import { BoxesIcon } from "./icons/BoxesIcon";
import { FileStackIcon } from "./file-stack";
import { FolderTreeIcon } from "./folder-tree";
import { HomeIcon } from "./icons/HomeIcon";
import { LayersIcon } from "./layers";
import { ShipIcon } from "./ship";
import { TelescopeIcon } from "./telescope";
import { TwitchIcon } from "./twitch";

export type AnimatedIconName =
  | "bookOpen"
  | "boxes"
  | "calendarDays"
  | "fileStack"
  | "folderTree"
  | "home"
  | "layers"
  | "ship"
  | "telescope"
  | "twitch";

export function AnimatedIcon({ className, name }: { className: string; name: AnimatedIconName }) {
  switch (name) {
    case "bookOpen":
      return <BookOpenIcon className={className} size={20} />;
    case "calendarDays":
      return <CalendarDaysIcon aria-hidden="true" className={className} size={20} />;
    case "ship":
      return <ShipIcon aria-hidden="true" className={className} size={20} />;
    case "fileStack":
      return <FileStackIcon aria-hidden="true" className={className} size={20} />;
    case "folderTree":
      return <FolderTreeIcon aria-hidden="true" className={className} size={20} />;
    case "boxes":
      return <BoxesIcon className={className} size={20} />;
    case "telescope":
      return <TelescopeIcon className={className} size={20} />;
    case "twitch":
      return <TwitchIcon className={className} size={20} />;
    case "home":
      return <HomeIcon className={className} />;
    case "layers":
      return <LayersIcon className={className} size={20} />;
  }
}
