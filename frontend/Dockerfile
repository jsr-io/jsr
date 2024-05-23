FROM denoland/deno:alpine-2.0.5

WORKDIR /app

COPY . .

RUN deno task build
RUN deno install --allow-import -e main.ts

RUN timeout 10s deno run -A main.ts || [ $? -eq 124 ] || exit 1

CMD ["run", "-A", "--cached-only", "main.ts"]
