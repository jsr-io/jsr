// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { IS_BROWSER } from "$fresh/runtime.ts";
import type { TraceSpan } from "./tracing.ts";

export type QueryParams = Record<string, string | number>;

export interface APIRequest<T> {
  path: APIPath;
  method: string;
  query?: QueryParams;
  body?: T;
  signal?: AbortSignal;
  anonymous?: boolean;
}

export type APIResponse<T> = APIResponseOK<T> | APIResponseError;

export interface APIResponseOK<T> {
  ok: true;
  data: T;
  traceId: string | null;
  response: Response | null;
}

export interface APIResponseError {
  ok: false;
  status: number;
  code: string;
  message: string;
  traceId: string | null;
  response: Response | null;
}

type APIPath = string & { __apiPath: never };

/**
 * Template literal to build API request paths. Example:
 *
 * ```ts
 * const req = path`/users/${userId}/packages/${packageName}`;
 * ```
 *
 * All parameters are encoded with `encodeURIComponent`. It is validated that
 * the path does not contain any ? or # characters.
 */
export function path(
  strings: TemplateStringsArray,
  ...params: string[]
): APIPath {
  let path = "";
  for (let i = 0; i < strings.length; i++) {
    path += strings[i];
    if (i < params.length) {
      path += encodeURIComponent(params[i]);
    }
  }
  if (path.includes("?") || path.includes("#")) {
    throw new Error(
      "Path cannot contain ? or # characters, include query in APIRequest.query",
    );
  }
  return path as APIPath;
}

interface APIOptions {
  token?: string | null;
  sudo?: boolean;
  span?: TraceSpan | null;
}

interface RequestOptions {
  signal?: AbortSignal;
  anonymous?: boolean;
}

export class API {
  #apiRoot: string;
  #token: string | null;
  #sudo: boolean;
  #span: TraceSpan | null;

  constructor(apiRoot: string, { token, sudo, span }: APIOptions = {}) {
    this.#apiRoot = apiRoot;
    this.#token = token ?? null;
    this.#sudo = sudo ?? false;
    this.#span = span ?? null;
  }

  hasToken(): boolean {
    return this.#token !== null;
  }

  token(): string | null {
    return this.#token;
  }

  get<RespT = unknown>(
    path: APIPath,
    query?: QueryParams,
    opts?: RequestOptions,
  ): Promise<APIResponse<RespT>> {
    return this.request({
      method: "GET",
      path,
      query,
      signal: opts?.signal,
      anonymous: opts?.anonymous,
    });
  }

  post<RespT = unknown, ReqT = unknown>(
    path: APIPath,
    body: ReqT,
    query?: QueryParams,
    opts?: RequestOptions,
  ): Promise<APIResponse<RespT>> {
    return this.request({
      method: "POST",
      path,
      query,
      body,
      signal: opts?.signal,
      anonymous: opts?.anonymous,
    });
  }

  patch<RespT = unknown, ReqT = unknown>(
    path: APIPath,
    body: ReqT,
    query?: QueryParams,
    opts?: RequestOptions,
  ): Promise<APIResponse<RespT>> {
    return this.request({
      method: "PATCH",
      path,
      query,
      body,
      signal: opts?.signal,
      anonymous: opts?.anonymous,
    });
  }

  delete<RespT = unknown>(
    path: APIPath,
    query?: QueryParams,
    opts?: RequestOptions,
  ): Promise<APIResponse<RespT>> {
    return this.request({
      method: "DELETE",
      path,
      query,
      signal: opts?.signal,
      anonymous: opts?.anonymous,
    });
  }

  async request<RespT = unknown, ReqT = unknown>(
    req: APIRequest<ReqT>,
  ): Promise<APIResponse<RespT>> {
    const start = new Date();
    const span = this.#span ? this.#span.child() : null;
    const url = new URL(this.#apiRoot + req.path);
    let result: APIResponse<RespT>;
    for (const [key, value] of Object.entries(req.query ?? {})) {
      url.searchParams.append(key, String(value));
    }
    const headers = new Headers();
    if (this.#token && !req.anonymous) {
      headers.append("Authorization", `Bearer ${this.#token}`);
    }
    if (this.#sudo && !req.anonymous) {
      headers.append("x-jsr-sudo", "true");
    }
    if (req.body) {
      headers.append("Content-Type", "application/json");
    }
    if (span) {
      headers.set("x-cloud-trace-context", span.cloudTraceContext);
      headers.set("traceparent", span.traceparent);
    }
    try {
      const resp = await fetch(url.href, {
        method: req.method,
        headers,
        body: req.body ? JSON.stringify(req.body) : undefined,
        signal: req.signal,
      });
      const traceId = resp.headers.get("x-deno-ray");
      if (resp.status === 200) {
        const data = await resp.json();
        result = { ok: true, data, traceId, response: resp };
      } else if (resp.status === 204) {
        await resp.body?.cancel();
        result = { ok: true, data: null as RespT, traceId, response: resp };
      } else {
        const body = await resp.text();
        try {
          const err = JSON.parse(body);
          result = {
            ok: false,
            status: resp.status,
            code: err.code,
            message: err.message,
            traceId,
            response: resp,
          };
        } catch {
          result = {
            ok: false,
            status: resp.status,
            code: "invalidResponse",
            message: `Failed to decode response. Body: ${body}`,
            traceId,
            response: resp,
          };
        }
      }
    } catch (err) {
      console.error(err);
      result = {
        ok: false,
        status: 500,
        code: "networkError",
        message: `Failed to make API call for ${req.path}`,
        traceId: null,
        response: null,
      };
    }
    const end = new Date();
    if (span && span.isSampled) {
      span.record(
        `${req.method} ${url.pathname}`,
        start,
        end,
        {
          "http.url": url.href,
          "http.method": req.method,
          "http.host": url.host,
          ...(result.ok
            ? {
              "http.status_code": BigInt(200),
            }
            : {
              "error": true,
              "http.status_code": BigInt(result.status),
              "error.type": result.code,
              "error.message": result.message,
            }),
        },
      );
    }
    return result;
  }
}

let api: API;
if (IS_BROWSER) api = new API(new URL("/api", location.href).href);
export { api };
