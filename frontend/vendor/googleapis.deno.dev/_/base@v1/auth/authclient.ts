/**
 * Defines the root interface for all clients that generate credentials
 * for calling Google APIs. All clients should implement this interface.
 */
export interface CredentialsClient {
  /**
   * The project ID corresponding to the current credentials if available.
   */
  projectId?: string | null;

  /**
   * The main authentication interface. It takes an optional url which when
   * present is the endpoint being accessed, and returns a Promise which
   * resolves with authorization header fields.
   *
   * The result has the form:
   * { Authorization: 'Bearer <access_token_value>' }
   * @param url The URI being authorized.
   */
  getRequestHeaders(url?: string): Promise<Record<string, string>>;
}

// https://github.com/googleapis/google-auth-library-nodejs/blob/main/src/auth/jwtclient.ts
// https://cloud.google.com/spanner/docs/getting-started/nodejs#look_through_sample_files
// https://github.com/googleapis/node-gtoken/blob/main/src/index.ts
