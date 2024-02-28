#!/usr/bin/env -S deno run -A --watch=static/,routes/
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import dev from "$fresh/dev.ts";
import config from "./fresh.config.ts";

Deno.env.set("OTLP_ENDPOINT", "http://localhost:4318");

await dev(import.meta.url, "./main.ts", config);
