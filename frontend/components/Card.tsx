// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";

type CardVariant = "cyan" | "gray" | "red" | "blue" | "green";

interface CardProps {
  children: ComponentChildren;
  href?: string;
  variant?: CardVariant;
  filled?: boolean;
  interactive?: boolean;
  class?: string;
}

const variantStyles: Record<
  CardVariant,
  { base: string; filled: string; interactive: string }
> = {
  cyan: {
    base: "border-jsr-cyan-200 dark:border-jsr-cyan-800",
    filled: "bg-jsr-cyan-50 dark:bg-jsr-cyan-900/30",
    interactive:
      "hover:border-jsr-cyan-400 dark:hover:border-jsr-cyan-600 hover:bg-jsr-cyan-50/50 dark:hover:bg-jsr-cyan-900/30",
  },
  gray: {
    base: "border-jsr-gray-200 dark:border-jsr-gray-800",
    filled: "bg-jsr-gray-50 dark:bg-jsr-gray-900/30",
    interactive:
      "hover:border-jsr-gray-300 dark:hover:border-jsr-gray-700 hover:bg-jsr-gray-50/50 dark:hover:bg-jsr-gray-900/30",
  },
  red: {
    base: "border-red-200 dark:border-red-800",
    filled: "bg-red-50 dark:bg-red-900/30",
    interactive:
      "hover:border-red-300 dark:hover:border-red-700 hover:bg-red-100 dark:hover:bg-red-800/40",
  },
  blue: {
    base: "border-blue-200 dark:border-blue-700",
    filled: "bg-blue-50 dark:bg-blue-900/30",
    interactive:
      "hover:border-blue-300 dark:hover:border-blue-600 hover:bg-blue-100 dark:hover:bg-blue-800/40",
  },
  green: {
    base: "border-green-300 dark:border-green-700",
    filled: "bg-green-50 dark:bg-green-900/30",
    interactive:
      "hover:border-green-400 dark:hover:border-green-600 hover:bg-green-100 dark:hover:bg-green-800/40",
  },
};

export function Card({
  children,
  href,
  variant = "cyan",
  filled = false,
  interactive,
  class: className,
}: CardProps) {
  const styles = variantStyles[variant];
  const isInteractive = interactive ?? !!href;

  const baseClasses = `border-1.5 ${styles.base} rounded-md px-4 py-4 block`;
  const filledClasses = filled ? styles.filled : "";
  const interactiveClasses = isInteractive
    ? `${styles.interactive} transition-colors`
    : "";

  const classes =
    `${baseClasses} ${filledClasses} ${interactiveClasses} ${className ?? ""}`
      .trim();

  if (href) {
    return (
      <a href={href} class={classes}>
        {children}
      </a>
    );
  }
  return <div class={classes}>{children}</div>;
}
