// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { App, staticFiles, trailingSlashes } from "fresh";
import { State } from "./util.ts";

export const app = new App<State>()
  .use(trailingSlashes("never"))
  .use(staticFiles())
  .fsRoutes();
