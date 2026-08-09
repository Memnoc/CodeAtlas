export const shout = (s: string): string => s.toUpperCase();

const local = function (s: string): string {
  return s.toLowerCase();
};

export function shoutTwice(s: string): string {
  return shout(s) + shout(s) + local(s);
}
