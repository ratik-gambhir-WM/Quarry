import { useRef, useState } from "react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import { Icon } from "../ui/Icon";
import { PlusIcon, type PlusIconHandle } from "../ui/plus";
import { AddDealModal } from "../hub/sidebar/AddDealModal";
import { DealsToolbarButton } from "./DealsToolbarButton";

type AddDealMenuProps = {
  email?: string;
};

export function AddDealMenu({ email }: AddDealMenuProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const openingModalRef = useRef(false);
  const plusIconRef = useRef<PlusIconHandle>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  function closeModal() {
    setModalOpen(false);
    window.requestAnimationFrame(() => triggerRef.current?.focus());
  }

  return (
    <>
      <DropdownMenu onOpenChange={setMenuOpen} open={menuOpen}>
        <DropdownMenuTrigger asChild>
          <DealsToolbarButton
            aria-label="Deal portfolio actions"
            onBlur={() => plusIconRef.current?.stopAnimation()}
            onFocus={() => plusIconRef.current?.startAnimation()}
            onMouseEnter={() => plusIconRef.current?.startAnimation()}
            onMouseLeave={() => plusIconRef.current?.stopAnimation()}
            ref={triggerRef}
          >
            <PlusIcon className="h-4 w-4" ref={plusIconRef} size={16} />
          </DealsToolbarButton>
        </DropdownMenuTrigger>
        <DropdownMenuContent
          align="end"
          className="deals-add-menu w-40 rounded-xl border border-outline-variant bg-surface-container-lowest p-1.5 text-text-main"
          onCloseAutoFocus={(event) => {
            if (openingModalRef.current) event.preventDefault();
            openingModalRef.current = false;
          }}
          sideOffset={8}
        >
          <DropdownMenuItem
            className="rounded-lg px-3 py-2 text-[13px] font-medium"
            onSelect={() => {
              openingModalRef.current = true;
              setMenuOpen(false);
              window.requestAnimationFrame(() => setModalOpen(true));
            }}
          >
            <Icon className="h-4 w-4 text-muted" name="plus" />
            <span>Add deal</span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      {modalOpen ? <AddDealModal email={email} onClose={closeModal} /> : null}
    </>
  );
}
