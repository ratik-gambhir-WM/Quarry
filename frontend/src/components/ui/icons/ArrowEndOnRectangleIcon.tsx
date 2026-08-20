import type { Variants } from "motion/react";
import { motion, useAnimation } from "motion/react";
import type { CSSProperties, HTMLAttributes, MouseEvent } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

export interface ArrowEndOnRectangleIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

type ArrowEndOnRectangleIconProps = HTMLAttributes<HTMLDivElement> & {
  direction?: "left" | "right";
  size?: number;
};

const ARROW_VARIANTS: Variants = {
  normal: { translateX: 0 },
  animate: {
    translateX: [0, -2, 0],
    transition: {
      duration: 0.5,
      times: [0, 0.4, 1],
    },
  },
};

const ArrowEndOnRectangleIcon = forwardRef<ArrowEndOnRectangleIconHandle, ArrowEndOnRectangleIconProps>(
  (
    {
      className,
      direction = "left",
      onMouseEnter,
      onMouseLeave,
      size = 28,
      style,
      ...props
    },
    ref,
  ) => {
    const controls = useAnimation();
    const isControlledRef = useRef(false);

    useImperativeHandle(ref, () => {
      isControlledRef.current = true;

      return {
        startAnimation: () => controls.start("animate"),
        stopAnimation: () => controls.start("normal"),
      };
    });

    const handleMouseEnter = useCallback(
      (event: MouseEvent<HTMLDivElement>) => {
        if (!isControlledRef.current) {
          controls.start("animate");
        }

        onMouseEnter?.(event);
      },
      [controls, onMouseEnter],
    );

    const handleMouseLeave = useCallback(
      (event: MouseEvent<HTMLDivElement>) => {
        if (isControlledRef.current) {
          onMouseLeave?.(event);
        } else {
          controls.start("normal");
        }
      },
      [controls, onMouseLeave],
    );

    const iconStyle: CSSProperties | undefined = className ? style : { height: size, width: size, ...style };

    return (
      <div
        aria-hidden="true"
        className={className}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        style={iconStyle}
        {...props}
      >
        <svg
          className={`h-full w-full ${direction === "right" ? "-scale-x-100" : ""}`}
          fill="none"
          height={size}
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.5"
          viewBox="0 0 24 24"
          width={size}
          xmlns="http://www.w3.org/2000/svg"
        >
          <path d="M15.75 9V5.25A2.25 2.25 0 0 0 13.5 3h-6a2.25 2.25 0 0 0-2.25 2.25v13.5A2.25 2.25 0 0 0 7.5 21h6a2.25 2.25 0 0 0 2.25-2.25V15" />
          <motion.g animate={controls} initial="normal" variants={ARROW_VARIANTS}>
            <path d="M12 9l-3 3m0 0 3 3m-3-3h12.75" />
          </motion.g>
        </svg>
      </div>
    );
  },
);

ArrowEndOnRectangleIcon.displayName = "ArrowEndOnRectangleIcon";

export { ArrowEndOnRectangleIcon };
