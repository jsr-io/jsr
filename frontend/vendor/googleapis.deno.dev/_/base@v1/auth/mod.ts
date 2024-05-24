import { getEnv } from "../util.ts";
import { CredentialsClient } from "./authclient.ts";
import { JWT, JWTInput } from "./jwt.ts";

export type { CredentialsClient, JWTInput };

export interface ADCResponse {
  credential: CredentialsClient;
  projectId: string | null;
}

const EXTERNAL_ACCOUNT_TYPE = "external_account";

export type JSONClient = JWT;

export class GoogleAuth {
  /**
   * Create a credentials instance using the given input options.
   * @param json The input object.
   * @returns JWT Client with data
   */
  fromJSON(json: JWTInput): JSONClient {
    let client: JSONClient;
    if (!json) {
      throw new Error(
        "Must pass in a JSON object containing the Google auth settings.",
      );
    }
    if (json.type === "authorized_user") {
      throw new Error("TBD");
    } else if (json.type === EXTERNAL_ACCOUNT_TYPE) {
      throw new Error("TBD");
    } else {
      client = JWT.fromJSON(json);
    }
    return client;
  }

  async getApplicationDefault(): Promise<ADCResponse> {
    let client: JSONClient | null = null;
    client = await this.#tryGetApplicationCredentialsFromEnvironmentVariable();
    if (client !== null) {
      return {
        credential: client,
        projectId: client.projectId ?? null,
      };
    }

    // TODO(lucacasonato): check wellknown file

    // TODO(lucacasonato): use GCE metadata server

    throw new Error(
      "Could not load the default credentials. Browse to https://cloud.google.com/docs/authentication/getting-started for more information.",
    );
  }

  async #tryGetApplicationCredentialsFromEnvironmentVariable(): Promise<
    JSONClient | null
  > {
    const filePath = getEnv("GOOGLE_APPLICATION_CREDENTIALS") ||
      getEnv("google_application_credentials");
    if (filePath === undefined || filePath.length === 0) {
      return null;
    }
    try {
      const text = await Deno.readTextFile(filePath);
      return this.fromJSON(JSON.parse(text));
    } catch (e) {
      if (
        e instanceof Deno.errors.NotFound ||
        e instanceof Deno.errors.PermissionDenied
      ) {
        throw new Error(
          `Unable to read the credential file specified by the GOOGLE_APPLICATION_CREDENTIALS environment variable.`,
          { cause: e },
        );
      }
      throw e;
    }
  }
}

export const auth = new GoogleAuth();
