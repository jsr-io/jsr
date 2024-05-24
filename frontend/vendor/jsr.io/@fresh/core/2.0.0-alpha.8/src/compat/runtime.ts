import { type ComponentChildren, Fragment, h, type VNode } from "npm:preact@^10.20.2";

/**
 * @deprecated FIXME explain why + link to docs
 */
export function Head({ children }: { children: ComponentChildren }): VNode {
  return h(Fragment, null, children);
}
