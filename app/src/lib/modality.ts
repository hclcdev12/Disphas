/**
 * Whether the last interaction came from the mouse or the keyboard, kept on
 * `<html data-input="mouse|keyboard">`. The focus outline only shows after a
 * navigation key (index.css): never after a click, even when the app moves
 * the focus itself (a menu that closes, a dialog that opens).
 */
const NAVIGATION = new Set([
  "Tab",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown",
  "Enter",
  " ",
  "Escape",
]);

export function trackInputModality() {
  const root = document.documentElement;
  root.dataset.input = "mouse";
  document.addEventListener(
    "pointerdown",
    () => {
      root.dataset.input = "mouse";
    },
    true,
  );
  document.addEventListener(
    "keydown",
    (event) => {
      if (NAVIGATION.has(event.key)) root.dataset.input = "keyboard";
    },
    true,
  );
}
