// The decoy for the value-receiver row: a real module, exporting a real
// function, sitting right beside the file that calls `logger.info(…)` on a
// parameter of the same name.
export function info(message: string): string {
  return message;
}
