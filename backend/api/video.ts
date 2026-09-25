/**
 * GET /api/video?q=<song artist> → the best-matching YouTube video
 *
 * Answers in YouTube's shape, trimmed to `items[0].id.videoId`.
 *
 * A YouTube search costs 100 of the 10,000 quota units a key gets per day,
 * so a key serving everyone runs out after about a hundred searches. The
 * same song almost always resolves to the same video, so answers are cached
 * for a day: each distinct song costs quota once, not once per play.
 */

import { fail, fetchJson, json, serviceKey } from "../lib/upstream.js";
import { searchQuery, videoId } from "../lib/validate.js";

const CACHE_SECONDS = 60 * 60 * 24;

export async function GET(request: Request): Promise<Response> {
  const key = serviceKey("YOUTUBE_API_KEY");
  if (key === null) {
    return fail(503, "Playing Discover songs isn't available right now. Try again later.");
  }

  const query = searchQuery(new URL(request.url).searchParams.get("q"));
  if (query === null) {
    return fail(400, "That search is empty, too long or contains characters it can't use.");
  }

  const params = new URLSearchParams({
    part: "snippet",
    q: query,
    type: "video",
    maxResults: "1",
    key,
  });
  const upstream = await fetchJson(
    `https://www.googleapis.com/youtube/v3/search?${params}`,
    {},
    20_000,
  );
  if (!upstream.ok) {
    return fail(502, "Couldn't reach YouTube. Try again in a moment.");
  }
  if (upstream.status !== 200 || "error" in upstream.body) {
    // Almost always the daily quota.
    return fail(502, "YouTube isn't taking searches right now. Try again later.");
  }

  const items = upstream.body.items;
  const first = Array.isArray(items) ? (items[0] as Record<string, unknown> | undefined) : undefined;
  const id = videoId(
    ((first?.id as Record<string, unknown> | undefined)?.videoId as string | undefined) ?? null,
  );
  if (id === null) {
    // Cached like a hit: searching again won't find what isn't there.
    return json({ items: [] }, CACHE_SECONDS);
  }
  return json({ items: [{ id: { videoId: id } }] }, CACHE_SECONDS);
}
