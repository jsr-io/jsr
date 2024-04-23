export class Test {
  /**
   * Executes the given function or string whose first argument is a DOM element and returns the result of the execution.
   *
   * @example
   * ```ts
   * /// <reference lib="dom" />
   * const value: string = await element.evaluate((element: HTMLInputElement) => element.value)
   * ```
   *
   * @example
   * ```
   * /// <reference lib="dom" />
   * await element.evaluate(
   *  (el: HTMLInputElement, key: string, value: string) => el.setAttribute(key, value),
   *  { args: ["href", "astral"] }
   * )
   * ```
   */
  async test(): Promise<void> {}
}
