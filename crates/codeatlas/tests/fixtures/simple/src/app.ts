import { shout } from "./arrow";
import { helper } from "./lib";
import { ghost } from "./missing";
import * as fs from "node:fs";

export function app(): void {
  console.log(shout(helper()));
  ghost(fs);
}
