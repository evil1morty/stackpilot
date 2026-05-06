import { Channel, ipc } from "$lib/ipc";
import type { ScoopEvent } from "$lib/types";

export type OpKind = "install" | "uninstall" | "bootstrap";
export type OpState = "running" | "finished" | "errored" | "cancelled";

export type LogLine = {
  kind: "stdout" | "stderr" | "system";
  text: string;
  t: number;
};

export type CurrentOp = {
  kind: OpKind;
  target: string;
  command: string;
  state: OpState;
  exitCode: number | null;
  lines: LogLine[];
  startedAt: number;
  endedAt: number | null;
};

class OperationStore {
  current = $state<CurrentOp | null>(null);
  history = $state<CurrentOp[]>([]);

  get busy(): boolean {
    return this.current?.state === "running";
  }

  async runInstall(app: string): Promise<void> {
    return this.run("install", app);
  }

  async runUninstall(app: string): Promise<void> {
    return this.run("uninstall", app);
  }

  async runBootstrap(): Promise<void> {
    return this.run("bootstrap", "Scoop");
  }

  async cancel(): Promise<void> {
    if (!this.busy) return;
    try {
      await ipc.scoopCancel();
      if (this.current) {
        this.current.state = "cancelled";
        this.current.endedAt = Date.now();
        this.current.lines = [
          ...this.current.lines,
          { kind: "system", text: "── cancelled ──", t: Date.now() },
        ];
      }
    } catch (e) {
      this.appendSystem(`cancel failed: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  clear() {
    if (this.busy) return;
    this.current = null;
  }

  private async run(kind: OpKind, target: string): Promise<void> {
    if (this.busy) {
      throw new Error("Another scoop operation is already running");
    }

    if (this.current) this.history = [this.current, ...this.history].slice(0, 8);

    this.current = {
      kind,
      target,
      command: "",
      state: "running",
      exitCode: null,
      lines: [],
      startedAt: Date.now(),
      endedAt: null,
    };

    const channel = new Channel<ScoopEvent>();
    channel.onmessage = (msg) => this.handleEvent(msg);

    try {
      switch (kind) {
        case "install":
          await ipc.scoopInstall(target, channel);
          break;
        case "uninstall":
          await ipc.scoopUninstall(target, channel);
          break;
        case "bootstrap":
          await ipc.scoopBootstrap(channel);
          break;
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      if (this.current && this.current.state === "running") {
        this.current.state = "errored";
        this.current.endedAt = Date.now();
      }
      this.appendSystem(`error: ${msg}`);
    }

    if (this.current && this.current.state === "running") {
      this.current.state = this.current.exitCode === 0 ? "finished" : "errored";
      this.current.endedAt = Date.now();
    }
  }

  private handleEvent(msg: ScoopEvent) {
    if (!this.current) return;
    switch (msg.type) {
      case "started":
        this.current.command = msg.payload.command;
        break;
      case "stdout":
        this.current.lines = [
          ...this.current.lines,
          { kind: "stdout", text: msg.payload.line, t: Date.now() },
        ];
        break;
      case "stderr":
        this.current.lines = [
          ...this.current.lines,
          { kind: "stderr", text: msg.payload.line, t: Date.now() },
        ];
        break;
      case "finished":
        this.current.exitCode = msg.payload.exitCode;
        break;
      case "error":
        this.appendSystem(msg.payload.message);
        this.current.state = "errored";
        break;
    }

    // Cap retained log lines to avoid unbounded memory.
    const MAX = 4000;
    if (this.current.lines.length > MAX) {
      this.current.lines = this.current.lines.slice(-MAX);
    }
  }

  private appendSystem(text: string) {
    if (!this.current) return;
    this.current.lines = [
      ...this.current.lines,
      { kind: "system", text, t: Date.now() },
    ];
  }
}

export const operation = new OperationStore();
