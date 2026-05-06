export type ThemePref = "dark" | "light" | "system";

const STORAGE_KEY = "stackpilot.theme";

class ThemeStore {
  pref = $state<ThemePref>("system");
  resolved = $state<"dark" | "light">("dark");
  private mq: MediaQueryList | null = null;

  init() {
    if (typeof window === "undefined") return;
    const stored = localStorage.getItem(STORAGE_KEY) as ThemePref | null;
    this.pref = stored ?? "system";

    this.mq = window.matchMedia("(prefers-color-scheme: dark)");
    this.mq.addEventListener("change", () => this.apply());
    this.apply();
  }

  set(pref: ThemePref) {
    this.pref = pref;
    localStorage.setItem(STORAGE_KEY, pref);
    this.apply();
  }

  private apply() {
    const sysDark = this.mq?.matches ?? true;
    this.resolved =
      this.pref === "system" ? (sysDark ? "dark" : "light") : this.pref;
    document.documentElement.dataset.theme = this.resolved;
  }
}

export const theme = new ThemeStore();
