FROM denoland/deno:alpine-2.8.0

WORKDIR /app

COPY . .

# Deno's lockfile doesn't capture npm's `libc` field, so on alpine the musl
# native bindings for rolldown (via vite) and tailwindcss oxide aren't picked
# up automatically. Install deps first, then force the musl variants.
RUN deno install
RUN deno install npm:@rolldown/binding-linux-x64-musl@1.0.2
RUN deno install npm:@tailwindcss/oxide-linux-x64-musl@4.3.0
RUN deno install npm:@tailwindcss/oxide-linux-x64-musl@4.2.2
RUN deno task build
RUN rm -rf $DENO_DIR
RUN deno install
RUN deno install --allow-import -e _fresh/server.js

ENV PORT=8000

CMD deno serve -A --cached-only --port=$PORT _fresh/server.js
