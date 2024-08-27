import { RouteConfig } from "$fresh/server.ts";

const style = `body:not(:has(div[data-v-app])) {
  margin: 0;
  height: 100vh;
  display: flex;
  justify-content: center;
  align-items: center;
}

body:not(:has(div[data-v-app])) #loading {
  font-family: sans-serif;
  font-size: 1.5em;
  color: #666;
}

body:has(div[data-v-app]) #loading {
  display: none;
}

@media (prefers-color-scheme: dark) {
  body:not(:has(div[data-v-app])) {
    background-color: #0f0f0f;
    color: #ccc;
  }

  body:not(:has(div[data-v-app])) #loading {
    color: #ccc;
  }
}`;

export default function ApiReference() {
  return (
    <html>
      <head>
        <title>JSR API Reference</title>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <style dangerouslySetInnerHTML={{ __html: style }} />
      </head>
      <body>
        <div id="loading">The API reference is loading...</div>
        <script
          id="api-reference"
          data-url="https://api.jsr.io/.well-known/openapi"
        >
        </script>
        <script src="/scripts/api-reference.js">
        </script>
      </body>
    </html>
  );
}

export const config = {
  skipAppWrapper: true,
  skipInheritedLayouts: true,
} satisfies RouteConfig;
