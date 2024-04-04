// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export default function NotFoundPage() {
  return (
    <div class="w-full overflow-x-hidden relative flex justify-between flex-col flex-wrap">
      <div class="flex-top">
        <header class="text-center px-8 py-[10vh]">
          <h1 class="font-extrabold text-5xl leading-10 tracking-tight text-gray-900">
            404
          </h1>
          <h2 class="mt-4 sm:mt-5 font-light text-2xl text-center leading-tight text-gray-900">
            Couldn't find what you're looking for.
          </h2>
        </header>
      </div>
    </div>
  );
}
