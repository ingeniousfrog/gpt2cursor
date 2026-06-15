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
        signal: {
          amber: "#f5b84b",
          red: "#ff6b75",
          blue: "#6fb9ff",
        },
      },
      boxShadow: {
        panel:
          "0 28px 70px rgba(36, 76, 128, 0.20), 0 0 0 1px rgba(255, 255, 255, 0.72), inset 0 1px 0 rgba(255, 255, 255, 0.86)",
        control:
          "inset 0 1px 0 rgba(255,255,255,0.70), 0 12px 28px rgba(72, 114, 160, 0.12)",
        logo: "0 18px 36px rgba(71, 139, 225, 0.20)",
        glow: "0 14px 32px rgba(55, 148, 255, 0.26)",
      },
      backgroundImage: {
        panel:
          "radial-gradient(circle at 50% -8%, rgba(148, 210, 255, 0.45), transparent 34%), radial-gradient(circle at 92% 88%, rgba(178, 157, 255, 0.12), transparent 26%), linear-gradient(168deg, #f9fcff 0%, #f1f7ff 52%, #fafcff 100%)",
        mesh:
          "linear-gradient(118deg, rgba(56, 132, 214, 0.07), transparent 40%), repeating-linear-gradient(90deg, rgba(21, 56, 96, 0.028) 0, rgba(21, 56, 96, 0.028) 1px, transparent 1px, transparent 16px)",
        run: "linear-gradient(135deg, #5ddcff 0%, #408cff 58%, #7667ff 100%)",
        stop: "linear-gradient(135deg, #fb7185 0%, #ef4444 100%)",
      },
    },
  },
  plugins: [],
};
