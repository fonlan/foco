import {
  Activity,
  CheckCircle2,
  LoaderCircle,
  RefreshCw,
  ServerCrash,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";

type HealthResponse = {
  service: string;
  status: string;
};

type HealthState =
  | { checkedAt?: string; kind: "loading"; message: string }
  | { checkedAt: string; kind: "online"; message: string }
  | { checkedAt: string; kind: "offline"; message: string };

export function App() {
  const [health, setHealth] = useState<HealthState>({
    kind: "loading",
    message: "Checking local server",
  });

  const refreshHealth = useCallback(async () => {
    setHealth({ kind: "loading", message: "Checking local server" });

    try {
      const response = await fetch("/api/health", { cache: "no-store" });

      if (!response.ok) {
        throw new Error(`/api/health returned ${response.status}`);
      }

      const data = (await response.json()) as HealthResponse;

      if (data.status !== "ok") {
        throw new Error(`/api/health returned status "${data.status}"`);
      }

      setHealth({
        checkedAt: new Date().toLocaleTimeString(),
        kind: "online",
        message: `${data.service} server is online`,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : "Unknown error";

      setHealth({
        checkedAt: new Date().toLocaleTimeString(),
        kind: "offline",
        message,
      });
    }
  }, []);

  useEffect(() => {
    void refreshHealth();
  }, [refreshHealth]);

  const StatusIcon =
    health.kind === "online"
      ? CheckCircle2
      : health.kind === "offline"
        ? ServerCrash
        : LoaderCircle;

  return (
    <main className="min-h-screen bg-[#f7f8fb] text-zinc-950">
      <section className="mx-auto flex min-h-screen w-full max-w-3xl flex-col justify-center gap-6 px-6 py-10">
        <header className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <div className="flex items-center gap-2 text-emerald-700">
              <Activity aria-hidden="true" className="size-6" />
              <span className="text-sm font-semibold uppercase tracking-wide">
                Foco
              </span>
            </div>
            <h1 className="mt-3 text-3xl font-semibold text-zinc-950">
              Local app status
            </h1>
          </div>

          <button
            className="inline-flex h-10 items-center gap-2 border border-zinc-300 bg-white px-4 text-sm font-medium text-zinc-900 shadow-sm transition hover:border-zinc-400 hover:bg-zinc-50"
            onClick={() => void refreshHealth()}
            type="button"
          >
            <RefreshCw aria-hidden="true" className="size-4" />
            Refresh
          </button>
        </header>

        <div className="border border-zinc-200 bg-white p-5 shadow-sm">
          <div className="flex items-start gap-4">
            <div
              className={
                health.kind === "online"
                  ? "text-emerald-600"
                  : health.kind === "offline"
                    ? "text-rose-600"
                    : "text-cyan-700"
              }
            >
              <StatusIcon
                aria-hidden="true"
                className={
                  health.kind === "loading" ? "size-7 animate-spin" : "size-7"
                }
              />
            </div>

            <div className="min-w-0 flex-1" aria-live="polite">
              <p className="text-sm font-medium uppercase tracking-wide text-zinc-500">
                Server health
              </p>
              <p className="mt-2 break-words text-2xl font-semibold text-zinc-950">
                {health.message}
              </p>
              {health.checkedAt ? (
                <p className="mt-3 text-sm text-zinc-500">
                  Last checked at {health.checkedAt}
                </p>
              ) : null}
            </div>
          </div>
        </div>
      </section>
    </main>
  );
}
