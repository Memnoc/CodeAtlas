import { greet } from "./barrel";

export function viaBarrel(): string {
  return greet("barrel");
}
