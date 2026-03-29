const tabs = Array.from(document.querySelectorAll(".tab"));
const panels = Array.from(document.querySelectorAll(".command-panel"));
const toast = document.getElementById("copy-toast");
const menu = document.getElementById("mobile-menu");
const menuToggle = document.querySelector("[data-menu-toggle]");
const menuCloseTargets = Array.from(document.querySelectorAll("[data-menu-close], [data-menu-link]"));

function setMenuOpen(open) {
  if (!menu || !menuToggle) {
    return;
  }

  menu.hidden = !open;
  menuToggle.setAttribute("aria-expanded", String(open));
}

function showPlatform(target) {
  tabs.forEach((tab) => {
    tab.classList.toggle("is-active", tab.dataset.target === target);
  });

  panels.forEach((panel) => {
    panel.classList.toggle("is-active", panel.dataset.platform === target);
  });
}

tabs.forEach((tab) => {
  tab.addEventListener("click", () => showPlatform(tab.dataset.target));
});

if (menu && menuToggle) {
  menuToggle.addEventListener("click", () => {
    const isOpen = menu.hidden === false;
    setMenuOpen(!isOpen);
  });

  menuCloseTargets.forEach((node) => {
    node.addEventListener("click", () => setMenuOpen(false));
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      setMenuOpen(false);
    }
  });
}

function showToast(message) {
  if (!toast) {
    return;
  }

  toast.textContent = message;
  toast.classList.add("is-visible");

  window.clearTimeout(showToast.timeoutId);
  showToast.timeoutId = window.setTimeout(() => {
    toast.classList.remove("is-visible");
  }, 1800);
}

async function copyFromElement(id) {
  const element = document.getElementById(id);
  if (!element) {
    return;
  }

  try {
    await navigator.clipboard.writeText(element.textContent.trim());
    showToast("Install command copied.");
  } catch (error) {
    showToast("Clipboard copy failed.");
  }
}

document.querySelectorAll("[data-copy-target]").forEach((button) => {
  button.addEventListener("click", () => copyFromElement(button.dataset.copyTarget));
});
