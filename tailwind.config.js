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
          "radial-gradient(circle at 50% 0%, rgba(148, 224, 255, 0.62), transparent 30%), radial-gradient(circle at 18% 82%, rgba(178, 157, 255, 0.18), transparent 28%), linear-gradient(155deg, #f8fcff 0%, #eef8ff 46%, #f9fbff 100%)",
        mesh:
          "linear-gradient(115deg, rgba(64, 164, 255, 0.12), transparent 42%), repeating-linear-gradient(90deg, rgba(24, 80, 140, 0.045) 0, rgba(24, 80, 140, 0.045) 1px, transparent 1px, transparent 18px)",
        run: "linear-gradient(135deg, #5ddcff 0%, #408cff 58%, #7667ff 100%)",
        stop: "linear-gradient(135deg, #fb7185 0%, #ef4444 100%)",
      },
    },
  },
  plugins: [],
};
