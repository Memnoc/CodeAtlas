// NodeNext specifiers: `./util.js` is `util.ts` on disk. Real TypeScript
// projects have no choice about this, so the baseline fixture uses it too.
import { greet } from "./util.js";
import { twin } from "./twin.js";
import { widget } from "./widget.jsx";

export function nodenext(): string {
  return greet(twin() + widget());
}
