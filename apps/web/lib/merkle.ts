/** 每日结算凭证的浏览器侧复算（与 back-end flux-shared::merkle 同算法）。
 *
 *  叶子 = SHA-256(0x00 || 原文)，内部节点 = SHA-256(0x01 || 左 || 右)。
 *  前端只对服务端下发的原文做哈希，不重新拼串——拼串规则一旦两边
 *  各写一份就会漂移，历史证明会整批失效。
 */

export interface ProofStep {
  position: "left" | "right";
  hash: string;
}

/** 必须是 ArrayBuffer 支撑的视图，WebCrypto 只收 BufferSource。 */
type Bytes = Uint8Array<ArrayBuffer>;

function toHex(bytes: Uint8Array): string {
  let out = "";
  for (const b of bytes) out += b.toString(16).padStart(2, "0");
  return out;
}

function fromHex(hex: string): Bytes | null {
  if (hex.length !== 64 || /[^0-9a-f]/.test(hex)) return null;
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

async function sha256(data: Bytes): Promise<Bytes> {
  if (!globalThis.crypto?.subtle) {
    throw new Error("WebCrypto unavailable");
  }
  const buf = await crypto.subtle.digest("SHA-256", data);
  return new Uint8Array(buf);
}

export async function leafHash(preimage: string): Promise<string> {
  const body = new TextEncoder().encode(preimage);
  const buf = new Uint8Array(body.length + 1);
  buf[0] = 0;
  buf.set(body, 1);
  return toHex(await sha256(buf));
}

async function nodeHash(left: Bytes, right: Bytes): Promise<Bytes> {
  const buf = new Uint8Array(65);
  buf[0] = 1;
  buf.set(left, 1);
  buf.set(right, 33);
  return sha256(buf);
}

/** 原文 → 叶哈希 → 沿兄弟路径折叠 → 与公开树根比对。 */
export async function verifyLeaf(
  preimage: string,
  leaf: string,
  proof: ProofStep[],
  root: string,
): Promise<boolean> {
  if ((await leafHash(preimage)) !== leaf) return false;
  let cur = fromHex(leaf);
  if (!cur) return false;
  for (const step of proof) {
    const sib = fromHex(step.hash);
    if (!sib) return false;
    if (step.position === "left") {
      cur = await nodeHash(sib, cur);
    } else if (step.position === "right") {
      cur = await nodeHash(cur, sib);
    } else {
      return false;
    }
  }
  return toHex(cur) === root;
}
