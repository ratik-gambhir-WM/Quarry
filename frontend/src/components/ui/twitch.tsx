"use client";

import type { Variants } from "motion/react";
import { motion, useAnimation, useReducedMotion } from "motion/react";
import type { HTMLAttributes, MouseEvent } from "react";
import { forwardRef, useCallback, useImperativeHandle, useRef } from "react";

import { cn } from "@/lib/utils";

export interface TwitchIconHandle {
  startAnimation: () => void;
  stopAnimation: () => void;
}

interface TwitchIconProps extends HTMLAttributes<HTMLDivElement> {
  size?: number;
}

const PATH_VARIANTS: Variants = {
  normal: {
    opacity: 1,
    pathLength: 1,
    pathOffset: 0,
    transition: {
      duration: 0.4,
      opacity: { duration: 0.1 },
    },
  },
  animate: {
    opacity: [0, 1],
    pathLength: [0, 1],
    pathOffset: [1, 0],
    transition: {
      duration: 0.6,
      ease: "linear",
      opacity: { duration: 0.1 },
    },
  },
};

const TwitchIcon = forwardRef<TwitchIconHandle, TwitchIconProps>(
  ({ onMouseEnter, onMouseLeave, className, size = 28, ...props }, ref) => {
    const pathControls = useAnimation();
    const line1Controls = useAnimation();
    const line2Controls = useAnimation();
    const shouldReduceMotion = useReducedMotion() ?? false;
    const isControlledRef = useRef(false);

    useImperativeHandle(
      ref,
      () => {
        isControlledRef.current = true;

        return {
          startAnimation: () => {
            if (shouldReduceMotion) return;
            pathControls.start("animate");
            line1Controls.start("animate");
            line2Controls.start("animate");
          },
          stopAnimation: () => {
            pathControls.start("normal");
            line1Controls.start("normal");
            line2Controls.start("normal");
          },
        };
      },
      [line1Controls, line2Controls, pathControls, shouldReduceMotion],
    );

    const handleMouseEnter = useCallback(
      (event: MouseEvent<HTMLDivElement>) => {
        if (isControlledRef.current) {
          onMouseEnter?.(event);
        } else if (!shouldReduceMotion) {
          pathControls.start("animate");
          line1Controls.start("animate");
          line2Controls.start("animate");
        }
      },
      [line1Controls, line2Controls, onMouseEnter, pathControls, shouldReduceMotion],
    );

    const handleMouseLeave = useCallback(
      (event: MouseEvent<HTMLDivElement>) => {
        if (isControlledRef.current) {
          onMouseLeave?.(event);
        } else {
          pathControls.start("normal");
          line1Controls.start("normal");
          line2Controls.start("normal");
        }
      },
      [line1Controls, line2Controls, onMouseLeave, pathControls],
    );

    return (
      <div
        aria-hidden="true"
        className={cn(className)}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        {...props}
      >
        <svg
          className="h-full w-full"
          fill="none"
          height={size}
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="2"
          viewBox="0 0 24 24"
          width={size}
          xmlns="http://www.w3.org/2000/svg"
        >
          <motion.path
            animate={pathControls}
            d="M21 2H3v16h5v4l4-4h5l4-4V2z"
            initial="normal"
            variants={PATH_VARIANTS}
          />
          <motion.path
            animate={line1Controls}
            d="M11 11V7"
            initial="normal"
            variants={PATH_VARIANTS}
          />
          <motion.path
            animate={line2Controls}
            d="M16 11V7"
            initial="normal"
            variants={PATH_VARIANTS}
          />
        </svg>
      </div>
    );
  },
);

TwitchIcon.displayName = "TwitchIcon";

export { TwitchIcon };
