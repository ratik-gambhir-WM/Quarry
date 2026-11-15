// Adapted from shadcn/ui's radix-nova accordion; Quarry owns theme and motion tokens.
import type { ComponentProps } from "react";
import { Accordion as AccordionPrimitive } from "radix-ui";
import { ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";

function Accordion({ className, ...props }: ComponentProps<typeof AccordionPrimitive.Root>) {
  return <AccordionPrimitive.Root data-slot="accordion" className={cn("flex w-full flex-col", className)} {...props} />;
}

function AccordionItem({ className, ...props }: ComponentProps<typeof AccordionPrimitive.Item>) {
  return <AccordionPrimitive.Item data-slot="accordion-item" className={cn("not-last:border-b", className)} {...props} />;
}

function AccordionTrigger({
  className,
  children,
  headingLevel = 3,
  ...props
}: ComponentProps<typeof AccordionPrimitive.Trigger> & { headingLevel?: 2 | 3 | 4 | 5 | 6 }) {
  const Heading = `h${headingLevel}` as const;
  return (
    <AccordionPrimitive.Header asChild>
      <Heading className="flex">
        <AccordionPrimitive.Trigger
          data-slot="accordion-trigger"
          className={cn(
            "quarry-accordion-trigger group/accordion-trigger flex flex-1 items-start justify-between gap-4 rounded-lg py-2.5 text-left text-sm font-medium outline-none transition-colors [@media(hover:hover)]:hover:underline focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50",
            className,
          )}
          {...props}
        >
          {children}
          <ChevronDown aria-hidden="true" data-slot="accordion-trigger-icon" className="quarry-accordion-chevron pointer-events-none mt-1 size-4 shrink-0 text-muted-foreground group-aria-expanded/accordion-trigger:rotate-180" />
        </AccordionPrimitive.Trigger>
      </Heading>
    </AccordionPrimitive.Header>
  );
}

function AccordionContent({ className, children, ...props }: ComponentProps<typeof AccordionPrimitive.Content>) {
  return (
    <AccordionPrimitive.Content data-slot="accordion-content" className="quarry-accordion-content overflow-hidden text-sm" {...props}>
      <div className={cn("pb-2.5", className)}>{children}</div>
    </AccordionPrimitive.Content>
  );
}

export { Accordion, AccordionItem, AccordionTrigger, AccordionContent };
