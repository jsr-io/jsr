export const options = {
  stages: [
    { duration: "10s", target: 10 },
    { duration: "30s", target: 50 },
    { duration: "90s", target: 200 },
    { duration: "10s", target: 0 },
  ],

  ext: {
    loadimpact: {
      projectID: 3636975,
      name: "jsr.io",
    },
  },
};

import http from "k6/http";
import { check } from "k6";
import { Counter } from "k6/metrics";

const cached = new Counter("cached");

export default function () {
  const res = http.get("https://jsr.io", {
    headers: { "x-jsr-bypass-waitlist": "1" },
  });
  cached.add(1, { cache: res.headers["X-Jsr-Cache-Status"] });
  check(res, {
    "is status 200": (r) => r.status === 200,
  });
}
