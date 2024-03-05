// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { assertStringIncludes } from "./utils/assert.ts";
import { assertEquals } from "./utils/assert.ts";
import { JSR_URL } from "./utils/configuration.ts";

async function assertHtml(resp: Response) {
  const text = await resp.text();
  assertStringIncludes(text, "<!DOCTYPE html>");
  assertEquals(resp.status, 200);
  assertEquals(resp.headers.get("content-type"), "text/html; charset=utf-8");
}

Deno.test("[GET /, accept/html, no Sec-Fetch-Dest] should return html 200", async () => {
  // This is the IE11 case, where no `Sec-Fetch-Dest` header is sent.
  const response = await fetch(JSR_URL, {
    headers: { "accept": "text/html" },
  });
  await assertHtml(response);
});

Deno.test("[GET /, accept/html, Sec-Fetch-Dest: document] should return html 200", async () => {
  // This is how modern browsers send the request.
  const response = await fetch(JSR_URL, {
    headers: { "accept": "text/html", "sec-fetch-dest": "document" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, accept/html, no Sec-Fetch-Dest] should return html 200", async () => {
  // IE11 case for the meta.json file.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "accept": "text/html" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, accept/html, Sec-Fetch-Dest: document] should return html 200", async () => {
  // Modern browser case for the meta.json file.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "accept": "text/html", "sec-fetch-dest": "document" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, text/newhtml, Sec-Fetch-Dest: document] should return html 200", async () => {
  // Only Sec-Fetch-Dest is enough to trigger the HTML response.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "accept": "text/newhtml", "sec-fetch-dest": "document" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: script] should return html 200", async () => {
  // Only Sec-Fetch-Dest is enough to trigger the HTML response.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "script" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: style] should return html 200", async () => {
  // Only Sec-Fetch-Dest is enough to trigger the HTML response.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "script" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, no Sec-Fetch-Dest] should return plain text 200", async () => {
  // No Sec-Fetch-Dest, and no Accept will result in a plain text response.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: {},
  });
  const text = await response.text();
  assertStringIncludes(text, `".": "./main.ts"`);
  assertEquals(response.status, 200);
  assertEquals(
    response.headers.get("content-type"),
    "application/json",
  );
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: empty] should return plain text 200", async () => {
  // Sec-Fetch-Dest: document is not enough to trigger the HTML response.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "empty" },
  });
  const text = await response.text();
  assertStringIncludes(text, `".": "./main.ts"`);
  assertEquals(response.status, 200);
  assertEquals(
    response.headers.get("content-type"),
    "application/json",
  );
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: image] should return html 200", async () => {
  // Sec-Fetch-Dest: image is enough to trigger the HTML response.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "image" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: image, Sec-Fetch-Site: same-origin] should return plain text 200", async () => {
  // Sec-Fetch-Dest: image will trigger the JSON response if it's a same-origin request.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "image", "sec-fetch-site": "same-origin" },
  });
  const text = await response.text();
  assertStringIncludes(text, `".": "./main.ts"`);
  assertEquals(response.status, 200);
  assertEquals(
    response.headers.get("content-type"),
    "application/json",
  );
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: image, Sec-Fetch-Site: cross-origin] should return html 200", async () => {
  // Sec-Fetch-Dest: image will trigger the HTML response if it's a cross-origin request.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "image", "sec-fetch-site": "cross-origin" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: video] should return html 200", async () => {
  // Sec-Fetch-Dest: video is enough to trigger the HTML response.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "video" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: video, Sec-Fetch-Site: same-origin] should return plain text 200", async () => {
  // Sec-Fetch-Dest: video will trigger the JSON response if it's a same-origin request.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "video", "sec-fetch-site": "same-origin" },
  });
  const text = await response.text();
  assertStringIncludes(text, `".": "./main.ts"`);
  assertEquals(response.status, 200);
  assertEquals(
    response.headers.get("content-type"),
    "application/json",
  );
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json, no Accept, Sec-Fetch-Dest: video, Sec-Fetch-Site: cross-origin] should return html 200", async () => {
  // Sec-Fetch-Dest: video will trigger the HTML response if it's a cross-origin request.
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`, {
    headers: { "sec-fetch-dest": "video", "sec-fetch-site": "cross-origin" },
  });
  await assertHtml(response);
});

Deno.test("[GET /@luca/flag/meta.json] is valid", async () => {
  const response = await fetch(`${JSR_URL}/@luca/flag/meta.json`);
  const json = await response.json();
  assertEquals(json, {
    "scope": "luca",
    "name": "flag",
    "latest": "1.0.1",
    "versions": {
      "1.0.1": {},
      "1.0.0": {},
    },
  });
  assertEquals(response.status, 200);
  assertEquals(
    response.headers.get("content-type"),
    "application/json",
  );
});

Deno.test("[GET /@luca/flag/1.0.0_meta.json] is valid", async () => {
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0_meta.json`);
  const json = await response.json();
  assertEquals(json, {
    "manifest": {
      "/deno.json": {
        "size": 75,
        "checksum":
          "sha256-98719bf861369684be254b01f1427084dc6d16b506809719122890784542496b",
      },
      "/LICENSE": {
        "size": 1070,
        "checksum":
          "sha256-c3f0644e8374585b209ea5206ab88055c1c503c202bff5d1f01bb29c07041fbb",
      },
      "/README.md": {
        "size": 279,
        "checksum":
          "sha256-f544a1489e93e93957d6bd03f069e0db7a9bef4af6eeae46a86b4e3316e598c3",
      },
      "/main.ts": {
        "size": 2989,
        "checksum":
          "sha256-a41796ceb0be1bca3aa446ddebebcd732492ccb2cdcb8912adbabef3375fafc8",
      },
    },
    "moduleGraph1": {
      "/main.ts": {},
    },
    "exports": {
      ".": "./main.ts",
    },
  });
  assertEquals(response.status, 200);
  assertEquals(
    response.headers.get("content-type"),
    "application/json",
  );
});

Deno.test("[GET /@luca/flag/1.0.0/main.ts] is valid", async () => {
  const response = await fetch(`${JSR_URL}/@luca/flag/1.0.0/main.ts`);
  const text = await response.text();
  assertEquals(
    text,
    `/**
colors of the progress flag

Red: #E40303.
Orange: #FF8C00.
Yellow: #FFED00.
Green: #008026.
Indigo: #24408E.
Violet: #732982.
Pink: #FFAFC8.
Blue: #74D7EE.
Brown: 613915
Black: 000000

l = blue
p = pink
w = white
b = brown
B = black
r = red
o = orange
y = yellow
g = green
i = indigo
v = violet
 */

const colors = {
  l: "#74D7EE",
  p: "#FFAFC8",
  w: "#FFFFFF",
  b: "#613915",
  B: "#000000",
  r: "#E40303",
  o: "#FF8C00",
  y: "#FFED00",
  g: "#008026",
  i: "#24408E",
  v: "#732982",
};

const progressFlag = \`
lbbbbbBBBBBrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr
lllbbbbbBBBBBrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr
lllllbbbbbBBBBBrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr
pplllllbbbbbBBBBBrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr
pppplllllbbbbbBBBBBooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo
wppppplllllbbbbbBBBBBooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo
wwwppppplllllbbbbbBBBBBooooooooooooooooooooooooooooooooooooooooooooooooooooooooo
wwwwwppppplllllbbbbbBBBBBooooooooooooooooooooooooooooooooooooooooooooooooooooooo
wwwwwwwppppplllllbbbbbBBBBByyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
wwwwwwwwwppppplllllbbbbbBBBBByyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
wwwwwwwwwwwppppplllllbbbbbBBBBByyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
wwwwwwwwwwwwwppppplllllbbbbbBBBBByyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
wwwwwwwwwwwwwppppplllllbbbbbBBBBBggggggggggggggggggggggggggggggggggggggggggggggg
wwwwwwwwwwwppppplllllbbbbbBBBBBggggggggggggggggggggggggggggggggggggggggggggggggg
wwwwwwwwwppppplllllbbbbbBBBBBggggggggggggggggggggggggggggggggggggggggggggggggggg
wwwwwwwppppplllllbbbbbBBBBBggggggggggggggggggggggggggggggggggggggggggggggggggggg
wwwwwppppplllllbbbbbBBBBBiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
wwwppppplllllbbbbbBBBBBiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
wppppplllllbbbbbBBBBBiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
pppplllllbbbbbBBBBBiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiiii
pplllllbbbbbBBBBBvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv
lllllbbbbbBBBBBvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv
lllbbbbbBBBBBvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv
lbbbbbBBBBBvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv
\`;

/**
 * Print the progress flag to the terminal with a size of 24x80 characters.
 */
export function printProgress() {
  const flag = progressFlag.trim();
  const lines = flag.split("\\n");
  for (const line of lines) {
    let print = "";
    const csses = [];
    for (const char of line) {
      const color = colors[char as keyof typeof colors];
      print += \`%c %c\`;
      csses.push(\`background-color: \${color}\`, "");
    }
    console.log(print, ...csses);
  }
}

if (import.meta.main) {
  printProgress();
}
`,
  );
  assertEquals(response.status, 200);
  assertEquals(
    response.headers.get("content-type"),
    "text/typescript",
  );
});
