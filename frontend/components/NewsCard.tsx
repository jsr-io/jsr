export function NewsCard({
  title,
  description,
  image,
  url,
}: {
  title: string;
  description: string;
  image: string;
  url: string;
}) {
  return (
    <li class="group border-1.5 border-jsr-cyan-950 rounded list-none overflow-hidden hover:border-jsr-cyan-400 focus-within:border-jsr-cyan-400 transition-colors duration-150">
      <a
        href={url}
        class="h-full flex flex-col justify-stretch cursor-pointer"
        tabIndex={0}
      >
        <img
          src={image}
          crossOrigin={"anonymous"}
          alt=""
          class="w-full h-48 object-cover border-b-1.5 border-jsr-cyan-950 group-hover:border-jsr-cyan-400 group-focus-within:border-jsr-cyan-400 transition-colors duration-150"
        />
        <div class="p-4 flex flex-grow flex-col gap-4">
          <h3 class="text-xl lg:text-2xl font-semibold !leading-tight text-balance">
            {title}
          </h3>
          <p class="text-sm">
            {description}
          </p>
        </div>
      </a>
    </li>
  );
}
