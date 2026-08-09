import { external } from "./alias";

export function callAliased(): string {
  return external();
}
