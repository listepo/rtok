import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { sampleSnapshot } from "./sample";
import { connectWs, parseFrame, wsUrl, type ConnectionState, type Frame, type Socket } from "./ws";

class FakeSocket implements Socket {
  static all: FakeSocket[] = [];
  onopen: (() => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onclose: (() => void) | null = null;
  sent: string[] = [];
  closed = false;

  constructor(readonly url: string) {
    FakeSocket.all.push(this);
  }
  send(data: string) {
    this.sent.push(data);
  }
  close() {
    this.closed = true;
    this.onclose?.();
  }
}

const location = { protocol: "http:", host: "127.0.0.1:3333" };

function setup() {
  const states: ConnectionState[] = [];
  const frames: Frame[] = [];
  const connection = connectWs(
    { onState: (s) => states.push(s), onFrame: (f) => frames.push(f) },
    { socket: FakeSocket, location },
  );
  return { states, frames, connection };
}

const last = () => FakeSocket.all[FakeSocket.all.length - 1]!;

beforeEach(() => {
  FakeSocket.all = [];
  vi.useFakeTimers();
});
afterEach(() => vi.useRealTimers());

describe("wsUrl", () => {
  test("follows the page scheme", () => {
    expect(wsUrl({ protocol: "http:", host: "h:1" })).toBe("ws://h:1/ws");
    expect(wsUrl({ protocol: "https:", host: "h" })).toBe("wss://h/ws");
  });
});

describe("parseFrame", () => {
  test("snapshot, snapshot failure, message and expand", () => {
    const snap = parseFrame(JSON.stringify(sampleSnapshot));
    expect(snap).toEqual({ type: "snapshot", snapshot: sampleSnapshot });
    expect(parseFrame('{"type":"snapshot","error":"boom"}')).toEqual({
      type: "snapshot_error",
      error: "boom",
    });
    expect(parseFrame('{"type":"message","text":"refused key x"}')).toEqual({
      type: "message",
      text: "refused key x",
    });
    expect(parseFrame('{"type":"expand","id":"a1","text":"body"}')).toEqual({
      type: "expand",
      id: "a1",
      text: "body",
    });
  });

  test.each([
    ["malformed JSON", "{nope"],
    ["binary payload", new ArrayBuffer(2)],
    ["non-object", "42"],
    ["null", "null"],
    ["unknown type", '{"type":"other"}'],
    ["message without text", '{"type":"message"}'],
    ["expand with a numeric id", '{"type":"expand","id":1,"text":"x"}'],
    ["snapshot with neither pages nor error", '{"type":"snapshot"}'],
  ])("ignores %s", (_name, raw) => {
    expect(parseFrame(raw)).toBeNull();
  });
});

describe("connectWs", () => {
  test("connects to /ws, reports state and delivers frames; bad frames are skipped", () => {
    const { states, frames } = setup();
    expect(last().url).toBe("ws://127.0.0.1:3333/ws");
    last().onopen?.();
    last().onmessage?.({ data: "{nope" });
    last().onmessage?.({ data: '{"type":"message","text":"hi"}' });
    expect(states).toEqual(["connecting", "open"]);
    expect(frames).toEqual([{ type: "message", text: "hi" }]);
  });

  test("send serialises while open and reports false otherwise", () => {
    const { connection } = setup();
    expect(connection.send({ expand: "a1" })).toBe(true);
    expect(last().sent).toEqual(['{"expand":"a1"}']);
    last().onclose?.();
    expect(connection.send({ expand: "a1" })).toBe(false);
  });

  test("reconnects with doubling delays capped at the maximum, reset after an open", () => {
    const { states } = setup();
    const expectedDelays = [500, 1000, 2000, 4000, 8000, 10_000, 10_000];
    for (const [i, delay] of expectedDelays.entries()) {
      const before = FakeSocket.all.length;
      last().onclose?.();
      vi.advanceTimersByTime(delay - 1);
      expect(FakeSocket.all.length, `attempt ${i} fired early`).toBe(before);
      vi.advanceTimersByTime(1);
      expect(FakeSocket.all.length, `attempt ${i} did not fire`).toBe(before + 1);
    }

    last().onopen?.();
    last().onclose?.();
    vi.advanceTimersByTime(499);
    const before = FakeSocket.all.length;
    vi.advanceTimersByTime(1);
    expect(FakeSocket.all.length).toBe(before + 1);
    expect(states.slice(0, 3)).toEqual(["connecting", "closed", "connecting"]);
  });

  test("a throwing socket constructor is retried, not fatal", () => {
    let calls = 0;
    class Flaky extends FakeSocket {
      constructor(url: string) {
        if (calls++ === 0) throw new Error("blocked");
        super(url);
      }
    }
    connectWs({ onState: () => {}, onFrame: () => {} }, { socket: Flaky, location });
    expect(FakeSocket.all).toHaveLength(0);
    vi.advanceTimersByTime(500);
    expect(FakeSocket.all).toHaveLength(1);
  });

  test("close stops reconnecting and closes the socket", () => {
    const { states, connection } = setup();
    const socket = last();
    connection.close();
    expect(socket.closed).toBe(true);
    vi.advanceTimersByTime(60_000);
    expect(FakeSocket.all).toHaveLength(1);
    expect(states.at(-1)).toBe("closed");
    expect(connection.send({ expand: "a1" })).toBe(false);
  });

  test("close during the backoff wait cancels the pending attempt", () => {
    const { connection } = setup();
    last().onclose?.();
    connection.close();
    vi.advanceTimersByTime(60_000);
    expect(FakeSocket.all).toHaveLength(1);
  });
});
