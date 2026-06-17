/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: [
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "SF Pro Text",
          "Segoe UI",
          "sans-serif",
        ],
        mono: [
          "SF Mono",
          "ui-monospace",
          "Menlo",
          "Monaco",
          "Consolas",
          "monospace",
        ],
      },
      colors: {
        ink: {
          base: "#121c32",
          raised: "#1a2744",
        },
        accent: {
          cyan: "#5ddcff",
          blue: "#3f8cff",
          violet: "#7c6bff",
        },
      },
      boxShadow: {
        panel:
          "0 28px 70px rgba(20, 40, 80, 0.35), inset 0 1px 0 rgba(255, 255, 255, 0.1)",
        glass:
          "inset 0 1px 0 rgba(255, 255, 255, 0.1), 0 8px 24px rgba(20, 40, 80, 0.2)",
        glow: "0 12px 30px rgba(63, 140, 255, 0.42)",
        stop: "0 12px 30px rgba(251, 113, 133, 0.34)",
        pop: "0 24px 60px rgba(0, 0, 0, 0.6), inset 0 1px 0 rgba(255, 255, 255, 0.08)",
        "panel-light":
          "0 28px 70px rgba(36, 76, 128, 0.18), inset 0 1px 0 rgba(255, 255, 255, 0.72)",
        "glass-light":
          "inset 0 1px 0 rgba(255,255,255,0.70), 0 12px 28px rgba(72, 114, 160, 0.12)",
        "pop-light":
          "0 18px 40px rgba(15, 23, 42, 0.14), inset 0 1px 0 rgba(255, 255, 255, 0.86)",
      },
      backgroundImage: {
        panel:
          "radial-gradient(120% 90% at 50% -10%, rgba(100, 170, 255, 0.32), transparent 55%), radial-gradient(90% 70% at 100% 105%, rgba(150, 130, 255, 0.2), transparent 50%), radial-gradient(70% 60% at -10% 90%, rgba(120, 220, 255, 0.14), transparent 50%), linear-gradient(165deg, #1a2848 0%, #121c32 55%, #162040 100%)",
        "panel-light":
          "radial-gradient(circle at 50% -8%, rgba(148, 210, 255, 0.45), transparent 34%), radial-gradient(circle at 92% 88%, rgba(178, 157, 255, 0.12), transparent 26%), linear-gradient(168deg, #f9fcff 0%, #f1f7ff 52%, #fafcff 100%)",
        mesh:
          "linear-gradient(rgba(255, 255, 255, 0.04) 1px, transparent 1px), linear-gradient(90deg, rgba(255, 255, 255, 0.04) 1px, transparent 1px)",
        "mesh-light":
          "linear-gradient(rgba(21, 56, 96, 0.028) 1px, transparent 1px), linear-gradient(90deg, rgba(21, 56, 96, 0.028) 1px, transparent 1px)",
        run: "linear-gradient(135deg, #5ddcff 0%, #3f8cff 55%, #7c6bff 100%)",
        stop: "linear-gradient(135deg, #fb7185 0%, #ef4444 100%)",
        "accent-text": "linear-gradient(120deg, #93e6ff 0%, #6fb0ff 50%, #a99bff 100%)",
      },
    },
  },
  plugins: [],
};
