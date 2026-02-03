// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { SectionContentCtx, SectionCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";
import { DocEntry } from "./DocEntry.tsx";
import { Example } from "./Example.tsx";
import { IndexSignature } from "./IndexSignature.tsx";
import { NamespaceSection } from "./NamespaceSection.tsx";
import { See } from "./See.tsx";

function SectionContent({ content }: { content: SectionContentCtx }) {
  switch (content.kind) {
    case "empty":
      return null;
    case "namespace_section":
      return <NamespaceSection items={content.content} />;
    case "see":
      return <See items={content.content} />;
    case "example":
      return (
        <div class="space-y-8">
          {content.content.map((example, index, arr) => (
            <>
              <Example example={example} />
              {index < arr.length - 1 && (
                <div class="border-b border-gray-300 dark:border-gray-700" />
              )}
            </>
          ))}
        </div>
      );
    case "index_signature":
      return (
        <div class="space-y-8">
          {content.content.map((sig, i) => (
            <IndexSignature key={i} signature={sig} />
          ))}
        </div>
      );
    case "doc_entry":
      return (
        <div class="space-y-8">
          {content.content.map((entry, i) => (
            <DocEntry
              key={i}
              entry={entry}
            />
          ))}
        </div>
      );
    default:
      return null;
  }
}

export function Section(
  { section: { header, content } }: { section: SectionCtx },
) {
  return (
    <section class="section" id={header?.anchor.id}>
      {header && (
        <div>
          <h2 class="anchorable mb-1">
            <Anchor anchor={header.anchor} />
            {header.href
              ? (
                <a href={header.href} class="contextLink">
                  {header.title}
                </a>
              )
              : header.title}
          </h2>
          {header.doc && (
            <div
              // jsdoc rendering
              // deno-lint-ignore react-no-danger
              dangerouslySetInnerHTML={{ __html: header.doc }}
            />
          )}
        </div>
      )}

      <SectionContent content={content} />
    </section>
  );
}
