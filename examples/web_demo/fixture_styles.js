const THEMES = {
  light: {
    background: "#ffffff",
    text: "#191919",
    grid: "#D6DCDE",
  },
  dark: {
    background: "#131722",
    text: "#D1D4DC",
    grid: "#2B2B43",
  },
};

/** Return the explicit cross-library palette for a parity fixture. */
export function fixture_theme(name) {
  return THEMES[name] ?? THEMES.light;
}
