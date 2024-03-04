// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function AnimatedLogo() {
  return (
    <svg
      width="100%"
      height="100%"
      id="jsr-logo"
      viewBox="0 0 833 539"
      version="1.1"
      xmlns="http://www.w3.org/2000/svg"
      xmlnsXlink="http://www.w3.org/1999/xlink"
      xml:space="preserve"
      style="fill-rule:evenodd;clip-rule:evenodd;stroke-linejoin:round;stroke-miterlimit:2;"
      aria-hidden="true"
    >
      <style>
        {`#jsr-logo{height:99px}@media(min-width:768px){#jsr-logo{height:143px}}#jsr-logo g,#jsr-logo g rect{transform-origin:center}#jsr-logo g:nth-of-type(28) rect,#jsr-logo g:nth-of-type(36) rect{animation-duration:1287ms}#jsr-logo g:first-of-type rect,#jsr-logo g:nth-of-type(18) rect,#jsr-logo g:nth-of-type(20) rect,#jsr-logo g:nth-of-type(22) rect,#jsr-logo g:nth-of-type(32) rect,#jsr-logo g:nth-of-type(33) rect,#jsr-logo g:nth-of-type(36) rect,#jsr-logo g:nth-of-type(38) rect,#jsr-logo g:nth-of-type(44) rect,#jsr-logo g:nth-of-type(45) rect,#jsr-logo g:nth-of-type(5) rect,#jsr-logo g:nth-of-type(53) rect,#jsr-logo g:nth-of-type(56) rect{animation-name:jsr_logo_square_slide_in_down}#jsr-logo g:nth-of-type(12) rect,#jsr-logo g:nth-of-type(58) rect{animation-duration:1841ms}#jsr-logo g:nth-of-type(10) rect,#jsr-logo g:nth-of-type(15) rect,#jsr-logo g:nth-of-type(16) rect,#jsr-logo g:nth-of-type(23) rect,#jsr-logo g:nth-of-type(26) rect,#jsr-logo g:nth-of-type(27) rect,#jsr-logo g:nth-of-type(29) rect,#jsr-logo g:nth-of-type(34) rect,#jsr-logo g:nth-of-type(35) rect,#jsr-logo g:nth-of-type(40) rect,#jsr-logo g:nth-of-type(49) rect,#jsr-logo g:nth-of-type(52) rect,#jsr-logo g:nth-of-type(55) rect,#jsr-logo g:nth-of-type(58) rect,#jsr-logo g:nth-of-type(8) rect,#jsr-logo g:nth-of-type(9) rect{animation-name:jsr_logo_square_slide_in_left}#jsr-logo g:nth-of-type(11) rect,#jsr-logo g:nth-of-type(12) rect,#jsr-logo g:nth-of-type(19) rect,#jsr-logo g:nth-of-type(21) rect,#jsr-logo g:nth-of-type(24) rect,#jsr-logo g:nth-of-type(28) rect,#jsr-logo g:nth-of-type(30) rect,#jsr-logo g:nth-of-type(31) rect,#jsr-logo g:nth-of-type(37) rect,#jsr-logo g:nth-of-type(39) rect,#jsr-logo g:nth-of-type(48) rect,#jsr-logo g:nth-of-type(57) rect,#jsr-logo g:nth-of-type(59) rect{animation-name:jsr_logo_square_slide_in_right}#jsr-logo g:nth-of-type(13) rect,#jsr-logo g:nth-of-type(14) rect,#jsr-logo g:nth-of-type(17) rect,#jsr-logo g:nth-of-type(2) rect,#jsr-logo g:nth-of-type(25) rect,#jsr-logo g:nth-of-type(3) rect,#jsr-logo g:nth-of-type(41) rect,#jsr-logo g:nth-of-type(42) rect,#jsr-logo g:nth-of-type(43) rect,#jsr-logo g:nth-of-type(47) rect,#jsr-logo g:nth-of-type(50) rect,#jsr-logo g:nth-of-type(51) rect,#jsr-logo g:nth-of-type(54) rect,#jsr-logo g:nth-of-type(6) rect,#jsr-logo g:nth-of-type(60) rect,#jsr-logo g:nth-of-type(7) rect{animation-name:jsr_logo_square_slide_in_up}#jsr-logo g rect{animation:.5s cubic-bezier(.77,0,.175,1) forwards jsr_logo_square_slide_in_up;transform-box:fill-box;opacity:1}#jsr-logo g:first-of-type rect{animation-duration:1697ms}#jsr-logo g:nth-of-type(2) rect{animation-duration:1186ms}#jsr-logo g:nth-of-type(3) rect{animation-duration:1754ms}#jsr-logo g:nth-of-type(4) rect,#jsr-logo g:nth-of-type(46) rect{animation-duration:923ms;animation-name:jsr_logo_square_slide_in_right}#jsr-logo g:nth-of-type(5) rect{animation-duration:715ms}#jsr-logo g:nth-of-type(6) rect{animation-duration:601ms}#jsr-logo g:nth-of-type(7) rect{animation-duration:401ms}#jsr-logo g:nth-of-type(8) rect{animation-duration:1345ms}#jsr-logo g:nth-of-type(9) rect{animation-duration:684ms}#jsr-logo g:nth-of-type(10) rect{animation-duration:1051ms}#jsr-logo g:nth-of-type(11) rect{animation-duration:579ms}#jsr-logo g:nth-of-type(13) rect{animation-duration:844ms}#jsr-logo g:nth-of-type(14) rect{animation-duration:958ms}#jsr-logo g:nth-of-type(15) rect{animation-duration:1041ms}#jsr-logo g:nth-of-type(16) rect{animation-duration:786ms}#jsr-logo g:nth-of-type(17) rect{animation-duration:1022ms}#jsr-logo g:nth-of-type(18) rect{animation-duration:849ms}#jsr-logo g:nth-of-type(19) rect{animation-duration:823ms}#jsr-logo g:nth-of-type(20) rect{animation-duration:614ms}#jsr-logo g:nth-of-type(21) rect{animation-duration:1628ms}#jsr-logo g:nth-of-type(22) rect{animation-duration:1008ms}#jsr-logo g:nth-of-type(23) rect{animation-duration:1382ms}#jsr-logo g:nth-of-type(24) rect{animation-duration:1873ms}#jsr-logo g:nth-of-type(25) rect{animation-duration:1059ms}#jsr-logo g:nth-of-type(26) rect{animation-duration:1175ms}#jsr-logo g:nth-of-type(27) rect{animation-duration:570ms}#jsr-logo g:nth-of-type(29) rect{animation-duration:1.47s}#jsr-logo g:nth-of-type(30) rect{animation-duration:1879ms}#jsr-logo g:nth-of-type(31) rect{animation-duration:1488ms}#jsr-logo g:nth-of-type(32) rect{animation-duration:653ms}#jsr-logo g:nth-of-type(33) rect{animation-duration:1409ms}#jsr-logo g:nth-of-type(34) rect{animation-duration:1953ms}#jsr-logo g:nth-of-type(35) rect{animation-duration:1558ms}#jsr-logo g:nth-of-type(37) rect{animation-duration:603ms}#jsr-logo g:nth-of-type(38) rect{animation-duration:1154ms}#jsr-logo g:nth-of-type(39) rect{animation-duration:678ms}#jsr-logo g:nth-of-type(40) rect{animation-duration:1959ms}#jsr-logo g:nth-of-type(41) rect{animation-duration:1862ms}#jsr-logo g:nth-of-type(42) rect{animation-duration:1044ms}#jsr-logo g:nth-of-type(43) rect{animation-duration:1713ms}#jsr-logo g:nth-of-type(44) rect{animation-duration:1159ms}#jsr-logo g:nth-of-type(45) rect{animation-duration:872ms}#jsr-logo g:nth-of-type(47) rect{animation-duration:1.88s}#jsr-logo g:nth-of-type(48) rect{animation-duration:1085ms}#jsr-logo g:nth-of-type(49) rect{animation-duration:685ms}#jsr-logo g:nth-of-type(50) rect{animation-duration:1352ms}#jsr-logo g:nth-of-type(51) rect{animation-duration:683ms}#jsr-logo g:nth-of-type(52) rect{animation-duration:1546ms}#jsr-logo g:nth-of-type(53) rect{animation-duration:605ms}#jsr-logo g:nth-of-type(54) rect{animation-duration:1848ms}#jsr-logo g:nth-of-type(55) rect{animation-duration:1133ms}#jsr-logo g:nth-of-type(56) rect{animation-duration:1635ms}#jsr-logo g:nth-of-type(57) rect{animation-duration:959ms}#jsr-logo g:nth-of-type(59) rect{animation-duration:1411ms}#jsr-logo g:nth-of-type(60) rect{animation-duration:678ms}@keyframes fade_in{from{opacity:0}to{opacity:1}}@keyframes jsr_logo_square_slide_in_up{0%,60%{transform:translateY(200%)}100%{transform:translateY(0)}}@keyframes jsr_logo_square_slide_in_right{0%,60%{transform:translateX(-200%)}100%{transform:translateX(0)}}@keyframes jsr_logo_square_slide_in_down{0%,60%{transform:translateY(-200%)}100%{transform:translateY(0)}}@keyframes jsr_logo_square_slide_in_left{0%,60%{transform:translateX(200%)}100%{transform:translateX(0)}}`}
      </style>
      <g transform="matrix(1,0,0,1,98,98)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,98,147)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,8.52651e-14,196)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,8.52651e-14,245)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,8.52651e-14,294)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,8.52651e-14,343)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,294,196)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,294,245)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,294,294)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,294,343)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,294,392)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,294,98)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,294,147)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,196,196)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,196,245)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,196,294)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,196,343)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,196,392)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,490,196)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,490,245)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,490,294)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,490,343)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,490,392)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,490,147)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,392,196)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,392,245)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,392,294)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,392,343)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,392,392)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,196,98)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,196,147)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,98,196)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,98,245)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,98,294)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,98,343)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,392,98)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,392,147)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,588,196)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,588,245)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,588,294)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,588,147)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,147,98)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,147,147)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,49,196)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,49,245)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,49,294)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,49,343)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,343,196)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,343,245)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,343,294)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,343,343)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,343,392)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,343,98)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,343,147)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,245,196)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,245,245)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,245,294)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,245,343)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,245,392)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,539,196)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,539,245)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,539,294)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,539,147)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,441,196)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,441,245)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,441,294)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,441,343)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,441,392)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,245,98)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,245,147)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,147,196)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,147,245)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,147,294)">
        <rect
          x="98"
          y="0"
          width="49"
          height="49"
          style="fill:#f7df1e;"
        />
      </g>
      <g transform="matrix(1,0,0,1,147,343)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
      <g transform="matrix(1,0,0,1,441,147)">
        <rect x="98" y="0" width="49" height="49" style="fill:#083344" />
      </g>
    </svg>
  );
}
