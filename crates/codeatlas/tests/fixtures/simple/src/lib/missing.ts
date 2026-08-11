// The decoy for the resolves-nowhere row. `src/app.ts` imports `./missing`,
// which is `src/missing.ts` and does not exist; this file has the same base
// name one directory down, so a resolver searching by name rather than by path
// would wire the two together.
export function ghost(): string {
  return "ghost";
}
