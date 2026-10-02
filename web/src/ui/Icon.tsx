// Raw SVGs so `currentColor` follows the text colour; they are our own bundled assets.
const icons = import.meta.glob<string>("../../assets/icons/*.svg", {
    query: "?raw",
    import: "default",
    eager: true,
});

const byName = new Map(
    Object.entries(icons).map(([path, svg]) => [path.split("/").pop()?.replace(".svg", ""), svg]),
);

export const hasIcon = (name: string) => byName.has(name);

export function Icon({ name }: { name: string }) {
    const svg = byName.get(name);
    return (
        <span
            aria-hidden="true"
            className="inline-flex size-4 shrink-0 [&>svg]:size-4"
            dangerouslySetInnerHTML={svg ? { __html: svg.trim() } : undefined}
        />
    );
}
