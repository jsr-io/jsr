const getRandomStyles = () =>
  [
    `animation-duration: ${Math.ceil(Math.random() * 1400) + 600}ms`,
    `animation-name: ${
      [
        "jsr_logo_square_slide_in_down",
        "jsr_logo_square_slide_in_left",
        "jsr_logo_square_slide_in_right",
        "jsr_logo_square_slide_in_up",
      ][Math.floor(Math.random() * 4)]
    }`,
  ].join(";");

const colors = {
  "░": "#f7df1e",
  "█": "#083344",
} as const;

const squares = `
  ███████
  █░█░░░█████
███░█░███░░░█
█░█░█░░░█░█░█
█░░░███░█░███
█████░░░█░█
    ███████
`.split("\n")
  .filter(Boolean)
  .map<Array<{ color: keyof typeof colors; x: number; y: number }>>((line, y) =>
    line.split("").flatMap((color, x) =>
      color === "█" || color === "░" ? [{ color, x, y }] : []
    )
  ).flat();

// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function AnimatedLogo() {
  return (
    <svg
      width="100%"
      height="100%"
      id="jsr-logo"
      viewBox="-2 -2 17 11"
      aria-hidden="true"
    >
      <style>
        {`
        #jsr-logo{height:110px}

        @media(min-width:768px){
          #jsr-logo{height:143px}
        }
        
        #jsr-logo rect{transform-origin:center}
        
        #jsr-logo rect{
          animation:.5s cubic-bezier(.77,0,.175,1) forwards jsr_logo_square_slide_in_up;
          transform-box:fill-box;opacity:1
        }

        @keyframes jsr_logo_square_slide_in_up {
          0%,60% {transform:translateY(2px)}
          100% {transform:translateY(0)}
        }
        
        @keyframes jsr_logo_square_slide_in_right {
          0%,60% {transform:translateX(-2px)}
          100%{transform:translateX(0)}
        }
        
        @keyframes jsr_logo_square_slide_in_down {
          0%,60% {transform:translateY(-2px)}
          100%{transform:translateY(0)}
        }
        
        @keyframes jsr_logo_square_slide_in_left {
          0%,60%{transform:translateX(2px)}
          100%{transform:translateX(0)}
        }`}
      </style>

      {squares.map(({ color, x, y }) => (
        <rect
          x={x}
          y={y}
          width="1"
          height="1"
          fill={colors[color]}
          style={getRandomStyles()}
        />
      ))}
    </svg>
  );
}
