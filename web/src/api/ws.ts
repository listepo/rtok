// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

// Typed client for the `/ws` contract (T310.2). Transport-agnostic on purpose: the socket
// constructor and timers are injectable so Vitest drives it without a server or a browser.
import type { ClientMessage, Fixed, Plan, ServerFrame, Snapshot } from "./snapshot.gen";

export type ConnectionState = "connecting" | "open" | "closed";

// The server's panic fallback is `{"type":"snapshot","error":...}` with none of the page
// fields, so a snapshot frame is split in two rather than cast blindly to `Snapshot`.
export type Frame =
  | { type: "snapshot"; snapshot: Snapshot }
  | { type: "snapshot_error"; error: string }
  | ServerFrame;

export interface Handlers {
  onState(state: ConnectionState): void;
  onFrame(frame: Frame): void;
}

export interface Connection {
  /** `false` when the frame was not sent because the link is not open. */
  send(message: ClientMessage): boolean;
  close(): void;
}

export type Connect = (handlers: Handlers) => Connection;

export interface Socket {
  onopen: (() => void) | null;
  onmessage: ((event: { data: unknown }) => void) | null;
  onclose: (() => void) | null;
  send(data: string): void;
  close(): void;
}

export interface WsOptions {
  socket?: new (url: string) => Socket;
  location?: Pick<Location, "protocol" | "host">;
  baseDelayMs?: number;
  maxDelayMs?: number;
}

export function wsUrl(location: Pick<Location, "protocol" | "host">): string {
  const scheme = location.protocol === "https:" ? "wss:" : "ws:";
  return `${scheme}//${location.host}/ws`;
}

export function parseFrame(raw: unknown): Frame | null {
  if (typeof raw !== "string") return null;
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return null;
  }
  if (typeof value !== "object" || value === null) return null;
  const v = value as Record<string, unknown>;
  switch (v.type) {
    case "message":
      return typeof v.text === "string" ? { type: "message", text: v.text } : null;
    case "expand":
      return typeof v.id === "string" && typeof v.text === "string"
        ? { type: "expand", id: v.id, text: v.text }
        : null;
    case "doctorplan": {
      const plan = v.plan as Record<string, unknown> | null;
      return plan && Array.isArray(plan.items) && typeof plan.diff === "string"
        ? { type: "doctorplan", plan: plan as unknown as Plan }
        : null;
    }
    case "doctorfixed": {
      const fixed = v.fixed as Record<string, unknown> | null;
      return fixed && typeof fixed.text === "string" && typeof fixed.code === "number"
        ? { type: "doctorfixed", fixed: fixed as unknown as Fixed }
        : null;
    }
    case "snapshot":
      if (Array.isArray(v.plugins)) return { type: "snapshot", snapshot: v as unknown as Snapshot };
      return typeof v.error === "string" ? { type: "snapshot_error", error: v.error } : null;
    default:
      return null;
  }
}

export function connectWs(
  handlers: Handlers,
  {
    socket: SocketCtor = globalThis.WebSocket as unknown as new (url: string) => Socket,
    location: loc = globalThis.location,
    baseDelayMs = 500,
    maxDelayMs = 10_000,
  }: WsOptions = {},
): Connection {
  let socket: Socket | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let attempt = 0;
  let stopped = false;

  const scheduleRetry = () => {
    handlers.onState("closed");
    if (stopped) return;
    const delay = Math.min(baseDelayMs * 2 ** attempt, maxDelayMs);
    attempt += 1;
    timer = setTimeout(open, delay);
  };

  function open() {
    handlers.onState("connecting");
    let current: Socket;
    try {
      current = new SocketCtor(wsUrl(loc));
    } catch {
      // A constructor that throws (blocked scheme, bad host) is a failed attempt, not a crash.
      scheduleRetry();
      return;
    }
    socket = current;
    current.onopen = () => {
      attempt = 0;
      handlers.onState("open");
    };
    current.onmessage = (event) => {
      const frame = parseFrame(event.data);
      if (frame) handlers.onFrame(frame);
    };
    current.onclose = () => {
      if (socket === current) {
        socket = null;
        scheduleRetry();
      }
    };
  }

  open();

  return {
    send(message) {
      if (!socket) return false;
      try {
        socket.send(JSON.stringify(message));
        return true;
      } catch {
        // CONNECTING sockets throw on send; the caller reports the failed send.
        return false;
      }
    },
    close() {
      stopped = true;
      clearTimeout(timer);
      const current = socket;
      socket = null;
      current?.close();
      handlers.onState("closed");
    },
  };
}
