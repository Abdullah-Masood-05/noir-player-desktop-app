/**
 * GET /api/health → which services this deployment has keys for.
 *
 * Reports only whether each key is set, never its value, and calls no
 * service, so checking a deployment costs no quota.
 */

import { json, serviceKey } from "../lib/upstream.js";

export function GET(): Response {
  return json({
    ok: true,
    lastfm: serviceKey("LASTFM_API_KEY") !== null,
    youtube: serviceKey("YOUTUBE_API_KEY") !== null,
    rapidapi: serviceKey("RAPIDAPI_KEY") !== null,
  });
}
