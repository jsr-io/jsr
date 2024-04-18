// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { signal } from "@preact/signals";
import { Logo } from "../components/Logo.tsx";

const rotationDegrees = signal(0);
const isAnimating = signal(false);

export function HeaderLogo(props: { class?: string }) {
  return (
    <div
      onMouseEnter={() => {
        if (isAnimating.value) return;
        isAnimating.value = true;
        rotationDegrees.value += 180;
      }}
    >
      <div
        class={`h-auto transition-transform duration-300 ${props.class}`}
        style={`transform: rotate(${rotationDegrees.value}deg)`}
        aria-hidden="true"
        onTransitionEnd={() => isAnimating.value = false}
      >
        <Logo size="medium" />
      </div>
    </div>
  );
}
