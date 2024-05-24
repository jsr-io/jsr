import * as jose from "https://deno.land/x/jose@v4.4.0/index.ts";
import { CredentialsClient } from "./authclient.ts";

export interface JWTInput {
  type?: string;
  client_email?: string;
  private_key?: string;
  private_key_id?: string;
  project_id?: string;
  client_id?: string;
  client_secret?: string;
  refresh_token?: string;
  quota_project_id?: string;
}

export class JWT implements CredentialsClient {
  #projectId: string | undefined;
  #clientEmail: string;
  #privateKeyId: string | undefined;
  #privateKeyString: string;
  #privateKey: Promise<jose.KeyLike> | undefined;

  constructor(
    projectId: string | undefined,
    clientEmail: string,
    privateKeyId: string | undefined,
    privateKeyString: string,
  ) {
    this.#projectId = projectId;
    this.#clientEmail = clientEmail;
    this.#privateKeyId = privateKeyId;
    this.#privateKeyString = privateKeyString;
  }

  static fromJSON(json: JWTInput) {
    if (!json) {
      throw new Error(
        "Must pass in a JSON object containing the service account auth settings.",
      );
    }
    if (!json.client_email) {
      throw new Error(
        "The incoming JSON object does not contain a client_email field",
      );
    }
    if (!json.private_key) {
      throw new Error(
        "The incoming JSON object does not contain a private_key field",
      );
    }

    return new JWT(
      json.project_id,
      json.client_email,
      json.private_key_id,
      json.private_key,
    );
  }

  get projectId() {
    return this.#projectId;
  }

  async getRequestHeaders(url: string): Promise<Record<string, string>> {
    const aud = new URL(url).origin + "/";
    const jwt = await this.#getJWT(aud);
    return {
      "Authorization": `Bearer ${jwt}`,
    };
  }

  #getPrivateKey(): Promise<jose.KeyLike> {
    if (!this.#privateKey) {
      this.#privateKey = jose.importPKCS8(this.#privateKeyString, "RS256");
    }
    return this.#privateKey;
  }

  async #getJWT(aud: string) {
    const key = await this.#getPrivateKey();
    return new jose.SignJWT({ aud })
      .setProtectedHeader({ alg: "RS256", kid: this.#privateKeyId })
      .setIssuer(this.#clientEmail)
      .setSubject(this.#clientEmail)
      .setIssuedAt()
      .setExpirationTime("1h")
      .sign(key);
  }
}
