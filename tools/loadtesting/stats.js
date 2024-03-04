export const options = {
  stages: [
    { duration: "10s", target: 5 },
    { duration: "30s", target: 20 },
    { duration: "60s", target: 50 },
    { duration: "10s", target: 0 },
  ],

  ext: {
    loadimpact: {
      projectID: 3636975,
      name: "api.jsr.io/stats",
    },
  },
};

import http from "k6/http";
import { check } from "k6";
import { Counter } from "k6/metrics";

const cached = new Counter("cached");

export default function () {
  const res = http.get("https://api.jsr.io/stats");
  cached.add(1, { cache: res.headers["X-Jsr-Cache-Status"] });
  check(res, {
    "is status 200": (r) => r.status === 200,
  });
}
