import { useRef } from "react";
import { Ellipsis, Trash2 } from "lucide-react";
import type { DealPortfolioView } from "../../data/dealsView";
import { Button } from "../ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";

export type DeleteDealTarget = Pick<DealPortfolioView, "id" | "name"> & {
  trigger: HTMLButtonElement | null;
  revision: number;
};

export function DealActionsMenu({
  deal,
  onDelete,
}: {
  deal: DealPortfolioView;
  onDelete: (target: DeleteDealTarget) => void;
}) {
  const trigger = useRef<HTMLButtonElement>(null);
  const openingDialog = useRef(false);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          ref={trigger}
          aria-label={`Actions for ${deal.name}`}
          className="shrink-0 text-muted-foreground"
          size="icon-sm"
          variant="ghost"
        >
          <Ellipsis aria-hidden="true" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="start"
        className="w-40"
        onCloseAutoFocus={(event) => {
          if (openingDialog.current) event.preventDefault();
          openingDialog.current = false;
        }}
      >
        <DropdownMenuItem
          variant="destructive"
          onSelect={() => {
            if (deal.revision === null) return;
            openingDialog.current = true;
            onDelete({ id: deal.id, name: deal.name, revision: deal.revision, trigger: trigger.current });
          }}
        >
          <Trash2 aria-hidden="true" />
          Delete deal
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
