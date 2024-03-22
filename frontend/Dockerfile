FROM denoland/deno:alpine-1.41.3
WORKDIR /app

COPY . .

RUN deno task build
RUN deno cache main.ts

CMD ["run", "-A", "--cached-only", "main.ts"]
