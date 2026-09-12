// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DeliverablesCarousel } from "@/components/deal-room/DeliverablesCarousel";

afterEach(cleanup);

describe("DeliverablesCarousel", () => {
  it("renders API-backed images and accessible slide/navigation labels", () => {
    const onDeleteSlide = vi.fn();
    const slide = {
      id: "template-exact-id",
      thumbnailAlt: "Template preview: template exact id",
      thumbnailHeight: 720,
      thumbnailSrc: "data:image/png;base64,dGVtcGxhdGU=",
      thumbnailWidth: 1280,
    };
    render(
      <DeliverablesCarousel
        deletingSlideId={null}
        onDeleteSlide={onDeleteSlide}
        sectionLabel="Start from templates slides"
        slides={[slide]}
      />,
    );

    const carousel = screen.getByRole("region", { name: "Start from templates slides" });
    const image = within(carousel).getByRole("img", {
      name: "Template preview: template exact id",
    });
    expect(image.getAttribute("src")).toBe("data:image/png;base64,dGVtcGxhdGU=");
    expect(image.getAttribute("width")).toBe("1280");
    expect(image.getAttribute("height")).toBe("720");
    const slideGroup = within(carousel).getByRole("group", { name: "template-exact-id, slide 1 of 1" });
    expect(slideGroup.classList.contains("basis-full")).toBe(true);
    expect(slideGroup.classList.contains("lg:basis-1/2")).toBe(false);
    expect(slideGroup.querySelector<HTMLElement>('[data-slot="card"]')?.style.aspectRatio).toBe("1280 / 720");
    expect(carousel.classList.contains("max-w-[1440px]")).toBe(true);
    expect(within(carousel).getByRole("button", { name: "Previous slide" })).toHaveProperty(
      "disabled",
      true,
    );
    expect(within(carousel).getByRole("button", { name: "Next slide" })).toHaveProperty(
      "disabled",
      true,
    );
    fireEvent.click(within(carousel).getByRole("button", { name: "Delete template exact id" }));
    expect(onDeleteSlide).toHaveBeenCalledWith(slide);
  });
});
