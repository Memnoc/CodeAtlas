// A real JavaScript file sitting beside a TypeScript file of the same
// name. `./twin.js` must resolve here and not to `twin.ts`: rewriting the
// extension may never shadow a file that actually exists.
export function twin() {
  return "js";
}
