declare module "node-diff3" {
  export interface Diff3Result {
    conflict: boolean;
    result: string[];
  }
  export function merge(
    a: string,
    o: string,
    b: string,
    options?: { stringSeparator?: string | RegExp },
  ): Diff3Result;
}
