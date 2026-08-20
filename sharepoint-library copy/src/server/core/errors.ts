/** Error class for all SharePoint client library failures. */
export class SharePointClientError extends Error {
  readonly statusCode: number;
  readonly details: unknown;

  constructor(message: string, statusCode = 513, details?: unknown) {
    super(message);
    this.name = "SharePointClientError";
    this.statusCode = statusCode;
    this.details = details;
    Object.setPrototypeOf(this, SharePointClientError.prototype);
  }
}
