// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { RuntimeCompat } from "../utils/api_types.ts";
import { define } from "../util.ts";
import { Card } from "../components/Card.tsx";
import { Nav, NavItem } from "../components/Nav.tsx";
import { Table, TableData, TableRow } from "../components/Table.tsx";
import { ListDisplay } from "../components/List.tsx";
import type { ListDisplayItem } from "../components/List.tsx";
import { ListPanel } from "../components/ListPanel.tsx";
import type { PanelEntry } from "../components/ListPanel.tsx";
import { Tooltip } from "../components/Tooltip.tsx";
import { Logo } from "../components/Logo.tsx";
import { RuntimeCompatIndicator } from "../components/RuntimeCompatIndicator.tsx";
import { QuotaCard } from "../components/QuotaCard.tsx";
import { CopyButton } from "../islands/CopyButton.tsx";

function Section(
  { title, description, children }: {
    title: string;
    description: string;
    children: preact.ComponentChildren;
  },
) {
  return (
    <section class="space-y-4">
      <h2 class="text-2xl font-bold">{title}</h2>
      <p class="text-tertiary">{description}</p>
      <div>{children}</div>
    </section>
  );
}

export default define.page<typeof handler>(function Components({ url }) {
  const listItems: ListDisplayItem[] = [
    { href: "#", content: <span>First item in the list</span> },
    { href: "#", content: <span>Second item in the list</span> },
    { href: "#", content: <span>Third item in the list</span> },
  ];

  const panelEntries: PanelEntry[] = [
    { value: "1.0.0", href: "#", label: "latest" },
    { value: "0.9.0", href: "#" },
    { value: "0.8.0", href: "#" },
    { value: "0.7.0", href: "#" },
  ];

  const compatFull: RuntimeCompat = {
    browser: true,
    deno: true,
    node: true,
    workerd: true,
    bun: true,
  };
  const compatPartial: RuntimeCompat = {
    deno: true,
    node: true,
  };
  const compatUnknown: RuntimeCompat = {};

  const columns = [
    { title: "Name" },
    { title: "Version" },
    { title: "Score", align: "right" as const },
  ];

  return (
    <div class="space-y-16 pb-16">
      <div>
        <h1 class="text-3xl font-bold">Component Library</h1>
        <p class="text-tertiary mt-2">
          Visual reference of reusable UI components used across JSR.
        </p>
      </div>

      {/* ── CSS Components ──────────────────────────────── */}

      <div class="space-y-12">
        <h2 class="text-2xl font-bold border-b pb-2">CSS Components</h2>

        <Section
          title="Buttons"
          description="Primary, danger, and small button variants with disabled states."
        >
          <div class="flex flex-wrap items-center gap-4">
            <button class="button-primary">Primary</button>
            <button class="button-danger">Danger</button>
            <button class="button-primary button-sm">Primary Small</button>
            <button class="button-danger button-sm">Danger Small</button>
            <button class="button-primary" disabled>Primary Disabled</button>
            <button class="button-danger" disabled>Danger Disabled</button>
          </div>
        </Section>

        <Section
          title="Chips"
          description="Small and large chip/badge styles."
        >
          <div class="flex flex-wrap items-center gap-4">
            <span class="chip bg-jsr-cyan-100 text-jsr-cyan-700">chip</span>
            <span class="chip bg-jsr-yellow-200 text-jsr-yellow-800">
              chip yellow
            </span>
            <span class="chip bg-red-100 text-red-700">chip red</span>
            <span class="big-chip bg-jsr-cyan-100 text-jsr-cyan-700">
              big-chip
            </span>
            <span class="big-chip bg-jsr-yellow-200 text-jsr-yellow-800">
              big-chip yellow
            </span>
          </div>
        </Section>

        <Section
          title="Links"
          description="Standard and header link styles."
        >
          <div class="flex flex-wrap items-center gap-6">
            <a href="#" class="link">Standard link</a>
            <a href="#" class="link-header">Header link</a>
          </div>
        </Section>

        <Section
          title="Inputs"
          description="Text inputs, search input, and select dropdown."
        >
          <div class="space-y-4 max-w-md">
            <div class="input-container px-4 py-2.5">
              <input
                type="text"
                class="input w-full"
                placeholder="Text input"
              />
            </div>
            <div class="input-container px-4 py-2.5">
              <input
                type="text"
                class="input w-full"
                placeholder="Disabled input"
                disabled
              />
            </div>
            <div class="search-input px-4 py-2.5">
              <input
                type="text"
                class="input w-full"
                placeholder="Search input"
              />
            </div>
            <div class="input-container px-4 py-2.5">
              <select class="select w-full pr-6">
                <option>Option 1</option>
                <option>Option 2</option>
                <option>Option 3</option>
              </select>
            </div>
          </div>
        </Section>
      </div>

      {/* ── Preact Components ───────────────────────────── */}

      <div class="space-y-12">
        <h2 class="text-2xl font-bold border-b pb-2">Components</h2>

        <Section
          title="Card"
          description="Card component with 6 color variants, filled/unfilled, and interactive/static modes."
        >
          <div class="space-y-6">
            <div class="grid grid-cols-2 md:grid-cols-3 gap-4">
              {(["cyan", "gray", "red", "blue", "green", "orange"] as const)
                .map((variant) => (
                  <Card variant={variant} key={variant}>
                    <p class="font-semibold capitalize">{variant}</p>
                    <p class="text-sm text-secondary">Unfilled card</p>
                  </Card>
                ))}
            </div>
            <div class="grid grid-cols-2 md:grid-cols-3 gap-4">
              {(["cyan", "gray", "red", "blue", "green", "orange"] as const)
                .map((variant) => (
                  <Card variant={variant} filled key={variant}>
                    <p class="font-semibold capitalize">{variant} filled</p>
                    <p class="text-sm">Filled card</p>
                  </Card>
                ))}
            </div>
            <div class="grid grid-cols-2 gap-4">
              <Card href="#" variant="cyan">
                <p class="font-semibold">Interactive (link)</p>
                <p class="text-sm text-secondary">Has hover styles</p>
              </Card>
              <Card variant="cyan">
                <p class="font-semibold">Static</p>
                <p class="text-sm text-secondary">No hover styles</p>
              </Card>
            </div>
          </div>
        </Section>

        <Section
          title="Nav + NavItem"
          description="Navigation bar with active/inactive items, chip counts, and notification badges."
        >
          <Nav>
            <NavItem href="#" active>Overview</NavItem>
            <NavItem href="#">Versions</NavItem>
            <NavItem href="#" chip={12}>Dependencies</NavItem>
            <NavItem href="#" chip={3} notification>Issues</NavItem>
            <NavItem href="#">Settings</NavItem>
          </Nav>
        </Section>

        <Section
          title="Table + TableRow + TableData"
          description="Data table with styled rows."
        >
          <Table columns={columns} currentUrl={url}>
            <TableRow>
              <TableData>@std/fs</TableData>
              <TableData>1.0.0</TableData>
              <TableData align="right">98</TableData>
            </TableRow>
            <TableRow>
              <TableData>@std/path</TableData>
              <TableData>0.224.0</TableData>
              <TableData align="right">95</TableData>
            </TableRow>
            <TableRow>
              <TableData>@oak/oak</TableData>
              <TableData>16.1.0</TableData>
              <TableData align="right">87</TableData>
            </TableRow>
          </Table>
        </Section>

        <Section
          title="ListDisplay"
          description="Simple navigable list of items."
        >
          <ListDisplay>{listItems}</ListDisplay>
        </Section>

        <Section
          title="ListPanel"
          description="Panel with selectable entries and optional labels."
        >
          <div class="max-w-xs">
            <ListPanel title="Versions" selected="1.0.0">
              {panelEntries}
            </ListPanel>
          </div>
        </Section>

        <Section
          title="Tooltip"
          description="Hover over the element to see the tooltip."
        >
          <div class="flex gap-6">
            <Tooltip tooltip="This is a tooltip">
              <span class="button-primary cursor-default">Hover me</span>
            </Tooltip>
          </div>
        </Section>

        <Section
          title="Logo"
          description="JSR logo at all 3 sizes: small, medium, and large."
        >
          <div class="flex items-end gap-6">
            <Logo size="small" />
            <Logo size="medium" />
            <Logo size="large" />
          </div>
        </Section>

        <Section
          title="RuntimeCompatIndicator"
          description="Runtime compatibility icons for various states."
        >
          <div class="space-y-4">
            <div class="flex items-center gap-4">
              <span class="text-sm w-24">All runtimes:</span>
              <RuntimeCompatIndicator runtimeCompat={compatFull} />
            </div>
            <div class="flex items-center gap-4">
              <span class="text-sm w-24">Partial:</span>
              <RuntimeCompatIndicator runtimeCompat={compatPartial} />
            </div>
            <div class="flex items-center gap-4">
              <span class="text-sm w-24">Unknown:</span>
              <RuntimeCompatIndicator runtimeCompat={compatUnknown} />
            </div>
            <div class="flex items-center gap-4">
              <span class="text-sm w-24">Compact:</span>
              <RuntimeCompatIndicator
                runtimeCompat={compatFull}
                compact
              />
            </div>
          </div>
        </Section>

        <Section
          title="QuotaCard"
          description="Quota usage cards at various utilization levels."
        >
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
            <QuotaCard
              title="Low usage"
              description="20 of 100 used"
              limit={100}
              usage={20}
            />
            <QuotaCard
              title="High usage"
              description="85 of 100 used"
              limit={100}
              usage={85}
            />
            <QuotaCard
              title="Critical usage"
              description="95 of 100 used"
              limit={100}
              usage={95}
            />
            <QuotaCard
              title="Over limit"
              description="110 of 100 used"
              limit={100}
              usage={110}
            />
          </div>
        </Section>

        <Section
          title="CopyButton"
          description="Click to copy text to clipboard."
        >
          <div class="flex items-center gap-4">
            <code class="bg-jsr-gray-100 dark:bg-jsr-gray-800 px-3 py-1.5 rounded text-sm">
              deno add @std/fs
            </code>
            <CopyButton title="Copy command" text="deno add @std/fs" />
          </div>
        </Section>
      </div>
    </div>
  );
});

export const handler = define.handlers({
  GET(ctx) {
    ctx.state.meta = {
      title: "Component Library",
      description: "Visual reference of reusable UI components",
    };
    return { data: {} };
  },
});

