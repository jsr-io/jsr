FROM denoland/deno:alpine-2.8.0

WORKDIR /app

COPY . .

# Deno's lockfile doesn't capture npm's `libc`, so on alpine it links the
# glibc binding for @tailwindcss/oxide. Install the musl bindings as direct
# deps and symlink them into each oxide version's nested node_modules.
RUN deno install
RUN deno install npm:@tailwindcss/oxide-linux-x64-musl@4.3.0
RUN deno install npm:@tailwindcss/oxide-linux-x64-musl@4.2.2
RUN ln -sf ../../../@tailwindcss+oxide-linux-x64-musl@4.3.0/node_modules/@tailwindcss/oxide-linux-x64-musl /app/node_modules/.deno/@tailwindcss+oxide@4.3.0/node_modules/@tailwindcss/oxide-linux-x64-musl
RUN ln -sf ../../../@tailwindcss+oxide-linux-x64-musl@4.2.2/node_modules/@tailwindcss/oxide-linux-x64-musl /app/node_modules/.deno/@tailwindcss+oxide@4.2.2/node_modules/@tailwindcss/oxide-linux-x64-musl
RUN deno task build
RUN rm -rf $DENO_DIR
RUN deno install
RUN deno install --allow-import -e _fresh/server.js

ENV PORT=8000

CMD deno serve -A --cached-only --port=$PORT _fresh/server.js
