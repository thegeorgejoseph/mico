declare module "node:test" {
  type TestFn = () => void | Promise<void>;
  export default function test(name: string, fn: TestFn): void;
}

declare module "node:assert/strict" {
  interface Assert {
    equal(actual: unknown, expected: unknown, message?: string): void;
    ok(value: unknown, message?: string): void;
  }

  const assert: Assert;
  export default assert;
}
