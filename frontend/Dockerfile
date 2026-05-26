FROM denoland/deno:alpine-2.8.0

WORKDIR /app

COPY . .

# Deno's lockfile doesn't capture npm's `libc`, so on alpine it links the
# glibc binding for @tailwindcss/oxide. Pull the musl bindings as direct
# deps and drop the .node file next to oxide/index.js, which matches the
# first require() the loader tries (./tailwindcss-oxide.linux-x64-musl.node).
# Done in one RUN so nothing re-syncs node_modules before the build.
RUN deno install && deno install npm:@tailwindcss/oxide-linux-x64-musl@4.3.0 && deno install npm:@tailwindcss/oxide-linux-x64-musl@4.2.2 && cp /app/node_modules/.deno/@tailwindcss+oxide-linux-x64-musl@4.3.0/node_modules/@tailwindcss/oxide-linux-x64-musl/tailwindcss-oxide.linux-x64-musl.node /app/node_modules/.deno/@tailwindcss+oxide@4.3.0/node_modules/@tailwindcss/oxide/ && cp /app/node_modules/.deno/@tailwindcss+oxide-linux-x64-musl@4.2.2/node_modules/@tailwindcss/oxide-linux-x64-musl/tailwindcss-oxide.linux-x64-musl.node /app/node_modules/.deno/@tailwindcss+oxide@4.2.2/node_modules/@tailwindcss/oxide/ && deno task build
RUN rm -rf $DENO_DIR
RUN deno install
RUN deno install --allow-import -e _fresh/server.js

ENV PORT=8000

CMD deno serve -A --cached-only --port=$PORT _fresh/server.js
