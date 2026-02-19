// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { DiffStatus } from "@deno/doc/html-types";

export { Anchor } from "./Anchor.tsx";
export { Breadcrumbs } from "./Breadcrumbs.tsx";
export { CategoryPanel } from "./CategoryPanel.tsx";
export { Deprecated } from "./Deprecated.tsx";
export { DocBlockSubtitleClass } from "./DocBlockSubtitleClass.tsx";
export { DocBlockSubtitleInterface } from "./DocBlockSubtitleInterface.tsx";
export { DocEntry } from "./DocEntry.tsx";
export { DocNodeKindIcon } from "./DocNodeKindIcon.tsx";
export { Example } from "./Example.tsx";
export { Function } from "./Function.tsx";
export { IndexSignature } from "./IndexSignature.tsx";
export { ModuleDoc } from "./ModuleDoc.tsx";
export { NamespaceSection } from "./NamespaceSection.tsx";
export { Section } from "./Section.tsx";
export { See } from "./See.tsx";
export { SourceButton } from "./SourceButton.tsx";
export { SymbolContent } from "./SymbolContent.tsx";
export { SymbolGroup } from "./SymbolGroup.tsx";
export { Tag } from "./Tag.tsx";
export { Toc } from "./Toc.tsx";

export function getDiffColor(
  diffStatus: DiffStatus | undefined,
  allowModified: boolean,
) {
  if (!diffStatus) return "";

  switch (diffStatus.kind) {
    case "added":
      return "diff-added";
    case "removed":
      return "diff-removed";
    case "modified": {
      if (allowModified) {
        return "diff-modified";
      } else {
        return "";
      }
    }
    case "renamed":
      return "";
  }
}
