// The module a plain, name-free import reaches. Importing a module for its
// side effect alone is still a dependency, and the map has to say so.
export function sideEffect(): string {
  return "side";
}
