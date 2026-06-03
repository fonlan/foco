import { Blocks, Code2, Server, Terminal } from "lucide-react";

const panels = [
  {
    icon: Blocks,
    label: "Workspace",
    value: "Cargo + npm",
  },
  {
    icon: Server,
    label: "Backend",
    value: "Rust",
  },
  {
    icon: Code2,
    label: "Frontend",
    value: "React 19",
  },
];

export function App() {
  return (
    <main className="min-h-screen bg-zinc-950 text-zinc-100">
      <section className="mx-auto flex min-h-screen w-full max-w-5xl flex-col justify-center gap-8 px-6 py-10">
        <header className="space-y-3">
          <div className="flex items-center gap-3 text-emerald-300">
            <Terminal aria-hidden="true" className="size-7" />
            <span className="text-sm font-medium uppercase tracking-wide">
              Foco
            </span>
          </div>
          <h1 className="text-4xl font-semibold text-white sm:text-5xl">
            Local agent workspace
          </h1>
          <p className="max-w-2xl text-base leading-7 text-zinc-300">
            The repository skeleton is ready for the backend server, provider
            runtime, graph tools, storage layer, and browser UI.
          </p>
        </header>

        <div className="grid gap-3 sm:grid-cols-3">
          {panels.map((panel) => {
            const Icon = panel.icon;

            return (
              <article
                className="border border-zinc-800 bg-zinc-900/70 p-4"
                key={panel.label}
              >
                <Icon aria-hidden="true" className="mb-5 size-6 text-cyan-300" />
                <p className="text-sm text-zinc-400">{panel.label}</p>
                <p className="mt-1 text-lg font-medium text-white">
                  {panel.value}
                </p>
              </article>
            );
          })}
        </div>
      </section>
    </main>
  );
}
