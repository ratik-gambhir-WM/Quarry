import type { ProfilePhotoOptions, ProfilePhotoResult } from "./types";

const DEFAULT_GRAPH_BASE_URL = "https://graph.microsoft.com/v1.0";
const DEFAULT_PHOTO_SIZE = "48x48";

/**
 * Fetch the signed-in user's Microsoft 365 profile photo via Graph.
 *
 * Returns both the raw `Blob` and an `objectUrl` created via
 * `URL.createObjectURL`. Callers should revoke the object URL when no longer
 * needed (e.g. `URL.revokeObjectURL(result.objectUrl)`).
 *
 * Throws on non-2xx responses — handle retry / fallback in the caller.
 */
export async function fetchTeamsProfilePhoto(
  graphToken: string,
  options: ProfilePhotoOptions = {},
): Promise<ProfilePhotoResult> {
  const size = options.size ?? DEFAULT_PHOTO_SIZE;
  const baseUrl = options.graphBaseUrl ?? DEFAULT_GRAPH_BASE_URL;

  const response = await fetch(`${baseUrl}/me/photos/${size}/$value`, {
    headers: { Authorization: `Bearer ${graphToken}` },
  });

  if (!response.ok) {
    throw new Error(
      `Failed to fetch profile photo: ${response.status} ${response.statusText}`,
    );
  }

  const blob = await response.blob();
  const objectUrl = URL.createObjectURL(blob);
  return { blob, objectUrl };
}
