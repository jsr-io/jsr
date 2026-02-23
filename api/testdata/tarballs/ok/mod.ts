/**
 * This is a test module.
 *
 * @module
 */

/**
 * This is a test constant.
 */
export const hello = "Hello, world!";
export const 读取多键1 = 1;

/**
 * A high-performance HTTP server implementation.
 *
 * Supports HTTP/1.1 and handles keep-alive connections. The server
 * runs on a single thread and uses an event loop for concurrency.
 *
 * ## Configuration
 *
 * The server accepts a `ServerConfig` object with the following:
 * - `port` - The port number to listen on
 * - `host` - The hostname to bind to
 * - `maxConnections` - Maximum concurrent connections
 *
 * ## Lifecycle
 *
 * 1. Create the server with `new Server(config)`
 * 2. Register handlers with `server.on(path, handler)`
 * 3. Start listening with `server.listen()`
 * 4. Shut down with `server.close()`
 *
 * ## Security
 *
 * All connections are plain HTTP. For HTTPS, use `SecureServer` instead.
 *
 * @param hello
 * @param qaz
 */
export function foo(hello: string, qaz: number): string {
  return "";
}