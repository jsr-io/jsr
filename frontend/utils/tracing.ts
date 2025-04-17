// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import {
  AttributeValue,
  CloudTrace,
  CredentialsClient,
  Span,
} from "./cloudtrace:v2.ts";

const CLOUD_TRACE = Deno.env.get("CLOUD_TRACE") === "true";
let CLOUD_TRACE_AUTH: CredentialsClient | null = null;
if (CLOUD_TRACE) {
  const resp = await fetch(
    "http://metadata.google.internal/computeMetadata/v1/project/project-id",
    { headers: { "Metadata-Flavor": "Google" } },
  );
  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(
      `Failed to fetch project id for Cloud Trace: ${resp.status}: ${text}`,
    );
  }
  const projectId = await resp.text();

  let token: { token: string; expiresAt: Date } | null = null;

  CLOUD_TRACE_AUTH = {
    projectId,
    async getRequestHeaders(): Promise<Record<string, string>> {
      if (token === null || token.expiresAt.getTime() < Date.now()) {
        const resp = await fetch(
          "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token",
          { headers: { "Metadata-Flavor": "Google" } },
        );
        if (!resp.ok) {
          const text = await resp.text();
          throw new Error(
            `Failed to fetch access token for Cloud Trace: ${resp.status}: ${text}`,
          );
        }
        const json = await resp.json();
        token = {
          token: json.access_token,
          expiresAt: new Date(json.expires_in * 1000),
        };
      }
      return { Authorization: `Bearer ${token.token}` };
    },
  };
}

const OTLP_ENDPOINT = Deno.env.get("OTLP_ENDPOINT");

const FLUSH_INTERVAL = 1000; // 5s

interface RecordedSpan {
  traceId: string;
  parentSpanId: string | null;
  spanId: string;
  startTime: Date;
  endTime: Date;
  displayName: string;
  attributes: Record<string, string | bigint | boolean>;
}

const BATCH_SPAN_IMMEDIATE_FLUSH_LEN = 100;
const BATCH_SPAN_OVERFLOW_LEN = 1000;

export class Tracer {
  #samplingRate = 0.1;

  #spans: RecordedSpan[] = [];
  #timerId: number | null = null;

  #cloudTrace: CloudTrace | null = null;
  #otlpEndpoint: string | null = null;

  constructor() {
    if (CLOUD_TRACE_AUTH) this.#cloudTrace = new CloudTrace(CLOUD_TRACE_AUTH);
    if (OTLP_ENDPOINT) this.#otlpEndpoint = OTLP_ENDPOINT;
    if (this.#cloudTrace !== null && this.#otlpEndpoint !== null) {
      throw new Error("Cannot use both Cloud Trace and OTLP");
    }
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
      this.#timerId = setTimeout(() => this.flush(), FLUSH_INTERVAL);
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
      } else if (this.#cloudTrace !== null) {
        await this.#flushCloudTrace(spans);
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

  async #flushCloudTrace(spans: RecordedSpan[]) {
    const projectName = `projects/${CLOUD_TRACE_AUTH?.projectId}`;
    const cloudTraceSpans = spans.map<Span>((span) => {
      const attributeMap = Object.fromEntries(
        Object.entries(span.attributes).map(([key, value]) => {
          let v: AttributeValue;
          switch (typeof value) {
            case "string":
              v = { stringValue: { value } };
              break;
            case "bigint":
              v = { intValue: value };
              break;
            case "boolean":
              v = { boolValue: value };
              break;
            default:
              throw new Error(`Unsupported attribute type: ${typeof value}`);
          }
          return [key, v];
        }),
      );
      return {
        name: `${projectName}/traces/${span.traceId}/spans/${span.spanId}`,
        spanId: span.spanId,
        parentSpanId: span.parentSpanId ?? undefined,
        displayName: { value: span.displayName },
        startTime: span.startTime,
        endTime: span.endTime,
        attributes: { attributeMap },
      } satisfies Span;
    });
    await this.#cloudTrace!.projectsTracesBatchWrite(
      `projects/${CLOUD_TRACE_AUTH?.projectId}`,
      { spans: cloudTraceSpans },
    );
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
    });
  }

  get cloudTraceContext(): string {
    return `${this.#traceId}/${this.#spanId};o=${this.#sampled ? 1 : 0}`;
  }

  get traceparent(): string {
    return `00-${this.traceId}-${this.spanId}-01`;
  }
}
