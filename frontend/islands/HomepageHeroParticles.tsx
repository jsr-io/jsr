// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect } from "preact/hooks";

declare global {
  interface Window {
    particlesJS: {
      load: (id: string, path: object, callback: () => void) => void;
    };
  }
}

const config = {
  particles: {
    number: {
      value: 56,
      density: {
        enable: true,
        value_area: 2084,
      },
    },
    color: {
      value: [
        "#22d3ee",
        "#ffd100",
        "#0e7490",
        "#a5f3fc",
        "#083344",
        "#cffafe",
        "#cbd5e1",
      ],
    },
    shape: {
      type: "polygon",
      stroke: {
        width: 0,
        color: "#22d3ee",
      },
      polygon: {
        nb_sides: 4,
      },
    },
    opacity: {
      value: 1,
      random: false,
    },
    size: {
      value: 14,
      random: true,
    },
    line_linked: {
      enable: true,
      distance: 160,
      color: "#22d3ee",
      opacity: 1,
      width: 1,
    },
    move: {
      enable: true,
      speed: 0.45,
      direction: "top",
      random: false,
      straight: false,
      out_mode: "out",
      bounce: false,
    },
  },
  interactivity: {
    detect_on: "canvas",
    events: {
      onhover: {
        enable: true,
        mode: "grab",
      },
      onclick: {
        enable: true,
        mode: "push",
      },
      resize: true,
    },
    modes: {
      grab: {
        distance: 140,
        line_linked: {
          opacity: 1,
        },
      },
      bubble: {
        distance: 400,
        size: 40,
        duration: 2,
        opacity: 8,
        speed: 3,
      },
      repulse: {
        distance: 200,
        duration: 0.4,
      },
      push: {
        particles_nb: 1,
      },
      remove: {
        particles_nb: 2,
      },
    },
  },
  retina_detect: true,
};

export function HomepageHeroParticles() {
  useEffect(() => {
    const reducedMotionQuery = globalThis.matchMedia(
      "(prefers-reduced-motion: reduce)",
    );
    const prefersReducedMotion = reducedMotionQuery.matches;
    if (prefersReducedMotion) config.particles.move.speed = 0;

    window.particlesJS.load(
      "particles-js",
      config,
      () => {
        const canvas = document.querySelector(
          ".particles-js-canvas-el",
        ) as HTMLCanvasElement;
        canvas.style.opacity = "1";
        canvas.ariaHidden = "true";
      },
    );
  }, []);

  return <></>;
}
