// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";

type CardVariant = "cyan" | "gray";

interface CardProps {
  children: ComponentChildren;
  href?: string;
  variant?: CardVariant;
  class?: string;
}

const variantStyles: Record<CardVariant, { base: string; interactive: string }> =
  {
    cyan: {
      base: "border-jsr-cyan-200 dark:border-jsr-cyan-800",
      interactive:
        "hover:border-jsr-cyan-400 dark:hover:border-jsr-cyan-600 hover:bg-jsr-cyan-50/50 dark:hover:bg-jsr-cyan-900/30",
    },
    gray: {
      base: "border-jsr-gray-200 dark:border-jsr-gray-800",
      interactive:
        "hover:border-jsr-gray-300 dark:hover:border-jsr-gray-700 hover:bg-jsr-gray-50/50 dark:hover:bg-jsr-gray-900/30",
    },
  };

export function Card({
  children,
  href,
  variant = "cyan",
  class: className,
}: CardProps) {
  const styles = variantStyles[variant];
  const baseClasses = `border-1.5 ${styles.base} rounded-md px-4 py-4 block`;
  const interactiveClasses = href
    ? `${styles.interactive} transition-colors`
    : "";

  const classes = `${baseClasses} ${interactiveClasses} ${className ?? ""}`;

  if (href) {
    return (
      <a href={href} class={classes}>
        {children}
      </a>
    );
  }
  return <div class={classes}>{children}</div>;
}
