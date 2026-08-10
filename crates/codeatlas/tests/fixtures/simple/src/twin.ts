// The decoy: same base name as `twin.js`, and nothing may resolve to it
// through a `./twin.js` specifier.
export function twinTs(): string {
  return "ts";
}
