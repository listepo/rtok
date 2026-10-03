// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// "Fix selected" (T331.12): the checklist `rtok doctor --fix` shows in a terminal. The server
// builds the list, the defaults and the diff; this panel only sends what the user changed and
// writes nothing until the explicit confirm.
import { useEffect, useReducer } from "react";
import { useDoctorApi } from "../api/query";
import type { Item } from "../api/snapshot.gen";
import { focusRing } from "../ui/cx";
import { Panel } from "../ui/Panel";
import { Pill } from "../ui/Pill";
import { fixReducer, initialFix, selectedCount, type FixState } from "./fixState";

const button = `${focusRing} rounded-md border border-border px-2.5 py-1 text-xs hover:bg-surface-2 disabled:opacity-50`;

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export function DoctorFix() {
    const api = useDoctorApi();
    const [state, dispatch] = useReducer(fixReducer, initialFix);
    const { seq, selection } = state;

    useEffect(() => {
        let current = true;
        api.doctorPlan(selection).then(
            (plan) => current && dispatch({ type: "planned", seq, plan }),
            (e) => current && dispatch({ type: "failed", error: message(e) }),
        );
        return () => {
            current = false;
        };
        // A selection only changes together with `seq`, which is what a new plan answers.
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [api, seq]);

    const confirm = () => {
        dispatch({ type: "applying" });
        api.doctorApply(selection).then(
            (fixed) => dispatch({ type: "applied", fixed }),
            (e) => dispatch({ type: "failed", error: message(e) }),
        );
    };

    return (
        <Panel
            title="fix"
            hint="removes broken and duplicate entries; every file is backed up first"
        >
            <FixBody state={state} dispatch={dispatch} confirm={confirm} />
        </Panel>
    );
}

function FixBody({
    state,
    dispatch,
    confirm,
}: {
    state: FixState;
    dispatch: (a: Parameters<typeof fixReducer>[1]) => void;
    confirm: () => void;
}) {
    const { plan, phase } = state;
    if (phase === "done" && state.result) {
        return (
            <div className="flex flex-col gap-3">
                <pre
                    role="status"
                    className="overflow-x-auto text-2xs whitespace-pre-wrap text-fg-muted"
                >
                    {state.result.text}
                </pre>
                <div>
                    <button
                        type="button"
                        className={button}
                        onClick={() => dispatch({ type: "again" })}
                    >
                        Check again
                    </button>
                </div>
            </div>
        );
    }
    if (!plan) {
        return phase === "error" ? (
            <p role="alert" className="text-xs text-delta-fg">
                {state.error}
            </p>
        ) : (
            <p className="text-xs text-fg-muted">Looking for entries to fix…</p>
        );
    }
    const n = selectedCount(plan);
    const busy = phase === "applying";
    return (
        <div className="flex flex-col gap-3">
            {plan.items.length === 0 && <p className="text-xs text-fg-muted">Nothing to fix.</p>}
            <ul className="divide-y divide-border/60 -m-4 mb-0">
                {plan.items.map((i) => (
                    <Row key={i.source + i.path} item={i} busy={busy} dispatch={dispatch} />
                ))}
            </ul>
            {plan.refused.map((r) => (
                <p key={r} className="text-2xs text-warn-fg">
                    {r}
                </p>
            ))}
            {plan.diff && (
                <pre
                    aria-label="diff"
                    className="max-h-80 overflow-auto rounded-md bg-surface-2 p-3 text-2xs"
                >
                    {plan.diff.split("\n").map((line, k) => (
                        <div key={k} className={diffTone(line)}>
                            {line || " "}
                        </div>
                    ))}
                </pre>
            )}
            {state.error && (
                <p role="alert" className="text-xs text-delta-fg">
                    {state.error}
                </p>
            )}
            <div className="flex items-center gap-2">
                {phase === "confirming" || busy ? (
                    <>
                        <span className="text-xs">
                            Write {n} {n === 1 ? "entry" : "entries"}?
                        </span>
                        <button type="button" className={button} disabled={busy} onClick={confirm}>
                            Confirm
                        </button>
                        <button
                            type="button"
                            className={button}
                            disabled={busy}
                            onClick={() => dispatch({ type: "cancel" })}
                        >
                            Cancel
                        </button>
                    </>
                ) : (
                    <button
                        type="button"
                        className={button}
                        disabled={n === 0 || phase !== "ready"}
                        onClick={() => dispatch({ type: "ask" })}
                    >
                        Fix selected ({n})
                    </button>
                )}
            </div>
        </div>
    );
}

const diffTone = (line: string) =>
    line.startsWith("+") && !line.startsWith("+++")
        ? "text-success-fg"
        : line.startsWith("-") && !line.startsWith("---")
          ? "text-delta-fg"
          : "text-fg-muted";

function Row({
    item,
    busy,
    dispatch,
}: {
    item: Item;
    busy: boolean;
    dispatch: (a: Parameters<typeof fixReducer>[1]) => void;
}) {
    const ref = { source: item.source, path: item.path };
    return (
        <li className="flex items-start gap-3 px-4 py-2.5">
            <input
                type="checkbox"
                className="mt-1"
                aria-label={`${item.label} in ${item.source}`}
                checked={item.selected}
                disabled={busy}
                onChange={() => dispatch({ type: "toggle", ref })}
            />
            <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-center gap-2 text-xs font-semibold">
                    <span className="break-all">{item.label}</span>
                    <Pill tone="warn">{item.kind}</Pill>
                    {item.shared && <Pill tone="info">shared</Pill>}
                </div>
                <div className="text-2xs break-all text-fg-muted">
                    {item.agent} · {item.source}
                </div>
                <div className="text-2xs break-words text-fg-muted">{item.detail}</div>
                {item.kept_in && (
                    <div className="text-2xs break-all text-fg-muted">kept in {item.kept_in}</div>
                )}
            </div>
            {item.can_keep && (
                <button
                    type="button"
                    className={button}
                    disabled={busy}
                    onClick={() => dispatch({ type: "keep", ref })}
                >
                    Keep this copy
                </button>
            )}
        </li>
    );
}
