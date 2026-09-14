import { useRef, useState } from "react";
import { Plus } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import { Button } from "../ui/button";
import { AddDealModal } from "../hub/sidebar/AddDealModal";

type AddDealMenuProps = {
  email?: string;
};

export function AddDealMenu({ email }: AddDealMenuProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);
  const openingModalRef = useRef(false);
  const triggerRef = useRef<HTMLButtonElement>(null);

  function closeModal() {
    setModalOpen(false);
    window.requestAnimationFrame(() => triggerRef.current?.focus());
  }

  return (
    <>
      <DropdownMenu onOpenChange={setMenuOpen} open={menuOpen}>
        <DropdownMenuTrigger asChild>
          <Button ref={triggerRef} size="sm">
            <Plus aria-hidden="true" />
            New
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent
          align="end"
          className="deals-add-menu w-40"
          onCloseAutoFocus={(event) => {
            if (openingModalRef.current) event.preventDefault();
            openingModalRef.current = false;
          }}
          sideOffset={12}
        >
          <DropdownMenuGroup>
            <DropdownMenuLabel>Create</DropdownMenuLabel>
            <DropdownMenuItem
              onSelect={() => {
                openingModalRef.current = true;
                setMenuOpen(false);
                window.requestAnimationFrame(() => setModalOpen(true));
              }}
            >
              <Plus aria-hidden="true" className="size-4 text-muted" />
              <span>Add deal</span>
            </DropdownMenuItem>
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>

      {modalOpen ? <AddDealModal email={email} onClose={closeModal} /> : null}
    </>
  );
}
