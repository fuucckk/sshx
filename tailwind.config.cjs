const defaultTheme = require("tailwindcss/defaultTheme");

/** @type {import("tailwindcss/tailwind-config").TailwindConfig} */
const config = {
  content: ["./src/**/*.{html,js,svelte,ts}"],

  darkMode: "class",
  theme: {
    extend: {
      fontFamily: {
        sans: ["InterVariable", ...defaultTheme.fontFamily.sans],
        mono: ["Fira Code", ...defaultTheme.fontFamily.mono],
      },
    },
  },

  plugins: [],
};

module.exports = config;
