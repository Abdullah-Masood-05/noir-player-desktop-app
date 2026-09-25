/**
 * GET /api/tracks          → Last.fm's global top tracks
 * GET /api/tracks?q=<text> → Last.fm track search
 *
 * Answers in Last.fm's own shape — `tracks.track[]` for the chart,
 * `results.trackmatches.track[]` for a search — trimmed to the fields the
 * app reads, so the app parses both sources with the same code.
 */

import { fail, fetchJson, json, serviceKey } from "../lib/upstream.js";
import { searchQuery } from "../lib/validate.js";

/** Charts move slowly; a search for the same words keeps its answer for hours. */
const CHART_CACHE_SECONDS = 60 * 30;
const SEARCH_CACHE_SECONDS = 60 * 60 * 6;

export async function GET(request: Request): Promise<Response> {
  const key = serviceKey("LASTFM_API_KEY");
  if (key === null) {
    return fail(503, "Discover isn't available right now. Try again later.");
  }

  const raw = new URL(request.url).searchParams.get("q");
  const searching = raw !== null && raw.trim() !== "";
  const query = searching ? searchQuery(raw) : null;
  if (searching && query === null) {
    return fail(400, "That search is too long or contains characters it can't use.");
  }

  const params = new URLSearchParams({
    method: query === null ? "chart.gettoptracks" : "track.search",
    api_key: key,
    format: "json",
    limit: "30",
  });
  if (query !== null) {
    params.set("track", query);
  }

  const upstream = await fetchJson(
    `https://ws.audioscrobbler.com/2.0/?${params}`,
    {},
    15_000,
  );
  if (!upstream.ok || upstream.status !== 200) {
    return fail(502, "Couldn't reach Last.fm. Try again in a moment.");
  }
  // Last.fm reports its own errors — a bad key, a rate limit — with a 200.
  if ("error" in upstream.body) {
    return fail(502, "Last.fm turned the request down. Try again later.");
  }

  const list =
    query === null
      ? (upstream.body.tracks as Record<string, unknown> | undefined)?.track
      : (
          (upstream.body.results as Record<string, unknown> | undefined)?.trackmatches as
            | Record<string, unknown>
            | undefined
        )?.track;
  if (!Array.isArray(list)) {
    return fail(502, "Last.fm sent back something unexpected. Try again later.");
  }

  const track = list.map((entry: Record<string, unknown>) => ({
    name: entry?.name,
    // A string in search results, an object with a `name` in the chart.
    artist: entry?.artist,
    url: entry?.url,
  }));
  return query === null
    ? json({ tracks: { track } }, CHART_CACHE_SECONDS)
    : json({ results: { trackmatches: { track } } }, SEARCH_CACHE_SECONDS);
}
