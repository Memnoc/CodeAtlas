// `logger` is a parameter holding an object, and `logger.ts` sits right beside
// this file. A resolver that treats any dotted receiver as a module path wires
// this call into that module — an edge the source does not contain, from a
// file that imports nothing at all.
export function handle(logger: { info(message: string): string }): string {
  return logger.info("hi");
}
