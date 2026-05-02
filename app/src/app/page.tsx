import { Button } from "@/components/ui/button";

const workspaceItems = [
  "engine crates",
  "Tauri shell",
  "signaling worker",
  "installer stubs",
] as const;

export default function Home() {
  return (
    <main className="min-h-screen bg-background text-foreground">
      <section className="mx-auto flex min-h-screen w-full max-w-5xl flex-col justify-center gap-8 px-8 py-12">
        <div className="space-y-3">
          <p className="text-sm font-medium uppercase tracking-normal text-accent-foreground">
            Stream
          </p>
          <h1 className="max-w-2xl text-4xl font-semibold tracking-normal text-balance">
            Repository skeleton
          </h1>
          <p className="max-w-2xl text-base leading-7 text-muted-foreground">
            The desktop shell is ready for engine IPC, pairing, and media pipeline work.
          </p>
        </div>

        <div className="grid gap-3 sm:grid-cols-2">
          {workspaceItems.map((item) => (
            <div
              className="rounded-lg border border-border bg-card px-4 py-3 text-sm text-card-foreground"
              key={item}
            >
              {item}
            </div>
          ))}
        </div>

        <div>
          <Button variant="secondary">No feature implemented</Button>
        </div>
      </section>
    </main>
  );
}
