// Adapted from https://www.stealth.pm/ui/follow-button (Stealth registry).
// Keep its reserved label widths and drawn check; deal roles remain server-owned.
import { Select as SelectPrimitive } from "@base-ui/react/select";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Check, ChevronDown, ShieldCheck } from "lucide-react";
import { useState } from "react";
import type { AssignableDealRole, DealRole } from "../../contracts/dealMemberships";
import { ease } from "../ui/stealth-date/motion";
import { Select, SelectItem, SelectPopup } from "../ui/stealth/select";

export const dealRoleLabels: Record<DealRole, string> = {
  owner: "Owner",
  manager: "Manager",
  contributor: "Contributor",
  viewer: "Viewer",
};
const roleOptions: { value: AssignableDealRole; label: string }[] = [
  { value: "manager", label: "Manager" },
  { value: "contributor", label: "Contributor" },
  { value: "viewer", label: "Viewer" },
];

export function DealMemberPermissions({
  role,
  username,
  editable,
  disabled,
  onChange,
}: {
  role: DealRole;
  username: string;
  editable: boolean;
  disabled: boolean;
  onChange: (role: AssignableDealRole) => void;
}) {
  const reducedMotion = useReducedMotion();
  const [keyboard, setKeyboard] = useState(false);
  const content = (
    <span className="deal-permission-label">
      {/* All labels reserve the widest state, as in Stealth's FollowButton. */}
      {Object.values(dealRoleLabels).map((label) => (
        <span key={label} className="deal-permission-reserve" aria-hidden="true">
          <Check size={14} />{label}
        </span>
      ))}
      <AnimatePresence initial={false}>
        <motion.span
          key={role}
          className="deal-permission-current"
          variants={{
            enter: { opacity: 0, y: reducedMotion || keyboard ? 0 : 6 },
            show: { opacity: 1, y: 0 },
            leave: { opacity: 0, y: reducedMotion || keyboard ? 0 : -6 },
          }}
          initial="enter" animate="show" exit="leave"
          transition={{ duration: reducedMotion || keyboard ? 0 : 0.18, ease: ease.out }}
        >
          {role === "owner" ? <ShieldCheck size={14} aria-hidden="true" /> : (
            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <motion.path
                d="M3.5 8.5 6.5 11.5 12.5 4.5"
                stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"
                variants={{
                  enter: { pathLength: reducedMotion || keyboard ? 1 : 0 },
                  show: { pathLength: 1, transition: { duration: reducedMotion || keyboard ? 0 : 0.25, ease: ease.out } },
                  leave: {},
                }}
              />
            </svg>
          )}
          {dealRoleLabels[role]}
        </motion.span>
      </AnimatePresence>
    </span>
  );
  if (!editable || role === "owner") {
    return <span className="deal-permission" data-readonly="true" aria-label={`${dealRoleLabels[role]} permissions for ${username}`}>{content}</span>;
  }
  return (
    <Select<AssignableDealRole>
      items={roleOptions}
      value={role}
      disabled={disabled}
      onValueChange={(next) => { if (next && next !== role) onChange(next); }}
    >
      <SelectPrimitive.Trigger
        className="deal-permission"
        aria-label={`Permissions for ${username}`}
        onKeyDown={() => setKeyboard(true)}
        onPointerDown={() => setKeyboard(false)}
      >
        {content}
        <ChevronDown size={12} aria-hidden="true" className="deal-permission-chevron" />
      </SelectPrimitive.Trigger>
      <SelectPopup align="end" alignItemWithTrigger={false} className="min-w-44">
        {roleOptions.map((option) => <SelectItem key={option.value} value={option.value}>{option.label}</SelectItem>)}
      </SelectPopup>
    </Select>
  );
}
