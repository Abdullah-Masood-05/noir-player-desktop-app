/**
 * GET /api/resolve?id=<video id> → a downloadable audio link for the video
 *
 * Answers in the conversion service's shape, trimmed to what the app reads:
 * `{ "link": "https://…" }` when the audio is ready, or
 * `{ "status": "processing" }` while the service is still converting, in
 * which case the app waits and asks again.
 *
 * Never cached: links are short-lived, and a "processing" answer is only
 * true for a few seconds.
 *
 * The audio itself is downloaded by the app straight from that link. It is
 * not relayed through here: a song is several megabytes, and a function's
 * response is capped well below that.
 */

import { fail, fetchJson, json, leaksKey, serviceKey } from "../lib/upstream.js";
import { videoId } from "../lib/validate.js";

const HOST = "youtube-mp36.p.rapidapi.com";

export async function GET(request: Request): Promise<Response> {
  const key = serviceKey("RAPIDAPI_KEY");
  if (key === null) {
    return fail(503, "Playing Discover songs isn't available right now. Try again later.");
  }

  const id = videoId(new URL(request.url).searchParams.get("id"));
  if (id === null) {
    return fail(400, "That isn't a video this can look up.");
  }

  const upstream = await fetchJson(
    `https://${HOST}/dl?id=${id}`,
    { "X-RapidAPI-Key": key, "X-RapidAPI-Host": HOST },
    20_000,
  );
  if (!upstream.ok) {
    return fail(502, "Couldn't reach the audio service. Try again in a moment.");
  }
  if (upstream.status !== 200) {
    // A 429 or 403 here is the monthly quota.
    return fail(502, "The audio service isn't taking requests right now. Try again later.");
  }

  const link = upstream.body.link;
  if (typeof link === "string" && link !== "") {
    if (!link.startsWith("https://") || leaksKey(link)) {
      return fail(502, "The audio service sent back a link that isn't safe to use.");
    }
    return json({ link });
  }
  if (upstream.body.status === "processing") {
    return json({ status: "processing" });
  }
  return fail(502, "The audio service couldn't convert this song. Try another one.");
}
