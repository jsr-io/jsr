import { assertEquals, assertLessOrEqual } from "jsr:@std/assert@1.0.15";
import { numberFormat } from "./number_format.ts";

Deno.test("numberFormat should format numbers correctly", () => {
  assertEquals(numberFormat(1), "1");
  assertEquals(numberFormat(5), "5");
  assertEquals(numberFormat(9), "9");
  assertEquals(numberFormat(10), "10");
  assertEquals(numberFormat(55), "55");
  assertEquals(numberFormat(99), "99");
  assertEquals(numberFormat(100), "100");
  assertEquals(numberFormat(555), "555");
  assertEquals(numberFormat(999), "999");
  assertEquals(numberFormat(1_000), "1.0k");
  assertEquals(numberFormat(5_555), "5.5k");
  assertEquals(numberFormat(9_999), "9.9k");
  assertEquals(numberFormat(10_000), "10k");
  assertEquals(numberFormat(55_555), "55k");
  assertEquals(numberFormat(99_999), "99k");
  assertEquals(numberFormat(100_000), "100k");
  assertEquals(numberFormat(555_555), "555k");
  assertEquals(numberFormat(999_999), "999k");
  assertEquals(numberFormat(1_000_000), "1.0m");
  assertEquals(numberFormat(5_555_555), "5.5m");
  assertEquals(numberFormat(9_999_999), "9.9m");
  assertEquals(numberFormat(10_000_000), "10m");
  assertEquals(numberFormat(55_555_555), "55m");
  assertEquals(numberFormat(99_999_999), "99m");
  assertEquals(numberFormat(100_000_000), "100m");
  assertEquals(numberFormat(555_555_555), "555m");
  assertEquals(numberFormat(999_999_999), "999m");
  assertEquals(numberFormat(1_000_000_000), "1000m");
});

Deno.test("numberFormat floor non-integers to the nearest integer", () => {
  assertEquals(numberFormat(1), "1");
  assertEquals(numberFormat(5.5), "5");
  assertEquals(numberFormat(9.9), "9");
  assertEquals(numberFormat(10.5), "10");
  assertEquals(numberFormat(55.5), "55");
});

Deno.test("numberFormat should output at most 4 characters under 1B", () => {
  // Generate 1000 random numbers between 1 and 1B
  const numbers = Array.from(
    { length: 1000 },
    () => Math.floor(Math.random() * 1_000_000_000),
  );
  numbers.forEach((num) => {
    assertLessOrEqual(numberFormat(num).length, 4);
  });
});
