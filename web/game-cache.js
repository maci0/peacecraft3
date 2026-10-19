// One local recreation-owned profile. Script filenames never become storage paths.
const key = 'peacecraft3.campaign-cache.v1';
globalThis.peacecraft3GameCaches = {
  load(limit) {
    if (!Number.isSafeInteger(limit) || limit < 0) throw new RangeError('Invalid campaign cache limit');
    const text = localStorage.getItem(key);
    if (text === null) return undefined;
    if (text.length > Math.ceil(limit / 3) * 4) throw new RangeError('Saved campaign cache exceeds byte policy');
    const decoded = atob(text);
    if (decoded.length > limit) throw new RangeError('Decoded campaign cache exceeds byte policy');
    return Uint8Array.from(decoded, c => c.charCodeAt(0));
  },
  save(bytes) {
    if (!(bytes instanceof Uint8Array) || bytes.length > 8 * 1024 * 1024)
      throw new RangeError('Campaign cache exceeds byte policy');
    const chunks = [];
    for (let i = 0; i < bytes.length; i += 32768)
      chunks.push(String.fromCharCode(...bytes.subarray(i, i + 32768)));
    localStorage.setItem(key, btoa(chunks.join('')));
  },
};
