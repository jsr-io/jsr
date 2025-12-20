function findParent(el, find) {
  do {
    if (find(el)) {
      return el;
    }
  } while (el = el.parentElement);
}

document.addEventListener("click", (e) => {
  const target = findParent(
    e.target,
    (el) => el instanceof HTMLButtonElement && el.dataset["copy"],
  );
  if (target) {
    navigator?.clipboard?.writeText(target.dataset["copy"]);
    target.classList.add("copied");
    setTimeout(() => target.classList.remove("copied"), 1000);
  }
});

window.addEventListener("load", () => {
  const usageSelector = document.getElementById("usageSelector");

  document.addEventListener("mouseup", (e) => {
    if (
      findParent(
        e.target,
        (el) =>
          el.parentElement === usageSelector && el instanceof HTMLDivElement,
      )
    ) {
      usageSelector.open = false;
    }
  });
});
