FROM denoland/deno:alpine-1.39.4
WORKDIR /app

COPY . .

RUN deno task build
RUN deno cache main.ts

CMD ["run", "-A", "--cached-only", "main.ts"]
