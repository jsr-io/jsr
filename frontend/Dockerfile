FROM denoland/deno:alpine-2.6.8

WORKDIR /app

COPY . .

RUN deno task build
RUN rm -rf $DENO_DIR
RUN deno install
RUN deno install --allow-import -e _fresh/server.js

ENV PORT=8000

CMD deno serve -A --cached-only --port=$PORT _fresh/server.js
