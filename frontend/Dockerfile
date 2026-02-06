FROM denoland/deno:alpine-2.6.8

WORKDIR /app

COPY . .

RUN deno task build
RUN rm -rf $DENO_DIR
RUN deno install --allow-import -e main.ts

RUN timeout 2s deno run -A --cached-only main.ts || true

CMD ["task", "start"]
