/**
 * Input checks. The endpoints only accept exactly what the app sends, so the
 * deployment's keys can't be spent on arbitrary requests through it.
 */

/** Song and artist names are far shorter; this only bounds what gets forwarded. */
export const MAX_QUERY_LENGTH = 200;

/** A trimmed search string, or `null` when it is empty, too long or malformed. */
export function searchQuery(value: string | null): string | null {
  const trimmed = value?.trim() ?? "";
  if (trimmed === "" || trimmed.length > MAX_QUERY_LENGTH) {
    return null;
  }
  // Control characters have no place in a song search.
  if (/[\u0000-\u001f\u007f]/.test(trimmed)) {
    return null;
  }
  return trimmed;
}

/** YouTube video ids are exactly eleven URL-safe base64 characters. */
export function videoId(value: string | null): string | null {
  return value !== null && /^[A-Za-z0-9_-]{11}$/.test(value) ? value : null;
}
