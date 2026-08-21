FROM denoland/deno:2.8.0

WORKDIR /app

COPY . .

# Swap in the Cloud-Run-safe stub for the /og route. The real og.ts
# uses `workers-og`, whose static `.wasm` imports break Deno's
# graph analysis at server startup. The LB routes `/og` to the
# Cloudflare Worker frontend, so this code path never runs in
# production — the stub just keeps Fresh's route discovery happy.
RUN cp routes/package/og.cloud-run-stub.ts routes/package/og.ts

RUN deno task build
RUN rm -rf $DENO_DIR
RUN deno install
RUN deno install --allow-import -e _fresh/server.js

ENV PORT=8000

CMD deno serve -A --cached-only --port=$PORT _fresh/server.js
