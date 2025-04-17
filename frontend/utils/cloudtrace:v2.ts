// This file in vendored in because the service is not working properly.
// deno-fmt-ignore-file
// deno-lint-ignore-file

// Copyright 2022 Luca Casonato. All rights reserved. MIT license.
/**
 * Cloud Trace API Client for Deno
 * ===============================
 *
 * Sends application trace data to Cloud Trace for viewing. Trace data is collected for all App Engine applications by default. Trace data from other applications can be provided using this API. This library is used to interact with the Cloud Trace API directly. If you are looking to instrument your application for Cloud Trace, we recommend using OpenTelemetry.
 *
 * Docs: https://cloud.google.com/trace
 * Source: https://googleapis.deno.dev/v1/cloudtrace:v2.ts
 */

import { auth, CredentialsClient, GoogleAuth, request } from "https://googleapis.deno.dev/_/base@v1/mod.ts";
export { auth, GoogleAuth };
export type { CredentialsClient };

/**
 * Sends application trace data to Cloud Trace for viewing. Trace data is
 * collected for all App Engine applications by default. Trace data from other
 * applications can be provided using this API. This library is used to interact
 * with the Cloud Trace API directly. If you are looking to instrument your
 * application for Cloud Trace, we recommend using OpenTelemetry.
 */
export class CloudTrace {
  #client: CredentialsClient | undefined;
  #baseUrl: string;

  constructor(client?: CredentialsClient, baseUrl: string = "https://cloudtrace.googleapis.com/") {
    this.#client = client;
    this.#baseUrl = baseUrl;
  }

  /**
   * Batch writes new spans to new or existing traces. You cannot update
   * existing spans. If a span ID already exists, an additional copy of the span
   * will be stored.
   *
   * @param name Required. The name of the project where the spans belong. The format is `projects/[PROJECT_ID]`.
   */
  async projectsTracesBatchWrite(name: string, req: BatchWriteSpansRequest): Promise<Empty> {
    req = serializeBatchWriteSpansRequest(req);
    const url = new URL(`${this.#baseUrl}v2/${ name }/traces:batchWrite`);
    const body = JSON.stringify(req);
    const data = await request(url.href, {
      client: this.#client,
      method: "POST",
      body,
    });
    return data as Empty;
  }

  /**
   * Creates a new span. If a span ID already exists, an additional copy of the
   * span will be stored.
   *
   * @param name Required. The resource name of the span in the following format: * `projects/[PROJECT_ID]/traces/[TRACE_ID]/spans/[SPAN_ID]` `[TRACE_ID]` is a unique identifier for a trace within a project; it is a 32-character hexadecimal encoding of a 16-byte array. It should not be zero. `[SPAN_ID]` is a unique identifier for a span within a trace; it is a 16-character hexadecimal encoding of an 8-byte array. It should not be zero. .
   */
  async projectsTracesSpansCreateSpan(name: string, req: Span): Promise<Span> {
    req = serializeSpan(req);
    const url = new URL(`${this.#baseUrl}v2/${ name }`);
    const body = JSON.stringify(req);
    const data = await request(url.href, {
      client: this.#client,
      method: "POST",
      body,
    });
    return deserializeSpan(data);
  }
}

/**
 * Text annotation with a set of attributes.
 */
export interface Annotation {
  /**
   * A set of attributes on the annotation. You can have up to 4 attributes per
   * Annotation.
   */
  attributes?: Attributes;
  /**
   * A user-supplied message describing the event. The maximum length for the
   * description is 256 bytes.
   */
  description?: TruncatableString;
}

function serializeAnnotation(data: any): Annotation {
  return {
    ...data,
    attributes: data["attributes"] !== undefined ? serializeAttributes(data["attributes"]) : undefined,
  };
}

function deserializeAnnotation(data: any): Annotation {
  return {
    ...data,
    attributes: data["attributes"] !== undefined ? deserializeAttributes(data["attributes"]) : undefined,
  };
}

/**
 * A set of attributes as key-value pairs.
 */
export interface Attributes {
  /**
   * A set of attributes. Each attribute's key can be up to 128 bytes long. The
   * value can be a string up to 256 bytes, a signed 64-bit integer, or the
   * boolean values `true` or `false`. For example: "/instance_id": {
   * "string_value": { "value": "my-instance" } } "/http/request_bytes": {
   * "int_value": 300 } "example.com/myattribute": { "bool_value": false }
   */
  attributeMap?: {
    [key: string]: AttributeValue
  };
  /**
   * The number of attributes that were discarded. Attributes can be discarded
   * because their keys are too long or because there are too many attributes.
   * If this value is 0 then all attributes are valid.
   */
  droppedAttributesCount?: number;
}

function serializeAttributes(data: any): Attributes {
  return {
    ...data,
    attributeMap: data["attributeMap"] !== undefined ? Object.fromEntries(Object.entries(data["attributeMap"]).map(([k, v]: [string, any]) => ([k, serializeAttributeValue(v)]))) : undefined,
  };
}

function deserializeAttributes(data: any): Attributes {
  return {
    ...data,
    attributeMap: data["attributeMap"] !== undefined ? Object.fromEntries(Object.entries(data["attributeMap"]).map(([k, v]: [string, any]) => ([k, deserializeAttributeValue(v)]))) : undefined,
  };
}

/**
 * The allowed types for `[VALUE]` in a `[KEY]:[VALUE]` attribute.
 */
export interface AttributeValue {
  /**
   * A Boolean value represented by `true` or `false`.
   */
  boolValue?: boolean;
  /**
   * A 64-bit signed integer.
   */
  intValue?: bigint;
  /**
   * A string up to 256 bytes long.
   */
  stringValue?: TruncatableString;
}

function serializeAttributeValue(data: any): AttributeValue {
  return {
    ...data,
    intValue: data["intValue"] !== undefined ? String(data["intValue"]) : undefined,
  };
}

function deserializeAttributeValue(data: any): AttributeValue {
  return {
    ...data,
    intValue: data["intValue"] !== undefined ? BigInt(data["intValue"]) : undefined,
  };
}

/**
 * The request message for the `BatchWriteSpans` method.
 */
export interface BatchWriteSpansRequest {
  /**
   * Required. A list of new spans. The span names must not match existing
   * spans, otherwise the results are undefined.
   */
  spans?: Span[];
}

function serializeBatchWriteSpansRequest(data: any): BatchWriteSpansRequest {
  return {
    ...data,
    spans: data["spans"] !== undefined ? data["spans"].map((item: any) => (serializeSpan(item))) : undefined,
  };
}

function deserializeBatchWriteSpansRequest(data: any): BatchWriteSpansRequest {
  return {
    ...data,
    spans: data["spans"] !== undefined ? data["spans"].map((item: any) => (deserializeSpan(item))) : undefined,
  };
}

/**
 * A generic empty message that you can re-use to avoid defining duplicated
 * empty messages in your APIs. A typical example is to use it as the request or
 * the response type of an API method. For instance: service Foo { rpc
 * Bar(google.protobuf.Empty) returns (google.protobuf.Empty); }
 */
export interface Empty {
}

/**
 * A pointer from the current span to another span in the same trace or in a
 * different trace. For example, this can be used in batching operations, where
 * a single batch handler processes multiple requests from different traces or
 * when the handler receives a request from a different project.
 */
export interface Link {
  /**
   * A set of attributes on the link. Up to 32 attributes can be specified per
   * link.
   */
  attributes?: Attributes;
  /**
   * The `[SPAN_ID]` for a span within a trace.
   */
  spanId?: string;
  /**
   * The `[TRACE_ID]` for a trace within a project.
   */
  traceId?: string;
  /**
   * The relationship of the current span relative to the linked span.
   */
  type?:  | "TYPE_UNSPECIFIED" | "CHILD_LINKED_SPAN" | "PARENT_LINKED_SPAN";
}

function serializeLink(data: any): Link {
  return {
    ...data,
    attributes: data["attributes"] !== undefined ? serializeAttributes(data["attributes"]) : undefined,
  };
}

function deserializeLink(data: any): Link {
  return {
    ...data,
    attributes: data["attributes"] !== undefined ? deserializeAttributes(data["attributes"]) : undefined,
  };
}

/**
 * A collection of links, which are references from this span to a span in the
 * same or different trace.
 */
export interface Links {
  /**
   * The number of dropped links after the maximum size was enforced. If this
   * value is 0, then no links were dropped.
   */
  droppedLinksCount?: number;
  /**
   * A collection of links.
   */
  link?: Link[];
}

function serializeLinks(data: any): Links {
  return {
    ...data,
    link: data["link"] !== undefined ? data["link"].map((item: any) => (serializeLink(item))) : undefined,
  };
}

function deserializeLinks(data: any): Links {
  return {
    ...data,
    link: data["link"] !== undefined ? data["link"].map((item: any) => (deserializeLink(item))) : undefined,
  };
}

/**
 * An event describing a message sent/received between Spans.
 */
export interface MessageEvent {
  /**
   * The number of compressed bytes sent or received. If missing, the
   * compressed size is assumed to be the same size as the uncompressed size.
   */
  compressedSizeBytes?: bigint;
  /**
   * An identifier for the MessageEvent's message that can be used to match
   * `SENT` and `RECEIVED` MessageEvents.
   */
  id?: bigint;
  /**
   * Type of MessageEvent. Indicates whether the message was sent or received.
   */
  type?:  | "TYPE_UNSPECIFIED" | "SENT" | "RECEIVED";
  /**
   * The number of uncompressed bytes sent or received.
   */
  uncompressedSizeBytes?: bigint;
}

function serializeMessageEvent(data: any): MessageEvent {
  return {
    ...data,
    compressedSizeBytes: data["compressedSizeBytes"] !== undefined ? String(data["compressedSizeBytes"]) : undefined,
    id: data["id"] !== undefined ? String(data["id"]) : undefined,
    uncompressedSizeBytes: data["uncompressedSizeBytes"] !== undefined ? String(data["uncompressedSizeBytes"]) : undefined,
  };
}

function deserializeMessageEvent(data: any): MessageEvent {
  return {
    ...data,
    compressedSizeBytes: data["compressedSizeBytes"] !== undefined ? BigInt(data["compressedSizeBytes"]) : undefined,
    id: data["id"] !== undefined ? BigInt(data["id"]) : undefined,
    uncompressedSizeBytes: data["uncompressedSizeBytes"] !== undefined ? BigInt(data["uncompressedSizeBytes"]) : undefined,
  };
}

/**
 * Binary module.
 */
export interface Module {
  /**
   * A unique identifier for the module, usually a hash of its contents (up to
   * 128 bytes).
   */
  buildId?: TruncatableString;
  /**
   * For example: main binary, kernel modules, and dynamic libraries such as
   * libc.so, sharedlib.so (up to 256 bytes).
   */
  module?: TruncatableString;
}

/**
 * A span represents a single operation within a trace. Spans can be nested to
 * form a trace tree. Often, a trace contains a root span that describes the
 * end-to-end latency, and one or more subspans for its sub-operations. A trace
 * can also contain multiple root spans, or none at all. Spans do not need to be
 * contiguous. There might be gaps or overlaps between spans in a trace.
 */
export interface Span {
  /**
   * A set of attributes on the span. You can have up to 32 attributes per
   * span.
   */
  attributes?: Attributes;
  /**
   * Optional. The number of child spans that were generated while this span
   * was active. If set, allows implementation to detect missing child spans.
   */
  childSpanCount?: number;
  /**
   * Required. A description of the span's operation (up to 128 bytes). Cloud
   * Trace displays the description in the Cloud console. For example, the
   * display name can be a qualified method name or a file name and a line
   * number where the operation is called. A best practice is to use the same
   * display name within an application and at the same call point. This makes
   * it easier to correlate spans in different traces.
   */
  displayName?: TruncatableString;
  /**
   * Required. The end time of the span. On the client side, this is the time
   * kept by the local machine where the span execution ends. On the server
   * side, this is the time when the server application handler stops running.
   */
  endTime?: Date;
  /**
   * Links associated with the span. You can have up to 128 links per Span.
   */
  links?: Links;
  /**
   * Required. The resource name of the span in the following format: *
   * `projects/[PROJECT_ID]/traces/[TRACE_ID]/spans/[SPAN_ID]` `[TRACE_ID]` is a
   * unique identifier for a trace within a project; it is a 32-character
   * hexadecimal encoding of a 16-byte array. It should not be zero. `[SPAN_ID]`
   * is a unique identifier for a span within a trace; it is a 16-character
   * hexadecimal encoding of an 8-byte array. It should not be zero. .
   */
  name?: string;
  /**
   * The `[SPAN_ID]` of this span's parent span. If this is a root span, then
   * this field must be empty.
   */
  parentSpanId?: string;
  /**
   * Optional. Set this parameter to indicate whether this span is in the same
   * process as its parent. If you do not set this parameter, Trace is unable to
   * take advantage of this helpful information.
   */
  sameProcessAsParentSpan?: boolean;
  /**
   * Required. The `[SPAN_ID]` portion of the span's resource name.
   */
  spanId?: string;
  /**
   * Optional. Distinguishes between spans generated in a particular context.
   * For example, two spans with the same name may be distinguished using
   * `CLIENT` (caller) and `SERVER` (callee) to identify an RPC call.
   */
  spanKind?:  | "SPAN_KIND_UNSPECIFIED" | "INTERNAL" | "SERVER" | "CLIENT" | "PRODUCER" | "CONSUMER";
  /**
   * Stack trace captured at the start of the span.
   */
  stackTrace?: StackTrace;
  /**
   * Required. The start time of the span. On the client side, this is the time
   * kept by the local machine where the span execution starts. On the server
   * side, this is the time when the server's application handler starts
   * running.
   */
  startTime?: Date;
  /**
   * Optional. The final status for this span.
   */
  status?: Status;
  /**
   * A set of time events. You can have up to 32 annotations and 128 message
   * events per span.
   */
  timeEvents?: TimeEvents;
}

function serializeSpan(data: any): Span {
  return {
    ...data,
    attributes: data["attributes"] !== undefined ? serializeAttributes(data["attributes"]) : undefined,
    endTime: data["endTime"] !== undefined ? data["endTime"].toISOString() : undefined,
    links: data["links"] !== undefined ? serializeLinks(data["links"]) : undefined,
    stackTrace: data["stackTrace"] !== undefined ? serializeStackTrace(data["stackTrace"]) : undefined,
    startTime: data["startTime"] !== undefined ? data["startTime"].toISOString() : undefined,
    timeEvents: data["timeEvents"] !== undefined ? serializeTimeEvents(data["timeEvents"]) : undefined,
  };
}

function deserializeSpan(data: any): Span {
  return {
    ...data,
    attributes: data["attributes"] !== undefined ? deserializeAttributes(data["attributes"]) : undefined,
    endTime: data["endTime"] !== undefined ? new Date(data["endTime"]) : undefined,
    links: data["links"] !== undefined ? deserializeLinks(data["links"]) : undefined,
    stackTrace: data["stackTrace"] !== undefined ? deserializeStackTrace(data["stackTrace"]) : undefined,
    startTime: data["startTime"] !== undefined ? new Date(data["startTime"]) : undefined,
    timeEvents: data["timeEvents"] !== undefined ? deserializeTimeEvents(data["timeEvents"]) : undefined,
  };
}

/**
 * Represents a single stack frame in a stack trace.
 */
export interface StackFrame {
  /**
   * The column number where the function call appears, if available. This is
   * important in JavaScript because of its anonymous functions.
   */
  columnNumber?: bigint;
  /**
   * The name of the source file where the function call appears (up to 256
   * bytes).
   */
  fileName?: TruncatableString;
  /**
   * The fully-qualified name that uniquely identifies the function or method
   * that is active in this frame (up to 1024 bytes).
   */
  functionName?: TruncatableString;
  /**
   * The line number in `file_name` where the function call appears.
   */
  lineNumber?: bigint;
  /**
   * The binary module from where the code was loaded.
   */
  loadModule?: Module;
  /**
   * An un-mangled function name, if `function_name` is mangled. To get
   * information about name mangling, run [this
   * search](https://www.google.com/search?q=cxx+name+mangling). The name can be
   * fully-qualified (up to 1024 bytes).
   */
  originalFunctionName?: TruncatableString;
  /**
   * The version of the deployed source code (up to 128 bytes).
   */
  sourceVersion?: TruncatableString;
}

function serializeStackFrame(data: any): StackFrame {
  return {
    ...data,
    columnNumber: data["columnNumber"] !== undefined ? String(data["columnNumber"]) : undefined,
    lineNumber: data["lineNumber"] !== undefined ? String(data["lineNumber"]) : undefined,
  };
}

function deserializeStackFrame(data: any): StackFrame {
  return {
    ...data,
    columnNumber: data["columnNumber"] !== undefined ? BigInt(data["columnNumber"]) : undefined,
    lineNumber: data["lineNumber"] !== undefined ? BigInt(data["lineNumber"]) : undefined,
  };
}

/**
 * A collection of stack frames, which can be truncated.
 */
export interface StackFrames {
  /**
   * The number of stack frames that were dropped because there were too many
   * stack frames. If this value is 0, then no stack frames were dropped.
   */
  droppedFramesCount?: number;
  /**
   * Stack frames in this call stack.
   */
  frame?: StackFrame[];
}

function serializeStackFrames(data: any): StackFrames {
  return {
    ...data,
    frame: data["frame"] !== undefined ? data["frame"].map((item: any) => (serializeStackFrame(item))) : undefined,
  };
}

function deserializeStackFrames(data: any): StackFrames {
  return {
    ...data,
    frame: data["frame"] !== undefined ? data["frame"].map((item: any) => (deserializeStackFrame(item))) : undefined,
  };
}

/**
 * A call stack appearing in a trace.
 */
export interface StackTrace {
  /**
   * Stack frames in this stack trace. A maximum of 128 frames are allowed.
   */
  stackFrames?: StackFrames;
  /**
   * The hash ID is used to conserve network bandwidth for duplicate stack
   * traces within a single trace. Often multiple spans will have identical
   * stack traces. The first occurrence of a stack trace should contain both the
   * `stackFrame` content and a value in `stackTraceHashId`. Subsequent spans
   * within the same request can refer to that stack trace by only setting
   * `stackTraceHashId`.
   */
  stackTraceHashId?: bigint;
}

function serializeStackTrace(data: any): StackTrace {
  return {
    ...data,
    stackFrames: data["stackFrames"] !== undefined ? serializeStackFrames(data["stackFrames"]) : undefined,
    stackTraceHashId: data["stackTraceHashId"] !== undefined ? String(data["stackTraceHashId"]) : undefined,
  };
}

function deserializeStackTrace(data: any): StackTrace {
  return {
    ...data,
    stackFrames: data["stackFrames"] !== undefined ? deserializeStackFrames(data["stackFrames"]) : undefined,
    stackTraceHashId: data["stackTraceHashId"] !== undefined ? BigInt(data["stackTraceHashId"]) : undefined,
  };
}

/**
 * The `Status` type defines a logical error model that is suitable for
 * different programming environments, including REST APIs and RPC APIs. It is
 * used by [gRPC](https://github.com/grpc). Each `Status` message contains three
 * pieces of data: error code, error message, and error details. You can find
 * out more about this error model and how to work with it in the [API Design
 * Guide](https://cloud.google.com/apis/design/errors).
 */
export interface Status {
  /**
   * The status code, which should be an enum value of google.rpc.Code.
   */
  code?: number;
  /**
   * A list of messages that carry the error details. There is a common set of
   * message types for APIs to use.
   */
  details?: {
    [key: string]: any
  }[];
  /**
   * A developer-facing error message, which should be in English. Any
   * user-facing error message should be localized and sent in the
   * google.rpc.Status.details field, or localized by the client.
   */
  message?: string;
}

/**
 * A time-stamped annotation or message event in the Span.
 */
export interface TimeEvent {
  /**
   * Text annotation with a set of attributes.
   */
  annotation?: Annotation;
  /**
   * An event describing a message sent/received between Spans.
   */
  messageEvent?: MessageEvent;
  /**
   * The timestamp indicating the time the event occurred.
   */
  time?: Date;
}

function serializeTimeEvent(data: any): TimeEvent {
  return {
    ...data,
    annotation: data["annotation"] !== undefined ? serializeAnnotation(data["annotation"]) : undefined,
    messageEvent: data["messageEvent"] !== undefined ? serializeMessageEvent(data["messageEvent"]) : undefined,
    time: data["time"] !== undefined ? data["time"].toISOString() : undefined,
  };
}

function deserializeTimeEvent(data: any): TimeEvent {
  return {
    ...data,
    annotation: data["annotation"] !== undefined ? deserializeAnnotation(data["annotation"]) : undefined,
    messageEvent: data["messageEvent"] !== undefined ? deserializeMessageEvent(data["messageEvent"]) : undefined,
    time: data["time"] !== undefined ? new Date(data["time"]) : undefined,
  };
}

/**
 * A collection of `TimeEvent`s. A `TimeEvent` is a time-stamped annotation on
 * the span, consisting of either user-supplied key:value pairs, or details of a
 * message sent/received between Spans.
 */
export interface TimeEvents {
  /**
   * The number of dropped annotations in all the included time events. If the
   * value is 0, then no annotations were dropped.
   */
  droppedAnnotationsCount?: number;
  /**
   * The number of dropped message events in all the included time events. If
   * the value is 0, then no message events were dropped.
   */
  droppedMessageEventsCount?: number;
  /**
   * A collection of `TimeEvent`s.
   */
  timeEvent?: TimeEvent[];
}

function serializeTimeEvents(data: any): TimeEvents {
  return {
    ...data,
    timeEvent: data["timeEvent"] !== undefined ? data["timeEvent"].map((item: any) => (serializeTimeEvent(item))) : undefined,
  };
}

function deserializeTimeEvents(data: any): TimeEvents {
  return {
    ...data,
    timeEvent: data["timeEvent"] !== undefined ? data["timeEvent"].map((item: any) => (deserializeTimeEvent(item))) : undefined,
  };
}

/**
 * Represents a string that might be shortened to a specified length.
 */
export interface TruncatableString {
  /**
   * The number of bytes removed from the original string. If this value is 0,
   * then the string was not shortened.
   */
  truncatedByteCount?: number;
  /**
   * The shortened string. For example, if the original string is 500 bytes
   * long and the limit of the string is 128 bytes, then `value` contains the
   * first 128 bytes of the 500-byte string. Truncation always happens on a UTF8
   * character boundary. If there are multi-byte characters in the string, then
   * the length of the shortened string might be less than the size limit.
   */
  value?: string;
}