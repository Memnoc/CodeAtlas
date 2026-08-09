import { secret } from "./hidden";

export function trySneak(): string {
  return secret();
}
