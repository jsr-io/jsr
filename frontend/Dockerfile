FROM denoland/deno:alpine-v2.x
WORKDIR /app
RUN mkdir /deno_dir
ENV DENO_DIR /deno_dir

COPY . .

RUN deno task build
RUN deno cache main.ts

CMD ["run", "-A", "--cached-only", "main.ts"]
