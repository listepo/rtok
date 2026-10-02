import type { Preview } from "@storybook/react-vite";
import "../src/styles/app.css";

// Same switch the app uses (`data-theme` + `dark`/`light` class on <html>), so stories
// render with the real tokens in both themes.
const preview: Preview = {
    globalTypes: {
        theme: {
            description: "Theme",
            toolbar: { icon: "circlehollow", items: ["dark", "light"], dynamicTitle: true },
        },
    },
    initialGlobals: { theme: "dark" },
    decorators: [
        (Story, { globals }) => {
            const dark = globals["theme"] !== "light";
            const root = document.documentElement;
            root.classList.toggle("dark", dark);
            root.classList.toggle("light", !dark);
            root.setAttribute("data-theme", dark ? "dark" : "light");
            return (
                <div className="p-4">
                    <Story />
                </div>
            );
        },
    ],
    parameters: {
        layout: "fullscreen",
        // Violations fail the Vitest run instead of only showing in the panel.
        a11y: { test: "error" },
    },
};

export default preview;
