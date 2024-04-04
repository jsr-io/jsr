// Copyright 2024 the JSR authors. All rights reserved. MIT license.
function setItem<T>(
  key: string,
  value: T,
  ttl = 60, // 1 hour
) {
  const data = { value, expiry: new Date().getTime() + (ttl * 60_000) };
  localStorage.setItem(key, JSON.stringify(data));
}

export async function getOrInsertItem<T>(
  key: string,
  fn: () => Promise<T>,
  ttl?: number,
): Promise<T> {
  const item = localStorage.getItem(key);
  if (item) {
    const { value, expiry } = JSON.parse(item);
    if (expiry > new Date().getTime()) {
      return value;
    }
  }

  const insert = await fn();
  setItem(key, insert, ttl);

  return insert;
}
