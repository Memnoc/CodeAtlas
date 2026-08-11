// A plain module import: no braces, no namespace, no local name at all. The
// statement binds nothing, so the file edge is the only thing it can produce
// — and it is the only statement in this file that reaches `./side`.
import "./side";

export function plain(): string {
  return "plain";
}
