import { BookOpenIcon } from "./book-open";
import { BoxesIcon } from "./icons/BoxesIcon";
import { HomeIcon } from "./icons/HomeIcon";
import { TelescopeIcon } from "./telescope";

type IconName =
  | "arrowRight"
  | "bookmark"
  | "bookOpen"
  | "boxes"
  | "chevronDown"
  | "chevronLeft"
  | "chevronRight"
  | "check"
  | "checkCircle"
  | "dashboard"
  | "dataset"
  | "doc"
  | "filter"
  | "folderOpen"
  | "graph"
  | "grid"
  | "help"
  | "home"
  | "image"
  | "info"
  | "key"
  | "listAlt"
  | "mail"
  | "more"
  | "notification"
  | "openInNew"
  | "pdf"
  | "person"
  | "personSearch"
  | "plus"
  | "refresh"
  | "search"
  | "send"
  | "sharepoint"
  | "sheet"
  | "sidebar"
  | "shield"
  | "sparkles"
  | "settings"
  | "terminal"
  | "timeline"
  | "telescope"
  | "upload"
  | "alert";

type IconProps = {
  className?: string;
  name: IconName;
};

const iconClassName = "fill-none stroke-current stroke-2";

export function Icon({ className = "h-5 w-5", name }: IconProps) {
  switch (name) {
    case "bookOpen":
      return <BookOpenIcon className={className} size={20} />;
    case "boxes":
      return <BoxesIcon className={className} size={20} />;
    case "telescope":
      return <TelescopeIcon className={className} size={20} />;
    case "mail":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="10.5" rx="2" width="16" x="4" y="6.75" />
          <path className={iconClassName} d="m5 8 7 5 7-5" />
        </svg>
      );
    case "key":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="8.5" cy="15.5" r="3.5" />
          <path className={iconClassName} d="M11.5 13h8.75v2.5h-2v2h-2v-2h-2v-2.5" />
        </svg>
      );
    case "arrowRight":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M5 12h13" />
          <path className={iconClassName} d="m13 6 6 6-6 6" />
        </svg>
      );
    case "chevronRight":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="m9 6 6 6-6 6" />
        </svg>
      );
    case "chevronLeft":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="m15 6-6 6 6 6" />
        </svg>
      );
    case "chevronDown":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="m6 9 6 6 6-6" />
        </svg>
      );
    case "shield":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M12 3 5.75 5.5v5.75c0 4.25 2.6 8.15 6.25 9.75 3.65-1.6 6.25-5.5 6.25-9.75V5.5z" />
          <path className={iconClassName} d="m9.5 12 1.75 1.75L14.75 10" />
        </svg>
      );
    case "help":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="12" cy="12" r="9" />
          <path className={iconClassName} d="M9.75 9.25a2.25 2.25 0 1 1 4.13 1.23c-.55.77-1.88 1.31-1.88 2.77" />
          <path className={iconClassName} d="M12 17h.01" />
        </svg>
      );
    case "info":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="12" cy="12" r="8.5" />
          <path className={iconClassName} d="M12 10.75V17" />
          <path className={iconClassName} d="M12 7h.01" />
        </svg>
      );
    case "home":
      return <HomeIcon className={className} />;
    case "dataset":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="16" rx="2.5" width="16" x="4" y="4" />
          <path className={iconClassName} d="M8.5 4v16M15.5 4v16M4 9.5h16M4 14.5h16" />
        </svg>
      );
    case "dashboard":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="7" rx="1.5" width="7" x="4" y="4" />
          <rect className={iconClassName} height="7" rx="1.5" width="7" x="13" y="4" />
          <rect className={iconClassName} height="7" rx="1.5" width="7" x="4" y="13" />
          <rect className={iconClassName} height="7" rx="1.5" width="7" x="13" y="13" />
        </svg>
      );
    case "sidebar":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="16" rx="2.5" width="18" x="3" y="4" />
          <path className={iconClassName} d="M15 4v16M10.5 9 7.5 12l3 3" />
        </svg>
      );
    case "checkCircle":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="12" cy="12" r="8.5" />
          <path className={iconClassName} d="m8.5 12.2 2.2 2.2 4.8-5" />
        </svg>
      );
    case "personSearch":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="10" cy="8.5" r="2.5" />
          <path className={iconClassName} d="M5.5 17c.7-2.2 2.5-3.5 4.5-3.5s3.8 1.3 4.5 3.5" />
          <circle className={iconClassName} cx="17.5" cy="16.5" r="2.5" />
          <path className={iconClassName} d="m19.2 18.2 2.3 2.3" />
        </svg>
      );
    case "person":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="12" cy="8" r="3.25" />
          <path className={iconClassName} d="M5.75 19c.95-3.2 3.35-5 6.25-5s5.3 1.8 6.25 5" />
        </svg>
      );
    case "terminal":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="14" rx="2" width="18" x="3" y="5" />
          <path className={iconClassName} d="m7 10 3 2-3 2M12.5 16H17" />
        </svg>
      );
    case "timeline":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="m4 16 4-4 4 3 6-7" />
          <circle className={iconClassName} cx="4" cy="16" r="1" />
          <circle className={iconClassName} cx="8" cy="12" r="1" />
          <circle className={iconClassName} cx="12" cy="15" r="1" />
          <circle className={iconClassName} cx="18" cy="8" r="1" />
        </svg>
      );
    case "folderOpen":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M3.5 8.5h6l2 2h9v7a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z" />
          <path className={iconClassName} d="M3.5 8.5v-1a2 2 0 0 1 2-2h4l2 2h7a2 2 0 0 1 2 2v1" />
        </svg>
      );
    case "grid":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="5" rx="1" width="5" x="4" y="4" />
          <rect className={iconClassName} height="5" rx="1" width="5" x="15" y="4" />
          <rect className={iconClassName} height="5" rx="1" width="5" x="4" y="15" />
          <rect className={iconClassName} height="5" rx="1" width="5" x="15" y="15" />
        </svg>
      );
    case "graph":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="6" cy="7" r="2.25" />
          <circle className={iconClassName} cx="18" cy="6" r="2.25" />
          <circle className={iconClassName} cx="12" cy="18" r="2.25" />
          <path className={iconClassName} d="m8 8.2 8 7.6M16.4 7.8l-3.3 8M7.2 9l3.6 7" />
        </svg>
      );
    case "plus":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M12 5v14M5 12h14" />
        </svg>
      );
    case "refresh":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M20 5v5h-5" />
          <path className={iconClassName} d="M19 10a7 7 0 1 0 1.4 6.6" />
        </svg>
      );
    case "upload":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M12 16V4M7.5 8.5 12 4l4.5 4.5" />
          <path className={iconClassName} d="M5 14v4.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V14" />
        </svg>
      );
    case "listAlt":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="18" rx="2.5" width="14" x="5" y="3" />
          <path className={iconClassName} d="M9 8.5h6M9 12h6M9 15.5h6M7.5 8.5h.01M7.5 12h.01M7.5 15.5h.01" />
        </svg>
      );
    case "filter":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M4 6h16M7 12h10M10 18h4" />
        </svg>
      );
    case "pdf":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M7 3.5h7l4 4V20H7z" />
          <path className={iconClassName} d="M14 3.5v4h4" />
          <path className={iconClassName} d="M8.5 15h7M8.5 18h5" />
        </svg>
      );
    case "doc":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M7 3.5h7l4 4V20H7z" />
          <path className={iconClassName} d="M14 3.5v4h4M9 12.5h6M9 16h6" />
        </svg>
      );
    case "sharepoint":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="15.5" cy="7.5" r="3.5" />
          <circle className={iconClassName} cx="16.5" cy="16" r="3" />
          <rect className={iconClassName} height="13" rx="2" width="10" x="3.5" y="5.5" />
          <path className={iconClassName} d="M10.5 9H8a1.5 1.5 0 0 0 0 3h1a1.5 1.5 0 0 1 0 3H6.5" />
        </svg>
      );
    case "sheet":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="16" rx="2" width="16" x="4" y="4" />
          <path className={iconClassName} d="M4 9.5h16M9.5 9.5V20M14.5 9.5V20" />
        </svg>
      );
    case "sparkles":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="m12 4 1.4 4.6L18 10l-4.6 1.4L12 16l-1.4-4.6L6 10l4.6-1.4zM18.5 4.5 19 6l1.5.5-1.5.5-.5 1.5-.5-1.5L16.5 6l1.5-.5zM18 15l.8 2.2L21 18l-2.2.8L18 21l-.8-2.2L15 18l2.2-.8z" />
        </svg>
      );
    case "search":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="11" cy="11" r="6" />
          <path className={iconClassName} d="m16 16 4.5 4.5" />
        </svg>
      );
    case "notification":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M8 17.5h8M9 17.5v-5a3 3 0 0 1 6 0v5M7 17.5h10l-1.2-1.8a3.2 3.2 0 0 1-.5-1.8v-1.4a5.3 5.3 0 1 0-10.6 0v1.4c0 .64-.18 1.27-.52 1.8z" />
        </svg>
      );
    case "settings":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="12" cy="12" r="3.1" />
          <path className={iconClassName} d="m12 3.8 1 1.8 2.1.4 1.4-1.4 1.8 1.8-1.4 1.4.4 2.1 1.8 1-1.8 1.8-1.8-1-.4 2.1 1.4 1.4-1.8 1.8-1.4-1.4-2.1.4-1 1.8-1.8-1.8-2.1-.4-1.4 1.4-1.8-1.8 1.4-1.4-.4-2.1-1.8-1 1.8-1.8 1.8 1 .4-2.1-1.4-1.4 1.8-1.8 1.4 1.4 2.1-.4z" />
        </svg>
      );
    case "send":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M4 19 20 12 4 5l2.5 7z" />
          <path className={iconClassName} d="M6.5 12H20" />
        </svg>
      );
    case "bookmark":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M7 4.5h10v15l-5-3.5-5 3.5z" />
        </svg>
      );
    case "openInNew":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="M14 5h5v5M19 5l-8 8" />
          <path className={iconClassName} d="M10 7H7a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h8a2 2 0 0 0 2-2v-3" />
        </svg>
      );
    case "image":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <rect className={iconClassName} height="14" rx="2" width="18" x="3" y="5" />
          <circle className={iconClassName} cx="9" cy="10" r="1.5" />
          <path className={iconClassName} d="m5.5 17 4.5-4.5 3.5 3.5 2.5-2.5 2.5 3" />
        </svg>
      );
    case "more":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="6" cy="12" r="1" />
          <circle className={iconClassName} cx="12" cy="12" r="1" />
          <circle className={iconClassName} cx="18" cy="12" r="1" />
        </svg>
      );
    case "check":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <path className={iconClassName} d="m6 12 4 4 8-8" />
        </svg>
      );
    case "alert":
      return (
        <svg aria-hidden="true" className={className} viewBox="0 0 24 24">
          <circle className={iconClassName} cx="12" cy="12" r="8.5" />
          <path className={iconClassName} d="M12 8v5" />
          <path className={iconClassName} d="M12 16.5h.01" />
        </svg>
      );
    default:
      return null;
  }
}
