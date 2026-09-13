import { Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Carousel,
  CarouselContent,
  CarouselItem,
  CarouselNext,
  CarouselPrevious,
} from "@/components/ui/carousel";
import { templateDisplayName, type DeliverableSlide } from "@/data/deliverables";

type DeliverablesCarouselProps = {
  actionsDisabled?: boolean;
  deletingSlideId: string | null;
  onDeleteSlide: (slide: DeliverableSlide) => void;
  sectionLabel: string;
  slides: readonly DeliverableSlide[];
};

export function DeliverablesCarousel({
  actionsDisabled = false,
  deletingSlideId,
  onDeleteSlide,
  sectionLabel,
  slides,
}: DeliverablesCarouselProps) {
  return (
    <Carousel
      aria-label={sectionLabel}
      className="w-full max-w-[1440px] px-10"
      opts={{ align: "start", loop: false, slidesToScroll: 1 }}
    >
      <CarouselContent>
        {slides.map((slide, index) => (
          <CarouselItem
            aria-label={`${slide.id}, slide ${index + 1} of ${slides.length}`}
            className="basis-full"
            key={slide.id}
          >
            <Card
              className="relative w-full overflow-hidden bg-surface-container-low p-0 shadow-sm"
              style={{ aspectRatio: `${slide.thumbnailWidth} / ${slide.thumbnailHeight}` }}
            >
              <img
                alt={slide.thumbnailAlt}
                className="h-full w-full object-contain"
                decoding="async"
                height={slide.thumbnailHeight}
                loading="lazy"
                src={slide.thumbnailSrc}
                width={slide.thumbnailWidth}
              />
              <Button
                aria-label={
                  deletingSlideId === slide.id
                    ? `Deleting ${templateDisplayName(slide.id)}`
                    : `Delete ${templateDisplayName(slide.id)}`
                }
                className="absolute right-3 top-3 z-10 rounded-full shadow-sm"
                disabled={actionsDisabled || deletingSlideId !== null}
                onClick={() => onDeleteSlide(slide)}
                size="icon-sm"
                title={`Delete ${templateDisplayName(slide.id)}`}
                type="button"
                variant="destructive"
              >
                <Trash2 aria-hidden="true" />
              </Button>
            </Card>
          </CarouselItem>
        ))}
      </CarouselContent>
      <CarouselPrevious />
      <CarouselNext />
    </Carousel>
  );
}
