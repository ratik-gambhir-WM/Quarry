import type { Variants } from "motion/react";
import { motion, useAnimation } from "motion/react";
import type { CSSProperties, HTMLAttributes, MouseEvent } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

export interface HomeIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

type HomeIconProps = HTMLAttributes<HTMLDivElement> & {
  size?: number;
};

const SVG_VARIANTS: Variants = {
  normal: {
    scale: 1,
    y: 0,
  },
  animate: {
    scale: [1, 1.1, 1],
    y: [0, -1, 0],
    transition: {
      duration: 0.4,
      ease: "easeOut",
    },
  },
};

const HomeIcon = forwardRef<HomeIconHandle, HomeIconProps>(
  ({ className, onMouseEnter, onMouseLeave, size = 28, style, ...props }, ref) => {
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
        if (isControlledRef.current) {
          onMouseEnter?.(event);
        } else {
          controls.start("animate");
        }
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
        <motion.svg
          animate={controls}
          className="h-full w-full"
          fill="none"
          height={size}
          initial="normal"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="1.5"
          variants={SVG_VARIANTS}
          viewBox="0 0 24 24"
          width={size}
          xmlns="http://www.w3.org/2000/svg"
        >
          <path d="M2.25 12L11.2045 3.04549C11.6438 2.60615 12.3562 2.60615 12.7955 3.04549L21.75 12M4.5 9.75V19.875C4.5 20.4963 5.00368 21 5.625 21H9.75V16.125C9.75 15.5037 10.2537 15 10.875 15H13.125C13.7463 15 14.25 15.5037 14.25 16.125V21H18.375C18.9963 21 19.5 20.4963 19.5 19.875V9.75M8.25 21H16.5" />
        </motion.svg>
      </div>
    );
  },
);

HomeIcon.displayName = "HomeIcon";

export { HomeIcon };
