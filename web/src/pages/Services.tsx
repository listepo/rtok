import { Empty } from "../states";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { fmt } from "./format";
import { Kv, OtherLines, TextPage, WithSnapshot } from "./parts";
import {
    humanSecs,
    parseServices,
    type OtelView,
    type ServiceRow,
    type ServicesView,
} from "./text";

export function Services() {
    return (
        <WithSnapshot>
            {(snap) => (
                <TextPage
                    page="services"
                    text={snap.services}
                    command="rtok demon status"
                    note="supervisor state in ~/.rtok/demon"
                >
                    {(text) => <ServicesBody view={parseServices(text)} />}
                </TextPage>
            )}
        </WithSnapshot>
    );
}

// The page only reads: starting and stopping a service is a CLI verdict today, so the card
// names the command instead of showing a button that does nothing.
export const serviceCommand = (s: ServiceRow) =>
    `rtok demon ${s.running ? "restart" : "start"} ${s.name}`;

function ServicesBody({ view }: { view: ServicesView }) {
    const running = view.services.filter((s) => s.running).length;
    return (
        <>
            {view.services.length === 0 ? (
                <Panel title="services">
                    <Empty title="No services reported" />
                </Panel>
            ) : (
                <>
                    <p className="text-2xs text-fg-subtle">
                        {running} of {view.services.length} running
                    </p>
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
                        {view.services.map((s) => (
                            <Service key={s.name} service={s} />
                        ))}
                    </div>
                </>
            )}
            <Panel title="OpenTelemetry export" hint="rtok otel status">
                {view.otel ? (
                    <Otel otel={view.otel} lastFlush={view.lastFlush} />
                ) : (
                    <Empty title="otel status unavailable" />
                )}
            </Panel>
            <OtherLines lines={view.other} />
        </>
    );
}

function Service({ service: s }: { service: ServiceRow }) {
    return (
        <Panel title={s.name} hint="service">
            <div>
                {s.running ? (
                    <Pill tone="ok" dot>
                        running
                    </Pill>
                ) : (
                    <Pill>stopped</Pill>
                )}
            </div>
            <Kv
                rows={[
                    ["pid", s.pid ?? "-"],
                    ["uptime", humanSecs(s.uptimeSecs)],
                    ["log", s.log ?? "-"],
                ]}
            />
            <p className="text-2xs text-fg-subtle">
                CLI-only: <code>{serviceCommand(s)}</code>
            </p>
        </Panel>
    );
}

function Otel({ otel, lastFlush }: { otel: OtelView; lastFlush: string | null }) {
    const tiles: [string, number | null, number | null | "watermark"][] = [
        ["calls", otel.callsMark, otel.callsPending],
        ["logs", otel.logsMark, otel.logsPending],
        ["sessions", otel.sessionsMark, "watermark"],
    ];
    return (
        <>
            <Kv rows={[["endpoint", otel.endpoint ?? "-"]]} />
            <div className="grid grid-cols-3 gap-2">
                {tiles.map(([label, mark, pending]) => (
                    <div key={label} className="rounded-md bg-surface-2 p-2.5">
                        <div className="text-2xs font-semibold tracking-kicker text-fg-muted uppercase">
                            {label}
                        </div>
                        <div className="text-sm font-semibold">{fmt(mark)}</div>
                        <div
                            className={`text-2xs ${typeof pending === "number" && pending > 0 ? "text-warn-fg" : "text-fg-muted"}`}
                        >
                            {pending === "watermark" ? "watermark" : `${fmt(pending)} pending`}
                        </div>
                    </div>
                ))}
            </div>
            {lastFlush && <p className="text-2xs text-fg-muted">{lastFlush}</p>}
        </>
    );
}
