export interface Env {
  ROOMS: DurableObjectNamespace;
}

export class Room {
  constructor(
    private readonly state: DurableObjectState,
    private readonly env: Env,
  ) {}

  async fetch(): Promise<Response> {
    return new Response("stream signaling room placeholder\n", {
      headers: { "content-type": "text/plain; charset=UTF-8" },
    });
  }
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const roomName = url.pathname.split("/").filter(Boolean).at(1) ?? "default";
    const id = env.ROOMS.idFromName(roomName);

    return env.ROOMS.get(id).fetch(request);
  },
} satisfies ExportedHandler<Env>;
