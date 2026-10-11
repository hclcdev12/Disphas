// Passwords (optional; docs/site.md §8). Hashed with Argon2id, built into
// Node 24 (node:crypto, Stable), with the OWASP parameters: 19 MiB, 2 passes,
// 1 lane. Stored as a PHC string, so parameters can be raised later:
//   $argon2id$v=19$m=19456,t=2,p=1$<salt, base64>$<tag, base64>
// Better Auth's default (scrypt N=16384, r=16, p=1) is below OWASP's minimum.
import { argon2, randomBytes, timingSafeEqual } from "node:crypto";
import common from "./common-passwords.json";

const MEMORY_KIB = 19_456;
const PASSES = 2;
const LANES = 1;
const TAG_BYTES = 32;

export const MIN_LENGTH = 12;
export const MAX_LENGTH = 128;

const COMMON = new Set(common as string[]);

function derive(password: string, salt: Buffer, memory: number, passes: number, lanes: number, length: number) {
  return new Promise<Buffer>((resolve, reject) => {
    argon2(
      "argon2id",
      { message: password.normalize("NFKC"), nonce: salt, memory, passes, parallelism: lanes, tagLength: length },
      (error, key) => (error ? reject(error) : resolve(key)),
    );
  });
}

export async function hashPassword(password: string): Promise<string> {
  const salt = randomBytes(16);
  const tag = await derive(password, salt, MEMORY_KIB, PASSES, LANES, TAG_BYTES);
  return `$argon2id$v=19$m=${MEMORY_KIB},t=${PASSES},p=${LANES}$${salt.toString("base64")}$${tag.toString("base64")}`;
}

const PHC = /^\$argon2id\$v=19\$m=(\d+),t=(\d+),p=(\d+)\$([A-Za-z0-9+/=]+)\$([A-Za-z0-9+/=]+)$/;

/** Constant-time comparison; false for anything that is not one of our hashes. */
export async function verifyPassword({ hash, password }: { hash: string; password: string }): Promise<boolean> {
  const match = PHC.exec(hash);
  if (!match) return false;
  const [, memory, passes, lanes, salt, tag] = match;
  const expected = Buffer.from(tag, "base64");
  // Bounds: a tampered row must not make the server work forever.
  if (Number(memory) > 1_048_576 || Number(passes) > 10 || Number(lanes) > 8 || expected.length < 16) return false;
  const actual = await derive(password, Buffer.from(salt, "base64"), Number(memory), Number(passes), Number(lanes), expected.length);
  return timingSafeEqual(actual, expected);
}

/** Why `password` is refused, or null if it is acceptable. */
export function weakness(password: string, email: string): "length" | "common" | "email" | null {
  const length = [...password].length;
  if (length < MIN_LENGTH || length > MAX_LENGTH) return "length";
  const lower = password.toLowerCase();
  if (COMMON.has(lower)) return "common";
  const local = email.split("@")[0]?.toLowerCase() ?? "";
  if (local.length >= 4 && lower.includes(local)) return "email";
  if (new Set(lower).size < 4) return "common";
  return null;
}
