FROM denoland/deno:2.8.0

WORKDIR /app

COPY . .

RUN deno task build
RUN deno install

# Eager-analyze and cache the server graph so the runtime container can
# run with `--cached-only`. workers-og's `.wasm` static imports are
# fine for Deno at runtime (it imports them as WebAssembly modules),
# but Deno's static-graph analyzer treats their internal wasm imports
# as missing JS deps and aborts — `|| true` lets the cache populate
# everything it could resolve and skips the unreachable wasm graph.
RUN deno install --allow-import -e _fresh/server.js || true

ENV PORT=8000

CMD deno serve -A --port=$PORT _fresh/server.js
