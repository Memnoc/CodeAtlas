// A named import bound under another local name. The file edge would exist
// whether or not the rename were understood, so the teeth of this row are the
// call: `hello` has to be looked up in `./util` under its *exported* name.
import { greet as hello } from "./util";

export function viaRenamed(): string {
  return hello("renamed");
}
