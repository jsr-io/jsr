// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { type MutableRef } from "preact/hooks";
import { Highlight, type Position } from "@orama/highlight";

export interface CloudSearchItem {
  target_id: string;
  name: string;
  file: string;
  doc: string;
  url: string;
  deprecated: boolean;
  scope: string;
  package: string;
}

export function resetPreviousNodes(
  previousResultNodes: MutableRef<HTMLElement[]>,
  previousSections: MutableRef<Set<HTMLElement>>,
) {
  for (const node of previousResultNodes.current) {
    node.style.setProperty("display", "none");
    node.querySelectorAll("mark.orama-highlight").forEach((el) => {
      el.replaceWith(...el.childNodes);
    });
    node.normalize();
  }
  previousResultNodes.current = [];

  for (const section of previousSections.current) {
    section.hidden = true;
  }
  previousSections.current.clear();
}

export function highlight(
  highlighter: Highlight,
  term: string,
  searchDescription: string,
  section: HTMLElement,
  previousSections: MutableRef<Set<HTMLElement>>,
  node: HTMLElement,
  previousResultNodes: MutableRef<HTMLElement[]>,
) {
  section.hidden = false;
  previousSections.current.add(section);

  node.style.removeProperty("display");
  previousResultNodes.current.push(node);

  const titleElement = node.getElementsByClassName(
    "namespaceItemContent",
  )[0]
    .children[0] as HTMLAnchorElement;
  titleElement.innerHTML = highlighter.highlight(titleElement.title, term).HTML;

  const description = node.getElementsByClassName(
    "markdown_summary",
  )[0] as HTMLElement;

  if (description) {
    const positions = highlighter.highlight(searchDescription, term).positions;

    highlightSection(positions, description);
  }

  const subitems = node.getElementsByClassName(
    "namespaceItemContentSubItems",
  )[0] as HTMLElement;

  if (subitems) {
    for (const subitem of subitems.children) {
      const positions =
        highlighter.highlight(subitem.textContent ?? "", term).positions;

      highlightSection(positions, subitem as HTMLElement);
    }
  }
}

function highlightSection(positions: Position[], element: HTMLElement) {
  if (positions.length > 0) {
    const walker = document.createTreeWalker(
      element,
      NodeFilter.SHOW_TEXT,
    );

    let currentPosition = 0;
    let node = walker.nextNode();
    while (node && positions.length) {
      const currentNode = walker.currentNode as Text;
      const textContent = currentNode.textContent!;
      const length = textContent.length;

      const fragments = [];
      let start = 0;

      positionsLoop: for (let i = 0; i < positions.length; i++) {
        const position = positions[i];
        const localStart = position.start - currentPosition;
        const localEnd = position.end - currentPosition;

        if (localStart >= length) {
          // if the start is after the current node, there cannot be more highlights for this node
          break positionsLoop;
        }

        if ((localStart >= 0) && (localEnd < length)) {
          fragments.push(
            textContent.slice(start, localStart),
            textContent.slice(localStart, localEnd + 1),
          );
          start = localEnd + 1;
          positions.shift();
          i--; // we need to recheck the current position
        } else if (localStart >= 0) {
          fragments.push(
            textContent.slice(start, localStart),
            textContent.slice(localStart),
          );
          start = length;
          // if the end is not in this node, there cannot be more highlights for this node
          break positionsLoop;
        } else if (localEnd < length) {
          fragments.push(
            "",
            textContent.slice(start, localEnd + 1),
          );
          start = localEnd + 1;
          positions.shift();
          i--; // we need to recheck the current position
        } else {
          break positionsLoop;
        }
      }

      if (start !== length) {
        fragments.push(textContent.slice(start));
      }

      currentPosition += length;

      node = walker.nextNode();
      if (fragments.length > 1) {
        currentNode.replaceWith(
          document.createRange().createContextualFragment(
            fragments
              .map((fragment, i) =>
                i % 2 === 0
                  ? fragment
                  : fragment !== ""
                  ? `<mark class="orama-highlight">${fragment}</mark>`
                  : ""
              )
              .join(""),
          ),
        );
      }
    }
  }
}
