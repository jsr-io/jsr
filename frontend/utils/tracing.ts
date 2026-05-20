// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { env } from "./env.ts";

// Cloud Trace requires the GCE metadata service, which is only reachable
// from Google Cloud workloads. On Cloudflare Workers we rely on the OTLP
// exporter (set `OTLP_ENDPOINT`) or — by default — no exporter at all, in
// which case spans are silently dropped.

const FLUSH_INTERVAL = 1000;

export type SpanKind = "SERVER" | "CLIENT" | "INTERNAL";

interface RecordedSpan {
  traceId: string;
  parentSpanId: string | null;
  spanId: string;
  startTime: Date;
  endTime: Date;
  displayName: string;
  attributes: Record<string, string | bigint | boolean>;
  spanKind: SpanKind;
}

const OTLP_SPAN_KIND: Record<SpanKind, number> = {
  INTERNAL: 1,
  SERVER: 2,
  CLIENT: 3,
};

const BATCH_SPAN_IMMEDIATE_FLUSH_LEN = 100;
const BATCH_SPAN_OVERFLOW_LEN = 1000;

export class Tracer {
  #samplingRate = 0.1;

  #spans: RecordedSpan[] = [];
  #timerId: number | null = null;

  #otlpEndpoint: string | null;

  constructor() {
    this.#otlpEndpoint = env("OTLP_ENDPOINT") ?? null;
  }

  spanForRequest(req: Request) {
    let parentSpan: TraceSpan | null = null;
    const traceparent = req.headers.get("traceparent");
    if (traceparent !== null) {
      parentSpan = parseTraceParent(traceparent, this);
    }

    const url = new URL(req.url);
    let shouldSample = url.searchParams.has("trace");
    shouldSample ||= Math.random() < this.#samplingRate;

    if (parentSpan !== null) {
      return parentSpan.child(shouldSample);
    } else {
      return TraceSpan.root(shouldSample, this);
    }
  }

  recordSpan(span: RecordedSpan) {
    if (this.#spans.length >= BATCH_SPAN_OVERFLOW_LEN) {
      console.warn("Dropping span, too many spans pending to be sent");
      return;
    }
    this.#spans.push(span);
    if (this.#spans.length >= BATCH_SPAN_IMMEDIATE_FLUSH_LEN) {
      this.flush();
    } else if (this.#timerId === null) {
      this.#timerId = setTimeout(() => this.flush(), FLUSH_INTERVAL) as unknown as number;
    }
  }

  async flush() {
    this.#timerId = null;
    const spans = this.#spans;
    this.#spans = [];
    if (spans.length === 0) return;
    try {
      if (this.#otlpEndpoint !== null) {
        await this.#flushOTLP(spans);
      }
    } catch (err) {
      console.error("Failed to flush spans", err);
      for (const span of spans) {
        this.recordSpan(span);
      }
    } finally {
      this.#timerId = null;
    }
  }

  async #flushOTLP(spans: RecordedSpan[]) {
    const otlpSpans = spans.map((span) => {
      const attributes = Object.entries(span.attributes).map(
        ([key, value]) => {
          let v;
          switch (typeof value) {
            case "string":
              v = { stringValue: value };
              break;
            case "bigint":
              v = { intValue: String(value) };
              break;
            case "boolean":
              v = { boolValue: value };
              break;
            default:
              throw new Error(`Unsupported attribute type: ${typeof value}`);
          }
          return { key, value: v };
        },
      );
      return {
        traceId: span.traceId,
        parentSpanId: span.parentSpanId,
        spanId: span.spanId,
        name: span.displayName,
        kind: OTLP_SPAN_KIND[span.spanKind],
        startTimeUnixNano: span.startTime.getTime() * 1e6,
        endTimeUnixNano: span.endTime.getTime() * 1e6,
        attributes,
      };
    });
    const req = {
      resourceSpans: [
        {
          resource: {
            attributes: [
              { key: "service.name", value: { stringValue: "frontend" } },
            ],
          },
          scopeSpans: [{ spans: otlpSpans }],
        },
      ],
    };
    const resp = await fetch(`${this.#otlpEndpoint!}/v1/traces`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(req),
    });
    if (!resp.ok) {
      const body = await resp.text();
      throw new Error(
        `Failed to send spans to OTLP endpoint: ${resp.status}: ${body}`,
      );
    }
  }

  close() {
    if (this.#timerId !== null) {
      clearTimeout(this.#timerId);
      this.#timerId = null;
    }
    this.flush();
  }
}

let _tracer: Tracer | null = null;
export function getTracer(): Tracer {
  if (_tracer === null) _tracer = new Tracer();
  return _tracer;
}

function parseTraceParent(
  traceparent: string,
  tracer: Tracer,
): TraceSpan | null {
  // Parse traceparent header as per https://www.w3.org/TR/trace-context/#traceparent-header
  const parts = traceparent.split("-");
  if (parts.length !== 4) return null;
  const version = parts[0];
  const traceId = parts[1];
  const spanId = parts[2];
  const flags = parts[3];
  if (version !== "00") return null;
  if (traceId.length !== 32) return null;
  if (spanId.length !== 16) return null;
  const spanIdNum = BigInt("0x" + spanId);
  if (flags.length !== 2) return null;
  const sampled = flags === "01";
  return new TraceSpan(sampled, traceId, null, spanIdNum, tracer);
}

function randomHex(len: number): string {
  let hex = "";
  for (let i = 0; i < len; i++) {
    hex += Math.floor(Math.random() * 16).toString(16);
  }
  return hex;
}

export class TraceSpan {
  #sampled: boolean;
  #traceId: string;
  #parentSpanId: bigint | null;
  #spanId: bigint;
  #tracer: Tracer;

  constructor(
    sampled: boolean,
    traceId: string,
    parentSpanId: bigint | null,
    spanId: bigint,
    tracer: Tracer,
  ) {
    this.#sampled = sampled;
    this.#traceId = traceId;
    this.#parentSpanId = parentSpanId;
    this.#spanId = spanId;
    this.#tracer = tracer;
  }

  static root(sampled: boolean, tracer: Tracer) {
    const traceId = randomHex(32);
    const spanId = BigInt("0x" + randomHex(16));
    return new TraceSpan(sampled, traceId, null, spanId, tracer);
  }

  child(sampleEvenIfParentUnsampled?: boolean): TraceSpan {
    const spanId = BigInt("0x" + randomHex(16));
    return new TraceSpan(
      sampleEvenIfParentUnsampled || this.#sampled,
      this.#traceId,
      this.#spanId,
      spanId,
      this.#tracer,
    );
  }

  get traceId() {
    return this.#traceId;
  }

  get spanId() {
    return this.#spanId.toString(16).padStart(16, "0");
  }

  get parentSpanId() {
    if (this.#parentSpanId === null) return null;
    return this.#parentSpanId.toString(16).padStart(16, "0");
  }

  get isSampled() {
    return this.#sampled;
  }

  record(
    displayName: string,
    startTime: Date,
    endTime: Date,
    attributes: Record<string, string | bigint | boolean>,
    spanKind: SpanKind,
  ) {
    if (!this.isSampled) return;
    this.#tracer.recordSpan({
      traceId: this.traceId,
      spanId: this.spanId,
      parentSpanId: this.parentSpanId,
      startTime,
      endTime,
      displayName,
      attributes,
      spanKind,
    });
  }

  get cloudTraceContext(): string {
    return `${this.#traceId}/${this.#spanId};o=${this.#sampled ? 1 : 0}`;
  }

  get traceparent(): string {
    return `00-${this.traceId}-${this.spanId}-01`;
  }
}
