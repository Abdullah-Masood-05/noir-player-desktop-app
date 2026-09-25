/**
 * Everything the endpoints share: reading the service keys, calling a
 * service, and shaping responses.
 *
 * The keys live only here, in the deployment's environment.
 */

export type ServiceKey = "LASTFM_API_KEY" | "YOUTUBE_API_KEY" | "RAPIDAPI_KEY";

const SERVICE_KEYS: readonly ServiceKey[] = ["LASTFM_API_KEY", "YOUTUBE_API_KEY", "RAPIDAPI_KEY"];

/** The key for a service, or `null` when this deployment has none set. */
export function serviceKey(name: ServiceKey): string | null {
  const value = process.env[name]?.trim();
  return value ? value : null;
}

/** Responses from these services are small; anything bigger is not what we asked for. */
const MAX_BODY_BYTES = 1_048_576;

export type Upstream =
  | { ok: true; status: number; body: Record<string, unknown> }
  | { ok: false };

/**
 * GETs a JSON document from a service. Every failure — unreachable, timed
 * out, oversized, not an object — collapses to `{ ok: false }`; the caller
 * answers with a generic message rather than relaying upstream detail.
 */
export async function fetchJson(
  url: string,
  headers: Record<string, string>,
  timeoutMs: number,
): Promise<Upstream> {
  try {
    const response = await fetch(url, {
      headers,
      redirect: "error",
      signal: AbortSignal.timeout(timeoutMs),
    });
    const text = await response.text();
    if (text.length > MAX_BODY_BYTES) {
      return { ok: false };
    }
    const body: unknown = JSON.parse(text);
    if (body === null || typeof body !== "object" || Array.isArray(body)) {
      return { ok: false };
    }
    return { ok: true, status: response.status, body: body as Record<string, unknown> };
  } catch {
    return { ok: false };
  }
}

/**
 * True when `value` contains any of this deployment's keys. A service that
 * echoed a key back into a URL would otherwise hand it to every client.
 */
export function leaksKey(value: string): boolean {
  let decoded = value;
  try {
    decoded = decodeURIComponent(value);
  } catch {
    // A malformed escape cannot hide a key that the raw form doesn't show.
  }
  return SERVICE_KEYS.some((name) => {
    const key = serviceKey(name);
    return key !== null && (value.includes(key) || decoded.includes(key));
  });
}

/**
 * A JSON response. `cacheSeconds` lets Vercel's CDN answer repeat requests
 * without calling the service again; failures are never cached.
 */
export function json(body: unknown, cacheSeconds = 0): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control":
        cacheSeconds > 0
          ? `public, s-maxage=${cacheSeconds}, stale-while-revalidate=${cacheSeconds}`
          : "no-store",
    },
  });
}

/** An error the app shows as-is, so it is written for the person using it. */
export function fail(status: number, message: string): Response {
  return new Response(JSON.stringify({ error: message }), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
    },
  });
}
